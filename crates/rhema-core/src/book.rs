use serde::{Deserialize, Serialize};

use crate::canon;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Testament {
    Old,
    New,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Book {
    Genesis = 1,
    Exodus = 2,
    Leviticus = 3,
    Numbers = 4,
    Deuteronomy = 5,
    Joshua = 6,
    Judges = 7,
    Ruth = 8,
    FirstSamuel = 9,
    SecondSamuel = 10,
    FirstKings = 11,
    SecondKings = 12,
    FirstChronicles = 13,
    SecondChronicles = 14,
    Ezra = 15,
    Nehemiah = 16,
    Esther = 17,
    Job = 18,
    Psalms = 19,
    Proverbs = 20,
    Ecclesiastes = 21,
    SongOfSongs = 22,
    Isaiah = 23,
    Jeremiah = 24,
    Lamentations = 25,
    Ezekiel = 26,
    Daniel = 27,
    Hosea = 28,
    Joel = 29,
    Amos = 30,
    Obadiah = 31,
    Jonah = 32,
    Micah = 33,
    Nahum = 34,
    Habakkuk = 35,
    Zephaniah = 36,
    Haggai = 37,
    Zechariah = 38,
    Malachi = 39,
    Matthew = 40,
    Mark = 41,
    Luke = 42,
    John = 43,
    Acts = 44,
    Romans = 45,
    FirstCorinthians = 46,
    SecondCorinthians = 47,
    Galatians = 48,
    Ephesians = 49,
    Philippians = 50,
    Colossians = 51,
    FirstThessalonians = 52,
    SecondThessalonians = 53,
    FirstTimothy = 54,
    SecondTimothy = 55,
    Titus = 56,
    Philemon = 57,
    Hebrews = 58,
    James = 59,
    FirstPeter = 60,
    SecondPeter = 61,
    FirstJohn = 62,
    SecondJohn = 63,
    ThirdJohn = 64,
    Jude = 65,
    Revelation = 66,
}

impl Book {
    pub const ALL: [Book; 66] = [
        Book::Genesis,
        Book::Exodus,
        Book::Leviticus,
        Book::Numbers,
        Book::Deuteronomy,
        Book::Joshua,
        Book::Judges,
        Book::Ruth,
        Book::FirstSamuel,
        Book::SecondSamuel,
        Book::FirstKings,
        Book::SecondKings,
        Book::FirstChronicles,
        Book::SecondChronicles,
        Book::Ezra,
        Book::Nehemiah,
        Book::Esther,
        Book::Job,
        Book::Psalms,
        Book::Proverbs,
        Book::Ecclesiastes,
        Book::SongOfSongs,
        Book::Isaiah,
        Book::Jeremiah,
        Book::Lamentations,
        Book::Ezekiel,
        Book::Daniel,
        Book::Hosea,
        Book::Joel,
        Book::Amos,
        Book::Obadiah,
        Book::Jonah,
        Book::Micah,
        Book::Nahum,
        Book::Habakkuk,
        Book::Zephaniah,
        Book::Haggai,
        Book::Zechariah,
        Book::Malachi,
        Book::Matthew,
        Book::Mark,
        Book::Luke,
        Book::John,
        Book::Acts,
        Book::Romans,
        Book::FirstCorinthians,
        Book::SecondCorinthians,
        Book::Galatians,
        Book::Ephesians,
        Book::Philippians,
        Book::Colossians,
        Book::FirstThessalonians,
        Book::SecondThessalonians,
        Book::FirstTimothy,
        Book::SecondTimothy,
        Book::Titus,
        Book::Philemon,
        Book::Hebrews,
        Book::James,
        Book::FirstPeter,
        Book::SecondPeter,
        Book::FirstJohn,
        Book::SecondJohn,
        Book::ThirdJohn,
        Book::Jude,
        Book::Revelation,
    ];

    #[inline]
    pub const fn number(self) -> u8 {
        self as u8
    }

    pub fn from_number(num: u8) -> Option<Self> {
        if (1..=66).contains(&num) {
            Some(Self::ALL[(num - 1) as usize])
        } else {
            None
        }
    }

    #[inline]
    pub const fn testament(self) -> Testament {
        if self.number() <= 39 {
            Testament::Old
        } else {
            Testament::New
        }
    }

    pub const fn osis_code(self) -> &'static str {
        canon::book_meta(self).osis
    }

    pub const fn name_pt(self) -> &'static str {
        canon::book_meta(self).name_pt
    }

    pub const fn name_en(self) -> &'static str {
        canon::book_meta(self).name_en
    }

    pub const fn abbreviations_pt(self) -> &'static [&'static str] {
        canon::book_meta(self).abbr_pt
    }

    pub fn primary_abbreviation_pt(self) -> &'static str {
        self.abbreviations_pt()
            .first()
            .copied()
            .unwrap_or_else(|| self.name_pt())
    }

    pub const fn abbreviations_en(self) -> &'static [&'static str] {
        canon::book_meta(self).abbr_en
    }

    pub fn primary_abbreviation_en(self) -> &'static str {
        self.abbreviations_en()
            .first()
            .copied()
            .unwrap_or_else(|| self.name_en())
    }

    #[inline]
    pub fn chapter_count(self) -> u16 {
        canon::chapter_count(self)
    }

    #[inline]
    pub fn verse_count(self) -> u16 {
        canon::total_verses_in_book(self)
    }

    #[inline]
    pub fn verses_in_chapter(self, chapter: u16) -> Option<u16> {
        canon::verses_in_chapter(self, chapter)
    }

    pub fn find(query: &str) -> Option<Self> {
        let q_trimmed = query.trim();
        let q_lower = q_trimmed.to_lowercase();
        // 1ª passagem: correspondência exata de acentos (case-insensitive)
        for &book in &Self::ALL {
            if book.name_pt().to_lowercase() == q_lower
                || book.name_en().to_lowercase() == q_lower
                || book.osis_code().to_lowercase() == q_lower
            {
                return Some(book);
            }
            for &abbr in book.abbreviations_pt() {
                if abbr.to_lowercase() == q_lower {
                    return Some(book);
                }
            }
            for &abbr in book.abbreviations_en() {
                if abbr.to_lowercase() == q_lower {
                    return Some(book);
                }
            }
        }

        // 2ª passagem: sem acentos e sem pontuação
        let normalized = normalize_str(query);
        if normalized.is_empty() {
            return None;
        }
        for &book in &Self::ALL {
            if normalize_str(book.name_pt()) == normalized
                || normalize_str(book.name_en()) == normalized
                || normalize_str(book.osis_code()) == normalized
            {
                return Some(book);
            }
            for &abbr in book.abbreviations_pt() {
                if normalize_str(abbr) == normalized {
                    return Some(book);
                }
            }
            for &abbr in book.abbreviations_en() {
                if normalize_str(abbr) == normalized {
                    return Some(book);
                }
            }
        }
        None
    }
}

fn normalize_str(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            'á' | 'à' | 'ã' | 'â' | 'ä' => Some('a'),
            'é' | 'è' | 'ê' | 'ë' => Some('e'),
            'í' | 'ì' | 'î' | 'ï' => Some('i'),
            'ó' | 'ò' | 'õ' | 'ô' | 'ö' => Some('o'),
            'ú' | 'ù' | 'û' | 'ü' => Some('u'),
            'ç' => Some('c'),
            '.' | ' ' | '-' | '_' => None,
            other => Some(other),
        })
        .collect()
}
