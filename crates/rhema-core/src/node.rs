use serde::{Deserialize, Serialize};

use crate::verse_range::VerseRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StrongLang {
    Greek,
    Hebrew,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Node {
    Text(String),
    Italic(Vec<Node>),
    WordsOfJesus(Vec<Node>),
    OtQuote(Vec<Node>),
    Heading(u8, Vec<Node>),
    Strong {
        lang: StrongLang,
        num: u32,
        children: Vec<Node>,
    },
    Morph(String),
    Note(Vec<Node>),
    XRef(VerseRange),
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
            | Node::WordsOfJesus(nodes)
            | Node::OtQuote(nodes)
            | Node::Heading(_, nodes)
            | Node::Strong {
                children: nodes, ..
            }
            | Node::UnknownTag {
                children: nodes, ..
            } => {
                for node in nodes {
                    node.write_plain_text(buf);
                }
            }
            Node::Note(_) => {}
            Node::Paragraph | Node::LineBreak | Node::Morph(_) | Node::XRef(_) => {}
        }
    }
}
