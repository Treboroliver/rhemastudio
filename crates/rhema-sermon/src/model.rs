use std::collections::HashSet;

use rhema_core::VerseRange;
use serde::{Deserialize, Serialize};

use crate::error::{Result, SermonError};

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_BASE_TEXT_CHARS: usize = 200;
pub const MAX_BLOCK_BYTES: usize = 200 * 1024;
pub const MAX_BLOCKS: usize = 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Heading {
        id: String,
        level: u8,
        text: String,
    },
    Text {
        id: String,
        markdown: String,
    },
    Verse {
        id: String,
        range: VerseRange,
        module_id: String,
        module_abbr: String,
        snapshot: String,
    },
    Lexicon {
        id: String,
        strong_id: String,
        module_id: String,
        title: String,
        snapshot: String,
    },
    Note {
        id: String,
        markdown: String,
    },
    Illustration {
        id: String,
        markdown: String,
    },
}

impl Block {
    pub fn id(&self) -> &str {
        match self {
            Self::Heading { id, .. }
            | Self::Text { id, .. }
            | Self::Verse { id, .. }
            | Self::Lexicon { id, .. }
            | Self::Note { id, .. }
            | Self::Illustration { id, .. } => id,
        }
    }

    pub(crate) fn set_id(&mut self, new_id: String) {
        match self {
            Self::Heading { id, .. }
            | Self::Text { id, .. }
            | Self::Verse { id, .. }
            | Self::Lexicon { id, .. }
            | Self::Note { id, .. }
            | Self::Illustration { id, .. } => *id = new_id,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Heading { .. } => "heading",
            Self::Text { .. } => "text",
            Self::Verse { .. } => "verse",
            Self::Lexicon { .. } => "lexicon",
            Self::Note { .. } => "note",
            Self::Illustration { .. } => "illustration",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sermon {
    pub id: String,
    pub title: String,
    pub base_text: String,
    /// Milissegundos desde 1970 (UTC).
    pub created_at: i64,
    pub updated_at: i64,
    pub archived: bool,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SermonSummary {
    pub id: String,
    pub title: String,
    pub base_text: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived: bool,
    pub block_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionInfo {
    pub id: i64,
    pub sermon_id: String,
    pub created_at: i64,
    pub title: String,
    pub block_count: usize,
}

pub(crate) fn validate_title(title: &str) -> Result<()> {
    if title.trim().is_empty() {
        return Err(SermonError::TitleEmpty);
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(SermonError::TitleTooLong {
            max: MAX_TITLE_CHARS,
        });
    }
    Ok(())
}

pub(crate) fn validate_id(id: &str) -> Result<()> {
    uuid::Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| SermonError::InvalidId)
}

impl Sermon {
    /// Sermão novo e vazio, com identificador UUID v4.
    pub fn new(title: impl Into<String>, base_text: impl Into<String>, now: i64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            base_text: base_text.into(),
            created_at: now,
            updated_at: now,
            archived: false,
            blocks: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        validate_id(&self.id)?;
        validate_title(&self.title)?;
        if self.base_text.chars().count() > MAX_BASE_TEXT_CHARS {
            return Err(SermonError::BaseTextTooLong {
                max: MAX_BASE_TEXT_CHARS,
            });
        }
        if self.blocks.len() > MAX_BLOCKS {
            return Err(SermonError::TooManyBlocks { max: MAX_BLOCKS });
        }
        let mut seen = HashSet::with_capacity(self.blocks.len());
        for (index, block) in self.blocks.iter().enumerate() {
            validate_block(index, block)?;
            if !seen.insert(block.id()) {
                return Err(SermonError::DuplicateBlockId);
            }
        }
        Ok(())
    }
}

fn validate_block(index: usize, block: &Block) -> Result<()> {
    let invalid = |reason| SermonError::InvalidBlock { index, reason };
    if block.id().is_empty() || block.id().len() > 64 {
        return Err(invalid("identificador vazio ou longo demais"));
    }
    match block {
        Block::Heading { level, .. } if !(1..=3).contains(level) => {
            return Err(invalid("nível de título fora de 1 a 3"));
        }
        Block::Verse { range, .. } => {
            VerseRange::new(range.start, range.end)
                .map_err(|_| invalid("referência de versículo inválida"))?;
        }
        Block::Lexicon { strong_id, .. } if strong_id.trim().is_empty() => {
            return Err(invalid("Strong vazio"));
        }
        _ => {}
    }
    let size = serde_json::to_vec(block)
        .map_err(|_| invalid("bloco não serializável"))?
        .len();
    if size > MAX_BLOCK_BYTES {
        return Err(SermonError::BlockTooLarge {
            index,
            max: MAX_BLOCK_BYTES,
        });
    }
    Ok(())
}
