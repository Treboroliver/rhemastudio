use serde::Serialize;

/// Tipo de obra sendo importada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportKind {
    Dict,
    Comm,
    Book,
}

/// Tipo de marcador reconhecido ou desconhecido (preservado).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum KeyType {
    Headword,
    Bible,
    HebrewStrongs,
    GreekStrongs,
    Louw,
    Twot,
    GreekGk,
    HebrewGk,
    Unknown(String),
}

impl KeyType {
    /// Nome canônico para relatórios e agrupamentos.
    pub fn canonical_name(&self) -> &str {
        match self {
            Self::Headword => "Headword",
            Self::Bible => "Bible",
            Self::HebrewStrongs => "HebrewStrongs",
            Self::GreekStrongs => "GreekStrongs",
            Self::Louw => "louw",
            Self::Twot => "TWOT",
            Self::GreekGk => "GreekGK",
            Self::HebrewGk => "HebrewGK",
            Self::Unknown(name) => name.as_str(),
        }
    }

    pub fn is_dict_entry_key(&self) -> bool {
        matches!(
            self,
            Self::Headword | Self::HebrewStrongs | Self::GreekStrongs
        )
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

/// Chave associada a uma entrada ou âncora de parágrafo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntryKey {
    pub kind: KeyType,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
}

/// Estilo inline de um trecho de texto.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct SpanStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub superscript: bool,
}

/// Link inline oriundo de `[[TIPO: VALOR]]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InlineLink {
    pub kind: KeyType,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Trecho inline dentro de um parágrafo ou item de lista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Span {
    Text {
        text: String,
        style: SpanStyle,
        #[serde(skip_serializing_if = "Option::is_none")]
        field: Option<String>,
    },
    Link {
        link: InlineLink,
        style: SpanStyle,
        #[serde(skip_serializing_if = "Option::is_none")]
        field: Option<String>,
    },
}

impl Span {
    pub fn estimated_bytes(&self) -> usize {
        match self {
            Self::Text { text, field, .. } => {
                text.len() + field.as_ref().map_or(0, String::len) + 32
            }
            Self::Link { link, field, .. } => {
                link.value.len()
                    + link.lang.as_ref().map_or(0, String::len)
                    + link.label.as_ref().map_or(0, String::len)
                    + field.as_ref().map_or(0, String::len)
                    + 48
            }
        }
    }

    pub fn char_count(&self) -> usize {
        match self {
            Self::Text { text, .. } => text.chars().count(),
            Self::Link { link, .. } => link
                .label
                .as_deref()
                .unwrap_or(link.value.as_str())
                .chars()
                .count(),
        }
    }
}

/// Bloco estruturado dentro de uma entrada da IR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Block {
    Paragraph {
        #[serde(skip_serializing_if = "Option::is_none")]
        style_id: Option<String>,
        spans: Vec<Span>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        anchors: Vec<EntryKey>,
    },
    ListItem {
        level: u8,
        ordered: bool,
        spans: Vec<Span>,
    },
    Image {
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        content_type: Option<String>,
        size_bytes: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        alt: Option<String>,
    },
    Table {
        rows: Vec<Vec<String>>,
    },
}

impl Block {
    pub fn estimated_bytes(&self) -> usize {
        match self {
            Self::Paragraph {
                style_id,
                spans,
                anchors,
            } => {
                style_id.as_ref().map_or(0, String::len)
                    + spans.iter().map(Span::estimated_bytes).sum::<usize>()
                    + anchors
                        .iter()
                        .map(|k| k.value.len() + k.lang.as_ref().map_or(0, String::len) + 32)
                        .sum::<usize>()
                    + 48
            }
            Self::ListItem { spans, .. } => {
                spans.iter().map(Span::estimated_bytes).sum::<usize>() + 32
            }
            Self::Image {
                target,
                content_type,
                alt,
                ..
            } => {
                target.len()
                    + content_type.as_ref().map_or(0, String::len)
                    + alt.as_ref().map_or(0, String::len)
                    + 48
            }
            Self::Table { rows } => {
                rows.iter()
                    .map(|r| r.iter().map(|c| c.len() + 24).sum::<usize>() + 24)
                    .sum::<usize>()
                    + 32
            }
        }
    }

    pub fn char_count(&self) -> usize {
        match self {
            Self::Paragraph { spans, .. } | Self::ListItem { spans, .. } => {
                spans.iter().map(Span::char_count).sum()
            }
            Self::Image { .. } => 0,
            Self::Table { rows } => rows
                .iter()
                .flat_map(|r| r.iter())
                .map(|c| c.chars().count())
                .sum(),
        }
    }

    pub fn is_effectively_empty(&self) -> bool {
        match self {
            Self::Paragraph { spans, anchors, .. } => {
                anchors.is_empty()
                    && spans.iter().all(|s| match s {
                        Span::Text { text, .. } => text.trim().is_empty(),
                        Span::Link { .. } => false,
                    })
            }
            Self::ListItem { spans, .. } => spans.iter().all(|s| match s {
                Span::Text { text, .. } => text.trim().is_empty(),
                Span::Link { .. } => false,
            }),
            Self::Image { .. } => false,
            Self::Table { rows } => rows.is_empty(),
        }
    }
}

/// Entrada na representação intermediária (IR).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub id: usize,
    pub level: u8,
    pub title_path: Vec<String>,
    pub keys: Vec<EntryKey>,
    pub blocks: Vec<Block>,
    pub char_count: usize,
}

impl Entry {
    pub fn estimated_bytes(&self) -> usize {
        let title_bytes: usize = self.title_path.iter().map(|s| s.len() + 24).sum();
        let key_bytes: usize = self
            .keys
            .iter()
            .map(|k| k.value.len() + k.lang.as_ref().map_or(0, String::len) + 32)
            .sum();
        let block_bytes: usize = self.blocks.iter().map(Block::estimated_bytes).sum();
        title_bytes + key_bytes + block_bytes + 64
    }
}
