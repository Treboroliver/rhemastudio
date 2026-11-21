use std::fs::{self, File};
use std::time::Duration;

use rusqlite::{params, Connection};

use super::*;
use crate::build_links;

type Entry<'a> = (&'a str, &'a str, &'a str);

fn rtf(text: &str) -> String {
    let mut out = String::from("{\\rtf1 ");
    for c in text.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            out.push_str(&format!("\\u{}?", c as u32 as u16 as i16));
        }
    }
    out.push('}');
    out
}

fn fixture(path: &Path, entries: &[Entry<'_>], protected: bool) -> Result<(), LexiconError> {
    let db = Connection::open(path)?;
    db.execute_batch(
        "CREATE TABLE config(name TEXT, value TEXT);
         CREATE TABLE topics(id INTEGER,subject TEXT,strong_orig_word TEXT,content_type TEXT);
         CREATE TABLE content(topic_id INTEGER,data BLOB);",
    )?;
    if protected {
        db.execute("INSERT INTO config VALUES ('secure', '1')", [])?;
    }
    for (i, (key, original, body)) in entries.iter().enumerate() {
        db.execute(
            "INSERT INTO topics VALUES (?1,?2,?3,'rtf')",
            params![i as i64, key, original],
        )?;
        db.execute(
            "INSERT INTO content VALUES (?1,?2)",
            params![i as i64, rtf(body).as_bytes()],
        )?;
    }
    Ok(())
}

const STRONG: &[Entry<'static>] = &[
    (
        "G25",
        "ἀγαπάω",
        "G25 ἀγαπάω (agapáō) de 26; querer bem, texto inventado",
    ),
    (
        "G26",
        "ἀγάπη",
        "G26 ἀγάπη (agápē) de 25; afeição fraterna inventada",
    ),
    (
        "G3056",
        "λόγος",
        "G3056 λόγος (lógos) de 3004; dito inventado",
    ),
    (
        "G5485",
        "χάρις",
        "G5485 χάρις (cháris) favor imerecido, graça",
    ),
    (
        "H430",
        "אֱלֹהִים",
        "H430 אֱלֹהִים (’ĕlôhîym) de 433; termo inventado",
    ),
];
const LOUW: &[Entry<'static>] = &[
    ("25", "", "Domínio 25 lista inventada"),
    (
        "25.43",
        "",
        "Domínio: 25 Subdomínio: A (25.1 a 25.9) ἀγαπάω; ἀγάπη Significado: afeição inventada",
    ),
    (
        "25.44",
        "",
        "Domínio: 25 Subdomínio: A (25.1 a 25.9) φιλέω Significado: estima",
    ),
    (
        "26.1",
        "",
        "Domínio: 26 Subdomínio: A (26.1 a 26.9) λόγος Significado: razão",
    ),
    (
        "2.5",
        "",
        "Domínio: 2 Subdomínio: A (2.1 a 2.9) σάρξ Significado: corpo",
    ),
];
const DITAT: &[Entry<'static>] = &[
    (
        "57 אָהַב (’ahab)",
        "",
        "57 אָהַב (’ahab) <H157> querer, inventado. 57a אַהֲבָה (’ahaba) <H160> afeto",
    ),
    (
        "93 אֱלוֹהַּ (’eloah)",
        "",
        "93 אֱלוֹהַּ (’eloah) <H433> <H430> termo inventado",
    ),
];
const DITNT: &[Entry<'static>] = &[
    ("ἀγάπη, ἀγαπάω", "", "verbete inventado sobre amor G26 G25"),
    ("χάρις", "", "favor; graça G5485"),
];
const GLOSSARY: &[Entry<'static>] = &[
    ("Graça", "", "Favor concedido sem mérito, inventado."),
    ("Amor", "", "Sentimento inventado; amores diversos."),
    ("Logos", "", "Termo inventado."),
    ("Vida", "", "amor amor amor citado no corpo"),
];

struct Setup {
    _dir: tempfile::TempDir,
    modules: PathBuf,
    index: PathBuf,
}

fn setup() -> Result<Setup, LexiconError> {
    let dir = tempfile::tempdir()?;
    let modules = dir.path().join("modulos");
    fs::create_dir(&modules)?;
    fixture(&modules.join("strong_amg.dct.twm"), STRONG, false)?;
    fixture(&modules.join("louw-nida.dct.twm"), LOUW, false)?;
    fixture(&modules.join("ditat.dct.twm"), DITAT, false)?;
    fixture(&modules.join("ditnt.dct.twm"), DITNT, false)?;
    fixture(&modules.join("glossario.dct.twm"), GLOSSARY, false)?;
    fixture(&modules.join("protegido.dct.twm"), &[], true)?;
    let index = dir.path().join("indice");
    Ok(Setup {
        _dir: dir,
        modules,
        index,
    })
}

fn build(setup: &Setup) -> Result<IndexBuildReport, LexiconError> {
    build_index(
        &setup.index,
        std::slice::from_ref(&setup.modules),
        &mut |_| {},
    )
}

fn hits(results: &SearchResults) -> Vec<(String, String, MatchField)> {
    results
        .groups
        .iter()
        .flat_map(|g| {
            g.hits
                .iter()
                .map(move |h| (g.module_id.clone(), h.key.clone(), h.field))
        })
        .collect()
}

fn ordered(results: &SearchResults) -> Vec<SearchHit> {
    let mut all: Vec<SearchHit> = results.groups.iter().flat_map(|g| g.hits.clone()).collect();
    all.sort_by(|a, b| b.score.total_cmp(&a.score));
    all
}

fn keys_in(results: &SearchResults, module: &str) -> Vec<String> {
    results
        .groups
        .iter()
        .filter(|g| g.module_id == module)
        .flat_map(|g| g.hits.iter().map(|h| h.key.clone()))
        .collect()
}

#[test]
fn builds_and_reports_modules() -> Result<(), LexiconError> {
    let setup = setup()?;
    let mut events = 0;
    let report = build_index(
        &setup.index,
        std::slice::from_ref(&setup.modules),
        &mut |p| {
            assert!(p.module_index < p.module_count);
            events += 1;
        },
    )?;
    assert_eq!(report.modules.len(), 5);
    assert_eq!(report.protected_ignored, 1);
    assert_eq!(report.unreadable_modules, 0);
    assert!(events >= 10);
    let kinds: BTreeSet<LexKind> = report.modules.iter().map(|m| m.kind).collect();
    assert_eq!(kinds.len(), 5);
    let index = LexiconIndex::open(&setup.index)?;
    assert_eq!(index.metadata().schema_version, INDEX_SCHEMA_VERSION);
    assert_eq!(index.metadata().skipped.len(), 1);
    assert!(!index.status().rebuild_needed);
    assert_eq!(index.resolve_module("GLOSSARIO"), Some("glossario.dct.twm"));
    Ok(())
}

#[test]
fn searches_each_query_type_with_ranking() -> Result<(), LexiconError> {
    let setup = setup()?;
    build(&setup)?;
    let index = LexiconIndex::open(&setup.index)?;
    let none = SearchFilters::default();

    let strong = index.search("G25", 20, &none)?;
    let top = ordered(&strong);
    assert_eq!(
        (top[0].key.as_str(), top[0].field),
        ("G25", MatchField::Key)
    );
    assert!(hits(&strong).contains(&("strong_amg.dct.twm".into(), "G26".into(), MatchField::Refs)));
    assert!(hits(&strong).contains(&(
        "ditnt.dct.twm".into(),
        "ἀγάπη, ἀγαπάω".into(),
        MatchField::Refs
    )));

    let hebrew_number = index.search("h0430", 20, &none)?;
    assert_eq!(ordered(&hebrew_number)[0].key, "H430");
    assert!(keys_in(&hebrew_number, "ditat.dct.twm").contains(&"93 אֱלוֹהַּ (’eloah)".into()));

    let louw = index.search("25.43", 20, &none)?;
    assert_eq!(keys_in(&louw, "louw-nida.dct.twm"), ["25.43"]);

    let domain = index.search("25", 20, &none)?;
    assert_eq!(
        keys_in(&domain, "louw-nida.dct.twm"),
        ["25", "25.43", "25.44"]
    );
    assert_eq!(ordered(&domain)[1].field, MatchField::DomainPrefix);
    let paged = index.search("25", 2, &none)?;
    assert_eq!(paged.total, 3);
    assert_eq!(paged.shown, 2);

    let ditat = index.search("DITAT 57", 20, &none)?;
    assert_eq!(ordered(&ditat)[0].key, "57 אָהַב (’ahab)");
    let sub = index.search("57a", 20, &none)?;
    assert_eq!(ordered(&sub)[0].field, MatchField::Subentry);

    let greek = index.search("ἀγαπάω", 20, &none)?;
    let latin = index.search("agapao", 20, &none)?;
    assert_eq!(greek.groups[0].kind, LexKind::Strong);
    assert_eq!(greek.groups[0].hits[0].key, "G25");
    assert_eq!(latin.groups[0].kind, LexKind::Strong);
    assert_eq!(latin.groups[0].hits[0].key, "G25");
    assert_eq!(keys_in(&greek, "strong_amg.dct.twm"), ["G25"]);
    assert_eq!(keys_in(&latin, "strong_amg.dct.twm"), ["G25"]);
    assert_eq!(keys_in(&greek, "louw-nida.dct.twm"), ["25.43"]);
    assert_eq!(keys_in(&latin, "louw-nida.dct.twm"), ["25.43"]);
    assert!(ordered(&greek).iter().all(|h| h.field == MatchField::Lemma));
    assert!(ordered(&latin)
        .iter()
        .all(|h| h.field == MatchField::Translit));

    let strong_vs_ditnt = index.search("χάρις", 20, &none)?;
    assert_eq!(strong_vs_ditnt.groups[0].kind, LexKind::Ditnt);
    assert_eq!(strong_vs_ditnt.groups[0].hits[0].field, MatchField::Key);
    assert_eq!(strong_vs_ditnt.groups[1].kind, LexKind::Strong);
    assert_eq!(strong_vs_ditnt.groups[1].hits[0].key, "G5485");
    assert_eq!(strong_vs_ditnt.groups[1].hits[0].field, MatchField::Lemma);
    let translit_tie = index.search("charis", 20, &none)?;
    assert_eq!(translit_tie.groups[0].kind, LexKind::Strong);
    assert_eq!(translit_tie.groups[0].hits[0].key, "G5485");

    let prefix = index.search("λογ", 20, &none)?;
    assert!(ordered(&prefix)
        .iter()
        .all(|h| h.field == MatchField::LemmaPrefix));
    assert_eq!(keys_in(&prefix, "louw-nida.dct.twm"), ["26.1"]);

    let unpointed = index.search("אהב", 20, &none)?;
    assert_eq!(keys_in(&unpointed, "ditat.dct.twm"), ["57 אָהַב (’ahab)"]);

    let logos = ordered(&index.search("logos", 20, &none)?);
    assert_eq!(logos[0].field, MatchField::Translit);
    assert!(logos.iter().any(|h| h.field == MatchField::KeyNorm));

    let love = ordered(&index.search("amor", 20, &none)?);
    assert_eq!(
        (love[0].key.as_str(), love[0].field),
        ("Amor", MatchField::KeyNorm)
    );
    assert!(love.iter().skip(1).all(|h| h.field == MatchField::Body));
    assert!(love.iter().any(|h| h.key == "Vida"));
    let plural = ordered(&index.search("amores", 20, &none)?);
    assert_eq!(plural.len(), 1);
    assert_eq!(plural[0].key, "Amor");
    Ok(())
}

#[test]
fn ignores_accents_groups_and_filters() -> Result<(), LexiconError> {
    let setup = setup()?;
    build(&setup)?;
    let index = LexiconIndex::open(&setup.index)?;
    let none = SearchFilters::default();
    for query in ["graca", "Graça", "GRACA"] {
        let results = index.search(query, 20, &none)?;
        assert!(keys_in(&results, "glossario.dct.twm").contains(&"Graça".into()));
        assert!(keys_in(&results, "strong_amg.dct.twm").contains(&"G5485".into()));
        let ids: Vec<&str> = results
            .groups
            .iter()
            .map(|g| g.module_id.as_str())
            .collect();
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
    }
    let only_glossary = SearchFilters {
        modules: vec!["glossario.dct.twm".into()],
        kinds: Vec::new(),
    };
    let filtered = index.search("amor", 20, &only_glossary)?;
    assert_eq!(filtered.groups.len(), 1);
    assert_eq!(filtered.groups[0].kind, LexKind::Generic);
    let only_strong = SearchFilters {
        modules: Vec::new(),
        kinds: vec![LexKind::Strong],
    };
    let filtered = index.search("graca", 20, &only_strong)?;
    assert!(filtered.groups.iter().all(|g| g.kind == LexKind::Strong));
    let json = serde_json::to_string(&index.search("amor", 20, &none)?)
        .map_err(|e| LexiconError::Invalid(e.to_string()))?;
    assert!(!json.contains("inventad") && !json.contains("Sentimento"));
    assert_eq!(index.search("", 20, &none)?.total, 0);
    assert_eq!(index.search("@!", 20, &none)?.total, 0);
    Ok(())
}

#[test]
fn uses_links_with_provenance() -> Result<(), LexiconError> {
    let setup = setup()?;
    build(&setup)?;
    let links = setup.index.with_file_name("ligacoes.sqlite");
    build_links(&links, std::slice::from_ref(&setup.modules))?;
    let index = LexiconIndex::open(&setup.index)?.with_links(&links);
    let results = index.search("G25", 20, &SearchFilters::default())?;
    let linked: Vec<SearchHit> = ordered(&results)
        .into_iter()
        .filter(|h| h.field == MatchField::Link)
        .collect();
    assert!(linked
        .iter()
        .any(|h| h.key == "25.43" && h.provenance.as_deref() == Some("lema no cabeçalho")));
    assert!(linked.iter().all(|h| h.provenance.is_some()));
    assert!(results.notes.is_empty());
    Ok(())
}

#[test]
fn detects_new_changed_and_removed_modules() -> Result<(), LexiconError> {
    let setup = setup()?;
    build(&setup)?;
    let index = LexiconIndex::open(&setup.index)?;
    std::thread::sleep(Duration::from_millis(20));
    let louw = setup.modules.join("louw-nida.dct.twm");
    File::options()
        .write(true)
        .open(&louw)?
        .set_modified(SystemTime::now() + Duration::from_secs(5))?;
    let status = index.status();
    assert!(status.rebuild_needed);
    assert_eq!(status.changes.len(), 1);
    assert_eq!(status.changes[0].change, ChangeKind::Changed);
    assert!(status
        .message()
        .is_some_and(|m| m.contains("reconstrução necessária")));
    let db = Connection::open(setup.modules.join("glossario.dct.twm"))?;
    db.execute_batch("CREATE TABLE extra(x BLOB); INSERT INTO extra VALUES (zeroblob(9000));")?;
    drop(db);
    fs::remove_file(setup.modules.join("ditnt.dct.twm"))?;
    fixture(&setup.modules.join("novo.dct.twm"), GLOSSARY, false)?;
    let status = index.status();
    let count = |kind| status.changes.iter().filter(|c| c.change == kind).count();
    assert_eq!(count(ChangeKind::Changed), 2);
    assert_eq!(count(ChangeKind::Removed), 1);
    assert_eq!(count(ChangeKind::New), 1);
    drop(index);
    build(&setup)?;
    assert!(!LexiconIndex::open(&setup.index)?.status().rebuild_needed);
    Ok(())
}

#[test]
fn detects_change_recorded_in_metadata_and_schema() -> Result<(), LexiconError> {
    let setup = setup()?;
    build(&setup)?;
    let generation = fs::read_to_string(setup.index.join(POINTER_FILE))?;
    let meta_path = setup.index.join(generation.trim()).join(METADATA_FILE);
    let edit = |change: &dyn Fn(&mut serde_json::Value)| -> Result<(), LexiconError> {
        let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&meta_path)?)
            .map_err(|e| LexiconError::Invalid(e.to_string()))?;
        change(&mut value);
        fs::write(&meta_path, value.to_string())?;
        Ok(())
    };
    edit(&|v| v["modules"][0]["size"] = serde_json::json!(1))?;
    let status = LexiconIndex::open(&setup.index)?.status();
    assert_eq!(status.changes.len(), 1);
    assert_eq!(status.changes[0].change, ChangeKind::Changed);
    edit(&|v| v["schema_version"] = serde_json::json!(999))?;
    assert!(matches!(
        LexiconIndex::open(&setup.index),
        Err(LexiconError::RebuildRequired(_))
    ));
    Ok(())
}

#[test]
fn failed_rebuild_keeps_previous_index() -> Result<(), LexiconError> {
    let setup = setup()?;
    build(&setup)?;
    let pointer = fs::read_to_string(setup.index.join(POINTER_FILE))?;
    let target = setup.modules.join("ditat.dct.twm");
    let mut touched = false;
    let failed = build_index(
        &setup.index,
        std::slice::from_ref(&setup.modules),
        &mut |p| {
            if p.module_index == 1 && !touched {
                touched = true;
                let _ = File::options()
                    .write(true)
                    .open(&target)
                    .and_then(|f| f.set_modified(SystemTime::now() + Duration::from_secs(7)));
            }
        },
    );
    assert!(failed.is_err());
    assert_eq!(fs::read_to_string(setup.index.join(POINTER_FILE))?, pointer);
    let generations = fs::read_dir(&setup.index)?
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(GENERATION_PREFIX)
        })
        .count();
    assert_eq!(generations, 1);
    let index = LexiconIndex::open(&setup.index)?;
    assert_eq!(
        index.search("G25", 5, &SearchFilters::default())?.groups[0].hits[0].key,
        "G25"
    );
    Ok(())
}

#[test]
fn index_stays_outside_repository() -> Result<(), LexiconError> {
    let setup = setup()?;
    let repo = setup
        .modules
        .parent()
        .map(|p| p.join("repo"))
        .unwrap_or_default();
    fs::create_dir_all(repo.join(".git"))?;
    let inside = repo.join("alvo").join("indice");
    assert!(build_index(&inside, std::slice::from_ref(&setup.modules), &mut |_| {}).is_err());
    assert!(!inside.exists());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let workspace = workspace.canonicalize()?;
    if let Some(default) = default_index_dir() {
        assert!(!default.starts_with(&workspace));
    }
    Ok(())
}

#[test]
fn bench_queries_are_varied_and_parseable() {
    let queries = bench_queries(200);
    assert_eq!(queries.len(), 200);
    assert!(queries.iter().all(|q| !parse_query(q).is_empty()));
    let unique: HashSet<&String> = queries.iter().collect();
    assert!(unique.len() > 100);
}

#[test]
fn natural_order_of_keys() {
    let mut keys = vec!["25.10", "25.9", "25", "3", "25.100"];
    keys.sort_by(|a, b| natural_cmp(a, b));
    assert_eq!(keys, ["3", "25", "25.9", "25.10", "25.100"]);
}

#[test]
fn indexes_html_module_and_searches_body() -> Result<(), LexiconError> {
    let dir = tempfile::tempdir()?;
    let modules = dir.path().join("modulos");
    fs::create_dir(&modules)?;
    let path = modules.join("enciclopedia_html.dct.twm");
    let db = Connection::open(&path)?;
    db.execute_batch(
        "CREATE TABLE config(name TEXT, value TEXT);
         CREATE TABLE topics(id INTEGER,subject TEXT,strong_orig_word TEXT,content_type TEXT);
         CREATE TABLE content(topic_id INTEGER,data BLOB);",
    )?;
    db.execute("INSERT INTO config VALUES ('content.type', 'html')", [])?;
    let html = b"<style>.x{color:red}</style><script>var oculto='segredo';</script>\
        <h1>Verbete</h1><p>A esperan&ccedil;a e a f&#233; inventadas.</p>";
    db.execute("INSERT INTO topics VALUES (1,'Esperanca','',NULL)", [])?;
    db.execute(
        "INSERT INTO content VALUES (1,?1)",
        params![html.as_slice()],
    )?;
    drop(db);

    let index_dir = dir.path().join("indice");
    let report = build_index(&index_dir, std::slice::from_ref(&modules), &mut |_| {})?;
    assert_eq!(report.modules[0].unreadable, 0);
    let index = LexiconIndex::open(&index_dir)?;
    let none = SearchFilters::default();
    assert_eq!(index.search("esperanca", 10, &none)?.total, 1);
    assert_eq!(index.search("fe", 10, &none)?.total, 1);
    assert_eq!(index.search("segredo", 10, &none)?.total, 0);
    Ok(())
}
