use serde::{Deserialize, Serialize};
use std::fmt;

use crate::book::Book;
use crate::canon;
use crate::error::CanonError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VerseRef {
    pub book: Book,
    pub chapter: u16,
    pub verse: u16,
}

impl VerseRef {
    pub const fn new(book: Book, chapter: u16, verse: u16) -> Self {
        Self {
            book,
            chapter,
            verse,
        }
    }

    pub fn validate(&self) -> Result<(), CanonError> {
        let max_ch = self.book.chapter_count();
        if self.chapter == 0 || self.chapter > max_ch {
            return Err(CanonError::InvalidChapter {
                book: self.book,
                chapter: self.chapter,
                max_chapter: max_ch,
            });
        }
        let max_v =
            self.book
                .verses_in_chapter(self.chapter)
                .ok_or(CanonError::InvalidChapter {
                    book: self.book,
                    chapter: self.chapter,
                    max_chapter: max_ch,
                })?;
        if self.verse == 0 || self.verse > max_v {
            return Err(CanonError::InvalidVerse {
                book: self.book,
                chapter: self.chapter,
                verse: self.verse,
                max_verse: max_v,
            });
        }
        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    pub fn cumulative_index(&self) -> Option<usize> {
        canon::cumulative_verse_index(self)
    }

    pub fn from_cumulative_index(index: usize) -> Option<Self> {
        canon::verse_from_cumulative_index(index)
    }

    pub fn next_verse(&self) -> Option<Self> {
        let idx = self.cumulative_index()?;
        Self::from_cumulative_index(idx + 1)
    }

    pub fn prev_verse(&self) -> Option<Self> {
        let idx = self.cumulative_index()?;
        if idx == 0 {
            None
        } else {
            Self::from_cumulative_index(idx - 1)
        }
    }
}

impl fmt::Display for VerseRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}:{}",
            self.book.abbreviations_pt()[0],
            self.chapter,
            self.verse
        )
    }
}
