use thiserror::Error;

#[derive(Debug, Error)]
pub enum SermonError {
    #[error("O título do sermão não pode ficar vazio.")]
    TitleEmpty,
    #[error("O título passa de {max} caracteres.")]
    TitleTooLong { max: usize },
    #[error("O texto base passa de {max} caracteres.")]
    BaseTextTooLong { max: usize },
    #[error("O sermão tem blocos demais (máximo {max}).")]
    TooManyBlocks { max: usize },
    #[error("O bloco {index} passa de {max} bytes.")]
    BlockTooLarge { index: usize, max: usize },
    #[error("Bloco {index} inválido: {reason}")]
    InvalidBlock { index: usize, reason: &'static str },
    #[error("Há blocos com o mesmo identificador.")]
    DuplicateBlockId,
    #[error("Identificador de sermão inválido.")]
    InvalidId,
    #[error("Sermão ou versão não encontrado.")]
    NotFound,
    #[error("O banco de sermões é de uma versão mais nova do aplicativo (esquema {found}).")]
    UnsupportedSchema { found: i64 },
    #[error("Dados do sermão corrompidos: {0}")]
    Corrupt(String),
    #[error("Erro no banco de sermões: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Erro ao preparar a pasta de dados: {0}")]
    Io(#[from] std::io::Error),
    #[cfg(test)]
    #[error("falha simulada")]
    Injected,
}

impl SermonError {
    /// Código estável para a interface.
    pub fn code(&self) -> &'static str {
        match self {
            Self::TitleEmpty => "sermon_title_empty",
            Self::TitleTooLong { .. } => "sermon_title_too_long",
            Self::BaseTextTooLong { .. } => "sermon_base_text_too_long",
            Self::TooManyBlocks { .. } => "sermon_too_many_blocks",
            Self::BlockTooLarge { .. } => "sermon_block_too_large",
            Self::InvalidBlock { .. } => "sermon_invalid_block",
            Self::DuplicateBlockId => "sermon_duplicate_block_id",
            Self::InvalidId => "sermon_invalid_id",
            Self::NotFound => "sermon_not_found",
            Self::UnsupportedSchema { .. } => "sermon_unsupported_schema",
            Self::Corrupt(_) => "sermon_corrupt",
            Self::Database(_) => "sermon_database",
            Self::Io(_) => "sermon_io",
            #[cfg(test)]
            Self::Injected => "sermon_injected",
        }
    }
}

pub type Result<T> = std::result::Result<T, SermonError>;
