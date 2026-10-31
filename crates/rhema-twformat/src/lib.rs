pub mod bible_reader;
pub mod detect;
pub mod markup;
pub mod protection;
pub mod rtf;
pub mod twm;

pub use bible_reader::{BibleScope, OntReader};
pub use detect::{detect_signature, ModuleSignature, SQLITE_MAGIC, TW_ENCRYPTED_MAGIC};
pub use markup::{plain_text, tokenize_markup};
pub use protection::ensure_twm_unprotected;
pub use rtf::{
    convert_rtf, convert_rtf_page, convert_rtf_with_stats, decode_twm_content,
    decode_twm_content_page, page_cursor, RtfConversion, RtfPage, RtfStats,
};
pub use twm::{
    normalize_strong_key, strong_key, CommentaryEntry, CommentaryHeader, CommentaryScope,
    CommentarySource, DictionaryEntry, DictionarySource, TwmKind, TwmModule,
};

#[cfg(test)]
mod tests {
    use super::*;
    use rhema_core::{Book, ModuleError};
    use std::io::Write;
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
        let john3_16_nodes = reader.verse_nodes(Book::John, 3, 16).unwrap();
        assert!(john3_16_nodes
            .iter()
            .any(|node| matches!(node, rhema_core::Node::WordsOfJesus(_))));
        assert_eq!(
            plain_text(&john3_16_nodes),
            "Texto sintético de Jo 3:16 Porque Deus amou o mundo de tal maneira..."
        );

        // Ler Apocalipse 22:21
        let rev22_21 = reader.verse(Book::Revelation, 22, 21).unwrap();
        assert!(rev22_21.contains("Ap 22:21"));

        // Ler capítulo João 3 inteiro (36 versículos)
        let ch = reader.chapter(Book::John, 3).unwrap();
        assert_eq!(ch.len(), 36);
        assert_eq!(ch[15].0, 16); // versículo 16
        assert!(ch[15].1.contains("Jo 3:16"));
        let ch_nodes = reader.chapter_nodes(Book::John, 3).unwrap();
        assert_eq!(ch_nodes.len(), 36);
        assert!(ch_nodes[15]
            .1
            .iter()
            .any(|node| matches!(node, rhema_core::Node::WordsOfJesus(_))));

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
    fn test_property_continuations_may_begin_with_markup_delimiters() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(
            b"Verse one\n\ncharset=0\ntitle=Synthetic\nabout=first line\\\n<continued metadata>\n",
        )
        .unwrap();
        temp.flush().unwrap();

        let reader = OntReader::open(temp.path()).unwrap();
        assert_eq!(reader.verse_count(), 1);
        assert_eq!(
            reader.properties().get("charset").map(String::as_str),
            Some("0")
        );
        assert_eq!(
            reader.properties().get("about").map(String::as_str),
            Some("first line <continued metadata>")
        );
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

    #[test]
    fn test_unknown_charset_falls_back_to_utf8_then_windows_1252() {
        let mut utf8_file = NamedTempFile::new().unwrap();
        utf8_file
            .write_all("Texto com café\r\n\r\ncharset=9999\r\n".as_bytes())
            .unwrap();
        let utf8_reader = OntReader::open(utf8_file.path()).unwrap();
        assert_eq!(utf8_reader.encoding_name(), "UTF-8");
        assert_eq!(utf8_reader.line_by_index(0).unwrap(), "Texto com café");

        let mut windows_file = NamedTempFile::new().unwrap();
        windows_file
            .write_all(b"Texto com caf\xe9\r\n\r\ncharset=9999\r\n")
            .unwrap();
        let windows_reader = OntReader::open(windows_file.path()).unwrap();
        assert_eq!(windows_reader.encoding_name(), "windows-1252");
        assert_eq!(windows_reader.line_by_index(0).unwrap(), "Texto com café");

        let mut absent_file = NamedTempFile::new().unwrap();
        absent_file
            .write_all("Texto UTF-8 sem propriedade\r\n\r\n".as_bytes())
            .unwrap();
        let absent_reader = OntReader::open(absent_file.path()).unwrap();
        assert_eq!(absent_reader.encoding_name(), "UTF-8");
    }

    #[test]
    fn test_synthetic_windows_1252_accented_verse() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"Jo 3:16: a\xe7\xe3o, caf\xe9, p\xf5e\r\n\r\ncharset=0\r\n")
            .unwrap();

        let reader = OntReader::open(temp.path()).unwrap();

        assert_eq!(reader.encoding_name(), "windows-1252");
        assert_eq!(reader.line_by_index(0).unwrap(), "Jo 3:16: ação, café, põe");
    }
}
