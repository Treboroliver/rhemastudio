use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use rhema_core::{Book, Testament};
use rusqlite::Connection;

/// Gera bytes de um arquivo .ont canônico completo (31.102 versículos)
pub fn synthetic_ont_bytes(charset: u32, with_strong: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(31102 * 60);

    for &b in &Book::ALL {
        let ch_count = b.chapter_count();
        for ch in 1..=ch_count {
            let v_count = b.verses_in_chapter(ch).unwrap_or(1);
            for v in 1..=v_count {
                let strong_tag = if with_strong {
                    if b.testament() == Testament::Old {
                        "<WH3068>"
                    } else {
                        "<WG2532>"
                    }
                } else {
                    ""
                };

                let red_tag = if b == Book::John && ch == 3 && v == 16 {
                    "<FR>Porque Deus amou o mundo de tal maneira...<Fr>"
                } else {
                    ""
                };

                let line = format!(
                    "Texto sintético de {} {}:{}{} {}\r\n",
                    b.primary_abbreviation_pt(),
                    ch,
                    v,
                    strong_tag,
                    red_tag
                );

                if charset == 0 {
                    // Windows-1252 ANSI
                    let (encoded, _, _) = encoding_rs::WINDOWS_1252.encode(&line);
                    out.extend_from_slice(&encoded);
                } else {
                    // UTF-8
                    out.extend_from_slice(line.as_bytes());
                }
            }
        }
    }

    // Bloco de propriedades ao final
    let props = format!(
        "\r\n\
        title = Bíblia Sintética de Teste\r\n\
        short.title = BST\r\n\
        abbrev = BST\r\n\
        lang = por\r\n\
        charset = {}\r\n\
        description = Módulo sintético gerado para testes automatizados\r\n\
        creator = RhemaStudio Fixture Generator\r\n\
        verse.rule = \"<wt>(.*?)(<W([HG]\\d+)>)\" \"<wt><a href=_STRONG_$3>$1</a>$2\"\r\n",
        charset
    );

    if charset == 0 {
        let (encoded, _, _) = encoding_rs::WINDOWS_1252.encode(&props);
        out.extend_from_slice(&encoded);
    } else {
        out.extend_from_slice(props.as_bytes());
    }

    out
}

/// Gera um arquivo .ont sintético completo em disco
pub fn generate_synthetic_ont(path: &Path, charset: u32, with_strong: bool) -> Result<()> {
    let bytes = synthetic_ont_bytes(charset, with_strong);
    let mut f = File::create(path)?;
    f.write_all(&bytes)?;
    Ok(())
}

/// Gera um arquivo .ot sintético completo (23.145 versículos)
pub fn generate_synthetic_ot(path: &Path) -> Result<()> {
    let mut out = Vec::with_capacity(23145 * 50);

    for &b in &Book::ALL {
        if b.testament() != Testament::Old {
            continue;
        }
        let ch_count = b.chapter_count();
        for ch in 1..=ch_count {
            let v_count = b.verses_in_chapter(ch).unwrap_or(1);
            for v in 1..=v_count {
                let line = format!(
                    "Texto AT de {} {}:{}\r\n",
                    b.primary_abbreviation_pt(),
                    ch,
                    v
                );
                let (encoded, _, _) = encoding_rs::WINDOWS_1252.encode(&line);
                out.extend_from_slice(&encoded);
            }
        }
    }

    let props = "\r\n\
        title = Antigo Testamento Sintético\r\n\
        short.title = AT_SINT\r\n\
        lang = por\r\n\
        charset = 0\r\n";
    let (encoded, _, _) = encoding_rs::WINDOWS_1252.encode(props);
    out.extend_from_slice(&encoded);

    let mut f = File::create(path)?;
    f.write_all(&out)?;
    Ok(())
}

/// Gera um arquivo .nt sintético completo (7.957 versículos)
pub fn generate_synthetic_nt(path: &Path) -> Result<()> {
    let mut out = Vec::with_capacity(7957 * 50);

    for &b in &Book::ALL {
        if b.testament() != Testament::New {
            continue;
        }
        let ch_count = b.chapter_count();
        for ch in 1..=ch_count {
            let v_count = b.verses_in_chapter(ch).unwrap_or(1);
            for v in 1..=v_count {
                let line = format!(
                    "Texto NT de {} {}:{}\r\n",
                    b.primary_abbreviation_pt(),
                    ch,
                    v
                );
                let (encoded, _, _) = encoding_rs::WINDOWS_1252.encode(&line);
                out.extend_from_slice(&encoded);
            }
        }
    }

    let props = "\r\n\
        title = Novo Testamento Sintético\r\n\
        short.title = NT_SINT\r\n\
        lang = por\r\n\
        charset = 0\r\n";
    let (encoded, _, _) = encoding_rs::WINDOWS_1252.encode(props);
    out.extend_from_slice(&encoded);

    let mut f = File::create(path)?;
    f.write_all(&out)?;
    Ok(())
}

/// Gera um arquivo .ont degradado com quantidade arbitrária de versículos
pub fn generate_synthetic_degraded(path: &Path, count: usize) -> Result<()> {
    let mut out = Vec::with_capacity(count * 50);

    for i in 1..=count {
        let line = format!("Linha de versículo degradado #{}\r\n", i);
        out.extend_from_slice(line.as_bytes());
    }

    let props = "\r\n\
        title = Módulo Degradado de Teste\r\n\
        short.title = DEG\r\n\
        charset = 65001\r\n";
    out.extend_from_slice(props.as_bytes());

    let mut f = File::create(path)?;
    f.write_all(&out)?;
    Ok(())
}

/// Gera um arquivo encriptado falso com assinatura TWENCBMOD
pub fn generate_synthetic_encrypted(path: &Path) -> Result<()> {
    let mut f = File::create(path)?;
    f.write_all(b"TWENCBMOD\x01\x02\x03\x04dados_encriptados_falsos_para_teste")?;
    Ok(())
}

pub fn generate_synthetic_commentary_twm(path: &Path) -> Result<()> {
    let connection = create_twm(path, "2", "rtf")?;
    connection.execute_batch(
        "CREATE TABLE bible_refs (
            topic_id INTEGER PRIMARY KEY,
            bi INTEGER, ci INTEGER, fvi INTEGER, tvi INTEGER, content_type TEXT
        );
        CREATE TABLE content (topic_id INTEGER PRIMARY KEY, data BLOB, data2 BLOB);
        INSERT INTO bible_refs VALUES
            (10, 43, 0, 0, 0, NULL),
            (20, 43, 3, 0, 0, 'rtf'),
            (30, 43, 3, 16, 17, 'rtf'),
            (40, 43, 3, 16, 16, 'rtf'),
            (50, 43, 3, 17, 17, 'rvf'),
            (60, 43, 3, 18, 0, 'rtf'),
            (70, 43, 3, 6, 5, 'rtf');
        INSERT INTO content VALUES
            (10, '{\\rtf1 Synthetic book introduction}', NULL),
            (20, '{\\rtf1 Synthetic chapter introduction}', NULL),
            (30, '{\\rtf1 Synthetic verse range}', NULL),
            (40, '{\\rtf1 Synthetic verse entry}', NULL),
            (50, 'RVF fixture; deliberately unsupported', NULL),
            (60, '{\\rtf1 Synthetic open-ended range}', NULL),
            (70, '{\\rtf1 Synthetic invalid range}', NULL);",
    )?;
    Ok(())
}

pub fn generate_synthetic_dictionary_twm(path: &Path) -> Result<()> {
    let connection = create_twm(path, "1", "rtf")?;
    connection.execute_batch(
        "CREATE TABLE topics (
            id INTEGER PRIMARY KEY, pid INTEGER, subject TEXT, rel_order INTEGER,
            content_type TEXT, strong_orig_word TEXT
        );
        CREATE TABLE content (topic_id INTEGER PRIMARY KEY, data TEXT, data2 BLOB);
        CREATE TABLE topics_wordindex (id INTEGER, word TEXT, priority INTEGER);
        INSERT INTO topics VALUES
            (1, 0, 'G25', 1, NULL, NULL),
            (2, 0, 'G26', 1, 'rvf', NULL),
            (3, 0, 'H25', 1, 'rtf', NULL);
        INSERT INTO content VALUES
            (1, '{\\rtf1 Synthetic dictionary entry}', NULL),
            (2, 'RVF fixture; deliberately unsupported', NULL),
            (3, '{\\rtf1 Synthetic dictionary entry}', NULL);
        INSERT INTO topics_wordindex VALUES (1, 'invented', 1);",
    )?;
    Ok(())
}

pub fn generate_synthetic_protected_twm(path: &Path, flag: &str) -> Result<()> {
    if !matches!(flag, "do.encrypt" | "secure") {
        anyhow::bail!("Flag de proteção sintética desconhecida");
    }
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "CREATE TABLE config(name TEXT, value TEXT);
         CREATE TABLE bible_refs(topic_id INTEGER, bi INTEGER, ci INTEGER, fvi INTEGER, tvi INTEGER, content_type TEXT);
         CREATE TABLE content(topic_id INTEGER, data BLOB);",
    )?;
    connection.execute("INSERT INTO config(name, value) VALUES (?1, '1')", [flag])?;
    connection.execute("INSERT INTO config(name, value) VALUES ('type', '2')", [])?;
    drop(connection);
    Ok(())
}

/// Comentário sintético para a interface de estudo: entradas de todos os escopos em João 3,
/// uma entrada longa (`paragraphs` parágrafos), uma entrada RVF e uma com links.
pub fn generate_synthetic_study_commentary_twm(path: &Path, paragraphs: usize) -> Result<()> {
    let connection = create_twm(path, "2", "rtf")?;
    connection.execute_batch(
        "CREATE TABLE bible_refs (
            topic_id INTEGER PRIMARY KEY,
            bi INTEGER, ci INTEGER, fvi INTEGER, tvi INTEGER, content_type TEXT
        );
        CREATE TABLE content (topic_id INTEGER PRIMARY KEY, data BLOB, data2 BLOB);
        INSERT INTO bible_refs VALUES
            (10, 43, 0, 0, 0, 'rtf'),
            (20, 43, 3, 0, 0, 'rtf'),
            (30, 43, 3, 15, 17, 'rtf'),
            (40, 43, 3, 16, 16, 'rtf'),
            (50, 43, 3, 16, 16, 'rvf'),
            (60, 43, 3, 1, 36, 'rtf'),
            (70, 43, 3, 16, 16, 'rtf');
        INSERT INTO content VALUES
            (10, '{\\rtf1 Synthetic book introduction\\par}', NULL),
            (20, '{\\rtf1 Synthetic chapter introduction\\par}', NULL),
            (30, '{\\rtf1 Synthetic verse range\\par}', NULL),
            (50, 'RVF fixture; deliberately unsupported', NULL),
            (60, '{\\rtf1 Synthetic whole chapter\\par}', NULL);",
    )?;
    let mut long = String::from("{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Arial;}}");
    for index in 1..=paragraphs {
        long.push_str(&format!(
            "Synthetic paragraph {index} {{\\b bold}} {{\\i italic}} {{\\ul under}} x{{\\super 2}}\\par "
        ));
    }
    long.push('}');
    let links = concat!(
        "{\\rtf1 See ",
        "{\\field{\\*\\fldinst HYPERLINK \"tw://bible.*?id=43.3.16|_AUTODETECT_|\"}{\\fldrslt verse link}} ",
        "{\\field{\\*\\fldinst HYPERLINK \"_STRONG_G26|_NOLINK_\"}{\\fldrslt entry link}} ",
        "{\\field{\\*\\fldinst HYPERLINK \"tw://person.xyz?id=7\"}{\\fldrslt other link}}",
        "\\par}"
    );
    for (topic, data) in [(40_i64, long.as_str()), (70, links)] {
        connection.execute(
            "INSERT INTO content (topic_id, data, data2) VALUES (?1, ?2, NULL)",
            (topic, data),
        )?;
    }
    Ok(())
}

/// Dicionário sintético: G25 com links, G26 em RVF, H430 e G1 com link de tipo desconhecido.
pub fn generate_synthetic_study_dictionary_twm(path: &Path) -> Result<()> {
    let connection = create_twm(path, "1", "rtf")?;
    connection.execute_batch(
        "CREATE TABLE topics (
            id INTEGER PRIMARY KEY, pid INTEGER, subject TEXT, rel_order INTEGER,
            content_type TEXT, strong_orig_word TEXT
        );
        CREATE TABLE content (topic_id INTEGER PRIMARY KEY, data TEXT, data2 BLOB);
        INSERT INTO topics VALUES
            (1, 0, 'G25', 1, NULL, 'synthetic-lemma'),
            (2, 0, 'G26', 1, 'rvf', NULL),
            (3, 0, 'H430', 1, 'rtf', NULL),
            (4, 0, 'G1', 1, 'rtf', NULL);
        INSERT INTO content VALUES
            (1, '{\\rtf1 Synthetic \\b entry\\b0  {\\field{\\*\\fldinst HYPERLINK \"_STRONG_G1|_NOLINK_\"}{\\fldrslt entry link}} {\\field{\\*\\fldinst HYPERLINK \"tw://bible.*?id=43.3.16|_AUTODETECT_|\"}{\\fldrslt verse link}}\\par}', NULL),
            (2, 'RVF fixture; deliberately unsupported', NULL),
            (3, '{\\rtf1 Synthetic Hebrew entry\\par}', NULL),
            (4, '{\\rtf1 {\\field{\\*\\fldinst HYPERLINK \"tw://person.xyz?id=7\"}{\\fldrslt other link}}\\par}', NULL);",
    )?;
    Ok(())
}

fn create_twm(path: &Path, kind: &str, content_type: &str) -> Result<Connection> {
    let connection = Connection::open(path)?;
    connection.execute_batch("CREATE TABLE config(name TEXT, value TEXT);")?;
    connection.execute(
        "INSERT INTO config(name, value) VALUES ('type', ?1)",
        [kind],
    )?;
    connection.execute(
        "INSERT INTO config(name, value) VALUES ('content.type', ?1)",
        [content_type],
    )?;
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use rhema_core::{NEW_TESTAMENT_VERSES, OLD_TESTAMENT_VERSES, TOTAL_CANONICAL_VERSES};

    #[test]
    fn test_synthetic_counts() {
        assert_eq!(TOTAL_CANONICAL_VERSES, 31102);
        assert_eq!(OLD_TESTAMENT_VERSES, 23145);
        assert_eq!(NEW_TESTAMENT_VERSES, 7957);
    }
}
