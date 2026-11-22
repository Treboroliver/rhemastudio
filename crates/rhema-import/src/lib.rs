//! Importador de livros pessoais (`.docx`) em fluxo, com reconhecimento de marcadores,
//! representação intermediária (IR) e relatório de validação.

pub mod docx;
pub mod error;
pub mod ir;
pub mod markers;
pub mod report;
pub mod zip_stream;

pub use docx::{import_docx, validate_docx, ImportOptions, ImportResult};
pub use error::{
    ImportError, MAX_ENTRY_BYTES, MAX_IMAGE_BYTES, MAX_META_PART_BYTES,
    MAX_PART_DECOMPRESSED_BYTES, MAX_TABLE_CELLS,
};
pub use ir::{Block, Entry, EntryKey, ImportKind, InlineLink, KeyType, Span, SpanStyle};
pub use report::{
    truncate_section_title, write_problems_file, ProblemKind, ProblemRecord, StrongGapsReport,
    StrongRangeStats, SuspiciousTypeReport, ValidationReport,
};
