use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::CanonError;
use crate::verse_ref::VerseRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VerseRange {
    pub start: VerseRef,
    pub end: VerseRef,
}

impl VerseRange {
    pub fn new(start: VerseRef, end: VerseRef) -> Result<Self, CanonError> {
        start.validate()?;
        end.validate()?;
        if start > end {
            return Err(CanonError::InvalidRange { start, end });
        }
        Ok(Self { start, end })
    }

    pub fn single(verse: VerseRef) -> Self {
        Self {
            start: verse,
            end: verse,
        }
    }

    pub fn contains(&self, v: &VerseRef) -> bool {
        v >= &self.start && v <= &self.end
    }

    pub fn is_single_verse(&self) -> bool {
        self.start == self.end
    }

    pub fn is_single_chapter(&self) -> bool {
        self.start.book == self.end.book && self.start.chapter == self.end.chapter
    }
}

impl fmt::Display for VerseRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_single_verse() {
            write!(f, "{}", self.start)
        } else if self.start.book == self.end.book {
            if self.start.chapter == self.end.chapter {
                write!(
                    f,
                    "{} {}:{}-{}",
                    self.start.book.abbreviations_pt()[0],
                    self.start.chapter,
                    self.start.verse,
                    self.end.verse
                )
            } else {
                write!(
                    f,
                    "{} {}:{}-{}:{}",
                    self.start.book.abbreviations_pt()[0],
                    self.start.chapter,
                    self.start.verse,
                    self.end.chapter,
                    self.end.verse
                )
            }
        } else {
            write!(f, "{} - {}", self.start, self.end)
        }
    }
}
