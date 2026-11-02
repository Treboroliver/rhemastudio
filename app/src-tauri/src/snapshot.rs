//! Instantâneos de texto para inserir em sermões: versículos e verbetes de léxico.

use rhema_core::{Book, Node, VerseRange, VerseRef};
use rhema_library::ModuleInfo;
use rhema_refs::{FormatStyle, format};
use serde::{Deserialize, Serialize};

use crate::reader::{ApiError, ReaderCache, UiNode, ensure_readable_bible, parse_language};
use crate::study::{StudyCache, lookup_strong};

/// Máximo de versículos de uma faixa inserida no sermão.
pub const MAX_SNAPSHOT_VERSES: usize = 80;

/// Máximo de caracteres do texto de um verbete no sermão.
pub const MAX_LEXICON_CHARS: usize = 1_500;

/// Livro por número (saída de `resolve_reference`) ou pelo nome da variante (blocos salvos).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(untagged)]
pub enum BookArg {
    Number(u8),
    Name(Book),
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct PointArg {
    pub book: BookArg,
    pub chapter: u16,
    pub verse: u16,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct RangeArg {
    pub start: PointArg,
    pub end: PointArg,
}

impl PointArg {
    fn to_ref(self) -> Result<VerseRef, ApiError> {
        let book = match self.book {
            BookArg::Name(book) => book,
            BookArg::Number(number) => Book::from_number(number).ok_or_else(|| {
                ApiError::new("invalid_reference", format!("Livro inválido: {number}"))
            })?,
        };
        Ok(VerseRef::new(book, self.chapter, self.verse))
    }
}

impl RangeArg {
    pub fn to_range(self) -> Result<VerseRange, ApiError> {
        VerseRange::new(self.start.to_ref()?, self.end.to_ref()?)
            .map_err(|error| ApiError::new("invalid_reference", error.to_string()))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VerseSnapshotDto {
    pub reference_label: String,
    pub module_abbr: String,
    pub text: String,
    pub range: VerseRange,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LexiconSnapshotDto {
    pub strong_id: String,
    pub module_id: String,
    pub title: String,
    pub text: String,
}

fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Texto simples de um versículo: sem notas, sem títulos de seção e sem metadados.
fn verse_plain_text(nodes: &[Node]) -> String {
    let mut out = String::new();
    for node in nodes {
        match node {
            Node::Heading(..) => {}
            Node::Paragraph | Node::LineBreak => out.push(' '),
            other => out.push_str(&other.to_plain_text()),
        }
    }
    collapse_spaces(&out)
}

pub fn verse_snapshot(
    cache: &ReaderCache,
    module: &ModuleInfo,
    range: VerseRange,
    interface_language: &str,
) -> Result<VerseSnapshotDto, ApiError> {
    ensure_readable_bible(module)?;
    let (Some(first), Some(last)) = (range.start.cumulative_index(), range.end.cumulative_index())
    else {
        return Err(ApiError::new(
            "invalid_reference",
            "Passagem fora dos limites.",
        ));
    };
    let count = last.saturating_sub(first) + 1;
    if count > MAX_SNAPSHOT_VERSES {
        return Err(ApiError {
            max: Some(MAX_SNAPSHOT_VERSES as u16),
            ..ApiError::new(
                "range_too_large",
                format!("A passagem tem mais de {MAX_SNAPSHOT_VERSES} versículos."),
            )
        });
    }
    let reader = cache.get_or_open(module)?;
    let numbered = count > 1;
    let mut parts = Vec::with_capacity(count);
    let mut current = Some(range.start);
    while let Some(verse) = current {
        if verse > range.end {
            break;
        }
        let nodes = reader.verse_nodes(verse.book, verse.chapter, verse.verse)?;
        let text = verse_plain_text(&nodes);
        parts.push(if numbered {
            format!("{} {}", verse.verse, text)
        } else {
            text
        });
        current = verse.next_verse();
    }
    let language = parse_language(interface_language);
    Ok(VerseSnapshotDto {
        reference_label: format(&range, language, FormatStyle::Short),
        module_abbr: module
            .abbreviation
            .clone()
            .filter(|abbr| !abbr.trim().is_empty())
            .unwrap_or_else(|| module.title.clone()),
        text: parts.join(" "),
        range,
    })
}

fn push_ui_text(nodes: &[UiNode], out: &mut String) {
    for node in nodes {
        match node {
            UiNode::Text { value } => out.push_str(value),
            UiNode::Paragraph => out.push_str("\n\n"),
            UiNode::LineBreak => out.push('\n'),
            UiNode::Note { .. } => {}
            UiNode::Heading { children, .. } => {
                out.push_str("\n\n");
                push_ui_text(children, out);
                out.push_str("\n\n");
            }
            UiNode::Italic { children }
            | UiNode::Bold { children }
            | UiNode::Underline { children }
            | UiNode::Strikethrough { children }
            | UiNode::Caps { children }
            | UiNode::Superscript { children }
            | UiNode::Subscript { children }
            | UiNode::WordsOfJesus { children }
            | UiNode::OtQuote { children }
            | UiNode::Strong { children, .. }
            | UiNode::Link { children, .. }
            | UiNode::VerseLink { children, .. }
            | UiNode::EntryLink { children, .. } => push_ui_text(children, out),
        }
    }
}

/// Parágrafos separados por linha em branco; espaços repetidos viram um só.
fn paragraphs_text(nodes: &[UiNode]) -> String {
    let mut raw = String::new();
    push_ui_text(nodes, &mut raw);
    let mut paragraphs = Vec::new();
    for block in raw.split("\n\n") {
        let lines: Vec<String> = block
            .split('\n')
            .map(collapse_spaces)
            .filter(|line| !line.is_empty())
            .collect();
        if !lines.is_empty() {
            paragraphs.push(lines.join("\n"));
        }
    }
    paragraphs.join("\n\n")
}

/// Corta em `limit` caracteres, preferindo o fim de um parágrafo, depois de uma frase.
pub fn truncate_text(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let head: String = text.chars().take(limit).collect();
    let next_is_break = text.chars().nth(limit).is_none_or(char::is_whitespace);
    let cut = if next_is_break {
        Some(head.len())
    } else {
        head.rfind("\n\n").filter(|&at| at > 0).or_else(|| {
            let mut best = None;
            let chars: Vec<(usize, char)> = head.char_indices().collect();
            for window in chars.windows(2) {
                if matches!(window[0].1, '.' | '!' | '?' | '…') && window[1].1.is_whitespace() {
                    best = Some(window[1].0);
                }
            }
            best.filter(|&at| at > 0)
        })
    };
    let end = cut.or_else(|| head.rfind(char::is_whitespace).filter(|&at| at > 0));
    let kept = &head[..end.unwrap_or(head.len())];
    format!("{}…", kept.trim_end())
}

pub fn lexicon_snapshot(
    cache: &StudyCache,
    lexicons: &[ModuleInfo],
    strong_id: &str,
) -> Result<LexiconSnapshotDto, ApiError> {
    let entry = lookup_strong(cache, lexicons, strong_id)?;
    let title = entry
        .original_word
        .as_deref()
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .unwrap_or(entry.title.trim())
        .to_string();
    Ok(LexiconSnapshotDto {
        text: truncate_text(&paragraphs_text(&entry.nodes), MAX_LEXICON_CHARS),
        strong_id: entry.key,
        module_id: entry.module_id,
        title,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rhema_core::{ModuleKind, Testament};
    use rhema_library::ModuleState;

    use super::*;
    use crate::reader::READER_CACHE_CAPACITY;
    use crate::study::STUDY_CACHE_CAPACITY;

    fn write_nt(path: &Path) {
        let mut out = String::new();
        for book in Book::ALL.iter().filter(|b| b.testament() == Testament::New) {
            for chapter in 1..=book.chapter_count() {
                for verse in 1..=book.verses_in_chapter(chapter).unwrap_or(0) {
                    let line = match (book, chapter, verse) {
                        (Book::John, 3, 1) => {
                            "<TS1>Seção inventada<Ts1>Alfa <FI>beta<Fi><RF>nota<Rf> gama<CM>"
                                .to_string()
                        }
                        (Book::John, 3, 2) => "Delta<WG25><G8804> épsilon".to_string(),
                        _ => format!("Texto {verse}"),
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

    fn bible(path: &Path, state: ModuleState) -> ModuleInfo {
        ModuleInfo {
            id: "nt".into(),
            path: path.to_path_buf(),
            kind: ModuleKind::Bible,
            state,
            title: "Teste".into(),
            abbreviation: Some("NTT".into()),
            language: None,
            language_name: None,
            description: None,
            version: None,
            size: 1,
            has_strong: false,
        }
    }

    fn range(book: Book, ch: u16, from: u16, to: u16) -> VerseRange {
        VerseRange::new(VerseRef::new(book, ch, from), VerseRef::new(book, ch, to)).expect("range")
    }

    fn fixture() -> (tempfile::TempDir, ModuleInfo, ReaderCache) {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("teste.nt");
        write_nt(&path);
        let info = bible(&path, ModuleState::Ready);
        (temp, info, ReaderCache::new(READER_CACHE_CAPACITY))
    }

    #[test]
    fn single_verse_has_no_notes_headings_or_number() {
        let (_temp, info, cache) = fixture();
        let dto = verse_snapshot(&cache, &info, range(Book::John, 3, 1, 1), "pt-BR").expect("ok");
        assert_eq!(dto.text, "Alfa beta gama");
        assert_eq!(dto.module_abbr, "NTT");
        assert_eq!(dto.reference_label, "Jo 3:1");
        let en = verse_snapshot(&cache, &info, range(Book::John, 3, 2, 2), "en").expect("ok");
        assert_eq!(en.reference_label, "John 3:2");
        assert_eq!(en.text, "Delta épsilon");
    }

    #[test]
    fn range_is_numbered_per_verse() {
        let (_temp, info, cache) = fixture();
        let dto = verse_snapshot(&cache, &info, range(Book::John, 3, 1, 3), "pt-BR").expect("ok");
        assert_eq!(dto.text, "1 Alfa beta gama 2 Delta épsilon 3 Texto 3");
        assert_eq!(dto.reference_label, "Jo 3:1-3");
    }

    #[test]
    fn range_above_limit_is_rejected_before_opening() {
        let (_temp, info, cache) = fixture();
        let ok = verse_snapshot(&cache, &info, range(Book::John, 1, 1, 51), "pt-BR");
        assert!(ok.is_ok());
        let wide = VerseRange::new(
            VerseRef::new(Book::John, 1, 1),
            VerseRef::new(Book::John, 3, 36),
        )
        .expect("range");
        let error = verse_snapshot(&cache, &info, wide, "pt-BR").expect_err("too large");
        assert_eq!((error.code, error.max), ("range_too_large", Some(80)));
        let at_limit = VerseRange::new(
            VerseRef::new(Book::John, 3, 1),
            VerseRef::new(Book::John, 3, 36),
        )
        .expect("range");
        assert!(verse_snapshot(&cache, &info, at_limit, "pt-BR").is_ok());
    }

    #[test]
    fn missing_book_missing_verse_and_protected_are_typed_errors() {
        let (temp, info, cache) = fixture();
        let error = verse_snapshot(&cache, &info, range(Book::Genesis, 1, 1, 1), "pt-BR")
            .expect_err("book");
        assert_eq!(error.code, "book_not_in_module");
        let short = temp.path().join("curta.nt");
        std::fs::write(&short, "Um\r\nDois\r\n").expect("write");
        let truncated = bible(&short, ModuleState::Ready);
        let error = verse_snapshot(
            &cache,
            &ModuleInfo {
                id: "curta".into(),
                ..truncated
            },
            range(Book::Revelation, 22, 21, 21),
            "pt-BR",
        )
        .expect_err("verse");
        assert!(
            matches!(error.code, "verse_not_found" | "module_corrupted"),
            "{}",
            error.code
        );
        let protected = bible(&info.path, ModuleState::Protected);
        let before = cache.open_count();
        let error = verse_snapshot(&cache, &protected, range(Book::John, 3, 1, 1), "pt-BR")
            .expect_err("protected");
        assert_eq!(error.code, "module_protected");
        assert_eq!(cache.open_count(), before);
    }

    #[test]
    fn accepts_numeric_and_named_books() {
        let by_number: RangeArg = serde_json::from_str(
            r#"{"start":{"book":43,"chapter":3,"verse":1},"end":{"book":43,"chapter":3,"verse":2}}"#,
        )
        .expect("number");
        let by_name: RangeArg = serde_json::from_str(
            r#"{"start":{"book":"John","chapter":3,"verse":1},"end":{"book":"John","chapter":3,"verse":2}}"#,
        )
        .expect("name");
        assert_eq!(
            by_number.to_range().expect("a"),
            by_name.to_range().expect("b")
        );
        let invalid: RangeArg = serde_json::from_str(
            r#"{"start":{"book":99,"chapter":1,"verse":1},"end":{"book":99,"chapter":1,"verse":1}}"#,
        )
        .expect("parse");
        assert_eq!(
            invalid.to_range().expect_err("book").code,
            "invalid_reference"
        );
    }

    #[test]
    fn truncation_prefers_paragraph_then_sentence_and_marks_the_cut() {
        assert_eq!(truncate_text("curto", 50), "curto");
        let text = "Primeiro parágrafo aqui.\n\nSegundo parágrafo bem mais longo que o limite";
        assert_eq!(truncate_text(text, 40), "Primeiro parágrafo aqui.…");
        let sentences = "Uma frase curta. Outra frase que passa do limite definido";
        assert_eq!(truncate_text(sentences, 30), "Uma frase curta.…");
        let words = "palavra palavra palavra palavra";
        let cut = truncate_text(words, 20);
        assert!(cut.ends_with('…') && cut.chars().count() <= 21, "{cut}");
        assert!(!cut.contains("palavr…") || cut.starts_with("palavra"));
        let exact = truncate_text("abcde fghij", 5);
        assert_eq!(exact, "abcde…");
    }

    #[test]
    fn lexicon_text_has_paragraphs_without_links_and_is_capped() {
        let nodes = vec![
            UiNode::Text {
                value: "Amor  ".into(),
            },
            UiNode::EntryLink {
                lang: "G",
                num: 26,
                children: vec![UiNode::Text {
                    value: "ágape".into(),
                }],
            },
            UiNode::Paragraph,
            UiNode::Text {
                value: "Segundo".into(),
            },
            UiNode::Note {
                children: vec![UiNode::Text {
                    value: "oculta".into(),
                }],
            },
        ];
        assert_eq!(paragraphs_text(&nodes), "Amor ágape\n\nSegundo");
        let long = "x".repeat(MAX_LEXICON_CHARS * 2);
        let cut = truncate_text(&long, MAX_LEXICON_CHARS);
        assert_eq!(cut.chars().count(), MAX_LEXICON_CHARS + 1);
    }

    #[test]
    fn lexicon_snapshot_from_synthetic_dictionary_is_plain_and_uncut() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("lex.dct.twm");
        gen_fixtures::generate_synthetic_study_dictionary_twm(&path).expect("gerar");
        let lexicon = ModuleInfo {
            id: "lex".into(),
            kind: ModuleKind::Dictionary,
            title: "Léxico".into(),
            ..bible(&path, ModuleState::Ready)
        };
        let cache = StudyCache::new(STUDY_CACHE_CAPACITY);
        let dto = lexicon_snapshot(&cache, std::slice::from_ref(&lexicon), "g0025").expect("G25");
        assert_eq!(
            (dto.strong_id.as_str(), dto.module_id.as_str()),
            ("G25", "lex")
        );
        assert_eq!(dto.title, "synthetic-lemma");
        assert!(!dto.text.is_empty() && !dto.text.ends_with('…'));
        assert!(!dto.text.contains("tw://") && !dto.text.contains('{'));
    }

    #[test]
    fn lexicon_without_installed_module_reports_stable_code() {
        let cache = StudyCache::new(STUDY_CACHE_CAPACITY);
        let error = lexicon_snapshot(&cache, &[], "G25").expect_err("none");
        assert_eq!(error.code, "no_lexicon_installed");
        let error = lexicon_snapshot(&cache, &[], "zzz").expect_err("invalid");
        assert_eq!(error.code, "invalid_strong_id");
    }
}
