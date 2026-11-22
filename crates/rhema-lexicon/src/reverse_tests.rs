use std::fs::{self, File};
use std::time::{Duration, SystemTime};

use rusqlite::{params, Connection};

use super::*;
use crate::{build_index, LexKind, LexiconIndex, MatchField, SearchFilters};

fn write_ont(path: &Path, lines: &[&str]) -> Result<(), LexiconError> {
    let mut content = lines.join("\n");
    content.push('\n');
    let mut all = content;
    let total = 31102;
    let current = lines.len();
    for _ in current..total {
        all.push('\n');
    }
    all.push_str("title=Biblia Sintetica\ncharset=0\n");
    fs::write(path, all.as_bytes())?;
    Ok(())
}

fn write_strong_dict(path: &Path, entries: &[(&str, &str, &str)]) -> Result<(), LexiconError> {
    let db = Connection::open(path)?;
    db.execute_batch(
        "CREATE TABLE config(name TEXT, value TEXT);
         CREATE TABLE topics(id INTEGER,subject TEXT,strong_orig_word TEXT,content_type TEXT);
         CREATE TABLE content(topic_id INTEGER,data BLOB);",
    )?;
    for (i, (key, orig, body)) in entries.iter().enumerate() {
        db.execute(
            "INSERT INTO topics VALUES (?1,?2,?3,'txt')",
            params![i as i64, key, orig],
        )?;
        db.execute(
            "INSERT INTO content VALUES (?1,?2)",
            params![i as i64, body.as_bytes()],
        )?;
    }
    Ok(())
}

#[test]
fn tokenizes_latin_words_with_internal_punctuation() {
    assert_eq!(
        tokenize_latin_words("O amor d'água e bem-estar 123 a x-y"),
        vec!["amor", "d'agua", "bem-estar", "x-y"]
    );
    assert_eq!(tokenize_latin_words("1234 ' - a b"), Vec::<String>::new());
}

#[test]
fn builds_reverse_index_sums_modules_and_applies_dice_rules() -> Result<(), LexiconError> {
    let dir = tempfile::tempdir()?;
    let bibles = dir.path().join("biblias");
    fs::create_dir(&bibles)?;

    write_ont(
        &bibles.join("biblia1.ont"),
        &[
            // Trecho com várias palavras, palavra com duas tags, palavra comum "comum"
            "comum texto<WG10> palavra<WG25><WG26> rara<WG99>",
            "comum termo<WG10> palavra<WG25> outro<WG2>",
            "comum exemplo<WG3>",
            "comum algo<WG4>",
        ],
    )?;
    write_ont(
        &bibles.join("biblia2.ont"),
        &[
            "palavra<WG25> comum coisa<WG5>",
            "unica<WG77>",
            "sem tag alguma aqui",
        ],
    )?;
    // Bíblia sem tags Strong e módulo protegido devem ser ignorados
    write_ont(&bibles.join("sem_strong.ont"), &["verso comum sem tags"])?;
    fs::write(bibles.join("protegida.ontx"), b"encrypted")?;

    let cache = dir.path().join("reverse.sqlite");
    let report = build_reverse_index(&cache, std::slice::from_ref(&bibles))?;
    assert_eq!(report.bibles.len(), 2);
    assert_eq!(report.protected_ignored, 1);

    let rev = ReverseIndex::open(&cache)?;
    // "palavra" aparece 3x (biblia1: 2x com G25 e 1x com G26; biblia2: 1x com G25)
    let sug = rev.suggest_strongs("palavra", 10)?;
    assert_eq!(sug.len(), 1); // G26 tem n_ws=1 e é descartado porque há >1 candidato
    assert_eq!(sug[0].strong, "G25");
    assert_eq!(sug[0].n_ws, 3);
    assert_eq!(sug[0].n_w, 3);
    assert_eq!(sug[0].n_s, 3);
    assert!((sug[0].dice - 1.0).abs() < 1e-6);
    assert_eq!(sug[0].modules, vec!["biblia1.ont", "biblia2.ont"]);

    // "unica" tem apenas 1 acerto (n_ws=1) e deve ser mantida por ser única
    let sole = rev.suggest_strongs("unica", 10)?;
    assert_eq!(sole.len(), 1);
    assert_eq!(sole[0].strong, "G77");
    assert_eq!(sole[0].n_ws, 1);

    // "comum" aparece em muitos trechos -> Dice baixo
    let common = rev.suggest_strongs("comum", 10)?;
    assert!(common.is_empty() || common[0].dice < 0.5);
    Ok(())
}

#[test]
fn detects_stale_modules_keeps_atomic_and_rejects_repo() -> Result<(), LexiconError> {
    let dir = tempfile::tempdir()?;
    let bibles = dir.path().join("biblias");
    fs::create_dir(&bibles)?;
    let b1 = bibles.join("b1.ont");
    write_ont(&b1, &["alfa<WG1> alfa<WG1> beta<WG2> beta<WG2>"])?;

    let cache = dir.path().join("reverse.sqlite");
    build_reverse_index(&cache, std::slice::from_ref(&bibles))?;
    assert_eq!(suggest_strongs(&cache, "alfa", 5)?[0].strong, "G1");

    // Invalidação por alteração de arquivo
    std::thread::sleep(Duration::from_millis(20));
    File::options()
        .write(true)
        .open(&b1)?
        .set_modified(SystemTime::now() + Duration::from_secs(5))?;
    assert!(matches!(
        ReverseIndex::open(&cache),
        Err(LexiconError::RebuildRequired(_))
    ));

    // Falha na reconstrução não deixa arquivo parcial nem altera o existente
    let missing = dir.path().join("inexistente");
    assert!(build_reverse_index(&cache, &[missing]).is_err());
    let leftovers: Vec<_> = fs::read_dir(dir.path())?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
        .collect();
    assert!(leftovers.is_empty());

    // Rejeita gravar dentro do repositório
    let repo = dir.path().join("repo");
    fs::create_dir_all(repo.join(".git"))?;
    assert!(build_reverse_index(repo.join("reverse.sqlite"), &[bibles]).is_err());
    Ok(())
}

#[test]
fn integrates_reverse_suggestions_into_lexicon_search() -> Result<(), LexiconError> {
    let dir = tempfile::tempdir()?;
    let mods = dir.path().join("modulos");
    fs::create_dir(&mods)?;

    write_ont(
        &mods.join("biblia.ont"),
        &[
            "carinho<WG25> carinho<WG25> outro<WG99>",
            "carinho<WG25> texto<WG98>",
        ],
    )?;
    write_strong_dict(
        &mods.join("strong_amg.dct.twm"),
        &[("G25", "ἀγαπάω", "querer bem inventado")],
    )?;
    write_strong_dict(
        &mods.join("louw-nida.dct.twm"),
        &[(
            "25.43",
            "",
            "Domínio: 25 Subdomínio: A (25.1 a 25.9) ἀγαπάω Significado: afeição",
        )],
    )?;
    write_strong_dict(
        &mods.join("glossario.dct.twm"),
        &[("Outro", "", "verbete com carinho no corpo")],
    )?;

    let index_dir = dir.path().join("indice");
    let rev_path = dir.path().join("reverse.sqlite");
    let links_path = dir.path().join("links.sqlite");
    build_index(&index_dir, std::slice::from_ref(&mods), &mut |_| {})?;
    build_reverse_index(&rev_path, std::slice::from_ref(&mods))?;
    crate::build_links(&links_path, std::slice::from_ref(&mods))?;

    let index = LexiconIndex::open(&index_dir)?
        .with_links(&links_path)
        .with_reverse(&rev_path);
    let results = index.search("carinho", 20, &SearchFilters::default())?;
    assert_eq!(results.groups.len(), 3);
    assert_eq!(results.groups[0].module_id, "sugestões por palavra");
    assert_eq!(results.groups[0].kind, LexKind::Strong);
    assert_eq!(results.groups[0].hits[0].key, "G25");
    assert_eq!(results.groups[0].hits[0].field, MatchField::ReverseIndex);
    assert!(results.groups[0].hits[0]
        .provenance
        .as_deref()
        .is_some_and(|p| p.starts_with("reverse_index")));
    assert_eq!(results.groups[1].module_id, "ligados a G25");
    assert_eq!(results.groups[1].hits[0].key, "25.43");
    assert_eq!(results.groups[1].hits[0].field, MatchField::Link);
    assert_eq!(results.groups[2].module_id, "glossario.dct.twm");
    assert_eq!(results.groups[2].hits[0].field, MatchField::Body);
    Ok(())
}
