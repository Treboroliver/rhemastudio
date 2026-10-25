use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use encoding_rs::{UTF_8, WINDOWS_1252};
use rhema_core::{ModuleError, ModuleKind};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use walkdir::WalkDir;

const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";
const TW_ENCRYPTED_MAGIC: &[u8] = b"TWENCBMOD";
const BIBLE_PROPERTIES_MAX_BYTES: u64 = 16 * 1024 * 1024;
const BIBLE_PROPERTY_READ_CHUNK_BYTES: usize = 4 * 1024;
const BIBLE_TRAILING_NONPROPERTY_LINES_MAX: usize = 64;
const METADATA_VALUE_MAX_CHARS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleState {
    Ready,
    Protected,
    UnsupportedFormat { message: String },
    Corrupted { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub id: String,
    pub path: PathBuf,
    pub kind: ModuleKind,
    pub state: ModuleState,
    pub title: String,
    pub abbreviation: Option<String>,
    pub language: Option<String>,
    pub language_name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub size: u64,
    pub has_strong: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanWarning {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanResult {
    pub modules: Vec<ModuleInfo>,
    pub warnings: Vec<ScanWarning>,
    pub cache_hits: usize,
}

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("Falha ao gravar catálogo de módulos: {0}")]
    CacheWrite(String),
}

#[derive(Debug)]
pub struct Library {
    data_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct FolderCatalog {
    version: u32,
    modules: BTreeMap<String, CachedModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedModule {
    size: u64,
    modified_nanos: u64,
    info: ModuleInfo,
}

#[derive(Debug)]
struct FileIdentity {
    size: u64,
    modified_nanos: u64,
    id: String,
}

#[derive(Debug, Default)]
struct Metadata {
    title: Option<String>,
    abbreviation: Option<String>,
    language: Option<String>,
    description: Option<String>,
    version: Option<String>,
    has_strong: bool,
}

impl Library {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    pub fn default_folders() -> Vec<PathBuf> {
        #[cfg(windows)]
        {
            let root = PathBuf::from(r"C:\ProgramData\The Word");
            ["", "Bibles", "Commentaries", "Dictionaries", "Books"]
                .into_iter()
                .map(|child| root.join(child))
                .filter(|path| path.is_dir())
                .collect()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }

    pub fn scan(
        &self,
        folders: &[PathBuf],
        mut progress: impl FnMut(usize),
    ) -> Result<ScanResult, LibraryError> {
        let mut result = ScanResult::default();
        let mut seen_paths = BTreeSet::new();
        let mut known_modules: HashMap<String, ModuleInfo> = HashMap::new();
        let mut catalogs_to_write = Vec::new();

        for folder in folders {
            if !folder.is_dir() {
                result.warnings.push(ScanWarning {
                    path: folder.clone(),
                    message: "Pasta inexistente ou inacessível; ignorada".to_string(),
                });
                continue;
            }
            let cache_path = self.cache_path(folder);
            let catalog = read_catalog(&cache_path);
            let mut next_catalog = FolderCatalog {
                version: 2,
                modules: BTreeMap::new(),
            };

            for item in WalkDir::new(folder)
                .follow_links(false)
                .max_depth(6)
                .into_iter()
            {
                let entry = match item {
                    Ok(entry) => entry,
                    Err(error) => {
                        result.warnings.push(ScanWarning {
                            path: error.path().unwrap_or(folder).to_path_buf(),
                            message: format!("Falha ao percorrer pasta: {error}"),
                        });
                        continue;
                    }
                };
                if entry.depth() == 0 || !entry.file_type().is_file() || !is_candidate(entry.path())
                {
                    continue;
                }

                let path = entry.path().to_path_buf();
                let cache_key = path.to_string_lossy().into_owned();
                let identity = match file_identity(&path) {
                    Ok(identity) => identity,
                    Err(error) => {
                        if !seen_paths.insert(cache_key) {
                            continue;
                        }
                        let info = failed_module(&path, 0, &error.to_string());
                        result.modules.push(info);
                        progress(result.modules.len());
                        continue;
                    }
                };
                if !seen_paths.insert(cache_key.clone()) {
                    if let Some(info) = known_modules.get(&cache_key) {
                        next_catalog.modules.insert(
                            cache_key,
                            CachedModule {
                                size: identity.size,
                                modified_nanos: identity.modified_nanos,
                                info: info.clone(),
                            },
                        );
                    }
                    continue;
                }
                let cached = catalog.modules.get(&cache_key).filter(|cached| {
                    cached.size == identity.size && cached.modified_nanos == identity.modified_nanos
                });
                let info = if let Some(cached) = cached {
                    result.cache_hits += 1;
                    cached.info.clone()
                } else {
                    classify_module(&path, &identity)
                };
                next_catalog.modules.insert(
                    cache_key.clone(),
                    CachedModule {
                        size: identity.size,
                        modified_nanos: identity.modified_nanos,
                        info: info.clone(),
                    },
                );
                known_modules.insert(cache_key, info.clone());
                result.modules.push(info);
                progress(result.modules.len());
            }
            catalogs_to_write.push((cache_path, next_catalog));
        }

        for (cache_path, catalog) in catalogs_to_write {
            write_catalog(&cache_path, &catalog)?;
        }
        result
            .modules
            .sort_by(|left, right| left.path.cmp(&right.path));
        Ok(result)
    }

    pub fn list_cached(&self, folders: &[PathBuf]) -> Vec<ModuleInfo> {
        let mut modules = BTreeMap::new();
        for folder in folders {
            let catalog = read_catalog(&self.cache_path(folder));
            modules.extend(
                catalog
                    .modules
                    .into_iter()
                    .map(|(path, cached)| (path, cached.info)),
            );
        }
        modules.into_values().collect()
    }

    fn cache_path(&self, folder: &Path) -> PathBuf {
        let key = stable_hash(folder.to_string_lossy().as_bytes());
        self.data_dir.join(format!("catalog-{key:016x}.json"))
    }
}

fn read_catalog(path: &Path) -> FolderCatalog {
    let Ok(bytes) = fs::read(path) else {
        return FolderCatalog::default();
    };
    serde_json::from_slice::<FolderCatalog>(&bytes)
        .ok()
        .filter(|catalog| catalog.version == 2)
        .unwrap_or_default()
}

fn write_catalog(path: &Path, catalog: &FolderCatalog) -> Result<(), LibraryError> {
    fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))
        .map_err(|error| LibraryError::CacheWrite(error.to_string()))?;
    let bytes =
        serde_json::to_vec(catalog).map_err(|error| LibraryError::CacheWrite(error.to_string()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temp_path = path.with_extension(format!("{}.{}.tmp", std::process::id(), nonce));
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| LibraryError::CacheWrite(error.to_string()))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| LibraryError::CacheWrite(error.to_string()))?;
        fs::rename(&temp_path, path).map_err(|error| LibraryError::CacheWrite(error.to_string()))
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn file_identity(path: &Path) -> std::io::Result<FileIdentity> {
    let metadata = fs::metadata(path)?;
    let modified_nanos = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or_default();
    let mut hash_input = path.to_string_lossy().as_bytes().to_vec();
    hash_input.extend_from_slice(&metadata.len().to_le_bytes());
    hash_input.extend_from_slice(&modified_nanos.to_le_bytes());
    Ok(FileIdentity {
        size: metadata.len(),
        modified_nanos,
        id: format!("{:016x}", stable_hash(&hash_input)),
    })
}

fn stable_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn is_candidate(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".exe") || lower.ends_with(".zip") {
        return false;
    }
    [".ont", ".ot", ".nt", ".ontx", ".twm"]
        .iter()
        .any(|extension| lower.ends_with(extension))
}

fn classify_module(path: &Path, identity: &FileIdentity) -> ModuleInfo {
    let ext = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if ext == "ontx" {
        return module_info(
            path,
            identity,
            ModuleKind::Bible,
            ModuleState::Protected,
            Metadata::default(),
        );
    }

    match ext.as_str() {
        "ont" | "ot" | "nt" => classify_bible(path, identity),
        "twm" => classify_twm(path, identity),
        _ => failed_module(path, identity.size, "Extensão de módulo desconhecida"),
    }
}

fn classify_bible(path: &Path, identity: &FileIdentity) -> ModuleInfo {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => return failed_module(path, identity.size, &error.to_string()),
    };
    let mut signature = [0u8; 16];
    let read = match file.read(&mut signature) {
        Ok(read) => read,
        Err(error) => return failed_module(path, identity.size, &error.to_string()),
    };
    let signature = &signature[..read];
    if signature.starts_with(TW_ENCRYPTED_MAGIC) {
        return module_info(
            path,
            identity,
            ModuleKind::Bible,
            ModuleState::Protected,
            Metadata::default(),
        );
    }
    if signature.starts_with(SQLITE_MAGIC) || signature.is_empty() {
        return failed_module(
            path,
            identity.size,
            "Assinatura não corresponde a uma Bíblia em texto",
        );
    }
    if signature.contains(&0) {
        return failed_module(path, identity.size, "Assinatura de texto inválida");
    }

    match read_bible_metadata(&mut file, identity.size) {
        Ok(metadata) => module_info(
            path,
            identity,
            ModuleKind::Bible,
            ModuleState::Ready,
            metadata,
        ),
        Err(error) => failed_module(path, identity.size, &error),
    }
}

fn read_bible_metadata(file: &mut File, size: u64) -> Result<Metadata, String> {
    let mut cursor = size;
    let mut scanned = 0_u64;
    let mut properties = Vec::new();
    let mut skipped_footer_lines = 0;
    while cursor > 0 && scanned < BIBLE_PROPERTIES_MAX_BYTES {
        let Some((line_start, line_end)) = read_previous_line_range(file, &mut cursor)? else {
            break;
        };
        scanned = scanned.saturating_add(line_end.saturating_sub(line_start) + 1);
        let line_prefix = read_file_prefix(file, line_start, line_end, 256)?;
        let line = trim_ascii(&line_prefix);
        if line.is_empty() {
            continue;
        }
        let Some(separator) = line.iter().position(|byte| *byte == b'=') else {
            if is_trailing_nonproperty(&line_prefix)
                && skipped_footer_lines < BIBLE_TRAILING_NONPROPERTY_LINES_MAX
            {
                skipped_footer_lines += 1;
                continue;
            }
            break;
        };
        let key = trim_ascii(&line[..separator]);
        if key.is_empty()
            || !key
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(byte))
        {
            if is_trailing_nonproperty(&line_prefix)
                && skipped_footer_lines < BIBLE_TRAILING_NONPROPERTY_LINES_MAX
            {
                skipped_footer_lines += 1;
                continue;
            }
            break;
        }
        let key = key.to_ascii_lowercase();
        let key_name = std::str::from_utf8(&key).map_err(|error| error.to_string())?;
        if !is_metadata_key(key_name) {
            continue;
        }
        let leading_whitespace = line_prefix.len() - line_prefix.trim_ascii_start().len();
        let value_start = line_start + (leading_whitespace + separator + 1) as u64;
        let value = read_file_prefix(
            file,
            value_start,
            line_end,
            METADATA_VALUE_MAX_CHARS as u64 * 4,
        )?;
        properties.push((key, trim_ascii(&value).to_vec()));
    }

    let charset = properties
        .iter()
        .find_map(|(key, value)| (key.as_slice() == b"charset").then_some(value.as_slice()))
        .map(<[u8]>::to_vec);
    let mut props = HashMap::new();
    for (raw_key, value_bytes) in properties.into_iter().rev() {
        let key = String::from_utf8_lossy(&raw_key).to_ascii_lowercase();
        if !is_metadata_key(&key) {
            continue;
        }
        let value = decode_metadata_value(&value_bytes, charset.as_deref());
        props.insert(key, value);
    }
    let short_title = props.get("short.title").cloned();
    let description = props
        .get("description")
        .map(|value| value.chars().take(80).collect::<String>());
    let metadata = Metadata {
        title: description.clone().or_else(|| short_title.clone()),
        abbreviation: short_title,
        language: props.get("lang").cloned(),
        description,
        version: props
            .get("version.date")
            .or_else(|| props.get("version.major"))
            .cloned(),
        has_strong: props
            .get("strong")
            .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            || props.contains_key("untranslated.strong.format")
            || props.contains_key("verse.rule")
            || props
                .get("keywords")
                .is_some_and(|value| value.to_ascii_lowercase().contains("strong")),
    };
    Ok(metadata)
}

fn is_trailing_nonproperty(line: &[u8]) -> bool {
    line.first()
        .is_some_and(|byte| *byte == b'<' || *byte == b'#' || byte.is_ascii_whitespace())
}

fn decode_metadata_value(bytes: &[u8], charset: Option<&[u8]>) -> String {
    let decoded = match charset {
        Some(b"65001") => UTF_8.decode(bytes).0.into_owned(),
        Some(b"0") => WINDOWS_1252.decode(bytes).0.into_owned(),
        _ => match std::str::from_utf8(bytes) {
            Ok(value) => value.to_owned(),
            Err(_) => WINDOWS_1252.decode(bytes).0.into_owned(),
        },
    };
    decoded.chars().take(METADATA_VALUE_MAX_CHARS).collect()
}

fn read_previous_line_range(
    file: &mut File,
    cursor: &mut u64,
) -> Result<Option<(u64, u64)>, String> {
    if *cursor == 0 {
        return Ok(None);
    }
    let end = *cursor;
    let mut position = end;
    while position > 0 {
        let start = position.saturating_sub(BIBLE_PROPERTY_READ_CHUNK_BYTES as u64);
        let chunk_len = (position - start) as usize;
        file.seek(SeekFrom::Start(start))
            .map_err(|error| error.to_string())?;
        let mut chunk = vec![0; chunk_len];
        file.read_exact(&mut chunk)
            .map_err(|error| error.to_string())?;
        if let Some(newline) = chunk
            .iter()
            .rposition(|byte| *byte == b'\n' || *byte == b'\r')
        {
            let separator = start + newline as u64;
            *cursor = separator;
            return Ok(Some((separator + 1, end)));
        }
        position = start;
        *cursor = position;
    }
    Ok(Some((0, end)))
}

fn read_file_prefix(
    file: &mut File,
    start: u64,
    end: u64,
    max_len: u64,
) -> Result<Vec<u8>, String> {
    let length = end.saturating_sub(start).min(max_len) as usize;
    file.seek(SeekFrom::Start(start))
        .map_err(|error| error.to_string())?;
    let mut bytes = vec![0; length];
    file.read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map(|index| index + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

fn is_metadata_key(key: &str) -> bool {
    matches!(
        key,
        "title"
            | "short.title"
            | "abbrev"
            | "lang"
            | "description"
            | "version.date"
            | "version.major"
            | "strong"
            | "keywords"
            | "untranslated.strong.format"
            | "verse.rule"
            | "charset"
    )
}

fn classify_twm(path: &Path, identity: &FileIdentity) -> ModuleInfo {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => return failed_module(path, identity.size, &error.to_string()),
    };
    let mut signature = [0; 16];
    let read = match file.read(&mut signature) {
        Ok(read) => read,
        Err(error) => return failed_module(path, identity.size, &error.to_string()),
    };
    if !signature[..read].starts_with(SQLITE_MAGIC) {
        if signature[..read].starts_with(TW_ENCRYPTED_MAGIC) {
            return module_info(
                path,
                identity,
                ModuleKind::Unknown,
                ModuleState::Protected,
                Metadata::default(),
            );
        }
        return failed_module(
            path,
            identity.size,
            "Assinatura SQLite ausente ou arquivo truncado",
        );
    }

    if let Err(error) = rhema_twformat::ensure_twm_unprotected(path) {
        return match error {
            ModuleError::EncryptedModule => module_info(
                path,
                identity,
                kind_from_extension(path),
                ModuleState::Protected,
                Metadata::default(),
            ),
            other => failed_module(path, identity.size, &other.to_string()),
        };
    }
    let connection = match Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(connection) => connection,
        Err(error) => return failed_module(path, identity.size, &error.to_string()),
    };
    if let Err(error) = connection.execute_batch("PRAGMA query_only=ON;") {
        return failed_module(path, identity.size, &error.to_string());
    }
    let config_exists: Result<bool, rusqlite::Error> = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='config')",
        [],
        |row| row.get(0),
    );
    match config_exists {
        Ok(true) => {}
        Ok(false) => return failed_module(path, identity.size, "Tabela config ausente"),
        Err(error) => return failed_module(path, identity.size, &error.to_string()),
    }

    let mut metadata = Metadata::default();
    let query = "SELECT lower(trim(name)), substr(value, 1, 200) FROM config \
        WHERE lower(trim(name)) IN \
        ('title','abbrev','description','short.title','lang','version.date',\
         'type','strong','keywords','untranslated.strong.format','verse.rule')";
    let mut statement = match connection.prepare(query) {
        Ok(statement) => statement,
        Err(error) => return failed_module(path, identity.size, &error.to_string()),
    };
    let mut rows = match statement.query([]) {
        Ok(rows) => rows,
        Err(error) => return failed_module(path, identity.size, &error.to_string()),
    };
    let mut config_type = None;
    loop {
        let row = match rows.next() {
            Ok(Some(row)) => row,
            Ok(None) => break,
            Err(error) => return failed_module(path, identity.size, &error.to_string()),
        };
        let name: Result<String, _> = row.get(0);
        let value: Result<Option<String>, _> = row.get(1);
        let (Ok(name), Ok(value)) = (name, value) else {
            return failed_module(path, identity.size, "Config contém campos inválidos");
        };
        let Some(value) = value else {
            continue;
        };
        match name.as_str() {
            "title" => metadata.title = Some(value),
            "abbrev" | "short.title" => {
                if metadata.abbreviation.is_none() || name == "abbrev" {
                    metadata.abbreviation = Some(value);
                }
            }
            "description" => metadata.description = Some(value),
            "lang" => metadata.language = Some(value),
            "version.date" => metadata.version = Some(value),
            "type" => config_type = Some(value),
            "strong" => metadata.has_strong = value == "1",
            "keywords" => {
                metadata.has_strong |= value.to_ascii_lowercase().contains("strong");
            }
            "untranslated.strong.format" | "verse.rule" => metadata.has_strong = true,
            _ => {}
        }
    }

    let kind = match module_kind(path, config_type.as_deref()) {
        Ok(kind) => kind,
        Err(message) => return failed_module(path, identity.size, &message),
    };
    let state = match kind {
        ModuleKind::Commentary | ModuleKind::Dictionary => ModuleState::Ready,
        ModuleKind::Book | ModuleKind::Devotional => ModuleState::UnsupportedFormat {
            message: "Formato em breve".to_string(),
        },
        ModuleKind::Bible | ModuleKind::Unknown => ModuleState::UnsupportedFormat {
            message: "Tipo de módulo .twm não suportado".to_string(),
        },
    };
    module_info(path, identity, kind, state, metadata)
}

fn module_kind(path: &Path, config_type: Option<&str>) -> Result<ModuleKind, String> {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let extension_kind = if filename.ends_with(".cmt.twm") {
        Some(ModuleKind::Commentary)
    } else if filename.ends_with(".dct.twm") {
        Some(ModuleKind::Dictionary)
    } else if filename.ends_with(".gbk.twm") {
        Some(ModuleKind::Book)
    } else if filename.ends_with(".dev.twm") {
        Some(ModuleKind::Devotional)
    } else {
        None
    };
    let config_kind = match config_type.map(str::trim) {
        Some("1") => Some(ModuleKind::Dictionary),
        Some("2") => Some(ModuleKind::Commentary),
        Some("3") => Some(ModuleKind::Book),
        Some("") => None,
        Some(_) if extension_kind.is_some() => None,
        Some(_) => Some(ModuleKind::Unknown),
        None => None,
    };
    if let (Some(extension_kind), Some(config_kind)) = (extension_kind, config_kind)
        && extension_kind != config_kind
    {
        return Err("Extensão e config.type indicam tipos diferentes".to_string());
    }
    Ok(config_kind
        .or(extension_kind)
        .unwrap_or(ModuleKind::Unknown))
}

fn kind_from_extension(path: &Path) -> ModuleKind {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    if filename.ends_with(".cmt.twm") {
        ModuleKind::Commentary
    } else if filename.ends_with(".dct.twm") {
        ModuleKind::Dictionary
    } else if filename.ends_with(".gbk.twm") {
        ModuleKind::Book
    } else if filename.ends_with(".dev.twm") {
        ModuleKind::Devotional
    } else {
        ModuleKind::Unknown
    }
}

fn module_info(
    path: &Path,
    identity: &FileIdentity,
    kind: ModuleKind,
    state: ModuleState,
    metadata: Metadata,
) -> ModuleInfo {
    let filename = path
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Módulo".to_string());
    let fallback = filename.clone();
    let is_bible = kind == ModuleKind::Bible;
    let (language, language_name) = metadata
        .language
        .as_deref()
        .map(normalize_language)
        .map(|(code, name)| (Some(code.to_string()), Some(name.to_string())))
        .unwrap_or((None, None));
    ModuleInfo {
        id: identity.id.clone(),
        path: path.to_path_buf(),
        kind,
        state,
        title: metadata.title.unwrap_or(fallback),
        abbreviation: if is_bible {
            metadata.abbreviation.or(Some(filename))
        } else {
            metadata.abbreviation
        },
        language,
        language_name,
        description: metadata.description,
        version: metadata.version,
        size: identity.size,
        has_strong: metadata.has_strong,
    }
}

fn normalize_language(value: &str) -> (&str, &str) {
    match value.trim().to_ascii_lowercase().as_str() {
        "por" | "pt" | "pt-br" | "pt_br" | "br" => ("pt", "Português"),
        "eng" | "en" | "en-us" | "en-gb" => ("en", "Inglês"),
        "grc" | "el" | "el-gr" => ("gr", "Grego"),
        "heb" | "he" | "he-il" => ("he", "Hebraico"),
        "spa" | "es" | "es-es" | "es-mx" => ("es", "Espanhol"),
        "fra" | "fre" | "fr" => ("fr", "Francês"),
        "deu" | "ger" | "de" => ("de", "Alemão"),
        "ita" | "it" => ("it", "Italiano"),
        "rus" | "ru" => ("ru", "Russo"),
        _ => (value.trim(), value.trim()),
    }
}

fn failed_module(path: &Path, size: u64, message: &str) -> ModuleInfo {
    let modified_nanos = fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(u64::MAX as u128) as u64)
        .unwrap_or_default();
    let mut input = path.to_string_lossy().as_bytes().to_vec();
    input.extend_from_slice(&size.to_le_bytes());
    input.extend_from_slice(&modified_nanos.to_le_bytes());
    let identity = FileIdentity {
        size,
        modified_nanos,
        id: format!("{:016x}", stable_hash(&input)),
    };
    module_info(
        path,
        &identity,
        kind_from_extension(path),
        ModuleState::Corrupted {
            message: message.to_string(),
        },
        Metadata::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gen_fixtures::{
        generate_synthetic_commentary_twm, generate_synthetic_degraded,
        generate_synthetic_dictionary_twm, generate_synthetic_encrypted,
        generate_synthetic_protected_twm,
    };
    use rusqlite::params;
    use std::time::Instant;
    use tempfile::tempdir;

    fn library(root: &Path) -> Library {
        Library::new(root.join("user-data"))
    }

    fn sqlite_twm(path: &Path, kind: &str) {
        let connection = Connection::open(path).expect("fixture SQLite");
        connection
            .execute_batch("CREATE TABLE config(name TEXT, value TEXT);")
            .expect("schema config");
        connection
            .execute(
                "INSERT INTO config(name,value) VALUES \
                 ('type',?1),('title','Título sintético'),('abbrev','SINT'),\
                 ('lang','por'),('description','Descrição curta'),\
                 ('version.date','2026-01-01'),('strong','1'),\
                 ('about',?2)",
                params![kind, "x".repeat(10_000)],
            )
            .expect("valores config");
    }

    #[test]
    fn classifies_bibles_comments_dictionaries_books_and_devotionals() {
        let temp = tempdir().expect("tempdir");
        let modules = temp.path().join("modules");
        fs::create_dir(&modules).expect("module dir");
        let ont = modules.join("Synthetic.ont");
        generate_synthetic_degraded(&ont, 2).expect("synthetic Bible");
        fs::OpenOptions::new()
            .append(true)
            .open(&ont)
            .expect("append properties")
            .write_all(b"lang = por\r\n")
            .expect("append language");
        let cmt = modules.join("Synthetic.cmt.twm");
        generate_synthetic_commentary_twm(&cmt).expect("synthetic commentary");
        let dct = modules.join("Synthetic.dct.twm");
        generate_synthetic_dictionary_twm(&dct).expect("synthetic dictionary");
        let gbk = modules.join("Synthetic.gbk.twm");
        sqlite_twm(&gbk, "3");
        let dev = modules.join("Synthetic.dev.twm");
        sqlite_twm(&dev, "4");
        let other = modules.join("Synthetic.twm");
        sqlite_twm(&other, "99");

        let scan = library(temp.path()).scan(&[modules], |_| {}).expect("scan");
        let by_name = scan
            .modules
            .iter()
            .map(|module| (module.path.file_name().unwrap().to_string_lossy(), module))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(by_name["Synthetic.ont"].kind, ModuleKind::Bible);
        assert_eq!(by_name["Synthetic.ont"].state, ModuleState::Ready);
        assert_eq!(by_name["Synthetic.cmt.twm"].kind, ModuleKind::Commentary);
        assert_eq!(by_name["Synthetic.cmt.twm"].state, ModuleState::Ready);
        assert_eq!(by_name["Synthetic.dct.twm"].kind, ModuleKind::Dictionary);
        assert_eq!(by_name["Synthetic.dct.twm"].state, ModuleState::Ready);
        assert_eq!(by_name["Synthetic.gbk.twm"].kind, ModuleKind::Book);
        assert!(matches!(
            by_name["Synthetic.gbk.twm"].state,
            ModuleState::UnsupportedFormat { .. }
        ));
        assert_eq!(by_name["Synthetic.dev.twm"].kind, ModuleKind::Devotional);
        assert!(matches!(
            by_name["Synthetic.dev.twm"].state,
            ModuleState::UnsupportedFormat { .. }
        ));
        assert_eq!(by_name["Synthetic.twm"].kind, ModuleKind::Unknown);
        assert!(matches!(
            by_name["Synthetic.twm"].state,
            ModuleState::UnsupportedFormat { .. }
        ));
        assert_eq!(
            by_name["Synthetic.gbk.twm"].abbreviation.as_deref(),
            Some("SINT")
        );
        assert_eq!(by_name["Synthetic.gbk.twm"].language.as_deref(), Some("pt"));
        assert!(by_name["Synthetic.gbk.twm"].has_strong);
        assert_eq!(
            by_name["Synthetic.gbk.twm"].version.as_deref(),
            Some("2026-01-01")
        );
        let bible = by_name["Synthetic.ont"];
        assert_eq!(bible.title, "DEG");
        assert_eq!(bible.abbreviation.as_deref(), Some("DEG"));
        assert_eq!(bible.language.as_deref(), Some("pt"), "{bible:?}");
        assert!(bible.description.is_none());
        assert!(
            by_name["Synthetic.gbk.twm"]
                .description
                .as_deref()
                .is_some_and(|description| description.len() <= 200)
        );
        assert_eq!(by_name["Synthetic.cmt.twm"].title, "Synthetic.cmt");
        assert_eq!(by_name["Synthetic.dct.twm"].title, "Synthetic.dct");
    }

    #[test]
    fn ignores_non_modules_and_marks_empty_or_truncated_candidates_corrupt() {
        let temp = tempdir().expect("tempdir");
        let modules = temp.path().join("modules");
        fs::create_dir(&modules).expect("module dir");
        fs::write(modules.join("ignored.exe"), b"not a module").expect("exe");
        fs::write(modules.join("ignored.zip"), b"not a module").expect("zip");
        fs::write(modules.join("empty.cmt.twm"), []).expect("empty module");
        fs::write(
            modules.join("truncated.dct.twm"),
            b"SQLite format 3\0\x10truncated",
        )
        .expect("truncated database");
        fs::write(modules.join("broken.ont"), []).expect("empty Bible");

        let scan = library(temp.path()).scan(&[modules], |_| {}).expect("scan");
        assert_eq!(scan.modules.len(), 3);
        assert!(
            scan.modules
                .iter()
                .all(|module| matches!(module.state, ModuleState::Corrupted { .. }))
        );
    }

    #[test]
    fn reads_selected_bible_properties_using_declared_windows_charset() {
        let temp = tempdir().expect("tempdir");
        let modules = temp.path().join("modules");
        fs::create_dir(&modules).expect("module dir");
        let bytes = "Versículo sintético\n\ndescription = Bíblia de Teste\nshort.title = TST\nlang = por\ncharset = 0\nuntranslated.strong.format = 1\n";
        let (encoded, _, _) = WINDOWS_1252.encode(bytes);
        fs::write(modules.join("charset.ont"), encoded).expect("write module");
        fs::write(
            modules.join("utf8.ont"),
            "Versículo sintético\n\ndescription = Bíblia com UTF-8\nshort.title = UTF\n",
        )
        .expect("write utf8 module");
        let scan = library(temp.path()).scan(&[modules], |_| {}).expect("scan");
        assert_eq!(scan.modules.len(), 2);
        let module = scan
            .modules
            .iter()
            .find(|module| module.title == "Bíblia de Teste")
            .expect("Windows-1252 metadata");
        assert_eq!(module.title, "Bíblia de Teste");
        assert_eq!(module.abbreviation.as_deref(), Some("TST"));
        assert_eq!(module.language.as_deref(), Some("pt"));
        assert!(module.has_strong);
        assert!(scan.modules.iter().any(|module| {
            module.title == "Bíblia com UTF-8" && module.abbreviation.as_deref() == Some("UTF")
        }));
    }

    #[test]
    fn bible_metadata_uses_readable_title_language_and_keeps_duplicates() {
        let temp = tempdir().expect("tempdir");
        let modules = temp.path().join("modules");
        fs::create_dir(&modules).expect("module dir");

        let description = "Descrição legível ".repeat(8);
        let props = format!("description = {description}\r\nshort.title = TST\r\nlang = br\r\n");
        let first = modules.join("first.ont");
        generate_synthetic_degraded(&first, 1).expect("first Bible");
        OpenOptions::new()
            .append(true)
            .open(&first)
            .expect("append first metadata")
            .write_all(props.as_bytes())
            .expect("write first metadata");

        let second = modules.join("second.ont");
        generate_synthetic_degraded(&second, 2).expect("second Bible");
        OpenOptions::new()
            .append(true)
            .open(&second)
            .expect("append second metadata")
            .write_all(props.as_bytes())
            .expect("write second metadata");

        let no_description = modules.join("fallback.ont");
        generate_synthetic_degraded(&no_description, 1).expect("fallback Bible");
        OpenOptions::new()
            .append(true)
            .open(&no_description)
            .expect("append fallback metadata")
            .write_all(b"short.title = CURTA\r\nlang = eng\r\n")
            .expect("write fallback metadata");

        let filename_fallback = modules.join("filename.ont");
        fs::write(&filename_fallback, b"synthetic verse\n\nlang = xyz\r\n")
            .expect("write filename fallback Bible");

        let scan = library(temp.path())
            .scan(std::slice::from_ref(&modules), |_| {})
            .expect("scan");
        assert_eq!(scan.modules.len(), 4);
        let first_info = scan
            .modules
            .iter()
            .find(|module| module.path == first)
            .unwrap();
        assert_eq!(first_info.title.chars().count(), 80);
        assert_eq!(first_info.abbreviation.as_deref(), Some("TST"));
        assert_eq!(first_info.language.as_deref(), Some("pt"));
        assert_eq!(first_info.language_name.as_deref(), Some("Português"));
        let second_info = scan
            .modules
            .iter()
            .find(|module| module.path == second)
            .unwrap();
        assert_eq!(second_info.title, first_info.title);
        assert_ne!(second_info.size, first_info.size);
        assert_eq!(first_info.path.parent(), second_info.path.parent());
        let fallback = scan
            .modules
            .iter()
            .find(|module| module.path == no_description)
            .unwrap();
        assert_eq!(fallback.title, "CURTA");
        assert_eq!(fallback.abbreviation.as_deref(), Some("CURTA"));
        assert_eq!(fallback.language.as_deref(), Some("en"));
        assert_eq!(fallback.language_name.as_deref(), Some("Inglês"));
        let filename = scan
            .modules
            .iter()
            .find(|module| module.path == filename_fallback)
            .unwrap();
        assert_eq!(filename.title, "filename");
        assert_eq!(filename.abbreviation.as_deref(), Some("filename"));
        assert_eq!(filename.language.as_deref(), Some("xyz"));
        assert_eq!(filename.language_name.as_deref(), Some("xyz"));
    }

    #[test]
    fn normalizes_supported_language_aliases_and_preserves_unknown_codes() {
        for alias in ["por", "pt", "pt-BR", "br"] {
            assert_eq!(normalize_language(alias), ("pt", "Português"));
        }
        for alias in ["eng", "en"] {
            assert_eq!(normalize_language(alias), ("en", "Inglês"));
        }
        assert_eq!(normalize_language("grc"), ("gr", "Grego"));
        assert_eq!(normalize_language("heb"), ("he", "Hebraico"));
        assert_eq!(normalize_language("spa"), ("es", "Espanhol"));
        assert_eq!(normalize_language("xyz"), ("xyz", "xyz"));
    }

    #[test]
    fn reports_protection_without_opening_content_and_checks_ontx_extension() {
        let temp = tempdir().expect("tempdir");
        let modules = temp.path().join("modules");
        fs::create_dir(&modules).expect("module dir");
        let encrypted = modules.join("protected.ontx");
        fs::write(&encrypted, b"").expect("ontx");
        let encrypted_flag = modules.join("flagged.cmt.twm");
        generate_synthetic_protected_twm(&encrypted_flag, "do.encrypt").expect("protected fixture");
        let secure_flag = modules.join("secure.dct.twm");
        generate_synthetic_protected_twm(&secure_flag, "secure").expect("secure fixture");
        let signature = modules.join("signature.ont");
        generate_synthetic_encrypted(&signature).expect("encrypted signature");

        let scan = library(temp.path()).scan(&[modules], |_| {}).expect("scan");
        assert_eq!(scan.modules.len(), 4);
        assert!(
            scan.modules
                .iter()
                .all(|module| module.state == ModuleState::Protected)
        );
    }

    #[test]
    fn caches_metadata_invalidates_changed_files_and_repairs_corrupt_cache() {
        let temp = tempdir().expect("tempdir");
        let modules = temp.path().join("modules");
        fs::create_dir(&modules).expect("module dir");
        let module = modules.join("cache.dct.twm");
        sqlite_twm(&module, "1");
        let library = library(temp.path());
        let first = library
            .scan(std::slice::from_ref(&modules), |_| {})
            .expect("cold scan");
        assert_eq!(first.cache_hits, 0);
        let second = library
            .scan(std::slice::from_ref(&modules), |_| {})
            .expect("cached scan");
        assert_eq!(second.cache_hits, 1);
        assert_eq!(first.modules, second.modules);

        let cache_file = fs::read_dir(temp.path().join("user-data"))
            .expect("cache directory")
            .next()
            .expect("cache file")
            .expect("cache entry")
            .path();
        fs::write(&cache_file, b"{invalid json").expect("corrupt cache");
        let rebuilt = library
            .scan(std::slice::from_ref(&modules), |_| {})
            .expect("rebuild corrupt cache");
        assert_eq!(rebuilt.cache_hits, 0);
        assert_eq!(rebuilt.modules[0].title, "Título sintético");

        let connection = Connection::open(&module).expect("update fixture");
        connection
            .execute(
                "UPDATE config SET value='changed metadata with a longer value' WHERE name='title'",
                [],
            )
            .expect("update title");
        drop(connection);
        let invalidated = library
            .scan(std::slice::from_ref(&modules), |_| {})
            .expect("rescan changed module");
        assert_eq!(invalidated.cache_hits, 0);
        assert_eq!(
            invalidated.modules[0].title,
            "changed metadata with a longer value"
        );
    }

    #[test]
    fn handles_missing_folders_and_reports_progress() {
        let temp = tempdir().expect("tempdir");
        let modules = temp.path().join("modules");
        fs::create_dir(&modules).expect("module dir");
        let file = modules.join("one.ont");
        generate_synthetic_degraded(&file, 1).expect("Bible");
        let missing = temp.path().join("absent");
        let mut progress = Vec::new();
        let result = library(temp.path())
            .scan(&[missing.clone(), modules], |seen| progress.push(seen))
            .expect("scan with missing directory");
        assert_eq!(result.modules.len(), 1);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(progress, [1]);
    }

    #[test]
    fn symlink_cycle_is_not_followed_when_supported() {
        let temp = tempdir().expect("tempdir");
        let modules = temp.path().join("modules");
        fs::create_dir(&modules).expect("module dir");
        let file = modules.join("one.ont");
        generate_synthetic_degraded(&file, 1).expect("Bible");
        #[cfg(unix)]
        let link_result = std::os::unix::fs::symlink(&modules, modules.join("cycle"));
        #[cfg(windows)]
        let link_result = std::os::windows::fs::symlink_dir(&modules, modules.join("cycle"));
        if link_result.is_ok() {
            let result = library(temp.path())
                .scan(&[modules], |_| {})
                .expect("scan symlink cycle");
            assert_eq!(result.modules.len(), 1);
        }
    }

    #[test]
    fn scan_stops_at_depth_six() {
        let temp = tempdir().expect("tempdir");
        let modules = temp.path().join("modules");
        let mut nested = modules.clone();
        for level in 0..6 {
            nested.push(format!("level-{level}"));
        }
        fs::create_dir_all(&nested).expect("nested directories");
        generate_synthetic_degraded(&nested.join("too-deep.ont"), 1).expect("deep Bible");
        let result = library(temp.path())
            .scan(&[modules], |_| {})
            .expect("depth-limited scan");
        assert!(result.modules.is_empty());
    }

    #[test]
    fn fifty_module_catalog_reopens_within_budget() {
        let temp = tempdir().expect("tempdir");
        let modules = temp.path().join("modules");
        fs::create_dir(&modules).expect("module dir");
        for index in 0..50 {
            sqlite_twm(&modules.join(format!("module-{index}.dct.twm")), "1");
        }
        let library = library(temp.path());
        let cold = library
            .scan(std::slice::from_ref(&modules), |_| {})
            .expect("cold scan");
        assert_eq!(cold.modules.len(), 50);
        let started = Instant::now();
        let warm = library
            .scan(std::slice::from_ref(&modules), |_| {})
            .expect("warm scan");
        let elapsed = started.elapsed();
        eprintln!("Catálogo de 50 módulos em cache: {elapsed:?}");
        assert_eq!(warm.cache_hits, 50);
        assert!(
            elapsed.as_millis() < 100,
            "reabertura de 50 módulos demorou {elapsed:?}"
        );
    }

    #[test]
    fn reports_real_module_catalog_without_content_when_configured() {
        let Ok(root) = std::env::var("RHEMA_TEST_MODULES") else {
            return;
        };
        let root = PathBuf::from(root);
        if !root.is_dir() {
            eprintln!(
                "RHEMA_TEST_MODULES não é uma pasta acessível: {}",
                root.display()
            );
            return;
        }
        let temp = tempdir().expect("diretório temporário para cache de teste");
        let library = library(temp.path());
        let cold_start = Instant::now();
        let cold = library
            .scan(std::slice::from_ref(&root), |_| {})
            .expect("varredura a frio de módulos reais");
        let cold_elapsed = cold_start.elapsed();
        let warm_start = Instant::now();
        let warm = library
            .scan(std::slice::from_ref(&root), |_| {})
            .expect("varredura com cache de módulos reais");
        let warm_elapsed = warm_start.elapsed();
        eprintln!("M2.2 catálogo real: a frio={cold_elapsed:?}, cache={warm_elapsed:?}");
        let expected_names = [
            "ara+.ont",
            "kjv.ont",
            "acf2007.ontx",
            "holman-at.cmt.twm",
            "nicnt.cmt.twm",
            "vidanova.cmt.twm",
            "strong_amg2.dct.twm",
            "mickelson's strong.dct.twm",
        ];
        eprintln!("Nome\tTipo\tEstado\tIdioma");

        let expectations = [
            ("ara+.ont", ModuleKind::Bible, ModuleState::Ready),
            ("kjv.ont", ModuleKind::Bible, ModuleState::Ready),
            ("acf2007.ontx", ModuleKind::Bible, ModuleState::Protected),
            (
                "holman-at.cmt.twm",
                ModuleKind::Commentary,
                ModuleState::Ready,
            ),
            ("nicnt.cmt.twm", ModuleKind::Commentary, ModuleState::Ready),
            (
                "vidanova.cmt.twm",
                ModuleKind::Commentary,
                ModuleState::Ready,
            ),
            (
                "strong_amg2.dct.twm",
                ModuleKind::Dictionary,
                ModuleState::Ready,
            ),
            (
                "mickelson's strong.dct.twm",
                ModuleKind::Dictionary,
                ModuleState::Protected,
            ),
        ];
        for (name, kind, state) in expectations {
            let found = cold.modules.iter().find(|module| {
                module
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(name)
            });
            if let Some(module) = found {
                eprintln!(
                    "{}\t{:?}\t{:?}\t{}",
                    name,
                    module.kind,
                    module.state,
                    module.language.as_deref().unwrap_or("-")
                );
                assert_eq!(module.kind, kind, "tipo inesperado em {name}");
                assert_eq!(module.state, state, "estado inesperado em {name}");
                if matches!(name, "ara+.ont" | "kjv.ont") {
                    let filename = module
                        .path
                        .file_stem()
                        .map(|stem| stem.to_string_lossy())
                        .unwrap_or_default();
                    assert_ne!(
                        module.title, filename,
                        "título de exibição ausente em {name}"
                    );
                    assert!(
                        !module.title.contains('\u{fffd}'),
                        "título inválido em {name}"
                    );
                    assert!(module.title.chars().count() <= 80);
                    assert!(module.language.is_some(), "idioma ausente em {name}");
                }
            } else {
                eprintln!("{name}\tNão encontrado");
            }
        }
        let other_count = cold
            .modules
            .iter()
            .filter(|module| {
                let name = module
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                !expected_names.contains(&name.as_str())
            })
            .count();
        eprintln!("Outros módulos classificados: {other_count}");
        assert_eq!(warm.cache_hits, warm.modules.len());
    }
}
