use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rhema_core::{Book, ModuleError, Node, StrongLang, Testament};
use rhema_twformat::{tokenize_markup, BibleScope, OntReader};
use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use walkdir::WalkDir;

use crate::index::{default_data_dir, ensure_outside_repository};
use crate::reverse::{
    build_reverse_from_collected, collect_reverse_nodes, tokenize_latin_words, ReverseBuildReport,
};
use crate::{fold_latin, LexiconError};

const APPLICATION_ID: i32 = 0x524F4343;
const SCHEMA_VERSION: i32 = 1;

/// Idiomas ativos por padrão no índice de ocorrências.
pub const DEFAULT_ACTIVE_LANGUAGES: [&str; 3] = ["por", "grc", "heb"];

/// Caminho padrão do índice de ocorrências por Strong (fora do repositório).
pub fn default_occurrences_path() -> Option<PathBuf> {
    default_data_dir().map(|dir| dir.join("lexicon-occurrences.sqlite"))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BibleOccurrencesStats {
    pub id: String,
    pub title: String,
    pub abbreviation: Option<String>,
    pub lang: String,
    pub path: String,
    pub verses_read: usize,
    pub verses_with_strong: usize,
    pub occurrence_rows: usize,
    pub translation_rows: usize,
    pub ms: u128,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IgnoredBibleByLanguage {
    pub id: String,
    pub path: String,
    pub lang: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OccurrencesBuildReport {
    pub cache_path: String,
    pub bibles: Vec<BibleOccurrencesStats>,
    pub ignored_by_language: Vec<IgnoredBibleByLanguage>,
    pub protected_ignored: usize,
    pub unreadable_modules: usize,
    pub warnings: Vec<String>,
    pub scan_ms: u128,
    pub build_ms: u128,
    pub database_bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OccurrenceBibleInfo {
    pub module_id: String,
    pub title: String,
    pub abbreviation: Option<String>,
    pub lang: String,
    pub path: String,
    pub total_verses: u64,
    pub total_occurrences: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BookOccurrenceCount {
    pub book: u8,
    pub verses: u64,
    pub occurrences: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VerseOccurrence {
    pub book: u8,
    pub chapter: u16,
    pub verse: u16,
    pub n: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StrongOccurrencesResult {
    pub strong: String,
    pub selected_module_id: Option<String>,
    pub bibles: Vec<OccurrenceBibleInfo>,
    pub total_verses: u64,
    pub total_occurrences: u64,
    pub by_book: Vec<BookOccurrenceCount>,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
    pub items: Vec<VerseOccurrence>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TranslationCount {
    pub word: String,
    pub n: u64,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StrongTranslationsResult {
    pub strong: String,
    pub module_id: Option<String>,
    pub total: u64,
    pub items: Vec<TranslationCount>,
}

/// Valida e normaliza número Strong no formato `G` ou `H` seguido de 1 a 5 dígitos.
pub fn validate_strong_number(raw: &str) -> Result<String, LexiconError> {
    let trimmed = raw.trim();
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return Err(LexiconError::Invalid(
            "Número Strong inválido (use G ou H seguido de 1 a 5 dígitos)".into(),
        ));
    };
    let prefix = match first.to_ascii_uppercase() {
        'G' => 'G',
        'H' => 'H',
        _ => {
            return Err(LexiconError::Invalid(
                "Número Strong inválido (use G ou H seguido de 1 a 5 dígitos)".into(),
            ));
        }
    };
    let rest: &str = chars.as_str();
    if rest.is_empty() || rest.len() > 5 || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return Err(LexiconError::Invalid(
            "Número Strong inválido (use G ou H seguido de 1 a 5 dígitos)".into(),
        ));
    }
    let num: u32 = rest
        .parse()
        .map_err(|_| LexiconError::Invalid("Número Strong inválido".into()))?;
    if num == 0 {
        return Err(LexiconError::Invalid(
            "Número Strong inválido (deve ser maior que zero)".into(),
        ));
    }
    Ok(format!("{prefix}{num}"))
}

fn normalize_lang_tag(raw: &str) -> Option<String> {
    let cleaned = raw
        .trim()
        .trim_end_matches("<-")
        .trim()
        .to_ascii_lowercase();
    if cleaned.is_empty() {
        return None;
    }
    let mapped = match cleaned.as_str() {
        "pt" | "pt-br" | "pt_br" | "pt-pt" | "por" | "br" | "portuguese" | "português"
        | "portugues" => "por",
        "grc" | "grk" | "el" | "ell" | "greg" | "gre" | "greek" | "grego" => "grc",
        "heb" | "he" | "iw" | "hbo" | "hebrew" | "hebraico" => "heb",
        "en" | "en-us" | "en-gb" | "eng" | "english" | "inglês" | "ingles" => "eng",
        "es" | "spa" | "esp" | "spanish" | "espanhol" => "spa",
        other => return Some(other.to_string()),
    };
    Some(mapped.to_string())
}

fn strip_tags_for_lang_probe(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_tag = false;
    for ch in line.chars() {
        match ch {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// Detecta o idioma da Bíblia usando `config lang`, com reserva por análise de conteúdo.
pub fn detect_bible_language(reader: &OntReader) -> String {
    if let Some(lang) = reader
        .info()
        .language
        .as_deref()
        .and_then(normalize_lang_tag)
    {
        return lang;
    }

    let mut grc = 0usize;
    let mut heb = 0usize;
    let mut lat = 0usize;
    let mut pt_score = 0usize;
    let mut en_score = 0usize;
    let mut sampled = 0usize;

    const PT_WORDS: &[&str] = &[
        "que",
        "deus",
        "senhor",
        "para",
        "com",
        "nao",
        "disse",
        "por",
        "dos",
        "das",
        "uma",
        "como",
        "mas",
        "porque",
        "todos",
        "entao",
        "sobre",
        "terra",
        "filho",
        "espiritos",
    ];
    const EN_WORDS: &[&str] = &[
        "the", "and", "shall", "unto", "lord", "that", "for", "with", "his", "they", "from",
        "which", "not", "but", "have", "this", "thou", "thee", "god", "said",
    ];

    for idx in 0..reader.verse_count() {
        if sampled >= 200 {
            break;
        }
        let Ok(line) = reader.line_by_index(idx) else {
            continue;
        };
        let stripped = strip_tags_for_lang_probe(&line);
        if stripped.trim().is_empty() {
            continue;
        }
        sampled += 1;
        for ch in stripped.chars() {
            let cp = ch as u32;
            if (0x0370..=0x03FF).contains(&cp) || (0x1F00..=0x1FFF).contains(&cp) {
                grc += 1;
            } else if (0x0590..=0x05FF).contains(&cp) {
                heb += 1;
            } else if ch.is_ascii_alphabetic()
                || (ch.is_alphabetic() && matches!(cp, 0x00C0..=0x024F | 0x1E00..=0x1EFF))
            {
                lat += 1;
                if matches!(
                    ch,
                    'ã' | 'õ'
                        | 'ç'
                        | 'ê'
                        | 'á'
                        | 'é'
                        | 'í'
                        | 'ó'
                        | 'ú'
                        | 'â'
                        | 'ô'
                        | 'à'
                        | 'Ã'
                        | 'Õ'
                        | 'Ç'
                ) {
                    pt_score += 2;
                }
            }
        }
        if lat > 0 {
            for word in fold_latin(&stripped).split(|c: char| !c.is_ascii_alphabetic()) {
                if PT_WORDS.contains(&word) {
                    pt_score += 1;
                } else if EN_WORDS.contains(&word) {
                    en_score += 1;
                }
            }
        }
    }

    if heb > grc && heb >= 20 && heb * 2 >= lat {
        return "heb".to_string();
    }
    if grc > heb && grc >= 20 && grc * 2 >= lat {
        return "grc".to_string();
    }
    if lat > 0 {
        if pt_score > 0 && pt_score >= en_score {
            return "por".to_string();
        }
        if en_score > pt_score {
            return "eng".to_string();
        }
    }
    "other".to_string()
}

fn scope_verse_refs(scope: BibleScope) -> Vec<(u8, u16, u16)> {
    let mut refs = Vec::new();
    match scope {
        BibleScope::Full => {
            refs.reserve(31_102);
            for num in 1..=66 {
                if let Some(book) = Book::from_number(num) {
                    push_book_refs(book, &mut refs);
                }
            }
        }
        BibleScope::OldTestament => {
            refs.reserve(23_145);
            for num in 1..=66 {
                if let Some(book) = Book::from_number(num) {
                    if book.testament() == Testament::Old {
                        push_book_refs(book, &mut refs);
                    }
                }
            }
        }
        BibleScope::NewTestament => {
            refs.reserve(7_957);
            for num in 1..=66 {
                if let Some(book) = Book::from_number(num) {
                    if book.testament() == Testament::New {
                        push_book_refs(book, &mut refs);
                    }
                }
            }
        }
        BibleScope::Degraded { total_verses } => {
            refs.reserve(total_verses.min(31_102));
            'outer: for num in 1..=66 {
                let Some(book) = Book::from_number(num) else {
                    continue;
                };
                let bnum = book.number();
                let mut ch = 1u16;
                while let Some(max_v) = book.verses_in_chapter(ch) {
                    for v in 1..=max_v {
                        if refs.len() >= total_verses {
                            break 'outer;
                        }
                        refs.push((bnum, ch, v));
                    }
                    ch += 1;
                }
            }
        }
    }
    refs
}

fn push_book_refs(book: Book, out: &mut Vec<(u8, u16, u16)>) {
    let bnum = book.number();
    let mut ch = 1u16;
    while let Some(max_v) = book.verses_in_chapter(ch) {
        for v in 1..=max_v {
            out.push((bnum, ch, v));
        }
        ch += 1;
    }
}

fn collect_occurrence_nodes(
    nodes: &[Node],
    collect_translations: bool,
    active_strongs: &mut Vec<String>,
    verse_strongs: &mut HashMap<String, u64>,
    translations: &mut HashMap<(String, String), u64>,
) {
    for node in nodes {
        match node {
            Node::Text(text) => {
                if !collect_translations || active_strongs.is_empty() {
                    continue;
                }
                for word in tokenize_latin_words(text) {
                    for strong in active_strongs.iter() {
                        *translations
                            .entry((strong.clone(), word.clone()))
                            .or_insert(0) += 1;
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
                *verse_strongs.entry(tag.clone()).or_insert(0) += 1;
                let pushed = if !active_strongs.contains(&tag) {
                    active_strongs.push(tag);
                    true
                } else {
                    false
                };
                collect_occurrence_nodes(
                    children,
                    collect_translations,
                    active_strongs,
                    verse_strongs,
                    translations,
                );
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
                collect_occurrence_nodes(
                    c,
                    collect_translations,
                    active_strongs,
                    verse_strongs,
                    translations,
                );
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

fn validate_cache_path(cache_path: &Path) -> Result<(), LexiconError> {
    ensure_outside_repository(cache_path)?;
    if cache_path.extension().is_some_and(|e| {
        matches!(
            e.to_string_lossy().to_lowercase().as_str(),
            "twm" | "ont" | "ot" | "nt" | "ontx" | "otx" | "ntx"
        )
    }) {
        return Err(LexiconError::Invalid(
            "O arquivo de índice não pode substituir um módulo".into(),
        ));
    }
    Ok(())
}

fn scan_bible_paths(roots: &[PathBuf]) -> Result<(BTreeSet<PathBuf>, usize), LexiconError> {
    if roots.is_empty() {
        return Err(LexiconError::Invalid("Informe ao menos uma pasta".into()));
    }
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
    Ok((candidate_paths, protected_ignored))
}

/// Constrói o índice de ocorrências por Strong usando os idiomas ativos padrão (`por`, `grc`, `heb`).
pub fn build_occurrences_index(
    cache_path: impl AsRef<Path>,
    roots: &[PathBuf],
) -> Result<OccurrencesBuildReport, LexiconError> {
    build_occurrences_index_with_languages(cache_path, roots, &DEFAULT_ACTIVE_LANGUAGES)
}

/// Constrói o índice de ocorrências por Strong filtrando pelos idiomas informados.
pub fn build_occurrences_index_with_languages(
    cache_path: impl AsRef<Path>,
    roots: &[PathBuf],
    active_languages: &[&str],
) -> Result<OccurrencesBuildReport, LexiconError> {
    let (_, occ_report) =
        build_bible_indexes_inner(None, cache_path.as_ref(), roots, active_languages)?;
    Ok(occ_report)
}

/// Constrói o índice reverso e o índice de ocorrências na mesma varredura das Bíblias,
/// gravando cada arquivo em temporário com troca atômica.
pub fn build_reverse_and_occurrences(
    reverse_path: impl AsRef<Path>,
    occurrences_path: impl AsRef<Path>,
    roots: &[PathBuf],
) -> Result<
    (
        Result<ReverseBuildReport, LexiconError>,
        OccurrencesBuildReport,
    ),
    LexiconError,
> {
    let (rev, occ) = build_bible_indexes_inner(
        Some(reverse_path.as_ref()),
        occurrences_path.as_ref(),
        roots,
        &DEFAULT_ACTIVE_LANGUAGES,
    )?;
    let rev = rev.unwrap_or_else(|| {
        Err(LexiconError::Invalid(
            "Nenhuma Bíblia legível com tags Strong encontrada".into(),
        ))
    });
    Ok((rev, occ))
}

#[allow(clippy::type_complexity)]
fn build_bible_indexes_inner(
    reverse_path: Option<&Path>,
    occurrences_path: &Path,
    roots: &[PathBuf],
    active_languages: &[&str],
) -> Result<
    (
        Option<Result<ReverseBuildReport, LexiconError>>,
        OccurrencesBuildReport,
    ),
    LexiconError,
> {
    validate_cache_path(occurrences_path)?;
    if let Some(rp) = reverse_path {
        validate_cache_path(rp)?;
    }

    let scan_start = Instant::now();
    let (candidate_paths, mut protected_ignored) = scan_bible_paths(roots)?;
    let scan_ms = scan_start.elapsed().as_millis();

    let build_start = Instant::now();
    if let Some(parent) = occurrences_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let occ_tmp = occurrences_path.with_extension(format!("tmp.{}", now_nanos()));

    let active_set: BTreeSet<String> = active_languages
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();

    let mut occ_db = Connection::open_with_flags(
        &occ_tmp,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;
    if let Err(err) = init_occurrences_schema(&occ_db) {
        drop(occ_db);
        let _ = fs::remove_file(&occ_tmp);
        return Err(err);
    }

    let mut occ_bibles = Vec::new();
    let mut ignored_by_language = Vec::new();
    let mut warnings = Vec::new();
    let mut unreadable_modules = 0usize;
    let mut used_ids = BTreeSet::new();
    let mut reverse_collected = Vec::new();

    for path in &candidate_paths {
        let mod_start = Instant::now();
        let (size, modified_ns) = match stamp(path) {
            Ok(s) => s,
            Err(e) => {
                drop(occ_db);
                let _ = fs::remove_file(&occ_tmp);
                return Err(e);
            }
        };
        let reader = match OntReader::open(path) {
            Ok(r) => r,
            Err(ModuleError::EncryptedModule) => {
                protected_ignored += 1;
                continue;
            }
            Err(error) => {
                unreadable_modules += 1;
                warnings.push(format!("Módulo não analisado {}: {error}", path.display()));
                continue;
            }
        };

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "bible".into());
        let mut id = filename.clone();
        let mut n_suffix = 2;
        while !used_ids.insert(id.to_lowercase()) {
            id = format!("{filename}~{n_suffix}");
            n_suffix += 1;
        }

        let lang = detect_bible_language(&reader);
        let lang_active = active_set.contains(&lang);
        let collect_translations = lang_active && lang != "grc" && lang != "heb";
        let want_reverse = reverse_path.is_some();

        let refs = scope_verse_refs(reader.scope());
        let verses_read = reader.verse_count();
        let limit = verses_read.min(refs.len());

        let mut rev_pairs: HashMap<(String, String), u64> = HashMap::new();
        let mut rev_word_totals: HashMap<String, u64> = HashMap::new();
        let mut rev_strong_totals: HashMap<String, u64> = HashMap::new();

        let mut occ_rows: Vec<(String, u8, u16, u16, u64)> = Vec::new();
        let mut occ_translations: HashMap<(String, String), u64> = HashMap::new();
        let mut verses_with_strong = 0usize;
        let mut has_any_strong = false;

        let mut active_strongs = Vec::new();
        let mut verse_strongs: HashMap<String, u64> = HashMap::new();

        for (idx, &(book, chapter, verse)) in refs.iter().enumerate().take(limit) {
            let Ok(line) = reader.line_by_index(idx) else {
                continue;
            };
            if !line.contains("<W") && !line.contains("<w") {
                continue;
            }
            let nodes = tokenize_markup(&line);
            if want_reverse {
                active_strongs.clear();
                collect_reverse_nodes(
                    &nodes,
                    &mut active_strongs,
                    &mut rev_pairs,
                    &mut rev_word_totals,
                    &mut rev_strong_totals,
                );
            }
            verse_strongs.clear();
            active_strongs.clear();
            collect_occurrence_nodes(
                &nodes,
                collect_translations,
                &mut active_strongs,
                &mut verse_strongs,
                &mut occ_translations,
            );
            if !verse_strongs.is_empty() {
                has_any_strong = true;
                if lang_active {
                    verses_with_strong += 1;
                    for (strong, count) in verse_strongs.drain() {
                        occ_rows.push((strong, book, chapter, verse, count));
                    }
                }
            }
        }

        let current_stamp = match stamp(path) {
            Ok(s) => s,
            Err(e) => {
                drop(occ_db);
                let _ = fs::remove_file(&occ_tmp);
                return Err(e);
            }
        };
        if current_stamp != (size, modified_ns.clone()) {
            drop(occ_db);
            let _ = fs::remove_file(&occ_tmp);
            return Err(LexiconError::Invalid(format!(
                "Módulo alterado durante a indexação: {}",
                path.display()
            )));
        }

        let path_str = path.to_string_lossy().into_owned();
        if want_reverse && !rev_pairs.is_empty() {
            reverse_collected.push(crate::reverse::CollectedBibleReverse {
                id: id.clone(),
                path: path_str.clone(),
                size,
                modified_ns: modified_ns.clone(),
                verses_read,
                pairs: rev_pairs,
                word_totals: rev_word_totals,
                strong_totals: rev_strong_totals,
                ms: mod_start.elapsed().as_millis(),
            });
        }

        if !has_any_strong {
            continue;
        }
        if !lang_active {
            ignored_by_language.push(IgnoredBibleByLanguage {
                id,
                path: path_str,
                lang,
            });
            continue;
        }

        let title = reader
            .info()
            .title
            .trim()
            .trim_end_matches("<-")
            .trim()
            .to_string();
        let abbreviation = reader
            .info()
            .abbreviation
            .as_deref()
            .map(|s| s.trim().trim_end_matches("<-").trim().to_string())
            .filter(|s| !s.is_empty());
        let occ_count = occ_rows.len();
        let trans_count = occ_translations.len();
        let mut book_counts: HashMap<(String, u8), (u64, u64)> = HashMap::new();
        for (strong, book, _, _, n) in &occ_rows {
            let entry = book_counts.entry((strong.clone(), *book)).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += *n;
        }

        let write_res: Result<(), LexiconError> = (|| {
            let tx = occ_db.transaction()?;
            tx.execute(
                "INSERT INTO modules VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![
                    id,
                    title,
                    abbreviation,
                    lang,
                    path_str,
                    size as i64,
                    modified_ns,
                    verses_with_strong as i64,
                    occ_count as i64,
                    trans_count as i64
                ],
            )?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO occurrences(strong, module_id, book, chapter, verse, n) VALUES (?1,?2,?3,?4,?5,?6)",
                )?;
                for (strong, book, chapter, verse, n) in &occ_rows {
                    stmt.execute(params![
                        strong,
                        id,
                        *book as i64,
                        *chapter as i64,
                        *verse as i64,
                        *n as i64
                    ])?;
                }
            }
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO book_counts(strong, module_id, book, verses, occ) VALUES (?1,?2,?3,?4,?5)",
                )?;
                for ((strong, book), (verses, occ)) in &book_counts {
                    stmt.execute(params![
                        strong,
                        id,
                        *book as i64,
                        *verses as i64,
                        *occ as i64
                    ])?;
                }
            }
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO translations(strong, module_id, word, n) VALUES (?1,?2,?3,?4)",
                )?;
                for ((strong, word), n) in &occ_translations {
                    stmt.execute(params![strong, id, word, *n as i64])?;
                }
            }
            tx.commit()?;
            Ok(())
        })();
        if let Err(err) = write_res {
            drop(occ_db);
            let _ = fs::remove_file(&occ_tmp);
            return Err(err);
        }

        occ_bibles.push(BibleOccurrencesStats {
            id,
            title,
            abbreviation,
            lang,
            path: path_str,
            verses_read,
            verses_with_strong,
            occurrence_rows: occ_count,
            translation_rows: trans_count,
            ms: mod_start.elapsed().as_millis(),
        });
    }

    let rev_result = reverse_path.map(|rp| {
        build_reverse_from_collected(
            rp,
            reverse_collected,
            protected_ignored,
            unreadable_modules,
            warnings.clone(),
            scan_ms,
            build_start,
        )
    });

    drop(occ_db);

    if let Err(err) = fs::rename(&occ_tmp, occurrences_path) {
        let _ = fs::remove_file(&occ_tmp);
        return Err(err.into());
    }

    let database_bytes = fs::metadata(occurrences_path)?.len();
    let occ_report = OccurrencesBuildReport {
        cache_path: occurrences_path.to_string_lossy().into_owned(),
        bibles: occ_bibles,
        ignored_by_language,
        protected_ignored,
        unreadable_modules,
        warnings,
        scan_ms,
        build_ms: build_start.elapsed().as_millis(),
        database_bytes,
    };

    Ok((rev_result, occ_report))
}

fn init_occurrences_schema(db: &Connection) -> Result<(), LexiconError> {
    db.execute_batch(&format!(
        "PRAGMA application_id={APPLICATION_ID};
         PRAGMA user_version={SCHEMA_VERSION};
         PRAGMA journal_mode=OFF;
         PRAGMA synchronous=OFF;
         CREATE TABLE modules(
             id TEXT PRIMARY KEY,
             title TEXT NOT NULL,
             abbreviation TEXT,
             lang TEXT NOT NULL,
             path TEXT NOT NULL,
             size INTEGER NOT NULL,
             modified_ns TEXT NOT NULL,
             verses_with_strong INTEGER NOT NULL,
             occurrence_rows INTEGER NOT NULL,
             translation_rows INTEGER NOT NULL
         );
         CREATE TABLE occurrences(
             strong TEXT NOT NULL,
             module_id TEXT NOT NULL,
             book INTEGER NOT NULL,
             chapter INTEGER NOT NULL,
             verse INTEGER NOT NULL,
             n INTEGER NOT NULL,
             PRIMARY KEY(strong, module_id, book, chapter, verse)
         ) WITHOUT ROWID;
         CREATE TABLE book_counts(
             strong TEXT NOT NULL,
             module_id TEXT NOT NULL,
             book INTEGER NOT NULL,
             verses INTEGER NOT NULL,
             occ INTEGER NOT NULL,
             PRIMARY KEY(strong, module_id, book)
         ) WITHOUT ROWID;
         CREATE TABLE translations(
             strong TEXT NOT NULL,
             module_id TEXT NOT NULL,
             word TEXT NOT NULL,
             n INTEGER NOT NULL,
             PRIMARY KEY(strong, module_id, word)
         ) WITHOUT ROWID;"
    ))?;
    Ok(())
}

pub struct OccurrencesIndex {
    db: Connection,
}

impl OccurrencesIndex {
    pub fn open(cache_path: impl AsRef<Path>) -> Result<Self, LexiconError> {
        let path = cache_path.as_ref();
        if !path.exists() {
            return Err(LexiconError::RebuildRequired(format!(
                "índice de ocorrências inexistente em {}",
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
                "arquivo SQLite não é um índice de ocorrências RhemaStudio".into(),
            ));
        }
        if version != SCHEMA_VERSION {
            return Err(LexiconError::RebuildRequired(format!(
                "versão de índice de ocorrências {version} (esperada {SCHEMA_VERSION})"
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

    /// Lista os módulos indexados e seus totais para o Strong solicitado.
    pub fn bibles_for_strong(
        &self,
        strong: &str,
    ) -> Result<Vec<OccurrenceBibleInfo>, LexiconError> {
        let strong = validate_strong_number(strong)?;
        let mut stmt = self.db.prepare_cached(
            "SELECT m.id, m.title, m.abbreviation, m.lang, m.path,
                    COALESCE(o.verses, 0), COALESCE(o.occ, 0)
             FROM modules m
             LEFT JOIN (
                 SELECT module_id, SUM(verses) AS verses, SUM(occ) AS occ
                 FROM book_counts
                 WHERE strong = ?1
                 GROUP BY module_id
             ) o ON o.module_id = m.id
             ORDER BY CASE m.lang WHEN 'por' THEN 0 WHEN 'grc' THEN 1 WHEN 'heb' THEN 2 ELSE 3 END,
                      lower(m.title), m.id",
        )?;
        let mut rows = stmt.query([&strong])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(OccurrenceBibleInfo {
                module_id: row.get(0)?,
                title: row.get(1)?,
                abbreviation: row.get(2)?,
                lang: row.get(3)?,
                path: row.get(4)?,
                total_verses: row.get::<_, i64>(5)?.max(0) as u64,
                total_occurrences: row.get::<_, i64>(6)?.max(0) as u64,
            });
        }
        Ok(out)
    }

    /// Consulta ocorrências de um Strong em uma Bíblia específica (ou a primeira com ocorrências),
    /// com contagem por livro na ordem canônica e lista paginada de referências.
    pub fn occurrences(
        &self,
        strong: &str,
        module_id: Option<&str>,
        page: u32,
        page_size: u32,
    ) -> Result<StrongOccurrencesResult, LexiconError> {
        let strong = validate_strong_number(strong)?;
        if !(1..=200).contains(&page) {
            return Err(LexiconError::Invalid(
                "A página deve estar entre 1 e 200".into(),
            ));
        }
        let page_size = page_size.clamp(1, 200);
        let bibles = self.bibles_for_strong(&strong)?;

        let selected = if let Some(req) = module_id {
            let req = req.trim();
            bibles
                .iter()
                .find(|b| b.module_id == req)
                .map(|b| b.module_id.clone())
        } else {
            bibles
                .iter()
                .find(|b| b.total_occurrences > 0)
                .or_else(|| bibles.first())
                .map(|b| b.module_id.clone())
        };

        let Some(selected_id) = selected else {
            return Ok(StrongOccurrencesResult {
                strong,
                selected_module_id: None,
                bibles,
                total_verses: 0,
                total_occurrences: 0,
                by_book: Vec::new(),
                page,
                page_size,
                total_pages: 0,
                items: Vec::new(),
            });
        };

        let (total_verses, total_occurrences) = bibles
            .iter()
            .find(|b| b.module_id == selected_id)
            .map(|b| (b.total_verses, b.total_occurrences))
            .unwrap_or((0, 0));

        let mut by_book = Vec::new();
        {
            let mut stmt = self.db.prepare_cached(
                "SELECT book, verses, occ
                 FROM book_counts
                 WHERE strong = ?1 AND module_id = ?2
                 ORDER BY book ASC",
            )?;
            let mut rows = stmt.query(params![&strong, &selected_id])?;
            while let Some(row) = rows.next()? {
                by_book.push(BookOccurrenceCount {
                    book: row.get::<_, i64>(0)?.clamp(1, 66) as u8,
                    verses: row.get::<_, i64>(1)?.max(0) as u64,
                    occurrences: row.get::<_, i64>(2)?.max(0) as u64,
                });
            }
        }

        let total_pages = if total_verses == 0 {
            0
        } else {
            total_verses.div_ceil(page_size as u64).min(200) as u32
        };
        let offset = ((page - 1) as u64) * (page_size as u64);

        let mut items = Vec::new();
        {
            let mut stmt = self.db.prepare_cached(
                "SELECT book, chapter, verse, n
                 FROM occurrences
                 WHERE strong = ?1 AND module_id = ?2
                 ORDER BY book ASC, chapter ASC, verse ASC
                 LIMIT ?3 OFFSET ?4",
            )?;
            let mut rows = stmt.query(params![
                &strong,
                &selected_id,
                page_size as i64,
                offset as i64
            ])?;
            while let Some(row) = rows.next()? {
                items.push(VerseOccurrence {
                    book: row.get::<_, i64>(0)?.clamp(1, 66) as u8,
                    chapter: row.get::<_, i64>(1)?.max(1) as u16,
                    verse: row.get::<_, i64>(2)?.max(1) as u16,
                    n: row.get::<_, i64>(3)?.max(1) as u64,
                });
            }
        }

        Ok(StrongOccurrencesResult {
            strong,
            selected_module_id: Some(selected_id),
            bibles,
            total_verses,
            total_occurrences,
            by_book,
            page,
            page_size,
            total_pages,
            items,
        })
    }

    /// Consulta as palavras traduzidas mais usadas para um Strong (em uma Bíblia ou somando todas).
    pub fn translations(
        &self,
        strong: &str,
        module_id: Option<&str>,
        limit: usize,
    ) -> Result<StrongTranslationsResult, LexiconError> {
        let strong = validate_strong_number(strong)?;
        let limit = limit.clamp(1, 100);

        let (total, items) = if let Some(mid) = module_id.map(str::trim).filter(|s| !s.is_empty()) {
            let total: u64 = self
                .db
                .prepare_cached(
                    "SELECT COALESCE(SUM(n), 0) FROM translations WHERE module_id = ?1 AND strong = ?2",
                )?
                .query_row(params![mid, &strong], |r| r.get::<_, i64>(0))?
                .max(0) as u64;
            if total == 0 {
                return Ok(StrongTranslationsResult {
                    strong,
                    module_id: Some(mid.to_string()),
                    total: 0,
                    items: Vec::new(),
                });
            }
            let mut stmt = self.db.prepare_cached(
                "SELECT word, n FROM translations
                 WHERE module_id = ?1 AND strong = ?2
                 ORDER BY n DESC, word ASC
                 LIMIT ?3",
            )?;
            let mut rows = stmt.query(params![mid, &strong, limit as i64])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                let word: String = row.get(0)?;
                let n = row.get::<_, i64>(1)?.max(0) as u64;
                let percent = (n as f64 / total as f64) * 100.0;
                out.push(TranslationCount { word, n, percent });
            }
            (total, out)
        } else {
            let total: u64 = self
                .db
                .prepare_cached("SELECT COALESCE(SUM(n), 0) FROM translations WHERE strong = ?1")?
                .query_row([&strong], |r| r.get::<_, i64>(0))?
                .max(0) as u64;
            if total == 0 {
                return Ok(StrongTranslationsResult {
                    strong,
                    module_id: None,
                    total: 0,
                    items: Vec::new(),
                });
            }
            let mut stmt = self.db.prepare_cached(
                "SELECT word, SUM(n) AS total_n FROM translations
                 WHERE strong = ?1
                 GROUP BY word
                 ORDER BY total_n DESC, word ASC
                 LIMIT ?2",
            )?;
            let mut rows = stmt.query(params![&strong, limit as i64])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                let word: String = row.get(0)?;
                let n = row.get::<_, i64>(1)?.max(0) as u64;
                let percent = (n as f64 / total as f64) * 100.0;
                out.push(TranslationCount { word, n, percent });
            }
            (total, out)
        };

        Ok(StrongTranslationsResult {
            strong,
            module_id: module_id.map(str::to_string),
            total,
            items,
        })
    }
}

#[cfg(test)]
#[path = "occurrences_tests.rs"]
mod tests;
