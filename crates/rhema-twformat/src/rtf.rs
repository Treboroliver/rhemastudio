use std::collections::BTreeMap;

use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use rhema_core::{LinkTarget, ModuleError, Node, VerseRange, VerseRef};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RtfStats {
    pub control_words: BTreeMap<String, usize>,
    pub unknown_controls: BTreeMap<String, usize>,
    pub destinations: BTreeMap<String, usize>,
    pub unknown_destinations: BTreeMap<String, usize>,
    pub invalid_hex_escapes: usize,
    pub unknown_codepages: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtfConversion {
    pub nodes: Vec<Node>,
    pub stats: RtfStats,
}

#[derive(Clone, Default)]
struct FormatState {
    codepage: u16,
    unicode_fallback: usize,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    caps: bool,
    superscript: bool,
    subscript: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Destination {
    Normal,
    FieldInstruction,
    Skip,
}

struct Frame {
    state: FormatState,
    nodes: Vec<Node>,
    destination: Destination,
    field: bool,
    field_instruction: Option<String>,
    ignorable: bool,
}

impl Frame {
    fn root() -> Self {
        Self {
            state: FormatState {
                codepage: 1252,
                unicode_fallback: 1,
                ..FormatState::default()
            },
            nodes: Vec::new(),
            destination: Destination::Normal,
            field: false,
            field_instruction: None,
            ignorable: false,
        }
    }

    fn child(&self) -> Self {
        Self {
            state: self.state.clone(),
            nodes: Vec::new(),
            destination: self.destination,
            field: false,
            field_instruction: None,
            ignorable: false,
        }
    }
}

/// Converte RTF em nós, rejeitando conteúdo que não tenha cabeçalho RTF.
pub fn convert_rtf(input: &[u8]) -> Result<Vec<Node>, ModuleError> {
    convert_rtf_with_stats(input).map(|conversion| conversion.nodes)
}

/// Converte RTF e devolve contagens de controles desconhecidos e destinos especiais.
pub fn convert_rtf_with_stats(input: &[u8]) -> Result<RtfConversion, ModuleError> {
    convert_windowed(input, None).map(|(conversion, _)| conversion)
}

/// Uma página de conteúdo RTF convertido sob demanda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtfPage {
    pub nodes: Vec<Node>,
    /// Há conteúdo visível depois desta página.
    pub has_more: bool,
    /// Cursor da próxima página (ver [`page_cursor`]).
    pub next_offset: u64,
}

/// Codifica uma posição: parágrafo e caractere dentro dele.
pub fn page_cursor(paragraph: usize, character: usize) -> u64 {
    ((paragraph as u64) << 32) | (character as u64).min(u64::from(u32::MAX))
}

fn split_cursor(cursor: u64) -> (usize, usize) {
    (
        (cursor >> 32) as usize,
        (cursor & u64::from(u32::MAX)) as usize,
    )
}

/// Janela de leitura: até `max_paragraphs` parágrafos e `max_chars` caracteres a partir de
/// `(start, skip)`. Um parágrafo termina em cada `\par`.
struct Window {
    start: usize,
    skip: usize,
    limit: usize,
    max_chars: usize,
    emitted: usize,
    index: usize,
    in_paragraph: usize,
    more: bool,
    stop: bool,
    next: Option<u64>,
}

impl Window {
    /// Já passou da posição inicial?
    fn reached_start(&self) -> bool {
        self.index > self.start || (self.index == self.start && self.in_paragraph >= self.skip)
    }

    fn stop_before(&mut self, character: usize) {
        self.more = true;
        self.stop = true;
        self.next = Some(page_cursor(self.index, character));
    }

    /// Recorta um trecho de texto para a janela, contando os caracteres percorridos.
    fn slice(&mut self, text: &str) -> Option<String> {
        let count = text.chars().count();
        let before = self.in_paragraph;
        self.in_paragraph += count;
        let skipped = if self.index < self.start {
            count
        } else if self.index == self.start {
            self.skip.saturating_sub(before).min(count)
        } else {
            0
        };
        if skipped == count {
            return None;
        }
        let rest = text.chars().skip(skipped).collect::<String>();
        let remaining = if self.index >= self.limit {
            0
        } else {
            self.max_chars.saturating_sub(self.emitted)
        };
        let rest_count = count - skipped;
        if rest_count <= remaining {
            self.emitted += rest_count;
            return Some(rest);
        }
        let take = rest.chars().take(remaining).collect::<String>();
        let tail = rest.chars().skip(remaining).collect::<String>();
        if tail.trim().is_empty() {
            self.emitted += rest_count;
            return Some(rest);
        }
        self.emitted += remaining;
        self.stop_before(before + skipped + remaining);
        (!take.is_empty()).then_some(take)
    }
}

/// Converte uma página: no máximo `max_paragraphs` parágrafos e `max_chars` caracteres a
/// partir do cursor `offset` (zero é o início).
///
/// O texto anterior à janela é percorrido, mas não vira nós; a leitura termina assim que
/// aparece conteúdo visível depois da janela. Assim, entradas de centenas de KB nunca são
/// convertidas por inteiro e um parágrafo gigante também é dividido.
pub fn convert_rtf_page(
    input: &[u8],
    offset: u64,
    max_paragraphs: usize,
    max_chars: usize,
) -> Result<RtfPage, ModuleError> {
    let (start, skip) = split_cursor(offset);
    let window = Window {
        start,
        skip,
        limit: start.saturating_add(max_paragraphs.max(1)),
        max_chars: max_chars.max(1),
        emitted: 0,
        index: 0,
        in_paragraph: 0,
        more: false,
        stop: false,
        next: None,
    };
    let (conversion, window) = convert_windowed(input, Some(window))?;
    let (has_more, next) = window.map_or((false, None), |window| (window.more, window.next));
    Ok(RtfPage {
        nodes: conversion.nodes,
        has_more,
        next_offset: next
            .unwrap_or_else(|| page_cursor(start.saturating_add(max_paragraphs.max(1)), 0)),
    })
}

fn convert_windowed(
    input: &[u8],
    window: Option<Window>,
) -> Result<(RtfConversion, Option<Window>), ModuleError> {
    let header_offset = input
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(input.len());
    if !input[header_offset..].starts_with(b"{\\rtf") {
        return Err(ModuleError::Corrupted(
            "conteúdo declarado como RTF sem cabeçalho RTF".to_string(),
        ));
    }

    let mut parser = Parser {
        input,
        cursor: 0,
        frames: vec![Frame::root()],
        stats: RtfStats::default(),
        fallback_remaining: 0,
        pending_high_surrogate: None,
        ansi_run: Vec::new(),
        window,
    };
    parser.parse();
    let mut root = parser.frames.pop().unwrap_or_else(Frame::root);
    while let Some(mut frame) = parser.frames.pop() {
        if frame.destination != Destination::Skip {
            frame.nodes.append(&mut root.nodes);
        }
        root = frame;
    }
    Ok((
        RtfConversion {
            nodes: root.nodes,
            stats: parser.stats,
        },
        parser.window,
    ))
}

struct Parser<'a> {
    input: &'a [u8],
    cursor: usize,
    frames: Vec<Frame>,
    stats: RtfStats,
    fallback_remaining: usize,
    pending_high_surrogate: Option<u16>,
    ansi_run: Vec<u8>,
    window: Option<Window>,
}

impl Parser<'_> {
    fn parse(&mut self) {
        while self.cursor < self.input.len() && !self.window.as_ref().is_some_and(|w| w.stop) {
            let byte = self.input[self.cursor];
            if self.fallback_remaining > 0 && byte != b'{' && byte != b'}' {
                self.skip_fallback();
                continue;
            }
            match byte {
                b'{' => {
                    self.flush_ansi();
                    self.cursor += 1;
                    let child = self.current().child();
                    self.frames.push(child);
                }
                b'}' => {
                    self.flush_ansi();
                    self.cursor += 1;
                    self.close_group();
                }
                b'\\' => self.parse_escape(),
                _ => {
                    self.ansi_run.push(byte);
                    self.cursor += 1;
                }
            }
        }
        self.flush_ansi();
        self.flush_pending_surrogate();
    }

    fn current(&mut self) -> &mut Frame {
        let index = self.frames.len().saturating_sub(1);
        &mut self.frames[index]
    }

    fn parse_escape(&mut self) {
        self.flush_ansi();
        self.cursor += 1;
        if self.cursor >= self.input.len() {
            return;
        }
        let next = self.input[self.cursor];
        if next == b'\'' {
            self.parse_hex_escape();
        } else if next.is_ascii_alphabetic() {
            self.parse_control_word();
        } else {
            self.cursor += 1;
            *self
                .stats
                .control_words
                .entry(format!("\\{}", next as char))
                .or_default() += 1;
            match next {
                b'\\' | b'{' | b'}' => self.append_text(&(next as char).to_string()),
                b'~' => self.append_text("\u{00a0}"),
                b'_' => self.append_text("\u{2011}"),
                b'-' => self.append_text("\u{00ad}"),
                b'*' => self.current().ignorable = true,
                _ => self.count_unknown_control(format!("\\{}", next as char)),
            }
        }
    }

    fn parse_hex_escape(&mut self) {
        self.cursor += 1;
        let end = self.cursor.saturating_add(2);
        if end > self.input.len() {
            self.stats.invalid_hex_escapes += 1;
            self.cursor = self.input.len();
            return;
        }
        let value = hex_pair(self.input[self.cursor], self.input[self.cursor + 1]);
        self.cursor = end;
        *self
            .stats
            .control_words
            .entry("\\'hh".to_string())
            .or_default() += 1;
        if let Some(value) = value {
            self.ansi_run.push(value);
        } else {
            self.stats.invalid_hex_escapes += 1;
        }
    }

    fn parse_control_word(&mut self) {
        let name_start = self.cursor;
        while self.cursor < self.input.len() && self.input[self.cursor].is_ascii_alphabetic() {
            self.cursor += 1;
        }
        let name = String::from_utf8_lossy(&self.input[name_start..self.cursor]).to_string();
        let number_start = self.cursor;
        if self.cursor < self.input.len() && matches!(self.input[self.cursor], b'-' | b'+') {
            self.cursor += 1;
        }
        while self.cursor < self.input.len() && self.input[self.cursor].is_ascii_digit() {
            self.cursor += 1;
        }
        let parameter = if self.cursor > number_start
            && self.input[number_start..self.cursor]
                .iter()
                .any(u8::is_ascii_digit)
        {
            std::str::from_utf8(&self.input[number_start..self.cursor])
                .ok()
                .and_then(|value| value.parse::<i32>().ok())
        } else {
            self.cursor = number_start;
            None
        };
        if self.cursor < self.input.len() && self.input[self.cursor] == b' ' {
            self.cursor += 1;
        }
        *self.stats.control_words.entry(name.clone()).or_default() += 1;

        if self.current().destination == Destination::Skip {
            return;
        }
        if self.current().destination == Destination::FieldInstruction {
            if name == "fldrslt" {
                self.current().destination = Destination::Normal;
            }
            return;
        }
        self.apply_control(&name, parameter);
    }

    fn apply_control(&mut self, name: &str, parameter: Option<i32>) {
        match name {
            "rtf" | "ansi" | "deff" | "deflang" | "deflangfe" | "fbidis" | "ltrdoc" | "ltrch"
            | "rtlch" | "ltrpar" | "widctlpar" | "keep" | "keepn" | "ql" | "qj" | "qc" | "qr"
            | "li" | "ri" | "fi" | "sb" | "sa" | "sl" | "slmult" | "fs" | "f" | "cf" | "cb"
            | "red" | "green" | "blue" | "s" | "sbasedon" | "snext" | "ssemihidden"
            | "sunhideused" | "fnil" | "additive" | "lang" | "langfe" | "chcbpat" | "ulc"
            | "brdrb" | "brdrbtw" | "brdrcf" | "brdrdb" | "brdrs" | "brdrw" | "brsp" | "picw"
            | "pich" | "picwgoal" | "pichgoal" | "picscalex" | "picscaley" | "jpegblip"
            | "pngblip" | "bkmkstart" | "bkmkend" | "intbl" | "cell" | "cellx" | "row"
            | "trowd" | "trgaph" | "trleft" | "trpaddl" | "trpaddt" | "trpaddr" | "trpaddb"
            | "trpaddfl" | "trpaddft" | "trpaddfr" | "trpaddfb" | "trspdl" | "trspdfl"
            | "trspdr" | "trspdfr" | "trspdt" | "trspdft" | "trspdb" | "trspdfb" | "trwWidth"
            | "trftsWidth" | "richviewtbw" | "richviewcbw" | "richviewcbh" | "clwWidth"
            | "clftsWidth" | "clbrdrr" | "clbrdrl" | "clbrdrb" | "clbrdrt" | "clvmrg"
            | "clvmgf" | "lastrow" | "tx" | "tqc" | "tqr" | "itap" | "cbpat" => {}
            "ansicpg" => {
                let requested = parameter.unwrap_or(1252).clamp(0, u16::MAX as i32) as u16;
                let codepage = if requested == 0 { 1252 } else { requested };
                if encoding_for_codepage(codepage).is_none() {
                    let label = format!("windows-{codepage}");
                    *self
                        .stats
                        .unknown_codepages
                        .entry(label.clone())
                        .or_default() += 1;
                    tracing::warn!(codepage, "RTF contém uma página de códigos desconhecida");
                    self.current().state.codepage = 1252;
                } else {
                    self.current().state.codepage = codepage;
                }
            }
            "bin" => {
                let byte_count = parameter.unwrap_or(0).max(0) as usize;
                self.cursor = self.cursor.saturating_add(byte_count).min(self.input.len());
            }
            "uc" => {
                self.current().state.unicode_fallback =
                    parameter.unwrap_or(1).clamp(0, 32) as usize;
            }
            "u" => {
                if let Some(value) = parameter {
                    let unit = value.rem_euclid(65536) as u16;
                    self.append_unicode_unit(unit);
                    self.fallback_remaining = self.current().state.unicode_fallback;
                } else {
                    self.count_unknown_control(name.to_string());
                }
            }
            "b" => self.current().state.bold = parameter != Some(0),
            "i" => self.current().state.italic = parameter != Some(0),
            "ul" => self.current().state.underline = parameter != Some(0),
            "ulnone" => self.current().state.underline = false,
            "strike" => self.current().state.strike = parameter != Some(0),
            "caps" => self.current().state.caps = parameter != Some(0),
            "super" | "up" => {
                self.current().state.superscript = true;
                self.current().state.subscript = false;
            }
            "sub" | "dn" => {
                self.current().state.subscript = true;
                self.current().state.superscript = false;
            }
            "nosupersub" => {
                self.current().state.superscript = false;
                self.current().state.subscript = false;
            }
            "plain" => {
                let state = &mut self.current().state;
                state.bold = false;
                state.italic = false;
                state.underline = false;
                state.strike = false;
                state.caps = false;
                state.superscript = false;
                state.subscript = false;
            }
            "pard" => {}
            "par" => self.append_node(Node::Paragraph),
            "line" => self.append_node(Node::LineBreak),
            "tab" => self.append_text("\t"),
            "bullet" => self.append_text("•"),
            "emdash" => self.append_text("\u{2014}"),
            "endash" => self.append_text("\u{2013}"),
            "lquote" => self.append_text("\u{2018}"),
            "rquote" => self.append_text("\u{2019}"),
            "ldblquote" => self.append_text("\u{201c}"),
            "rdblquote" => self.append_text("\u{201d}"),
            "fonttbl" | "colortbl" | "stylesheet" | "info" | "pict" | "object" | "header"
            | "footer" | "generator" | "xmlopen" | "xmlclose" | "notvatxbx" => {
                self.set_destination(name, false);
            }
            "field" => self.current().field = true,
            "fldinst" => {
                self.current().destination = Destination::FieldInstruction;
                self.current().field_instruction = Some(String::new());
                self.current().ignorable = false;
            }
            "fldrslt" => {
                self.current().destination = Destination::Normal;
                self.current().ignorable = false;
            }
            "fcharset" => {}
            _ => {
                if self.current().ignorable {
                    self.set_destination(name, true);
                } else {
                    self.count_unknown_control(name.to_string());
                }
            }
        }
    }

    fn set_destination(&mut self, name: &str, unknown: bool) {
        *self.stats.destinations.entry(name.to_string()).or_default() += 1;
        if unknown {
            *self
                .stats
                .unknown_destinations
                .entry(name.to_string())
                .or_default() += 1;
        }
        let current = self.current();
        current.destination = Destination::Skip;
    }

    fn skip_fallback(&mut self) {
        match self.input[self.cursor] {
            b'\\' if self.cursor + 1 < self.input.len() => match self.input[self.cursor + 1] {
                b'\'' if self.cursor + 3 < self.input.len() => self.cursor += 4,
                symbol if !symbol.is_ascii_alphabetic() => self.cursor += 2,
                _ => {
                    self.cursor += 2;
                    while self.cursor < self.input.len()
                        && self.input[self.cursor].is_ascii_alphabetic()
                    {
                        self.cursor += 1;
                    }
                    while self.cursor < self.input.len() && self.input[self.cursor].is_ascii_digit()
                    {
                        self.cursor += 1;
                    }
                    if self.cursor < self.input.len() && self.input[self.cursor] == b' ' {
                        self.cursor += 1;
                    }
                }
            },
            _ => self.cursor += 1,
        }
        self.fallback_remaining = self.fallback_remaining.saturating_sub(1);
    }

    fn append_unicode_unit(&mut self, unit: u16) {
        if (0xd800..=0xdbff).contains(&unit) {
            self.flush_pending_surrogate();
            self.pending_high_surrogate = Some(unit);
        } else if (0xdc00..=0xdfff).contains(&unit) {
            if let Some(high) = self.pending_high_surrogate.take() {
                let scalar =
                    0x10000 + ((u32::from(high) - 0xd800) << 10) + (u32::from(unit) - 0xdc00);
                if let Some(character) = char::from_u32(scalar) {
                    self.append_text(&character.to_string());
                }
            } else {
                self.append_text("\u{fffd}");
            }
        } else {
            self.flush_pending_surrogate();
            if let Some(character) = char::from_u32(u32::from(unit)) {
                self.append_text(&character.to_string());
            }
        }
    }

    fn flush_pending_surrogate(&mut self) {
        if self.pending_high_surrogate.take().is_some() {
            self.append_text("\u{fffd}");
        }
    }

    fn flush_ansi(&mut self) {
        if self.ansi_run.is_empty() {
            return;
        }
        let bytes = std::mem::take(&mut self.ansi_run);
        let codepage = self.current().state.codepage;
        let encoding = encoding_for_codepage(codepage).unwrap_or(WINDOWS_1252);
        let (decoded, _, _) = encoding.decode(&bytes);
        self.append_text(&decoded);
    }

    fn append_text(&mut self, text: &str) {
        if text.is_empty() || self.current().destination == Destination::Skip {
            return;
        }
        if self.pending_high_surrogate.is_some() {
            self.flush_pending_surrogate();
        }
        if self.current().destination == Destination::FieldInstruction {
            if let Some(instruction) = &mut self.current().field_instruction {
                instruction.push_str(text);
            }
            return;
        }
        let sliced;
        let text = if let Some(window) = self.window.as_mut() {
            match window.slice(text) {
                Some(part) => {
                    sliced = part;
                    sliced.as_str()
                }
                None => return,
            }
        } else {
            text
        };
        let state = self.current().state.clone();
        let mut node = Node::Text(text.to_string());
        if state.superscript {
            node = Node::Superscript(vec![node]);
        }
        if state.subscript {
            node = Node::Subscript(vec![node]);
        }
        if state.underline {
            node = Node::Underline(vec![node]);
        }
        if state.strike {
            node = Node::Strikethrough(vec![node]);
        }
        if state.caps {
            node = Node::Caps(vec![node]);
        }
        if state.italic {
            node = Node::Italic(vec![node]);
        }
        if state.bold {
            node = Node::Bold(vec![node]);
        }
        // O texto já foi recortado pela janela; só quebras passam por `window_accepts`.
        self.current().nodes.push(node);
    }

    fn append_node(&mut self, node: Node) {
        match self.current().destination {
            Destination::Normal => {
                if self.window_accepts(&node) {
                    self.current().nodes.push(node);
                }
            }
            Destination::FieldInstruction => {
                if let Some(instruction) = &mut self.current().field_instruction {
                    instruction.push_str(&node.to_plain_text());
                }
            }
            Destination::Skip => {}
        }
    }

    /// Decide se uma quebra pertence à página pedida e acompanha o fim da janela.
    fn window_accepts(&mut self, node: &Node) -> bool {
        let Some(window) = self.window.as_mut() else {
            return true;
        };
        let inside = window.reached_start() && window.index < window.limit;
        if matches!(node, Node::Paragraph) {
            window.index += 1;
            window.in_paragraph = 0;
        }
        inside
    }

    fn count_unknown_control(&mut self, name: String) {
        *self.stats.unknown_controls.entry(name).or_default() += 1;
    }

    fn close_group(&mut self) {
        if self.frames.len() <= 1 {
            return;
        }
        let frame = self.frames.pop().unwrap_or_else(Frame::root);
        if frame.destination == Destination::Skip {
            return;
        }
        let parent = self.current();
        if frame.destination == Destination::FieldInstruction {
            parent.field_instruction = frame.field_instruction;
            return;
        }
        if frame.field {
            let instruction = frame
                .field_instruction
                .or_else(|| parent.field_instruction.take())
                .unwrap_or_default();
            let target = parse_link_target(&instruction);
            if frame.nodes.is_empty() {
                return;
            }
            parent.nodes.push(Node::Link {
                target,
                children: frame.nodes,
            });
        } else {
            if let Some(instruction) = frame.field_instruction {
                parent.field_instruction = Some(instruction);
            }
            parent.nodes.extend(frame.nodes);
        }
    }
}

fn hex_pair(high: u8, low: u8) -> Option<u8> {
    hex_value(high)?
        .checked_mul(16)?
        .checked_add(hex_value(low)?)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn encoding_for_codepage(codepage: u16) -> Option<&'static Encoding> {
    if codepage == 65001 {
        return Some(UTF_8);
    }
    let label = format!("windows-{codepage}");
    Encoding::for_label(label.as_bytes())
}

pub fn decode_twm_content(content_type: &str, input: &[u8]) -> Result<Vec<Node>, ModuleError> {
    match content_type {
        "rtf" => convert_rtf(input),
        other => Err(ModuleError::UnsupportedContent(other.to_string())),
    }
}

/// Como `decode_twm_content`, mas converte apenas uma página de parágrafos.
pub fn decode_twm_content_page(
    content_type: &str,
    input: &[u8],
    offset: u64,
    max_paragraphs: usize,
    max_chars: usize,
) -> Result<RtfPage, ModuleError> {
    match content_type {
        "rtf" => convert_rtf_page(input, offset, max_paragraphs, max_chars),
        other => Err(ModuleError::UnsupportedContent(other.to_string())),
    }
}

fn parse_link_target(instruction: &str) -> LinkTarget {
    let target = instruction
        .split_once("HYPERLINK")
        .map(|(_, value)| value.trim())
        .unwrap_or(instruction)
        .trim_matches('"')
        .trim()
        .to_string();
    for marker in ["_STRONG_", "_STRONG2_", "_STRONG3_"] {
        if let Some((_, suffix)) = target.split_once(marker) {
            return LinkTarget::Dictionary(suffix.split('|').next().unwrap_or(suffix).to_string());
        }
    }
    if let Some((_, suffix)) = target.split_once("id=") {
        let reference = suffix.split('|').next().unwrap_or(suffix);
        let parsed = (|| {
            let parse_ref = |text: &str| {
                let mut parts = text.split('.');
                let book = rhema_core::Book::from_number(parts.next()?.parse::<u8>().ok()?)?;
                let chapter = parts.next()?.parse::<u16>().ok()?;
                let verse = parts.next()?.parse::<u16>().ok()?;
                Some(VerseRef::new(book, chapter, verse))
            };
            let (start_text, end_text) = reference
                .split_once('-')
                .map(|(start, end)| (start, Some(end)))
                .unwrap_or((reference, None));
            let start = parse_ref(start_text)?;
            let end = match end_text {
                Some(text) => parse_ref(text)?,
                None => start,
            };
            VerseRange::new(start, end).ok()
        })();
        if let Some(range) = parsed {
            return LinkTarget::Bible(range);
        }
    }
    LinkTarget::Other(target)
}

#[cfg(test)]
fn merge_counts(target: &mut BTreeMap<String, usize>, source: BTreeMap<String, usize>) {
    for (name, count) in source {
        *target.entry(name).or_default() += count;
    }
}

#[cfg(test)]
fn count_links(nodes: &[Node], counts: &mut BTreeMap<String, usize>) {
    for node in nodes {
        let children = match node {
            Node::Link { target, children } => {
                let kind = match target {
                    LinkTarget::Bible(_) => "bíblia",
                    LinkTarget::Dictionary(_) => "dicionário",
                    LinkTarget::Other(target) if target.starts_with("http") => "externo",
                    LinkTarget::Other(target) if target.starts_with("tw://") => {
                        "tw não classificado"
                    }
                    LinkTarget::Other(target) if target.starts_with('_') => "marcação interna",
                    LinkTarget::Other(_) => "outro",
                };
                *counts.entry(kind.to_string()).or_default() += 1;
                children
            }
            Node::Italic(children)
            | Node::Bold(children)
            | Node::Underline(children)
            | Node::Strikethrough(children)
            | Node::Caps(children)
            | Node::Superscript(children)
            | Node::Subscript(children)
            | Node::WordsOfJesus(children)
            | Node::OtQuote(children)
            | Node::Heading(_, children)
            | Node::Strong { children, .. }
            | Node::Note(children)
            | Node::UnknownTag { children, .. } => children,
            _ => continue,
        };
        count_links(children, counts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markup::plain_text;
    use proptest::prelude::*;

    #[test]
    fn converts_basic_formatting_and_breaks() {
        let nodes = convert_rtf(br"{\rtf1 \b bold\b0  \i italics\i0  \ul underlined\ulnone  \super 2\nosupersub\par next\line line}")
            .expect("entrada RTF sintética válida");
        assert_eq!(plain_text(&nodes), "bold italics underlined 2nextline");
        assert!(nodes.iter().any(|node| matches!(node, Node::Bold(_))));
        assert!(nodes.iter().any(|node| matches!(node, Node::Italic(_))));
        assert!(nodes.iter().any(|node| matches!(node, Node::Underline(_))));
        assert!(nodes
            .iter()
            .any(|node| matches!(node, Node::Superscript(_))));
        assert!(nodes.iter().any(|node| matches!(node, Node::Paragraph)));
        assert!(nodes.iter().any(|node| matches!(node, Node::LineBreak)));
    }

    #[test]
    fn decodes_codepage_hex_and_unicode_fallback() {
        let nodes = convert_rtf(br"{\rtf1\ansi\ansicpg1252 caf\'e9 \uc1\u233? \u-10179?\u-8704?}")
            .expect("entrada RTF sintética válida");
        assert_eq!(plain_text(&nodes), "café é 😀");
        let ansi_zero = convert_rtf(br"{\rtf1\ansicpg0 \'e7 \'e3 \'e9 \'f5}")
            .expect("entrada RTF sintética válida");
        assert_eq!(plain_text(&ansi_zero), "ç ã é õ");
    }

    #[test]
    fn skips_known_and_unknown_destinations_and_counts_controls() {
        let output = convert_rtf_with_stats(
            br"{\rtf1 visible{\fonttbl hidden}{\colortbl hidden}{\*\private hidden}{\strangecontrol x}}",
        )
        .expect("entrada RTF sintética válida");
        assert_eq!(plain_text(&output.nodes), "visiblex");
        assert_eq!(output.stats.unknown_destinations.get("private"), Some(&1));
        assert_eq!(
            output.stats.unknown_controls.get("strangecontrol"),
            Some(&1)
        );
        assert_eq!(output.stats.control_words.get("rtf"), Some(&1));
    }

    #[test]
    fn extracts_bible_hyperlink_field() {
        let nodes = convert_rtf(
            br#"{\rtf1{\field{\*\fldinst HYPERLINK "tw://bible.*?id=43.3.16|_AUTODETECT_|"}{\fldrslt John 3:16}}}"#,
        )
        .expect("entrada RTF sintética válida");
        assert!(matches!(
            &nodes[0],
            Node::Link {
                target: LinkTarget::Bible(_),
                children,
            } if plain_text(children) == "John 3:16"
        ));
    }

    #[test]
    fn handles_rtf_binary_runs_and_dictionary_links() {
        let nodes = convert_rtf(
            br#"{\rtf1 before\bin3{x}after {\field{\*\fldinst HYPERLINK "_STRONG_G25|_NOLINK_"}{\fldrslt loved}}}"#,
        )
        .expect("entrada RTF sintética válida");
        assert_eq!(plain_text(&nodes), "beforeafter loved");
        assert!(nodes.iter().any(|node| matches!(
            node,
            Node::Link {
                target: LinkTarget::Dictionary(key),
                ..
            } if key == "G25"
        )));
    }

    #[test]
    fn preserves_caps_strikethrough_and_bullet_formatting() {
        let nodes = convert_rtf(br"{\rtf1\caps caps\caps0|\strike struck\strike0|\bullet}")
            .expect("entrada RTF sintética válida");
        assert_eq!(plain_text(&nodes), "caps|struck|•");
        assert!(nodes.iter().any(|node| matches!(node, Node::Caps(_))));
        assert!(nodes
            .iter()
            .any(|node| matches!(node, Node::Strikethrough(_))));
    }

    #[test]
    fn malformed_groups_controls_and_binary_are_safe() {
        assert_eq!(
            plain_text(&convert_rtf(b"{\\rtf1 open").expect("RTF incompleto tolerado")),
            "open"
        );
        assert!(matches!(
            convert_rtf(b"stray } brace"),
            Err(ModuleError::Corrupted(_))
        ));
        assert!(matches!(convert_rtf(b""), Err(ModuleError::Corrupted(_))));
        let bad = convert_rtf_with_stats(b"{\\rtf1\\u-10179? caf\\'zz}")
            .expect("RTF malformado tolerado");
        assert_eq!(plain_text(&bad.nodes), "\u{fffd} caf");
        assert_eq!(bad.stats.invalid_hex_escapes, 1);
        assert!(matches!(
            convert_rtf(&[0, 255, 0, 42]),
            Err(ModuleError::Corrupted(_))
        ));
    }

    #[test]
    fn malformed_unicode_parameter_is_ignored() {
        let output =
            convert_rtf_with_stats(br"{\rtf1\u?}").expect("RTF com controle inválido tolerado");
        assert_eq!(plain_text(&output.nodes), "?");
        assert_eq!(output.stats.unknown_controls.get("u"), Some(&1));
    }

    #[test]
    fn synthetic_rtf_snapshots() {
        insta::assert_debug_snapshot!(
            "synthetic_rtf_nested_formatting",
            convert_rtf(br"{\rtf1 before {\b bold {\i nested}} after\par end}")
                .expect("RTF sintético válido")
        );
        insta::assert_debug_snapshot!(
            "synthetic_rtf_unicode_and_link",
            convert_rtf(
                br#"{\rtf1\uc1\u233? {\field{\*\fldinst HYPERLINK "tw://bible.*?id=43.3.16|_AUTODETECT_|"}{\fldrslt John 3:16}}}"#
            )
            .expect("RTF sintético válido")
        );
    }

    #[test]
    fn pages_cover_the_entry_without_overlap_or_loss() {
        let rtf = br"{\rtf1\ansi {\fonttbl{\f0 Arial;}}\b First\b0  one\par Second {\i two}\par Third \ul three\ulnone\par Fourth\par Fifth tail}";
        let full = convert_rtf(rtf).expect("RTF sintético");
        let mut stitched = Vec::new();
        for size in 1..=6 {
            stitched.clear();
            let mut offset = 0;
            loop {
                let page = convert_rtf_page(rtf, offset, size, 8_000).expect("página");
                assert_eq!(page.next_offset, offset + ((size as u64) << 32));
                stitched.extend(page.nodes);
                if !page.has_more {
                    break;
                }
                offset = page.next_offset;
            }
            assert_eq!(stitched, full, "páginas de {size} parágrafos");
        }
    }

    #[test]
    fn preserves_literal_spaces_around_control_words_hex_unicode_and_groups() {
        let control_delimiter = convert_rtf(br"{\rtf1 A,\b B}").expect("delimitador de controle");
        assert_eq!(plain_text(&control_delimiter), "A,B");

        let text_before_control =
            convert_rtf(br"{\rtf1 A, \b B}").expect("espaço textual antes do controle");
        assert_eq!(plain_text(&text_before_control), "A, B");

        let repeated_delimiter =
            convert_rtf(br"{\rtf1 A,\b  B}").expect("espaço adicional após controle");
        assert_eq!(plain_text(&repeated_delimiter), "A, B");

        let hex_space = convert_rtf(br"{\rtf1 A,\'20 B}").expect("espaço hexadecimal");
        assert_eq!(plain_text(&hex_space), "A,  B");

        let unicode_space = convert_rtf(br"{\rtf1 A,\u233? B}").expect("espaço Unicode");
        assert_eq!(plain_text(&unicode_space), "A,é B");

        let group_space = convert_rtf(br"{\rtf1 A,{ }B}").expect("espaço de grupo");
        assert_eq!(plain_text(&group_space), "A, B");
    }

    #[test]
    fn character_limit_splits_a_giant_paragraph_without_loss() {
        let rtf =
            br"{\rtf1 Alpha beta gamma delta {\b epsilon zeta} eta theta iota kappa\par Next one}";
        let full = plain_text(&convert_rtf(rtf).expect("RTF sintético"));
        for limit in [1, 2, 3, 5, 7, 11, 20] {
            let mut text = String::new();
            let (mut offset, mut pages) = (0, 0);
            loop {
                let page = convert_rtf_page(rtf, offset, 60, limit).expect("página");
                assert!(
                    plain_text(&page.nodes).chars().count() <= limit + 1,
                    "limite {limit}"
                );
                text.push_str(&plain_text(&page.nodes));
                pages += 1;
                assert!(pages < 200, "sem progresso");
                if !page.has_more {
                    break;
                }
                assert!(page.next_offset > offset);
                offset = page.next_offset;
            }
            assert_eq!(text, full, "limite de {limit} caracteres");
        }
        let first = convert_rtf_page(rtf, 0, 60, 10).expect("primeira");
        assert!(first.has_more && first.next_offset == 10);
    }

    #[test]
    fn page_reports_more_only_when_visible_content_follows() {
        let rtf = br"{\rtf1 One\par Two\par \par }";
        let first = convert_rtf_page(rtf, 0, 1, 8_000).expect("primeira página");
        assert!(first.has_more);
        assert_eq!(plain_text(&first.nodes), "One");
        let rest = convert_rtf_page(rtf, page_cursor(1, 0), 10, 8_000).expect("restante");
        assert!(!rest.has_more);
        assert_eq!(plain_text(&rest.nodes), "Two");
        let beyond = convert_rtf_page(rtf, page_cursor(50, 0), 10, 8_000).expect("depois do fim");
        assert!(beyond.nodes.is_empty() && !beyond.has_more);
        let unterminated = convert_rtf_page(br"{\rtf1 Only text", 0, 60, 8_000).expect("sem par");
        assert_eq!(plain_text(&unterminated.nodes), "Only text");
        assert!(!unterminated.has_more);
    }

    #[test]
    fn page_keeps_formatting_state_from_skipped_paragraphs_and_links_whole() {
        let rtf = br#"{\rtf1 {\b Bold start\par still bold\par }plain {\field{\*\fldinst HYPERLINK "tw://bible.*?id=43.3.16|_AUTODETECT_|"}{\fldrslt link text}}\par next}"#;
        let page = convert_rtf_page(rtf, page_cursor(2, 0), 1, 8_000).expect("página com link");
        assert_eq!(plain_text(&page.nodes), "plain link text");
        assert!(page.nodes.iter().any(|node| matches!(
            node,
            Node::Link {
                target: LinkTarget::Bible(_),
                ..
            }
        )));
        assert!(page.has_more);
        assert!(convert_rtf_page(b"plain", 0, 10, 8_000).is_err());
    }

    #[test]
    fn page_stops_reading_after_the_window() {
        let mut rtf = String::from(r"{\rtf1 ");
        for index in 0..5_000 {
            rtf.push_str(&format!("Paragraph {index}\\par "));
        }
        rtf.push('}');
        let page = convert_rtf_page(rtf.as_bytes(), 0, 60, 8_000).expect("primeira página");
        assert!(page.has_more);
        let paragraphs = page
            .nodes
            .iter()
            .filter(|node| matches!(node, Node::Paragraph))
            .count();
        assert_eq!(paragraphs, 60);
        assert!(!plain_text(&page.nodes).contains("Paragraph 60"));
    }

    #[test]
    fn dispatches_page_conversion_by_declared_type() {
        assert!(decode_twm_content_page("rtf", br"{\rtf1 a\par b}", 0, 1, 100).is_ok());
        assert!(matches!(
            decode_twm_content_page("rvf", b"x", 0, 1, 100),
            Err(ModuleError::UnsupportedContent(kind)) if kind == "rvf"
        ));
    }

    #[test]
    fn dispatches_twm_content_by_declared_type() {
        let nodes = decode_twm_content("rtf", br"{\rtf1 decoded}").expect("RTF suportado");
        assert_eq!(plain_text(&nodes), "decoded");
        assert!(matches!(
            decode_twm_content("rvf", b"not decoded"),
            Err(ModuleError::UnsupportedContent(kind)) if kind == "rvf"
        ));
        assert!(matches!(
            decode_twm_content("custom", b"not decoded"),
            Err(ModuleError::UnsupportedContent(kind)) if kind == "custom"
        ));
        assert!(matches!(
            decode_twm_content("rtf", b"not RTF"),
            Err(ModuleError::Corrupted(_))
        ));
    }

    #[test]
    fn real_module_samples_are_optional_and_read_only() {
        let Ok(root) = std::env::var("RHEMA_TEST_MODULES") else {
            eprintln!("Ignorado: RHEMA_TEST_MODULES não foi definido");
            return;
        };

        use rusqlite::{types::ValueRef, Connection, OpenFlags};
        use std::path::Path;
        use std::time::Instant;

        let modules = [
            "Mickelson's Strong.dct.twm",
            "Holman-AT.cmt.twm",
            "NICNT.cmt.twm",
            "VidaNova.cmt.twm",
            "Strong_AMG2.dct.twm",
        ];
        for filename in modules {
            let path = Path::new(&root).join(filename);
            assert!(
                path.is_file(),
                "Módulo de teste ausente: {}",
                path.display()
            );
            match crate::ensure_twm_unprotected(&path) {
                Ok(()) => {}
                Err(rhema_core::ModuleError::EncryptedModule) => {
                    eprintln!("{filename}: protegido; conteúdo não consultado");
                    continue;
                }
                Err(error) => panic!("Falha na verificação de proteção de {filename}: {error}"),
            }
            let connection = Connection::open_with_flags(
                &path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .expect("abrir banco somente para leitura");
            let count: usize = connection
                .query_row("SELECT count(*) FROM content", [], |row| row.get(0))
                .expect("contar conteúdo");
            let sample_count = count.min(2_000);
            assert!(sample_count > 0, "{filename} não contém entradas");
            let mut query = connection
                .prepare("SELECT data FROM content ORDER BY topic_id")
                .expect("preparar amostragem distribuída");
            let mut rows = query.query([]).expect("abrir cursor de amostragem");
            let sample_offsets = if sample_count == 1 {
                vec![0]
            } else {
                (0..sample_count)
                    .map(|sample_index| sample_index * (count - 1) / (sample_count - 1))
                    .collect::<Vec<_>>()
            };
            let mut next_sample = 0usize;
            let mut control_words = BTreeMap::<String, usize>::new();
            let mut unknown_controls = BTreeMap::<String, usize>::new();
            let mut destinations = BTreeMap::<String, usize>::new();
            let mut unknown_destinations = BTreeMap::<String, usize>::new();
            let mut binary_entries = 0usize;
            let mut contains_backslash = 0usize;
            let mut contains_brace = 0usize;
            let mut escaped_backslashes = 0usize;
            let mut escaped_braces = 0usize;
            let mut greek_characters = 0usize;
            let mut hebrew_characters = 0usize;
            let mut link_targets = BTreeMap::<String, usize>::new();
            let mut storage_classes = BTreeMap::<String, usize>::new();
            let mut rtf_entries = 0usize;
            let mut conversion_duration = std::time::Duration::ZERO;
            let mut accent_characters = 0usize;
            let mut replacement_characters = 0usize;
            let mut residual_controls = Vec::<Vec<String>>::new();

            let mut row_index = 0usize;
            while let Some(row) = rows.next().expect("avançar cursor de amostragem") {
                if sample_offsets.get(next_sample) != Some(&row_index) {
                    row_index += 1;
                    continue;
                }
                let raw = match row.get_ref(0).expect("ler conteúdo") {
                    ValueRef::Text(bytes) => {
                        *storage_classes.entry("TEXT".to_string()).or_default() += 1;
                        bytes.to_vec()
                    }
                    ValueRef::Blob(bytes) => {
                        *storage_classes.entry("BLOB".to_string()).or_default() += 1;
                        bytes.to_vec()
                    }
                    ValueRef::Null => {
                        *storage_classes.entry("NULL".to_string()).or_default() += 1;
                        Vec::new()
                    }
                    ValueRef::Integer(value) => {
                        *storage_classes.entry("INTEGER".to_string()).or_default() += 1;
                        value.to_string().into_bytes()
                    }
                    ValueRef::Real(value) => {
                        *storage_classes.entry("REAL".to_string()).or_default() += 1;
                        value.to_string().into_bytes()
                    }
                };
                if raw.starts_with(b"{\\rtf") {
                    rtf_entries += 1;
                };
                let conversion_started = Instant::now();
                let converted = match convert_rtf_with_stats(&raw) {
                    Ok(converted) => converted,
                    Err(_) => {
                        conversion_duration += conversion_started.elapsed();
                        binary_entries += 1;
                        next_sample += 1;
                        row_index += 1;
                        continue;
                    }
                };
                conversion_duration += conversion_started.elapsed();
                merge_counts(&mut control_words, converted.stats.control_words);
                merge_counts(&mut unknown_controls, converted.stats.unknown_controls);
                merge_counts(&mut destinations, converted.stats.destinations);
                merge_counts(
                    &mut unknown_destinations,
                    converted.stats.unknown_destinations,
                );
                let text = plain_text(&converted.nodes);
                accent_characters += text
                    .chars()
                    .filter(|character| matches!(*character as u32, 0x00c0..=0x00ff))
                    .count();
                replacement_characters += text
                    .chars()
                    .filter(|character| *character == '\u{fffd}')
                    .count();
                if filename == "NICNT.cmt.twm" && text.contains(['\\', '{', '}']) {
                    residual_controls.push(
                        [b"\\\\".as_slice(), b"\\{", b"\\}"]
                            .into_iter()
                            .filter(|escape| {
                                raw.windows(escape.len()).any(|window| window == *escape)
                            })
                            .map(|escape| String::from_utf8_lossy(escape).into_owned())
                            .collect(),
                    );
                }
                contains_backslash += usize::from(text.contains('\\'));
                contains_brace += usize::from(text.contains('{') || text.contains('}'));
                escaped_backslashes += raw.windows(2).filter(|pair| *pair == b"\\\\").count();
                escaped_braces += raw
                    .windows(2)
                    .filter(|pair| *pair == b"\\{" || *pair == b"\\}")
                    .count();
                greek_characters += text
                    .chars()
                    .filter(
                        |character| matches!(*character as u32, 0x0370..=0x03ff | 0x1f00..=0x1fff),
                    )
                    .count();
                hebrew_characters += text
                    .chars()
                    .filter(|character| matches!(*character as u32, 0x0590..=0x05ff))
                    .count();
                count_links(&converted.nodes, &mut link_targets);
                next_sample += 1;
                row_index += 1;
            }
            assert_eq!(
                next_sample, sample_count,
                "amostragem incompleta de {filename}"
            );
            eprintln!(
                "{filename}: {sample_count} entradas; storage={storage_classes:?}; RTF={rtf_entries}; não RTF={}; binárias={binary_entries}; desconhecidos={unknown_controls:?}; destinos={destinations:?}; destinos desconhecidos={unknown_destinations:?}; texto com barra invertida={contains_backslash} (escapes={escaped_backslashes}); texto com chaves={contains_brace} (escapes={escaped_braces}); grego={greek_characters}; hebraico={hebrew_characters}; links={link_targets:?}; acentos={accent_characters}; U+FFFD={replacement_characters}; média conversão={:.3} ms/entrada; controles={control_words:?}",
                sample_count - rtf_entries,
                conversion_duration.as_secs_f64() * 1_000.0 / sample_count as f64
            );
            if filename == "NICNT.cmt.twm" {
                eprintln!(
                    "NICNT fragmentos estruturais residuais (sem texto): {residual_controls:?}"
                );
            }
            if filename == "VidaNova.cmt.twm" {
                eprintln!("VidaNova acentuação observada: {}; exemplo estrutural: Text(U+00E7/U+00E3); links bíblicos={}", accent_characters > 0 && replacement_characters == 0, link_targets.get("bíblia").copied().unwrap_or_default());
            }
            if filename == "Strong_AMG2.dct.twm" {
                print_strong_g25_shape(&connection);
            }
        }
    }

    fn print_strong_g25_shape(connection: &rusqlite::Connection) {
        use rusqlite::types::ValueRef;
        let columns = connection
            .prepare("PRAGMA table_info(topics)")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .expect("ler nomes das colunas sintéticas/reais de topics");
        let Some(key_column) = ["subject", "key", "word"].into_iter().find(|candidate| {
            columns
                .iter()
                .any(|column| column.eq_ignore_ascii_case(candidate))
        }) else {
            eprintln!("Strong_AMG2: nenhuma coluna de chave conhecida em topics");
            return;
        };
        let query = format!(
            "SELECT c.data FROM topics t JOIN content c ON c.topic_id=t.id WHERE t.\"{key_column}\"=?1 LIMIT 1"
        );
        let result = connection.query_row(&query, ["G25"], |row| {
            let raw = match row.get_ref(0)? {
                ValueRef::Text(bytes) | ValueRef::Blob(bytes) => bytes.to_vec(),
                ValueRef::Null => Vec::new(),
                ValueRef::Integer(value) => value.to_string().into_bytes(),
                ValueRef::Real(value) => value.to_string().into_bytes(),
            };
            Ok(raw)
        });
        match result {
            Ok(raw) => {
                let nodes = match convert_rtf(&raw) {
                    Ok(nodes) => nodes,
                    Err(error) => {
                        eprintln!("Strong_AMG2 G25: conteúdo RTF inválido: {error}");
                        return;
                    }
                };
                let mut shape = BTreeMap::new();
                collect_node_shapes(&nodes, &mut shape);
                let mut links = BTreeMap::new();
                collect_link_target_counts(&nodes, &mut links);
                eprintln!(
                    "Strong_AMG2 G25: existe; coluna-chave={key_column}; conteúdo convertido={}; árvore={shape:?}; tipos de destino de link={links:?}; formato={{\\field{{\\*\\fldinst HYPERLINK \"[valor]\"}}{{\\fldrslt [rótulo]}}}}",
                    !nodes.is_empty()
                );
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                eprintln!("Strong_AMG2 G25: inexistente; coluna-chave={key_column}");
            }
            Err(error) => panic!("Falha ao verificar G25 em Strong_AMG2: {error}"),
        }
    }

    fn collect_node_shapes(nodes: &[Node], counts: &mut BTreeMap<&'static str, usize>) {
        for node in nodes {
            let (name, children) = match node {
                Node::Text(_) => ("Text", None),
                Node::Italic(nodes) => ("Italic", Some(nodes)),
                Node::Bold(nodes) => ("Bold", Some(nodes)),
                Node::Underline(nodes) => ("Underline", Some(nodes)),
                Node::Strikethrough(nodes) => ("Strikethrough", Some(nodes)),
                Node::Caps(nodes) => ("Caps", Some(nodes)),
                Node::Superscript(nodes) => ("Superscript", Some(nodes)),
                Node::Subscript(nodes) => ("Subscript", Some(nodes)),
                Node::WordsOfJesus(nodes) => ("WordsOfJesus", Some(nodes)),
                Node::OtQuote(nodes) => ("OtQuote", Some(nodes)),
                Node::Heading(_, nodes) => ("Heading", Some(nodes)),
                Node::Strong { children, .. } => ("Strong", Some(children)),
                Node::Grammar { .. } => ("Grammar", None),
                Node::Morph(_) => ("Morph", None),
                Node::Note(nodes) => ("Note", Some(nodes)),
                Node::XRef(_) => ("XRef", None),
                Node::Link { children, .. } => ("Link", Some(children)),
                Node::Paragraph => ("Paragraph", None),
                Node::LineBreak => ("LineBreak", None),
                Node::UnknownTag { children, .. } => ("UnknownTag", Some(children)),
            };
            *counts.entry(name).or_default() += 1;
            if let Some(children) = children {
                collect_node_shapes(children, counts);
            }
        }
    }

    fn collect_link_target_counts(nodes: &[Node], counts: &mut BTreeMap<String, usize>) {
        for node in nodes {
            let children = match node {
                Node::Link { target, children } => {
                    let kind = match target {
                        LinkTarget::Bible(_) => "versículo".to_string(),
                        LinkTarget::Dictionary(key)
                            if key
                                .strip_prefix('G')
                                .or_else(|| key.strip_prefix('H'))
                                .is_some_and(|suffix| {
                                    !suffix.is_empty()
                                        && suffix
                                            .chars()
                                            .all(|character| character.is_ascii_digit())
                                }) =>
                        {
                            "verbete G/H".to_string()
                        }
                        LinkTarget::Dictionary(_) | LinkTarget::Other(_) => "outro".to_string(),
                    };
                    *counts.entry(kind).or_default() += 1;
                    children
                }
                Node::Italic(children)
                | Node::Bold(children)
                | Node::Underline(children)
                | Node::Strikethrough(children)
                | Node::Caps(children)
                | Node::Superscript(children)
                | Node::Subscript(children)
                | Node::WordsOfJesus(children)
                | Node::OtQuote(children)
                | Node::Heading(_, children)
                | Node::Strong { children, .. }
                | Node::Note(children)
                | Node::UnknownTag { children, .. } => children,
                _ => continue,
            };
            collect_link_target_counts(children, counts);
        }
    }

    proptest! {
        #[test]
        fn arbitrary_bytes_never_panic_or_hang(input in prop::collection::vec(any::<u8>(), 0..4096)) {
            let mut rtf = b"{\\rtf1".to_vec();
            rtf.extend(input);
            rtf.push(b'}');
            let output = convert_rtf_with_stats(&rtf);
            prop_assert!(output.is_ok());
            prop_assert!(output.unwrap().nodes.len() <= rtf.len().saturating_mul(2).saturating_add(1));
        }

    }
}
