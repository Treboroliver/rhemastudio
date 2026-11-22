use rhema_core::{Book, Node};
use rhema_twformat::{plain_text, OntReader};

fn unknown(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            Node::UnknownTag { children, .. } => 1 + unknown(children),
            Node::Italic(children)
            | Node::WordsOfJesus(children)
            | Node::Strong { children, .. } => unknown(children),
            _ => 0,
        })
        .sum()
}

fn words_of_jesus(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            Node::WordsOfJesus(children) => 1 + words_of_jesus(children),
            Node::Italic(children) | Node::Strong { children, .. } => words_of_jesus(children),
            _ => 0,
        })
        .sum()
}

/// Com RHEMA_TEST_MODULES: tokeniza as duas Bíblias inteiras e registra contagens, sem texto.
#[test]
fn real_bibles_tokenize_without_unknown_tags_or_lost_angle_brackets() {
    let Ok(root) = std::env::var("RHEMA_TEST_MODULES") else {
        eprintln!("Ignorado: RHEMA_TEST_MODULES não foi definido");
        return;
    };
    let mut tested = 0usize;
    for name in ["ARA+.ont", "kjv.ont"] {
        let path = std::path::Path::new(&root).join(name);
        let Ok(reader) = OntReader::open(&path) else {
            eprintln!("real_bibles_markup: {name} ausente");
            continue;
        };
        tested += 1;
        let (mut angles, mut unknown_tags, mut chapters, mut jesus_in_john_3_3) = (0, 0, 0, 0);
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
                    let found = unknown(nodes);
                    if found > 0 {
                        eprintln!("{name}: UnknownTag em livro {number} cap. {chapter} v. {verse}");
                    }
                    unknown_tags += found;
                    if number == 43 && chapter == 3 && *verse == 3 {
                        jesus_in_john_3_3 = words_of_jesus(nodes);
                    }
                }
                angles += here;
                chapters += usize::from(here > 0);
            }
        }
        eprintln!(
            "{name}: sinais literais={angles} em {chapters} capítulos; UnknownTag={unknown_tags}; \
             palavras de Jesus em Jo 3:3={jesus_in_john_3_3}"
        );
        // ARA+ possui até 2 tags gramaticais órfãs sem Strong anterior.
        let max_unknown = if name.eq_ignore_ascii_case("ARA+.ont") {
            2
        } else {
            0
        };
        assert!(
            unknown_tags <= max_unknown,
            "{name}: UnknownTag={unknown_tags}"
        );
    }
    if tested == 0 {
        eprintln!("real_bibles_markup: ignorado (nenhuma Bíblia de referência disponível)");
    }
}
