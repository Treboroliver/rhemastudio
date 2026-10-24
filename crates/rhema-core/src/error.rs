use thiserror::Error;

use crate::book::Book;
use crate::verse_ref::VerseRef;

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum CanonError {
    #[error("Livro inválido: número {0}")]
    InvalidBookNumber(u8),

    #[error("Livro não encontrado para o termo: \"{0}\"")]
    BookNotFound(String),

    #[error("Capítulo {chapter} inválido para {book:?} (máximo: {max_chapter})")]
    InvalidChapter {
        book: Book,
        chapter: u16,
        max_chapter: u16,
    },

    #[error("Versículo {verse} inválido para {book:?} {chapter} (máximo: {max_verse})")]
    InvalidVerse {
        book: Book,
        chapter: u16,
        verse: u16,
        max_verse: u16,
    },

    #[error("Faixa de versículos inválida: início {start} posterior ao fim {end}")]
    InvalidRange { start: VerseRef, end: VerseRef },
}

#[derive(Debug, Error, Clone)]
pub enum ModuleError {
    #[error("Módulo encriptado ou protegido. O RhemaStudio não remove proteção de arquivos proprietários.")]
    EncryptedModule,

    #[error("Módulo não encontrado: {0}")]
    NotFound(String),

    #[error("Arquivo inválido ou corrompido: {0}")]
    Corrupted(String),

    #[error("Versículo {0} não encontrado no módulo")]
    VerseNotFound(VerseRef),

    #[error("Referência bíblica inválida: {0}")]
    InvalidReference(VerseRef),

    #[error("Livro {0:?} não está presente no módulo (escopo: {1})")]
    BookNotInModule(Book, String),

    #[error("Formato de módulo não suportado: {0}")]
    UnsupportedFormat(String),

    #[error("Formato de conteúdo não suportado: {0}")]
    UnsupportedContent(String),

    #[error("Erro de E/S ao ler módulo: {0}")]
    Io(String),
}
