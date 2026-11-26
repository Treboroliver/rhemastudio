use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rhema_core::{Book, ModuleKind};
use rhema_library::{Library, ModuleInfo, ModuleState};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use thiserror::Error;

mod lex;
mod reader;
mod sermon;
mod snapshot;
mod study;

use reader::{ApiError, BookInfo, ChapterDto, READER_CACHE_CAPACITY, ReaderCache, ResolveDto};
use study::{STUDY_CACHE_CAPACITY, StudyCache};

const SETTINGS_SCHEMA_VERSION: u32 = 4;

fn legacy_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct LastPassage {
    pub book: u8,
    pub chapter: u16,
    pub verse: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppSettings {
    #[serde(default = "legacy_schema_version")]
    pub schema_version: u32,
    pub module_folders: Vec<PathBuf>,
    pub default_bible_id: Option<String>,
    pub selected_bible_ids: Vec<String>,
    pub interface_language: String,
    pub theme: String,
    pub show_strong_numbers: bool,
    pub bible_font_size: u8,
    pub first_use_completed: bool,
    pub last_passage: Option<LastPassage>,
    /// Léxicos em ordem de preferência. `None`: dicionários prontos em português primeiro.
    pub lexicon_module_ids: Option<Vec<String>>,
    /// Comentários ativos. `None`: todos os comentários prontos.
    pub commentary_module_ids: Option<Vec<String>>,
    pub study_panel_open: bool,
    pub study_panel_expanded: bool,
    pub highlight_words_of_jesus: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            module_folders: Vec::new(),
            default_bible_id: None,
            selected_bible_ids: Vec::new(),
            interface_language: "pt-BR".to_string(),
            theme: "dark".to_string(),
            show_strong_numbers: false,
            bible_font_size: 20,
            first_use_completed: false,
            last_passage: None,
            lexicon_module_ids: None,
            commentary_module_ids: None,
            study_panel_open: true,
            study_panel_expanded: false,
            highlight_words_of_jesus: true,
        }
    }
}

#[derive(Debug, Error)]
enum CommandError {
    #[error("Não foi possível acessar as configurações: {0}")]
    Settings(String),
    #[error("A pasta selecionada não existe ou não está acessível: {0}")]
    InvalidFolder(String),
    #[error("O idioma da interface não é válido.")]
    InvalidLanguage,
    #[error("O tema escolhido não é válido.")]
    InvalidTheme,
    #[error("Selecione no máximo quatro Bíblias prontas.")]
    InvalidSelectedBibles,
    #[error("O tamanho da fonte deve estar entre 14 e 32 px.")]
    InvalidFontSize,
    #[error("A Bíblia padrão precisa ser uma Bíblia pronta da biblioteca.")]
    InvalidDefaultBible,
    #[error("Os léxicos precisam ser dicionários prontos da biblioteca.")]
    InvalidLexiconModules,
    #[error("Os comentários ativos precisam ser comentários prontos da biblioteca.")]
    InvalidCommentaryModules,
    #[error("A última passagem lida não é válida.")]
    InvalidPassage,
    #[error("Não foi possível atualizar a biblioteca: {0}")]
    Library(String),
    #[error("Não foi possível abrir o seletor de pastas: {0}")]
    Dialog(String),
}

struct AppState {
    library: Library,
    settings_path: PathBuf,
    settings: Mutex<AppSettings>,
    catalog: Mutex<Vec<ModuleInfo>>,
    readers: Arc<ReaderCache>,
    study: Arc<StudyCache>,
    lex: Arc<lex::LexState>,
}

impl AppState {
    fn settings(&self) -> Result<AppSettings, CommandError> {
        self.settings
            .lock()
            .map_err(|_| CommandError::Settings("estado de configurações indisponível".into()))
            .map(|settings| settings.clone())
    }

    fn save_settings(&self, settings: AppSettings) -> Result<(), CommandError> {
        let encoded = serde_json::to_vec_pretty(&settings)
            .map_err(|error| CommandError::Settings(error.to_string()))?;
        atomic_write(&self.settings_path, &encoded)
            .map_err(|error| CommandError::Settings(error.to_string()))?;
        *self
            .settings
            .lock()
            .map_err(|_| CommandError::Settings("estado de configurações indisponível".into()))? =
            settings;
        Ok(())
    }
}

fn load_settings(path: &Path) -> Result<AppSettings, CommandError> {
    match fs::read(path) {
        Ok(bytes) => {
            let mut settings: AppSettings = serde_json::from_slice(&bytes)
                .map_err(|error| CommandError::Settings(format!("arquivo inválido: {error}")))?;
            if settings.schema_version < SETTINGS_SCHEMA_VERSION {
                settings.schema_version = SETTINGS_SCHEMA_VERSION;
                let encoded = serde_json::to_vec_pretty(&settings)
                    .map_err(|error| CommandError::Settings(error.to_string()))?;
                atomic_write(path, &encoded)
                    .map_err(|error| CommandError::Settings(error.to_string()))?;
            }
            Ok(settings)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(AppSettings::default()),
        Err(error) => Err(CommandError::Settings(error.to_string())),
    }
}

fn append_lex_error_log(
    settings_path: &Path,
    scope: &str,
    module_title: &str,
    code: &str,
    message: &str,
) {
    let Some(dir) = settings_path.parent() else {
        return;
    };
    let path = dir.join("lex-errors.log");
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{ts}\t{scope}\t{module_title}\t{code}\t{message}");
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "caminho sem pasta")
    })?;
    fs::create_dir_all(parent)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(std::io::Error::other)?
        .as_nanos();
    let temp_path = path.with_extension(format!("{nonce}.tmp"));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temp_path, path)
    })();
    if result.is_err() {
        fs::remove_file(&temp_path)?;
    }
    result
}

#[cfg(not(windows))]
fn replace_file(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    fs::rename(temp_path, path)
}

#[cfg(windows)]
fn replace_file(temp_path: &Path, path: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source = temp_path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn validate_folder(path: &Path) -> Result<(), CommandError> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(CommandError::InvalidFolder(path.display().to_string()))
    }
}

fn validate_passage(passage: &LastPassage) -> Result<(), CommandError> {
    let book = Book::from_number(passage.book).ok_or(CommandError::InvalidPassage)?;
    let verses = book
        .verses_in_chapter(passage.chapter)
        .ok_or(CommandError::InvalidPassage)?;
    match passage.verse {
        Some(verse) if verse == 0 || verse > verses => Err(CommandError::InvalidPassage),
        _ => Ok(()),
    }
}

fn validate_settings(
    mut settings: AppSettings,
    current_catalog: &[ModuleInfo],
) -> Result<AppSettings, CommandError> {
    if let Some(passage) = &settings.last_passage {
        validate_passage(passage)?;
    }
    for folder in &settings.module_folders {
        validate_folder(folder)?;
    }
    settings.module_folders.sort();
    settings.module_folders.dedup();
    settings.schema_version = SETTINGS_SCHEMA_VERSION;
    if !matches!(settings.interface_language.as_str(), "pt-BR" | "en") {
        return Err(CommandError::InvalidLanguage);
    }
    if !matches!(settings.theme.as_str(), "dark" | "light" | "auto") {
        return Err(CommandError::InvalidTheme);
    }
    if settings.bible_font_size < 14 || settings.bible_font_size > 32 {
        return Err(CommandError::InvalidFontSize);
    }
    let mut selected = std::collections::HashSet::new();
    settings
        .selected_bible_ids
        .retain(|id| selected.insert(id.clone()));
    if settings.selected_bible_ids.len() > 4 {
        return Err(CommandError::InvalidSelectedBibles);
    }
    if settings.selected_bible_ids.iter().any(|id| {
        !current_catalog.iter().any(|module| {
            module.id == *id
                && module.kind == ModuleKind::Bible
                && module.state == ModuleState::Ready
        })
    }) {
        return Err(CommandError::InvalidSelectedBibles);
    }
    if let Some(id) = &settings.default_bible_id
        && !current_catalog.iter().any(|module| {
            module.id == *id
                && module.kind == ModuleKind::Bible
                && module.state == ModuleState::Ready
        })
    {
        return Err(CommandError::InvalidDefaultBible);
    }
    if let Some(ids) = &mut settings.lexicon_module_ids
        && !prune_module_ids(ids, current_catalog, ModuleKind::Dictionary)
    {
        return Err(CommandError::InvalidLexiconModules);
    }
    if let Some(ids) = &mut settings.commentary_module_ids
        && !prune_module_ids(ids, current_catalog, ModuleKind::Commentary)
    {
        return Err(CommandError::InvalidCommentaryModules);
    }
    Ok(settings)
}

/// Remove duplicados e módulos que saíram da biblioteca. Devolve `false` quando um módulo que
/// existe não é pronto (por exemplo, protegido) ou é de outro tipo.
fn prune_module_ids(ids: &mut Vec<String>, catalog: &[ModuleInfo], kind: ModuleKind) -> bool {
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
    let rejected = ids.iter().any(|id| {
        catalog
            .iter()
            .find(|module| module.id == *id)
            .is_some_and(|module| module.kind != kind || module.state != ModuleState::Ready)
    });
    ids.retain(|id| catalog.iter().any(|module| module.id == *id));
    !rejected
}

fn current_folders(state: &AppState) -> Result<Vec<PathBuf>, CommandError> {
    let settings = state.settings()?;
    let mut folders = settings.module_folders;
    for folder in Library::default_folders() {
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    Ok(folders)
}

#[tauri::command]
fn settings_get(state: State<'_, AppState>) -> Result<AppSettings, String> {
    state.settings().map_err(|error| error.to_string())
}

#[tauri::command]
fn settings_set(settings: AppSettings, state: State<'_, AppState>) -> Result<AppSettings, String> {
    let catalog = state
        .catalog
        .lock()
        .map_err(|_| "catálogo indisponível".to_string())?;
    let settings = validate_settings(settings, &catalog).map_err(|error| error.to_string())?;
    drop(catalog);
    state
        .save_settings(settings.clone())
        .map_err(|error| error.to_string())?;
    Ok(settings)
}

#[tauri::command]
fn library_scan(
    paths: Option<Vec<PathBuf>>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ModuleInfo>, String> {
    let folders = match paths {
        Some(paths) if !paths.is_empty() => paths,
        _ => current_folders(&state).map_err(|error| error.to_string())?,
    };
    let result = state
        .library
        .scan(&folders, |seen| {
            if let Err(error) = app.emit("library:progress", seen) {
                log::warn!("Falha ao emitir progresso da biblioteca: {error}");
            }
        })
        .map_err(|error| CommandError::Library(error.to_string()).to_string())?;
    if !result.warnings.is_empty() {
        let _ = app.emit("library:warnings", &result.warnings);
    }
    let modules = result.modules;
    *state
        .catalog
        .lock()
        .map_err(|_| "catálogo indisponível".to_string())? = modules.clone();
    if lex::index_status(&state.lex, &modules).state == "rebuild_needed" {
        let scan_folders = lex::collect_scan_folders(&modules, &folders);
        if !scan_folders.is_empty() {
            let lex_state = Arc::clone(&state.lex);
            let handle = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let _ = lex::build_all_indexes(&lex_state, &scan_folders, |ev| {
                    let _ = handle.emit(lex::LEX_PROGRESS_EVENT, ev);
                });
            });
        }
    }
    Ok(modules)
}

#[tauri::command]
fn library_list(state: State<'_, AppState>) -> Result<Vec<ModuleInfo>, String> {
    let cached = state
        .library
        .list_cached(&current_folders(&state).map_err(|error| error.to_string())?);
    *state
        .catalog
        .lock()
        .map_err(|_| "catálogo indisponível".to_string())? = cached.clone();
    Ok(cached)
}

#[tauri::command]
async fn get_chapter(
    module_id: String,
    book: u8,
    chapter: u16,
    state: State<'_, AppState>,
) -> Result<ChapterDto, ApiError> {
    let module = state
        .catalog
        .lock()
        .map_err(|_| ApiError::internal("catálogo indisponível"))?
        .iter()
        .find(|module| module.id == module_id)
        .cloned()
        .ok_or_else(|| ApiError {
            code: "module_not_found",
            message: "Módulo não encontrado na biblioteca.".into(),
            book: None,
            max: None,
        })?;
    let readers = Arc::clone(&state.readers);
    tauri::async_runtime::spawn_blocking(move || {
        reader::read_chapter(&readers, &module, book, chapter)
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

#[tauri::command]
async fn get_chapters(
    module_ids: Vec<String>,
    book: u8,
    chapter: u16,
    state: State<'_, AppState>,
) -> Result<reader::AlignedChaptersDto, ApiError> {
    let modules = {
        let catalog = state
            .catalog
            .lock()
            .map_err(|_| ApiError::internal("catálogo indisponível"))?;
        module_ids
            .iter()
            .map(|module_id| {
                catalog
                    .iter()
                    .find(|module| module.id == *module_id)
                    .cloned()
                    .ok_or_else(|| {
                        ApiError::new("module_not_found", "Módulo não encontrado na biblioteca.")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    if modules.len() > 4 {
        return Err(ApiError::new(
            "invalid_reference",
            "A leitura paralela aceita no máximo quatro Bíblias.",
        ));
    }
    let readers = Arc::clone(&state.readers);
    tauri::async_runtime::spawn_blocking(move || {
        reader::read_chapters(&readers, &modules, book, chapter)
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

#[tauri::command]
fn resolve_reference(text: String, lang: String) -> ResolveDto {
    reader::resolve_reference(&text, &lang)
}

fn study_snapshot(state: &AppState) -> Result<(Vec<ModuleInfo>, AppSettings), ApiError> {
    let catalog = state
        .catalog
        .lock()
        .map_err(|_| ApiError::internal("catálogo indisponível"))?
        .clone();
    let settings = state
        .settings()
        .map_err(|error| ApiError::internal(error.to_string()))?;
    Ok((catalog, settings))
}

#[tauri::command]
fn lex_index_status(state: State<'_, AppState>) -> Result<lex::LexIndexStatusDto, ApiError> {
    let (catalog, _) = study_snapshot(&state)?;
    Ok(lex::index_status(&state.lex, &catalog))
}

#[tauri::command]
async fn lex_index_build(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<lex::LexIndexStatusDto, ApiError> {
    let (catalog, settings) = study_snapshot(&state)?;
    let folders = lex::collect_scan_folders(&catalog, &settings.module_folders);
    let lex_state = Arc::clone(&state.lex);
    tauri::async_runtime::spawn_blocking(move || {
        lex::build_all_indexes(&lex_state, &folders, |ev| {
            let _ = app.emit(lex::LEX_PROGRESS_EVENT, ev);
        })
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

#[tauri::command]
async fn lex_search(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<lex::LexSearchDto, ApiError> {
    let (catalog, _) = study_snapshot(&state)?;
    let lex_state = Arc::clone(&state.lex);
    tauri::async_runtime::spawn_blocking(move || lex::search(&lex_state, &catalog, &query, limit))
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
}

#[tauri::command]
async fn lex_entry(
    module_id: String,
    key: String,
    topic_id: Option<i64>,
    state: State<'_, AppState>,
) -> Result<study::LexiconEntryDto, ApiError> {
    let (catalog, _) = study_snapshot(&state)?;
    let cache = Arc::clone(&state.study);
    tauri::async_runtime::spawn_blocking(move || {
        lex::entry(&cache, &catalog, &module_id, &key, topic_id)
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

#[tauri::command]
async fn lex_related(
    strong_id: Option<String>,
    module_id: Option<String>,
    key: Option<String>,
    topic_id: Option<i64>,
    state: State<'_, AppState>,
) -> Result<lex::LexRelatedDto, ApiError> {
    let (catalog, settings) = study_snapshot(&state)?;
    let lex_state = Arc::clone(&state.lex);
    let cache = Arc::clone(&state.study);
    tauri::async_runtime::spawn_blocking(move || {
        lex::related_for(
            &lex_state,
            &cache,
            &catalog,
            settings.lexicon_module_ids.as_deref(),
            strong_id.as_deref(),
            module_id.as_deref(),
            key.as_deref(),
            topic_id,
        )
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

#[tauri::command]
async fn strong_occurrences(
    strong: String,
    module_id: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
    state: State<'_, AppState>,
) -> Result<lex::StrongOccurrencesDto, ApiError> {
    let (catalog, _) = study_snapshot(&state)?;
    let lex_state = Arc::clone(&state.lex);
    tauri::async_runtime::spawn_blocking(move || {
        lex::strong_occurrences(
            &lex_state,
            &catalog,
            &strong,
            module_id.as_deref(),
            page,
            page_size,
        )
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

#[tauri::command]
async fn strong_translations(
    strong: String,
    module_id: Option<String>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<lex::StrongTranslationsDto, ApiError> {
    let (catalog, _) = study_snapshot(&state)?;
    let lex_state = Arc::clone(&state.lex);
    tauri::async_runtime::spawn_blocking(move || {
        lex::strong_translations(&lex_state, &catalog, &strong, module_id.as_deref(), limit)
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

#[tauri::command]
async fn lookup_strong(
    strong_id: String,
    state: State<'_, AppState>,
) -> Result<study::LexiconEntryDto, ApiError> {
    let (catalog, settings) = study_snapshot(&state)?;
    let lexicons = study::effective_lexicons(settings.lexicon_module_ids.as_deref(), &catalog);
    let cache = Arc::clone(&state.study);
    tauri::async_runtime::spawn_blocking(move || {
        study::lookup_strong(&cache, &lexicons, &strong_id)
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

#[tauri::command]
async fn get_verse_snapshot(
    module_id: String,
    range: snapshot::RangeArg,
    state: State<'_, AppState>,
) -> Result<snapshot::VerseSnapshotDto, ApiError> {
    let range = range.to_range()?;
    let (catalog, settings) = study_snapshot(&state)?;
    let module = catalog
        .into_iter()
        .find(|module| module.id == module_id)
        .ok_or_else(|| ApiError::new("module_not_found", "Módulo não encontrado na biblioteca."))?;
    let readers = Arc::clone(&state.readers);
    tauri::async_runtime::spawn_blocking(move || {
        snapshot::verse_snapshot(&readers, &module, range, &settings.interface_language)
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

#[tauri::command]
async fn get_lexicon_snapshot(
    strong_id: String,
    state: State<'_, AppState>,
) -> Result<snapshot::LexiconSnapshotDto, ApiError> {
    let (catalog, settings) = study_snapshot(&state)?;
    let lexicons = study::effective_lexicons(settings.lexicon_module_ids.as_deref(), &catalog);
    let cache = Arc::clone(&state.study);
    tauri::async_runtime::spawn_blocking(move || {
        snapshot::lexicon_snapshot(&cache, &lexicons, &strong_id)
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?
}

#[tauri::command]
async fn get_commentary_index(
    book: u8,
    chapter: u16,
    verse: u16,
    state: State<'_, AppState>,
) -> Result<study::CommentaryIndexDto, ApiError> {
    let (catalog, settings) = study_snapshot(&state)?;
    let active = study::effective_commentaries(settings.commentary_module_ids.as_deref(), &catalog);
    let cache = Arc::clone(&state.study);
    let settings_path = state.settings_path.clone();
    let dto = tauri::async_runtime::spawn_blocking(move || {
        study::commentary_index(
            &cache,
            &active,
            book,
            chapter,
            verse,
            &settings.interface_language,
        )
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))??;
    for module in &dto.modules {
        if let Some(err) = &module.error {
            append_lex_error_log(
                &settings_path,
                "commentary_index",
                &module.module_title,
                err.code,
                &err.message,
            );
        }
    }
    Ok(dto)
}

#[tauri::command]
async fn get_commentary_entry(
    module_id: String,
    topic_id: i64,
    offset: u64,
    state: State<'_, AppState>,
) -> Result<study::CommentaryPageDto, ApiError> {
    let (catalog, _) = study_snapshot(&state)?;
    let module = catalog
        .into_iter()
        .find(|module| module.id == module_id)
        .ok_or_else(|| ApiError::new("module_not_found", "Módulo não encontrado na biblioteca."))?;
    let module_title = module.title.clone();
    let settings_path = state.settings_path.clone();
    let cache = Arc::clone(&state.study);
    let result = tauri::async_runtime::spawn_blocking(move || {
        study::commentary_entry(&cache, &module, topic_id, offset)
    })
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?;
    if let Err(ref err) = result {
        append_lex_error_log(
            &settings_path,
            "commentary_entry",
            &module_title,
            err.code,
            &err.message,
        );
    }
    result
}

#[tauri::command]
fn book_names(lang: String) -> Vec<BookInfo> {
    reader::book_names(&lang)
}

#[tauri::command]
fn set_last_passage(
    passage: LastPassage,
    state: State<'_, AppState>,
) -> Result<AppSettings, String> {
    validate_passage(&passage).map_err(|error| error.to_string())?;
    let mut settings = state.settings().map_err(|error| error.to_string())?;
    settings.last_passage = Some(passage);
    state
        .save_settings(settings.clone())
        .map_err(|error| error.to_string())?;
    Ok(settings)
}

#[tauri::command]
fn library_default_folders() -> Vec<PathBuf> {
    Library::default_folders()
}

#[tauri::command]
fn pick_folders(app: AppHandle) -> Result<Vec<PathBuf>, String> {
    let Some(folders) = app
        .dialog()
        .file()
        .set_title("Escolher pastas de módulos")
        .blocking_pick_folders()
    else {
        return Ok(Vec::new());
    };
    folders
        .into_iter()
        .map(|folder| {
            folder
                .into_path()
                .map_err(|error| CommandError::Dialog(error.to_string()).to_string())
        })
        .collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            let app_data = app.path().app_data_dir()?;
            let settings_path = app_data.join("settings.json");
            let settings = load_settings(&settings_path)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            let library = Library::new(app_data.join("library"));
            app.manage(sermon::SermonState::open(&app_data));
            let cached = library.list_cached(&settings.module_folders);
            let lex_state = Arc::new(lex::LexState::new(&app_data));
            let spawn_auto = lex::index_status(&lex_state, &cached).state == "rebuild_needed";
            let auto_folders = lex::collect_scan_folders(&cached, &settings.module_folders);
            app.manage(AppState {
                library,
                settings_path,
                settings: Mutex::new(settings),
                catalog: Mutex::new(cached),
                readers: Arc::new(ReaderCache::new(READER_CACHE_CAPACITY)),
                study: Arc::new(StudyCache::new(STUDY_CACHE_CAPACITY)),
                lex: Arc::clone(&lex_state),
            });
            if spawn_auto && !auto_folders.is_empty() {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let _ = lex::build_all_indexes(&lex_state, &auto_folders, |ev| {
                        let _ = handle.emit(lex::LEX_PROGRESS_EVENT, ev);
                    });
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            library_scan,
            library_list,
            library_default_folders,
            settings_get,
            settings_set,
            pick_folders,
            get_chapter,
            get_chapters,
            lookup_strong,
            lex_index_status,
            lex_index_build,
            lex_search,
            lex_entry,
            lex_related,
            strong_occurrences,
            strong_translations,
            get_verse_snapshot,
            get_lexicon_snapshot,
            get_commentary_index,
            get_commentary_entry,
            resolve_reference,
            book_names,
            set_last_passage,
            sermon::sermon_list,
            sermon::sermon_create,
            sermon::sermon_get,
            sermon::sermon_save,
            sermon::sermon_duplicate,
            sermon::sermon_archive,
            sermon::sermon_delete,
            sermon::sermon_versions,
            sermon::sermon_restore,
            sermon::sermon_export_markdown,
            sermon::sermon_save_export
        ])
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready_bible() -> ModuleInfo {
        ModuleInfo {
            id: "bible-1".into(),
            path: PathBuf::from(r"C:\modules\sample.ont"),
            kind: ModuleKind::Bible,
            state: ModuleState::Ready,
            title: "Bíblia sintética".into(),
            abbreviation: Some("SINT".into()),
            language: Some("pt".into()),
            language_name: Some("Português".into()),
            description: None,
            version: None,
            size: 10,
            has_strong: false,
        }
    }

    #[test]
    fn settings_are_written_atomically_and_reloaded() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("settings/settings.json");
        let settings = AppSettings {
            module_folders: vec![temp.path().to_path_buf()],
            default_bible_id: None,
            ..AppSettings::default()
        };
        let bytes = serde_json::to_vec(&settings).expect("serialize");
        atomic_write(&path, &bytes).expect("atomic settings write");
        let updated = AppSettings {
            theme: "light".into(),
            ..settings
        };
        let bytes = serde_json::to_vec(&updated).expect("serialize updated");
        atomic_write(&path, &bytes).expect("replace settings atomically");
        assert_eq!(load_settings(&path).expect("reload"), updated);
        assert_eq!(
            fs::read_dir(path.parent().expect("settings parent"))
                .expect("settings directory")
                .count(),
            1
        );
    }

    #[test]
    fn legacy_settings_load_with_versioned_defaults() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("settings.json");
        fs::write(
            &path,
            r#"{"module_folders":[],"default_bible_id":null,"interface_language":"pt-BR","theme":"dark","first_use_completed":true}"#,
        )
        .expect("write legacy settings");
        let settings = load_settings(&path).expect("load legacy settings");
        assert_eq!(settings.schema_version, 4);
        assert!(settings.selected_bible_ids.is_empty());
        assert!(!settings.show_strong_numbers);
        assert_eq!(settings.bible_font_size, 20);
        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.lexicon_module_ids, None);
        assert_eq!(settings.commentary_module_ids, None);
        assert!(settings.study_panel_open);
        assert!(settings.highlight_words_of_jesus);
        let persisted = fs::read_to_string(path).expect("read migrated settings");
        assert!(persisted.contains("\"schema_version\": 4"));
        assert!(persisted.contains("\"selected_bible_ids\": []"));
        assert!(persisted.contains("\"study_panel_open\": true"));
    }

    #[test]
    fn version_two_settings_gain_study_defaults_without_losing_values() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("settings.json");
        fs::write(
            &path,
            r#"{"schema_version":2,"module_folders":[],"default_bible_id":"bible-1","selected_bible_ids":["bible-1"],"interface_language":"en","theme":"light","show_strong_numbers":true,"bible_font_size":24,"first_use_completed":true,"last_passage":{"book":43,"chapter":3,"verse":16}}"#,
        )
        .expect("write version two settings");
        let settings = load_settings(&path).expect("migrate version two settings");
        assert_eq!(settings.schema_version, 4);
        assert_eq!(settings.theme, "light");
        assert_eq!(settings.bible_font_size, 24);
        assert!(settings.show_strong_numbers);
        assert_eq!(settings.selected_bible_ids, ["bible-1"]);
        assert_eq!(settings.lexicon_module_ids, None);
        assert_eq!(settings.commentary_module_ids, None);
        assert!(settings.study_panel_open);
        assert_eq!(load_settings(&path).expect("reload migrated"), settings);
    }

    fn study_module(id: &str, kind: ModuleKind, state: ModuleState, language: &str) -> ModuleInfo {
        let mut module = ready_bible();
        module.id = id.into();
        module.kind = kind;
        module.state = state;
        module.language = Some(language.into());
        module
    }

    #[test]
    fn study_module_lists_are_validated_pruned_and_never_accept_protected_modules() {
        let catalog = vec![
            study_module("dict-pt", ModuleKind::Dictionary, ModuleState::Ready, "pt"),
            study_module(
                "dict-locked",
                ModuleKind::Dictionary,
                ModuleState::Protected,
                "pt",
            ),
            study_module("cmt-1", ModuleKind::Commentary, ModuleState::Ready, "pt"),
            study_module(
                "cmt-locked",
                ModuleKind::Commentary,
                ModuleState::Protected,
                "pt",
            ),
        ];
        let valid = AppSettings {
            lexicon_module_ids: Some(vec!["dict-pt".into(), "dict-pt".into(), "gone".into()]),
            commentary_module_ids: Some(vec!["cmt-1".into()]),
            ..AppSettings::default()
        };
        let validated = validate_settings(valid, &catalog).expect("valid study settings");
        assert_eq!(
            validated.lexicon_module_ids,
            Some(vec!["dict-pt".to_string()])
        );
        assert_eq!(
            validated.commentary_module_ids,
            Some(vec!["cmt-1".to_string()])
        );

        let locked_lexicon = AppSettings {
            lexicon_module_ids: Some(vec!["dict-locked".into()]),
            ..AppSettings::default()
        };
        assert!(matches!(
            validate_settings(locked_lexicon, &catalog),
            Err(CommandError::InvalidLexiconModules)
        ));
        let locked_commentary = AppSettings {
            commentary_module_ids: Some(vec!["cmt-locked".into()]),
            ..AppSettings::default()
        };
        assert!(matches!(
            validate_settings(locked_commentary, &catalog),
            Err(CommandError::InvalidCommentaryModules)
        ));
        let wrong_kind = AppSettings {
            commentary_module_ids: Some(vec!["dict-pt".into()]),
            ..AppSettings::default()
        };
        assert!(matches!(
            validate_settings(wrong_kind, &catalog),
            Err(CommandError::InvalidCommentaryModules)
        ));
    }

    #[test]
    fn settings_reject_invalid_values_and_missing_folders() {
        let temp = tempfile::tempdir().expect("tempdir");
        let settings = AppSettings {
            interface_language: "fr".into(),
            ..AppSettings::default()
        };
        assert!(matches!(
            validate_settings(settings, &[]),
            Err(CommandError::InvalidLanguage)
        ));
        let settings = AppSettings {
            theme: "sepia".into(),
            ..AppSettings::default()
        };
        assert!(matches!(
            validate_settings(settings, &[]),
            Err(CommandError::InvalidTheme)
        ));
        let settings = AppSettings {
            module_folders: vec![temp.path().join("does-not-exist")],
            ..AppSettings::default()
        };
        assert!(matches!(
            validate_settings(settings, &[]),
            Err(CommandError::InvalidFolder(_))
        ));
        let settings = AppSettings {
            default_bible_id: Some("missing".into()),
            ..AppSettings::default()
        };
        assert!(matches!(
            validate_settings(settings, &[ready_bible()]),
            Err(CommandError::InvalidDefaultBible)
        ));
    }

    #[test]
    fn settings_validate_parallel_columns_font_and_auto_theme() {
        let bibles = ["bible-1", "bible-2", "bible-3", "bible-4", "bible-5"]
            .into_iter()
            .map(|id| {
                let mut module = ready_bible();
                module.id = id.into();
                module
            })
            .collect::<Vec<_>>();
        let valid = AppSettings {
            theme: "auto".into(),
            selected_bible_ids: vec!["bible-2".into(), "bible-1".into()],
            default_bible_id: Some("bible-2".into()),
            ..AppSettings::default()
        };
        let validated = validate_settings(valid, &bibles).expect("valid parallel settings");
        assert_eq!(validated.schema_version, 4);
        assert_eq!(validated.selected_bible_ids, ["bible-2", "bible-1"]);

        let too_many = AppSettings {
            selected_bible_ids: bibles.iter().map(|module| module.id.clone()).collect(),
            ..AppSettings::default()
        };
        assert!(matches!(
            validate_settings(too_many, &bibles),
            Err(CommandError::InvalidSelectedBibles)
        ));
        let invalid_font = AppSettings {
            bible_font_size: 33,
            ..AppSettings::default()
        };
        assert!(matches!(
            validate_settings(invalid_font, &bibles),
            Err(CommandError::InvalidFontSize)
        ));
        let protected = AppSettings {
            selected_bible_ids: vec!["locked".into()],
            ..AppSettings::default()
        };
        let mut locked = ready_bible();
        locked.id = "locked".into();
        locked.state = ModuleState::Protected;
        assert!(matches!(
            validate_settings(protected, &[locked]),
            Err(CommandError::InvalidSelectedBibles)
        ));
    }

    #[test]
    fn module_info_serializes_as_interface_ready_catalog_data() {
        let module = ready_bible();
        let view = serde_json::to_value(&module).expect("serialize interface module");
        assert_eq!(view["title"], "Bíblia sintética");
        assert_eq!(view["language"], "pt");
        assert_eq!(view["language_name"], "Português");
        assert_eq!(view["kind"], "Bible");
        assert_eq!(view["state"], "Ready");
        assert!(view["path"].is_string());
    }
}
