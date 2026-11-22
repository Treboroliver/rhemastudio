use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use flate2::read::ZlibDecoder;
use rhema_core::{Book, ModuleError, Node, StrongLang, VerseRef};
use rusqlite::{types::Value, Connection, OpenFlags, OptionalExtension, Row};

use crate::RtfPage;

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
            .prepare(
                "SELECT name, value FROM config \
                 WHERE lower(trim(name)) IN ('type', 'content.type', 'title', 'lang', 'user', 'compressed')",
            )
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

        let kind = module_kind(&config, path, &connection)?;
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
        if let Some(value) = row_type.map(str::trim).filter(|s| !s.is_empty()) {
            return value.to_ascii_lowercase();
        }
        if let Some(value) = self
            .config
            .get("content.type")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            return value.to_ascii_lowercase();
        }
        if self.config.get("user").is_some_and(|v| v.trim() == "1") {
            "rvf".to_string()
        } else {
            "rtf".to_string()
        }
    }

    fn resolve_meta_target(
        &self,
        initial_topic_id: i64,
        initial_row_type: Option<&str>,
        lookup_row_type: impl Fn(i64) -> Result<Option<String>, ModuleError>,
    ) -> Result<(i64, String), ModuleError> {
        const MAX_META_HOPS: usize = 5;
        let mut current_id = initial_topic_id;
        let mut current_ct = self.content_type(initial_row_type);
        let mut visited = vec![current_id];

        for _ in 0..MAX_META_HOPS {
            if current_ct != "meta" {
                return Ok((current_id, current_ct));
            }
            let bytes = self.read_content_bytes(current_id)?;
            let next_id = parse_meta_shortcut(&bytes)?;
            if visited.contains(&next_id) {
                return Err(ModuleError::Corrupted(
                    "meta_cycle: ciclo detectado em atalho meta".to_string(),
                ));
            }
            visited.push(next_id);
            let next_row_type = lookup_row_type(next_id)?;
            current_ct = self.content_type(next_row_type.as_deref());
            current_id = next_id;
        }
        if current_ct == "meta" {
            return Err(ModuleError::Corrupted(
                "meta_depth: limite de saltos excedido em atalho meta".to_string(),
            ));
        }
        Ok((current_id, current_ct))
    }

    fn decode_content_resolved(
        &self,
        topic_id: i64,
        row_type: Option<&str>,
        lookup_row_type: impl Fn(i64) -> Result<Option<String>, ModuleError>,
    ) -> Result<crate::DecodedContent, ModuleError> {
        let (resolved_id, content_type) =
            self.resolve_meta_target(topic_id, row_type, lookup_row_type)?;
        if !matches!(
            content_type.as_str(),
            "rtf" | "rvf" | "html" | "" | "unknown" | "txt" | "text"
        ) {
            return Err(ModuleError::UnsupportedContent(content_type));
        }
        let bytes = self.read_content_bytes(resolved_id)?;
        crate::decode_twm_content_checked(&content_type, &bytes)
    }

    fn decode_content_page_resolved(
        &self,
        topic_id: i64,
        row_type: Option<&str>,
        offset: u64,
        max_paragraphs: usize,
        max_chars: usize,
        lookup_row_type: impl Fn(i64) -> Result<Option<String>, ModuleError>,
    ) -> Result<RtfPage, ModuleError> {
        let (resolved_id, content_type) =
            self.resolve_meta_target(topic_id, row_type, lookup_row_type)?;
        if !matches!(
            content_type.as_str(),
            "rtf" | "rvf" | "html" | "" | "unknown" | "txt" | "text"
        ) {
            return Err(ModuleError::UnsupportedContent(content_type));
        }
        let bytes = self.read_content_bytes(resolved_id)?;
        crate::decode_twm_content_page(&content_type, &bytes, offset, max_paragraphs, max_chars)
    }

    /// Tamanho bruto do conteúdo, sem convertê-lo.
    fn content_size(&self, topic_id: i64) -> Result<Option<u64>, ModuleError> {
        let size: Option<Option<i64>> = self
            .connection
            .query_row(
                "SELECT length(CAST(data AS BLOB)) FROM content WHERE topic_id = ?1",
                [topic_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(corrupted)?;
        Ok(size.flatten().and_then(|value| u64::try_from(value).ok()))
    }

    fn read_content_bytes(&self, topic_id: i64) -> Result<Vec<u8>, ModuleError> {
        let data: Value = self
            .connection
            .query_row(
                "SELECT CAST(data AS BLOB) FROM content WHERE topic_id = ?1",
                [topic_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(corrupted)?
            .ok_or_else(|| {
                ModuleError::Corrupted(format!("Conteúdo ausente para id {topic_id}"))
            })?;
        let raw = match data {
            Value::Text(text) => text.into_bytes(),
            Value::Blob(bytes) => bytes,
            Value::Null => {
                return Err(ModuleError::Corrupted(format!(
                    "Conteúdo NULL para id {topic_id}"
                )));
            }
            _ => {
                return Err(ModuleError::Corrupted(format!(
                    "Tipo SQLite de conteúdo inválido para id {topic_id}"
                )));
            }
        };
        let is_compressed = self
            .config
            .get("compressed")
            .is_some_and(|v| v.trim() == "1");
        let looks_zlib =
            raw.len() >= 2 && raw[0] == 0x78 && matches!(raw[1], 0x01 | 0x5E | 0x9C | 0xDA);
        if is_compressed && looks_zlib {
            let mut decoder = ZlibDecoder::new(raw.as_slice());
            let mut out = Vec::new();
            decoder.read_to_end(&mut out).map_err(|error| {
                ModuleError::Corrupted(format!(
                    "Descompressão zlib falhou para id {topic_id}: {error}"
                ))
            })?;
            return Ok(out);
        }
        if !is_compressed && looks_zlib {
            let mut decoder = ZlibDecoder::new(raw.as_slice());
            let mut out = Vec::new();
            if decoder.read_to_end(&mut out).is_ok() {
                return Ok(out);
            }
        }
        Ok(raw)
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
    /// Conteúdo RVF em melhor esforço; a interface mostra um aviso discreto.
    pub degraded: bool,
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

/// Cabeçalho de uma entrada de comentário; não inclui o conteúdo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentaryHeader {
    pub topic_id: i64,
    pub bi: u8,
    pub ci: u16,
    pub fvi: u16,
    pub tvi: u16,
    pub scope: CommentaryScope,
    /// Tipo efetivo do conteúdo (`rtf`, `rvf`, ...).
    pub content_type: String,
    /// Falso quando o formato não tem conversor.
    pub supported: bool,
    pub size_bytes: Option<u64>,
}

pub struct CommentarySource {
    module: TwmModule,
    refs: Vec<CommentaryRef>,
    by_book: HashMap<Book, Vec<usize>>,
    by_topic: HashMap<i64, usize>,
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
        let mut by_topic = HashMap::<i64, usize>::new();
        for (index, reference) in refs.iter().enumerate() {
            by_book.entry(reference.book).or_default().push(index);
            by_topic.insert(reference.topic_id, index);
        }
        Ok(Self {
            module,
            refs,
            by_book,
            by_topic,
        })
    }

    fn matching(&self, verse: VerseRef) -> Result<Vec<&CommentaryRef>, ModuleError> {
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
        Ok(matching)
    }

    pub fn entries_for(&self, verse: VerseRef) -> Result<Vec<CommentaryEntry>, ModuleError> {
        Ok(self
            .matching(verse)?
            .into_iter()
            .map(|entry| {
                let decoded = self.module.decode_content_resolved(
                    entry.topic_id,
                    entry.content_type.as_deref(),
                    |id| self.row_content_type(id),
                );
                CommentaryEntry {
                    topic_id: entry.topic_id,
                    bi: entry.book.number(),
                    ci: entry.chapter,
                    fvi: entry.from_verse,
                    tvi: entry.to_verse,
                    scope: scope_for(entry),
                    degraded: decoded.as_ref().is_ok_and(|decoded| decoded.degraded),
                    content: decoded.map(|decoded| decoded.nodes),
                }
            })
            .collect())
    }

    /// Cabeçalhos das entradas que cobrem o versículo, da mais específica à mais ampla.
    /// Não lê nem converte o conteúdo; somente o tamanho bruto é consultado.
    pub fn headers_for(&self, verse: VerseRef) -> Result<Vec<CommentaryHeader>, ModuleError> {
        self.matching(verse)?
            .into_iter()
            .map(|entry| {
                let content_type = self.module.content_type(entry.content_type.as_deref());
                Ok(CommentaryHeader {
                    topic_id: entry.topic_id,
                    bi: entry.book.number(),
                    ci: entry.chapter,
                    fvi: entry.from_verse,
                    tvi: entry.to_verse,
                    scope: scope_for(entry),
                    supported: matches!(
                        content_type.as_str(),
                        "rtf" | "rvf" | "html" | "txt" | "text" | "meta"
                    ),
                    content_type,
                    size_bytes: self.module.content_size(entry.topic_id)?,
                })
            })
            .collect()
    }

    /// Converte uma página (parágrafos e caracteres limitados) de uma entrada deste módulo.
    pub fn entry_page(
        &self,
        topic_id: i64,
        offset: u64,
        max_paragraphs: usize,
        max_chars: usize,
    ) -> Result<RtfPage, ModuleError> {
        let entry = self
            .by_topic
            .get(&topic_id)
            .and_then(|index| self.refs.get(*index))
            .ok_or_else(|| ModuleError::NotFound(format!("entrada {topic_id}")))?;
        self.module.decode_content_page_resolved(
            entry.topic_id,
            entry.content_type.as_deref(),
            offset,
            max_paragraphs,
            max_chars,
            |id| self.row_content_type(id),
        )
    }

    fn row_content_type(&self, topic_id: i64) -> Result<Option<String>, ModuleError> {
        if let Some(r) = self
            .by_topic
            .get(&topic_id)
            .and_then(|idx| self.refs.get(*idx))
        {
            return Ok(r.content_type.clone());
        }
        let ct: Option<Option<String>> = self
            .module
            .connection
            .query_row(
                "SELECT content_type FROM bible_refs WHERE topic_id = ?1 LIMIT 1",
                [topic_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(corrupted)?;
        Ok(ct.flatten())
    }
}

#[derive(Debug)]
pub struct DictionaryEntry {
    pub topic_id: i64,
    pub subject: String,
    pub strong_orig_word: Option<String>,
    pub content: Result<Vec<Node>, ModuleError>,
    /// Conteúdo RVF em melhor esforço; a interface mostra um aviso discreto.
    pub degraded: bool,
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
    has_strong_orig_word: bool,
    has_content_type: bool,
    has_topics_flags: bool,
}

impl DictionarySource {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ModuleError> {
        let module = TwmModule::open(path)?;
        if module.kind != TwmKind::Dictionary {
            return Err(ModuleError::UnsupportedFormat(
                "O módulo não é um dicionário".to_string(),
            ));
        }
        module.validate_table("topics", &["id", "subject"])?;
        module.validate_table("content", &["topic_id", "data"])?;
        let has_col = |name: &str| -> Result<bool, ModuleError> {
            module
                .connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM pragma_table_info('topics') WHERE name=?1)",
                    [name],
                    |row| row.get(0),
                )
                .map_err(corrupted)
        };
        let has_strong_orig_word = has_col("strong_orig_word")?;
        let has_content_type = has_col("content_type")?;
        let has_topics_flags = module.table_exists("topics_flags")?;
        Ok(Self {
            module,
            has_strong_orig_word,
            has_content_type,
            has_topics_flags,
        })
    }

    fn topic_columns(&self, prefix: &str) -> String {
        let orig = if self.has_strong_orig_word {
            format!("{prefix}strong_orig_word")
        } else {
            "NULL".to_string()
        };
        let ct = if self.has_content_type {
            format!("{prefix}content_type")
        } else {
            "NULL".to_string()
        };
        format!("{prefix}id, {prefix}subject, {orig}, {ct}")
    }

    pub fn lookup(&self, key: &str) -> Result<Option<DictionaryEntry>, ModuleError> {
        let cols = self.topic_columns("");
        let sql = format!("SELECT {cols} FROM topics WHERE subject = ?1 COLLATE NOCASE LIMIT 1");
        let query = self
            .module
            .connection
            .query_row(&sql, [key], read_dictionary_ref)
            .optional()
            .map_err(corrupted)?;
        query.map(|entry| self.load_entry(entry)).transpose()
    }

    /// Busca uma entrada de dicionário diretamente pelo `topic_id` estável.
    pub fn lookup_by_id(&self, topic_id: i64) -> Result<Option<DictionaryEntry>, ModuleError> {
        let cols = self.topic_columns("");
        let sql = format!("SELECT {cols} FROM topics WHERE id = ?1 LIMIT 1");
        let query = self
            .module
            .connection
            .query_row(&sql, [topic_id], read_dictionary_ref)
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
        let cols = self.topic_columns("");
        let hidden_filter = if self.has_topics_flags {
            " AND NOT EXISTS (SELECT 1 FROM topics_flags tf WHERE tf.id = topics.id AND tf.hidden = 1)"
        } else {
            ""
        };
        let sql = format!(
            "SELECT {cols} FROM topics \
             WHERE subject LIKE ?1 ESCAPE '\\' COLLATE NOCASE{hidden_filter} \
             ORDER BY subject COLLATE NOCASE LIMIT ?2"
        );
        let mut statement = self.module.connection.prepare(&sql).map_err(corrupted)?;
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
        let cols = self.topic_columns("t.");
        let hidden_filter = if self.has_topics_flags {
            " AND NOT EXISTS (SELECT 1 FROM topics_flags tf WHERE tf.id = t.id AND tf.hidden = 1)"
        } else {
            ""
        };
        let sql = format!(
            "SELECT {cols} \
             FROM topics_wordindex w JOIN topics t ON t.id = w.id \
             WHERE w.word = ?1 COLLATE NOCASE{hidden_filter} \
             GROUP BY t.id, t.subject \
             ORDER BY MAX(w.priority) DESC, t.subject COLLATE NOCASE LIMIT ?2"
        );
        let mut statement = self.module.connection.prepare(&sql).map_err(corrupted)?;
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
        let decoded = self.module.decode_content_resolved(
            entry.topic_id,
            entry.content_type.as_deref(),
            |id| self.row_content_type(id),
        );
        Ok(DictionaryEntry {
            topic_id: entry.topic_id,
            subject: entry.subject,
            strong_orig_word: entry.strong_orig_word,
            degraded: decoded.as_ref().is_ok_and(|decoded| decoded.degraded),
            content: decoded.map(|decoded| decoded.nodes),
        })
    }

    fn row_content_type(&self, topic_id: i64) -> Result<Option<String>, ModuleError> {
        if !self.has_content_type {
            return Ok(None);
        }
        let ct: Option<Option<String>> = self
            .module
            .connection
            .query_row(
                "SELECT content_type FROM topics WHERE id = ?1 LIMIT 1",
                [topic_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(corrupted)?;
        Ok(ct.flatten())
    }
}

fn parse_meta_shortcut(bytes: &[u8]) -> Result<i64, ModuleError> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let Some(first) = lines.next() else {
        return Err(ModuleError::Corrupted(
            "meta_invalid: tópico meta vazio".to_string(),
        ));
    };
    if !first.eq_ignore_ascii_case("meta=_twmgc_") {
        return Err(ModuleError::Corrupted(
            "meta_invalid: cabeçalho meta=_twmgc_ ausente".to_string(),
        ));
    }
    let mut is_link = false;
    let mut target_id: Option<i64> = None;
    for line in lines {
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            let val = v.trim();
            if key.eq_ignore_ascii_case("type") && val.eq_ignore_ascii_case("link") {
                is_link = true;
            } else if key.eq_ignore_ascii_case("id") {
                target_id = val.parse::<i64>().ok();
            }
        }
    }
    if let (true, Some(id)) = (is_link, target_id) {
        return Ok(id);
    }
    Err(ModuleError::Corrupted(
        "meta_invalid: atalho meta sem type=link ou id válido".to_string(),
    ))
}

pub fn strong_key(lang: StrongLang, number: u32) -> String {
    let prefix = match lang {
        StrongLang::Greek => 'G',
        StrongLang::Hebrew => 'H',
    };
    format!("{prefix}{number}")
}

/// Normaliza uma chave de Strong: `g0025`, `G25a` e ` G25 ` viram `(Greek, 25)`.
/// Aceita zeros à esquerda e um sufixo de letras; rejeita número zero ou longo demais.
pub fn normalize_strong_key(text: &str) -> Option<(StrongLang, u32)> {
    let text = text.trim();
    let mut chars = text.chars();
    let lang = match chars.next()?.to_ascii_uppercase() {
        'G' => StrongLang::Greek,
        'H' => StrongLang::Hebrew,
        _ => return None,
    };
    let rest = chars.as_str();
    let digits_end = rest
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(rest.len());
    let (digits, suffix) = rest.split_at(digits_end);
    if digits.is_empty()
        || !suffix
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return None;
    }
    let significant = digits.trim_start_matches('0');
    if significant.is_empty() || significant.len() > 5 {
        return None;
    }
    significant.parse::<u32>().ok().map(|number| (lang, number))
}

fn module_kind(
    config: &HashMap<String, String>,
    path: &Path,
    connection: &Connection,
) -> Result<TwmKind, ModuleError> {
    let filename = path.to_string_lossy().to_ascii_lowercase();
    let has_topics: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='topics')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(false);
    let has_bible_refs: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='bible_refs')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(false);
    let declared = config
        .get("type")
        .map(|value| value.trim().to_ascii_lowercase());
    match declared.as_deref() {
        Some("1" | "dct" | "dictionary") => return Ok(TwmKind::Dictionary),
        Some("2" | "cmt" | "commentary") => return Ok(TwmKind::Commentary),
        Some("3" | "gbk" | "book" | "4" | "dev") => {
            if filename.ends_with(".dct.twm") && has_topics && !has_bible_refs {
                return Ok(TwmKind::Dictionary);
            }
            return Err(ModuleError::UnsupportedFormat(format!(
                "Módulo fora do escopo M1.4: {}",
                declared.unwrap_or_default()
            )));
        }
        _ => {}
    }
    if filename.ends_with(".dct.twm") && has_topics && !has_bible_refs {
        return Ok(TwmKind::Dictionary);
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
        generate_synthetic_protected_twm, generate_synthetic_study_commentary_twm,
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
        let rvf = unsupported
            .iter()
            .filter(|entry| entry.content.is_ok() && entry.degraded)
            .count();
        let errors = unsupported
            .iter()
            .filter(|entry| matches!(entry.content, Err(ModuleError::UnsupportedContent(_))))
            .count();
        assert_eq!(unsupported.len(), 4);
        assert_eq!(errors, 0);
        assert_eq!(rvf, 1);
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
        assert!(rvf.content.is_ok());
        assert!(rvf.degraded);
    }

    #[test]
    fn normalizes_strong_keys() {
        use StrongLang::{Greek, Hebrew};
        assert_eq!(normalize_strong_key("G25"), Some((Greek, 25)));
        assert_eq!(normalize_strong_key(" g0025 "), Some((Greek, 25)));
        assert_eq!(normalize_strong_key("G25a"), Some((Greek, 25)));
        assert_eq!(normalize_strong_key("h00430"), Some((Hebrew, 430)));
        for invalid in [
            "", "G", "G0", "G000", "25", "X25", "G2 5", "G25-1", "G123456", "G25é",
        ] {
            assert_eq!(normalize_strong_key(invalid), None, "{invalid:?}");
        }
    }

    #[test]
    fn commentary_headers_do_not_decode_content_and_page_on_demand() {
        let directory = tempdir().expect("diretório temporário");
        let path = directory.path().join("study.cmt.twm");
        generate_synthetic_study_commentary_twm(&path, 150).expect("gerar comentário");
        let source = CommentarySource::open(&path).expect("abrir comentário");
        let headers = source
            .headers_for(VerseRef::new(Book::John, 3, 16))
            .expect("cabeçalhos");
        assert_eq!(
            headers
                .iter()
                .map(|header| header.topic_id)
                .collect::<Vec<_>>(),
            [40, 50, 70, 30, 60, 20, 10]
        );
        assert!(headers[0].size_bytes.unwrap_or_default() > 1_000);
        let unsupported = headers
            .iter()
            .filter(|header| !header.supported)
            .map(|header| (header.topic_id, header.content_type.as_str()))
            .collect::<Vec<_>>();
        assert!(unsupported.is_empty());

        let first = source
            .entry_page(40, 0, 60, 8_000)
            .expect("primeira página");
        let paragraphs = |page: &RtfPage| {
            page.nodes
                .iter()
                .filter(|node| matches!(node, Node::Paragraph))
                .count()
        };
        assert_eq!(paragraphs(&first), 60);
        assert!(first.has_more);
        let second = source
            .entry_page(40, first.next_offset, 60, 8_000)
            .expect("segunda");
        let third = source
            .entry_page(40, second.next_offset, 60, 8_000)
            .expect("terceira");
        assert_eq!(paragraphs(&second), 60);
        assert_eq!(paragraphs(&third), 30);
        assert!(second.has_more && !third.has_more);

        assert!(
            source
                .entry_page(50, 0, 60, 8_000)
                .expect("RVF degradado")
                .degraded
        );
        assert!(matches!(
            source.entry_page(999, 0, 60, 8_000),
            Err(ModuleError::NotFound(_))
        ));
        assert!(matches!(
            source.headers_for(VerseRef::new(Book::John, 0, 1)),
            Err(ModuleError::InvalidReference(_))
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

    /// Diagnóstico sem texto: contagem de "vírgula + letra" e contextos mascarados (x/9).
    #[test]
    fn real_rtf_comma_spacing_diagnostic() {
        let Ok(root) = std::env::var("RHEMA_TEST_MODULES") else {
            return;
        };
        // Vírgula seguida de letra no RTF bruto: direto, ou só após palavras de controle
        // (o único espaço depois de uma palavra de controle é delimitador, não conteúdo).
        let raw_counts = |raw: &[u8]| {
            let (mut direct, mut delimited) = (0usize, 0usize);
            for (i, byte) in raw.iter().enumerate() {
                if *byte != b',' {
                    continue;
                }
                let mut j = i + 1;
                let mut skipped = false;
                while j < raw.len() {
                    match raw[j] {
                        b'{' | b'}' => j += 1,
                        b'\\' if raw.get(j + 1).is_some_and(u8::is_ascii_alphabetic) => {
                            j += 1;
                            while raw.get(j).is_some_and(u8::is_ascii_alphabetic) {
                                j += 1;
                            }
                            if raw.get(j) == Some(&b'-') {
                                j += 1;
                            }
                            while raw.get(j).is_some_and(u8::is_ascii_digit) {
                                j += 1;
                            }
                            if raw.get(j) == Some(&b' ') {
                                j += 1;
                            }
                        }
                        _ => break,
                    }
                    skipped = true;
                }
                if raw.get(j).is_some_and(u8::is_ascii_alphabetic) {
                    if skipped {
                        delimited += 1;
                    } else {
                        direct += 1;
                    }
                }
            }
            (direct, delimited)
        };
        for filename in ["Holman-AT.cmt.twm", "NICNT.cmt.twm", "Strong_AMG2.dct.twm"] {
            let path = std::path::Path::new(&root).join(filename);
            let module = TwmModule::open(&path).expect("abrir módulo real");
            let mut statement = module
                .connection
                .prepare("SELECT topic_id FROM content ORDER BY topic_id")
                .expect("ids");
            let ids = statement
                .query_map([], |row| row.get::<_, i64>(0))
                .expect("consulta")
                .flatten()
                .collect::<Vec<_>>();
            drop(statement);
            let step = (ids.len() / 2000).max(1);
            let (mut entries, mut hits) = (0usize, 0usize);
            let (mut raw_direct, mut raw_delimited) = (0usize, 0usize);
            for id in ids.iter().step_by(step).take(2000) {
                let Ok(raw) = module.read_content_bytes(*id) else {
                    continue;
                };
                let Ok(nodes) = crate::decode_twm_content("rtf", &raw) else {
                    continue;
                };
                entries += 1;
                let plain = nodes.iter().map(Node::to_plain_text).collect::<String>();
                let chars = plain.chars().collect::<Vec<_>>();
                let found = chars
                    .windows(2)
                    .filter(|w| w[0] == ',' && w[1].is_alphabetic())
                    .count();
                hits += found;
                let (direct, delimited) = raw_counts(&raw);
                raw_direct += direct;
                raw_delimited += delimited;
            }
            eprintln!(
                "{filename}: entries={entries} raw_direct={raw_direct} raw_via_controls={raw_delimited} converted={hits}"
            );
        }
    }

    /// Diagnóstico RVF somente com números: entradas, degradadas, construções, acentos, links e tempo.
    #[test]
    fn real_rvf_diagnostic() {
        let Ok(root) = std::env::var("RHEMA_TEST_MODULES") else {
            return;
        };
        let modules = [
            ("VidaNova", "VidaNova.cmt.twm", "bible_refs", "topic_id"),
            ("Strong_AMG2", "Strong_AMG2.dct.twm", "topics", "id"),
            ("Holman", "Holman-AT.cmt.twm", "bible_refs", "topic_id"),
            ("BEP", "BEP.cmt.twm", "bible_refs", "topic_id"),
        ];
        let mut dump = std::env::var("RHEMA_RVF_DUMP")
            .ok()
            .and_then(|path| std::fs::File::create(path).ok());
        for (label, filename, table, id_column) in modules {
            let path = std::path::Path::new(&root).join(filename);
            let module = TwmModule::open(&path).expect("abrir módulo real");
            let sql = format!("SELECT {id_column} FROM {table} WHERE content_type = 'rvf'");
            let mut statement = module.connection.prepare(&sql).expect("ids");
            let ids = statement
                .query_map([], |row| row.get::<_, i64>(0))
                .expect("consulta")
                .flatten()
                .collect::<Vec<_>>();
            drop(statement);
            let (mut converted, mut degraded, mut good, mut broken, mut empty) = (0, 0, 0, 0, 0);
            let (mut bad_chars, mut shown) = (0usize, 0usize);
            let mut stats = crate::RvfStats::default();
            let started = std::time::Instant::now();
            for id in &ids {
                let Ok(raw) = module.read_content_bytes(*id) else {
                    continue;
                };
                let result = crate::convert_rvf(&raw);
                converted += 1;
                degraded += usize::from(result.degraded);
                empty += usize::from(result.nodes.is_empty());
                stats.items += result.stats.items;
                stats.utf16_rejected += result.stats.utf16_rejected;
                for (key, value) in result.stats.links {
                    *stats.links.entry(key).or_default() += value;
                }
                for (key, value) in result.stats.unknown {
                    *stats.unknown.entry(key).or_default() += value;
                }
                let plain = result
                    .nodes
                    .iter()
                    .map(Node::to_plain_text)
                    .collect::<String>();
                let chars = plain.chars().collect::<Vec<_>>();
                if let Some(dump) = dump.as_mut() {
                    use std::io::Write;
                    let visible = chars.iter().filter(|c| **c != '\n').count();
                    let _ = writeln!(dump, "{label},{id},{visible}");
                }
                let mojibake = chars
                    .windows(2)
                    .any(|w| w[0] == 'Ã' && "©£³§\u{ad}¡ªº".contains(w[1]));
                let bad = crate::rvf::suspicious_count(&plain);
                if mojibake || bad > 0 {
                    broken += 1;
                    bad_chars += bad;
                    let at = chars
                        .windows(2)
                        .position(|w| w[0] == 'Ã' && "©£³§\u{ad}¡ªº".contains(w[1]))
                        .or_else(|| {
                            chars
                                .iter()
                                .position(|c| crate::rvf::is_suspicious_char(*c))
                        })
                        .unwrap_or(0);
                    let from = at.saturating_sub(12);
                    let masked = chars[from..(at + 12).min(chars.len())]
                        .iter()
                        .map(|c| {
                            if crate::rvf::is_suspicious_char(*c) {
                                '#'
                            } else if c.is_alphabetic() {
                                'x'
                            } else {
                                *c
                            }
                        })
                        .collect::<String>();
                    if shown < 5 {
                        shown += 1;
                        eprintln!(
                            "{label} suspicious id={id} chars={bad} mojibake={mojibake} ctx={masked:?} cps={:x?}",
                            chars
                                .iter()
                                .filter(|c| crate::rvf::is_suspicious_char(**c))
                                .take(8)
                                .map(|c| *c as u32)
                                .collect::<Vec<_>>()
                        );
                    }
                } else if plain.chars().any(|c| "áéíóúãõçâêô".contains(c)) {
                    good += 1;
                }
            }
            let elapsed = started.elapsed();
            let per_entry_us = elapsed.as_micros() / converted.max(1) as u128;
            eprintln!(
                "{label}: rvf={} converted={converted} degraded={degraded} empty={empty} items={} rejected_utf16={} accents_ok={good} suspicious_entries={broken} suspicious_chars={bad_chars} links={:?} unknown={:?} us_per_entry={per_entry_us}",
                ids.len(),
                stats.items,
                stats.utf16_rejected,
                stats.links,
                stats.unknown
            );
        }
        let path = std::path::Path::new(&root).join("Strong_AMG2.dct.twm");
        let module = TwmModule::open(&path).expect("abrir módulo real");
        let id = module
            .connection
            .query_row(
                "SELECT id FROM topics WHERE subject = 'G3056' AND content_type = 'rvf'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .ok();
        let ok = id
            .and_then(|id| module.read_content_bytes(id).ok())
            .map(|raw| {
                let result = crate::convert_rvf(&raw);
                let plain = result
                    .nodes
                    .iter()
                    .map(Node::to_plain_text)
                    .collect::<String>();
                (
                    !result.nodes.is_empty(),
                    result.degraded,
                    crate::rvf::suspicious_count(&plain),
                )
            });
        eprintln!("G3056 (nós, degradado, suspeitos): {ok:?}");

        // Primeira página da maior entrada do VidaNova.
        let path = std::path::Path::new(&root).join("VidaNova.cmt.twm");
        let source = CommentarySource::open(&path).expect("abrir VidaNova");
        let module = TwmModule::open(&path).expect("abrir módulo real");
        let biggest = module
            .connection
            .query_row(
                "SELECT topic_id, length(CAST(data AS BLOB)) FROM content ORDER BY 2 DESC LIMIT 1",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .expect("maior entrada");
        for label in ["frio", "morno"] {
            let started = std::time::Instant::now();
            let page = source
                .entry_page(biggest.0, 0, 60, 8_000)
                .expect("primeira página");
            eprintln!(
                "VidaNova maior entrada ({} KB) primeira página {label}: {:.1} ms nós={} has_more={}",
                biggest.1 / 1024,
                started.elapsed().as_secs_f64() * 1000.0,
                page.nodes.len(),
                page.has_more
            );
        }
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

    #[test]
    fn resolves_meta_shortcuts_cycles_hidden_topics_and_zlib_compression() {
        use flate2::{write::ZlibEncoder, Compression};
        use std::io::Write;

        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("meta_hidden_zlib.twm");
        let conn = Connection::open(&path).expect("criar sqlite");
        conn.execute_batch(
            "CREATE TABLE config (name TEXT, value TEXT);
             CREATE TABLE topics (id INTEGER PRIMARY KEY, subject TEXT, content_type TEXT);
             CREATE TABLE topics_flags (id INTEGER PRIMARY KEY, hidden INT);
             CREATE TABLE topics_wordindex (id INTEGER, word TEXT, priority INT);
             CREATE TABLE content (topic_id INTEGER PRIMARY KEY, data BLOB);
             INSERT INTO config (name, value) VALUES ('type', '1'), ('compressed', '1'), ('user', '0');",
        )
        .expect("schema");

        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
        enc.write_all(br"{\rtf1 Alvo comprimido}")
            .expect("write zlib");
        let compressed_rtf = enc.finish().expect("finish zlib");

        conn.execute(
            "INSERT INTO topics (id, subject, content_type) VALUES (1, 'ALVO', NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO content (topic_id, data) VALUES (1, ?1)",
            [compressed_rtf],
        )
        .unwrap();

        // Atalho meta válido -> 1
        conn.execute(
            "INSERT INTO topics (id, subject, content_type) VALUES (2, 'ATALHO', 'meta')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO content (topic_id, data) VALUES (2, 'meta=_twmgc_\ntype=link\nid=1\nanchor=x')",
            [],
        )
        .unwrap();

        // Ciclo meta: 3 -> 4 -> 3
        conn.execute(
            "INSERT INTO topics (id, subject, content_type) VALUES (3, 'CICLO1', 'meta'), (4, 'CICLO2', 'meta')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO content (topic_id, data) VALUES (3, 'meta=_twmgc_\ntype=link\nid=4'), (4, 'meta=_twmgc_\ntype=link\nid=3')",
            [],
        )
        .unwrap();

        // Tópico oculto
        conn.execute(
            "INSERT INTO topics (id, subject, content_type) VALUES (5, 'OCULTO', 'rtf')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO topics_flags (id, hidden) VALUES (5, 1)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO topics_wordindex (id, word, priority) VALUES (5, 'segredo', 10), (1, 'segredo', 5)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO content (topic_id, data) VALUES (5, ?1)",
            [br"{\rtf1 Texto oculto}".as_slice()],
        )
        .unwrap();
        drop(conn);

        let src = DictionarySource::open(&path).expect("abrir dicionário sem extensão .dct");
        // Zlib + default rtf (user=0)
        let alvo = src.lookup("ALVO").expect("lookup").expect("existe");
        assert_eq!(
            crate::plain_text(&alvo.content.expect("decodificado")),
            "Alvo comprimido"
        );
        // Meta shortcut
        let atalho = src.lookup("ATALHO").expect("lookup").expect("existe");
        assert_eq!(
            crate::plain_text(&atalho.content.expect("meta resolvido")),
            "Alvo comprimido"
        );
        // Meta cycle
        let ciclo = src.lookup("CICLO1").expect("lookup").expect("existe");
        assert!(matches!(
            ciclo.content,
            Err(ModuleError::Corrupted(ref msg)) if msg.contains("meta_cycle")
        ));
        // Hidden: fora de prefix_search e search_word, mas abre direto
        let pref = src.prefix_search("OCUL", 10).expect("prefix");
        assert!(pref.is_empty());
        let word_hits = src.search_word("segredo", 10).expect("word");
        assert_eq!(word_hits.len(), 1);
        assert_eq!(word_hits[0].topic_id, 1);
        let direto = src
            .lookup("OCULTO")
            .expect("lookup direto")
            .expect("existe");
        assert_eq!(
            crate::plain_text(&direto.content.expect("oculto direto")),
            "Texto oculto"
        );
    }
}
