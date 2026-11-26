use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rhema_core::{ModuleError, Node, StrongLang};
use rhema_twformat::{tokenize_markup, OntReader};
use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use walkdir::WalkDir;

use crate::index::{default_data_dir, ensure_outside_repository};
use crate::{fold_latin, LexiconError};

const APPLICATION_ID: i32 = 0x52524556;
const SCHEMA_VERSION: i32 = 1;

/// Caminho padrão do índice reverso palavra → Strong (fora do repositório).
pub fn default_reverse_path() -> Option<PathBuf> {
    default_data_dir().map(|dir| dir.join("lexicon-reverse.sqlite"))
}

#[derive(Debug, Clone, Serialize)]
pub struct BibleReverseStats {
    pub id: String,
    pub path: String,
    pub verses_read: usize,
    pub distinct_words: usize,
    pub pairs: usize,
    pub ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReverseBuildReport {
    pub cache_path: String,
    pub bibles: Vec<BibleReverseStats>,
    pub protected_ignored: usize,
    pub unreadable_modules: usize,
    pub warnings: Vec<String>,
    pub scan_ms: u128,
    pub build_ms: u128,
    pub database_bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StrongSuggestion {
    pub strong: String,
    pub dice: f64,
    pub n_ws: u64,
    pub n_w: u64,
    pub n_s: u64,
    pub modules: Vec<String>,
}

fn is_latin_letter(c: char) -> bool {
    c.is_ascii_alphabetic()
        || (c.is_alphabetic() && matches!(c as u32, 0x00c0..=0x024f | 0x1e00..=0x1eff))
}

/// Tokeniza palavras latinas com apóstrofo ou hífen internos, usando `fold_latin`.
/// Ignora números e palavras com menos de 2 letras; sem lista de stopwords.
pub fn tokenize_latin_words(text: &str) -> Vec<String> {
    let folded = fold_latin(text);
    let chars: Vec<char> = folded.chars().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut letters = 0usize;

    for (i, &c) in chars.iter().enumerate() {
        if is_latin_letter(c) {
            current.push(c);
            letters += 1;
        } else if matches!(c, '\'' | '’' | '-')
            && !current.is_empty()
            && chars.get(i + 1).copied().is_some_and(is_latin_letter)
        {
            current.push(if c == '’' { '\'' } else { c });
        } else if !current.is_empty() {
            if letters >= 2 {
                tokens.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
            letters = 0;
        }
    }
    if !current.is_empty() && letters >= 2 {
        tokens.push(current);
    }
    tokens
}

pub(crate) fn collect_reverse_nodes(
    nodes: &[Node],
    active_strongs: &mut Vec<String>,
    pairs: &mut HashMap<(String, String), u64>,
    word_totals: &mut HashMap<String, u64>,
    strong_totals: &mut HashMap<String, u64>,
) {
    collect_nodes(nodes, active_strongs, pairs, word_totals, strong_totals);
}

pub(crate) struct CollectedBibleReverse {
    pub id: String,
    pub path: String,
    pub size: u64,
    pub modified_ns: String,
    pub verses_read: usize,
    pub pairs: HashMap<(String, String), u64>,
    pub word_totals: HashMap<String, u64>,
    pub strong_totals: HashMap<String, u64>,
    pub ms: u128,
}

pub(crate) fn build_reverse_from_collected(
    cache_path: &Path,
    collected: Vec<CollectedBibleReverse>,
    protected_ignored: usize,
    unreadable_modules: usize,
    warnings: Vec<String>,
    scan_ms: u128,
    build_start: Instant,
) -> Result<ReverseBuildReport, LexiconError> {
    if collected.is_empty() {
        return Err(LexiconError::Invalid(
            "Nenhuma Bíblia legível com tags Strong encontrada".into(),
        ));
    }
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = cache_path.with_extension(format!("tmp.{}", now_nanos()));
    let write_res: Result<Vec<BibleReverseStats>, LexiconError> = (|| {
        let mut db = Connection::open_with_flags(
            &tmp_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        db.execute_batch(&format!(
            "PRAGMA application_id={APPLICATION_ID};
             PRAGMA user_version={SCHEMA_VERSION};
             PRAGMA journal_mode=OFF;
             PRAGMA synchronous=OFF;
             CREATE TABLE modules(
                 id TEXT PRIMARY KEY,
                 path TEXT NOT NULL,
                 size INTEGER NOT NULL,
                 modified_ns TEXT NOT NULL,
                 verses INTEGER NOT NULL,
                 distinct_words INTEGER NOT NULL,
                 pairs INTEGER NOT NULL
             );
             CREATE TABLE word_totals(
                 module_id TEXT NOT NULL,
                 word TEXT NOT NULL,
                 n_w INTEGER NOT NULL,
                 PRIMARY KEY(module_id, word)
             );
             CREATE TABLE strong_totals(
                 module_id TEXT NOT NULL,
                 strong TEXT NOT NULL,
                 n_s INTEGER NOT NULL,
                 PRIMARY KEY(module_id, strong)
             );
             CREATE TABLE cooccurrences(
                 module_id TEXT NOT NULL,
                 word TEXT NOT NULL,
                 strong TEXT NOT NULL,
                 n_ws INTEGER NOT NULL,
                 PRIMARY KEY(module_id, word, strong)
             );"
        ))?;

        let mut bibles = Vec::with_capacity(collected.len());
        for item in collected {
            let distinct_words = item.word_totals.len();
            let pair_count = item.pairs.len();
            let tx = db.transaction()?;
            tx.execute(
                "INSERT INTO modules VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![
                    item.id,
                    item.path,
                    item.size as i64,
                    item.modified_ns,
                    item.verses_read as i64,
                    distinct_words as i64,
                    pair_count as i64
                ],
            )?;
            {
                let mut stmt_w = tx.prepare("INSERT INTO word_totals VALUES (?1,?2,?3)")?;
                for (word, n_w) in &item.word_totals {
                    stmt_w.execute(params![item.id, word, *n_w as i64])?;
                }
            }
            {
                let mut stmt_s = tx.prepare("INSERT INTO strong_totals VALUES (?1,?2,?3)")?;
                for (strong, n_s) in &item.strong_totals {
                    stmt_s.execute(params![item.id, strong, *n_s as i64])?;
                }
            }
            {
                let mut stmt_c = tx.prepare("INSERT INTO cooccurrences VALUES (?1,?2,?3,?4)")?;
                for ((word, strong), n_ws) in &item.pairs {
                    stmt_c.execute(params![item.id, word, strong, *n_ws as i64])?;
                }
            }
            tx.commit()?;
            bibles.push(BibleReverseStats {
                id: item.id,
                path: item.path,
                verses_read: item.verses_read,
                distinct_words,
                pairs: pair_count,
                ms: item.ms,
            });
        }
        db.execute_batch(
            "CREATE INDEX idx_cooc_word ON cooccurrences(word);
             CREATE INDEX idx_wt_word ON word_totals(word);
             CREATE INDEX idx_st_strong ON strong_totals(strong);",
        )?;
        Ok(bibles)
    })();

    match write_res {
        Ok(bibles) => {
            fs::rename(&tmp_path, cache_path)?;
            Ok(ReverseBuildReport {
                cache_path: cache_path.to_string_lossy().into_owned(),
                bibles,
                protected_ignored,
                unreadable_modules,
                warnings,
                scan_ms,
                build_ms: build_start.elapsed().as_millis(),
                database_bytes: fs::metadata(cache_path)?.len(),
            })
        }
        Err(error) => {
            let _ = fs::remove_file(&tmp_path);
            Err(error)
        }
    }
}

fn collect_nodes(
    nodes: &[Node],
    active_strongs: &mut Vec<String>,
    pairs: &mut HashMap<(String, String), u64>,
    word_totals: &mut HashMap<String, u64>,
    strong_totals: &mut HashMap<String, u64>,
) {
    for node in nodes {
        match node {
            Node::Text(text) => {
                if active_strongs.is_empty() {
                    continue;
                }
                for word in tokenize_latin_words(text) {
                    *word_totals.entry(word.clone()).or_insert(0) += 1;
                    for strong in active_strongs.iter() {
                        *strong_totals.entry(strong.clone()).or_insert(0) += 1;
                        *pairs.entry((word.clone(), strong.clone())).or_insert(0) += 1;
                    }
                }
            }
            Node::Strong {
                lang,
                num,
                children,
            } => {
                let prefix = match lang {
                    StrongLang::Greek => 'G',
                    StrongLang::Hebrew => 'H',
                };
                let tag = format!("{prefix}{num}");
                let pushed = if !active_strongs.contains(&tag) {
                    active_strongs.push(tag);
                    true
                } else {
                    false
                };
                collect_nodes(children, active_strongs, pairs, word_totals, strong_totals);
                if pushed {
                    active_strongs.pop();
                }
            }
            Node::Italic(c)
            | Node::Bold(c)
            | Node::Underline(c)
            | Node::Strikethrough(c)
            | Node::Caps(c)
            | Node::Superscript(c)
            | Node::Subscript(c)
            | Node::WordsOfJesus(c)
            | Node::OtQuote(c)
            | Node::Heading(_, c)
            | Node::Link { children: c, .. }
            | Node::UnknownTag { children: c, .. } => {
                collect_nodes(c, active_strongs, pairs, word_totals, strong_totals);
            }
            Node::Note(_)
            | Node::XRef(_)
            | Node::Grammar { .. }
            | Node::Morph(_)
            | Node::Paragraph
            | Node::LineBreak => {}
        }
    }
}

fn stamp(path: &Path) -> Result<(u64, String), LexiconError> {
    let meta = fs::metadata(path)?;
    let modified = meta
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(|e| LexiconError::Invalid(format!("Data inválida: {e}")))?
        .as_nanos();
    Ok((meta.len(), modified.to_string()))
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default()
}

fn is_bible_extension(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".ont") || lower.ends_with(".ot") || lower.ends_with(".nt")
}

fn is_protected_bible_extension(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".ontx") || lower.ends_with(".otx") || lower.ends_with(".ntx")
}

/// Constrói o índice reverso palavra → Strong em arquivo temporário com troca atômica.
pub fn build_reverse_index(
    cache_path: impl AsRef<Path>,
    roots: &[PathBuf],
) -> Result<ReverseBuildReport, LexiconError> {
    if roots.is_empty() {
        return Err(LexiconError::Invalid("Informe ao menos uma pasta".into()));
    }
    let cache_path = cache_path.as_ref();
    ensure_outside_repository(cache_path)?;
    if cache_path.extension().is_some_and(|e| {
        matches!(
            e.to_string_lossy().to_lowercase().as_str(),
            "twm" | "ont" | "ot" | "nt" | "ontx" | "otx" | "ntx"
        )
    }) {
        return Err(LexiconError::Invalid(
            "O arquivo do índice reverso não pode substituir um módulo".into(),
        ));
    }

    let scan_start = Instant::now();
    let mut candidate_paths = BTreeSet::new();
    let mut protected_ignored = 0usize;
    for root in roots {
        if !root.is_dir() {
            return Err(LexiconError::Invalid(format!(
                "Pasta inacessível: {}",
                root.display()
            )));
        }
        for item in WalkDir::new(root).follow_links(false) {
            let item =
                item.map_err(|e| LexiconError::Invalid(format!("Falha na varredura: {e}")))?;
            if !item.file_type().is_file() {
                continue;
            }
            let name = item.file_name().to_string_lossy();
            if is_protected_bible_extension(&name) {
                protected_ignored += 1;
            } else if is_bible_extension(&name) {
                if let Ok(canon) = item.path().canonicalize() {
                    candidate_paths.insert(canon);
                }
            }
        }
    }
    let scan_ms = scan_start.elapsed().as_millis();

    let build_start = Instant::now();
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = cache_path.with_extension(format!("tmp.{}", now_nanos()));
    let result = build_reverse_into(
        &tmp_path,
        &candidate_paths,
        &mut protected_ignored,
        scan_ms,
        build_start,
        cache_path,
    );
    match result {
        Ok(report) => {
            fs::rename(&tmp_path, cache_path)?;
            Ok(ReverseBuildReport {
                database_bytes: fs::metadata(cache_path)?.len(),
                ..report
            })
        }
        Err(error) => {
            let _ = fs::remove_file(&tmp_path);
            Err(error)
        }
    }
}

fn build_reverse_into(
    tmp_path: &Path,
    candidate_paths: &BTreeSet<PathBuf>,
    protected_ignored: &mut usize,
    scan_ms: u128,
    build_start: Instant,
    final_path: &Path,
) -> Result<ReverseBuildReport, LexiconError> {
    let mut db = Connection::open_with_flags(
        tmp_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;
    db.execute_batch(&format!(
        "PRAGMA application_id={APPLICATION_ID};
         PRAGMA user_version={SCHEMA_VERSION};
         PRAGMA journal_mode=OFF;
         PRAGMA synchronous=OFF;
         CREATE TABLE modules(
             id TEXT PRIMARY KEY,
             path TEXT NOT NULL,
             size INTEGER NOT NULL,
             modified_ns TEXT NOT NULL,
             verses INTEGER NOT NULL,
             distinct_words INTEGER NOT NULL,
             pairs INTEGER NOT NULL
         );
         CREATE TABLE word_totals(
             module_id TEXT NOT NULL,
             word TEXT NOT NULL,
             n_w INTEGER NOT NULL,
             PRIMARY KEY(module_id, word)
         );
         CREATE TABLE strong_totals(
             module_id TEXT NOT NULL,
             strong TEXT NOT NULL,
             n_s INTEGER NOT NULL,
             PRIMARY KEY(module_id, strong)
         );
         CREATE TABLE cooccurrences(
             module_id TEXT NOT NULL,
             word TEXT NOT NULL,
             strong TEXT NOT NULL,
             n_ws INTEGER NOT NULL,
             PRIMARY KEY(module_id, word, strong)
         );"
    ))?;

    let mut bibles = Vec::new();
    let mut warnings = Vec::new();
    let mut unreadable_modules = 0usize;
    let mut used_ids = BTreeSet::new();

    for path in candidate_paths {
        let mod_start = Instant::now();
        let (size, modified_ns) = stamp(path)?;
        let reader = match OntReader::open(path) {
            Ok(r) => r,
            Err(ModuleError::EncryptedModule) => {
                *protected_ignored += 1;
                continue;
            }
            Err(error) => {
                unreadable_modules += 1;
                warnings.push(format!("Módulo não analisado {}: {error}", path.display()));
                continue;
            }
        };

        let mut pairs: HashMap<(String, String), u64> = HashMap::new();
        let mut word_totals: HashMap<String, u64> = HashMap::new();
        let mut strong_totals: HashMap<String, u64> = HashMap::new();
        let mut active_strongs = Vec::new();
        let verses_read = reader.verse_count();

        for idx in 0..verses_read {
            let Ok(line) = reader.line_by_index(idx) else {
                continue;
            };
            if !line.contains("<W") && !line.contains("<w") {
                continue;
            }
            active_strongs.clear();
            let nodes = tokenize_markup(&line);
            collect_nodes(
                &nodes,
                &mut active_strongs,
                &mut pairs,
                &mut word_totals,
                &mut strong_totals,
            );
        }

        if pairs.is_empty() {
            continue;
        }

        let current_stamp = stamp(path)?;
        if current_stamp != (size, modified_ns.clone()) {
            return Err(LexiconError::Invalid(format!(
                "Módulo alterado durante a indexação: {}",
                path.display()
            )));
        }

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "bible".into());
        let mut id = filename.clone();
        let mut n = 2;
        while !used_ids.insert(id.to_lowercase()) {
            id = format!("{filename}~{n}");
            n += 1;
        }

        let distinct_words = word_totals.len();
        let pair_count = pairs.len();
        let path_str = path.to_string_lossy().into_owned();

        let tx = db.transaction()?;
        tx.execute(
            "INSERT INTO modules VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                id,
                path_str,
                size as i64,
                modified_ns,
                verses_read as i64,
                distinct_words as i64,
                pair_count as i64
            ],
        )?;
        {
            let mut stmt_w = tx.prepare("INSERT INTO word_totals VALUES (?1,?2,?3)")?;
            for (word, n_w) in &word_totals {
                stmt_w.execute(params![id, word, *n_w as i64])?;
            }
        }
        {
            let mut stmt_s = tx.prepare("INSERT INTO strong_totals VALUES (?1,?2,?3)")?;
            for (strong, n_s) in &strong_totals {
                stmt_s.execute(params![id, strong, *n_s as i64])?;
            }
        }
        {
            let mut stmt_c = tx.prepare("INSERT INTO cooccurrences VALUES (?1,?2,?3,?4)")?;
            for ((word, strong), n_ws) in &pairs {
                stmt_c.execute(params![id, word, strong, *n_ws as i64])?;
            }
        }
        tx.commit()?;

        bibles.push(BibleReverseStats {
            id,
            path: path_str,
            verses_read,
            distinct_words,
            pairs: pair_count,
            ms: mod_start.elapsed().as_millis(),
        });
    }

    if bibles.is_empty() {
        return Err(LexiconError::Invalid(
            "Nenhuma Bíblia legível com tags Strong encontrada".into(),
        ));
    }

    db.execute_batch(
        "CREATE INDEX idx_cooc_word ON cooccurrences(word);
         CREATE INDEX idx_wt_word ON word_totals(word);
         CREATE INDEX idx_st_strong ON strong_totals(strong);",
    )?;
    drop(db);

    Ok(ReverseBuildReport {
        cache_path: final_path.to_string_lossy().into_owned(),
        bibles,
        protected_ignored: *protected_ignored,
        unreadable_modules,
        warnings,
        scan_ms,
        build_ms: build_start.elapsed().as_millis(),
        database_bytes: 0,
    })
}

pub struct ReverseIndex {
    db: Connection,
}

impl ReverseIndex {
    pub fn open(cache_path: impl AsRef<Path>) -> Result<Self, LexiconError> {
        let path = cache_path.as_ref();
        if !path.exists() {
            return Err(LexiconError::RebuildRequired(format!(
                "índice reverso inexistente em {}",
                path.display()
            )));
        }
        let db = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        db.execute_batch("PRAGMA query_only=ON")?;
        let app: i32 = db.pragma_query_value(None, "application_id", |r| r.get(0))?;
        let version: i32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if app != APPLICATION_ID {
            return Err(LexiconError::RebuildRequired(
                "arquivo SQLite não é um índice reverso RhemaStudio".into(),
            ));
        }
        if version != SCHEMA_VERSION {
            return Err(LexiconError::RebuildRequired(format!(
                "versão de índice reverso {version} (esperada {SCHEMA_VERSION})"
            )));
        }
        let mut stmt = db.prepare("SELECT path, size, modified_ns FROM modules")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let mod_path: String = row.get(0)?;
            let size: i64 = row.get(1)?;
            let modified_ns: String = row.get(2)?;
            let Ok((cur_size, cur_mod)) = stamp(Path::new(&mod_path)) else {
                return Err(LexiconError::RebuildRequired(format!(
                    "módulo ausente: {mod_path}"
                )));
            };
            if cur_size as i64 != size || cur_mod != modified_ns {
                return Err(LexiconError::RebuildRequired(format!(
                    "módulo alterado: {mod_path}"
                )));
            }
        }
        drop(rows);
        drop(stmt);
        Ok(Self { db })
    }

    pub fn suggest_strongs(
        &self,
        word: &str,
        limit: usize,
    ) -> Result<Vec<StrongSuggestion>, LexiconError> {
        let tokens = tokenize_latin_words(word);
        let Some(token) = tokens.first() else {
            return Ok(Vec::new());
        };
        let limit = limit.clamp(1, 500);

        let n_w: u64 = self
            .db
            .prepare_cached("SELECT COALESCE(SUM(n_w), 0) FROM word_totals WHERE word = ?1")?
            .query_row([token], |r| r.get::<_, i64>(0))?
            .max(0) as u64;
        if n_w == 0 {
            return Ok(Vec::new());
        }

        let mut stmt = self.db.prepare_cached(
            "SELECT c.strong,
                    SUM(c.n_ws) AS total_ws,
                    (SELECT COALESCE(SUM(st.n_s), 0) FROM strong_totals st WHERE st.strong = c.strong) AS total_s,
                    GROUP_CONCAT(DISTINCT c.module_id) AS mods
             FROM cooccurrences c
             WHERE c.word = ?1
             GROUP BY c.strong",
        )?;
        let mut rows = stmt.query([token])?;
        let mut candidates = Vec::new();
        while let Some(row) = rows.next()? {
            let strong: String = row.get(0)?;
            let n_ws = row.get::<_, i64>(1)?.max(0) as u64;
            let n_s = row.get::<_, i64>(2)?.max(0) as u64;
            let mods_raw: String = row.get(3)?;
            let mut modules: Vec<String> = mods_raw
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            modules.sort();
            let denom = (n_w + n_s).max(1) as f64;
            let dice = (2.0 * n_ws as f64) / denom;
            candidates.push(StrongSuggestion {
                strong,
                dice,
                n_ws,
                n_w,
                n_s,
                modules,
            });
        }

        if candidates.len() > 1 {
            candidates.retain(|c| c.n_ws >= 2);
        }
        candidates.sort_by(|a, b| {
            b.dice
                .total_cmp(&a.dice)
                .then_with(|| b.n_ws.cmp(&a.n_ws))
                .then_with(|| a.strong.cmp(&b.strong))
        });
        candidates.truncate(limit);
        Ok(candidates)
    }
}

pub fn suggest_strongs(
    cache_path: impl AsRef<Path>,
    word: &str,
    limit: usize,
) -> Result<Vec<StrongSuggestion>, LexiconError> {
    ReverseIndex::open(cache_path)?.suggest_strongs(word, limit)
}

#[cfg(test)]
#[path = "reverse_tests.rs"]
mod tests;
