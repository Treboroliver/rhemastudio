use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::BufReader;
use std::path::Path;
use std::time::Instant;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use rhema_core::StrongLang;

use crate::error::{
    ImportError, MAX_ENTRY_BYTES, MAX_IMAGE_BYTES, MAX_META_PART_BYTES,
    MAX_PART_DECOMPRESSED_BYTES, MAX_TABLE_CELLS,
};
use crate::ir::{Block, Entry, EntryKey, ImportKind, KeyType, Span, SpanStyle};
use crate::markers::{
    parse_paragraph_runs, parse_strong_number, resolve_bible_ref, suggest_known_type,
};
use crate::report::{
    truncate_section_title, ProblemKind, ProblemRecord, StrongGapsReport, StrongRangeStats,
    SuspiciousTypeReport, ValidationReport,
};
use crate::zip_stream::ZipStream;

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub kind: ImportKind,
    pub entry_level: Option<u8>,
    pub max_entry_bytes: usize,
    pub max_image_bytes: u64,
    pub max_table_cells: usize,
    pub max_part_bytes: u64,
    pub collect_entries: bool,
    pub collect_problems: bool,
}

impl ImportOptions {
    pub fn new(kind: ImportKind) -> Self {
        Self {
            kind,
            entry_level: None,
            max_entry_bytes: MAX_ENTRY_BYTES,
            max_image_bytes: MAX_IMAGE_BYTES,
            max_table_cells: MAX_TABLE_CELLS,
            max_part_bytes: MAX_PART_DECOMPRESSED_BYTES,
            collect_entries: true,
            collect_problems: false,
        }
    }

    pub fn report_only(kind: ImportKind) -> Self {
        Self {
            collect_entries: false,
            ..Self::new(kind)
        }
    }
}

#[derive(Debug)]
pub struct ImportResult {
    pub entries: Vec<Entry>,
    pub report: ValidationReport,
    pub problems: Vec<ProblemRecord>,
}

#[derive(Default)]
struct StyleInfo {
    name: Option<String>,
    outline_lvl: Option<u8>,
}

#[derive(Default)]
struct RelsInfo {
    internal_targets: HashMap<String, String>,
    bible_external_targets: HashMap<String, String>,
    external_domains: BTreeMap<String, usize>,
}

#[derive(Default)]
struct ParagraphBuilder {
    style_id: Option<String>,
    outline_lvl: Option<u8>,
    is_list: bool,
    list_level: u8,
    runs: Vec<(String, SpanStyle)>,
    images: Vec<(Option<String>, Option<String>)>, // (rId, alt)
}

#[derive(Default)]
struct TableBuilder {
    rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    current_cell: String,
    cell_count: usize,
    in_row: bool,
    in_cell: bool,
}

/// Lê e valida um arquivo `.docx` em fluxo, devolvendo a IR (se `collect_entries`) e o relatório.
pub fn import_docx(
    path: impl AsRef<Path>,
    options: &ImportOptions,
) -> Result<ImportResult, ImportError> {
    let started = Instant::now();
    let mut zip = ZipStream::open(path.as_ref())?;

    // 1. Estilos (opcional)
    let styles = if zip.entry_meta("word/styles.xml").is_some() {
        let bytes = zip.read_part_to_vec("word/styles.xml", MAX_META_PART_BYTES)?;
        parse_styles(&bytes)?
    } else {
        HashMap::new()
    };

    // 2. Relações (opcional; nunca segue hiperlinks externos, só extrai domínio)
    let rels = if zip.entry_meta("word/_rels/document.xml.rels").is_some() {
        let bytes = zip.read_part_to_vec("word/_rels/document.xml.rels", MAX_META_PART_BYTES)?;
        parse_rels(&bytes)?
    } else {
        RelsInfo::default()
    };

    // 3. Metadados de mídia (valida teto de 20 MB por imagem sem carregar bytes em RAM)
    let media_names: Vec<String> = zip.media_entries().map(|e| e.name.clone()).collect();
    let mut media_sizes: HashMap<String, u64> = HashMap::with_capacity(media_names.len());
    for name in media_names {
        if let Some(meta) = zip.entry_meta(&name) {
            if meta.uncompressed_size > options.max_image_bytes {
                return Err(ImportError::ImageTooLarge {
                    path: name,
                    size: meta.uncompressed_size,
                    limit: options.max_image_bytes,
                });
            }
        }
        let real_size = match zip.verify_part_size(&name, options.max_image_bytes) {
            Ok(sz) => sz,
            Err(ImportError::ZipBomb { part, limit }) => {
                return Err(ImportError::ImageTooLarge {
                    path: part,
                    size: limit.saturating_add(1),
                    limit,
                });
            }
            Err(err) => return Err(err),
        };
        media_sizes.insert(name, real_size);
    }

    // 4. Streaming de word/document.xml
    let part_reader = zip.open_part("word/document.xml", options.max_part_bytes)?;
    let mut xml = Reader::from_reader(BufReader::with_capacity(64 * 1024, part_reader));
    xml.config_mut().trim_text(false);

    let mut state = ImportState::new(options, styles, rels, media_sizes);
    let mut buf = Vec::with_capacity(8 * 1024);

    loop {
        buf.clear();
        match xml.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => state.on_start(&e)?,
            Ok(Event::Empty(e)) => state.on_empty(&e)?,
            Ok(Event::End(e)) => state.on_end(e.name().as_ref())?,
            Ok(Event::Text(e)) => {
                let text = e.xml_content(XmlVersion::default());
                state.on_text(&text);
            }
            Ok(Event::CData(e)) => {
                let text = e.xml_content(XmlVersion::default());
                state.on_text(&text);
            }
            Ok(Event::GeneralRef(e)) => {
                let decoded = e.xml_content(XmlVersion::default());
                if let Some(ch) = decode_xml_entity(&decoded) {
                    state.on_text(&ch);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => {
                let inner = xml.get_ref().get_ref();
                if inner.bomb_triggered() {
                    return Err(ImportError::ZipBomb {
                        part: inner.part_name().to_string(),
                        limit: inner.limit(),
                    });
                }
                return Err(ImportError::XmlError {
                    part: "word/document.xml".into(),
                    message: err.to_string(),
                });
            }
        }
    }

    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    Ok(state.finish(elapsed_ms))
}

/// Valida um `.docx` em streaming puro sem manter os blocos das entradas em memória.
pub fn validate_docx(
    path: impl AsRef<Path>,
    kind: ImportKind,
) -> Result<ValidationReport, ImportError> {
    let options = ImportOptions::report_only(kind);
    let result = import_docx(path, &options)?;
    Ok(result.report)
}

struct ImportState<'a> {
    options: &'a ImportOptions,
    styles: HashMap<String, StyleInfo>,
    rels: RelsInfo,
    media_sizes: HashMap<String, u64>,

    // Estado XML corrente
    in_p: bool,
    in_ppr: bool,
    in_numpr: bool,
    in_r: bool,
    in_rpr: bool,
    in_t: bool,
    active_bible_hyperlink: Option<String>,
    hyperlink_buf: String,
    cur_style: SpanStyle,
    cur_run_text: String,
    cur_p: ParagraphBuilder,
    table_depth: usize,
    cur_table: TableBuilder,
    last_docpr_alt: Option<String>,

    // Estado da IR e hierarquia
    paragraph_index: usize,
    heading_stack: Vec<(u8, String)>,
    entry_level: Option<u8>,
    pre_level_untagged: Vec<(u8, usize, String)>,
    next_entry_id: usize,
    current_entry: Option<Entry>,
    current_entry_bytes: usize,
    entries: Vec<Entry>,
    problems: Vec<ProblemRecord>,

    // Contadores do relatório
    entries_count: usize,
    hierarchy_by_level: BTreeMap<u8, usize>,
    paragraphs_by_style: BTreeMap<String, usize>,
    keys_by_type: BTreeMap<String, usize>,
    tags_by_type: BTreeMap<String, usize>,
    links_by_type: BTreeMap<String, usize>,
    unbalanced_brackets: usize,
    unclosed_fields: usize,
    suspicious_types: BTreeMap<String, SuspiciousTypeReport>,
    seen_headwords: HashSet<(String, Option<String>)>,
    duplicate_headwords: usize,
    seen_hebrew: HashSet<u32>,
    seen_greek: HashSet<u32>,
    dup_hebrew: usize,
    dup_greek: usize,
    untagged_entries_at_level: usize,
    empty_entries: usize,
    bible_refs_resolved: usize,
    bible_refs_unresolved: usize,
    unresolved_ref_examples: Vec<String>,
    unmarked_bible_refs: usize,
    images_count: usize,
    tables_count: usize,
    list_items_count: usize,
    max_entry_chars: usize,
    peak_streaming_bytes: usize,
}

impl<'a> ImportState<'a> {
    fn new(
        options: &'a ImportOptions,
        styles: HashMap<String, StyleInfo>,
        rels: RelsInfo,
        media_sizes: HashMap<String, u64>,
    ) -> Self {
        Self {
            options,
            styles,
            rels,
            media_sizes,
            in_p: false,
            in_ppr: false,
            in_numpr: false,
            in_r: false,
            in_rpr: false,
            in_t: false,
            active_bible_hyperlink: None,
            hyperlink_buf: String::new(),
            cur_style: SpanStyle::default(),
            cur_run_text: String::new(),
            cur_p: ParagraphBuilder::default(),
            table_depth: 0,
            cur_table: TableBuilder::default(),
            last_docpr_alt: None,
            paragraph_index: 0,
            heading_stack: Vec::new(),
            entry_level: options.entry_level,
            pre_level_untagged: Vec::new(),
            next_entry_id: 1,
            current_entry: None,
            current_entry_bytes: 0,
            entries: Vec::new(),
            problems: Vec::new(),
            entries_count: 0,
            hierarchy_by_level: BTreeMap::new(),
            paragraphs_by_style: BTreeMap::new(),
            keys_by_type: BTreeMap::new(),
            tags_by_type: BTreeMap::new(),
            links_by_type: BTreeMap::new(),
            unbalanced_brackets: 0,
            unclosed_fields: 0,
            suspicious_types: BTreeMap::new(),
            seen_headwords: HashSet::new(),
            duplicate_headwords: 0,
            seen_hebrew: HashSet::new(),
            seen_greek: HashSet::new(),
            dup_hebrew: 0,
            dup_greek: 0,
            untagged_entries_at_level: 0,
            empty_entries: 0,
            bible_refs_resolved: 0,
            bible_refs_unresolved: 0,
            unresolved_ref_examples: Vec::new(),
            unmarked_bible_refs: 0,
            images_count: 0,
            tables_count: 0,
            list_items_count: 0,
            max_entry_chars: 0,
            peak_streaming_bytes: 0,
        }
    }

    fn on_start(&mut self, e: &BytesStart<'_>) -> Result<(), ImportError> {
        let name = e.name();
        let local = local_name(name.as_ref());
        match local {
            "tbl" => {
                self.table_depth += 1;
                if self.table_depth == 1 {
                    self.cur_table = TableBuilder::default();
                }
            }
            "tr" if self.table_depth == 1 => {
                self.cur_table.in_row = true;
                self.cur_table.current_row.clear();
            }
            "tc" if self.table_depth == 1 && self.cur_table.in_row => {
                self.cur_table.cell_count += 1;
                if self.cur_table.cell_count > self.options.max_table_cells {
                    return Err(ImportError::TableTooLarge {
                        cells: self.cur_table.cell_count,
                        limit: self.options.max_table_cells,
                    });
                }
                self.cur_table.in_cell = true;
                self.cur_table.current_cell.clear();
            }
            "p" if self.table_depth == 0 => {
                self.in_p = true;
                self.cur_p = ParagraphBuilder::default();
            }
            "pPr" if self.in_p => self.in_ppr = true,
            "numPr" if self.in_ppr => {
                self.in_numpr = true;
                self.cur_p.is_list = true;
            }
            "r" if self.in_p || self.cur_table.in_cell => {
                self.in_r = true;
                self.cur_style = SpanStyle::default();
                self.cur_run_text.clear();
            }
            "hyperlink" if self.in_p && self.table_depth == 0 => {
                if let Some(rid) = attr_val(e, "id") {
                    if let Some(target) = self.rels.bible_external_targets.get(&rid) {
                        self.active_bible_hyperlink = Some(target.clone());
                        self.hyperlink_buf.clear();
                    }
                }
            }
            "rPr" if self.in_r => self.in_rpr = true,
            "t" if self.in_r => self.in_t = true,
            _ => self.handle_leaf_element(local, e),
        }
        Ok(())
    }

    fn on_empty(&mut self, e: &BytesStart<'_>) -> Result<(), ImportError> {
        let name = e.name();
        let local = local_name(name.as_ref());
        match local {
            "tc" if self.table_depth == 1 && self.cur_table.in_row => {
                self.cur_table.cell_count += 1;
                if self.cur_table.cell_count > self.options.max_table_cells {
                    return Err(ImportError::TableTooLarge {
                        cells: self.cur_table.cell_count,
                        limit: self.options.max_table_cells,
                    });
                }
                self.cur_table.current_row.push(String::new());
            }
            "p" if self.table_depth == 0 => {
                self.paragraph_index += 1;
                *self
                    .paragraphs_by_style
                    .entry("(Normal)".to_string())
                    .or_default() += 1;
            }
            _ => self.handle_leaf_element(local, e),
        }
        Ok(())
    }

    fn handle_leaf_element(&mut self, local: &str, e: &BytesStart<'_>) {
        if self.in_ppr {
            match local {
                "pStyle" => {
                    if let Some(val) = attr_val(e, "val") {
                        self.cur_p.style_id = Some(val);
                    }
                }
                "outlineLvl" => {
                    if let Some(val) = attr_val(e, "val") {
                        if let Ok(n) = val.parse::<u8>() {
                            if n < 9 {
                                self.cur_p.outline_lvl = Some(n + 1);
                            }
                        }
                    }
                }
                "ilvl" if self.in_numpr => {
                    if let Some(val) = attr_val(e, "val") {
                        if let Ok(n) = val.parse::<u8>() {
                            self.cur_p.list_level = n;
                        }
                    }
                }
                _ => {}
            }
        } else if self.in_rpr {
            match local {
                "b" => self.cur_style.bold = !is_attr_false(e, "val"),
                "i" => self.cur_style.italic = !is_attr_false(e, "val"),
                "u" => {
                    self.cur_style.underline = attr_val(e, "val")
                        .map(|v| !v.eq_ignore_ascii_case("none"))
                        .unwrap_or(true);
                }
                "vertAlign"
                    if attr_val(e, "val")
                        .is_some_and(|v| v.eq_ignore_ascii_case("superscript")) =>
                {
                    self.cur_style.superscript = true;
                }
                _ => {}
            }
        } else if self.in_r {
            match local {
                "tab" => self.cur_run_text.push('\t'),
                "br" | "cr" => self.cur_run_text.push('\n'),
                "docPr" => {
                    self.last_docpr_alt = attr_val(e, "descr")
                        .filter(|s| !s.trim().is_empty())
                        .or_else(|| attr_val(e, "name").filter(|s| !s.trim().is_empty()));
                }
                "blip" => {
                    let rid = attr_val(e, "embed").or_else(|| attr_val(e, "link"));
                    self.cur_p.images.push((rid, self.last_docpr_alt.take()));
                }
                "imagedata" => {
                    let rid = attr_val(e, "id");
                    self.cur_p.images.push((rid, self.last_docpr_alt.take()));
                }
                _ => {}
            }
        }
    }

    fn on_text(&mut self, text: &str) {
        if self.in_t {
            if self.table_depth > 0 && self.cur_table.in_cell {
                self.cur_table.current_cell.push_str(text);
            } else if self.in_r {
                self.cur_run_text.push_str(text);
            }
        }
    }

    fn on_end(&mut self, name: &str) -> Result<(), ImportError> {
        let local = local_name(name);
        match local {
            "t" => self.in_t = false,
            "rPr" => self.in_rpr = false,
            "r" => {
                if self.in_p && !self.cur_run_text.is_empty() {
                    let text = std::mem::take(&mut self.cur_run_text);
                    if self.active_bible_hyperlink.is_some() {
                        self.hyperlink_buf.push_str(&text);
                    } else {
                        self.cur_p.runs.push((text, self.cur_style));
                    }
                }
                self.in_r = false;
                self.in_rpr = false;
                self.in_t = false;
            }
            "hyperlink" if self.in_p && self.table_depth == 0 => {
                if let Some(target) = self.active_bible_hyperlink.take() {
                    let label = std::mem::take(&mut self.hyperlink_buf);
                    if !label.is_empty() {
                        // Codifica como link de Bíblia inline preservando o rótulo e o destino tw://
                        let marker = format!("[[{label} >> Bible: {target}]]");
                        self.cur_p.runs.push((marker, self.cur_style));
                    }
                }
            }
            "numPr" => self.in_numpr = false,
            "pPr" => self.in_ppr = false,
            "p" if self.table_depth == 0 && self.in_p => {
                self.in_p = false;
                let p = std::mem::take(&mut self.cur_p);
                self.process_paragraph(p)?;
            }
            "p" if self.table_depth == 1 && self.cur_table.in_cell => {
                if !self.cur_table.current_cell.is_empty() {
                    self.cur_table.current_cell.push('\n');
                }
            }
            "tc" if self.table_depth == 1 && self.cur_table.in_cell => {
                self.cur_table.in_cell = false;
                let cell = std::mem::take(&mut self.cur_table.current_cell)
                    .trim()
                    .to_string();
                self.cur_table.current_row.push(cell);
            }
            "tr" if self.table_depth == 1 && self.cur_table.in_row => {
                self.cur_table.in_row = false;
                let row = std::mem::take(&mut self.cur_table.current_row);
                self.cur_table.rows.push(row);
            }
            "tbl" if self.table_depth > 0 => {
                self.table_depth -= 1;
                if self.table_depth == 0 {
                    let tbl = std::mem::take(&mut self.cur_table);
                    self.tables_count += 1;
                    self.push_block(Block::Table { rows: tbl.rows })?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn resolve_heading_level(&self, p: &ParagraphBuilder) -> Option<u8> {
        if let Some(lvl) = p.outline_lvl {
            return Some(lvl);
        }
        let sid = p.style_id.as_deref()?;
        if let Some(info) = self.styles.get(sid) {
            if let Some(lvl) = info.outline_lvl {
                return Some(lvl);
            }
            if let Some(name) = info.name.as_deref() {
                if let Some(lvl) = heading_level_from_name(name) {
                    return Some(lvl);
                }
            }
        }
        heading_level_from_name(sid)
    }

    fn process_paragraph(&mut self, p: ParagraphBuilder) -> Result<(), ImportError> {
        self.paragraph_index += 1;
        let style_key = p.style_id.clone().unwrap_or_else(|| "(Normal)".to_string());
        *self.paragraphs_by_style.entry(style_key).or_default() += 1;

        let heading_lvl = self.resolve_heading_level(&p);
        let parsed = parse_paragraph_runs(&p.runs, heading_lvl.is_none());

        self.unbalanced_brackets += parsed.unbalanced_brackets;
        for _ in 0..parsed.unbalanced_brackets {
            self.push_problem(ProblemKind::UnbalancedBracket, None);
        }
        self.unclosed_fields += parsed.unclosed_fields;
        for _ in 0..parsed.unclosed_fields {
            self.push_problem(ProblemKind::UnclosedField, None);
        }
        self.unmarked_bible_refs += parsed.unmarked_bible_refs;

        for (tag, _) in &parsed.tags {
            self.record_tag(tag);
        }
        for link in &parsed.links {
            self.record_link(&link.kind, &link.value);
        }

        // Emite imagens referenciadas neste parágrafo
        let mut image_blocks = Vec::new();
        for (rid, alt) in p.images {
            self.images_count += 1;
            let target = rid
                .as_deref()
                .and_then(|id| self.rels.internal_targets.get(id))
                .cloned()
                .unwrap_or_default();
            let full_target = if target.is_empty() || target.starts_with("word/") {
                target
            } else {
                format!("word/{}", target.trim_start_matches('/'))
            };
            let size_bytes = self.media_sizes.get(&full_target).copied().unwrap_or(0);
            let content_type = guess_image_mime(&full_target);
            image_blocks.push(Block::Image {
                target: full_target,
                content_type,
                size_bytes,
                alt,
            });
        }

        if let Some(lvl) = heading_lvl {
            *self.hierarchy_by_level.entry(lvl).or_default() += 1;
            let clean_title = spans_plain_text(&parsed.spans).trim().to_string();
            let tags: Vec<EntryKey> = parsed.tags.into_iter().map(|(k, _)| k).collect();
            self.handle_heading(lvl, clean_title, tags, p.style_id, parsed.spans)?;
            for img in image_blocks {
                self.push_block(img)?;
            }
            return Ok(());
        }

        // Parágrafo comum ou item de lista
        let mut anchors = Vec::new();
        for (k, at_start) in parsed.tags {
            if self.options.kind == ImportKind::Comm && at_start && k.kind == KeyType::Bible {
                self.record_entry_key(&k);
                anchors.push(k);
            } else if self.options.kind == ImportKind::Dict {
                if let Some(entry) = self.current_entry.as_mut() {
                    if matches!(
                        k.kind,
                        KeyType::Headword
                            | KeyType::Louw
                            | KeyType::Twot
                            | KeyType::GreekGk
                            | KeyType::HebrewGk
                    ) {
                        entry.keys.push(k);
                    }
                }
            }
        }

        if self.options.kind == ImportKind::Comm
            && !anchors.is_empty()
            && self.current_entry.is_none()
        {
            self.start_entry(0, anchors.clone());
        }

        if p.is_list {
            self.list_items_count += 1;
            self.push_block(Block::ListItem {
                level: p.list_level,
                ordered: true,
                spans: parsed.spans,
            })?;
        } else if !parsed.spans.is_empty() || !anchors.is_empty() {
            self.push_block(Block::Paragraph {
                style_id: p.style_id,
                spans: parsed.spans,
                anchors,
            })?;
        }

        for img in image_blocks {
            self.push_block(img)?;
        }
        Ok(())
    }

    fn handle_heading(
        &mut self,
        lvl: u8,
        title: String,
        tags: Vec<EntryKey>,
        style_id: Option<String>,
        spans: Vec<Span>,
    ) -> Result<(), ImportError> {
        // Atualiza pilha de títulos
        while self.heading_stack.last().is_some_and(|(l, _)| *l >= lvl) {
            self.heading_stack.pop();
        }
        self.heading_stack.push((lvl, title.clone()));

        match self.options.kind {
            ImportKind::Dict => {
                let has_dict_tag = tags.iter().any(|k| k.kind.is_dict_entry_key());
                if has_dict_tag {
                    match self.entry_level {
                        None => {
                            self.entry_level = Some(lvl);
                            let pre = std::mem::take(&mut self.pre_level_untagged);
                            for (plvl, pnum, ptitle) in pre {
                                if plvl == lvl {
                                    self.untagged_entries_at_level += 1;
                                    self.push_problem_at(
                                        pnum,
                                        ProblemKind::UntaggedHeading,
                                        &ptitle,
                                        None,
                                    );
                                }
                            }
                        }
                        Some(el) if lvl < el && self.options.entry_level.is_none() => {
                            self.entry_level = Some(lvl);
                        }
                        _ => {}
                    }
                    let el = self.entry_level.unwrap_or(lvl);
                    if lvl <= el {
                        self.flush_current_entry();
                        for k in &tags {
                            self.record_entry_key(k);
                        }
                        self.start_entry(lvl, tags);
                    } else {
                        self.push_block(Block::Paragraph {
                            style_id,
                            spans,
                            anchors: tags,
                        })?;
                    }
                } else {
                    match self.entry_level {
                        None => {
                            self.pre_level_untagged
                                .push((lvl, self.paragraph_index, title));
                        }
                        Some(el) if lvl < el => {
                            // Título de hierarquia (letra/faixa): fecha entrada anterior.
                            self.flush_current_entry();
                        }
                        Some(el) if lvl == el => {
                            // Título no nível de entrada sem etiqueta: fecha anterior e contabiliza.
                            self.flush_current_entry();
                            self.untagged_entries_at_level += 1;
                            self.push_problem(ProblemKind::UntaggedHeading, None);
                            self.start_entry(lvl, Vec::new());
                        }
                        Some(_) => {
                            // Subtítulo interno à entrada atual.
                            self.push_block(Block::Paragraph {
                                style_id,
                                spans,
                                anchors: Vec::new(),
                            })?;
                        }
                    }
                }
            }
            ImportKind::Comm => {
                let has_bible_tag = tags.iter().any(|k| k.kind == KeyType::Bible);
                if has_bible_tag {
                    if self.options.entry_level.is_none()
                        && (self.entry_level.is_none()
                            || self.entry_level.is_some_and(|el| lvl < el))
                    {
                        self.entry_level = Some(lvl);
                        let pre = std::mem::take(&mut self.pre_level_untagged);
                        for (plvl, pnum, ptitle) in pre {
                            if plvl == lvl {
                                self.untagged_entries_at_level += 1;
                                self.push_problem_at(
                                    pnum,
                                    ProblemKind::UntaggedHeading,
                                    &ptitle,
                                    None,
                                );
                            }
                        }
                    }
                    self.flush_current_entry();
                    for k in &tags {
                        self.record_entry_key(k);
                    }
                    self.start_entry(lvl, tags);
                } else if let Some(el) = self.entry_level {
                    if lvl < el {
                        self.flush_current_entry();
                    } else if lvl == el {
                        self.flush_current_entry();
                        self.untagged_entries_at_level += 1;
                        self.push_problem(ProblemKind::UntaggedHeading, None);
                        self.start_entry(lvl, Vec::new());
                    } else {
                        self.push_block(Block::Paragraph {
                            style_id,
                            spans,
                            anchors: Vec::new(),
                        })?;
                    }
                } else {
                    self.pre_level_untagged
                        .push((lvl, self.paragraph_index, title));
                }
            }
            ImportKind::Book => {
                let target_lvl = self.options.entry_level.unwrap_or(u8::MAX);
                if lvl <= target_lvl {
                    self.flush_current_entry();
                    for k in &tags {
                        self.record_entry_key(k);
                    }
                    self.start_entry(lvl, tags);
                } else {
                    self.push_block(Block::Paragraph {
                        style_id,
                        spans,
                        anchors: tags,
                    })?;
                }
            }
        }
        Ok(())
    }

    fn start_entry(&mut self, level: u8, keys: Vec<EntryKey>) {
        let id = self.next_entry_id;
        self.next_entry_id += 1;
        let title_path: Vec<String> = self.heading_stack.iter().map(|(_, t)| t.clone()).collect();
        let entry = Entry {
            id,
            level,
            title_path,
            keys,
            blocks: Vec::new(),
            char_count: 0,
        };
        self.current_entry_bytes = entry.estimated_bytes();
        self.current_entry = Some(entry);
    }

    fn push_block(&mut self, block: Block) -> Result<(), ImportError> {
        let Some(entry) = self.current_entry.as_mut() else {
            return Ok(());
        };
        let block_bytes = block.estimated_bytes();
        self.current_entry_bytes = self.current_entry_bytes.saturating_add(block_bytes);
        if self.current_entry_bytes > self.options.max_entry_bytes {
            return Err(ImportError::EntryTooLarge {
                entry_id: entry.id,
                limit: self.options.max_entry_bytes,
            });
        }
        entry.char_count = entry.char_count.saturating_add(block.char_count());
        entry.blocks.push(block);
        Ok(())
    }

    fn flush_current_entry(&mut self) {
        let Some(entry) = self.current_entry.take() else {
            return;
        };
        self.entries_count += 1;
        if entry.char_count > self.max_entry_chars {
            self.max_entry_chars = entry.char_count;
        }
        if entry.blocks.is_empty() || entry.blocks.iter().all(Block::is_effectively_empty) {
            self.empty_entries += 1;
        }
        if self.current_entry_bytes > self.peak_streaming_bytes {
            self.peak_streaming_bytes = self.current_entry_bytes;
        }
        self.current_entry_bytes = 0;
        if self.options.collect_entries {
            self.entries.push(entry);
        }
    }

    fn record_tag(&mut self, key: &EntryKey) {
        let label = format_key_type_label(&key.kind, key.lang.as_deref());
        *self.tags_by_type.entry(label).or_default() += 1;
        self.check_suspicious(&key.kind);
        if key.kind == KeyType::Bible {
            self.check_bible_ref(&key.value);
        }
    }

    fn record_link(&mut self, kind: &KeyType, value: &str) {
        *self
            .links_by_type
            .entry(kind.canonical_name().to_string())
            .or_default() += 1;
        self.check_suspicious(kind);
        if *kind == KeyType::Bible {
            self.check_bible_ref(value);
        }
    }

    fn record_entry_key(&mut self, key: &EntryKey) {
        let label = format_key_type_label(&key.kind, key.lang.as_deref());
        *self.keys_by_type.entry(label).or_default() += 1;

        match key.kind {
            KeyType::Headword => {
                let norm = (key.value.trim().to_lowercase(), key.lang.clone());
                if !norm.0.is_empty() && !self.seen_headwords.insert(norm) {
                    self.duplicate_headwords += 1;
                    self.push_problem(ProblemKind::DuplicateEntry, None);
                }
            }
            KeyType::HebrewStrongs | KeyType::GreekStrongs => {
                if let Some((lang, num)) = parse_strong_number(&key.kind, &key.value) {
                    match lang {
                        StrongLang::Hebrew => {
                            if !self.seen_hebrew.insert(num) {
                                self.dup_hebrew += 1;
                                self.push_problem(
                                    ProblemKind::DuplicateEntry,
                                    Some(format!("H{num}")),
                                );
                            }
                        }
                        StrongLang::Greek => {
                            if !self.seen_greek.insert(num) {
                                self.dup_greek += 1;
                                self.push_problem(
                                    ProblemKind::DuplicateEntry,
                                    Some(format!("G{num}")),
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn check_suspicious(&mut self, kind: &KeyType) {
        if let KeyType::Unknown(raw) = kind {
            if let Some(suggestion) = suggest_known_type(raw) {
                let entry = self.suspicious_types.entry(raw.clone()).or_insert_with(|| {
                    SuspiciousTypeReport {
                        count: 0,
                        suggestion: suggestion.to_string(),
                    }
                });
                entry.count += 1;
                self.push_problem(
                    ProblemKind::SuspiciousType,
                    Some(format!("{raw}->{suggestion}")),
                );
            }
        }
    }

    fn check_bible_ref(&mut self, raw: &str) {
        if resolve_bible_ref(raw) {
            self.bible_refs_resolved += 1;
        } else {
            self.bible_refs_unresolved += 1;
            let trimmed = raw.trim();
            let short: String = trimmed.chars().take(60).collect();
            self.push_problem(ProblemKind::UnresolvedRef, Some(short.clone()));
            if self.unresolved_ref_examples.len() < 10
                && !self.unresolved_ref_examples.contains(&short)
            {
                self.unresolved_ref_examples.push(short);
            }
        }
    }

    fn push_problem(&mut self, kind: ProblemKind, detail: Option<String>) {
        if !self.options.collect_problems {
            return;
        }
        let title = self
            .heading_stack
            .last()
            .map(|(_, t)| t.as_str())
            .unwrap_or("(início)");
        self.problems.push(ProblemRecord {
            paragraph: self.paragraph_index,
            kind,
            section_title: truncate_section_title(title, 40),
            detail,
        });
    }

    fn push_problem_at(
        &mut self,
        paragraph: usize,
        kind: ProblemKind,
        title: &str,
        detail: Option<String>,
    ) {
        if !self.options.collect_problems {
            return;
        }
        self.problems.push(ProblemRecord {
            paragraph,
            kind,
            section_title: truncate_section_title(title, 40),
            detail,
        });
    }

    fn finish(mut self, elapsed_ms: f64) -> ImportResult {
        self.flush_current_entry();

        let hebrew_stats = strong_stats(&self.seen_hebrew, self.dup_hebrew);
        let greek_stats = strong_stats(&self.seen_greek, self.dup_greek);

        let estimated_memory_bytes = if self.options.collect_entries {
            self.entries.iter().map(Entry::estimated_bytes).sum()
        } else {
            // Memória de trabalho em streaming: pico de 1 entrada + sets de dedup + buffers
            let dedup_bytes = self.seen_headwords.len() * 48
                + (self.seen_hebrew.len() + self.seen_greek.len()) * 8;
            self.peak_streaming_bytes + dedup_bytes + 64 * 1024
        };

        let report = ValidationReport {
            kind: self.options.kind,
            entries: self.entries_count,
            entry_level: self.entry_level,
            hierarchy_by_level: self.hierarchy_by_level,
            paragraphs_by_style: self.paragraphs_by_style,
            keys_by_type: self.keys_by_type,
            tags_by_type: self.tags_by_type,
            links_by_type: self.links_by_type,
            unbalanced_brackets: self.unbalanced_brackets,
            unclosed_fields: self.unclosed_fields,
            suspicious_types: self.suspicious_types,
            duplicate_headwords: self.duplicate_headwords,
            duplicate_strongs: self.dup_hebrew + self.dup_greek,
            strong_gaps: StrongGapsReport {
                hebrew: hebrew_stats,
                greek: greek_stats,
            },
            untagged_entries_at_level: self.untagged_entries_at_level,
            empty_entries: self.empty_entries,
            bible_refs_resolved: self.bible_refs_resolved,
            bible_refs_unresolved: self.bible_refs_unresolved,
            unresolved_ref_examples: self.unresolved_ref_examples,
            unmarked_bible_refs: self.unmarked_bible_refs,
            external_hyperlinks_by_domain: self.rels.external_domains,
            images: self.images_count,
            tables: self.tables_count,
            list_items: self.list_items_count,
            max_entry_chars: self.max_entry_chars,
            elapsed_ms,
            estimated_memory_bytes,
        };

        ImportResult {
            entries: self.entries,
            report,
            problems: self.problems,
        }
    }
}

fn strong_stats(seen: &HashSet<u32>, duplicates: usize) -> StrongRangeStats {
    if seen.is_empty() {
        return StrongRangeStats::default();
    }
    let min = seen.iter().copied().min();
    let max = seen.iter().copied().max();
    let gaps = match (min, max) {
        (Some(lo), Some(hi)) if hi >= lo => ((hi - lo) as usize + 1).saturating_sub(seen.len()),
        _ => 0,
    };
    StrongRangeStats {
        unique_count: seen.len(),
        duplicates,
        min,
        max,
        gaps,
    }
}

fn format_key_type_label(kind: &KeyType, lang: Option<&str>) -> String {
    match lang {
        Some(l) => format!("{}+{l}", kind.canonical_name()),
        None => kind.canonical_name().to_string(),
    }
}

fn parse_styles(bytes: &[u8]) -> Result<HashMap<String, StyleInfo>, ImportError> {
    let mut reader = Reader::from_reader(bytes);
    let mut styles = HashMap::new();
    let mut cur_id: Option<String> = None;
    let mut cur_info = StyleInfo::default();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let local = local_name(name.as_ref());
                if local == "style" {
                    cur_id = attr_val(&e, "styleId");
                    cur_info = StyleInfo::default();
                } else if cur_id.is_some() {
                    read_style_child(local, &e, &mut cur_info);
                }
            }
            Ok(Event::Empty(e)) => {
                if cur_id.is_some() {
                    read_style_child(local_name(e.name().as_ref()), &e, &mut cur_info);
                }
            }
            Ok(Event::End(e)) => {
                if local_name(e.name().as_ref()) == "style" {
                    if let Some(id) = cur_id.take() {
                        styles.insert(id, std::mem::take(&mut cur_info));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => {
                return Err(ImportError::XmlError {
                    part: "word/styles.xml".into(),
                    message: err.to_string(),
                });
            }
        }
    }
    Ok(styles)
}

fn read_style_child(local: &str, e: &BytesStart<'_>, info: &mut StyleInfo) {
    match local {
        "name" => info.name = attr_val(e, "val"),
        "outlineLvl" => {
            if let Some(v) = attr_val(e, "val") {
                if let Ok(n) = v.parse::<u8>() {
                    if n < 9 {
                        info.outline_lvl = Some(n + 1);
                    }
                }
            }
        }
        _ => {}
    }
}

fn parse_rels(bytes: &[u8]) -> Result<RelsInfo, ImportError> {
    let mut reader = Reader::from_reader(bytes);
    let mut info = RelsInfo::default();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if local_name(e.name().as_ref()) == "Relationship" {
                    let id = attr_val(&e, "Id");
                    let target = attr_val(&e, "Target");
                    let mode = attr_val(&e, "TargetMode");
                    if let Some(tgt) = target {
                        if mode
                            .as_deref()
                            .is_some_and(|m| m.eq_ignore_ascii_case("External"))
                        {
                            let dom = extract_domain_only(&tgt);
                            *info.external_domains.entry(dom.clone()).or_default() += 1;
                            if dom.starts_with("bible.")
                                || tgt.to_ascii_lowercase().starts_with("tw://bible.")
                            {
                                if let Some(rid) = id {
                                    let clean =
                                        tgt.split('|').next().unwrap_or(&tgt).trim().to_string();
                                    info.bible_external_targets.insert(rid, clean);
                                }
                            }
                        } else if let Some(rid) = id {
                            info.internal_targets.insert(rid, tgt);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(err) => {
                return Err(ImportError::XmlError {
                    part: "word/_rels/document.xml.rels".into(),
                    message: err.to_string(),
                });
            }
        }
    }
    Ok(info)
}

/// Extrai apenas o domínio (ou esquema) de um hiperlink externo, sem nunca segui-lo.
fn extract_domain_only(target: &str) -> String {
    let trimmed = target.trim();
    if let Some((scheme, rest)) = trimmed.split_once("://") {
        let host_port = rest.split(['/', '?', '#']).next().unwrap_or("");
        let host = host_port.split('@').next_back().unwrap_or(host_port);
        if host.is_empty() {
            return scheme.to_ascii_lowercase();
        }
        return host.to_ascii_lowercase();
    }
    if let Some((scheme, _)) = trimmed.split_once(':') {
        return scheme.to_ascii_lowercase();
    }
    "(unknown)".to_string()
}

fn heading_level_from_name(name: &str) -> Option<u8> {
    let lower = name.trim().to_ascii_lowercase();
    for prefix in ["heading", "título", "titulo", "ttulo"] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let digits = rest.trim();
            if let Ok(n) = digits.parse::<u8>() {
                if (1..=9).contains(&n) {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn spans_plain_text(spans: &[Span]) -> String {
    let mut out = String::new();
    for s in spans {
        match s {
            Span::Text { text, .. } => out.push_str(text),
            Span::Link { link, .. } => {
                out.push_str(link.label.as_deref().unwrap_or(link.value.as_str()));
            }
        }
    }
    out
}

fn guess_image_mime(path: &str) -> Option<String> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "emf" => "image/x-emf",
        "wmf" => "image/x-wmf",
        _ => return None,
    };
    Some(mime.to_string())
}

fn local_name(full: &str) -> &str {
    match full.rsplit_once(':') {
        Some((_, local)) => local,
        None => full,
    }
}

fn attr_val(e: &BytesStart<'_>, wanted_local: &str) -> Option<String> {
    for attr in e.attributes().flatten() {
        if local_name(attr.key.as_ref()) == wanted_local {
            return attr
                .normalized_value(XmlVersion::default())
                .ok()
                .map(|v| v.into_owned());
        }
    }
    None
}

fn is_attr_false(e: &BytesStart<'_>, wanted_local: &str) -> bool {
    attr_val(e, wanted_local).is_some_and(|v| v == "0" || v.eq_ignore_ascii_case("false"))
}

fn decode_xml_entity(entity: &str) -> Option<String> {
    match entity {
        "amp" => Some("&".into()),
        "lt" => Some("<".into()),
        "gt" => Some(">".into()),
        "quot" => Some("\"".into()),
        "apos" => Some("'".into()),
        other => {
            let code = if let Some(hex) = other
                .strip_prefix("#x")
                .or_else(|| other.strip_prefix("#X"))
            {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                let dec = other.strip_prefix('#')?;
                dec.parse::<u32>().ok()?
            };
            char::from_u32(code).map(|c| c.to_string())
        }
    }
}
