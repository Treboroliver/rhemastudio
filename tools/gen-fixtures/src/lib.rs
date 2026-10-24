use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use rhema_core::{Book, Testament};

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
