use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::error::{Result, SermonError};
use crate::model::{Block, Sermon, SermonSummary, VersionInfo, validate_id, validate_title};

pub const LATEST_SCHEMA: i64 = 2;
pub const MAX_VERSIONS: usize = 20;
pub const BACKUP_INTERVAL_MS: i64 = 60_000;
const COPY_SUFFIX: &str = " (cópia)";

pub(crate) const SCHEMA_V1: &str = "
CREATE TABLE sermons (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE sermon_blocks (
    sermon_id TEXT NOT NULL REFERENCES sermons(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    block_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    json TEXT NOT NULL,
    PRIMARY KEY (sermon_id, position)
);";

const SCHEMA_V2: &str = "
ALTER TABLE sermons ADD COLUMN base_text TEXT NOT NULL DEFAULT '';
CREATE TABLE sermon_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sermon_id TEXT NOT NULL REFERENCES sermons(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    title TEXT NOT NULL,
    block_count INTEGER NOT NULL,
    snapshot TEXT NOT NULL
);
CREATE INDEX idx_sermon_versions_sermon ON sermon_versions (sermon_id, id);
CREATE INDEX idx_sermons_updated ON sermons (updated_at);";

const MIGRATIONS: [&str; 2] = [SCHEMA_V1, SCHEMA_V2];

type Clock = Box<dyn Fn() -> i64 + Send + Sync>;

fn system_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

pub struct SermonStore {
    conn: Mutex<Connection>,
    clock: Clock,
}

impl SermonStore {
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_clock(path, Box::new(system_now))
    }

    pub fn open_with_clock(path: &Path, clock: Clock) -> Result<Self> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.query_row("PRAGMA journal_mode = WAL", [], |row| {
            row.get::<_, String>(0)
        })?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            clock,
        })
    }

    fn lock(&self) -> MutexGuard<'_, Connection> {
        self.conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .lock()
            .query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }

    /// Cria um sermão vazio e o grava.
    pub fn create(&self, title: &str, base_text: &str) -> Result<Sermon> {
        self.save(&Sermon::new(title, base_text, (self.clock)()))
    }

    pub fn get(&self, id: &str) -> Result<Sermon> {
        validate_id(id)?;
        load_sermon(&self.lock(), id)?.ok_or(SermonError::NotFound)
    }

    pub fn list(&self, include_archived: bool) -> Result<Vec<SermonSummary>> {
        let conn = self.lock();
        let mut statement = conn.prepare(
            "SELECT s.id, s.title, s.base_text, s.created_at, s.updated_at, s.archived,
                    (SELECT COUNT(*) FROM sermon_blocks b WHERE b.sermon_id = s.id)
             FROM sermons s
             WHERE (?1 OR s.archived = 0)
             ORDER BY s.updated_at DESC, s.id",
        )?;
        let rows = statement.query_map([include_archived], |row| {
            Ok(SermonSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                base_text: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                archived: row.get::<_, i64>(5)? != 0,
                block_count: row.get::<_, i64>(6)?.max(0) as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Busca por trecho do título, sem diferenciar maiúsculas de minúsculas.
    pub fn search_by_title(
        &self,
        query: &str,
        include_archived: bool,
    ) -> Result<Vec<SermonSummary>> {
        let needle = query.trim().to_lowercase();
        Ok(self
            .list(include_archived)?
            .into_iter()
            .filter(|summary| needle.is_empty() || summary.title.to_lowercase().contains(&needle))
            .collect())
    }

    /// Grava o sermão numa transação; devolve o estado gravado (com `updated_at` novo).
    pub fn save(&self, sermon: &Sermon) -> Result<Sermon> {
        self.save_inner(sermon, None)
    }

    fn save_inner(&self, sermon: &Sermon, fail_after_blocks: Option<usize>) -> Result<Sermon> {
        sermon.validate()?;
        let now = (self.clock)();
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        if let Some(previous) = load_sermon(&tx, &sermon.id)?
            && content_differs(&previous, sermon)
        {
            backup_if_due(&tx, &previous, now)?;
        }
        tx.execute(
            "INSERT INTO sermons (id, title, base_text, created_at, updated_at, archived)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                 title = excluded.title, base_text = excluded.base_text,
                 updated_at = excluded.updated_at, archived = excluded.archived",
            params![
                sermon.id,
                sermon.title,
                sermon.base_text,
                sermon.created_at,
                now,
                sermon.archived
            ],
        )?;
        write_blocks(&tx, &sermon.id, &sermon.blocks, fail_after_blocks)?;
        let created_at: i64 = tx.query_row(
            "SELECT created_at FROM sermons WHERE id = ?1",
            [&sermon.id],
            |row| row.get(0),
        )?;
        tx.commit()?;
        let mut saved = sermon.clone();
        saved.created_at = created_at;
        saved.updated_at = now;
        Ok(saved)
    }

    pub fn duplicate(&self, id: &str) -> Result<Sermon> {
        let mut copy = self.get(id)?;
        let now = (self.clock)();
        copy.id = uuid::Uuid::new_v4().to_string();
        copy.created_at = now;
        copy.updated_at = now;
        copy.archived = false;
        let keep = crate::model::MAX_TITLE_CHARS - COPY_SUFFIX.chars().count();
        let base: String = copy.title.chars().take(keep).collect();
        copy.title = format!("{base}{COPY_SUFFIX}");
        for block in &mut copy.blocks {
            block.set_id(uuid::Uuid::new_v4().to_string());
        }
        self.save(&copy)
    }

    pub fn rename(&self, id: &str, title: &str) -> Result<()> {
        validate_id(id)?;
        validate_title(title)?;
        let changed = self.lock().execute(
            "UPDATE sermons SET title = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, title, (self.clock)()],
        )?;
        found(changed)
    }

    pub fn archive(&self, id: &str) -> Result<()> {
        self.set_archived(id, true)
    }

    pub fn unarchive(&self, id: &str) -> Result<()> {
        self.set_archived(id, false)
    }

    fn set_archived(&self, id: &str, archived: bool) -> Result<()> {
        validate_id(id)?;
        let changed = self.lock().execute(
            "UPDATE sermons SET archived = ?2 WHERE id = ?1",
            params![id, archived],
        )?;
        found(changed)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        validate_id(id)?;
        let changed = self
            .lock()
            .execute("DELETE FROM sermons WHERE id = ?1", [id])?;
        found(changed)
    }

    /// Cópias de segurança do sermão, da mais recente para a mais antiga.
    pub fn versions(&self, sermon_id: &str) -> Result<Vec<VersionInfo>> {
        validate_id(sermon_id)?;
        let conn = self.lock();
        let mut statement = conn.prepare(
            "SELECT id, sermon_id, created_at, title, block_count
             FROM sermon_versions WHERE sermon_id = ?1 ORDER BY id DESC",
        )?;
        let rows = statement.query_map([sermon_id], |row| {
            Ok(VersionInfo {
                id: row.get(0)?,
                sermon_id: row.get(1)?,
                created_at: row.get(2)?,
                title: row.get(3)?,
                block_count: row.get::<_, i64>(4)?.max(0) as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Restaura uma cópia. O estado atual é guardado antes, para a restauração ser desfeita.
    pub fn restore_version(&self, sermon_id: &str, version_id: i64) -> Result<Sermon> {
        validate_id(sermon_id)?;
        let now = (self.clock)();
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let snapshot: String = tx
            .query_row(
                "SELECT snapshot FROM sermon_versions WHERE id = ?1 AND sermon_id = ?2",
                params![version_id, sermon_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(SermonError::NotFound)?;
        let restored: Sermon = serde_json::from_str(&snapshot)
            .map_err(|error| SermonError::Corrupt(error.to_string()))?;
        let current = load_sermon(&tx, sermon_id)?.ok_or(SermonError::NotFound)?;
        restored.validate()?;
        if content_differs(&current, &restored) {
            insert_version(&tx, &current, now)?;
        }
        tx.execute(
            "UPDATE sermons SET title = ?2, base_text = ?3, updated_at = ?4 WHERE id = ?1",
            params![sermon_id, restored.title, restored.base_text, now],
        )?;
        write_blocks(&tx, sermon_id, &restored.blocks, None)?;
        tx.commit()?;
        Ok(Sermon {
            id: current.id,
            created_at: current.created_at,
            updated_at: now,
            archived: current.archived,
            ..restored
        })
    }
}

fn found(changed: usize) -> Result<()> {
    if changed == 0 {
        Err(SermonError::NotFound)
    } else {
        Ok(())
    }
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if current > LATEST_SCHEMA {
        return Err(SermonError::UnsupportedSchema { found: current });
    }
    for target in (current + 1)..=LATEST_SCHEMA {
        let tx = conn.transaction()?;
        tx.execute_batch(MIGRATIONS[(target - 1) as usize])?;
        tx.pragma_update(None, "user_version", target)?;
        tx.commit()?;
    }
    Ok(())
}

fn content_differs(a: &Sermon, b: &Sermon) -> bool {
    a.title != b.title || a.base_text != b.base_text || a.blocks != b.blocks
}

fn load_sermon(conn: &Connection, id: &str) -> Result<Option<Sermon>> {
    let header = conn
        .query_row(
            "SELECT title, base_text, created_at, updated_at, archived FROM sermons WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)? != 0,
                ))
            },
        )
        .optional()?;
    let Some((title, base_text, created_at, updated_at, archived)) = header else {
        return Ok(None);
    };
    let mut statement = conn
        .prepare_cached("SELECT json FROM sermon_blocks WHERE sermon_id = ?1 ORDER BY position")?;
    let blocks = statement
        .query_map([id], |row| row.get::<_, String>(0))?
        .map(|json| {
            let json = json?;
            serde_json::from_str::<Block>(&json)
                .map_err(|error| SermonError::Corrupt(error.to_string()))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(Sermon {
        id: id.to_string(),
        title,
        base_text,
        created_at,
        updated_at,
        archived,
        blocks,
    }))
}

fn write_blocks(
    tx: &Transaction<'_>,
    sermon_id: &str,
    blocks: &[Block],
    fail_after: Option<usize>,
) -> Result<()> {
    tx.execute(
        "DELETE FROM sermon_blocks WHERE sermon_id = ?1",
        [sermon_id],
    )?;
    let mut insert = tx.prepare_cached(
        "INSERT INTO sermon_blocks (sermon_id, position, block_id, kind, json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for (position, block) in blocks.iter().enumerate() {
        if fail_after == Some(position) {
            return Err(injected_failure());
        }
        let json = serde_json::to_string(block)
            .map_err(|error| SermonError::Corrupt(error.to_string()))?;
        insert.execute(params![
            sermon_id,
            position as i64,
            block.id(),
            block.kind(),
            json
        ])?;
    }
    Ok(())
}

#[cfg(test)]
fn injected_failure() -> SermonError {
    SermonError::Injected
}

#[cfg(not(test))]
fn injected_failure() -> SermonError {
    SermonError::Corrupt("falha simulada fora de testes".to_string())
}

/// Guarda o estado anterior se a última cópia tem mais de um minuto.
fn backup_if_due(tx: &Transaction<'_>, previous: &Sermon, now: i64) -> Result<()> {
    let last: Option<i64> = tx.query_row(
        "SELECT MAX(created_at) FROM sermon_versions WHERE sermon_id = ?1",
        [&previous.id],
        |row| row.get(0),
    )?;
    if last.is_some_and(|last| now.saturating_sub(last) < BACKUP_INTERVAL_MS) {
        return Ok(());
    }
    insert_version(tx, previous, now)
}

fn insert_version(tx: &Transaction<'_>, sermon: &Sermon, now: i64) -> Result<()> {
    let snapshot =
        serde_json::to_string(sermon).map_err(|error| SermonError::Corrupt(error.to_string()))?;
    tx.execute(
        "INSERT INTO sermon_versions (sermon_id, created_at, title, block_count, snapshot)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            sermon.id,
            now,
            sermon.title,
            sermon.blocks.len() as i64,
            snapshot
        ],
    )?;
    tx.execute(
        "DELETE FROM sermon_versions WHERE sermon_id = ?1 AND id NOT IN (
             SELECT id FROM sermon_versions WHERE sermon_id = ?1 ORDER BY id DESC LIMIT ?2)",
        params![sermon.id, MAX_VERSIONS as i64],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    use rhema_core::{Book, VerseRange, VerseRef};
    use tempfile::TempDir;

    use super::*;

    struct Fixture {
        _dir: TempDir,
        path: std::path::PathBuf,
        clock: Arc<AtomicI64>,
        store: SermonStore,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().expect("pasta temporária");
        let path = dir.path().join("dados").join("rhema.db");
        let clock = Arc::new(AtomicI64::new(1_000_000));
        let shared = clock.clone();
        let store =
            SermonStore::open_with_clock(&path, Box::new(move || shared.load(Ordering::SeqCst)))
                .expect("abrir");
        Fixture {
            _dir: dir,
            path,
            clock,
            store,
        }
    }

    fn text(id: usize, markdown: &str) -> Block {
        Block::Text {
            id: format!("b{id}"),
            markdown: markdown.to_string(),
        }
    }

    fn verse_block(id: &str) -> Block {
        Block::Verse {
            id: id.to_string(),
            range: VerseRange::single(VerseRef::new(Book::John, 3, 16)),
            module_id: "mod-x".to_string(),
            module_abbr: "XYZ".to_string(),
            snapshot: "Texto inventado do versículo.".to_string(),
        }
    }

    fn sample_blocks() -> Vec<Block> {
        vec![
            Block::Heading {
                id: "h1".into(),
                level: 2,
                text: "Título ✝".into(),
            },
            text(1, "Parágrafo com **negrito** e emoji 🙏"),
            verse_block("v1"),
            Block::Lexicon {
                id: "l1".into(),
                strong_id: "G25".into(),
                module_id: "dict-x".into(),
                title: "amar".into(),
                snapshot: "Definição inventada.".into(),
            },
            Block::Note {
                id: "n1".into(),
                markdown: "nota".into(),
            },
            Block::Illustration {
                id: "i1".into(),
                markdown: "história inventada".into(),
            },
        ]
    }

    #[test]
    fn crud_roundtrip_with_all_block_types() {
        let f = fixture();
        let mut sermon = f.store.create("Esperança 🌱", "Jo 3:16-17").expect("criar");
        sermon.blocks = sample_blocks();
        f.clock.store(2_000_000, Ordering::SeqCst);
        let saved = f.store.save(&sermon).expect("salvar");
        assert_eq!(saved.updated_at, 2_000_000);
        let loaded = f.store.get(&sermon.id).expect("ler");
        assert_eq!(loaded, saved);
        assert_eq!(loaded.created_at, 1_000_000);
        assert_eq!(loaded.blocks.len(), 6);

        f.store
            .rename(&sermon.id, "Outro título")
            .expect("renomear");
        assert_eq!(f.store.get(&sermon.id).expect("ler").title, "Outro título");
        f.store.delete(&sermon.id).expect("apagar");
        assert!(matches!(
            f.store.get(&sermon.id),
            Err(SermonError::NotFound)
        ));
        assert!(matches!(
            f.store.delete(&sermon.id),
            Err(SermonError::NotFound)
        ));
        assert!(matches!(
            f.store.rename(&sermon.id, "x"),
            Err(SermonError::NotFound)
        ));
        assert!(matches!(
            f.store.archive(&sermon.id),
            Err(SermonError::NotFound)
        ));
    }

    #[test]
    fn list_filters_archived_counts_blocks_and_searches_titles() {
        let f = fixture();
        let mut a = f.store.create("Graça Maravilhosa", "").expect("criar");
        a.blocks = sample_blocks();
        f.store.save(&a).expect("salvar");
        let b = f.store.create("Fé e obras", "Tg 2").expect("criar");
        f.store.archive(&b.id).expect("arquivar");

        let active = f.store.list(false).expect("listar");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].block_count, 6);
        assert_eq!(f.store.list(true).expect("listar").len(), 2);
        assert_eq!(
            f.store
                .search_by_title("graça", false)
                .expect("buscar")
                .len(),
            1
        );
        assert_eq!(
            f.store.search_by_title("FÉ", false).expect("buscar").len(),
            0
        );
        assert_eq!(
            f.store.search_by_title("FÉ", true).expect("buscar").len(),
            1
        );
        assert_eq!(
            f.store.search_by_title("  ", true).expect("buscar").len(),
            2
        );

        f.store.unarchive(&b.id).expect("desarquivar");
        assert_eq!(f.store.list(false).expect("listar").len(), 2);
    }

    #[test]
    fn duplicate_copies_content_with_new_ids() {
        let f = fixture();
        let mut original = f.store.create("Modelo", "Sl 23").expect("criar");
        original.blocks = sample_blocks();
        f.store.save(&original).expect("salvar");
        f.store.archive(&original.id).expect("arquivar");
        let copy = f.store.duplicate(&original.id).expect("duplicar");
        assert_eq!(copy.title, "Modelo (cópia)");
        assert_ne!(copy.id, original.id);
        assert!(!copy.archived);
        assert_eq!(copy.blocks.len(), 6);
        assert!(
            copy.blocks
                .iter()
                .all(|block| original.blocks.iter().all(|o| o.id() != block.id()))
        );
        assert_eq!(f.store.get(&copy.id).expect("ler"), copy);

        let long = f.store.create(&"a".repeat(200), "").expect("criar");
        let long_copy = f.store.duplicate(&long.id).expect("duplicar");
        assert_eq!(long_copy.title.chars().count(), 200);
        assert!(long_copy.title.ends_with(COPY_SUFFIX));
    }

    #[test]
    fn limits_and_invalid_input_are_typed_errors() {
        let f = fixture();
        assert!(matches!(
            f.store.create("   ", ""),
            Err(SermonError::TitleEmpty)
        ));
        assert!(matches!(
            f.store.create(&"x".repeat(201), ""),
            Err(SermonError::TitleTooLong { .. })
        ));
        assert!(f.store.create(&"é".repeat(200), "").is_ok());
        assert!(matches!(
            f.store.create("ok", &"x".repeat(201)),
            Err(SermonError::BaseTextTooLong { .. })
        ));

        let mut sermon = f.store.create("Limites", "").expect("criar");
        sermon.blocks = vec![text(0, &"a".repeat(200 * 1024))];
        assert!(matches!(
            f.store.save(&sermon),
            Err(SermonError::BlockTooLarge { index: 0, .. })
        ));
        sermon.blocks = vec![text(0, &"a".repeat(150 * 1024))];
        assert!(f.store.save(&sermon).is_ok());

        sermon.blocks = (0..1001).map(|i| text(i, "x")).collect();
        assert!(matches!(
            f.store.save(&sermon),
            Err(SermonError::TooManyBlocks { .. })
        ));
        sermon.blocks = (0..1000).map(|i| text(i, "x")).collect();
        assert!(f.store.save(&sermon).is_ok());

        sermon.blocks = vec![text(1, "a"), text(1, "b")];
        assert!(matches!(
            f.store.save(&sermon),
            Err(SermonError::DuplicateBlockId)
        ));
        sermon.blocks = vec![Block::Heading {
            id: "h".into(),
            level: 4,
            text: "x".into(),
        }];
        assert!(matches!(
            f.store.save(&sermon),
            Err(SermonError::InvalidBlock { .. })
        ));
        sermon.blocks = vec![Block::Heading {
            id: "h".into(),
            level: 0,
            text: "x".into(),
        }];
        assert!(matches!(
            f.store.save(&sermon),
            Err(SermonError::InvalidBlock { .. })
        ));
        sermon.blocks = vec![text(1, "a")];
        sermon.blocks[0].set_id(String::new());
        assert!(matches!(
            f.store.save(&sermon),
            Err(SermonError::InvalidBlock { .. })
        ));
        sermon.blocks = vec![Block::Verse {
            id: "v".into(),
            range: VerseRange::single(VerseRef::new(Book::John, 99, 1)),
            module_id: String::new(),
            module_abbr: String::new(),
            snapshot: String::new(),
        }];
        assert!(matches!(
            f.store.save(&sermon),
            Err(SermonError::InvalidBlock { .. })
        ));

        assert!(matches!(
            f.store.get("não-é-uuid"),
            Err(SermonError::InvalidId)
        ));
        assert!(matches!(
            f.store.get(&uuid::Uuid::new_v4().to_string()),
            Err(SermonError::NotFound)
        ));
        let mut bad = Sermon::new("x", "", 0);
        bad.id = "123".into();
        assert!(matches!(f.store.save(&bad), Err(SermonError::InvalidId)));
        assert_eq!(SermonError::NotFound.code(), "sermon_not_found");
    }

    #[test]
    fn failed_save_rolls_back_everything() {
        let f = fixture();
        let mut sermon = f.store.create("Original", "").expect("criar");
        sermon.blocks = sample_blocks();
        let stored = f.store.save(&sermon).expect("salvar");

        f.clock.store(9_000_000, Ordering::SeqCst);
        let mut changed = stored.clone();
        changed.title = "Alterado".into();
        changed.blocks = (0..10).map(|i| text(i, "novo")).collect();
        let versions_before = f.store.versions(&sermon.id).expect("versões");
        assert!(matches!(
            f.store.save_inner(&changed, Some(5)),
            Err(SermonError::Injected)
        ));
        assert_eq!(f.store.get(&sermon.id).expect("ler"), stored);
        assert_eq!(
            f.store.versions(&sermon.id).expect("versões"),
            versions_before
        );

        let fresh = Sermon::new("Novo", "", 1);
        let mut fresh_with_blocks = fresh.clone();
        fresh_with_blocks.blocks = vec![text(1, "a"), text(2, "b")];
        assert!(f.store.save_inner(&fresh_with_blocks, Some(1)).is_err());
        assert!(matches!(f.store.get(&fresh.id), Err(SermonError::NotFound)));
    }

    #[test]
    fn backups_are_rate_limited_and_rotated() {
        let f = fixture();
        let mut sermon = f.store.create("Rotação", "").expect("criar");
        for round in 0..3 {
            sermon.blocks = vec![text(0, &format!("rascunho {round}"))];
            sermon = f.store.save(&sermon).expect("salvar");
        }
        // 1ª gravação é criação; a 2ª guarda o estado vazio; a 3ª cai no mesmo minuto.
        assert_eq!(f.store.versions(&sermon.id).expect("versões").len(), 1);

        for round in 0..30 {
            f.clock.fetch_add(61_000, Ordering::SeqCst);
            sermon.blocks = vec![text(0, &format!("versão {round}"))];
            sermon = f.store.save(&sermon).expect("salvar");
        }
        let versions = f.store.versions(&sermon.id).expect("versões");
        assert_eq!(versions.len(), MAX_VERSIONS);
        assert!(versions.windows(2).all(|pair| pair[0].id > pair[1].id));

        // Gravação idêntica não cria cópia.
        f.clock.fetch_add(61_000, Ordering::SeqCst);
        f.store.save(&sermon).expect("salvar");
        assert_eq!(
            f.store.versions(&sermon.id).expect("versões").len(),
            MAX_VERSIONS
        );
    }

    #[test]
    fn the_21st_backup_removes_the_oldest() {
        let f = fixture();
        let mut sermon = f.store.create("Vinte e um", "").expect("criar");
        let mut first_ids = Vec::new();
        for round in 0..=MAX_VERSIONS {
            f.clock.fetch_add(61_000, Ordering::SeqCst);
            sermon.blocks = vec![text(0, &format!("v{round}"))];
            sermon = f.store.save(&sermon).expect("salvar");
            first_ids.push(
                f.store
                    .versions(&sermon.id)
                    .expect("versões")
                    .first()
                    .map(|v| v.id),
            );
        }
        // Cada gravação guarda o estado anterior: 21 cópias criadas, 20 mantidas.
        let versions = f.store.versions(&sermon.id).expect("versões");
        assert_eq!(versions.len(), MAX_VERSIONS);
        let first = first_ids[0].expect("primeira cópia");
        assert!(versions.iter().all(|version| version.id != first));
        assert_eq!(
            versions.last().expect("última").id,
            first_ids[1].expect("segunda cópia")
        );
    }

    #[test]
    fn restore_returns_old_content_and_keeps_current_recoverable() {
        let f = fixture();
        let mut sermon = f.store.create("Versão A", "").expect("criar");
        sermon.blocks = vec![text(0, "conteúdo A")];
        sermon = f.store.save(&sermon).expect("salvar");
        f.clock.fetch_add(61_000, Ordering::SeqCst);
        sermon.title = "Versão B".into();
        sermon.blocks = vec![text(0, "conteúdo B")];
        sermon = f.store.save(&sermon).expect("salvar");

        let versions = f.store.versions(&sermon.id).expect("versões");
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].title, "Versão A");
        let restored = f
            .store
            .restore_version(&sermon.id, versions[0].id)
            .expect("restaurar");
        assert_eq!(restored.title, "Versão A");
        assert_eq!(restored.blocks, vec![text(0, "conteúdo A")]);
        assert_eq!(f.store.get(&sermon.id).expect("ler"), restored);
        let after = f.store.versions(&sermon.id).expect("versões");
        assert_eq!(after.len(), 3);
        assert_eq!(after[0].title, "Versão B");

        assert!(matches!(
            f.store.restore_version(&sermon.id, 9_999),
            Err(SermonError::NotFound)
        ));
        let other = f.store.create("Outro", "").expect("criar");
        assert!(matches!(
            f.store.restore_version(&other.id, versions[0].id),
            Err(SermonError::NotFound)
        ));
    }

    #[test]
    fn migrates_from_schema_v1_keeping_data() {
        let dir = tempfile::tempdir().expect("pasta");
        let path = dir.path().join("rhema.db");
        let id = uuid::Uuid::new_v4().to_string();
        {
            let conn = Connection::open(&path).expect("abrir");
            conn.execute_batch(SCHEMA_V1).expect("esquema 1");
            conn.pragma_update(None, "user_version", 1).expect("versão");
            conn.execute(
                "INSERT INTO sermons (id, title, created_at, updated_at, archived) VALUES (?1, 'Antigo', 5, 6, 0)",
                [&id],
            )
            .expect("inserir");
            let block = serde_json::to_string(&text(1, "texto antigo")).expect("json");
            conn.execute(
                "INSERT INTO sermon_blocks (sermon_id, position, block_id, kind, json) VALUES (?1, 0, 'b1', 'text', ?2)",
                params![id, block],
            )
            .expect("bloco");
        }
        let store = SermonStore::open(&path).expect("migrar");
        assert_eq!(store.schema_version().expect("versão"), LATEST_SCHEMA);
        let sermon = store.get(&id).expect("ler");
        assert_eq!(sermon.title, "Antigo");
        assert_eq!(sermon.base_text, "");
        assert_eq!(sermon.blocks.len(), 1);
        assert!(store.versions(&id).expect("versões").is_empty());
        drop(store);
        assert!(SermonStore::open(&path).is_ok());

        let conn = Connection::open(&path).expect("abrir");
        conn.pragma_update(None, "user_version", 99)
            .expect("versão");
        drop(conn);
        assert!(matches!(
            SermonStore::open(&path),
            Err(SermonError::UnsupportedSchema { found: 99 })
        ));
    }

    #[test]
    fn database_uses_wal_and_foreign_keys_and_cascades() {
        let f = fixture();
        let mut sermon = f.store.create("Cascata", "").expect("criar");
        sermon.blocks = vec![text(0, "a")];
        f.store.save(&sermon).expect("salvar");
        f.clock.fetch_add(61_000, Ordering::SeqCst);
        sermon.blocks = vec![text(0, "b")];
        f.store.save(&sermon).expect("salvar");
        let conn = f.store.lock();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .expect("modo");
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .expect("fk");
        assert_eq!((mode.as_str(), fk), ("wal", 1));
        drop(conn);
        f.store.delete(&sermon.id).expect("apagar");
        let conn = f.store.lock();
        for table in ["sermon_blocks", "sermon_versions"] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .expect("contar");
            assert_eq!(count, 0, "{table}");
        }
        assert!(f.path.exists());
    }

    #[test]
    fn thousand_blocks_save_and_load_timings() {
        let f = fixture();
        let mut sermon = f.store.create("Mil blocos", "").expect("criar");
        sermon.blocks = (0..1000)
            .map(|i| match i % 3 {
                0 => text(i, &format!("Parágrafo inventado número {i}. ").repeat(20)),
                1 => verse_block(&format!("v{i}")),
                _ => Block::Note {
                    id: format!("n{i}"),
                    markdown: "nota curta 🙏".repeat(10),
                },
            })
            .collect();
        let started = std::time::Instant::now();
        let saved = f.store.save(&sermon).expect("salvar");
        let save_ms = started.elapsed().as_secs_f64() * 1000.0;
        f.clock.fetch_add(61_000, Ordering::SeqCst);
        let mut again = saved.clone();
        again.title = "Mil blocos (2)".into();
        let started = std::time::Instant::now();
        f.store.save(&again).expect("salvar de novo");
        let resave_ms = started.elapsed().as_secs_f64() * 1000.0;
        let started = std::time::Instant::now();
        let loaded = f.store.get(&sermon.id).expect("ler");
        let load_ms = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(loaded.blocks.len(), 1000);
        eprintln!(
            "1000 blocos: salvar {save_ms:.1} ms; salvar com cópia {resave_ms:.1} ms; carregar {load_ms:.1} ms"
        );
        if !cfg!(debug_assertions) {
            assert!(
                save_ms < 100.0 && resave_ms < 100.0,
                "salvar {save_ms} / {resave_ms} ms"
            );
            assert!(load_ms < 50.0, "carregar {load_ms} ms");
        }
    }
}
