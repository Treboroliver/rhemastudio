pub mod book;
pub mod canon;
pub mod error;
pub mod module_info;
pub mod node;
pub mod verse_range;
pub mod verse_ref;

pub use book::{Book, Testament};
pub use canon::{
    NEW_TESTAMENT_VERSES, OLD_TESTAMENT_VERSES, TOTAL_CANONICAL_BOOKS, TOTAL_CANONICAL_CHAPTERS,
    TOTAL_CANONICAL_VERSES,
};
pub use error::CanonError;
pub use module_info::{ModuleInfo, ModuleKind};
pub use node::{Node, StrongLang};
pub use verse_range::VerseRange;
pub use verse_ref::VerseRef;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_totals() {
        let mut total_verses = 0usize;
        let mut ot_verses = 0usize;
        let mut nt_verses = 0usize;
        let mut total_chapters = 0usize;

        for &b in &Book::ALL {
            let ch_count = b.chapter_count() as usize;
            total_chapters += ch_count;

            for ch in 1..=(ch_count as u16) {
                let v = b.verses_in_chapter(ch).expect("Capítulo deve existir");
                total_verses += v as usize;
                if b.testament() == Testament::Old {
                    ot_verses += v as usize;
                } else {
                    nt_verses += v as usize;
                }
            }
        }

        assert_eq!(Book::ALL.len(), TOTAL_CANONICAL_BOOKS);
        assert_eq!(total_chapters, TOTAL_CANONICAL_CHAPTERS);
        assert_eq!(ot_verses, OLD_TESTAMENT_VERSES);
        assert_eq!(nt_verses, NEW_TESTAMENT_VERSES);
        assert_eq!(total_verses, TOTAL_CANONICAL_VERSES);
        assert_eq!(total_verses, 31102);
    }

    #[test]
    fn test_book_find() {
        assert_eq!(Book::find("Gênesis"), Some(Book::Genesis));
        assert_eq!(Book::find("genesis"), Some(Book::Genesis));
        assert_eq!(Book::find("Gn"), Some(Book::Genesis));
        assert_eq!(Book::find("1 Co"), Some(Book::FirstCorinthians));
        assert_eq!(Book::find("1Co"), Some(Book::FirstCorinthians));
        assert_eq!(Book::find("1 Coríntios"), Some(Book::FirstCorinthians));
        assert_eq!(Book::find("Jo"), Some(Book::John));
        assert_eq!(Book::find("João"), Some(Book::John));
        assert_eq!(Book::find("John"), Some(Book::John));
        assert_eq!(Book::find("Sl"), Some(Book::Psalms));
        assert_eq!(Book::find("Salmos"), Some(Book::Psalms));
        assert_eq!(Book::find("Psalm"), Some(Book::Psalms));
        assert_eq!(Book::find("Ap"), Some(Book::Revelation));
        assert_eq!(Book::find("Apocalipse"), Some(Book::Revelation));
        assert_eq!(Book::find("Rev"), Some(Book::Revelation));
        assert_eq!(Book::find("LivroInexistente"), None);
    }

    #[test]
    fn test_verse_ref_validation() {
        // Válido
        let john_3_16 = VerseRef::new(Book::John, 3, 16);
        assert!(john_3_16.is_valid());

        // Gênesis 1:1 e Apocalipse 22:21
        assert!(VerseRef::new(Book::Genesis, 1, 1).is_valid());
        assert!(VerseRef::new(Book::Revelation, 22, 21).is_valid());

        // Capítulo 0
        let inv_ch0 = VerseRef::new(Book::John, 0, 1);
        assert!(!inv_ch0.is_valid());

        // Versículo 0
        let inv_v0 = VerseRef::new(Book::John, 3, 0);
        assert!(!inv_v0.is_valid());

        // Capítulo além do máximo (João tem 21 capítulos)
        let inv_ch = VerseRef::new(Book::John, 22, 1);
        assert!(!inv_ch.is_valid());

        // Versículo além do máximo (João 3 tem 36 versículos)
        let inv_v = VerseRef::new(Book::John, 3, 37);
        assert!(!inv_v.is_valid());
    }

    #[test]
    fn test_verse_cumulative_indexing() {
        let gen_1_1 = VerseRef::new(Book::Genesis, 1, 1);
        assert_eq!(gen_1_1.cumulative_index(), Some(0));
        assert_eq!(VerseRef::from_cumulative_index(0), Some(gen_1_1));

        let rev_22_21 = VerseRef::new(Book::Revelation, 22, 21);
        assert_eq!(rev_22_21.cumulative_index(), Some(31101));
        assert_eq!(VerseRef::from_cumulative_index(31101), Some(rev_22_21));
        assert_eq!(VerseRef::from_cumulative_index(31102), None);

        // Testar navegação
        assert_eq!(
            gen_1_1.next_verse(),
            Some(VerseRef::new(Book::Genesis, 1, 2))
        );
        assert_eq!(gen_1_1.prev_verse(), None);
        assert_eq!(rev_22_21.next_verse(), None);
    }

    #[test]
    fn test_verse_range() {
        let v1 = VerseRef::new(Book::John, 3, 16);
        let v2 = VerseRef::new(Book::John, 3, 18);
        let range = VerseRange::new(v1, v2).expect("Faixa válida");

        assert!(range.contains(&VerseRef::new(Book::John, 3, 16)));
        assert!(range.contains(&VerseRef::new(Book::John, 3, 17)));
        assert!(range.contains(&VerseRef::new(Book::John, 3, 18)));
        assert!(!range.contains(&VerseRef::new(Book::John, 3, 19)));
        assert!(!range.contains(&VerseRef::new(Book::John, 3, 15)));

        // Range invertido deve falhar
        assert!(VerseRange::new(v2, v1).is_err());
    }
}
