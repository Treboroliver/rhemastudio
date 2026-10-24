use serde::{Deserialize, Serialize};

use crate::verse_range::VerseRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StrongLang {
    Greek,
    Hebrew,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkTarget {
    Bible(VerseRange),
    Dictionary(String),
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Node {
    Text(String),
    Italic(Vec<Node>),
    Bold(Vec<Node>),
    Underline(Vec<Node>),
    Strikethrough(Vec<Node>),
    Caps(Vec<Node>),
    Superscript(Vec<Node>),
    Subscript(Vec<Node>),
    WordsOfJesus(Vec<Node>),
    OtQuote(Vec<Node>),
    Heading(u8, Vec<Node>),
    Strong {
        lang: StrongLang,
        num: u32,
        children: Vec<Node>,
    },
    Grammar {
        lang: StrongLang,
        code: u32,
    },
    Morph(String),
    Note(Vec<Node>),
    XRef(VerseRange),
    Link {
        target: LinkTarget,
        children: Vec<Node>,
    },
    Paragraph,
    LineBreak,
    UnknownTag {
        name: String,
        raw: String,
        children: Vec<Node>,
    },
}

impl Node {
    pub fn to_plain_text(&self) -> String {
        let mut buf = String::new();
        self.write_plain_text(&mut buf);
        buf
    }

    fn write_plain_text(&self, buf: &mut String) {
        match self {
            Node::Text(s) => buf.push_str(s),
            Node::Italic(nodes)
            | Node::Bold(nodes)
            | Node::Underline(nodes)
            | Node::Strikethrough(nodes)
            | Node::Caps(nodes)
            | Node::Superscript(nodes)
            | Node::Subscript(nodes)
            | Node::WordsOfJesus(nodes)
            | Node::OtQuote(nodes)
            | Node::Heading(_, nodes)
            | Node::Strong {
                children: nodes, ..
            }
            | Node::UnknownTag {
                children: nodes, ..
            }
            | Node::Link {
                children: nodes, ..
            } => {
                for node in nodes {
                    node.write_plain_text(buf);
                }
            }
            Node::Note(_) => {}
            Node::Grammar { .. }
            | Node::Paragraph
            | Node::LineBreak
            | Node::Morph(_)
            | Node::XRef(_) => {}
        }
    }
}
