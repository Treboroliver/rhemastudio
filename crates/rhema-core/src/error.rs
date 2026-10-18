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
