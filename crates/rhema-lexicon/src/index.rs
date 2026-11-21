use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rhema_core::ModuleError;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use tantivy::collector::{Count, TopDocs};
use tantivy::merge_policy::NoMergePolicy;
use tantivy::query::{
    BooleanQuery, ConstScoreQuery, Occur, Query as TQuery, RegexQuery, TermQuery,
};
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value, STORED, STRING,
};
use tantivy::tokenizer::TokenStream;
use tantivy::{DocAddress, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};
use walkdir::WalkDir;

use crate::links::{classify, decoded, entries, read_db, Kind, Row};
use crate::translit::{translit_greek, translit_norm};
use crate::{
    detect_script, extract_ditat, extract_ditnt, extract_generic, extract_louw_nida,
    extract_strong, fold_greek, fold_hebrew, fold_latin, links_for_strong, parse_query,
    LexiconError, Query, Script,
};

/// Versão do esquema do índice; mudança exige reconstrução.
pub const INDEX_SCHEMA_VERSION: u32 = 1;
const METADATA_FILE: &str = "rhema-index.json";
const POINTER_FILE: &str = "CURRENT";
const GENERATION_PREFIX: &str = "gen-";
const WRITER_HEAP: usize = 100_000_000;
const PROGRESS_STEP: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LexKind {
    Strong,
    LouwNida,
    Ditat,
    Ditnt,
    Generic,
}

impl LexKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strong => "strong",
            Self::LouwNida => "louw_nida",
            Self::Ditat => "ditat",
            Self::Ditnt => "ditnt",
            Self::Generic => "generic",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "strong" => Self::Strong,
            "louw_nida" => Self::LouwNida,
            "ditat" => Self::Ditat,
            "ditnt" => Self::Ditnt,
            _ => Self::Generic,
        }
    }

    fn priority(self) -> u8 {
        match self {
            Self::Strong => 0,
            Self::LouwNida => 1,
            Self::Ditat => 2,
            Self::Ditnt => 3,
            Self::Generic => 4,
        }
    }
}

impl From<Kind> for LexKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Strong => Self::Strong,
            Kind::Louw => Self::LouwNida,
            Kind::Ditat => Self::Ditat,
            Kind::Ditnt => Self::Ditnt,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexedModule {
    pub id: String,
    pub path: String,
    pub kind: LexKind,
    pub size: u64,
    pub modified_ns: String,
    pub entries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkippedModule {
    pub path: String,
    pub size: u64,
    pub modified_ns: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    pub schema_version: u32,
    pub built_at: u64,
    pub roots: Vec<String>,
    pub modules: Vec<IndexedModule>,
    pub skipped: Vec<SkippedModule>,
}

#[derive(Debug, Clone)]
pub struct BuildProgress<'a> {
    pub module_index: usize,
    pub module_count: usize,
    pub module_id: &'a str,
    pub kind: LexKind,
    pub entries_processed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleBuildReport {
    pub id: String,
    pub kind: LexKind,
    pub entries: usize,
    pub unreadable: usize,
    pub ms: u128,
    pub file_bytes: u64,
    pub index_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexBuildReport {
    pub index_dir: String,
    pub modules: Vec<ModuleBuildReport>,
    pub protected_ignored: usize,
    pub unreadable_modules: usize,
    pub warnings: Vec<String>,
    pub scan_ms: u128,
    pub build_ms: u128,
    pub index_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    New,
    Changed,
    Removed,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleChange {
    pub path: String,
    pub id: Option<String>,
    pub change: ChangeKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexStatus {
    pub rebuild_needed: bool,
    pub changes: Vec<ModuleChange>,
}

impl IndexStatus {
    pub fn message(&self) -> Option<String> {
        let count = |kind| self.changes.iter().filter(|c| c.change == kind).count();
        self.rebuild_needed.then(|| {
            format!(
                "reconstrução necessária: {} novo(s), {} alterado(s), {} removido(s)",
                count(ChangeKind::New),
                count(ChangeKind::Changed),
                count(ChangeKind::Removed)
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchField {
    Key,
    Subentry,
    DomainPrefix,
    Link,
    Lemma,
    LemmaPrefix,
    Translit,
    KeyNorm,
    Refs,
    ReverseIndex,
    Body,
}

impl MatchField {
    fn base(self) -> f32 {
        match self {
            Self::Key => 1000.0,
            Self::Subentry => 900.0,
            Self::DomainPrefix => 800.0,
            Self::Link => 700.0,
            Self::Lemma => 600.0,
            Self::LemmaPrefix => 500.0,
            Self::Translit => 400.0,
            Self::KeyNorm => 350.0,
            Self::Refs => 200.0,
            Self::ReverseIndex => 150.0,
            Self::Body => 0.0,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Key => "key",
            Self::Subentry => "subentrada",
            Self::DomainPrefix => "dominio",
            Self::Link => "ligacao",
            Self::Lemma => "lemma",
            Self::LemmaPrefix => "lemma_prefixo",
            Self::Translit => "translit",
            Self::KeyNorm => "key_norm",
            Self::Refs => "refs",
            Self::ReverseIndex => "reverse_index",
            Self::Body => "body",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub modules: Vec<String>,
    pub kinds: Vec<LexKind>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub key: String,
    pub field: MatchField,
    pub score: f32,
    pub interpretation: String,
    pub provenance: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleHits {
    pub module_id: String,
    pub kind: LexKind,
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResults {
    pub interpretations: Vec<Query>,
    pub total: usize,
    pub shown: usize,
    pub groups: Vec<ModuleHits>,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy)]
struct Fields {
    module_id: Field,
    kind: Field,
    key: Field,
    key_norm: Field,
    lemma: Field,
    lemma_norm: Field,
    translit: Field,
    refs: Field,
    body: Field,
}

const FIELD_NAMES: [&str; 9] = [
    "module_id",
    "kind",
    "key",
    "key_norm",
    "lemma",
    "lemma_norm",
    "translit",
    "refs",
    "body",
];

fn schema() -> Schema {
    let mut builder = Schema::builder();
    for name in &FIELD_NAMES[..3] {
        builder.add_text_field(name, STRING | STORED);
    }
    for name in &FIELD_NAMES[3..8] {
        builder.add_text_field(name, STRING);
    }
    let body = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer("default")
            .set_index_option(IndexRecordOption::WithFreqs),
    );
    builder.add_text_field(FIELD_NAMES[8], body);
    builder.build()
}

fn fields(schema: &Schema) -> Result<Fields, LexiconError> {
    let get = |name: &str| {
        schema
            .get_field(name)
            .map_err(|_| LexiconError::RebuildRequired(format!("campo ausente no índice: {name}")))
    };
    Ok(Fields {
        module_id: get("module_id")?,
        kind: get("kind")?,
        key: get("key")?,
        key_norm: get("key_norm")?,
        lemma: get("lemma")?,
        lemma_norm: get("lemma_norm")?,
        translit: get("translit")?,
        refs: get("refs")?,
        body: get("body")?,
    })
}

/// Pasta de dados local do RhemaStudio (fora do repositório).
pub fn default_data_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
        })
        .map(|base| base.join("RhemaStudio"))
}

/// Pasta padrão do índice lexical.
pub fn default_index_dir() -> Option<PathBuf> {
    default_data_dir().map(|dir| dir.join("lexicon-index"))
}

pub(crate) fn ensure_outside_repository(dir: &Path) -> Result<(), LexiconError> {
    let absolute = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(dir)
    };
    let existing = absolute
        .ancestors()
        .find(|a| a.exists())
        .ok_or_else(|| LexiconError::Invalid("Pasta do índice inválida".into()))?;
    let canonical = existing.canonicalize()?;
    if canonical.ancestors().any(|a| a.join(".git").exists()) {
        return Err(LexiconError::Invalid(
            "A pasta do índice não pode ficar dentro de um repositório".into(),
        ));
    }
    Ok(())
}

fn stamp(path: &Path) -> Result<(u64, String), LexiconError> {
    let metadata = fs::metadata(path)?;
    let modified = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(|e| LexiconError::Invalid(format!("Data inválida: {e}")))?
        .as_nanos();
    Ok((metadata.len(), modified.to_string()))
}

fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default()
}

fn scan_dictionaries(roots: &[PathBuf], strict: bool) -> Result<BTreeSet<PathBuf>, LexiconError> {
    let mut paths = BTreeSet::new();
    for root in roots {
        if !root.is_dir() {
            if strict {
                return Err(LexiconError::Invalid(format!(
                    "Pasta inacessível: {}",
                    root.display()
                )));
            }
            continue;
        }
        for item in WalkDir::new(root).follow_links(false) {
            let item = match item {
                Ok(item) => item,
                Err(e) if strict => {
                    return Err(LexiconError::Invalid(format!("Falha na varredura: {e}")))
                }
                Err(_) => continue,
            };
            if item.file_type().is_file()
                && item
                    .file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .ends_with(".dct.twm")
            {
                if let Ok(path) = item.path().canonicalize() {
                    paths.insert(path);
                }
            }
        }
    }
    Ok(paths)
}

struct Found {
    id: String,
    path: PathBuf,
    path_str: String,
    kind: LexKind,
    size: u64,
    modified_ns: String,
}

fn module_kind(path: &Path, filename: &str) -> Result<LexKind, LexiconError> {
    let db = read_db(path)?;
    let title: Option<String> = db
        .query_row(
            "SELECT value FROM config WHERE lower(trim(name))='title' LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(classify(filename)
        .or_else(|| title.as_deref().and_then(classify))
        .map_or(LexKind::Generic, LexKind::from))
}

type Discovery = (Vec<Found>, Vec<SkippedModule>, Vec<String>);

fn discover(roots: &[PathBuf]) -> Result<Discovery, LexiconError> {
    if roots.is_empty() {
        return Err(LexiconError::Invalid("Informe ao menos uma pasta".into()));
    }
    let mut found: Vec<Found> = Vec::new();
    let mut skipped = Vec::new();
    let mut warnings = Vec::new();
    let mut ids = HashSet::new();
    for path in scan_dictionaries(roots, true)? {
        let path_str = path.to_string_lossy().into_owned();
        let (size, modified_ns) = stamp(&path)?;
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let kind = match module_kind(&path, &filename) {
            Ok(kind) => kind,
            Err(error) => {
                let reason = if matches!(error, LexiconError::Module(ModuleError::EncryptedModule))
                {
                    "protegido".to_string()
                } else {
                    warnings.push(format!("Módulo não analisado {path_str}: {error}"));
                    "ilegível".to_string()
                };
                skipped.push(SkippedModule {
                    path: path_str,
                    size,
                    modified_ns,
                    reason,
                });
                continue;
            }
        };
        let mut id = filename.clone();
        let mut n = 2;
        while !ids.insert(id.to_lowercase()) {
            id = format!("{filename}~{n}");
            n += 1;
        }
        found.push(Found {
            id,
            path,
            path_str,
            kind,
            size,
            modified_ns,
        });
    }
    if found.is_empty() {
        return Err(LexiconError::Invalid(
            "Nenhum dicionário legível encontrado; índice não construído".into(),
        ));
    }
    Ok((found, skipped, warnings))
}

fn add_lemma(doc: &mut TantivyDocument, f: &Fields, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    doc.add_text(f.lemma, value);
    let normalized = match detect_script(value) {
        Script::Greek => fold_greek(value),
        Script::Hebrew => fold_hebrew(value),
        _ => return,
    };
    let normalized = normalized.trim();
    if !normalized.is_empty() {
        doc.add_text(f.lemma_norm, normalized);
    }
}

fn add_translit(doc: &mut TantivyDocument, f: &Fields, value: &str) {
    let value = translit_norm(value);
    if !value.is_empty() {
        doc.add_text(f.translit, value);
    }
}

fn add_refs(doc: &mut TantivyDocument, f: &Fields, values: &[String]) {
    let unique: BTreeSet<&String> = values.iter().collect();
    for value in unique {
        doc.add_text(f.refs, value);
    }
}

fn entry_document(f: &Fields, module: &Found, row: &Row, text: &str) -> TantivyDocument {
    let mut doc = TantivyDocument::default();
    doc.add_text(f.module_id, &module.id);
    doc.add_text(f.kind, module.kind.as_str());
    doc.add_text(f.key, &row.key);
    match module.kind {
        LexKind::Strong => {
            if let Some(entry) = extract_strong(&row.key, &row.original, text) {
                doc.add_text(f.key_norm, &entry.number);
                add_lemma(&mut doc, f, &entry.lemma);
                if let Some(translit) = &entry.transliteration {
                    add_translit(&mut doc, f, translit);
                }
                add_refs(&mut doc, f, &entry.cited_numbers);
            }
        }
        LexKind::LouwNida => {
            if let Some(entry) = extract_louw_nida(&row.key, text) {
                let key = entry.entry.map_or(entry.domain.to_string(), |e| {
                    format!("{}.{e}", entry.domain)
                });
                doc.add_text(f.key_norm, key);
                for lexeme in &entry.lexemes {
                    add_lemma(&mut doc, f, lexeme);
                    add_translit(&mut doc, f, &translit_greek(lexeme));
                }
            }
        }
        LexKind::Ditat => {
            if let Some(entry) = extract_ditat(&row.key, text) {
                doc.add_text(f.key_norm, &entry.number);
                for sub in &entry.subentries {
                    doc.add_text(f.key_norm, format!("sub:{}", sub.number));
                }
                add_lemma(&mut doc, f, &entry.lemma);
                if let Some(translit) = &entry.transliteration {
                    add_translit(&mut doc, f, translit);
                }
                add_refs(&mut doc, f, &entry.strong_numbers);
            }
        }
        LexKind::Ditnt => {
            let entry = extract_ditnt(&row.key, text);
            for lemma in &entry.lemmas {
                add_lemma(&mut doc, f, lemma);
                add_translit(&mut doc, f, &translit_greek(lemma));
            }
            add_refs(&mut doc, f, &entry.initial_strong);
        }
        LexKind::Generic => {
            let title = extract_generic(&row.key);
            if !title.is_empty() {
                doc.add_text(f.key_norm, title);
            }
        }
    }
    if !text.is_empty() {
        doc.add_text(f.body, fold_latin(text));
    }
    doc
}

/// Constrói o índice em uma geração nova e só então troca o ponteiro `CURRENT`.
pub fn build_index(
    index_dir: impl AsRef<Path>,
    roots: &[PathBuf],
    progress: &mut dyn FnMut(&BuildProgress<'_>),
) -> Result<IndexBuildReport, LexiconError> {
    crate::extract::validate_patterns().map_err(LexiconError::Invalid)?;
    let index_dir = index_dir.as_ref();
    ensure_outside_repository(index_dir)?;
    let scan_start = Instant::now();
    let (modules, skipped, warnings) = discover(roots)?;
    let scan_ms = scan_start.elapsed().as_millis();
    fs::create_dir_all(index_dir)?;
    let generation = format!("{GENERATION_PREFIX}{}", now_nanos());
    let generation_dir = index_dir.join(&generation);
    let result = build_generation(&generation_dir, roots, &modules, skipped, progress);
    match result {
        Ok((module_reports, mut extra_warnings)) => {
            let pointer_tmp = index_dir.join(format!("{POINTER_FILE}.tmp"));
            fs::write(&pointer_tmp, &generation)?;
            fs::rename(&pointer_tmp, index_dir.join(POINTER_FILE))?;
            cleanup_generations(index_dir, Some(&generation));
            let mut all_warnings = warnings;
            all_warnings.append(&mut extra_warnings);
            let protected = module_reports.1;
            let unreadable_modules = module_reports.2;
            Ok(IndexBuildReport {
                index_dir: index_dir.to_string_lossy().into_owned(),
                modules: module_reports.0,
                protected_ignored: protected,
                unreadable_modules,
                warnings: all_warnings,
                scan_ms,
                build_ms: scan_start.elapsed().as_millis() - scan_ms,
                index_bytes: dir_size(&generation_dir),
            })
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&generation_dir);
            Err(error)
        }
    }
}

type GenerationOutput = ((Vec<ModuleBuildReport>, usize, usize), Vec<String>);

fn build_generation(
    generation_dir: &Path,
    roots: &[PathBuf],
    modules: &[Found],
    skipped: Vec<SkippedModule>,
    progress: &mut dyn FnMut(&BuildProgress<'_>),
) -> Result<GenerationOutput, LexiconError> {
    let tantivy_dir = generation_dir.join("tantivy");
    fs::create_dir_all(&tantivy_dir)?;
    let index = Index::create_in_dir(&tantivy_dir, schema())?;
    let f = fields(&index.schema())?;
    let mut writer: IndexWriter = index.writer(WRITER_HEAP)?;
    writer.set_merge_policy(Box::new(NoMergePolicy));
    let mut reports = Vec::new();
    let mut warnings = Vec::new();
    let mut indexed = Vec::new();
    for (module_index, module) in modules.iter().enumerate() {
        let start = Instant::now();
        let before = dir_size(&tantivy_dir);
        let report = |entries| BuildProgress {
            module_index,
            module_count: modules.len(),
            module_id: &module.id,
            kind: module.kind,
            entries_processed: entries,
        };
        progress(&report(0));
        let mut count = 0usize;
        let mut unreadable = 0usize;
        let visited = entries(&module.path, |row| {
            let text = match decoded(&row) {
                Ok(Some(text)) => text,
                Ok(None) | Err(_) => {
                    unreadable += 1;
                    String::new()
                }
            };
            writer.add_document(entry_document(&f, module, &row, &text))?;
            count += 1;
            if count.is_multiple_of(PROGRESS_STEP) {
                progress(&report(count));
            }
            Ok(())
        });
        if let Err(error) = visited {
            return Err(LexiconError::Invalid(format!(
                "Falha ao indexar {}: {error}",
                module.id
            )));
        }
        writer.commit()?;
        progress(&report(count));
        if unreadable > 0 {
            warnings.push(format!(
                "{}: {unreadable} entradas sem texto legível",
                module.id
            ));
        }
        reports.push(ModuleBuildReport {
            id: module.id.clone(),
            kind: module.kind,
            entries: count,
            unreadable,
            ms: start.elapsed().as_millis(),
            file_bytes: module.size,
            index_bytes: dir_size(&tantivy_dir).saturating_sub(before),
        });
        indexed.push(IndexedModule {
            id: module.id.clone(),
            path: module.path_str.clone(),
            kind: module.kind,
            size: module.size,
            modified_ns: module.modified_ns.clone(),
            entries: count,
        });
    }
    for module in modules {
        let current = stamp(&module.path)?;
        if current != (module.size, module.modified_ns.clone()) {
            return Err(LexiconError::Invalid(format!(
                "Módulo alterado durante a indexação: {}",
                module.id
            )));
        }
    }
    let segments = index.searchable_segment_ids()?;
    if segments.len() > 1 {
        writer.merge(&segments).wait()?;
    }
    writer.garbage_collect_files().wait()?;
    writer.wait_merging_threads()?;
    let protected = skipped.iter().filter(|s| s.reason == "protegido").count();
    let unreadable_modules = skipped.len() - protected;
    let metadata = IndexMetadata {
        schema_version: INDEX_SCHEMA_VERSION,
        built_at: (now_nanos() / 1_000_000_000) as u64,
        roots: roots
            .iter()
            .map(|r| {
                r.canonicalize()
                    .unwrap_or_else(|_| r.clone())
                    .to_string_lossy()
                    .into_owned()
            })
            .collect(),
        modules: indexed,
        skipped,
    };
    let json = serde_json::to_vec_pretty(&metadata)
        .map_err(|e| LexiconError::Invalid(format!("Metadados inválidos: {e}")))?;
    fs::write(generation_dir.join(METADATA_FILE), json)?;
    Ok(((reports, protected, unreadable_modules), warnings))
}

fn cleanup_generations(index_dir: &Path, keep: Option<&str>) {
    let Ok(items) = fs::read_dir(index_dir) else {
        return;
    };
    for item in items.flatten() {
        let name = item.file_name().to_string_lossy().into_owned();
        if name.starts_with(GENERATION_PREFIX)
            && Some(name.as_str()) != keep
            && item.path().is_dir()
        {
            let _ = fs::remove_dir_all(item.path());
        }
    }
}

struct Plan {
    field: MatchField,
    query: Box<dyn TQuery>,
    interpretation: String,
    provenance: Option<String>,
}

fn term(field: Field, value: &str) -> Box<dyn TQuery> {
    Box::new(TermQuery::new(
        Term::from_field_text(field, value),
        IndexRecordOption::Basic,
    ))
}

fn all(queries: Vec<Box<dyn TQuery>>) -> Box<dyn TQuery> {
    Box::new(BooleanQuery::new(
        queries.into_iter().map(|q| (Occur::Must, q)).collect(),
    ))
}

fn any(queries: Vec<Box<dyn TQuery>>) -> Box<dyn TQuery> {
    Box::new(BooleanQuery::new(
        queries.into_iter().map(|q| (Occur::Should, q)).collect(),
    ))
}

fn interpretation_label(query: &Query) -> String {
    match query {
        Query::Strong(v) => format!("strong:{v}"),
        Query::LouwNida(v) => format!("louw_nida:{v}"),
        Query::Ditat(v) => format!("ditat:{v}"),
        Query::Greek(v) => format!("grego:{v}"),
        Query::Hebrew(v) => format!("hebraico:{v}"),
        Query::Latin(v) => format!("latino:{v}"),
    }
}

fn natural_cmp(a: &str, b: &str) -> Ordering {
    let chunks = |s: &str| {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let digit = c.is_ascii_digit();
            match out.last_mut() {
                Some((d, chunk)) if *d == digit => chunk.push(c),
                _ => out.push((digit, c.to_string())),
            }
        }
        out
    };
    let (x, y) = (chunks(a), chunks(b));
    for ((dx, cx), (dy, cy)) in x.iter().zip(y.iter()) {
        let order = if *dx && *dy {
            let (tx, ty) = (cx.trim_start_matches('0'), cy.trim_start_matches('0'));
            tx.len().cmp(&ty.len()).then_with(|| tx.cmp(ty))
        } else {
            cx.cmp(cy)
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    x.len().cmp(&y.len())
}

pub struct LexiconIndex {
    generation_dir: PathBuf,
    metadata: IndexMetadata,
    index: Index,
    reader: IndexReader,
    f: Fields,
    links_db: Option<PathBuf>,
    reverse_db: Option<PathBuf>,
    path_to_id: HashMap<String, String>,
}

impl LexiconIndex {
    pub fn open(index_dir: impl AsRef<Path>) -> Result<Self, LexiconError> {
        let index_dir = index_dir.as_ref();
        let pointer = fs::read_to_string(index_dir.join(POINTER_FILE)).map_err(|_| {
            LexiconError::RebuildRequired(format!("índice inexistente em {}", index_dir.display()))
        })?;
        let generation = pointer.trim();
        if !generation.starts_with(GENERATION_PREFIX) || generation.contains(['/', '\\', '.']) {
            return Err(LexiconError::RebuildRequired("ponteiro inválido".into()));
        }
        let generation_dir = index_dir.join(generation);
        let raw = fs::read(generation_dir.join(METADATA_FILE))
            .map_err(|e| LexiconError::RebuildRequired(format!("metadados ausentes: {e}")))?;
        let metadata: IndexMetadata = serde_json::from_slice(&raw)
            .map_err(|e| LexiconError::RebuildRequired(format!("metadados inválidos: {e}")))?;
        if metadata.schema_version != INDEX_SCHEMA_VERSION {
            return Err(LexiconError::RebuildRequired(format!(
                "versão do esquema {} (esperada {INDEX_SCHEMA_VERSION})",
                metadata.schema_version
            )));
        }
        let index = Index::open_in_dir(generation_dir.join("tantivy"))?;
        let f = fields(&index.schema())?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        let path_to_id = metadata
            .modules
            .iter()
            .map(|m| (m.path.clone(), m.id.clone()))
            .collect();
        Ok(Self {
            generation_dir,
            metadata,
            index,
            reader,
            f,
            links_db: None,
            reverse_db: None,
            path_to_id,
        })
    }

    /// Usa o arquivo de ligações (E1a.1) nas consultas Strong, se existir.
    pub fn with_links(mut self, path: impl Into<PathBuf>) -> Self {
        self.links_db = Some(path.into());
        self
    }

    /// Usa o índice reverso palavra → Strong (E1b.1) nas consultas em latim, se existir.
    pub fn with_reverse(mut self, path: impl Into<PathBuf>) -> Self {
        self.reverse_db = Some(path.into());
        self
    }

    pub fn metadata(&self) -> &IndexMetadata {
        &self.metadata
    }

    pub fn size_bytes(&self) -> u64 {
        dir_size(&self.generation_dir)
    }

    pub fn resolve_module(&self, name: &str) -> Option<&str> {
        let name = name.trim();
        let stem = |id: &str| id.to_lowercase().trim_end_matches(".dct.twm").to_string();
        self.metadata
            .modules
            .iter()
            .find(|m| m.id.eq_ignore_ascii_case(name) || stem(&m.id) == name.to_lowercase())
            .map(|m| m.id.as_str())
    }

    /// Compara metadados com os arquivos atuais; nunca abre os módulos.
    pub fn status(&self) -> IndexStatus {
        let mut changes = Vec::new();
        let mut known = HashSet::new();
        let recorded = self
            .metadata
            .modules
            .iter()
            .map(|m| (Some(m.id.clone()), &m.path, m.size, &m.modified_ns))
            .chain(
                self.metadata
                    .skipped
                    .iter()
                    .map(|s| (None, &s.path, s.size, &s.modified_ns)),
            );
        for (id, path, size, modified) in recorded {
            known.insert(path.clone());
            let change = match stamp(Path::new(path)) {
                Err(_) => Some(ChangeKind::Removed),
                Ok((s, m)) if s != size || &m != modified => Some(ChangeKind::Changed),
                Ok(_) => None,
            };
            if let Some(change) = change {
                changes.push(ModuleChange {
                    path: path.clone(),
                    id,
                    change,
                });
            }
        }
        let roots: Vec<PathBuf> = self.metadata.roots.iter().map(PathBuf::from).collect();
        if let Ok(paths) = scan_dictionaries(&roots, false) {
            for path in paths {
                let path = path.to_string_lossy().into_owned();
                if !known.contains(&path) {
                    changes.push(ModuleChange {
                        path,
                        id: None,
                        change: ChangeKind::New,
                    });
                }
            }
        }
        IndexStatus {
            rebuild_needed: !changes.is_empty(),
            changes,
        }
    }

    fn body_terms(&self, text: &str) -> Result<Vec<String>, LexiconError> {
        let mut analyzer = self.index.tokenizer_for_field(self.f.body)?;
        let folded = fold_latin(text);
        let mut stream = analyzer.token_stream(&folded);
        let mut terms = Vec::new();
        while stream.advance() {
            terms.push(stream.token().text.clone());
        }
        Ok(terms)
    }

    fn plans(
        &self,
        query: &Query,
        filters: &SearchFilters,
        plans: &mut Vec<Plan>,
        notes: &mut Vec<String>,
    ) -> Result<(), LexiconError> {
        let f = self.f;
        let label = interpretation_label(query);
        let mut push = |field, query| {
            plans.push(Plan {
                field,
                query,
                interpretation: label.clone(),
                provenance: None,
            })
        };
        let kind = |k: LexKind| term(f.kind, k.as_str());
        match query {
            Query::Strong(number) => {
                push(
                    MatchField::Key,
                    all(vec![kind(LexKind::Strong), term(f.key_norm, number)]),
                );
                push(MatchField::Refs, term(f.refs, number));
            }
            Query::LouwNida(value) => {
                push(
                    MatchField::Key,
                    all(vec![kind(LexKind::LouwNida), term(f.key_norm, value)]),
                );
                if !value.contains('.') {
                    let regex = RegexQuery::from_pattern(&format!("{value}\\.[0-9]+"), f.key_norm)?;
                    push(
                        MatchField::DomainPrefix,
                        all(vec![kind(LexKind::LouwNida), Box::new(regex)]),
                    );
                }
            }
            Query::Ditat(value) => {
                push(
                    MatchField::Key,
                    all(vec![kind(LexKind::Ditat), term(f.key_norm, value)]),
                );
                push(
                    MatchField::Subentry,
                    all(vec![
                        kind(LexKind::Ditat),
                        term(f.key_norm, &format!("sub:{value}")),
                    ]),
                );
            }
            Query::Greek(value) | Query::Hebrew(value) => {
                let value = value.trim();
                if !value.is_empty() {
                    push(MatchField::Lemma, term(f.lemma_norm, value));
                    let regex = RegexQuery::from_pattern(
                        &format!("{}.+", regex::escape(value)),
                        f.lemma_norm,
                    )?;
                    push(MatchField::LemmaPrefix, Box::new(regex));
                }
            }
            Query::Latin(value) => {
                let translit = translit_norm(value);
                if !translit.is_empty() {
                    push(MatchField::Translit, term(f.translit, &translit));
                }
                let title = value.split_whitespace().collect::<Vec<_>>().join(" ");
                if !title.is_empty() {
                    push(MatchField::KeyNorm, term(f.key_norm, &title));
                }
                let terms: Vec<Box<dyn TQuery>> = self
                    .body_terms(value)?
                    .iter()
                    .map(|t| {
                        Box::new(TermQuery::new(
                            Term::from_field_text(f.body, t),
                            IndexRecordOption::WithFreqs,
                        )) as Box<dyn TQuery>
                    })
                    .collect();
                if !terms.is_empty() {
                    push(MatchField::Body, all(terms));
                }
            }
        }
        if let Query::Strong(number) = query {
            self.link_plans(number, filters, &label, plans, notes);
        }
        Ok(())
    }

    fn link_plans(
        &self,
        number: &str,
        filters: &SearchFilters,
        label: &str,
        plans: &mut Vec<Plan>,
        notes: &mut Vec<String>,
    ) {
        let Some(db) = self.links_db.as_ref().filter(|db| db.exists()) else {
            return;
        };
        let links = match links_for_strong(db, number) {
            Ok(links) => links,
            Err(error) => {
                notes.push(format!("Ligações indisponíveis: {error}"));
                return;
            }
        };
        let mut grouped: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
        for link in links {
            let Some(id) = self.path_to_id.get(&link.target_module) else {
                continue;
            };
            if !filters.modules.is_empty() && !filters.modules.contains(id) {
                continue;
            }
            let provenance = match &link.subentry {
                Some(sub) => format!("{} [{sub}]", link.provenance),
                None => link.provenance.clone(),
            };
            grouped
                .entry((id.clone(), link.key.clone()))
                .or_default()
                .insert(provenance);
        }
        for ((id, key), provenance) in grouped {
            plans.push(Plan {
                field: MatchField::Link,
                query: all(vec![term(self.f.module_id, &id), term(self.f.key, &key)]),
                interpretation: label.to_string(),
                provenance: Some(provenance.into_iter().collect::<Vec<_>>().join("; ")),
            });
        }
    }

    fn filtered(&self, query: Box<dyn TQuery>, filters: &SearchFilters) -> Box<dyn TQuery> {
        let mut clauses: Vec<(Occur, Box<dyn TQuery>)> = vec![(Occur::Must, query)];
        if !filters.modules.is_empty() {
            let modules = filters
                .modules
                .iter()
                .map(|m| term(self.f.module_id, m))
                .collect();
            clauses.push((
                Occur::Must,
                Box::new(ConstScoreQuery::new(any(modules), 0.0)),
            ));
        }
        if !filters.kinds.is_empty() {
            let kinds = filters
                .kinds
                .iter()
                .map(|k| term(self.f.kind, k.as_str()))
                .collect();
            clauses.push((Occur::Must, Box::new(ConstScoreQuery::new(any(kinds), 0.0))));
        }
        if clauses.len() == 1 {
            clauses.remove(0).1
        } else {
            Box::new(BooleanQuery::new(clauses))
        }
    }

    /// Busca por chave, lema, transliteração, referências e corpo; nunca devolve texto.
    pub fn search(
        &self,
        text: &str,
        limit: usize,
        filters: &SearchFilters,
    ) -> Result<SearchResults, LexiconError> {
        let limit = limit.clamp(1, 1000);
        let interpretations = parse_query(text);
        let mut plans = Vec::new();
        let mut notes = Vec::new();
        let raw = text.trim();
        if !raw.is_empty() {
            plans.push(Plan {
                field: MatchField::Key,
                query: term(self.f.key, raw),
                interpretation: "chave".into(),
                provenance: None,
            });
        }
        for query in &interpretations {
            self.plans(query, filters, &mut plans, &mut notes)?;
        }
        let searcher = self.reader.searcher();
        let mut best: HashMap<DocAddress, (f32, usize)> = HashMap::new();
        let mut meta = Vec::new();
        let mut count_queries: Vec<Box<dyn TQuery>> = Vec::with_capacity(plans.len());
        for (i, plan) in plans.into_iter().enumerate() {
            let query = self.filtered(plan.query, filters);
            let found = searcher.search(&*query, &TopDocs::with_limit(limit).order_by_score())?;
            for (score, address) in found {
                let score = match plan.field {
                    MatchField::Body => score.min(99.0),
                    field => field.base(),
                };
                let slot = best.entry(address).or_insert((score, i));
                if score > slot.0 {
                    *slot = (score, i);
                }
            }
            count_queries.push(query);
            meta.push((plan.field, plan.interpretation, plan.provenance));
        }
        let total = if count_queries.is_empty() {
            0
        } else {
            searcher.search(&*any(count_queries), &Count)?
        };
        let mut hits = Vec::new();
        for (address, (score, i)) in best {
            let doc: TantivyDocument = searcher.doc(address)?;
            let get = |field| {
                doc.get_first(field)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string()
            };
            let (field, interpretation, provenance) = &meta[i];
            hits.push((
                get(self.f.module_id),
                LexKind::parse(&get(self.f.kind)),
                SearchHit {
                    key: get(self.f.key),
                    field: *field,
                    score,
                    interpretation: interpretation.clone(),
                    provenance: provenance.clone(),
                },
            ));
        }
        hits.sort_by(|a, b| {
            b.2.score
                .total_cmp(&a.2.score)
                .then_with(|| a.1.priority().cmp(&b.1.priority()))
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| natural_cmp(&a.2.key, &b.2.key))
        });
        hits.truncate(limit);
        let shown = hits.len();
        let mut groups: Vec<ModuleHits> = Vec::new();
        for (module_id, kind, hit) in hits {
            match groups.iter_mut().find(|g| g.module_id == module_id) {
                Some(group) => group.hits.push(hit),
                None => groups.push(ModuleHits {
                    module_id,
                    kind,
                    hits: vec![hit],
                }),
            }
        }
        if let Some(sug_group) =
            self.reverse_suggestions(&interpretations, limit, filters, &searcher, &mut notes)?
        {
            let body_pos = groups
                .iter()
                .position(|g| g.hits.iter().all(|h| h.field == MatchField::Body))
                .unwrap_or(groups.len());
            groups.insert(body_pos, sug_group);
        }
        Ok(SearchResults {
            interpretations,
            total,
            shown,
            groups,
            notes,
        })
    }

    fn reverse_suggestions(
        &self,
        interpretations: &[Query],
        limit: usize,
        filters: &SearchFilters,
        searcher: &tantivy::Searcher,
        notes: &mut Vec<String>,
    ) -> Result<Option<ModuleHits>, LexiconError> {
        let Some(db) = self.reverse_db.as_ref().filter(|p| p.exists()) else {
            return Ok(None);
        };
        if !filters.kinds.is_empty() && !filters.kinds.contains(&LexKind::Strong) {
            return Ok(None);
        }
        let latin_word = interpretations.iter().find_map(|q| match q {
            Query::Latin(w) if !w.trim().contains(char::is_whitespace) => Some(w.as_str()),
            _ => None,
        });
        let Some(word) = latin_word else {
            return Ok(None);
        };
        let suggestions =
            match crate::ReverseIndex::open(db).and_then(|r| r.suggest_strongs(word, limit)) {
                Ok(s) => s,
                Err(error) => {
                    notes.push(format!("Índice reverso indisponível: {error}"));
                    return Ok(None);
                }
            };
        let mut hits = Vec::new();
        for sug in suggestions {
            let q = self.filtered(
                all(vec![
                    term(self.f.kind, LexKind::Strong.as_str()),
                    term(self.f.key_norm, &sug.strong),
                ]),
                filters,
            );
            let found = searcher.search(&*q, &TopDocs::with_limit(1).order_by_score())?;
            if let Some((_, addr)) = found.first() {
                let doc: TantivyDocument = searcher.doc(*addr)?;
                let key = doc
                    .get_first(self.f.key)
                    .and_then(|v| v.as_str())
                    .unwrap_or(&sug.strong)
                    .to_string();
                hits.push(SearchHit {
                    key,
                    field: MatchField::ReverseIndex,
                    score: MatchField::ReverseIndex.base() + (sug.dice as f32) * 40.0,
                    interpretation: format!("latino:{word}"),
                    provenance: Some(format!("reverse_index (Dice {:.3})", sug.dice)),
                });
            }
        }
        if hits.is_empty() {
            Ok(None)
        } else {
            Ok(Some(ModuleHits {
                module_id: "sugestões por palavra".into(),
                kind: LexKind::Strong,
                hits,
            }))
        }
    }
}

/// Consultas variadas e determinísticas para medir latência (sem texto de módulo).
pub fn bench_queries(count: usize) -> Vec<String> {
    const GREEK: [&str; 12] = [
        "λόγος",
        "ἀγαπάω",
        "χάρις",
        "πίστις",
        "ἀγάπη",
        "θεός",
        "κύριος",
        "πνεῦμα",
        "σάρξ",
        "δόξα",
        "λογ",
        "ἀγαπ",
    ];
    const HEBREW: [&str; 6] = ["אָב", "שָׁלוֹם", "אֱלֹהִים", "חֶסֶד", "תּוֹרָה", "בְּרִית"];
    const LATIN: [&str; 24] = [
        "agapao",
        "logos",
        "charis",
        "pistis",
        "theos",
        "pneuma",
        "amor",
        "graça",
        "graca",
        "fé",
        "esperança",
        "paz",
        "justiça",
        "pecado",
        "salvação",
        "unigênito",
        "verdade",
        "vida",
        "igreja",
        "reino",
        "luz",
        "palavra",
        "amores",
        "misericordia",
    ];
    let mut seed: u64 = 0x5eed_1e71;
    let mut next = |max: u64| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) % max
    };
    (0..count)
        .map(|i| match i % 8 {
            0 => format!("G{}", next(5624) + 1),
            1 => format!("H{}", next(8674) + 1),
            2 => format!("{}.{}", next(93) + 1, next(120) + 1),
            3 => format!("{}", next(93) + 1),
            4 => format!("DITAT {}", next(2800) + 1),
            5 => GREEK[next(GREEK.len() as u64) as usize].to_string(),
            6 => HEBREW[next(HEBREW.len() as u64) as usize].to_string(),
            _ => LATIN[next(LATIN.len() as u64) as usize].to_string(),
        })
        .collect()
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;
