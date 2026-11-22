use std::collections::HashSet;
use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rhema_core::{Book, StrongLang, VerseRange};
use rhema_refs::Language;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::ImportError;
use crate::ir::{Block, Entry, ImportKind, KeyType, Span, SpanStyle};
use crate::markers::parse_strong_number;
use crate::report::{truncate_section_title, ProblemKind, ProblemRecord};

#[derive(Debug, Clone)]
pub struct TwmExportOptions {
    pub out_path: PathBuf,
    pub title: String,
    pub description: String,
    pub lang: String,
    pub version: String,
    pub force: bool,
}

impl TwmExportOptions {
    pub fn new(out_path: PathBuf, title: String, description: String) -> Self {
        Self {
            out_path,
            title,
            description,
            lang: "por".to_string(),
            version: "1.0".to_string(),
            force: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TwmExportReport {
    pub kind: ImportKind,
    pub out_path: String,
    pub size_bytes: u64,
    pub ir_entries: usize,
    pub written_entries: usize,
    pub internal_links_resolved: usize,
    pub internal_links_unresolved: usize,
    pub bible_links_written: usize,
    pub skipped_without_passage: usize,
    pub images_omitted: usize,
    pub elapsed_ms: f64,
}

impl TwmExportReport {
    pub fn format_summary(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "twm_gerado={} tipo={:?} tamanho_bytes={} entradas_gravadas={}/{} sem_passagem={}",
            self.out_path,
            self.kind,
            self.size_bytes,
            self.written_entries,
            self.ir_entries,
            self.skipped_without_passage
        );
        let _ = writeln!(
            out,
            "links_internos(resolvidos={}, nao_resolvidos={}) links_biblia={} imagens_omitidas={} tempo_ms={:.1}",
            self.internal_links_resolved,
            self.internal_links_unresolved,
            self.bible_links_written,
            self.images_omitted,
            self.elapsed_ms
        );
        if self.images_omitted > 0 {
            let _ = writeln!(
                out,
                "aviso: {} imagem(ns) omitida(s) na exportação RTF (não suportado em RTF pela especificação).",
                self.images_omitted
            );
        }
        out
    }
}

#[derive(Default)]
struct RtfCounters {
    internal_links_resolved: usize,
    internal_links_unresolved: usize,
    bible_links_written: usize,
    images_omitted: usize,
}

/// Exporta a IR (`entries`) para um módulo SQLite `.twm` (dicionário ou comentário).
pub fn write_twm(
    kind: ImportKind,
    entries: &[Entry],
    options: &TwmExportOptions,
    extra_problems: Option<&mut Vec<ProblemRecord>>,
) -> Result<TwmExportReport, ImportError> {
    if !matches!(kind, ImportKind::Dict | ImportKind::Comm) {
        return Err(ImportError::UnsupportedExportKind(format!("{kind:?}")));
    }
    if options.out_path.exists() && !options.force {
        return Err(ImportError::OutputExists(
            options.out_path.display().to_string(),
        ));
    }

    let started = Instant::now();
    if let Some(parent) = options.out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let tmp_path = temp_path_for(&options.out_path);
    if tmp_path.exists() {
        let _ = std::fs::remove_file(&tmp_path);
    }

    let result = write_twm_inner(kind, entries, options, &tmp_path, extra_problems);
    let stats = match result {
        Ok(s) => s,
        Err(err) => {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(err);
        }
    };

    if options.out_path.exists() && options.force {
        std::fs::remove_file(&options.out_path)?;
    }
    std::fs::rename(&tmp_path, &options.out_path)?;

    let size_bytes = std::fs::metadata(&options.out_path)?.len();
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;

    Ok(TwmExportReport {
        kind,
        out_path: options.out_path.display().to_string(),
        size_bytes,
        ir_entries: entries.len(),
        written_entries: stats.written_entries,
        internal_links_resolved: stats.counters.internal_links_resolved,
        internal_links_unresolved: stats.counters.internal_links_unresolved,
        bible_links_written: stats.counters.bible_links_written,
        skipped_without_passage: stats.skipped_without_passage,
        images_omitted: stats.counters.images_omitted,
        elapsed_ms,
    })
}

struct WriteStats {
    written_entries: usize,
    skipped_without_passage: usize,
    counters: RtfCounters,
}

fn write_twm_inner(
    kind: ImportKind,
    entries: &[Entry],
    options: &TwmExportOptions,
    tmp_path: &Path,
    mut extra_problems: Option<&mut Vec<ProblemRecord>>,
) -> Result<WriteStats, ImportError> {
    let mut conn = Connection::open(tmp_path)?;
    conn.execute_batch("PRAGMA synchronous = OFF; PRAGMA journal_mode = MEMORY;")?;

    let is_strong_dict = kind == ImportKind::Dict
        && entries.iter().any(|e| {
            e.keys
                .iter()
                .any(|k| matches!(k.kind, KeyType::HebrewStrongs | KeyType::GreekStrongs))
        });

    let tx = conn.transaction()?;
    create_schema(&tx, kind, is_strong_dict, options)?;

    let mut counters = RtfCounters::default();
    let mut written_entries = 0usize;
    let mut skipped_without_passage = 0usize;

    match kind {
        ImportKind::Dict => {
            // 1º passo: resolve subject de cada entrada e monta conjunto de destinos válidos.
            let resolved: Vec<(String, Option<String>)> =
                entries.iter().map(resolve_dict_subject_and_orig).collect();
            let known_subjects: HashSet<String> = resolved
                .iter()
                .map(|(s, _)| s.to_ascii_uppercase())
                .filter(|s| !s.is_empty())
                .collect();

            let mut ins_topic = tx.prepare(
                "INSERT INTO topics (id, pid, subject, rel_order, content_type, strong_orig_word) \
                 VALUES (?1, 0, ?2, ?3, 'rtf', ?4)",
            )?;
            let mut ins_content =
                tx.prepare("INSERT INTO content (topic_id, data, data2) VALUES (?1, ?2, NULL)")?;
            let mut ins_key = tx.prepare(
                "INSERT INTO rhema_keys (topic_id, tipo, valor, idioma) VALUES (?1, ?2, ?3, ?4)",
            )?;

            let mut next_id = 1i64; // Nunca 0 ou -1
            for (entry, (subject, orig_word)) in entries.iter().zip(resolved.iter()) {
                if subject.is_empty() {
                    continue;
                }
                let heading = entry
                    .title_path
                    .last()
                    .map(String::as_str)
                    .filter(|s| !s.is_empty())
                    .unwrap_or(subject.as_str());
                let rtf = render_entry_rtf(
                    entry,
                    Some(heading),
                    false,
                    Some(&known_subjects),
                    &mut counters,
                );
                let tid = next_id;
                next_id += 1;

                ins_topic.execute(params![tid, subject, tid, orig_word.as_deref()])?;
                ins_content.execute(params![tid, rtf])?;

                for k in &entry.keys {
                    ins_key.execute(params![
                        tid,
                        k.kind.canonical_name(),
                        k.value.trim(),
                        k.lang.as_deref()
                    ])?;
                }
                written_entries += 1;
            }
        }
        ImportKind::Comm => {
            let mut ins_ref = tx.prepare(
                "INSERT INTO bible_refs (topic_id, bi, ci, fvi, tvi, content_type) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 'rtf')",
            )?;
            let mut ins_content =
                tx.prepare("INSERT INTO content (topic_id, data, data2) VALUES (?1, ?2, NULL)")?;

            let mut next_id = 1i64; // Nunca 0 ou -1
            for entry in entries {
                let ranges = extract_comm_ranges(entry);
                if ranges.is_empty() {
                    skipped_without_passage += 1;
                    if let Some(ref mut problems) = extra_problems {
                        let title = entry
                            .title_path
                            .last()
                            .map(String::as_str)
                            .unwrap_or("(sem titulo)");
                        problems.push(ProblemRecord {
                            paragraph: entry.id,
                            kind: ProblemKind::UntaggedHeading,
                            section_title: truncate_section_title(title, 40),
                            detail: Some("sem_passagem".to_string()),
                        });
                    }
                    continue;
                }

                let heading = entry
                    .title_path
                    .last()
                    .map(String::as_str)
                    .filter(|s| !s.is_empty());
                let rtf = render_entry_rtf(entry, heading, true, None, &mut counters);

                let rows = expand_ranges_to_rows(&ranges);
                for (bi, ci, fvi, tvi) in rows {
                    let tid = next_id;
                    next_id += 1;
                    ins_ref.execute(params![tid, bi, ci, fvi, tvi])?;
                    ins_content.execute(params![tid, rtf])?;
                }
                written_entries += 1;
            }
        }
        ImportKind::Book => {}
    }

    tx.commit()?;
    Ok(WriteStats {
        written_entries,
        skipped_without_passage,
        counters,
    })
}

fn create_schema(
    tx: &rusqlite::Transaction<'_>,
    kind: ImportKind,
    is_strong_dict: bool,
    options: &TwmExportOptions,
) -> Result<(), ImportError> {
    tx.execute_batch("CREATE TABLE config (name TEXT, value TEXT);")?;

    let type_val = match kind {
        ImportKind::Dict => "1",
        ImportKind::Comm => "2",
        ImportKind::Book => "3",
    };
    let (ver_major, ver_minor) = split_version(&options.version);
    let abbrev = derive_abbrev(&options.title);

    let mut cfg: Vec<(&str, String)> = vec![
        ("type", type_val.to_string()),
        ("title", options.title.clone()),
        ("abbrev", abbrev),
        ("description", options.description.clone()),
        ("version", options.version.clone()),
        ("version.major", ver_major),
        ("version.minor", ver_minor),
        ("lang", options.lang.clone()),
        ("content.type", "rtf".to_string()),
        ("user", "0".to_string()),
        ("schema.version", "1".to_string()),
    ];
    if is_strong_dict {
        cfg.push(("strong", "1".to_string()));
    }

    {
        let mut ins_cfg = tx.prepare("INSERT INTO config (name, value) VALUES (?1, ?2)")?;
        for (name, value) in cfg {
            ins_cfg.execute(params![name, value])?;
        }
    }

    match kind {
        ImportKind::Dict => {
            tx.execute_batch(
                "CREATE TABLE topics (
                    id INTEGER PRIMARY KEY,
                    pid INTEGER DEFAULT 0,
                    subject TEXT,
                    rel_order INTEGER DEFAULT 1,
                    content_type TEXT,
                    strong_orig_word TEXT
                );
                CREATE INDEX idx_topics_subject ON topics (subject);
                CREATE INDEX idx_topics_rel_order ON topics (rel_order);
                CREATE TABLE content (
                    topic_id INTEGER PRIMARY KEY,
                    data TEXT,
                    data2 BLOB
                );
                CREATE TABLE rhema_keys (
                    topic_id INTEGER,
                    tipo TEXT,
                    valor TEXT,
                    idioma TEXT
                );
                CREATE INDEX idx_rhema_keys_lookup ON rhema_keys (tipo, valor);",
            )?;
        }
        ImportKind::Comm => {
            tx.execute_batch(
                "CREATE TABLE bible_refs (
                    topic_id INTEGER PRIMARY KEY,
                    bi INTEGER,
                    ci INTEGER,
                    fvi INTEGER,
                    tvi INTEGER,
                    content_type TEXT
                );
                CREATE INDEX idx_bible_refs ON bible_refs (bi, ci, fvi, tvi);
                CREATE INDEX idx_bible_refs_bi_ci ON bible_refs (bi, ci);
                CREATE TABLE content (
                    topic_id INTEGER PRIMARY KEY,
                    data TEXT,
                    data2 BLOB
                );",
            )?;
        }
        ImportKind::Book => {}
    }
    Ok(())
}

fn resolve_dict_subject_and_orig(entry: &Entry) -> (String, Option<String>) {
    // 1. Se houver etiqueta Strong, ela define o subject (ex.: H3, G5622).
    let strong_subject = entry.keys.iter().find_map(|k| {
        if matches!(k.kind, KeyType::HebrewStrongs | KeyType::GreekStrongs) {
            parse_strong_number(&k.kind, &k.value).map(|(lang, num)| match lang {
                StrongLang::Hebrew => format!("H{num}"),
                StrongLang::Greek => format!("G{num}"),
            })
        } else {
            None
        }
    });

    // 2. Palavra original: primeira etiqueta Headword com idioma (+he / +el).
    let orig_word = entry.keys.iter().find_map(|k| {
        if k.kind == KeyType::Headword && k.lang.is_some() {
            let v = k.value.trim();
            (!v.is_empty()).then(|| v.to_string())
        } else {
            None
        }
    });

    if let Some(sub) = strong_subject {
        return (sub, orig_word);
    }

    // 3. Sem Strong: usa o primeiro Headword limpo.
    if let Some(hw) = entry.keys.iter().find_map(|k| {
        if k.kind == KeyType::Headword {
            let v = k.value.trim();
            (!v.is_empty()).then(|| v.to_string())
        } else {
            None
        }
    }) {
        return (hw, orig_word);
    }

    // 4. Fallback: último título da hierarquia.
    let fallback = entry
        .title_path
        .last()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    (fallback, orig_word)
}

fn extract_comm_ranges(entry: &Entry) -> Vec<VerseRange> {
    let mut ranges = Vec::new();
    for k in &entry.keys {
        if k.kind == KeyType::Bible {
            ranges.extend(parse_bible_ranges(&k.value));
        }
    }
    ranges
}

fn parse_bible_ranges(raw: &str) -> Vec<VerseRange> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let pt = rhema_refs::parse(trimmed, Language::PtBr);
    if !pt.faixas.is_empty() && pt.erros.is_empty() {
        return pt.faixas;
    }
    let en = rhema_refs::parse(trimmed, Language::En);
    if !en.faixas.is_empty() && en.erros.is_empty() {
        return en.faixas;
    }
    Vec::new()
}

fn expand_ranges_to_rows(ranges: &[VerseRange]) -> Vec<(i64, i64, i64, i64)> {
    let mut rows = Vec::new();
    for range in ranges {
        let book = range.start.book;
        let bi = i64::from(book.number());
        // Livro inteiro
        if range.start.book == range.end.book
            && range.start.chapter == 1
            && range.start.verse == 1
            && range.end.chapter == book.chapter_count()
            && book.verses_in_chapter(range.end.chapter) == Some(range.end.verse)
        {
            rows.push((bi, 0, 0, 0));
            continue;
        }
        // Capítulo inteiro
        if range.start.book == range.end.book
            && range.start.chapter == range.end.chapter
            && range.start.verse == 1
            && book.verses_in_chapter(range.start.chapter) == Some(range.end.verse)
        {
            rows.push((bi, i64::from(range.start.chapter), 0, 0));
            continue;
        }
        // Mesmo capítulo
        if range.start.book == range.end.book && range.start.chapter == range.end.chapter {
            rows.push((
                bi,
                i64::from(range.start.chapter),
                i64::from(range.start.verse),
                i64::from(range.end.verse),
            ));
            continue;
        }
        // Faixa cruzando capítulos no mesmo livro: gera uma linha por capítulo coberto.
        if range.start.book == range.end.book {
            for ch in range.start.chapter..=range.end.chapter {
                let fvi = if ch == range.start.chapter {
                    range.start.verse
                } else {
                    1
                };
                let tvi = if ch == range.end.chapter {
                    range.end.verse
                } else {
                    book.verses_in_chapter(ch).unwrap_or(1)
                };
                rows.push((bi, i64::from(ch), i64::from(fvi), i64::from(tvi)));
            }
        }
    }
    rows
}

/// Gera documento RTF válido (Tahoma 10pt, sem cores azul/vermelho, Unicode via `\uN?`).
fn render_entry_rtf(
    entry: &Entry,
    heading: Option<&str>,
    auto_link_bible_refs: bool,
    known_dict_subjects: Option<&HashSet<String>>,
    counters: &mut RtfCounters,
) -> String {
    let mut rtf = String::with_capacity(entry.char_count.saturating_mul(2).max(256));
    rtf.push_str(
        r"{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fnil\fcharset0 Tahoma;}}\viewkind4\uc1\pard\f0\fs20 ",
    );

    if let Some(title) = heading {
        let trimmed = title.trim();
        if !trimmed.is_empty() {
            rtf.push_str(r"{\b ");
            write_rtf_escaped(&mut rtf, trimmed);
            rtf.push_str("}\\par\n");
        }
    }

    for block in &entry.blocks {
        match block {
            Block::Paragraph { spans, anchors, .. } => {
                for anchor in anchors {
                    let v = anchor.value.trim();
                    if !v.is_empty() {
                        rtf.push_str(r"{\b ");
                        write_rtf_escaped(&mut rtf, v);
                        rtf.push_str("} ");
                    }
                }
                render_spans_rtf(
                    &mut rtf,
                    spans,
                    auto_link_bible_refs,
                    known_dict_subjects,
                    counters,
                );
                rtf.push_str("\\par\n");
            }
            Block::ListItem { level, spans, .. } => {
                let indent = 360u32.saturating_mul(u32::from(*level) + 1);
                let _ = write!(rtf, "\\pard\\li{indent}\\f0\\fs20 \\u8226? ");
                render_spans_rtf(
                    &mut rtf,
                    spans,
                    auto_link_bible_refs,
                    known_dict_subjects,
                    counters,
                );
                rtf.push_str("\\par\\pard\\f0\\fs20\n");
            }
            Block::Table { rows } => {
                render_table_rtf(&mut rtf, rows);
            }
            Block::Image { .. } => {
                counters.images_omitted += 1;
            }
        }
    }

    rtf.push('}');
    rtf
}

fn render_table_rtf(rtf: &mut String, rows: &[Vec<String>]) {
    for row in rows {
        if row.is_empty() {
            continue;
        }
        rtf.push_str(r"\trowd");
        let col_w = 2400usize;
        for (i, _) in row.iter().enumerate() {
            let _ = write!(rtf, "\\cellx{}", (i + 1) * col_w);
        }
        for cell in row {
            rtf.push_str(r"\intbl ");
            write_rtf_escaped(rtf, cell);
            rtf.push_str(r"\cell ");
        }
        rtf.push_str("\\row\\pard\\f0\\fs20\n");
    }
}

fn render_spans_rtf(
    rtf: &mut String,
    spans: &[Span],
    auto_link_bible_refs: bool,
    known_dict_subjects: Option<&HashSet<String>>,
    counters: &mut RtfCounters,
) {
    for span in spans {
        match span {
            Span::Text { text, style, .. } => {
                if auto_link_bible_refs {
                    render_text_with_bible_autolinks(rtf, text, *style, counters);
                } else {
                    write_styled_text(rtf, text, *style);
                }
            }
            Span::Link { link, style, .. } => {
                let label = link.label.as_deref().unwrap_or(link.value.as_str()).trim();
                match &link.kind {
                    KeyType::HebrewStrongs | KeyType::GreekStrongs => {
                        let normalized =
                            parse_strong_number(&link.kind, &link.value).map(|(lang, num)| {
                                match lang {
                                    StrongLang::Hebrew => format!("H{num}"),
                                    StrongLang::Greek => format!("G{num}"),
                                }
                            });
                        let target_key = normalized.as_deref().unwrap_or(link.value.trim());
                        let exists = known_dict_subjects
                            .is_some_and(|set| set.contains(&target_key.to_ascii_uppercase()));
                        if exists {
                            let url = format!("tw://[self]?t={target_key}");
                            let shown = if label.is_empty() { target_key } else { label };
                            write_rtf_hyperlink(rtf, &url, shown, *style);
                            counters.internal_links_resolved += 1;
                        } else {
                            let shown = if label.is_empty() { target_key } else { label };
                            write_styled_text(rtf, shown, *style);
                            counters.internal_links_unresolved += 1;
                        }
                    }
                    KeyType::Bible => {
                        if let Some(url) = bible_link_url(&link.value) {
                            let shown = if label.is_empty() {
                                link.value.trim()
                            } else {
                                label
                            };
                            write_rtf_hyperlink(rtf, &url, shown, *style);
                            counters.bible_links_written += 1;
                        } else {
                            write_styled_text(rtf, label, *style);
                        }
                    }
                    _ => {
                        write_styled_text(rtf, label, *style);
                    }
                }
            }
        }
    }
}

fn bible_link_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.to_ascii_lowercase().starts_with("tw://bible.") {
        return Some(trimmed.split('|').next().unwrap_or(trimmed).to_string());
    }
    let ranges = parse_bible_ranges(trimmed);
    let range = ranges.first()?;
    Some(range_to_tw_url(range))
}

fn range_to_tw_url(range: &VerseRange) -> String {
    let s = &range.start;
    let e = &range.end;
    if s == e {
        format!(
            "tw://bible.*?id={}.{}.{}",
            s.book.number(),
            s.chapter,
            s.verse
        )
    } else {
        format!(
            "tw://bible.*?id={}.{}.{}-{}.{}.{}",
            s.book.number(),
            s.chapter,
            s.verse,
            e.book.number(),
            e.chapter,
            e.verse
        )
    }
}

fn render_text_with_bible_autolinks(
    rtf: &mut String,
    text: &str,
    style: SpanStyle,
    counters: &mut RtfCounters,
) {
    let matches = find_unmarked_bible_refs(text);
    if matches.is_empty() {
        write_styled_text(rtf, text, style);
        return;
    }
    let mut cursor = 0usize;
    for (start, end, range) in matches {
        if start > cursor {
            write_styled_text(rtf, &text[cursor..start], style);
        }
        let url = range_to_tw_url(&range);
        write_rtf_hyperlink(rtf, &url, &text[start..end], style);
        counters.bible_links_written += 1;
        cursor = end;
    }
    if cursor < text.len() {
        write_styled_text(rtf, &text[cursor..], style);
    }
}

/// Encontra referências bíblicas não marcadas no texto e devolve `(start, end, VerseRange)`.
fn find_unmarked_bible_refs(text: &str) -> Vec<(usize, usize, VerseRange)> {
    let mut found = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;

    while i < len {
        if (bytes[i] == b'.' || bytes[i] == b':')
            && i > 0
            && i + 1 < len
            && bytes[i - 1].is_ascii_digit()
            && bytes[i + 1].is_ascii_digit()
        {
            let mut cap_start = i - 1;
            while cap_start > 0 && bytes[cap_start - 1].is_ascii_digit() {
                cap_start -= 1;
            }
            let mut end = i + 1;
            while end < len && bytes[end].is_ascii_digit() {
                end += 1;
            }
            // Estende para faixa opcional `-12` ou `-3.12`
            if end + 1 < len && bytes[end] == b'-' && bytes[end + 1].is_ascii_digit() {
                let mut ext = end + 1;
                while ext < len && bytes[ext].is_ascii_digit() {
                    ext += 1;
                }
                if ext + 1 < len
                    && (bytes[ext] == b'.' || bytes[ext] == b':')
                    && bytes[ext + 1].is_ascii_digit()
                {
                    ext += 1;
                    while ext < len && bytes[ext].is_ascii_digit() {
                        ext += 1;
                    }
                }
                end = ext;
            }

            if cap_start >= 2 && bytes[cap_start - 1] == b' ' {
                let mut bk_end = cap_start - 1;
                while bk_end > 0 && bytes[bk_end - 1] == b' ' {
                    bk_end -= 1;
                }
                let mut bk_start = bk_end;
                while bk_start > 0 {
                    match text[..bk_start].chars().next_back() {
                        Some(c) if c.is_alphabetic() => bk_start -= c.len_utf8(),
                        _ => break,
                    }
                }
                if bk_start > 0 && matches!(bytes[bk_start - 1], b'1' | b'2' | b'3') {
                    let prev_ok = bk_start == 1
                        || !text[..bk_start - 1]
                            .chars()
                            .next_back()
                            .is_some_and(char::is_alphanumeric);
                    if prev_ok {
                        bk_start -= 1;
                    }
                } else if bk_start >= 2
                    && bytes[bk_start - 1] == b' '
                    && matches!(bytes[bk_start - 2], b'1' | b'2' | b'3')
                {
                    let prev_ok = bk_start == 2
                        || !text[..bk_start - 2]
                            .chars()
                            .next_back()
                            .is_some_and(char::is_alphanumeric);
                    if prev_ok {
                        bk_start -= 2;
                    }
                }

                if bk_start < bk_end {
                    let book_token = text[bk_start..bk_end].trim();
                    if looks_like_book(book_token) {
                        let candidate = &text[bk_start..end];
                        let parsed = rhema_refs::parse(candidate, Language::PtBr);
                        if parsed.erros.is_empty() {
                            if let Some(range) = parsed.faixas.into_iter().next() {
                                found.push((bk_start, end, range));
                                i = end;
                                continue;
                            }
                        }
                    }
                }
            }
            i = end;
            continue;
        }
        i += 1;
    }
    found
}

fn looks_like_book(token: &str) -> bool {
    let char_count = token.chars().count();
    if !(2..=20).contains(&char_count) {
        return false;
    }
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let valid_start = if matches!(first, '1' | '2' | '3') {
        chars
            .find(|c| !c.is_whitespace())
            .is_some_and(char::is_uppercase)
    } else {
        first.is_uppercase()
    };
    if !valid_start {
        return false;
    }
    Book::ALL.iter().any(|b| {
        b.primary_abbreviation_pt().eq_ignore_ascii_case(token)
            || b.name_pt().eq_ignore_ascii_case(token)
            || b.primary_abbreviation_en().eq_ignore_ascii_case(token)
    }) || token.len() <= 5
}

fn write_rtf_hyperlink(rtf: &mut String, url: &str, label: &str, style: SpanStyle) {
    rtf.push_str(r#"{\field{\*\fldinst HYPERLINK ""#);
    write_rtf_escaped(rtf, url);
    rtf.push_str(r#""}{\fldrslt "#);
    write_styled_text(rtf, label, style);
    rtf.push_str("}}");
}

fn write_styled_text(rtf: &mut String, text: &str, style: SpanStyle) {
    if text.is_empty() {
        return;
    }
    let has_style = style.bold || style.italic || style.underline || style.superscript;
    if has_style {
        rtf.push('{');
        if style.bold {
            rtf.push_str(r"\b");
        }
        if style.italic {
            rtf.push_str(r"\i");
        }
        if style.underline {
            rtf.push_str(r"\ul");
        }
        if style.superscript {
            rtf.push_str(r"\super");
        }
        rtf.push(' ');
        write_rtf_escaped(rtf, text);
        rtf.push('}');
    } else {
        write_rtf_escaped(rtf, text);
    }
}

/// Escapa texto UTF-8 para RTF válido com `\uN?` para não-ASCII (acentos, grego, hebraico).
pub fn write_rtf_escaped(out: &mut String, text: &str) {
    let mut utf16 = [0u16; 2];
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            '{' => out.push_str(r"\{"),
            '}' => out.push_str(r"\}"),
            '\n' => out.push_str(r"\line "),
            '\r' => {}
            '\t' => out.push_str(r"\tab "),
            c if c.is_ascii() => out.push(c),
            c => {
                for unit in c.encode_utf16(&mut utf16) {
                    let signed = *unit as i16;
                    let _ = write!(out, "\\u{signed}?");
                }
            }
        }
    }
}

fn split_version(version: &str) -> (String, String) {
    match version.trim().split_once('.') {
        Some((maj, min)) => (maj.to_string(), min.to_string()),
        None => (version.trim().to_string(), "0".to_string()),
    }
}

fn derive_abbrev(title: &str) -> String {
    let clean: String = title
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect();
    let trimmed = clean.trim();
    if trimmed.is_empty() {
        return "MOD".to_string();
    }
    trimmed.chars().take(12).collect()
}

fn temp_path_for(out_path: &Path) -> PathBuf {
    let pid = std::process::id();
    let file_name = out_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "module.twm".to_string());
    out_path.with_file_name(format!("{file_name}.tmp.{pid}"))
}
