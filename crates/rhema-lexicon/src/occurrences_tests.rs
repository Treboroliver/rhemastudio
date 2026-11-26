use std::fs;
use std::path::Path;

use tempfile::tempdir;

use super::*;

fn write_synthetic_nt(path: &Path, title: &str, lang: &str, verses: &[(usize, &str)]) {
    let mut lines: Vec<String> = vec![".".to_string(); 7957];
    for &(idx, text) in verses {
        lines[idx] = text.to_string();
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out.push_str(&format!("title={title}\n"));
    out.push_str("short.title=SINT\n");
    if !lang.is_empty() {
        out.push_str(&format!("lang={lang}\n"));
    }
    out.push_str("charset=65001\n");
    fs::write(path, out).unwrap();
}

#[test]
fn validates_strong_numbers_and_pages() {
    assert_eq!(validate_strong_number("G25").unwrap(), "G25");
    assert_eq!(validate_strong_number(" h0430 ").unwrap(), "H430");
    assert_eq!(validate_strong_number("G99999").unwrap(), "G99999");

    for bad in ["", "25", "X25", "G0", "H000", "G123456", "G25a", "G-1"] {
        assert!(
            validate_strong_number(bad).is_err(),
            "esperava erro para {bad:?}"
        );
    }
}

#[test]
fn indexes_single_phrase_multi_tags_repeats_and_language_filter() {
    let dir = tempdir().unwrap();
    let mods = dir.path().join("mods");
    fs::create_dir(&mods).unwrap();

    // Bíblia sintética portuguesa (textos totalmente inventados)
    write_synthetic_nt(
        &mods.join("sint_pt.nt"),
        "Sintética PT",
        "pt",
        &[
            // Mt 1:1 (idx 0): palavra única, repetição do mesmo Strong (n=4) e número/1-letra ignorados
            (0, "luz<WG25> a<WG25> 123<WG25> luz<WG25> forte"),
            // Mt 1:2 (idx 1): tag dentro de itálico e múltiplas tags na mesma palavra
            (1, "<i>grande estrela</i><WG3056> vento<WG25><WG26> calmo"),
            // Mt 1:3 (idx 2): acento dobrado (fold_latin) com repetição
            (2, "ação<WG25> ação<WG25> vento<WG25> clara"),
        ],
    );

    // Bíblia sintética grega: só entra em occurrences, não em translations
    write_synthetic_nt(
        &mods.join("sint_grc.nt"),
        "Sintética GRC",
        "grc",
        &[(0, "αβγδ<WG25> latim<WG25>"), (1, "εζηθ<WG3056>")],
    );

    // Bíblia sintética em inglês: fora dos idiomas ativos -> ignorada e listada no relatório
    write_synthetic_nt(
        &mods.join("sint_en.nt"),
        "Synthetic EN",
        "en",
        &[(0, "bright<WG25> star<WG3056>")],
    );

    // Módulo protegido: deve ser ignorado sem leitura
    fs::write(mods.join("bloqueada.ontx"), b"fake protected payload").unwrap();

    let db_path = dir.path().join("occ.sqlite");
    let report = build_occurrences_index(&db_path, std::slice::from_ref(&mods)).unwrap();

    assert_eq!(report.protected_ignored, 1);
    assert_eq!(report.bibles.len(), 2);
    assert_eq!(report.ignored_by_language.len(), 1);
    assert_eq!(report.ignored_by_language[0].id, "sint_en.nt");
    assert_eq!(report.ignored_by_language[0].lang, "eng");

    // Sem arquivo temporário residual após troca atômica
    let leftovers: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
        .collect();
    assert!(leftovers.is_empty());

    let idx = OccurrencesIndex::open(&db_path).unwrap();

    // Página inválida rejeitada
    assert!(idx.occurrences("G25", None, 0, 50).is_err());
    assert!(idx.occurrences("G25", None, 201, 50).is_err());

    // G25 na Bíblia PT (preferida por ser 'por'):
    // idx 0 -> 4 tags G25 (n=4), idx 1 -> 1 tag G25 (n=1), idx 2 -> 3 tags G25 (n=3) => 3 versículos, 8 ocorrências
    let res = idx.occurrences("G25", Some("sint_pt.nt"), 1, 2).unwrap();
    assert_eq!(res.strong, "G25");
    assert_eq!(res.selected_module_id.as_deref(), Some("sint_pt.nt"));
    assert_eq!(res.total_verses, 3);
    assert_eq!(res.total_occurrences, 8);
    assert_eq!(res.by_book.len(), 1);
    assert_eq!(res.by_book[0].book, 40);
    assert_eq!(res.by_book[0].verses, 3);
    assert_eq!(res.by_book[0].occurrences, 8);
    assert_eq!(res.total_pages, 2);
    assert_eq!(res.items.len(), 2);
    assert_eq!(
        res.items[0],
        VerseOccurrence {
            book: 40,
            chapter: 1,
            verse: 1,
            n: 4
        }
    );

    let page2 = idx.occurrences("G25", Some("sint_pt.nt"), 2, 2).unwrap();
    assert_eq!(page2.items.len(), 1);
    assert_eq!(page2.items[0].verse, 3);

    // Traduções de G25 na Bíblia PT: 'a' (1 letra) e '123' (número) ignorados; 'ação' -> 'acao'
    let tr_pt = idx.translations("G25", Some("sint_pt.nt"), 10).unwrap();
    let words: Vec<&str> = tr_pt.items.iter().map(|i| i.word.as_str()).collect();
    assert!(words.contains(&"luz"));
    assert!(words.contains(&"vento"));
    assert!(words.contains(&"acao"));
    assert!(!words.contains(&"a"));
    assert!(!words.contains(&"123"));

    // Tag em palavra dentro de itálico captura a palavra etiquetada
    let tr_3056 = idx.translations("G3056", Some("sint_pt.nt"), 10).unwrap();
    let w3056: Vec<&str> = tr_3056.items.iter().map(|i| i.word.as_str()).collect();
    assert_eq!(w3056, vec!["estrela"]);

    // Trecho multi-palavra sob um único Node::Strong captura todas as palavras do trecho
    let phrase_nodes = vec![Node::Strong {
        lang: StrongLang::Greek,
        num: 3056,
        children: vec![Node::Text("grande estrela 99 a".into())],
    }];
    let mut active = Vec::new();
    let mut verse_strongs = HashMap::new();
    let mut phrase_trans = HashMap::new();
    collect_occurrence_nodes(
        &phrase_nodes,
        true,
        &mut active,
        &mut verse_strongs,
        &mut phrase_trans,
    );
    assert_eq!(verse_strongs.get("G3056"), Some(&1));
    assert_eq!(
        phrase_trans.get(&("G3056".into(), "grande".into())),
        Some(&1)
    );
    assert_eq!(
        phrase_trans.get(&("G3056".into(), "estrela".into())),
        Some(&1)
    );
    assert!(!phrase_trans.contains_key(&("G3056".into(), "99".into())));
    assert!(!phrase_trans.contains_key(&("G3056".into(), "a".into())));

    // Bíblia grega tem ocorrências mas não contribui para translations
    let occ_grc = idx.occurrences("G25", Some("sint_grc.nt"), 1, 50).unwrap();
    assert_eq!(occ_grc.total_verses, 1);
    assert_eq!(occ_grc.total_occurrences, 2);
    let tr_grc = idx.translations("G25", Some("sint_grc.nt"), 10).unwrap();
    assert_eq!(tr_grc.total, 0);
    assert!(tr_grc.items.is_empty());

    // Strong sem ocorrências
    let empty = idx.occurrences("H999", Some("sint_pt.nt"), 1, 50).unwrap();
    assert_eq!(empty.total_verses, 0);
    assert_eq!(empty.total_occurrences, 0);
    assert!(empty.items.is_empty());
}

#[test]
fn invalidates_when_module_changes_and_builds_together_with_reverse() {
    let dir = tempdir().unwrap();
    let mods = dir.path().join("mods");
    fs::create_dir(&mods).unwrap();
    let bible = mods.join("sint.nt");
    write_synthetic_nt(
        &bible,
        "Sintética",
        "por",
        &[(0, "pedra<WG25> forte"), (1, "pedra<WG25> alta")],
    );

    let rev_path = dir.path().join("reverse.sqlite");
    let occ_path = dir.path().join("occurrences.sqlite");
    let (rev_res, occ_rep) =
        build_reverse_and_occurrences(&rev_path, &occ_path, std::slice::from_ref(&mods)).unwrap();
    assert!(rev_res.is_ok());
    assert_eq!(occ_rep.bibles.len(), 1);
    assert!(OccurrencesIndex::open(&occ_path).is_ok());

    // Altera o módulo -> deve invalidar o índice
    write_synthetic_nt(
        &bible,
        "Sintética Alterada",
        "por",
        &[(0, "pedra<WG25> nova diferente")],
    );
    let err = OccurrencesIndex::open(&occ_path).err().unwrap();
    assert!(matches!(err, LexiconError::RebuildRequired(_)));
}

#[test]
fn ranks_translations_by_dice_and_filters_short_or_singleton_words() {
    let dir = tempdir().unwrap();
    let mods = dir.path().join("mods");
    fs::create_dir(&mods).unwrap();

    // Palavra de ligação "que" aparece muitas vezes em outros Strongs (n_w alto) e 3x em G25,
    // enquanto "brilha" aparece 2x exclusivamente em G25 -> Dice de "brilha" deve vencer "que".
    // Palavra de 2 letras ("de") é descartada; palavra com n_ws=1 ("rara") é descartada quando há outras,
    // mas mantida quando é a única (G99 -> "unico").
    let mut verses: Vec<(usize, String)> = vec![
        (0, "que<WG25> de<WG25> brilha<WG25>".into()),
        (1, "que<WG25> de<WG25> brilha<WG25>".into()),
        (2, "que<WG25> de<WG25> rara<WG25>".into()),
        (3, "unico<WG99>".into()),
    ];
    for i in 4..24 {
        verses.push((i, "que<WG100>".into()));
    }
    let refs: Vec<(usize, &str)> = verses.iter().map(|(i, s)| (*i, s.as_str())).collect();
    write_synthetic_nt(&mods.join("dice_pt.nt"), "Dice PT", "pt", &refs);

    let db_path = dir.path().join("occ-dice.sqlite");
    build_occurrences_index(&db_path, std::slice::from_ref(&mods)).unwrap();
    let idx = OccurrencesIndex::open(&db_path).unwrap();

    let tr = idx.translations("G25", Some("dice_pt.nt"), 10).unwrap();
    let words: Vec<&str> = tr.items.iter().map(|i| i.word.as_str()).collect();
    assert_eq!(words, vec!["brilha", "que"]);
    assert_eq!(tr.items[0].n, 2);
    assert_eq!(tr.items[1].n, 3);

    // Quando há apenas uma candidata com n_ws = 1, ela é mantida
    let tr_single = idx.translations("G99", Some("dice_pt.nt"), 10).unwrap();
    assert_eq!(tr_single.items.len(), 1);
    assert_eq!(tr_single.items[0].word, "unico");
    assert_eq!(tr_single.items[0].n, 1);
}

#[test]
#[ignore = "verificação E2a com módulos reais via RHEMA_TEST_MODULES"]
fn real_modules_e2a_verification() {
    let Ok(root) = std::env::var("RHEMA_TEST_MODULES") else {
        return;
    };
    let root = PathBuf::from(root);
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("occ-real.sqlite");

    let report = build_occurrences_index(&db_path, &[root]).unwrap();
    println!(
        "BUILD: scan={}ms build={}ms bytes={} protected={} unreadable={}",
        report.scan_ms,
        report.build_ms,
        report.database_bytes,
        report.protected_ignored,
        report.unreadable_modules
    );
    for b in &report.bibles {
        println!(
            "BIBLE: {} | lang={} | vS={} | occ_rows={} | tr_rows={} | {}ms",
            b.id, b.lang, b.verses_with_strong, b.occurrence_rows, b.translation_rows, b.ms
        );
    }
    for ig in &report.ignored_by_language {
        println!("IGNORED_LANG: {} | lang={}", ig.id, ig.lang);
    }

    let t_open = Instant::now();
    let idx = OccurrencesIndex::open(&db_path).unwrap();
    let open_ms = t_open.elapsed().as_secs_f64() * 1000.0;
    println!("OPEN: {:.2}ms", open_ms);

    let targets = ["G25", "G3056", "G26", "H430", "H3068", "H160"];
    for s in targets {
        let res = idx.occurrences(s, None, 1, 50).unwrap();
        let summary: Vec<String> = res
            .bibles
            .iter()
            .filter(|b| b.total_occurrences > 0)
            .map(|b| {
                format!(
                    "{}:v{}/n{}",
                    b.module_id, b.total_verses, b.total_occurrences
                )
            })
            .collect();
        println!("STRONG {s}: {}", summary.join(" ; "));
        for b in res
            .bibles
            .iter()
            .filter(|b| b.lang == "por" && b.total_occurrences > 0)
        {
            let occ = idx.occurrences(s, Some(&b.module_id), 1, 50).unwrap();
            let mut books = occ.by_book.clone();
            books.sort_by(|a, c| {
                c.occurrences
                    .cmp(&a.occurrences)
                    .then_with(|| a.book.cmp(&c.book))
            });
            let top3: Vec<String> = books
                .iter()
                .take(3)
                .map(|x| format!("b{}:{}", x.book, x.occurrences))
                .collect();
            let tr = idx.translations(s, Some(&b.module_id), 5).unwrap();
            let top5: Vec<String> = tr
                .items
                .iter()
                .map(|x| format!("{}:{}({:.1}%)", x.word, x.n, x.percent))
                .collect();
            println!(
                "  PT {} ({s}): top3=[{}] top5=[{}]",
                b.module_id,
                top3.join(","),
                top5.join(",")
            );
        }
    }

    // Latência p50/p95 em 200 consultas
    let mut times_ms = Vec::with_capacity(200);
    for i in 0..200 {
        let s = targets[i % targets.len()];
        let t0 = Instant::now();
        let _ = idx.occurrences(s, None, 1, 50).unwrap();
        let _ = idx.translations(s, None, 10).unwrap();
        times_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    times_ms.sort_by(|a, b| a.total_cmp(b));
    let p50 = times_ms[100];
    let p95 = times_ms[190];
    let max = *times_ms.last().unwrap();
    println!(
        "LATENCY_200: p50={:.2}ms p95={:.2}ms max={:.2}ms",
        p50, p95, max
    );
    assert!(p95 < 50.0, "p95={p95:.2}ms excedeu 50 ms");
}
