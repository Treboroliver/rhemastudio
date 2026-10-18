use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use regex::Regex;
use rusqlite::{Connection, OpenFlags};

#[derive(Parser)]
#[command(
    name = "rhema-cli",
    about = "RhemaStudio CLI — Ferramenta de inspeção e utilitários para módulos theWord"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspeciona a estrutura, DDL, metadados e tags de um módulo
    Inspect {
        /// Caminho para o arquivo (.twm, .ont, .ot, .nt, etc.)
        file: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Inspect { file } => inspect_file(&file),
    }
}

fn inspect_file(path: &Path) -> Result<()> {
    if !path.exists() {
        bail!("Arquivo não encontrado: {}", path.display());
    }

    let file_size = std::fs::metadata(path)?.len();
    println!("============================================================");
    println!("INSPEÇÃO DE MÓDULO: {}", path.display());
    println!(
        "Tamanho do arquivo: {} bytes ({:.2} MB)",
        file_size,
        file_size as f64 / (1024.0 * 1024.0)
    );
    println!("============================================================\n");

    let mut header = [0u8; 16];
    let mut f = File::open(path)?;
    let read_bytes = f.read(&mut header)?;
    drop(f);

    let is_sqlite = read_bytes >= 15 && &header[0..15] == b"SQLite format 3";
    let is_encrypted = read_bytes >= 9 && &header[0..9] == b"TWENCBMOD";

    if is_encrypted {
        println!("Formato detectado: Módulo theWord Encriptado (Assinatura TWENCBMOD / DRM)");
        println!("Aviso: O módulo utiliza criptografia proprietária do theWord com proteção DRM.");
        println!("Por diretriz do projeto (AGENTS.md / docs/02), o RhemaStudio não implementa engenharia reversa de DRM.");
        return Ok(());
    }

    if is_sqlite
        || path
            .extension()
            .is_some_and(|ext| ext == "twm" || ext == "mybible")
    {
        inspect_sqlite(path)?;
    } else {
        inspect_bible_file(path)?;
    }

    Ok(())
}

fn inspect_sqlite(path: &Path) -> Result<()> {
    println!("Formato detectado: SQLite (banco de dados)");
    println!("Abrindo em modo estritamente SOMENTE LEITURA...\n");

    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("Falha ao abrir SQLite: {}", path.display()))?;

    conn.execute_batch("PRAGMA query_only = ON;")?;

    // 1. sqlite_master DDL
    println!("------------------------------------------------------------");
    println!("1. ESTRUTURA DO BANCO (sqlite_master / DDL)");
    println!("------------------------------------------------------------");

    let mut stmt = conn.prepare(
        "SELECT type, name, tbl_name, sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY type, name;"
    )?;
    let ddl_rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    let mut table_names = Vec::new();
    for row in ddl_rows {
        let (obj_type, name, _tbl_name, sql) = row?;
        println!("[{}] {}", obj_type.to_uppercase(), name);
        if obj_type == "table" && !name.starts_with("sqlite_") {
            table_names.push(name.clone());
        }
        for line in sql.lines() {
            println!("    {}", line);
        }
        println!();
    }

    // 2. Tabela de Metadados (config / details / info / properties / etc.)
    println!("------------------------------------------------------------");
    println!("2. METADADOS DO MÓDULO");
    println!("------------------------------------------------------------");

    let mut metadata_found = false;
    for &candidate in &[
        "config",
        "details",
        "info",
        "properties",
        "meta",
        "module_info",
    ] {
        if table_names
            .iter()
            .any(|t| t.eq_ignore_ascii_case(candidate))
        {
            let actual_name = table_names
                .iter()
                .find(|t| t.eq_ignore_ascii_case(candidate))
                .unwrap();
            println!("Encontrada tabela de metadados: \"{}\"", actual_name);

            // Verificar colunas
            let pragma_query = format!("PRAGMA table_info(\"{}\");", actual_name);
            let mut p_stmt = conn.prepare(&pragma_query)?;
            let col_names: Vec<String> = p_stmt
                .query_map([], |r| r.get::<_, String>(1))?
                .filter_map(Result::ok)
                .collect();

            let select_query = format!("SELECT * FROM \"{}\";", actual_name);
            let mut m_stmt = conn.prepare(&select_query)?;
            let col_count = m_stmt.column_count();

            let mut rows = m_stmt.query([])?;
            let mut row_idx = 0;
            while let Some(r) = rows.next()? {
                row_idx += 1;
                print!("  [#{}]: ", row_idx);
                let mut parts = Vec::new();
                for i in 0..col_count {
                    let col_name = col_names.get(i).map(|s| s.as_str()).unwrap_or("col");
                    let val = match r.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => "NULL".to_string(),
                        rusqlite::types::ValueRef::Integer(v) => v.to_string(),
                        rusqlite::types::ValueRef::Real(v) => v.to_string(),
                        rusqlite::types::ValueRef::Text(v) => {
                            let s = String::from_utf8_lossy(v);
                            if s.len() > 80 {
                                format!("{}... (len: {})", &s[..80], s.len())
                            } else {
                                s.to_string()
                            }
                        }
                        rusqlite::types::ValueRef::Blob(v) => format!("[BLOB {} bytes]", v.len()),
                    };
                    parts.push(format!("{} = {:?}", col_name, val));
                }
                println!("{}", parts.join(" | "));
            }
            metadata_found = true;
            println!();
        }
    }

    if !metadata_found {
        println!("(Nenhuma tabela convencional de metadados [config/details/info] identificada)\n");
    }

    // Análise Específica: bible_refs (se presente em comentários)
    if table_names
        .iter()
        .any(|t| t.eq_ignore_ascii_case("bible_refs"))
    {
        println!("------------------------------------------------------------");
        println!("ANÁLISE DE REFERÊNCIAS BÍBLICAS (bible_refs)");
        println!("------------------------------------------------------------");
        let min_max: (i64, i64, i64, i64, i64, i64, i64, i64) = conn.query_row(
            "SELECT MIN(bi), MAX(bi), MIN(ci), MAX(ci), MIN(fvi), MAX(fvi), MIN(tvi), MAX(tvi) FROM bible_refs;",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
        )?;
        println!("  Range de Livros (bi):      {} a {}", min_max.0, min_max.1);
        println!("  Range de Capítulos (ci):   {} a {}", min_max.2, min_max.3);
        println!("  Range Versículo De (fvi):  {} a {}", min_max.4, min_max.5);
        println!("  Range Versículo Até (tvi): {} a {}", min_max.6, min_max.7);

        // Amostras de categorias: Livro intro (ci=0), Capítulo intro (ci>0, fvi=0), Versículo único (fvi=tvi), Faixa (fvi<tvi)
        println!("\n  Exemplos de padrões de referência em bible_refs:");
        let mut b_stmt = conn.prepare(
            "SELECT topic_id, bi, ci, fvi, tvi, content_type FROM bible_refs WHERE ci = 0 LIMIT 2;",
        )?;
        let mut b_rows = b_stmt.query([])?;
        while let Some(r) = b_rows.next()? {
            println!("    [Introdução de Livro]: topic_id={}, bi={}, ci={}, fvi={}, tvi={}, content_type={}",
                r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?);
        }

        let mut c_stmt = conn.prepare(
            "SELECT topic_id, bi, ci, fvi, tvi, content_type FROM bible_refs WHERE ci > 0 AND fvi = 0 LIMIT 2;"
        )?;
        let mut c_rows = c_stmt.query([])?;
        while let Some(r) = c_rows.next()? {
            println!("    [Introdução de Capítulo]: topic_id={}, bi={}, ci={}, fvi={}, tvi={}, content_type={}",
                r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?);
        }

        let mut v_stmt = conn.prepare(
            "SELECT topic_id, bi, ci, fvi, tvi, content_type FROM bible_refs WHERE ci > 0 AND fvi > 0 AND fvi = tvi LIMIT 2;"
        )?;
        let mut v_rows = v_stmt.query([])?;
        while let Some(r) = v_rows.next()? {
            println!(
                "    [Versículo Único]: topic_id={}, bi={}, ci={}, fvi={}, tvi={}, content_type={}",
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, String>(5)?
            );
        }

        let mut r_stmt = conn.prepare(
            "SELECT topic_id, bi, ci, fvi, tvi, content_type FROM bible_refs WHERE ci > 0 AND fvi > 0 AND fvi < tvi LIMIT 2;"
        )?;
        let mut r_rows = r_stmt.query([])?;
        while let Some(r) = r_rows.next()? {
            println!("    [Faixa de Versículos (Perícope)]: topic_id={}, bi={}, ci={}, fvi={}, tvi={}, content_type={}",
                r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?);
        }
        println!();
    }

    // Análise Específica: topics (se presente em dicionários)
    if table_names.iter().any(|t| t.eq_ignore_ascii_case("topics")) {
        println!("------------------------------------------------------------");
        println!("ANÁLISE DE TÓPICOS / CHAVES DE DICIONÁRIO (topics)");
        println!("------------------------------------------------------------");

        // Verificar chaves que parecem Strong Grego (G... ou numéricas)
        let mut g_stmt = conn.prepare(
            "SELECT id, pid, subject, strong_orig_word FROM topics WHERE subject LIKE 'G%' OR subject GLOB '[0-9]*' LIMIT 5;"
        )?;
        let mut g_rows = g_stmt.query([])?;
        println!("  Amostra de entradas Gregas / Numéricas em topics:");
        while let Some(r) = g_rows.next()? {
            println!(
                "    id={}, pid={}, subject={:?}, strong_orig_word={:?}",
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?
            );
        }

        // Verificar chaves que parecem Strong Hebraico (H...)
        let mut h_stmt = conn.prepare(
            "SELECT id, pid, subject, strong_orig_word FROM topics WHERE subject LIKE 'H%' LIMIT 5;"
        )?;
        let mut h_rows = h_stmt.query([])?;
        println!("  Amostra de entradas Hebraicas em topics:");
        while let Some(r) = h_rows.next()? {
            println!(
                "    id={}, pid={}, subject={:?}, strong_orig_word={:?}",
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?
            );
        }
        println!();
    }

    // 3. Amostra de cada tabela (até 5 linhas)
    println!("------------------------------------------------------------");
    println!("3. AMOSTRAS DAS TABELAS (até 5 linhas cada)");
    println!("------------------------------------------------------------");

    for tbl in &table_names {
        let count_query = format!("SELECT count(*) FROM \"{}\";", tbl);
        let total_rows: i64 = conn.query_row(&count_query, [], |r| r.get(0)).unwrap_or(-1);

        println!("Tabela: \"{}\" (Total de linhas: {})", tbl, total_rows);

        let pragma_query = format!("PRAGMA table_info(\"{}\");", tbl);
        let mut p_stmt = conn.prepare(&pragma_query)?;
        let columns: Vec<(String, String)> = p_stmt
            .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?
            .filter_map(Result::ok)
            .collect();

        println!(
            "  Colunas: {}",
            columns
                .iter()
                .map(|(c, t)| format!("{}: {}", c, t))
                .collect::<Vec<_>>()
                .join(", ")
        );

        let sample_query = format!("SELECT * FROM \"{}\" LIMIT 5;", tbl);
        let mut s_stmt = conn.prepare(&sample_query)?;
        let col_count = s_stmt.column_count();

        let mut rows = s_stmt.query([])?;
        let mut sample_idx = 0;
        while let Some(r) = rows.next()? {
            sample_idx += 1;
            print!("    Linha {}: ", sample_idx);
            let mut parts = Vec::new();
            for i in 0..col_count {
                let col_name = columns.get(i).map(|(c, _)| c.as_str()).unwrap_or("col");
                let val = match r.get_ref(i)? {
                    rusqlite::types::ValueRef::Null => "NULL".to_string(),
                    rusqlite::types::ValueRef::Integer(v) => v.to_string(),
                    rusqlite::types::ValueRef::Real(v) => v.to_string(),
                    rusqlite::types::ValueRef::Text(v) => {
                        let s = String::from_utf8_lossy(v);
                        if s.len() > 100 {
                            format!("{}... (len: {})", &s[..100], s.len())
                        } else {
                            s.to_string()
                        }
                    }
                    rusqlite::types::ValueRef::Blob(v) => format!("[BLOB {} bytes]", v.len()),
                };
                parts.push(format!("{}: {:?}", col_name, val));
            }
            println!("{}", parts.join(", "));
        }
        println!();
    }

    Ok(())
}

fn inspect_bible_file(path: &Path) -> Result<()> {
    println!("Formato detectado: Bíblia em texto plano (.ont / .ot / .nt / .ontx)");

    let mut raw_bytes = Vec::new();
    let mut file = File::open(path)?;
    file.read_to_end(&mut raw_bytes)?;

    // 1. Identificar Codificação
    let mut encoding_name = "UTF-8";
    let (text, _, had_errors) = if raw_bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        encoding_name = "UTF-8 com BOM";
        encoding_rs::UTF_8.decode(&raw_bytes[3..])
    } else {
        let (t, _, err) = encoding_rs::UTF_8.decode(&raw_bytes);
        if err {
            encoding_name = "Windows-1252 (fallback)";
            let (w1252_text, _, _) = encoding_rs::WINDOWS_1252.decode(&raw_bytes);
            (w1252_text, encoding_rs::WINDOWS_1252, false)
        } else {
            (t, encoding_rs::UTF_8, false)
        }
    };

    println!(
        "Codificação identificada: {} (erros de decodificação: {})",
        encoding_name, had_errors
    );

    // 2. Linhas de versículos vs Propriedades
    let mut verse_lines = 0usize;
    let mut empty_lines = 0usize;
    let mut property_lines = Vec::new();
    let mut properties = HashMap::new();

    let tag_regex = Regex::new(r"<([^>]+)>")?;
    let mut tag_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut tag_families: BTreeMap<String, usize> = BTreeMap::new();

    let mut in_properties = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            empty_lines += 1;
            continue;
        }

        // Bloco de propriedades ao final costuma iniciar com chave=valor
        // Geralmente após os versículos
        if verse_lines >= 7000 && trimmed.contains('=') && !trimmed.starts_with('<') {
            in_properties = true;
        }

        if in_properties {
            property_lines.push(trimmed.to_string());
            if let Some((k, v)) = trimmed.split_once('=') {
                properties.insert(k.trim().to_string(), v.trim().to_string());
            }
        } else {
            verse_lines += 1;

            // Escanear todas as tags da linha
            for cap in tag_regex.captures_iter(line) {
                let full_tag = &cap[0];
                let tag_content = &cap[1];
                *tag_counts.entry(full_tag.to_string()).or_insert(0) += 1;

                // Identificar família da tag
                let family = if tag_content.starts_with("WG") {
                    "<WG####> (Strong Grego)".to_string()
                } else if tag_content.starts_with("WH") {
                    "<WH####> (Strong Hebraico)".to_string()
                } else if tag_content.starts_with("WT") {
                    "<WT...> (Morfologia)".to_string()
                } else if tag_content.starts_with("TS") {
                    "<TS...> (Título de Seção)".to_string()
                } else if tag_content.starts_with("Ts") {
                    "<Ts...> (Fim Título de Seção)".to_string()
                } else if tag_content.starts_with("RX") {
                    "<RX...> (Referência Cruzada)".to_string()
                } else if tag_content.starts_with("RF") {
                    "<RF...> (Início Nota Rodapé)".to_string()
                } else if tag_content.starts_with("Rf") {
                    "<Rf> (Fim Nota Rodapé)".to_string()
                } else {
                    full_tag.to_string()
                };

                *tag_families.entry(family).or_insert(0) += 1;
            }
        }
    }

    println!("\n------------------------------------------------------------");
    println!("CONTAGEM DE LINHAS");
    println!("------------------------------------------------------------");
    println!("Total de linhas de versículos: {}", verse_lines);
    println!("Total de linhas de propriedades: {}", property_lines.len());
    println!("Linhas vazias ignoradas: {}", empty_lines);

    // Comparação com cânon
    println!("\nComparação com contagem canônica esperada:");
    println!("  .ont esperado: 31.102 | Encontrado: {}", verse_lines);
    println!("  .ot esperado:  23.145");
    println!("  .nt esperado:   7.957");

    // 3. Bloco de propriedades
    println!("\n------------------------------------------------------------");
    println!("PROPRIEDADES DO MÓDULO (chave=valor)");
    println!("------------------------------------------------------------");
    for (k, v) in &properties {
        println!("  {} = {}", k, v);
    }

    // 4. Todas as tags distintas
    println!("\n------------------------------------------------------------");
    println!("DISTRIBUIÇÃO DE FAMÍLIAS DE TAGS ENCONTRADAS");
    println!("------------------------------------------------------------");
    let total_tags: usize = tag_families.values().sum();
    println!("Total geral de tags no arquivo: {}\n", total_tags);

    for (fam, count) in &tag_families {
        println!("  {:35} : {:8} ocorrências", fam, count);
    }

    println!("\n------------------------------------------------------------");
    println!("AMOSTRA DE TAGS DISTINTAS (Top 30 mais frequentes)");
    println!("------------------------------------------------------------");
    let mut sorted_tags: Vec<(&String, &usize)> = tag_counts.iter().collect();
    sorted_tags.sort_by(|a, b| b.1.cmp(a.1));

    for (tag, count) in sorted_tags.iter().take(30) {
        println!("  {:25} : {}x", tag, count);
    }

    Ok(())
}
