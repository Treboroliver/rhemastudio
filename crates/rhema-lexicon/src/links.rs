use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

use rhema_core::{ModuleError, Node};
use rhema_twformat::decode_twm_content;
use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use thiserror::Error;
use walkdir::WalkDir;

use crate::extract::{normalized_greek, normalized_hebrew};
use crate::{extract_ditat, extract_ditnt, extract_louw_nida, extract_strong};

const APPLICATION_ID: i32 = 0x524c4558;
const SCHEMA_VERSION: i32 = 2;

#[derive(Debug, Error)]
pub enum LexiconError {
    #[error("Erro de arquivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("Erro SQLite: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("Erro de módulo: {0}")]
    Module(#[from] ModuleError),
    #[error("{0}")]
    Invalid(String),
    #[error("Erro no índice: {0}")]
    Index(#[from] tantivy::TantivyError),
    #[error("Reconstrução necessária: {0}")]
    RebuildRequired(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum LinkKind {
    Ditat,
    LouwNida,
    Ditnt,
}

impl LinkKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ditat => "ditat",
            Self::LouwNida => "louw_nida",
            Self::Ditnt => "ditnt",
        }
    }

    fn parse(value: &str) -> Result<Self, LexiconError> {
        match value {
            "ditat" => Ok(Self::Ditat),
            "louw_nida" => Ok(Self::LouwNida),
            "ditnt" => Ok(Self::Ditnt),
            _ => Err(LexiconError::Invalid(format!(
                "Tipo de ligação inválido: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    pub strong: String,
    pub strong_module: String,
    pub kind: LinkKind,
    pub target_module: String,
    pub key: String,
    pub subentry: Option<String>,
    pub provenance: String,
    pub lemma_agrees: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildReport {
    pub modules: usize,
    pub strong_entries: usize,
    pub ditat: usize,
    pub louw_nida: usize,
    pub ditnt: usize,
    pub ditat_header_entries: usize,
    pub ditat_header_comparable_entries: usize,
    pub ditat_header_agreeing_entries: usize,
    pub missing_louw_headers: usize,
    pub unreadable_entries: usize,
    pub warnings: Vec<String>,
    pub scan_ms: u128,
    pub build_ms: u128,
    pub database_bytes: u64,
    pub reused: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Strong,
    Louw,
    Ditat,
    Ditnt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Module {
    id: String,
    path: PathBuf,
    kind: Kind,
    size: u64,
    modified_ns: u128,
}

pub(crate) fn classify(name: &str) -> Option<Kind> {
    let name = name.to_lowercase();
    if name.contains("strong_amg") || name.contains("strong anotado pela amg") {
        Some(Kind::Strong)
    } else if name.contains("louw-nida") || name.contains("louw_nida") {
        Some(Kind::Louw)
    } else if name.contains("ditat") {
        Some(Kind::Ditat)
    } else if name.contains("ditnt")
        || name.trim() == "dicionário internacional de teologia do novo testamento"
    {
        Some(Kind::Ditnt)
    } else {
        None
    }
}

pub(crate) fn read_db(path: &Path) -> Result<Connection, LexiconError> {
    rhema_twformat::ensure_twm_unprotected(path)?;
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.execute_batch("PRAGMA query_only=ON")?;
    Ok(db)
}

fn discover(folders: &[PathBuf], cache: &Path) -> Result<(Vec<Module>, Vec<String>), LexiconError> {
    if folders.is_empty() {
        return Err(LexiconError::Invalid("Informe ao menos uma pasta".into()));
    }
    let mut paths = BTreeSet::new();
    for folder in folders {
        if !folder.is_dir() {
            return Err(LexiconError::Invalid(format!(
                "Pasta inacessível: {}",
                folder.display()
            )));
        }
        for item in WalkDir::new(folder).follow_links(false) {
            let item =
                item.map_err(|e| LexiconError::Invalid(format!("Falha na varredura: {e}")))?;
            if item.file_type().is_file()
                && item
                    .file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .ends_with(".dct.twm")
            {
                paths.insert(item.path().canonicalize()?);
            }
        }
    }
    if cache.exists() {
        let canonical = cache.canonicalize()?;
        if paths.contains(&canonical) {
            return Err(LexiconError::Invalid(
                "O arquivo de ligações não pode ser um módulo".into(),
            ));
        }
    }
    let mut modules = Vec::new();
    let mut warnings = Vec::new();
    for path in paths {
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let conn = match read_db(&path) {
            Ok(db) => db,
            Err(LexiconError::Module(ModuleError::EncryptedModule)) => {
                warnings.push(format!("Módulo protegido ignorado: {}", path.display()));
                continue;
            }
            Err(e) => {
                warnings.push(format!("Módulo não analisado {}: {e}", path.display()));
                continue;
            }
        };
        let title: Option<String> = conn
            .query_row(
                "SELECT value FROM config WHERE lower(trim(name))='title' LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let kind = classify(&filename).or_else(|| title.as_deref().and_then(classify));
        if let Some(kind) = kind {
            let metadata = fs::metadata(&path)?;
            let modified_ns = metadata
                .modified()?
                .duration_since(UNIX_EPOCH)
                .map_err(|e| LexiconError::Invalid(format!("Data inválida: {e}")))?
                .as_nanos();
            modules.push(Module {
                id: path.to_string_lossy().into_owned(),
                path,
                kind,
                size: metadata.len(),
                modified_ns,
            });
        }
    }
    if modules.is_empty() {
        return Err(LexiconError::Invalid(
            "Nenhum módulo lexical reconhecido".into(),
        ));
    }
    Ok((modules, warnings))
}

use rusqlite::OptionalExtension;

fn open_cache(path: &Path, create: bool) -> Result<Connection, LexiconError> {
    if path.extension().is_some_and(|e| {
        matches!(
            e.to_string_lossy().to_lowercase().as_str(),
            "twm" | "ont" | "ot" | "nt" | "ontx" | "otx" | "ntx"
        )
    }) {
        return Err(LexiconError::Invalid(
            "Arquivo de ligações não pode ser .twm".into(),
        ));
    }
    let existed = path.exists();
    if !create && !existed {
        return Err(LexiconError::Invalid(format!(
            "Arquivo de ligações não existe: {}",
            path.display()
        )));
    }
    if existed {
        let probe = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        probe.execute_batch("PRAGMA query_only=ON")?;
        let app: i32 = probe.pragma_query_value(None, "application_id", |r| r.get(0))?;
        let version: i32 = probe.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if app != APPLICATION_ID {
            return Err(LexiconError::Invalid(
                "Arquivo SQLite não é um índice lexical RhemaStudio".into(),
            ));
        }
        if version != SCHEMA_VERSION {
            return Err(LexiconError::Invalid(format!(
                "Versão de índice não suportada: {version}"
            )));
        }
    }
    let flags = if create {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    } else {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    };
    let db = Connection::open_with_flags(path, flags)?;
    if !create {
        db.execute_batch("PRAGMA query_only=ON")?;
    }
    let app: i32 = db.pragma_query_value(None, "application_id", |r| r.get(0))?;
    if existed && app != APPLICATION_ID {
        return Err(LexiconError::Invalid(
            "Arquivo SQLite não é um índice lexical RhemaStudio".into(),
        ));
    }
    let version: i32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if existed && version != SCHEMA_VERSION {
        return Err(LexiconError::Invalid(format!(
            "Versão de índice não suportada: {version}"
        )));
    }
    if !existed {
        db.execute_batch(&format!(
            "PRAGMA application_id={APPLICATION_ID}; PRAGMA user_version={SCHEMA_VERSION};
             CREATE TABLE modules(id TEXT PRIMARY KEY, size INTEGER NOT NULL, modified_ns TEXT NOT NULL);
             CREATE TABLE links(strong TEXT NOT NULL, strong_module TEXT NOT NULL, kind TEXT NOT NULL,
                target_module TEXT NOT NULL, key TEXT NOT NULL, subentry TEXT NOT NULL, provenance TEXT NOT NULL CHECK(length(provenance)>0),
                lemma_agrees INTEGER, PRIMARY KEY(strong,strong_module,kind,target_module,key,subentry,provenance));
             CREATE INDEX links_strong ON links(strong);"
        ))?;
    }
    Ok(db)
}

pub(crate) struct Row {
    pub(crate) key: String,
    pub(crate) original: String,
    pub(crate) kind: String,
    pub(crate) bytes: Option<Vec<u8>>,
}

pub(crate) fn entries<F>(path: &Path, mut visit: F) -> Result<usize, LexiconError>
where
    F: FnMut(Row) -> Result<(), LexiconError>,
{
    let db = read_db(path)?;
    let has_original: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('topics') WHERE name='strong_orig_word')",
        [],
        |r| r.get(0),
    )?;
    let has_type: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('topics') WHERE name='content_type')",
        [],
        |r| r.get(0),
    )?;
    let orig = if has_original {
        "t.strong_orig_word"
    } else {
        "NULL"
    };
    let kind = if has_type {
        "COALESCE(t.content_type,(SELECT value FROM config WHERE lower(trim(name))='content.type' LIMIT 1))"
    } else {
        "(SELECT value FROM config WHERE lower(trim(name))='content.type' LIMIT 1)"
    };
    let sql = format!("SELECT t.subject,{orig},{kind},CAST(c.data AS BLOB) FROM topics t LEFT JOIN content c ON c.topic_id=t.id ORDER BY t.id");
    let mut stmt = db.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        visit(Row {
            key: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
            original: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            kind: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            bytes: row.get(3)?,
        })?;
        count += 1;
    }
    Ok(count)
}

pub(crate) fn decoded(row: &Row) -> Result<Option<String>, ModuleError> {
    row.bytes
        .as_ref()
        .map(|bytes| {
            decode_twm_content(&row.kind.to_ascii_lowercase(), bytes).map(|nodes| {
                let mut out = String::new();
                for node in nodes {
                    if matches!(node, Node::Paragraph) {
                        if !out.ends_with('\n') && !out.is_empty() {
                            out.push('\n');
                        }
                    } else {
                        out.push_str(&node.to_plain_text());
                    }
                }
                out
            })
        })
        .transpose()
}

pub fn build_links(
    cache: impl AsRef<Path>,
    folders: &[PathBuf],
) -> Result<BuildReport, LexiconError> {
    crate::extract::validate_patterns().map_err(LexiconError::Invalid)?;
    let cache = cache.as_ref();
    let scan_start = Instant::now();
    let (modules, mut warnings) = discover(folders, cache)?;
    let scan_ms = scan_start.elapsed().as_millis();
    let build_start = Instant::now();
    let mut db = open_cache(cache, true)?;
    let mut strong: Vec<(String, String, String)> = Vec::new();
    let mut greek: HashMap<String, Vec<usize>> = HashMap::new();
    let mut unreadable = 0;
    for module in modules.iter().filter(|m| m.kind == Kind::Strong) {
        entries(&module.path, |row| {
            let text = match decoded(&row) {
                Ok(Some(text)) => text,
                Ok(None) => {
                    unreadable += 1;
                    return Ok(());
                }
                Err(e) => {
                    unreadable += 1;
                    warnings.push(format!("{} / {}: {e}", module.id, row.key));
                    return Ok(());
                }
            };
            if let Some(entry) = extract_strong(&row.key, &row.original, &text) {
                let i = strong.len();
                if entry.number.starts_with('G') {
                    let lemma = normalized_greek(&entry.lemma);
                    if !lemma.is_empty() {
                        greek.entry(lemma).or_default().push(i);
                    }
                }
                strong.push((entry.number, module.id.clone(), entry.lemma));
            }
            Ok(())
        })?;
    }
    if strong.is_empty() {
        return Err(LexiconError::Invalid(
            "Nenhum Strong reconhecido; índice não construído".into(),
        ));
    }
    let by_number: HashMap<String, Vec<usize>> = {
        let mut map: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, (number, _, _)) in strong.iter().enumerate() {
            map.entry(number.clone()).or_default().push(i);
        }
        map
    };
    let mut links = BTreeMap::<(String, String, String, String, String, String), Link>::new();
    let mut missing_headers = 0usize;
    let mut ditat_header_entries = 0usize;
    let mut ditat_header_comparable_entries = 0usize;
    let mut ditat_header_agreeing_entries = 0usize;
    for module in modules.iter().filter(|m| m.kind != Kind::Strong) {
        entries(&module.path, |row| {
            let text = match decoded(&row) {
                Ok(Some(text)) => text,
                Ok(None) => {
                    unreadable += 1;
                    return Ok(());
                }
                Err(e) => {
                    unreadable += 1;
                    warnings.push(format!("{} / {}: {e}", module.id, row.key));
                    return Ok(());
                }
            };
            let mut add = |index: usize,
                           kind: LinkKind,
                           key: &str,
                           subentry: Option<&str>,
                           provenance: &str,
                           agrees: Option<bool>| {
                let (number, source, _) = &strong[index];
                let link = Link {
                    strong: number.clone(),
                    strong_module: source.clone(),
                    kind,
                    target_module: module.id.clone(),
                    key: key.to_string(),
                    subentry: subentry.map(str::to_string),
                    provenance: provenance.to_string(),
                    lemma_agrees: agrees,
                };
                links.insert(
                    (
                        number.clone(),
                        source.clone(),
                        kind.as_str().into(),
                        module.id.clone(),
                        key.to_string(),
                        subentry.unwrap_or("").to_string(),
                    ),
                    link,
                );
            };
            match module.kind {
                Kind::Louw => {
                    if let Some(entry) = extract_louw_nida(&row.key, &text) {
                        if !entry.header_recognized {
                            missing_headers += 1;
                        }
                        for lemma in entry.lexemes {
                            if let Some(indices) = greek.get(&normalized_greek(&lemma)) {
                                for &i in indices {
                                    add(
                                        i,
                                        LinkKind::LouwNida,
                                        &row.key,
                                        None,
                                        "lema no cabeçalho",
                                        Some(true),
                                    );
                                }
                            }
                        }
                    }
                }
                Kind::Ditnt => {
                    for lemma in extract_ditnt(&row.key, &text).lemmas {
                        if let Some(indices) = greek.get(&normalized_greek(&lemma)) {
                            for &i in indices {
                                add(i, LinkKind::Ditnt, &row.key, None, "lema igual", Some(true));
                            }
                        }
                    }
                }
                Kind::Ditat => {
                    if let Some(entry) = extract_ditat(&row.key, &text) {
                        if !entry.strong_numbers.is_empty() {
                            ditat_header_entries += 1;
                            let indices: Vec<_> = entry
                                .strong_numbers
                                .iter()
                                .filter_map(|number| by_number.get(number))
                                .flatten()
                                .copied()
                                .collect();
                            if !indices.is_empty() {
                                ditat_header_comparable_entries += 1;
                            }
                            let lemma = normalized_hebrew(&entry.lemma);
                            if !lemma.is_empty()
                                && indices
                                    .iter()
                                    .any(|&i| lemma == normalized_hebrew(&strong[i].2))
                            {
                                ditat_header_agreeing_entries += 1;
                            }
                        }
                        for (subentry, lemma, numbers) in
                            std::iter::once((None, entry.lemma.as_str(), &entry.strong_numbers))
                                .chain(entry.subentries.iter().map(|s| {
                                    (Some(s.number.as_str()), s.lemma.as_str(), &s.strong_numbers)
                                }))
                        {
                            for number in numbers {
                                if let Some(indices) = by_number.get(number) {
                                    for &i in indices {
                                        let a = normalized_hebrew(lemma);
                                        let b = normalized_hebrew(&strong[i].2);
                                        add(
                                            i,
                                            LinkKind::Ditat,
                                            &row.key,
                                            subentry,
                                            "citado no verbete",
                                            Some(!a.is_empty() && a == b),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                Kind::Strong => {}
            }
            Ok(())
        })?;
    }
    for module in &modules {
        let meta = fs::metadata(&module.path)?;
        let mtime = meta
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_err(|e| LexiconError::Invalid(e.to_string()))?
            .as_nanos();
        if meta.len() != module.size || mtime != module.modified_ns {
            return Err(LexiconError::Invalid(format!(
                "Módulo alterado durante a análise: {}",
                module.path.display()
            )));
        }
    }
    let transaction = db.transaction()?;
    transaction.execute("DELETE FROM links", [])?;
    transaction.execute("DELETE FROM modules", [])?;
    for m in &modules {
        transaction.execute(
            "INSERT INTO modules VALUES (?1,?2,?3)",
            params![m.id, m.size, m.modified_ns.to_string()],
        )?;
    }
    for link in links.values() {
        transaction.execute(
            "INSERT INTO links VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                link.strong,
                link.strong_module,
                link.kind.as_str(),
                link.target_module,
                link.key,
                link.subentry.as_deref().unwrap_or(""),
                link.provenance,
                link.lemma_agrees
            ],
        )?;
    }
    transaction.commit()?;
    let count = |kind| links.values().filter(|link| link.kind == kind).count();
    Ok(BuildReport {
        modules: modules.len(),
        strong_entries: strong.len(),
        ditat: count(LinkKind::Ditat),
        louw_nida: count(LinkKind::LouwNida),
        ditnt: count(LinkKind::Ditnt),
        ditat_header_entries,
        ditat_header_comparable_entries,
        ditat_header_agreeing_entries,
        missing_louw_headers: missing_headers,
        unreadable_entries: unreadable,
        warnings,
        scan_ms,
        build_ms: build_start.elapsed().as_millis(),
        database_bytes: fs::metadata(cache)?.len(),
        reused: false,
    })
}

pub fn links_for_strong(cache: impl AsRef<Path>, query: &str) -> Result<Vec<Link>, LexiconError> {
    let number = match crate::parse_query(query).first() {
        Some(crate::Query::Strong(number)) => number.clone(),
        _ => return Err(LexiconError::Invalid("Informe número Strong válido".into())),
    };
    lookup(cache.as_ref(), &number, false)
}

pub fn links_for_ditnt(cache: impl AsRef<Path>, key: &str) -> Result<Vec<Link>, LexiconError> {
    if key.trim().is_empty() {
        return Err(LexiconError::Invalid(
            "Informe uma chave DITNT válida".into(),
        ));
    }
    lookup(cache.as_ref(), key, true)
}

fn lookup(cache: &Path, key: &str, reverse: bool) -> Result<Vec<Link>, LexiconError> {
    let db = open_cache(cache, false)?;
    let mut modules = db.prepare("SELECT id,size,modified_ns FROM modules")?;
    let mut module_rows = modules.query([])?;
    while let Some(row) = module_rows.next()? {
        let path: String = row.get(0)?;
        let size: u64 = row.get(1)?;
        let mtime: String = row.get(2)?;
        let metadata = fs::metadata(&path).map_err(|error| {
            LexiconError::Invalid(format!("Índice desatualizado: {path}: {error}"))
        })?;
        let modified_ns = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_err(|e| LexiconError::Invalid(e.to_string()))?
            .as_nanos()
            .to_string();
        if metadata.len() != size || modified_ns != mtime {
            return Err(LexiconError::Invalid(format!(
                "Índice desatualizado; reconstrua: {path}"
            )));
        }
    }
    drop(module_rows);
    drop(modules);
    let sql = if reverse {
        "SELECT strong,strong_module,kind,target_module,key,subentry,provenance,lemma_agrees FROM links WHERE kind='ditnt' AND key=?1 ORDER BY strong,strong_module"
    } else {
        "SELECT strong,strong_module,kind,target_module,key,subentry,provenance,lemma_agrees FROM links WHERE strong=?1 ORDER BY kind,target_module,key,subentry"
    };
    let mut stmt = db.prepare(sql)?;
    let mut rows = stmt.query([key])?;
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        let kind: String = row.get(2)?;
        result.push(Link {
            strong: row.get(0)?,
            strong_module: row.get(1)?,
            kind: LinkKind::parse(&kind)?,
            target_module: row.get(3)?,
            key: row.get(4)?,
            subentry: {
                let value: String = row.get(5)?;
                (!value.is_empty()).then_some(value)
            },
            provenance: row.get(6)?,
            lemma_agrees: row.get(7)?,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(
        path: &Path,
        entries: &[(&str, &str, &str)],
        protected: bool,
    ) -> Result<(), LexiconError> {
        let db = Connection::open(path)?;
        db.execute_batch("CREATE TABLE config(name TEXT, value TEXT); CREATE TABLE topics(id INTEGER,subject TEXT,strong_orig_word TEXT,content_type TEXT); CREATE TABLE content(topic_id INTEGER,data BLOB);")?;
        if protected {
            db.execute("INSERT INTO config VALUES ('secure', '1')", [])?;
        }
        for (i, (key, original, body)) in entries.iter().enumerate() {
            db.execute(
                "INSERT INTO topics VALUES (?1,?2,?3,'rtf')",
                params![i as i64, key, original],
            )?;
            db.execute(
                "INSERT INTO content VALUES (?1,?2)",
                params![i as i64, body.as_bytes()],
            )?;
        }
        Ok(())
    }

    #[test]
    fn builds_links_and_rejects_stale_modules() -> Result<(), LexiconError> {
        let dir = tempfile::tempdir()?;
        let strong = dir.path().join("strong_amg.dct.twm");
        let louw = dir.path().join("louw-nida.dct.twm");
        let ditat = dir.path().join("ditat.dct.twm");
        let ditnt = dir.path().join("ditnt.dct.twm");
        fixture(
            &strong,
            &[("G1", "λόγος", "{\\rtf1 x}"), ("H1", "אָב", "{\\rtf1 x}")],
            false,
        )?;
        fixture(&louw, &[("25.43", "", "{\\rtf1 Dominio: 25 Subdom\\u237?nio: A (25.1 a 25.9) \\u955?\\u972?\\u947?\\u959?\\u962? Significado: x}")], false)?;
        fixture(
            &ditat,
            &[(
                "100 אָב (av)",
                "",
                "{\\rtf1 <H1> x 100a \\u1488?\\u1489? <H1> x}",
            )],
            false,
        )?;
        fixture(&ditnt, &[("λόγος", "", "{\\rtf1 x}")], false)?;
        fixture(&dir.path().join("protected.dct.twm"), &[], true)?;
        let cache = dir.path().join("links.sqlite");
        let report = build_links(&cache, &[dir.path().to_path_buf()])?;
        assert_eq!(report.ditat, 2);
        assert_eq!(report.ditnt, 1);
        assert_eq!(report.louw_nida, 1);
        assert_eq!(report.warnings.len(), 1);
        let found = links_for_strong(&cache, "g001")?;
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|link| !link.provenance.is_empty()));
        let hebrew = links_for_strong(&cache, "H1")?;
        assert!(hebrew
            .iter()
            .any(|link| link.key == "100 אָב (av)" && link.subentry.as_deref() == Some("100a")));
        assert_eq!(links_for_ditnt(&cache, "λόγος")?[0].strong, "G1");
        fs::remove_file(ditnt)?;
        assert!(links_for_strong(&cache, "G1").is_err());
        let rebuilt = build_links(&cache, &[dir.path().to_path_buf()])?;
        assert_eq!(rebuilt.ditnt, 0);
        assert_eq!(links_for_strong(&cache, "G1")?.len(), 1);
        std::thread::sleep(std::time::Duration::from_millis(20));
        let db = Connection::open(&louw)?;
        db.execute(
            "UPDATE topics SET subject='25.44' WHERE subject='25.43'",
            [],
        )?;
        drop(db);
        assert!(links_for_strong(&cache, "G1").is_err());
        Ok(())
    }

    #[test]
    fn refuses_existing_other_database() -> Result<(), LexiconError> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("other.sqlite");
        let db = Connection::open(&path)?;
        db.execute_batch(
            "CREATE TABLE preserved(value INTEGER); INSERT INTO preserved VALUES (7);",
        )?;
        drop(db);
        assert!(open_cache(&path, true).is_err());
        let db = Connection::open(path)?;
        assert_eq!(
            db.query_row("SELECT value FROM preserved", [], |row| row
                .get::<_, i32>(0))?,
            7
        );
        Ok(())
    }
}
