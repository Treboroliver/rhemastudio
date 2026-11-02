//! Modelo, armazenamento SQLite e exportação Markdown de sermões.

mod error;
mod export;
mod model;
mod store;

pub use error::{Result, SermonError};
pub use export::export_markdown;
pub use model::{
    Block, MAX_BASE_TEXT_CHARS, MAX_BLOCK_BYTES, MAX_BLOCKS, MAX_TITLE_CHARS, Sermon,
    SermonSummary, VersionInfo,
};
pub use store::{BACKUP_INTERVAL_MS, LATEST_SCHEMA, MAX_VERSIONS, SermonStore};
