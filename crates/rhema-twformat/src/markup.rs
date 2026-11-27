use rhema_core::{Book, Node, StrongLang, VerseRange, VerseRef};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ContainerKind {
    Italic,
    Bold,
    Superscript,
    Subscript,
    Transparent,
    WordsOfJesus,
    OtQuote,
    Heading(u8),
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FrameKind {
    Container(ContainerKind),
    Unknown,
}

struct Frame {
    kind: FrameKind,
    name: String,
    raw_open: String,
    raw_close: String,
    children: Vec<Node>,
}

enum Token {
    Open {
        kind: FrameKind,
        name: String,
        self_closing: bool,
    },
    Close {
        kind: FrameKind,
        name: String,
    },
    Inline(Node),
    Strong(StrongLang, u32),
    Grammar(StrongLang, u32),
    Unknown {
        name: String,
        self_closing: bool,
    },
    UnknownClose(String),
}

/// Converte marcação de versículo em nós tolerantes, preservando todo o texto.
pub fn tokenize_markup(input: &str) -> Vec<Node> {
    let mut root = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();
    let mut cursor = 0;

    while cursor < input.len() {
        let Some(relative_start) = input[cursor..].find('<') else {
            append_text(&mut root, &mut stack, &input[cursor..]);
            break;
        };
        let start = cursor + relative_start;
        if start > cursor {
            append_text(&mut root, &mut stack, &input[cursor..start]);
        }

        let Some(relative_end) = input[start..].find('>') else {
            append_text(&mut root, &mut stack, &input[start..]);
            break;
        };
        let end = start + relative_end;
        let mut start = start;
        // Um "<" antes de outro "<" sem ">" entre eles não abre tag: é texto literal.
        if let Some(inner) = input[start + 1..end].rfind('<') {
            let tag_start = start + 1 + inner;
            append_text(&mut root, &mut stack, &input[start..tag_start]);
            start = tag_start;
        }
        let raw_tag = &input[start..=end];
        let contents = &input[start + 1..end];

        match parse_token(contents) {
            Token::Open {
                kind,
                name,
                self_closing,
            } => {
                if self_closing {
                    append_node(
                        &mut root,
                        &mut stack,
                        build_node(Frame {
                            kind,
                            name,
                            raw_open: raw_tag.to_string(),
                            raw_close: String::new(),
                            children: Vec::new(),
                        }),
                    );
                } else {
                    stack.push(Frame {
                        kind,
                        name,
                        raw_open: raw_tag.to_string(),
                        raw_close: String::new(),
                        children: Vec::new(),
                    });
                }
            }
            Token::Close { kind, name } => {
                if let Some(index) = stack
                    .iter()
                    .rposition(|frame| frame.kind == kind && frame.name == name)
                {
                    while stack.len() > index + 1 {
                        if let Some(frame) = stack.pop() {
                            append_frame(&mut root, &mut stack, frame);
                        }
                    }
                    if let Some(mut frame) = stack.pop() {
                        frame.raw_close = raw_tag.to_string();
                        append_frame(&mut root, &mut stack, frame);
                    }
                }
            }
            Token::Inline(node) => append_node(&mut root, &mut stack, node),
            Token::Strong(lang, num) => {
                if !attach_strong(&mut root, &mut stack, lang, num) {
                    append_node(
                        &mut root,
                        &mut stack,
                        Node::Strong {
                            lang,
                            num,
                            children: Vec::new(),
                        },
                    );
                }
            }
            Token::Grammar(lang, code) => {
                if !attach_grammar(&mut root, &mut stack, lang, code) {
                    append_node(
                        &mut root,
                        &mut stack,
                        Node::UnknownTag {
                            name: match lang {
                                StrongLang::Greek => format!("G{code}"),
                                StrongLang::Hebrew => format!("H{code}"),
                            },
                            raw: raw_tag.to_string(),
                            children: Vec::new(),
                        },
                    );
                }
            }
            Token::Unknown { name, self_closing } => {
                let kind = FrameKind::Unknown;
                if self_closing {
                    append_node(
                        &mut root,
                        &mut stack,
                        Node::UnknownTag {
                            name,
                            raw: raw_tag.to_string(),
                            children: Vec::new(),
                        },
                    );
                } else {
                    stack.push(Frame {
                        kind,
                        name,
                        raw_open: raw_tag.to_string(),
                        raw_close: String::new(),
                        children: Vec::new(),
                    });
                }
            }
            Token::UnknownClose(name) => {
                if let Some(index) = stack
                    .iter()
                    .rposition(|frame| frame.kind == FrameKind::Unknown && frame.name == name)
                {
                    while stack.len() > index + 1 {
                        if let Some(frame) = stack.pop() {
                            append_frame(&mut root, &mut stack, frame);
                        }
                    }
                    if let Some(mut frame) = stack.pop() {
                        frame.raw_close = raw_tag.to_string();
                        append_node(&mut root, &mut stack, build_node(frame));
                    }
                }
            }
        }
        cursor = end + 1;
    }

    while let Some(frame) = stack.pop() {
        append_frame(&mut root, &mut stack, frame);
    }
    root
}

/// Concatena texto para busca, sem notas, metadados ou marcadores estruturais.
pub fn plain_text(nodes: &[Node]) -> String {
    nodes.iter().map(Node::to_plain_text).collect()
}

fn parse_token(contents: &str) -> Token {
    if let Some((kind, name)) = parse_container_name(contents, true) {
        return Token::Close { kind, name };
    }
    if let Some(closing) = contents.strip_prefix('/') {
        let html_close = match closing {
            "i" => Some((ContainerKind::Italic, "i")),
            "b" => Some((ContainerKind::Bold, "b")),
            "sup" => Some((ContainerKind::Superscript, "sup")),
            "sub" => Some((ContainerKind::Subscript, "sub")),
            c if c.eq_ignore_ascii_case("font") => Some((ContainerKind::Transparent, "font")),
            _ => None,
        };
        if let Some((kind, name)) = html_close {
            return Token::Close {
                kind: FrameKind::Container(kind),
                name: name.to_string(),
            };
        }
        return match parse_container_name(closing, true) {
            Some((kind, name)) => Token::Close { kind, name },
            None => Token::UnknownClose(closing.to_string()),
        };
    }

    let trimmed = contents.trim_end();
    let (tag, self_closing) = if let Some(tag) = trimmed.strip_suffix('/') {
        (tag.trim_end(), true)
    } else {
        (contents, false)
    };

    if let Some(kind) = parse_inline(tag) {
        return Token::Inline(kind);
    }
    if let Some(strong) = parse_strong(tag) {
        return Token::Strong(strong.0, strong.1);
    }
    if let Some(grammar) = parse_grammar(tag) {
        return Token::Grammar(grammar.0, grammar.1);
    }
    if let Some(payload) = tag.strip_prefix("WT") {
        let code = payload.split_whitespace().next().unwrap_or_default().trim();
        if !code.is_empty() {
            return Token::Inline(Node::Morph(code.to_string()));
        }
    }
    if let Some(payload) = tag.strip_prefix("RX") {
        if let Some(range) = parse_xref(payload) {
            return Token::Inline(Node::XRef(range));
        }
        return Token::Unknown {
            name: tag.to_string(),
            self_closing: true,
        };
    }
    if let Some((kind, name)) = parse_container_name(tag, false) {
        return Token::Open {
            kind,
            name,
            self_closing,
        };
    }
    if tag.is_empty() {
        return Token::Unknown {
            name: tag.to_string(),
            self_closing: true,
        };
    }
    Token::Unknown {
        name: tag.to_string(),
        self_closing,
    }
}

fn parse_inline(tag: &str) -> Option<Node> {
    match tag {
        "CM" => Some(Node::Paragraph),
        "CL" => Some(Node::LineBreak),
        _ => None,
    }
}

fn parse_strong(tag: &str) -> Option<(StrongLang, u32)> {
    let (prefix, lang) = if tag.starts_with("XWG") || tag.starts_with("WG") {
        (
            if tag.starts_with("XWG") { "XWG" } else { "WG" },
            StrongLang::Greek,
        )
    } else if tag.starts_with("WH") {
        ("WH", StrongLang::Hebrew)
    } else {
        return None;
    };
    let payload = tag.strip_prefix(prefix)?;
    let digits = payload.strip_suffix('x').unwrap_or(payload);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().map(|num| (lang, num))
}

fn parse_grammar(tag: &str) -> Option<(StrongLang, u32)> {
    let (prefix, lang) = match tag.as_bytes().first()? {
        b'G' => ("G", StrongLang::Greek),
        b'H' => ("H", StrongLang::Hebrew),
        _ => return None,
    };
    let digits = tag.strip_prefix(prefix)?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().map(|code| (lang, code))
}

fn parse_container_name(tag: &str, closing: bool) -> Option<(FrameKind, String)> {
    let (kind, name) = match tag {
        "FI" if !closing => (ContainerKind::Italic, "FI".to_string()),
        "Fi" if closing => (ContainerKind::Italic, "FI".to_string()),
        "FR" if !closing => (ContainerKind::WordsOfJesus, "FR".to_string()),
        "Fr" if closing => (ContainerKind::WordsOfJesus, "FR".to_string()),
        "FO" if !closing => (ContainerKind::OtQuote, "FO".to_string()),
        "Fo" if closing => (ContainerKind::OtQuote, "FO".to_string()),
        "RF" if !closing => (ContainerKind::Note, "RF".to_string()),
        "Rf" if closing => (ContainerKind::Note, "RF".to_string()),
        "i" if !closing => (ContainerKind::Italic, "i".to_string()),
        "b" if !closing => (ContainerKind::Bold, "b".to_string()),
        "sup" if !closing => (ContainerKind::Superscript, "sup".to_string()),
        "sub" if !closing => (ContainerKind::Subscript, "sub".to_string()),
        t if !closing && is_html_font_open(t) => (ContainerKind::Transparent, "font".to_string()),
        _ => {
            let level = tag.strip_prefix(if closing { "Ts" } else { "TS" })?;
            let level = if level.is_empty() {
                1
            } else if level.bytes().all(|byte| byte.is_ascii_digit()) {
                level.parse::<u8>().ok()?
            } else {
                return None;
            };
            if level == 0 {
                return None;
            }
            (ContainerKind::Heading(level), format!("TS{level}"))
        }
    };
    Some((FrameKind::Container(kind), name))
}

fn parse_xref(payload: &str) -> Option<VerseRange> {
    let payload = payload.trim();
    let separator = payload.rfind(char::is_whitespace)?;
    let book = Book::find(payload[..separator].trim())?;
    let location = payload[separator..].trim();
    let (start, end) = match location.split_once('-') {
        Some((start, end)) => {
            let start = parse_xref_ref(book, start)?;
            let end = if end.contains(':') {
                parse_xref_ref(book, end)?
            } else {
                VerseRef::new(book, start.chapter, end.parse().ok()?)
            };
            (start, end)
        }
        None => {
            let verse = parse_xref_ref(book, location)?;
            (verse, verse)
        }
    };
    VerseRange::new(start, end).ok()
}

fn parse_xref_ref(book: Book, location: &str) -> Option<VerseRef> {
    let (chapter, verse) = location.split_once(':')?;
    Some(VerseRef::new(
        book,
        chapter.parse().ok()?,
        verse.parse().ok()?,
    ))
}

fn append_text(root: &mut Vec<Node>, stack: &mut [Frame], text: &str) {
    if text.is_empty() {
        return;
    }
    let nodes = stack
        .last_mut()
        .map(|frame| &mut frame.children)
        .unwrap_or(root);
    if let Some(Node::Text(previous)) = nodes.last_mut() {
        previous.push_str(text);
    } else {
        nodes.push(Node::Text(text.to_string()));
    }
}

fn append_node(root: &mut Vec<Node>, stack: &mut [Frame], node: Node) {
    stack
        .last_mut()
        .map(|frame| &mut frame.children)
        .unwrap_or(root)
        .push(node);
}

fn is_html_font_open(tag: &str) -> bool {
    let bytes = tag.trim_start().as_bytes();
    bytes.len() >= 4
        && bytes[..4].eq_ignore_ascii_case(b"font")
        && (bytes.len() == 4 || bytes[4].is_ascii_whitespace())
}

fn append_frame(root: &mut Vec<Node>, stack: &mut [Frame], frame: Frame) {
    if frame.kind == FrameKind::Container(ContainerKind::Transparent) {
        for child in frame.children {
            match child {
                Node::Text(text) => append_text(root, stack, &text),
                other => append_node(root, stack, other),
            }
        }
    } else {
        append_node(root, stack, build_node(frame));
    }
}

fn build_node(frame: Frame) -> Node {
    match frame.kind {
        FrameKind::Container(ContainerKind::Italic) => Node::Italic(frame.children),
        FrameKind::Container(ContainerKind::Bold) => Node::Bold(frame.children),
        FrameKind::Container(ContainerKind::Superscript) => Node::Superscript(frame.children),
        FrameKind::Container(ContainerKind::Subscript) => Node::Subscript(frame.children),
        FrameKind::Container(ContainerKind::Transparent) => Node::Text(String::new()),
        FrameKind::Container(ContainerKind::WordsOfJesus) => Node::WordsOfJesus(frame.children),
        FrameKind::Container(ContainerKind::OtQuote) => Node::OtQuote(frame.children),
        FrameKind::Container(ContainerKind::Heading(level)) => Node::Heading(level, frame.children),
        FrameKind::Container(ContainerKind::Note) => Node::Note(frame.children),
        FrameKind::Unknown => Node::UnknownTag {
            name: frame.name,
            raw: format!("{}{}", frame.raw_open, frame.raw_close),
            children: frame.children,
        },
    }
}

fn attach_strong(root: &mut Vec<Node>, stack: &mut [Frame], lang: StrongLang, num: u32) -> bool {
    for frame in stack.iter_mut().rev() {
        if attach_strong_to_nodes(&mut frame.children, lang, num) {
            return true;
        }
    }
    attach_strong_to_nodes(root, lang, num)
}

fn attach_strong_to_nodes(nodes: &mut Vec<Node>, lang: StrongLang, num: u32) -> bool {
    for index in (0..nodes.len()).rev() {
        match &mut nodes[index] {
            Node::Text(text) => {
                let Some((start, end)) = trailing_word_range(text) else {
                    continue;
                };
                let word = text[start..end].to_string();
                let remainder = format!("{}{}", &text[..start], &text[end..]);
                let mut replacement = Vec::with_capacity(3);
                if !remainder.is_empty() {
                    replacement.push(Node::Text(remainder));
                }
                replacement.push(Node::Strong {
                    lang,
                    num,
                    children: vec![Node::Text(word)],
                });
                nodes.splice(index..=index, replacement);
                return true;
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
            | Node::UnknownTag { children, .. }
            | Node::Link { children, .. } => {
                if attach_strong_to_nodes(children, lang, num) {
                    return true;
                }
            }
            Node::Grammar { .. }
            | Node::Morph(_)
            | Node::XRef(_)
            | Node::Paragraph
            | Node::LineBreak => {}
        }
    }
    false
}

fn attach_grammar(root: &mut [Node], stack: &mut [Frame], lang: StrongLang, code: u32) -> bool {
    for frame in stack.iter_mut().rev() {
        if attach_grammar_to_nodes(&mut frame.children, lang, code) {
            return true;
        }
    }
    attach_grammar_to_nodes(root, lang, code)
}

fn attach_grammar_to_nodes(nodes: &mut [Node], lang: StrongLang, code: u32) -> bool {
    let Some(last) = nodes.last_mut() else {
        return false;
    };
    match last {
        Node::Strong {
            lang: strong_lang,
            children,
            ..
        } if *strong_lang == lang => {
            children.push(Node::Grammar { lang, code });
            true
        }
        Node::Strong { children, .. } => attach_grammar_to_nodes(children, lang, code),
        _ => false,
    }
}

fn trailing_word_range(text: &str) -> Option<(usize, usize)> {
    let (end, _) = text
        .char_indices()
        .rev()
        .find(|(_, character)| !character.is_whitespace())?;
    let end = end + text[end..].chars().next()?.len_utf8();
    let start = text[..end]
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map(|(index, character)| index + character.len_utf8())
        .unwrap_or(0);
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn visible_text(nodes: &[Node]) -> String {
        let mut output = String::new();
        for node in nodes {
            match node {
                Node::Text(text) => output.push_str(text),
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
                | Node::UnknownTag { children, .. }
                | Node::Link { children, .. } => output.push_str(&visible_text(children)),
                Node::Grammar { .. }
                | Node::Paragraph
                | Node::LineBreak
                | Node::Morph(_)
                | Node::XRef(_) => {}
            }
        }
        output
    }

    fn count_text_nodes(nodes: &[Node]) -> usize {
        nodes
            .iter()
            .map(|node| match node {
                Node::Text(_) => 1,
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
                | Node::UnknownTag { children, .. }
                | Node::Link { children, .. } => count_text_nodes(children),
                _ => 0,
            })
            .sum()
    }

    #[test]
    fn parses_confirmed_container_and_inline_tags() {
        let nodes = tokenize_markup(
            "<FR>Jesus<Fr> <FI>added<Fi> <FO>quoted<Fo><RF>note<Rf><CM><CL><i>word</i>",
        );
        assert!(matches!(nodes[0], Node::WordsOfJesus(_)));
        assert!(matches!(nodes[1], Node::Text(ref text) if text == " "));
        assert!(matches!(nodes[2], Node::Italic(_)));
        assert!(matches!(nodes[4], Node::OtQuote(_)));
        assert!(matches!(nodes[5], Node::Note(_)));
        assert!(nodes.iter().any(|node| matches!(node, Node::Paragraph)));
        assert!(nodes.iter().any(|node| matches!(node, Node::LineBreak)));
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Node::Italic(_)))
                .count(),
            2
        );
        assert_eq!(plain_text(&nodes), "Jesus added quotedword");
        assert_eq!(visible_text(&nodes), "Jesus added quotednoteword");
    }

    #[test]
    fn parses_strong_tags_and_attaches_them_to_the_previous_word() {
        let nodes = tokenize_markup("<FR>loved<Fr><WG25><WH1><XWG2532>");
        assert!(matches!(
            &nodes[0],
            Node::WordsOfJesus(children)
                if matches!(
                    &children[0],
                    Node::Strong {
                        lang: StrongLang::Greek,
                        num: 25,
                        children: first,
                    } if matches!(
                        &first[0],
                        Node::Strong {
                            lang: StrongLang::Hebrew,
                            num: 1,
                            children: second,
                        } if matches!(
                            &second[0],
                            Node::Strong {
                                lang: StrongLang::Greek,
                                num: 2532,
                                children: third,
                            } if matches!(&third[0], Node::Text(word) if word == "loved")
                        )
                    )
                )
        ));
        assert_eq!(plain_text(&nodes), "loved");
        assert!(matches!(
            tokenize_markup("word<WG25x>").as_slice(),
            [Node::Strong {
                lang: StrongLang::Greek,
                num: 25,
                ..
            }]
        ));
    }

    #[test]
    fn parses_grammar_tags_as_non_text_children_of_the_previous_strong() {
        let nodes = tokenize_markup("word<WG25><G5656>");
        assert!(matches!(
            nodes.as_slice(),
            [Node::Strong {
                lang: StrongLang::Greek,
                num: 25,
                children,
            }] if matches!(
                children.as_slice(),
                [Node::Text(word), Node::Grammar {
                    lang: StrongLang::Greek,
                    code: 5656,
                }] if word == "word"
            )
        ));
        assert_eq!(plain_text(&nodes), "word");
        assert!(!nodes.iter().any(|node| matches!(node, Node::Link { .. })));
    }

    #[test]
    fn unmatched_or_mismatched_grammar_tags_remain_unknown() {
        let unmatched = tokenize_markup("word<G5656>");
        assert!(matches!(
            unmatched.as_slice(),
            [Node::Text(_), Node::UnknownTag { name, raw, .. }]
                if name == "G5656" && raw == "<G5656>"
        ));

        let mismatched = tokenize_markup("word<WH1><G5656>");
        assert!(matches!(
            mismatched.last(),
            Some(Node::UnknownTag { name, .. }) if name == "G5656"
        ));
    }

    #[test]
    fn last_word_is_selected_for_strong_when_text_has_multiple_words() {
        let nodes = tokenize_markup("one, two words<WG7>");
        assert!(matches!(
            &nodes[1],
            Node::Strong { children, .. }
                if matches!(&children[0], Node::Text(word) if word == "words")
        ));
        assert_eq!(plain_text(&nodes), "one, two words");
    }

    #[test]
    fn parses_unconfirmed_tags_as_documented() {
        let nodes = tokenize_markup("<TS2>title<Ts2><WTV-PAI><RXJo 3:16-18>");
        assert!(matches!(&nodes[0], Node::Heading(2, _)));
        assert!(matches!(&nodes[1], Node::Morph(code) if code == "V-PAI"));
        assert!(matches!(
            nodes[2],
            Node::XRef(VerseRange {
                start: VerseRef {
                    book: Book::John,
                    chapter: 3,
                    verse: 16,
                },
                end: VerseRef {
                    book: Book::John,
                    chapter: 3,
                    verse: 18,
                }
            })
        ));
        assert_eq!(plain_text(&nodes), "title");
    }

    #[test]
    fn unknown_tags_preserve_name_raw_and_inner_text() {
        let nodes = tokenize_markup("before<custom data>inside</custom data>after");
        assert_eq!(visible_text(&nodes), "beforeinsideafter");
        assert!(matches!(
            &nodes[1],
            Node::UnknownTag {
                name,
                raw,
                children,
            } if name == "custom data"
                && raw == "<custom data></custom data>"
                && matches!(&children[0], Node::Text(text) if text == "inside")
        ));
    }

    #[test]
    fn handles_nested_unbalanced_and_unmatched_tags() {
        let nested = tokenize_markup("<FI>a<FR>b<Fr>c<Fi>");
        assert_eq!(visible_text(&nested), "abc");
        assert!(
            matches!(&nested[0], Node::Italic(children) if matches!(&children[1], Node::WordsOfJesus(_)))
        );

        let unbalanced = tokenize_markup("<FI>open");
        assert!(matches!(&unbalanced[0], Node::Italic(_)));
        assert_eq!(visible_text(&unbalanced), "open");

        let unmatched = tokenize_markup("a</Fi>b</unknown>");
        assert_eq!(visible_text(&unmatched), "ab");
        assert_eq!(count_text_nodes(&unmatched), 1);
    }

    #[test]
    fn empty_and_loose_angle_bracket_are_safe() {
        assert!(tokenize_markup("").is_empty());
        assert_eq!(tokenize_markup("<"), vec![Node::Text("<".to_string())]);
        assert_eq!(plain_text(&tokenize_markup("a < b")), "a < b");
    }

    fn has_unknown(nodes: &[Node]) -> bool {
        nodes.iter().any(|node| match node {
            Node::UnknownTag { .. } => true,
            Node::Italic(children)
            | Node::WordsOfJesus(children)
            | Node::Strong { children, .. } => has_unknown(children),
            _ => false,
        })
    }

    #[test]
    fn loose_angle_brackets_before_tags_stay_literal_text() {
        // Padrões da ARA+: "<[palavra]<WGn>", "<[palavra] [palavra]<WGn>", ">" inicial e "<" final.
        for (input, text) in [
            ("<word<WG25>", "<word"),
            ("<word one<WG25> two", "<word one two"),
            ("one, <word<WG25><G5656> two", "one, <word two"),
            ("><word<WG25> end<", "><word end<"),
            ("a > b<WG25>", "a > b"),
            ("<<WG25>", "<"),
            ("<<<CM>x", "<<x"),
        ] {
            let nodes = tokenize_markup(input);
            assert_eq!(visible_text(&nodes), text, "{input}");
            assert!(!has_unknown(&nodes), "{input}");
        }
        let nodes = tokenize_markup("<word<WG25>");
        assert!(matches!(
            nodes.as_slice(),
            [Node::Text(_), Node::Strong { num: 25, .. }] | [Node::Strong { num: 25, .. }]
        ));
    }

    #[test]
    fn synthetic_markup_snapshots() {
        insta::assert_debug_snapshot!(
            "synthetic_markup_strong_in_jesus_words",
            tokenize_markup("<FR>loved<Fr><WG25>")
        );
        insta::assert_debug_snapshot!(
            "synthetic_markup_strong_grammar",
            tokenize_markup("loved<WG25><G5656>")
        );
        insta::assert_debug_snapshot!(
            "synthetic_markup_nested_unknown_and_note",
            tokenize_markup("<FI>grace <custom>kept</custom><Fi><RF>synthetic note<Rf>")
        );
    }

    #[test]
    fn maps_html_b_sup_and_sub_tags_including_inside_notes() {
        let nodes = tokenize_markup(
            "<sup><b>12b</b></sup> texto H<sub>2</sub>O<RF>nota <b><i>12</i></b> <font size=\"-1\">extra</font><Rf>",
        );
        assert_eq!(
            nodes,
            vec![
                Node::Superscript(vec![Node::Bold(vec![Node::Text("12b".into())])]),
                Node::Text(" texto H".into()),
                Node::Subscript(vec![Node::Text("2".into())]),
                Node::Text("O".into()),
                Node::Note(vec![
                    Node::Text("nota ".into()),
                    Node::Bold(vec![Node::Italic(vec![Node::Text("12".into())])]),
                    Node::Text(" extra".into()),
                ]),
            ]
        );
        assert_eq!(plain_text(&nodes), "12b texto H2O");
    }

    proptest! {
        #[test]
        fn arbitrary_markup_never_panics_and_preserves_text(input in any::<String>()) {
            let nodes = tokenize_markup(&input);
            let mut expected = String::new();
            let mut cursor = 0;
            while cursor < input.len() {
                let Some(start_offset) = input[cursor..].find('<') else {
                    expected.push_str(&input[cursor..]);
                    break;
                };
                let start = cursor + start_offset;
                expected.push_str(&input[cursor..start]);
                let Some(end_offset) = input[start..].find('>') else {
                    expected.push_str(&input[start..]);
                    break;
                };
                let end = start + end_offset;
                if let Some(inner) = input[start + 1..end].rfind('<') {
                    expected.push_str(&input[start..start + 1 + inner]);
                }
                cursor = end + 1;
            }
            prop_assert_eq!(visible_text(&nodes), expected);
        }
    }
}
