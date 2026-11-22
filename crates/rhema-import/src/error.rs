use thiserror::Error;

/// Limite máximo de bytes por entrada (5 MB).
pub const MAX_ENTRY_BYTES: usize = 5 * 1024 * 1024;

/// Limite máximo de bytes por imagem no pacote (20 MB).
pub const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

/// Limite máximo de células por tabela.
pub const MAX_TABLE_CELLS: usize = 10_000;

/// Limite padrão de descompressão para partes principais de XML (256 MB).
pub const MAX_PART_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;

/// Limite padrão de descompressão para partes auxiliares (styles, rels: 16 MB).
pub const MAX_META_PART_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("erro de E/S: {0}")]
    Io(#[from] std::io::Error),

    #[error("arquivo .docx inválido ou corrompido: {0}")]
    InvalidArchive(String),

    #[error("limite de descompressão excedido na parte '{part}' (máximo {limit} bytes)")]
    ZipBomb { part: String, limit: u64 },

    #[error("parte obrigatória ausente no pacote .docx: {0}")]
    MissingPart(String),

    #[error("erro de XML em '{part}': {message}")]
    XmlError { part: String, message: String },

    #[error("entrada #{entry_id} excedeu o limite de tamanho ({limit} bytes)")]
    EntryTooLarge { entry_id: usize, limit: usize },

    #[error("imagem '{path}' excedeu o limite de tamanho ({size} > {limit} bytes)")]
    ImageTooLarge { path: String, size: u64, limit: u64 },

    #[error("tabela excedeu o limite de células ({cells} > {limit})")]
    TableTooLarge { cells: usize, limit: usize },
}
