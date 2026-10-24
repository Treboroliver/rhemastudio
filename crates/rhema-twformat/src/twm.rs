use std::collections::HashMap;
use std::path::Path;

use rhema_core::{Book, ModuleError, Node, StrongLang, VerseRef};
use rusqlite::{types::Value, Connection, OpenFlags, OptionalExtension, Row};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwmKind {
    Commentary,
    Dictionary,
}

pub struct TwmModule {
    connection: Connection,
    config: HashMap<String, String>,
    kind: TwmKind,
}

impl TwmModule {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ModuleError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ModuleError::NotFound(path.display().to_string()));
        }
        crate::ensure_twm_unprotected(path).map_err(|error| match error {
            ModuleError::EncryptedModule => error,
            other => corrupted(format!("verificação de proteção: {other}")),
        })?;
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(corrupted)?;
        connection
            .execute_batch("PRAGMA query_only = ON;")
            .map_err(|error| corrupted(format!("PRAGMA query_only: {error}")))?;
        let config_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='config')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| corrupted(format!("consulta de tabela config: {error}")))?;
        if !config_exists {
            return Err(ModuleError::Corrupted(
                "Tabela config ausente no módulo .twm".to_string(),
            ));
        }

        let mut config = HashMap::new();
        let mut statement = connection
            .prepare("SELECT name, value FROM config")
            .map_err(|error| corrupted(format!("leitura de configuração: {error}")))?;
        let mut rows = statement
            .query([])
            .map_err(|error| corrupted(format!("consulta config: {error}")))?;
        while let Some(row) = rows
            .next()
            .map_err(|error| corrupted(format!("leitura config: {error}")))?
        {
            let name: Option<String> = row
                .get(0)
                .map_err(|error| corrupted(format!("coluna config.name: {error}")))?;
            let value: Option<String> = row
                .get(1)
                .map_err(|error| corrupted(format!("coluna config.value: {error}")))?;
            if let (Some(name), Some(value)) = (name, value) {
                config.insert(name.trim().to_ascii_lowercase(), value);
            }
        }
        drop(rows);
        drop(statement);

        let kind = module_kind(&config, path)?;
        Ok(Self {
            connection,
            config,
            kind,
        })
    }

    pub fn config(&self) -> &HashMap<String, String> {
        &self.config
    }

    pub fn kind(&self) -> TwmKind {
        self.kind
    }

    fn content_type(&self, row_type: Option<&str>) -> String {
        let value = match row_type {
            Some(value) => value.trim(),
            None => self
                .config
                .get("content.type")
                .map(String::as_str)
                .unwrap_or_default()
                .trim(),
        };
        if value.is_empty() {
            "unknown".to_string()
        } else {
            value.to_ascii_lowercase()
        }
    }

    fn decode_content(
        &self,
        topic_id: i64,
        row_type: Option<&str>,
    ) -> Result<Vec<Node>, ModuleError> {
        let content_type = self.content_type(row_type);
        if content_type != "rtf" {
            return Err(ModuleError::UnsupportedContent(content_type));
        }
        let data: Value = self
            .connection
            .query_row(
                "SELECT data FROM content WHERE topic_id = ?1",
                [topic_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(corrupted)?
            .ok_or_else(|| {
                ModuleError::Corrupted(format!("Conteúdo ausente para id {topic_id}"))
            })?;
        let bytes = match data {
            Value::Text(text) => text.into_bytes(),
            Value::Blob(bytes) => bytes,
            Value::Null => {
                return Err(ModuleError::Corrupted(format!(
                    "Conteúdo NULL para id {topic_id}"
                )))
            }
            _ => {
                return Err(ModuleError::Corrupted(format!(
                    "Tipo SQLite de conteúdo inválido para id {topic_id}"
                )))
            }
        };
        crate::decode_twm_content(&content_type, &bytes)
    }

    fn validate_table(&self, table: &str, required: &[&str]) -> Result<(), ModuleError> {
        if !matches!(
            table,
            "bible_refs" | "topics" | "content" | "topics_wordindex"
        ) {
            return Err(ModuleError::Corrupted(format!(
                "Tabela não suportada: {table}"
            )));
        }
        let mut statement = self
            .connection
            .prepare("SELECT name FROM pragma_table_info(?1)")
            .map_err(|error| corrupted(format!("schema de {table}: {error}")))?;
        let columns = statement
            .query_map([table], |row| row.get::<_, String>(0))
            .map_err(|error| corrupted(format!("consulta de schema {table}: {error}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| corrupted(format!("leitura de schema {table}: {error}")))?;
        if columns.is_empty() {
            return Err(ModuleError::Corrupted(format!("Tabela {table} ausente")));
        }
        for column in required {
            if !columns
                .iter()
                .any(|found| found.eq_ignore_ascii_case(column))
            {
                return Err(ModuleError::Corrupted(format!(
                    "Coluna {column} ausente em {table}"
                )));
            }
        }
        Ok(())
    }

    fn table_exists(&self, table: &str) -> Result<bool, ModuleError> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                [table],
                |row| row.get(0),
            )
            .map_err(corrupted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentaryScope {
    Verse,
    Chapter,
    Book,
}

#[derive(Debug)]
pub struct CommentaryEntry {
    pub topic_id: i64,
    pub bi: u8,
    pub ci: u16,
    pub fvi: u16,
    pub tvi: u16,
    pub scope: CommentaryScope,
    pub content: Result<Vec<Node>, ModuleError>,
}

#[derive(Debug, Clone)]
struct CommentaryRef {
    topic_id: i64,
    book: Book,
    chapter: u16,
    from_verse: u16,
    to_verse: u16,
    content_type: Option<String>,
}

pub struct CommentarySource {
    module: TwmModule,
    refs: Vec<CommentaryRef>,
    by_book: HashMap<Book, Vec<usize>>,
}

impl CommentarySource {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ModuleError> {
        let module = TwmModule::open(path)?;
        if module.kind != TwmKind::Commentary {
            return Err(ModuleError::UnsupportedFormat(
                "O módulo não é um comentário".to_string(),
            ));
        }
        module.validate_table(
            "bible_refs",
            &["topic_id", "bi", "ci", "fvi", "tvi", "content_type"],
        )?;
        module.validate_table("content", &["topic_id", "data"])?;
        let mut statement = module
            .connection
            .prepare(
                "SELECT topic_id, bi, ci, fvi, tvi, content_type FROM bible_refs \
                 ORDER BY bi, ci, fvi, tvi, topic_id",
            )
            .map_err(|error| corrupted(format!("consulta de bible_refs: {error}")))?;
        let mut rows = statement
            .query_map([], read_commentary_ref)
            .map_err(|error| corrupted(format!("leitura de bible_refs: {error}")))?;
        let mut refs = Vec::new();
        for row in &mut rows {
            match row.map_err(|error| corrupted(format!("linha de bible_refs: {error}")))? {
                Some(reference) => refs.push(reference),
                None => tracing::warn!("Ignorando faixa bíblica inconsistente em bible_refs"),
            }
        }
        drop(rows);
        drop(statement);
        let mut by_book = HashMap::<Book, Vec<usize>>::new();
        for (index, reference) in refs.iter().enumerate() {
            by_book.entry(reference.book).or_default().push(index);
        }
        Ok(Self {
            module,
            refs,
            by_book,
        })
    }

    pub fn entries_for(&self, verse: VerseRef) -> Result<Vec<CommentaryEntry>, ModuleError> {
        if verse.validate().is_err() {
            return Err(ModuleError::InvalidReference(verse));
        }
        let mut matching = self
            .by_book
            .get(&verse.book)
            .into_iter()
            .flatten()
            .filter_map(|index| self.refs.get(*index))
            .filter(|entry| reference_contains(entry, verse))
            .collect::<Vec<_>>();
        matching.sort_by_key(|entry| specificity_key(entry));
        Ok(matching
            .into_iter()
            .map(|entry| CommentaryEntry {
                topic_id: entry.topic_id,
                bi: entry.book.number(),
                ci: entry.chapter,
                fvi: entry.from_verse,
                tvi: entry.to_verse,
                scope: scope_for(entry),
                content: self
                    .module
                    .decode_content(entry.topic_id, entry.content_type.as_deref()),
            })
            .collect())
    }
}

#[derive(Debug)]
pub struct DictionaryEntry {
    pub topic_id: i64,
    pub subject: String,
    pub strong_orig_word: Option<String>,
    pub content: Result<Vec<Node>, ModuleError>,
}

#[derive(Debug)]
struct DictionaryRef {
    topic_id: i64,
    subject: String,
    strong_orig_word: Option<String>,
    content_type: Option<String>,
}

pub struct DictionarySource {
    module: TwmModule,
}

impl DictionarySource {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ModuleError> {
        let module = TwmModule::open(path)?;
        if module.kind != TwmKind::Dictionary {
            return Err(ModuleError::UnsupportedFormat(
                "O módulo não é um dicionário".to_string(),
            ));
        }
        module.validate_table(
            "topics",
            &["id", "subject", "strong_orig_word", "content_type"],
        )?;
        module.validate_table("content", &["topic_id", "data"])?;
        Ok(Self { module })
    }

    pub fn lookup(&self, key: &str) -> Result<Option<DictionaryEntry>, ModuleError> {
        let query = self
            .module
            .connection
            .query_row(
                "SELECT id, subject, strong_orig_word, content_type FROM topics \
                 WHERE subject = ?1 COLLATE NOCASE LIMIT 1",
                [key],
                read_dictionary_ref,
            )
            .optional()
            .map_err(corrupted)?;
        query.map(|entry| self.load_entry(entry)).transpose()
    }

    pub fn prefix_search(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<DictionaryEntry>, ModuleError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let escaped = escape_like(prefix);
        let pattern = format!("{escaped}%");
        let mut statement = self
            .module
            .connection
            .prepare(
                "SELECT id, subject, strong_orig_word, content_type FROM topics \
                 WHERE subject LIKE ?1 ESCAPE '\\' COLLATE NOCASE \
                 ORDER BY subject COLLATE NOCASE LIMIT ?2",
            )
            .map_err(corrupted)?;
        let records = statement
            .query_map((&pattern, sql_limit(limit)), read_dictionary_ref)
            .map_err(corrupted)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(corrupted)?;
        records
            .into_iter()
            .map(|entry| self.load_entry(entry))
            .collect()
    }

    pub fn search_word(
        &self,
        word: &str,
        limit: usize,
    ) -> Result<Vec<DictionaryEntry>, ModuleError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        if !self.module.table_exists("topics_wordindex")? {
            return Err(ModuleError::UnsupportedFormat(
                "Índice topics_wordindex indisponível".to_string(),
            ));
        }
        self.module
            .validate_table("topics_wordindex", &["id", "word", "priority"])?;
        let mut statement = self
            .module
            .connection
            .prepare(
                "SELECT t.id, t.subject, t.strong_orig_word, t.content_type \
                 FROM topics_wordindex w JOIN topics t ON t.id = w.id \
                 WHERE w.word = ?1 COLLATE NOCASE \
                 GROUP BY t.id, t.subject, t.strong_orig_word, t.content_type \
                 ORDER BY MAX(w.priority) DESC, t.subject COLLATE NOCASE LIMIT ?2",
            )
            .map_err(corrupted)?;
        let records = statement
            .query_map((word, sql_limit(limit)), read_dictionary_ref)
            .map_err(corrupted)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(corrupted)?;
        records
            .into_iter()
            .map(|entry| self.load_entry(entry))
            .collect()
    }

    fn load_entry(&self, entry: DictionaryRef) -> Result<DictionaryEntry, ModuleError> {
        Ok(DictionaryEntry {
            topic_id: entry.topic_id,
            subject: entry.subject,
            strong_orig_word: entry.strong_orig_word,
            content: self
                .module
                .decode_content(entry.topic_id, entry.content_type.as_deref()),
        })
    }
}

pub fn strong_key(lang: StrongLang, number: u32) -> String {
    let prefix = match lang {
        StrongLang::Greek => 'G',
        StrongLang::Hebrew => 'H',
    };
    format!("{prefix}{number}")
}

fn module_kind(config: &HashMap<String, String>, path: &Path) -> Result<TwmKind, ModuleError> {
    let declared = config
        .get("type")
        .map(|value| value.trim().to_ascii_lowercase());
    match declared.as_deref() {
        Some("1" | "dct" | "dictionary") => return Ok(TwmKind::Dictionary),
        Some("2" | "cmt" | "commentary") => return Ok(TwmKind::Commentary),
        Some("3" | "gbk" | "book" | "4" | "dev") => {
            return Err(ModuleError::UnsupportedFormat(format!(
                "Módulo fora do escopo M1.4: {}",
                declared.unwrap_or_default()
            )))
        }
        _ => {}
    }
    let filename = path.to_string_lossy().to_ascii_lowercase();
    if filename.ends_with(".cmt.twm") {
        return Ok(TwmKind::Commentary);
    }
    if filename.ends_with(".dct.twm") {
        return Ok(TwmKind::Dictionary);
    }
    Err(ModuleError::UnsupportedFormat(
        "Não foi possível identificar comentário ou dicionário".to_string(),
    ))
}

fn read_commentary_ref(row: &Row<'_>) -> rusqlite::Result<Option<CommentaryRef>> {
    let book_num: i64 = row.get(1)?;
    let chapter: i64 = row.get(2)?;
    let from_verse: i64 = row.get(3)?;
    let to_verse: i64 = row.get(4)?;
    let book = u8::try_from(book_num)
        .ok()
        .and_then(Book::from_number)
        .ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(1, "bi".into(), rusqlite::types::Type::Integer)
        })?;
    let chapter = u16::try_from(chapter).map_err(|_| {
        rusqlite::Error::InvalidColumnType(2, "ci".into(), rusqlite::types::Type::Integer)
    })?;
    let from_verse = u16::try_from(from_verse).map_err(|_| {
        rusqlite::Error::InvalidColumnType(3, "fvi".into(), rusqlite::types::Type::Integer)
    })?;
    let to_verse = u16::try_from(to_verse).map_err(|_| {
        rusqlite::Error::InvalidColumnType(4, "tvi".into(), rusqlite::types::Type::Integer)
    })?;
    if (chapter == 0 && (from_verse != 0 || to_verse != 0))
        || (chapter > 0
            && !((from_verse == 0 && to_verse == 0)
                || (from_verse > 0 && (to_verse == 0 || to_verse >= from_verse))))
    {
        return Ok(None);
    }
    Ok(Some(CommentaryRef {
        topic_id: row.get(0)?,
        book,
        chapter,
        from_verse,
        to_verse,
        content_type: row.get(5)?,
    }))
}

fn read_dictionary_ref(row: &Row<'_>) -> rusqlite::Result<DictionaryRef> {
    Ok(DictionaryRef {
        topic_id: row.get(0)?,
        subject: row.get(1)?,
        strong_orig_word: row.get(2)?,
        content_type: row.get(3)?,
    })
}

fn reference_contains(entry: &CommentaryRef, verse: VerseRef) -> bool {
    if entry.book != verse.book {
        return false;
    }
    if entry.chapter == 0 {
        return true;
    }
    entry.chapter == verse.chapter
        && if entry.from_verse == 0 {
            entry.to_verse == 0
        } else {
            entry.from_verse <= verse.verse
                && (entry.to_verse == 0 || entry.to_verse >= verse.verse)
        }
}

fn specificity_key(entry: &CommentaryRef) -> (u8, u16, u16, i64) {
    if entry.chapter == 0 {
        (3, 0, 0, entry.topic_id)
    } else if entry.from_verse == 0 {
        (2, 0, 0, entry.topic_id)
    } else if entry.from_verse == entry.to_verse {
        (0, 0, 0, entry.topic_id)
    } else {
        (
            1,
            if entry.to_verse == 0 {
                u16::MAX
            } else {
                entry.to_verse - entry.from_verse
            },
            entry.from_verse,
            entry.topic_id,
        )
    }
}

fn scope_for(entry: &CommentaryRef) -> CommentaryScope {
    if entry.chapter == 0 {
        CommentaryScope::Book
    } else if entry.from_verse == 0 {
        CommentaryScope::Chapter
    } else {
        CommentaryScope::Verse
    }
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn sql_limit(limit: usize) -> i64 {
    i64::try_from(limit).unwrap_or(i64::MAX)
}

fn corrupted(error: impl std::fmt::Display) -> ModuleError {
    ModuleError::Corrupted(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gen_fixtures::{
        generate_synthetic_commentary_twm, generate_synthetic_dictionary_twm,
        generate_synthetic_protected_twm,
    };
    use proptest::prelude::*;
    use tempfile::tempdir;

    #[test]
    fn commentary_orders_specific_to_broad_and_isolates_rvf() {
        let directory = tempdir().expect("diretório temporário");
        let path = directory.path().join("synthetic.cmt.twm");
        generate_synthetic_commentary_twm(&path).expect("gerar comentário sintético");
        let source = CommentarySource::open(&path).expect("abrir comentário");
        let query_only: i64 = source
            .module
            .connection
            .query_row("PRAGMA query_only", [], |row| row.get(0))
            .expect("consultar query_only");
        assert_eq!(query_only, 1);
        assert!(source
            .module
            .connection
            .execute(
                "INSERT INTO config(name, value) VALUES ('write-test', '1')",
                [],
            )
            .is_err());
        let verse = VerseRef::new(Book::John, 3, 16);
        let entries = source.entries_for(verse).expect("consultar comentário");
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.topic_id)
                .collect::<Vec<_>>(),
            [40, 30, 20, 10]
        );
        assert_eq!(entries[0].scope, CommentaryScope::Verse);
        assert!(entries[0].content.is_ok());
        assert_eq!(entries[1].scope, CommentaryScope::Verse);
        assert_eq!(entries[2].scope, CommentaryScope::Chapter);
        assert_eq!(entries[3].scope, CommentaryScope::Book);

        let book_intro = source
            .entries_for(VerseRef::new(Book::John, 4, 1))
            .expect("introdução do livro");
        assert_eq!(
            book_intro
                .iter()
                .map(|entry| entry.topic_id)
                .collect::<Vec<_>>(),
            [10]
        );
        let open_range = source
            .entries_for(VerseRef::new(Book::John, 3, 18))
            .expect("faixa aberta e introduções");
        assert_eq!(
            open_range
                .iter()
                .map(|entry| entry.topic_id)
                .collect::<Vec<_>>(),
            [60, 20, 10]
        );

        let unsupported = source
            .entries_for(VerseRef::new(Book::John, 3, 17))
            .expect("consultar entrada RVF sem falhar a consulta");
        let errors = unsupported
            .iter()
            .filter(|entry| matches!(entry.content, Err(ModuleError::UnsupportedContent(_))))
            .count();
        assert_eq!(unsupported.len(), 4);
        assert_eq!(errors, 1);
    }

    #[test]
    fn commentary_handles_missing_and_invalid_references() {
        let directory = tempdir().expect("diretório temporário");
        let path = directory.path().join("synthetic.cmt.twm");
        generate_synthetic_commentary_twm(&path).expect("gerar comentário sintético");
        let source = CommentarySource::open(&path).expect("abrir comentário");
        assert!(source
            .entries_for(VerseRef::new(Book::Matthew, 1, 1))
            .expect("consulta sem comentários")
            .is_empty());
        assert!(matches!(
            source.entries_for(VerseRef::new(Book::John, 0, 1)),
            Err(ModuleError::InvalidReference(_))
        ));
    }

    #[test]
    fn dictionary_lookup_prefix_and_word_search_work() {
        let directory = tempdir().expect("diretório temporário");
        let path = directory.path().join("synthetic.dct.twm");
        generate_synthetic_dictionary_twm(&path).expect("gerar dicionário sintético");
        let source = DictionarySource::open(&path).expect("abrir dicionário");
        let entry = source
            .lookup("g25")
            .expect("buscar chave")
            .expect("G25 existe");
        assert_eq!(entry.subject, "G25");
        assert!(entry.content.is_ok());
        assert_eq!(strong_key(StrongLang::Hebrew, 25), "H25");
        assert_eq!(source.prefix_search("g", 1).expect("prefixo").len(), 1);
        assert!(source
            .prefix_search("%", 5)
            .expect("prefixo literal")
            .is_empty());
        assert_eq!(source.search_word("invented", 5).expect("palavra").len(), 1);
        assert!(source
            .lookup("not-present")
            .expect("chave ausente")
            .is_none());

        let rvf = source
            .lookup("G26")
            .expect("buscar entrada RVF")
            .expect("G26 existe");
        assert!(matches!(
            rvf.content,
            Err(ModuleError::UnsupportedContent(ref kind)) if kind == "rvf"
        ));
    }

    #[test]
    fn protected_empty_and_truncated_modules_return_errors() {
        let directory = tempdir().expect("diretório temporário");
        for flag in ["do.encrypt", "secure"] {
            let path = directory.path().join(format!("{flag}.cmt.twm"));
            generate_synthetic_protected_twm(&path, flag).expect("gerar protegido");
            assert!(matches!(
                TwmModule::open(path),
                Err(ModuleError::EncryptedModule)
            ));
        }
        let empty = directory.path().join("empty.cmt.twm");
        std::fs::write(&empty, []).expect("criar arquivo vazio");
        assert!(matches!(
            TwmModule::open(&empty),
            Err(ModuleError::Corrupted(_))
        ));
        let truncated = directory.path().join("truncated.cmt.twm");
        std::fs::write(&truncated, b"SQLite format 3\0").expect("criar arquivo truncado");
        assert!(matches!(
            TwmModule::open(&truncated),
            Err(ModuleError::Corrupted(_))
        ));
    }

    #[test]
    fn real_twm_reader_timings_when_modules_are_configured() {
        let Ok(root) = std::env::var("RHEMA_TEST_MODULES") else {
            return;
        };
        let root = std::path::Path::new(&root);
        for filename in [
            "Holman-AT.cmt.twm",
            "NICNT.cmt.twm",
            "VidaNova.cmt.twm",
            "Strong_AMG2.dct.twm",
        ] {
            let path = root.join(filename);
            assert!(path.is_file(), "Módulo não encontrado: {}", path.display());
            if filename.ends_with(".cmt.twm") {
                let started = std::time::Instant::now();
                let source = CommentarySource::open(&path).expect("abrir comentário real");
                let open_elapsed = started.elapsed();
                let started = std::time::Instant::now();
                let _ = source
                    .entries_for(VerseRef::new(Book::John, 3, 16))
                    .expect("consultar João 3:16");
                let query_elapsed = started.elapsed();
                let started = std::time::Instant::now();
                let mut state = 0xA341_316Cu32;
                for _ in 0..500 {
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let book = Book::ALL[state as usize % Book::ALL.len()];
                    let chapter = (state.rotate_left(9) % u32::from(book.chapter_count())) + 1;
                    let verse_count = book.verses_in_chapter(chapter as u16).unwrap_or(1);
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let verse = (state % u32::from(verse_count)) + 1;
                    let _ = source
                        .entries_for(VerseRef::new(book, chapter as u16, verse as u16))
                        .expect("consulta aleatória válida");
                }
                eprintln!(
                    "{filename}: abertura={:.3} ms; João 3:16={:.3} ms; G25=n/a; 500 consultas={:.3} ms",
                    open_elapsed.as_secs_f64() * 1000.0,
                    query_elapsed.as_secs_f64() * 1000.0,
                    started.elapsed().as_secs_f64() * 1000.0
                );
            } else {
                let started = std::time::Instant::now();
                let source = DictionarySource::open(&path).expect("abrir dicionário real");
                let open_elapsed = started.elapsed();
                let started = std::time::Instant::now();
                let _ = source.lookup("G25").expect("consultar G25");
                eprintln!(
                    "{filename}: abertura={:.3} ms; João 3:16=n/a; G25={:.3} ms; 500 consultas=n/a",
                    open_elapsed.as_secs_f64() * 1000.0,
                    started.elapsed().as_secs_f64() * 1000.0
                );
            }
        }
    }

    proptest! {
        #[test]
        fn arbitrary_verse_queries_never_panic(book in 0u8..=255, chapter in 0u16..=u16::MAX, verse in 0u16..=u16::MAX) {
            let directory = tempdir().expect("diretório temporário");
            let path = directory.path().join("property.cmt.twm");
            generate_synthetic_commentary_twm(&path).expect("gerar comentário sintético");
            let source = CommentarySource::open(path).expect("abrir comentário sintético");
            if let Some(book) = Book::from_number(book) {
                let result = source.entries_for(VerseRef::new(book, chapter, verse));
                prop_assert!(result.is_ok() || matches!(result, Err(ModuleError::InvalidReference(_))));
            }
        }
    }
}
