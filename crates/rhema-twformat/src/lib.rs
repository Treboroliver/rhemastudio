pub mod bible_reader;
pub mod detect;

pub use bible_reader::{BibleScope, OntReader};
pub use detect::{detect_signature, ModuleSignature, SQLITE_MAGIC, TW_ENCRYPTED_MAGIC};

#[cfg(test)]
mod tests {
    use super::*;
    use rhema_core::{Book, ModuleError};
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_synthetic_ont() {
        let temp = NamedTempFile::new().unwrap();
        gen_fixtures::generate_synthetic_ont(temp.path(), 0, true).unwrap();

        let reader = OntReader::open(temp.path()).unwrap();
        assert_eq!(reader.scope(), BibleScope::Full);
        assert!(!reader.is_degraded());
        assert_eq!(reader.verse_count(), 31102);

        // Ler Gênesis 1:1
        let gen1_1 = reader.verse(Book::Genesis, 1, 1).unwrap();
        assert!(gen1_1.contains("Gn 1:1"));
        assert!(gen1_1.contains("<WH3068>"));

        // Ler João 3:16
        let john3_16 = reader.verse(Book::John, 3, 16).unwrap();
        assert!(john3_16.contains("Jo 3:16"));
        assert!(john3_16.contains("<FR>"));

        // Ler Apocalipse 22:21
        let rev22_21 = reader.verse(Book::Revelation, 22, 21).unwrap();
        assert!(rev22_21.contains("Ap 22:21"));

        // Ler capítulo João 3 inteiro (36 versículos)
        let ch = reader.chapter(Book::John, 3).unwrap();
        assert_eq!(ch.len(), 36);
        assert_eq!(ch[15].0, 16); // versículo 16
        assert!(ch[15].1.contains("Jo 3:16"));

        // Verificar propriedades
        let props = reader.properties();
        assert_eq!(
            props.get("title").map(|s| s.as_str()),
            Some("Bíblia Sintética de Teste")
        );
        assert_eq!(props.get("charset").map(|s| s.as_str()), Some("0"));
        assert!(props.contains_key("verse.rule"));
        assert!(reader.info().has_strong);
    }

    #[test]
    fn test_read_synthetic_ot() {
        let temp = NamedTempFile::new().unwrap();
        gen_fixtures::generate_synthetic_ot(temp.path()).unwrap();

        let reader = OntReader::open(temp.path()).unwrap();
        assert_eq!(reader.scope(), BibleScope::OldTestament);
        assert!(!reader.is_degraded());
        assert_eq!(reader.verse_count(), 23145);

        assert!(reader.verse(Book::Genesis, 1, 1).is_ok());
        assert!(reader.verse(Book::Malachi, 4, 6).is_ok());

        // Testar livro do NT no módulo OT: deve retornar BookNotInModule
        let err = reader.verse(Book::Matthew, 1, 1).unwrap_err();
        assert!(matches!(
            err,
            ModuleError::BookNotInModule(Book::Matthew, _)
        ));
    }

    #[test]
    fn test_read_synthetic_nt() {
        let temp = NamedTempFile::new().unwrap();
        gen_fixtures::generate_synthetic_nt(temp.path()).unwrap();

        let reader = OntReader::open(temp.path()).unwrap();
        assert_eq!(reader.scope(), BibleScope::NewTestament);
        assert!(!reader.is_degraded());
        assert_eq!(reader.verse_count(), 7957);

        assert!(reader.verse(Book::Matthew, 1, 1).is_ok());
        assert!(reader.verse(Book::Revelation, 22, 21).is_ok());

        // Testar livro do AT no módulo NT: deve retornar BookNotInModule
        let err = reader.verse(Book::Genesis, 1, 1).unwrap_err();
        assert!(matches!(
            err,
            ModuleError::BookNotInModule(Book::Genesis, _)
        ));
    }

    #[test]
    fn test_read_degraded_module() {
        let temp = NamedTempFile::new().unwrap();
        gen_fixtures::generate_synthetic_degraded(temp.path(), 50).unwrap();

        let reader = OntReader::open(temp.path()).unwrap();
        assert!(reader.is_degraded());
        assert_eq!(reader.verse_count(), 50);

        // Acesso direto por linha funciona
        let line1 = reader.line_by_index(0).unwrap();
        assert!(line1.contains("#1"));

        // Acesso fora do range degradado retorna VerseNotFound
        let err = reader.verse(Book::Revelation, 22, 21).unwrap_err();
        assert!(matches!(err, ModuleError::VerseNotFound(_)));
    }

    #[test]
    fn test_encrypted_module_returns_typed_error() {
        let temp = NamedTempFile::new().unwrap();
        gen_fixtures::generate_synthetic_encrypted(temp.path()).unwrap();

        let res = OntReader::open(temp.path());
        assert!(matches!(res, Err(ModuleError::EncryptedModule)));
    }

    #[test]
    fn test_empty_file_returns_error() {
        let temp = NamedTempFile::new().unwrap();
        let res = OntReader::open(temp.path());
        assert!(matches!(res, Err(ModuleError::Corrupted(_))));
    }

    #[test]
    fn test_invalid_verse_ref_returns_not_found() {
        let temp = NamedTempFile::new().unwrap();
        gen_fixtures::generate_synthetic_ont(temp.path(), 65001, false).unwrap();

        let reader = OntReader::open(temp.path()).unwrap();
        // João 3:37 não existe (capítulo 3 tem 36 versículos)
        let res = reader.verse(Book::John, 3, 37);
        assert!(matches!(res, Err(ModuleError::VerseNotFound(_))));
    }
}
