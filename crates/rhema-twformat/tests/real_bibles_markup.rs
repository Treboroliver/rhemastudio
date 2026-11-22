use std::collections::BTreeMap;

use rhema_core::{Book, Node};
use rhema_twformat::{plain_text, OntReader};

fn collect_unknown(nodes: &[Node], counts: &mut BTreeMap<String, usize>) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            Node::UnknownTag { name, children, .. } => {
                // Só o nome da tag (antes de espaço ou '='), sem valores de atributo nem texto
                let tag_only = name
                    .split(|c: char| c.is_whitespace() || c == '=')
                    .next()
                    .unwrap_or(name);
                let label: String = tag_only.chars().take(24).collect();
                *counts.entry(label).or_default() += 1;
                1 + collect_unknown(children, counts)
            }
            Node::Italic(children)
            | Node::Bold(children)
            | Node::Superscript(children)
            | Node::Subscript(children)
            | Node::Note(children)
            | Node::WordsOfJesus(children)
            | Node::OtQuote(children)
            | Node::Heading(_, children)
            | Node::Strong { children, .. } => collect_unknown(children, counts),
            _ => 0,
        })
        .sum()
}

fn words_of_jesus(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            Node::WordsOfJesus(children) => 1 + words_of_jesus(children),
            Node::Italic(children)
            | Node::Bold(children)
            | Node::Superscript(children)
            | Node::Subscript(children)
            | Node::Strong { children, .. } => words_of_jesus(children),
            _ => 0,
        })
        .sum()
}

/// Com RHEMA_TEST_MODULES: tokeniza as Bíblias `.ont` da pasta e registra contagens, sem texto.
#[test]
fn real_bibles_tokenize_without_unknown_tags_or_lost_angle_brackets() {
    let Ok(root) = std::env::var("RHEMA_TEST_MODULES") else {
        eprintln!("Ignorado: RHEMA_TEST_MODULES não foi definido");
        return;
    };
    let Ok(entries) = std::fs::read_dir(&root) else {
        eprintln!("Ignorado: RHEMA_TEST_MODULES inválido");
        return;
    };
    let mut bibles: Vec<(String, std::path::PathBuf)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("ont"))
        })
        .filter_map(|p| Some((p.file_name()?.to_string_lossy().into_owned(), p)))
        .collect();
    bibles.sort_by(|a, b| a.0.cmp(&b.0));

    if bibles.is_empty() {
        eprintln!("real_bibles_markup: ignorado (nenhuma Bíblia .ont disponível)");
        return;
    }

    for (name, path) in bibles {
        let Ok(reader) = OntReader::open(&path) else {
            eprintln!("real_bibles_markup: {name} não pôde ser aberta");
            continue;
        };
        let (mut angles, mut unknown_tags, mut chapters, mut jesus_in_john_3_3) = (0, 0, 0, 0);
        let mut unknown_names = BTreeMap::<String, usize>::new();
        for number in 1..=66u8 {
            let Some(book) = Book::from_number(number) else {
                continue;
            };
            for chapter in 1..=150u16 {
                if book.verses_in_chapter(chapter).is_none() {
                    break;
                }
                let verses = reader.chapter_nodes(book, chapter).expect("capítulo");
                let mut here = 0;
                for (verse, nodes) in &verses {
                    here += plain_text(nodes).matches(['<', '>']).count();
                    let found = collect_unknown(nodes, &mut unknown_names);
                    unknown_tags += found;
                    if number == 43 && chapter == 3 && *verse == 3 {
                        jesus_in_john_3_3 = words_of_jesus(nodes);
                    }
                }
                angles += here;
                chapters += usize::from(here > 0);
            }
        }
        let mut top10: Vec<(&String, &usize)> = unknown_names.iter().collect();
        top10.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        top10.truncate(10);
        eprintln!(
            "{name}: sinais literais={angles} em {chapters} capítulos; UnknownTag={unknown_tags} (top10={top10:?}); \
             palavras de Jesus em Jo 3:3={jesus_in_john_3_3}"
        );
        if name.eq_ignore_ascii_case("ARA+.ont") {
            assert_eq!(
                unknown_tags, 0,
                "{name}: UnknownTag={unknown_tags} (top10={top10:?})"
            );
        }
    }
}
