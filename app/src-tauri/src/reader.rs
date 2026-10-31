//! Leitura de capítulos bíblicos, resolução de referências e nomes de livros para a interface.
//!
//! Toda a conversão de `Node` para a representação da interface (`UiNode`) acontece aqui.
//! A interface nunca recebe marcação bruta do módulo: apenas árvores estruturadas.

use std::collections::HashMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use rhema_core::{Book, ModuleError, Node, StrongLang, Testament, VerseRange, VerseRef};
use rhema_library::{ModuleInfo, ModuleState};
use rhema_refs::{FormatStyle, Language, RefError, format, parse, suggest_books};
use rhema_twformat::OntReader;
use serde::Serialize;

/// Quantidade máxima de Bíblias abertas ao mesmo tempo.
pub const READER_CACHE_CAPACITY: usize = 6;

/// Erro serializável com código estável; a interface traduz o código para o usuário.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<u16>,
}

impl ApiError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            book: None,
            max: None,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("internal", message)
    }
}

impl From<ModuleError> for ApiError {
    fn from(error: ModuleError) -> Self {
        let message = error.to_string();
        match error {
            ModuleError::EncryptedModule => Self::new("module_protected", message),
            ModuleError::NotFound(_) => Self::new("module_not_found", message),
            ModuleError::Corrupted(_) => Self::new("module_corrupted", message),
            ModuleError::VerseNotFound(_) => Self::new("verse_not_found", message),
            ModuleError::InvalidReference(_) => Self::new("invalid_reference", message),
            ModuleError::BookNotInModule(book, _) => Self {
                book: Some(book.number()),
                ..Self::new("book_not_in_module", message)
            },
            ModuleError::UnsupportedFormat(_) | ModuleError::UnsupportedContent(_) => {
                Self::new("module_unsupported", message)
            }
            ModuleError::Io(_) => Self::new("io_error", message),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UiNode {
    Text {
        value: String,
    },
    Italic {
        children: Vec<UiNode>,
    },
    Bold {
        children: Vec<UiNode>,
    },
    Underline {
        children: Vec<UiNode>,
    },
    Strikethrough {
        children: Vec<UiNode>,
    },
    Caps {
        children: Vec<UiNode>,
    },
    Superscript {
        children: Vec<UiNode>,
    },
    Subscript {
        children: Vec<UiNode>,
    },
    WordsOfJesus {
        children: Vec<UiNode>,
    },
    OtQuote {
        children: Vec<UiNode>,
    },
    Heading {
        level: u8,
        children: Vec<UiNode>,
    },
    Strong {
        lang: &'static str,
        num: u32,
        children: Vec<UiNode>,
    },
    Note {
        children: Vec<UiNode>,
    },
    Link {
        children: Vec<UiNode>,
    },
    Paragraph,
    LineBreak,
}

/// Converte nós do módulo na representação da interface.
///
/// - `Grammar`, `Morph` e `XRef` são metadados e não geram nó;
/// - `UnknownTag` vira apenas o seu conteúdo interno;
/// - sinais `<` e `>` são texto literal quando o tokenizador não os reconhece como tag, e a
///   interface nunca altera o texto do módulo.
pub fn convert_nodes(nodes: &[Node]) -> Vec<UiNode> {
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        convert_node(node, &mut out);
    }
    out
}

fn convert_node(node: &Node, out: &mut Vec<UiNode>) {
    let wrap = |children: &[Node]| convert_nodes(children);
    match node {
        Node::Text(text) => {
            if !text.is_empty() {
                out.push(UiNode::Text {
                    value: text.clone(),
                });
            }
        }
        Node::Italic(children) => out.push(UiNode::Italic {
            children: wrap(children),
        }),
        Node::Bold(children) => out.push(UiNode::Bold {
            children: wrap(children),
        }),
        Node::Underline(children) => out.push(UiNode::Underline {
            children: wrap(children),
        }),
        Node::Strikethrough(children) => {
            out.push(UiNode::Strikethrough {
                children: wrap(children),
            });
        }
        Node::Caps(children) => out.push(UiNode::Caps {
            children: wrap(children),
        }),
        Node::Superscript(children) => out.push(UiNode::Superscript {
            children: wrap(children),
        }),
        Node::Subscript(children) => out.push(UiNode::Subscript {
            children: wrap(children),
        }),
        Node::WordsOfJesus(children) => out.push(UiNode::WordsOfJesus {
            children: wrap(children),
        }),
        Node::OtQuote(children) => out.push(UiNode::OtQuote {
            children: wrap(children),
        }),
        Node::Heading(level, children) => out.push(UiNode::Heading {
            level: (*level).clamp(1, 6),
            children: wrap(children),
        }),
        Node::Strong {
            lang,
            num,
            children,
        } => {
            let children = wrap(children);
            if !children.is_empty() {
                out.push(UiNode::Strong {
                    lang: match lang {
                        StrongLang::Greek => "G",
                        StrongLang::Hebrew => "H",
                    },
                    num: *num,
                    children,
                });
            }
        }
        Node::Note(children) => out.push(UiNode::Note {
            children: wrap(children),
        }),
        Node::Link { children, .. } => out.push(UiNode::Link {
            children: wrap(children),
        }),
        Node::Paragraph => out.push(UiNode::Paragraph),
        Node::LineBreak => out.push(UiNode::LineBreak),
        Node::UnknownTag { children, .. } => {
            for child in children {
                convert_node(child, out);
            }
        }
        Node::Grammar { .. } | Node::Morph(_) | Node::XRef(_) => {}
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VerseDto {
    pub number: u16,
    pub nodes: Vec<UiNode>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChapterDto {
    pub module_id: String,
    pub book: u8,
    pub chapter: u16,
    /// Verdadeiro quando a Bíblia não tem todos os versículos do capítulo.
    pub incomplete: bool,
    pub verses: Vec<VerseDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AlignedVerseDto {
    pub number: u16,
    pub columns: Vec<Option<Vec<UiNode>>>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChapterColumnDto {
    pub module_id: String,
    pub error: Option<ApiError>,
    pub incomplete: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AlignedChaptersDto {
    pub columns: Vec<ChapterColumnDto>,
    pub rows: Vec<AlignedVerseDto>,
}

/// Cache LRU de leitores abertos. O primeiro item é o menos usado recentemente.
pub struct ReaderCache {
    capacity: usize,
    entries: Mutex<Vec<(String, Arc<OntReader>)>>,
    opens: AtomicUsize,
}

impl ReaderCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Mutex::new(Vec::new()),
            opens: AtomicUsize::new(0),
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    #[cfg(test)]
    pub fn contains(&self, module_id: &str) -> bool {
        self.lock().iter().any(|(id, _)| id == module_id)
    }

    /// Quantas vezes um arquivo foi realmente aberto (para verificar o reuso).
    #[cfg(test)]
    pub fn open_count(&self) -> usize {
        self.opens.load(Ordering::Relaxed)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<(String, Arc<OntReader>)>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn get_or_open(&self, module: &ModuleInfo) -> Result<Arc<OntReader>, ApiError> {
        {
            let mut entries = self.lock();
            if let Some(position) = entries.iter().position(|(id, _)| *id == module.id) {
                let entry = entries.remove(position);
                let reader = Arc::clone(&entry.1);
                entries.push(entry);
                return Ok(reader);
            }
        }
        let reader = Arc::new(OntReader::open(&module.path)?);
        self.opens.fetch_add(1, Ordering::Relaxed);
        let mut entries = self.lock();
        if let Some(position) = entries.iter().position(|(id, _)| *id == module.id) {
            entries.remove(position);
        }
        entries.push((module.id.clone(), Arc::clone(&reader)));
        while entries.len() > self.capacity {
            entries.remove(0);
        }
        Ok(reader)
    }
}

/// Lê um capítulo de uma Bíblia pronta. Módulos protegidos ou não prontos nunca são abertos.
pub fn read_chapter(
    cache: &ReaderCache,
    module: &ModuleInfo,
    book: u8,
    chapter: u16,
) -> Result<ChapterDto, ApiError> {
    match &module.state {
        ModuleState::Ready => {}
        ModuleState::Protected => {
            return Err(ApiError::new(
                "module_protected",
                "Módulo protegido: não é lido pelo RhemaStudio.",
            ));
        }
        ModuleState::UnsupportedFormat { message } => {
            return Err(ApiError::new("module_unsupported", message.clone()));
        }
        ModuleState::Corrupted { message } => {
            return Err(ApiError::new("module_corrupted", message.clone()));
        }
    }
    if module.kind != rhema_core::ModuleKind::Bible {
        return Err(ApiError::new(
            "module_unsupported",
            "O módulo não é uma Bíblia.",
        ));
    }
    let book = Book::from_number(book)
        .ok_or_else(|| ApiError::new("invalid_reference", format!("Livro inválido: {book}")))?;
    let Some(verse_count) = book.verses_in_chapter(chapter) else {
        return Err(ApiError {
            book: Some(book.number()),
            max: Some(book.chapter_count()),
            ..ApiError::new("invalid_reference", "Capítulo fora dos limites do livro.")
        });
    };
    let reader = cache.get_or_open(module)?;
    let mut verses = Vec::with_capacity(usize::from(verse_count));
    let mut incomplete = false;
    for number in 1..=verse_count {
        match reader.verse_nodes(book, chapter, number) {
            Ok(nodes) => verses.push(VerseDto {
                number,
                nodes: convert_nodes(&nodes),
            }),
            Err(ModuleError::VerseNotFound(_)) => incomplete = true,
            Err(error) => return Err(error.into()),
        }
    }
    if verses.is_empty() {
        return Err(ApiError::new(
            "verse_not_found",
            "Nenhum versículo deste capítulo existe nesta Bíblia.",
        ));
    }
    Ok(ChapterDto {
        module_id: module.id.clone(),
        book: book.number(),
        chapter,
        incomplete,
        verses,
    })
}

/// Lê cada coluna independentemente e alinha os resultados pelo número canônico do versículo.
pub fn read_chapters(
    cache: &ReaderCache,
    modules: &[ModuleInfo],
    book_number: u8,
    chapter_number: u16,
) -> Result<AlignedChaptersDto, ApiError> {
    let book = Book::from_number(book_number)
        .ok_or_else(|| ApiError::new("invalid_reference", "Livro inválido."))?;
    let verse_count = book
        .verses_in_chapter(chapter_number)
        .ok_or_else(|| ApiError {
            book: Some(book.number()),
            max: Some(book.chapter_count()),
            ..ApiError::new("invalid_reference", "Capítulo fora dos limites do livro.")
        })?;
    if modules.len() > 4 {
        return Err(ApiError::new(
            "invalid_reference",
            "A leitura paralela aceita no máximo quatro Bíblias.",
        ));
    }

    let mut columns = Vec::with_capacity(modules.len());
    for module in modules {
        match read_chapter(cache, module, book_number, chapter_number) {
            Ok(chapter) => {
                let verses = chapter
                    .verses
                    .into_iter()
                    .map(|verse| (verse.number, verse.nodes))
                    .collect::<HashMap<_, _>>();
                columns.push((module.id.clone(), None, chapter.incomplete, verses));
            }
            Err(error) => columns.push((module.id.clone(), Some(error), false, HashMap::new())),
        }
    }

    let rows = (1..=verse_count)
        .map(|number| AlignedVerseDto {
            number,
            columns: columns
                .iter_mut()
                .map(|(_, error, _, verses)| {
                    if error.is_some() {
                        None
                    } else {
                        verses.remove(&number)
                    }
                })
                .collect(),
        })
        .collect();
    Ok(AlignedChaptersDto {
        columns: columns
            .into_iter()
            .map(|(module_id, error, incomplete, _)| ChapterColumnDto {
                module_id,
                error,
                incomplete,
            })
            .collect(),
        rows,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RefPoint {
    pub book: u8,
    pub chapter: u16,
    pub verse: u16,
}

impl From<VerseRef> for RefPoint {
    fn from(value: VerseRef) -> Self {
        Self {
            book: value.book.number(),
            chapter: value.chapter,
            verse: value.verse,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RangeDto {
    pub start: RefPoint,
    pub end: RefPoint,
    pub label: String,
    pub short_label: String,
    /// Cobre um capítulo (ou livro) inteiro: a interface não seleciona um versículo.
    pub whole: bool,
}

fn range_dto(range: &VerseRange, language: Language) -> RangeDto {
    let whole = range.start.verse == 1
        && range.start != range.end
        && range.end.book.verses_in_chapter(range.end.chapter) == Some(range.end.verse);
    RangeDto {
        start: range.start.into(),
        end: range.end.into(),
        label: format(range, language, FormatStyle::Full),
        short_label: format(range, language, FormatStyle::Short),
        whole,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RefErrorDto {
    pub code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verse: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl RefErrorDto {
    fn bare(code: &'static str) -> Self {
        Self {
            code,
            book: None,
            book_name: None,
            chapter: None,
            verse: None,
            max: None,
            text: None,
        }
    }
}

fn book_name(book: Book, language: Language) -> String {
    match language {
        Language::PtBr => book.name_pt(),
        Language::En => book.name_en(),
    }
    .to_string()
}

fn error_dto(error: &RefError, language: Language) -> RefErrorDto {
    match error {
        RefError::UnknownBook(text) => RefErrorDto {
            text: Some(text.chars().take(80).collect()),
            ..RefErrorDto::bare("unknown_book")
        },
        RefError::InvalidSyntax(text) => RefErrorDto {
            text: Some(text.chars().take(80).collect()),
            ..RefErrorDto::bare("invalid_syntax")
        },
        RefError::InvalidChapter { book, chapter, max } => RefErrorDto {
            book: Some(book.number()),
            book_name: Some(book_name(*book, language)),
            chapter: Some(*chapter),
            max: Some(*max),
            ..RefErrorDto::bare("invalid_chapter")
        },
        RefError::InvalidVerse {
            book,
            chapter,
            verse,
            max,
        } => RefErrorDto {
            book: Some(book.number()),
            book_name: Some(book_name(*book, language)),
            chapter: Some(*chapter),
            verse: Some(*verse),
            max: Some(*max),
            ..RefErrorDto::bare("invalid_verse")
        },
        RefError::InvalidRange => RefErrorDto::bare("invalid_range"),
        RefError::InputTooLong => RefErrorDto::bare("input_too_long"),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SuggestionDto {
    pub book: u8,
    pub name: String,
    pub abbreviation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<RangeDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ResolveDto {
    pub ranges: Vec<RangeDto>,
    pub errors: Vec<RefErrorDto>,
    pub suggestions: Vec<SuggestionDto>,
}

pub fn parse_language(lang: &str) -> Language {
    if lang.to_ascii_lowercase().starts_with("en") {
        Language::En
    } else {
        Language::PtBr
    }
}

fn abbreviation(book: Book, language: Language) -> String {
    match language {
        Language::PtBr => book.primary_abbreviation_pt(),
        Language::En => book.primary_abbreviation_en(),
    }
    .to_string()
}

fn suggestion_dto(book: Book, range: Option<&VerseRange>, language: Language) -> SuggestionDto {
    SuggestionDto {
        book: book.number(),
        name: book_name(book, language),
        abbreviation: abbreviation(book, language),
        range: range.map(|range| range_dto(range, language)),
    }
}

/// Interpreta o texto digitado. Para entrada parcial, devolve também sugestões de livros.
pub fn resolve_reference(text: &str, lang: &str) -> ResolveDto {
    let language = parse_language(lang);
    let outcome = parse(text, language);
    let mut suggestions: Vec<SuggestionDto> = outcome
        .sugestões
        .iter()
        .map(|item| suggestion_dto(item.book, item.reference.as_ref(), language))
        .collect();
    let trimmed = text.trim();
    let only_book_prefix = !trimmed.is_empty()
        && !trimmed
            .chars()
            .any(|c| c.is_ascii_digit() || matches!(c, ':' | ';' | ','));
    if suggestions.is_empty() && outcome.faixas.is_empty() && only_book_prefix {
        suggestions = suggest_books(trimmed, language, 8)
            .into_iter()
            .map(|book| suggestion_dto(book, None, language))
            .collect();
    }
    ResolveDto {
        ranges: outcome
            .faixas
            .iter()
            .map(|range| range_dto(range, language))
            .collect(),
        errors: outcome
            .erros
            .iter()
            .map(|error| error_dto(error, language))
            .collect(),
        suggestions,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BookInfo {
    pub number: u8,
    pub name: String,
    pub abbreviation: String,
    pub chapters: u16,
    pub testament: &'static str,
}

pub fn book_names(lang: &str) -> Vec<BookInfo> {
    let language = parse_language(lang);
    Book::ALL
        .iter()
        .map(|&book| BookInfo {
            number: book.number(),
            name: book_name(book, language),
            abbreviation: abbreviation(book, language),
            chapters: book.chapter_count(),
            testament: if book.testament() == Testament::Old {
                "old"
            } else {
                "new"
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    use rhema_core::{LinkTarget, ModuleKind};

    use super::*;

    fn text(value: &str) -> Node {
        Node::Text(value.to_string())
    }

    fn one(node: Node) -> UiNode {
        let mut converted = convert_nodes(&[node]);
        assert_eq!(converted.len(), 1, "{converted:?}");
        converted.remove(0)
    }

    fn json(node: &UiNode) -> serde_json::Value {
        serde_json::to_value(node).expect("serialize")
    }

    #[test]
    fn converts_text_and_keeps_literal_angle_brackets() {
        assert_eq!(
            one(text("a <b> c")),
            UiNode::Text {
                value: "a <b> c".into()
            }
        );
        assert_eq!(one(text("<>")), UiNode::Text { value: "<>".into() });
    }

    #[test]
    fn converts_each_inline_style() {
        let cases: Vec<(Node, &str)> = vec![
            (Node::Italic(vec![text("x")]), "italic"),
            (Node::Bold(vec![text("x")]), "bold"),
            (Node::Underline(vec![text("x")]), "underline"),
            (Node::Strikethrough(vec![text("x")]), "strikethrough"),
            (Node::Caps(vec![text("x")]), "caps"),
            (Node::Superscript(vec![text("x")]), "superscript"),
            (Node::Subscript(vec![text("x")]), "subscript"),
            (Node::WordsOfJesus(vec![text("x")]), "wordsOfJesus"),
            (Node::OtQuote(vec![text("x")]), "otQuote"),
            (Node::Note(vec![text("x")]), "note"),
        ];
        for (node, expected) in cases {
            let value = json(&one(node));
            assert_eq!(value["type"], expected);
            assert_eq!(value["children"][0]["value"], "x");
        }
    }

    #[test]
    fn converts_heading_and_clamps_level() {
        let value = json(&one(Node::Heading(9, vec![text("Título de exemplo")])));
        assert_eq!(value["type"], "heading");
        assert_eq!(value["level"], 6);
    }

    #[test]
    fn converts_strong_without_grammar_and_drops_empty_strong() {
        let node = Node::Strong {
            lang: StrongLang::Greek,
            num: 25,
            children: vec![
                text("[palavra]"),
                Node::Grammar {
                    lang: StrongLang::Greek,
                    code: 8804,
                },
            ],
        };
        let value = json(&one(node));
        assert_eq!(value["type"], "strong");
        assert_eq!(value["lang"], "G");
        assert_eq!(value["num"], 25);
        assert_eq!(value["children"].as_array().map(Vec::len), Some(1));
        let empty = Node::Strong {
            lang: StrongLang::Hebrew,
            num: 1,
            children: vec![],
        };
        assert!(convert_nodes(&[empty]).is_empty());
    }

    #[test]
    fn converts_breaks_and_links() {
        assert_eq!(json(&one(Node::Paragraph))["type"], "paragraph");
        assert_eq!(json(&one(Node::LineBreak))["type"], "lineBreak");
        let link = Node::Link {
            target: LinkTarget::Other("x".into()),
            children: vec![text("ligação")],
        };
        let value = json(&one(link));
        assert_eq!(value["type"], "link");
        assert!(value.get("target").is_none());
    }

    #[test]
    fn metadata_nodes_produce_no_output() {
        let nodes = [
            Node::Grammar {
                lang: StrongLang::Hebrew,
                code: 1,
            },
            Node::Morph("x".into()),
            Node::XRef(VerseRange::single(VerseRef::new(Book::John, 3, 16))),
        ];
        assert!(convert_nodes(&nodes).is_empty());
    }

    #[test]
    fn unknown_tag_keeps_only_inner_text() {
        let node = Node::UnknownTag {
            name: "ZZ".into(),
            raw: "<ZZ9>".into(),
            children: vec![text("interno")],
        };
        assert_eq!(
            one(node),
            UiNode::Text {
                value: "interno".into()
            }
        );
        let serialized = serde_json::to_string(&convert_nodes(&[Node::UnknownTag {
            name: "ZZ".into(),
            raw: "<ZZ9>".into(),
            children: vec![],
        }]))
        .expect("serialize");
        assert_eq!(serialized, "[]");
    }

    fn write_nt(path: &Path) {
        let mut out = String::new();
        let mut number = 0;
        for book in Book::ALL.iter().filter(|b| b.testament() == Testament::New) {
            for chapter in 1..=book.chapter_count() {
                for verse in 1..=book.verses_in_chapter(chapter).unwrap_or(0) {
                    number += 1;
                    let line = match (book, chapter, verse) {
                        (Book::John, 3, 1) => {
                            "<TS1>Título de exemplo<Ts1>Texto de exemplo <FI>itálico<Fi> <FR>fala<Fr><RF>nota<Rf><CM>".to_string()
                        }
                        (Book::John, 3, 2) => {
                            "palavra<WG25><G8804> <ZZ9>interno<Zz> fim".to_string()
                        }
                        _ => format!("Texto de exemplo {number}"),
                    };
                    out.push_str(&line);
                    out.push_str("\r\n");
                }
            }
        }
        out.push_str(
            "\r\ntitle = NT de teste\r\nshort.title = NTT\r\nlang = por\r\ncharset = 65001\r\n",
        );
        std::fs::write(path, out).expect("write NT");
    }

    fn module(id: &str, path: &Path, kind: ModuleKind, state: ModuleState) -> ModuleInfo {
        ModuleInfo {
            id: id.into(),
            path: path.to_path_buf(),
            kind,
            state,
            title: "Teste".into(),
            abbreviation: None,
            language: None,
            language_name: None,
            description: None,
            version: None,
            size: 1,
            has_strong: false,
        }
    }

    #[test]
    fn reads_synthetic_chapter_with_structured_nodes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("teste.nt");
        write_nt(&path);
        let cache = ReaderCache::new(READER_CACHE_CAPACITY);
        let info = module("nt", &path, ModuleKind::Bible, ModuleState::Ready);
        let chapter = read_chapter(&cache, &info, Book::John.number(), 3).expect("chapter");
        assert_eq!(chapter.verses.len(), 36);
        assert_eq!(chapter.verses[0].number, 1);
        let encoded = serde_json::to_string(&chapter).expect("serialize");
        for fragment in [
            "heading",
            "italic",
            "wordsOfJesus",
            "note",
            "paragraph",
            "strong",
        ] {
            assert!(encoded.contains(fragment), "faltou {fragment}: {encoded}");
        }
        assert!(!encoded.contains("grammar") && !encoded.contains("unknown"));
        assert!(
            !encoded.contains('<') && !encoded.contains('>'),
            "{encoded}"
        );
        assert!(encoded.contains("interno"));
    }

    #[test]
    fn parallel_chapters_align_rows_and_isolate_column_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("teste.nt");
        write_nt(&path);
        let ready = module("ready", &path, ModuleKind::Bible, ModuleState::Ready);
        let protected = module(
            "protected",
            &path,
            ModuleKind::Bible,
            ModuleState::Protected,
        );
        let cache = ReaderCache::new(READER_CACHE_CAPACITY);
        let aligned = read_chapters(&cache, &[ready, protected], 43, 3).expect("aligned");
        assert_eq!(aligned.rows.len(), 36);
        assert_eq!(aligned.rows[0].number, 1);
        assert_eq!(aligned.rows[0].columns.len(), 2);
        assert!(aligned.rows[0].columns[0].is_some());
        assert!(aligned.rows[0].columns[1].is_none());
        assert_eq!(aligned.columns[0].error, None);
        assert_eq!(
            aligned.columns[1].error.as_ref().map(|error| error.code),
            Some("module_protected")
        );
        assert_eq!(cache.open_count(), 1);
    }

    #[test]
    fn nt_only_bible_reports_missing_book() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("teste.nt");
        write_nt(&path);
        let cache = ReaderCache::new(2);
        let info = module("nt", &path, ModuleKind::Bible, ModuleState::Ready);
        let error = read_chapter(&cache, &info, Book::Genesis.number(), 1).expect_err("missing");
        assert_eq!(error.code, "book_not_in_module");
        assert_eq!(error.book, Some(1));
        let value = serde_json::to_value(&error).expect("serialize");
        assert_eq!(value["code"], "book_not_in_module");
    }

    #[test]
    fn invalid_chapter_and_book_report_stable_codes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("teste.nt");
        write_nt(&path);
        let cache = ReaderCache::new(2);
        let info = module("nt", &path, ModuleKind::Bible, ModuleState::Ready);
        let error = read_chapter(&cache, &info, Book::John.number(), 22).expect_err("chapter");
        assert_eq!((error.code, error.max), ("invalid_reference", Some(21)));
        let error = read_chapter(&cache, &info, 0, 1).expect_err("book");
        assert_eq!(error.code, "invalid_reference");
        let error = read_chapter(&cache, &info, 67, 1).expect_err("book");
        assert_eq!(error.code, "invalid_reference");
        assert_eq!(
            cache.open_count(),
            0,
            "validação ocorre antes de abrir o arquivo"
        );
    }

    #[test]
    fn protected_and_unready_modules_are_never_opened() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("protegido.ontx");
        std::fs::write(&path, b"TWENCBMOD dados falsos").expect("fake");
        let cache = ReaderCache::new(2);
        let protected = module("p", &path, ModuleKind::Bible, ModuleState::Protected);
        let error = read_chapter(&cache, &protected, 43, 3).expect_err("protected");
        assert_eq!(error.code, "module_protected");
        let corrupted = module(
            "c",
            &path,
            ModuleKind::Bible,
            ModuleState::Corrupted {
                message: "x".into(),
            },
        );
        assert_eq!(
            read_chapter(&cache, &corrupted, 43, 3).expect_err("c").code,
            "module_corrupted"
        );
        let unsupported = module(
            "u",
            &path,
            ModuleKind::Bible,
            ModuleState::UnsupportedFormat {
                message: "x".into(),
            },
        );
        assert_eq!(
            read_chapter(&cache, &unsupported, 43, 3)
                .expect_err("u")
                .code,
            "module_unsupported"
        );
        let dictionary = module("d", &path, ModuleKind::Dictionary, ModuleState::Ready);
        assert_eq!(
            read_chapter(&cache, &dictionary, 43, 3)
                .expect_err("d")
                .code,
            "module_unsupported"
        );
        assert_eq!(cache.open_count(), 0);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn missing_or_truncated_files_become_errors_not_panics() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache = ReaderCache::new(2);
        let absent = module(
            "a",
            &temp.path().join("nao-existe.ont"),
            ModuleKind::Bible,
            ModuleState::Ready,
        );
        assert_eq!(
            read_chapter(&cache, &absent, 43, 3)
                .expect_err("absent")
                .code,
            "module_not_found"
        );
        let empty = temp.path().join("vazio.ont");
        std::fs::write(&empty, b"").expect("empty");
        let info = module("e", &empty, ModuleKind::Bible, ModuleState::Ready);
        assert_eq!(
            read_chapter(&cache, &info, 43, 3).expect_err("empty").code,
            "module_corrupted"
        );
        let short = temp.path().join("curto.ont");
        std::fs::write(&short, "Texto de exemplo 1\r\nTexto de exemplo 2\r\n").expect("short");
        let info = module("s", &short, ModuleKind::Bible, ModuleState::Ready);
        assert_eq!(
            read_chapter(&cache, &info, 43, 3).expect_err("short").code,
            "verse_not_found"
        );
    }

    #[test]
    fn partially_missing_chapter_returns_existing_verses_and_flag() {
        let temp = tempfile::tempdir().expect("tempdir");
        let first = VerseRef::new(Book::John, 3, 1)
            .cumulative_index()
            .expect("index");
        let path = temp.path().join("parcial.ont");
        let lines: String = (0..first + 5)
            .map(|n| format!("Texto de exemplo {n}\r\n"))
            .collect();
        std::fs::write(&path, lines).expect("write");
        let cache = ReaderCache::new(2);
        let info = module("p", &path, ModuleKind::Bible, ModuleState::Ready);
        let chapter = read_chapter(&cache, &info, 43, 3).expect("partial chapter");
        assert!(chapter.incomplete);
        assert_eq!(chapter.verses.len(), 5);
        assert_eq!(chapter.verses[0].number, 1);
        let complete = read_chapter(&cache, &info, 43, 2).expect("complete chapter");
        assert!(!complete.incomplete);
    }

    #[test]
    fn literal_angle_brackets_reach_the_interface_unchanged() {
        let nodes = vec![Node::Text("< a > b <".to_string())];
        assert_eq!(
            convert_nodes(&nodes),
            vec![UiNode::Text {
                value: "< a > b <".to_string()
            }]
        );
    }

    #[test]
    fn reader_cache_reuses_and_evicts_least_recently_used() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache = ReaderCache::new(READER_CACHE_CAPACITY);
        let mut modules = Vec::new();
        for index in 0..(READER_CACHE_CAPACITY + 1) {
            let path = temp.path().join(format!("m{index}.nt"));
            write_nt(&path);
            modules.push(module(
                &format!("m{index}"),
                &path,
                ModuleKind::Bible,
                ModuleState::Ready,
            ));
        }
        for info in &modules[..READER_CACHE_CAPACITY] {
            read_chapter(&cache, info, 43, 1).expect("read");
        }
        assert_eq!(
            (cache.len(), cache.open_count()),
            (READER_CACHE_CAPACITY, READER_CACHE_CAPACITY)
        );
        read_chapter(&cache, &modules[0], 43, 2).expect("reuse");
        assert_eq!(
            cache.open_count(),
            READER_CACHE_CAPACITY,
            "reuso não reabre o arquivo"
        );
        read_chapter(&cache, &modules[READER_CACHE_CAPACITY], 43, 1).expect("evict");
        assert_eq!(cache.len(), READER_CACHE_CAPACITY);
        assert!(cache.contains("m0"), "m0 foi usado recentemente");
        assert!(!cache.contains("m1"), "m1 era o menos usado");
        assert!(cache.contains(&format!("m{READER_CACHE_CAPACITY}")));
    }

    #[test]
    fn resolves_references_with_limits_and_suggestions() {
        let ok = resolve_reference("Jo 3:16-18", "pt-BR");
        assert_eq!(ok.ranges.len(), 1);
        assert_eq!(ok.ranges[0].label, "João 3:16-18");
        assert_eq!(ok.ranges[0].start.verse, 16);
        assert!(!ok.ranges[0].whole);
        assert!(resolve_reference("Jo 3", "pt-BR").ranges[0].whole);
        assert!(!resolve_reference("Jo 3:1", "pt-BR").ranges[0].whole);
        assert!(!resolve_reference("Jo 3:1-5", "pt-BR").ranges[0].whole);
        let bad = resolve_reference("Jo 22", "pt-BR");
        assert_eq!(bad.errors[0].code, "invalid_chapter");
        assert_eq!(bad.errors[0].max, Some(21));
        assert_eq!(bad.errors[0].book_name.as_deref(), Some("João"));
        let verse = resolve_reference("Jo 3:99", "pt-BR");
        assert_eq!(verse.errors[0].code, "invalid_verse");
        let partial = resolve_reference("Jo", "pt-BR");
        assert!(partial.errors.is_empty() && !partial.suggestions.is_empty());
        let prefix = resolve_reference("gên", "pt-BR");
        assert!(prefix.suggestions.iter().any(|s| s.book == 1), "{prefix:?}");
        let english = resolve_reference("John 3:16", "en");
        assert_eq!(english.ranges[0].label, "John 3:16");
        assert_eq!(
            resolve_reference("", "pt-BR"),
            ResolveDto {
                ranges: vec![],
                errors: vec![],
                suggestions: vec![]
            }
        );
        assert_eq!(
            resolve_reference("zzzz 1", "pt-BR").errors[0].code,
            "unknown_book"
        );
        let long = "x".repeat(20_000);
        assert_eq!(
            resolve_reference(&long, "pt-BR").errors[0].code,
            "input_too_long"
        );
    }

    #[test]
    fn book_names_lists_66_books_per_language() {
        let pt = book_names("pt-BR");
        assert_eq!(pt.len(), 66);
        assert_eq!(
            (
                pt[42].name.as_str(),
                pt[42].abbreviation.as_str(),
                pt[42].chapters
            ),
            ("João", "Jo", 21)
        );
        assert_eq!(pt[0].testament, "old");
        assert_eq!(pt[65].testament, "new");
        let en = book_names("en");
        assert_eq!(en[42].name, "John");
    }

    fn find_module(folder: &Path, wanted: &str) -> Option<PathBuf> {
        std::fs::read_dir(folder)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().to_lowercase() == wanted)
            })
    }

    fn raw_has_angle(node: &Node) -> bool {
        match node {
            Node::Text(text) => text.contains(['<', '>']),
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
            | Node::Note(c)
            | Node::Strong { children: c, .. }
            | Node::Link { children: c, .. }
            | Node::UnknownTag { children: c, .. } => c.iter().any(raw_has_angle),
            _ => false,
        }
    }

    /// Roda somente com RHEMA_TEST_MODULES definido. Não imprime texto de módulos.
    #[test]
    fn real_bibles_read_all_chapters_without_markup_residue() {
        let Ok(folder) = std::env::var("RHEMA_TEST_MODULES") else {
            return;
        };
        let folder = PathBuf::from(folder);
        let cache = ReaderCache::new(READER_CACHE_CAPACITY);
        for name in ["ara+.ont", "kjv.ont"] {
            let Some(path) = find_module(&folder, name) else {
                eprintln!("{name}: ausente");
                continue;
            };
            let info = module(name, &path, ModuleKind::Bible, ModuleState::Ready);
            let (mut total, mut worst, mut worst_at, mut chapters) =
                (0.0f64, 0.0f64, (0, 0), 0usize);
            let (mut bad_chars, mut errors) = (0usize, 0usize);
            let raw_reader = OntReader::open(&path).expect("open real Bible");
            let mut raw_angle_chapters = 0usize;
            for book in Book::ALL {
                for chapter in 1..=book.chapter_count() {
                    if let Ok(verses) = raw_reader.chapter_nodes(book, chapter)
                        && verses
                            .iter()
                            .any(|(_, nodes)| nodes.iter().any(raw_has_angle))
                    {
                        raw_angle_chapters += 1;
                    }
                }
            }
            eprintln!(
                "{name}: capítulos com < ou > no texto bruto do tokenizador={raw_angle_chapters}"
            );
            for book in Book::ALL {
                for chapter in 1..=book.chapter_count() {
                    let started = Instant::now();
                    let result = read_chapter(&cache, &info, book.number(), chapter);
                    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                    match result {
                        Ok(dto) => {
                            let encoded = serde_json::to_string(&dto).expect("serialize");
                            // A interface não altera o texto: o que sobra é literal do módulo.
                            if encoded.contains('<') || encoded.contains('>') {
                                bad_chars += 1;
                            }
                            if name == "kjv.ont"
                                && book == Book::John
                                && chapter == 3
                                && !encoded.contains("wordsOfJesus")
                            {
                                errors += 1;
                            }
                        }
                        Err(_) => errors += 1,
                    }
                    total += elapsed;
                    chapters += 1;
                    if elapsed > worst {
                        worst = elapsed;
                        worst_at = (book.number(), chapter);
                    }
                }
            }
            eprintln!(
                "{name}: capítulos={chapters} erros={errors} com_sinais={bad_chars} média={:.3} ms máximo={:.3} ms (livro {} cap. {})",
                total / chapters as f64,
                worst,
                worst_at.0,
                worst_at.1
            );
            assert_eq!(chapters, 1189);
            assert_eq!(
                bad_chars, raw_angle_chapters,
                "{name}: a interface alterou sinais < ou >"
            );
            assert_eq!(errors, 0, "{name}: capítulos com erro");
        }
    }

    #[test]
    fn real_parallel_bible_chapter_counts_and_release_timing() {
        let Ok(folder) = std::env::var("RHEMA_TEST_MODULES") else {
            return;
        };
        let folder = PathBuf::from(folder);
        let ara_path = find_module(&folder, "ara+.ont").expect("ARA+ ausente");
        let kjv_path = find_module(&folder, "kjv.ont").expect("KJV ausente");
        let modules = [
            module("ara+", &ara_path, ModuleKind::Bible, ModuleState::Ready),
            module("kjv", &kjv_path, ModuleKind::Bible, ModuleState::Ready),
        ];
        let cache = ReaderCache::new(READER_CACHE_CAPACITY);
        let mut elapsed_total = 0.0f64;
        let mut elapsed_max = 0.0f64;
        let mut chapter_count = 0usize;
        let mut different_counts = Vec::new();
        for book in Book::ALL {
            for chapter in 1..=book.chapter_count() {
                let started = Instant::now();
                let aligned = read_chapters(&cache, &modules, book.number(), chapter)
                    .expect("leitura paralela");
                let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                let counts = aligned
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(column, _)| {
                        aligned
                            .rows
                            .iter()
                            .filter(|row| row.columns[column].is_some())
                            .count()
                    })
                    .collect::<Vec<_>>();
                assert!(aligned.columns.iter().all(|column| column.error.is_none()));
                if counts[0] != counts[1] {
                    different_counts.push((book.number(), chapter));
                }
                elapsed_total += elapsed;
                elapsed_max = elapsed_max.max(elapsed);
                chapter_count += 1;
            }
        }
        let examples = different_counts
            .iter()
            .take(10)
            .map(|(book, chapter)| format!("{book}:{chapter}"))
            .collect::<Vec<_>>()
            .join(",");
        eprintln!(
            "parallel_chapters={} divergent={} first10={} avg_ms={:.3} max_ms={:.3}",
            chapter_count,
            different_counts.len(),
            examples,
            elapsed_total / chapter_count as f64,
            elapsed_max
        );
        assert_eq!(chapter_count, 1189);
    }
}
