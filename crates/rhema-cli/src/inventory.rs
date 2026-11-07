use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use regex::Regex;
use rhema_core::ModuleError;
use rhema_twformat::{decode_twm_content_checked, plain_text, OntReader};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use walkdir::WalkDir;

struct Patterns {
    strong: Regex,
    domain: Regex,
    number: Regex,
    citations: Regex,
    tags: Regex,
    html: Regex,
}

impl Patterns {
    fn new() -> Result<Self> {
        Ok(Self {
            strong: Regex::new(r"(?i)^[gh]\d+[a-z]?$")?,
            domain: Regex::new(r"^\d{1,2}\.\d+[a-z]?$")?,
            number: Regex::new(r"(?i)^\d+[a-z]?$")?,
            citations: Regex::new(r"(?i)\b(?:TWOT|DITAT|LN|G\d+|H\d+)\b")?,
            tags: Regex::new(r"<(/?[A-Za-z][A-Za-z0-9._:-]*)(?:\s+[^<>]*)?>")?,
            html: Regex::new(r"<[^>]*>")?,
        })
    }

    fn key_kind(&self, key: &str) -> &'static str {
        let key = key.trim();
        if self.strong.is_match(key) {
            "strong"
        } else if self.domain.is_match(key) {
            "dominio_decimal"
        } else if self.number.is_match(key) {
            "numero"
        } else if key.chars().any(is_greek) {
            "grego"
        } else if key.chars().any(is_hebrew) {
            "hebraico"
        } else if key.is_empty() {
            "vazio"
        } else {
            "outro"
        }
    }
}

fn is_greek(c: char) -> bool {
    matches!(c as u32, 0x0370..=0x03ff | 0x1f00..=0x1fff)
}

fn is_hebrew(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x05ff)
}

fn masked(value: &str) -> String {
    value
        .chars()
        .take(60)
        .map(|c| if c.is_alphabetic() { 'x' } else { c })
        .collect()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

pub fn run(folders: &[PathBuf]) -> Result<()> {
    let patterns = Patterns::new()?;
    let mut files = BTreeSet::new();
    let mut errors = Vec::new();
    for folder in folders {
        if !folder.is_dir() {
            errors.push(format!(
                "Pasta inexistente ou inacessível: {}",
                folder.display()
            ));
            continue;
        }
        for item in WalkDir::new(folder).follow_links(false) {
            match item {
                Ok(entry) if entry.file_type().is_file() => {
                    let name = file_name(entry.path()).to_lowercase();
                    if name.ends_with(".dct.twm")
                        || [".ont", ".ot", ".nt", ".ontx", ".otx", ".ntx"]
                            .iter()
                            .any(|suffix| name.ends_with(suffix))
                    {
                        files.insert(entry.path().to_path_buf());
                    }
                }
                Ok(_) => {}
                Err(error) => errors.push(error.to_string()),
            }
        }
    }
    let mut modules = Vec::new();
    for path in files {
        let name = file_name(&path);
        let lower = name.to_lowercase();
        let size = match path.metadata() {
            Ok(metadata) => metadata.len(),
            Err(error) => {
                errors.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        let result = if lower.ends_with(".dct.twm") {
            dictionary(&path, size, &patterns)
        } else if lower.ends_with(".ontx") || lower.ends_with(".otx") || lower.ends_with(".ntx") {
            Ok(
                json!({"arquivo": name, "tipo": "biblia", "estado": "protegido",
                "tamanho_bytes": size, "titulo": null, "idioma": null, "entradas": null}),
            )
        } else {
            bible(&path, size, &patterns)
        };
        match result {
            Ok(module) => modules.push(module),
            Err(error) => errors.push(format!("{}: {error:#}", path.display())),
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "modulos": modules,
            "erros": errors,
            "versificacao": "O formato sequencial não fornece limites independentes de capítulo; totais canônicos não provam alinhamento.",
            "citacoes": "Contagens de ocorrências textuais, não prova de mapeamento entre sistemas."
        }))?
    );
    if !errors.is_empty() {
        anyhow::bail!("Inventário incompleto: {} erro(s)", errors.len());
    }
    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info(?1) WHERE lower(name)=lower(?2))",
        [table, column],
        |row| row.get(0),
    )?)
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )?)
}

fn dictionary(path: &Path, size: u64, patterns: &Patterns) -> Result<Value> {
    let started = Instant::now();
    let mut signature = Vec::with_capacity(9);
    File::open(path)?.take(9).read_to_end(&mut signature)?;
    if matches!(
        rhema_twformat::detect_signature(&signature),
        Err(ModuleError::EncryptedModule)
    ) {
        return Ok(json!({"arquivo": file_name(path), "tipo": "dicionario",
            "estado": "protegido", "tamanho_bytes": size, "titulo": null,
            "idioma": null, "entradas": null}));
    }
    match rhema_twformat::ensure_twm_unprotected(path) {
        Ok(()) => {}
        Err(ModuleError::EncryptedModule) => {
            return Ok(json!({"arquivo": file_name(path), "tipo": "dicionario",
                "estado": "protegido", "tamanho_bytes": size, "titulo": null,
                "idioma": null, "entradas": null}));
        }
        Err(error) => return Err(error.into()),
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.execute_batch("PRAGMA query_only=ON")?;
    let mut config = BTreeMap::new();
    let mut config_stmt = conn.prepare(
        "SELECT lower(trim(name)), value FROM config WHERE lower(trim(name)) IN ('title', 'lang', 'content.type')",
    )?;
    let config_rows = config_stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    for row in config_rows {
        let (key, value) = row?;
        if let Some(value) = value {
            config.insert(key, value);
        }
    }
    let open_ms = started.elapsed().as_millis();
    let has_type = column_exists(&conn, "topics", "content_type")?;
    let has_original = column_exists(&conn, "topics", "strong_orig_word")?;
    let has_index = table_exists(&conn, "topics_wordindex")?;
    let index_terms: Option<i64> = if has_index {
        Some(conn.query_row(
            "SELECT count(DISTINCT word) FROM topics_wordindex",
            [],
            |r| r.get(0),
        )?)
    } else {
        None
    };
    let original_count: Option<i64> = if has_original {
        Some(conn.query_row(
            "SELECT count(*) FROM topics WHERE strong_orig_word IS NOT NULL AND trim(strong_orig_word) <> ''",
            [], |r| r.get(0),
        )?)
    } else {
        None
    };
    let types = if has_type { "content_type" } else { "NULL" };
    let query = format!("SELECT subject, {types} FROM topics");
    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([])?;
    let mut keys: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    let mut types_count: BTreeMap<String, usize> = BTreeMap::new();
    let mut count = 0usize;
    while let Some(row) = rows.next()? {
        let subject: Option<String> = row.get(0)?;
        let key = subject.as_deref().unwrap_or("");
        let group = keys.entry(patterns.key_kind(key).to_string()).or_default();
        group.0 += 1;
        if group.1.len() < 10 {
            group.1.push(masked(key));
        }
        let content_type: Option<String> = row.get(1)?;
        *types_count
            .entry(content_type.unwrap_or_else(|| "NULL".into()))
            .or_default() += 1;
        count += 1;
    }
    let keys_json: BTreeMap<_, _> = keys
        .into_iter()
        .map(|(name, (count, samples))| (name, json!({"contagem": count, "amostras": samples})))
        .collect();

    let content_query = format!(
        "SELECT {types}, CAST(c.data AS BLOB) FROM topics t LEFT JOIN content c ON c.topic_id=t.id"
    );
    let mut content = conn.prepare(&content_query)?;
    let mut rows = content.query([])?;
    let mut citations: BTreeMap<&str, usize> = [
        ("TWOT", 0),
        ("LN", 0),
        ("DITAT", 0),
        ("G\\d+", 0),
        ("H\\d+", 0),
    ]
    .into_iter()
    .collect();
    let mut unavailable = 0usize;
    let mut degraded = 0usize;
    while let Some(row) = rows.next()? {
        let row_type: Option<String> = row.get(0)?;
        let data: Option<Vec<u8>> = row.get(1)?;
        let Some(data) = data else {
            unavailable += 1;
            continue;
        };
        let kind = row_type
            .as_deref()
            .or_else(|| config.get("content.type").map(String::as_str))
            .map(str::trim)
            .map(str::to_ascii_lowercase);
        let text = match kind.as_deref() {
            Some("rtf" | "rvf") => {
                match decode_twm_content_checked(kind.as_deref().unwrap_or_default(), &data) {
                    Ok(decoded) => {
                        degraded += usize::from(decoded.degraded);
                        plain_text(&decoded.nodes)
                    }
                    Err(_) => {
                        unavailable += 1;
                        continue;
                    }
                }
            }
            Some("html") => patterns
                .html
                .replace_all(&String::from_utf8_lossy(&data), " ")
                .into_owned(),
            _ => {
                unavailable += 1;
                continue;
            }
        };
        for found in patterns.citations.find_iter(&text) {
            let upper = found.as_str().to_ascii_uppercase();
            let group = if upper.starts_with('G') {
                "G\\d+"
            } else if upper.starts_with('H') {
                "H\\d+"
            } else if upper == "LN" {
                "LN"
            } else if upper == "TWOT" {
                "TWOT"
            } else {
                "DITAT"
            };
            if let Some(total) = citations.get_mut(group) {
                *total += 1;
            }
        }
    }
    Ok(json!({
        "arquivo": file_name(path), "tipo": "dicionario", "estado": "pronto",
        "titulo": config.get("title"), "idioma": config.get("lang"), "tamanho_bytes": size,
        "entradas": count, "abertura_ms": open_ms, "chaves": keys_json,
        "strong_orig_word_preenchido": original_count, "indice_termos_distintos": index_terms,
        "content_type": types_count, "content_type_padrao": config.get("content.type"),
        "citacoes": citations, "conteudo_nao_analisado": unavailable,
        "conteudo_degradado": degraded
    }))
}

fn bible(path: &Path, size: u64, patterns: &Patterns) -> Result<Value> {
    let reader = match OntReader::open(path) {
        Ok(reader) => reader,
        Err(ModuleError::EncryptedModule) => {
            return Ok(json!({
                "arquivo": file_name(path), "tipo": "biblia", "estado": "protegido",
                "titulo": null, "idioma": null, "tamanho_bytes": size, "entradas": null
            }))
        }
        Err(error) => return Err(error.into()),
    };
    let info = reader.info();
    let mut tags: BTreeMap<String, usize> = BTreeMap::new();
    let mut distinct_tags: BTreeMap<String, usize> = BTreeMap::new();
    let mut morph: BTreeMap<String, usize> = BTreeMap::new();
    let mut numeric_morph: BTreeMap<String, usize> = BTreeMap::new();
    let mut hebrew_vowels = false;
    let mut lemma_tags = 0usize;
    let mut strong = 0usize;
    let mut verse_count = reader.verse_count();
    while verse_count > 0 {
        let line = reader.line_by_index(verse_count - 1)?;
        let line = line.trim();
        if line.is_empty() || (line.starts_with('#') && line.contains('=')) {
            verse_count -= 1;
        } else {
            break;
        }
    }
    for index in 0..verse_count {
        let line = reader.line_by_index(index)?;
        hebrew_vowels |= line
            .chars()
            .any(|c| matches!(c as u32, 0x05b0..=0x05bd | 0x05bf | 0x05c1..=0x05c2 | 0x05c7));
        for cap in patterns.tags.captures_iter(&line) {
            let Some(name) = cap.get(1).map(|m| m.as_str()) else {
                continue;
            };
            let bare = name.trim_start_matches('/');
            let upper = bare.to_ascii_uppercase();
            *distinct_tags.entry(name.to_string()).or_default() += 1;
            let group = if upper.starts_with("WG") {
                "WG".to_string()
            } else if upper.starts_with("WH") {
                "WH".to_string()
            } else if upper.starts_with("WT") {
                "WT".to_string()
            } else {
                masked(name)
            };
            *tags.entry(group).or_default() += 1;
            if upper.starts_with("WG") || upper.starts_with("WH") {
                strong += 1;
            }
            if upper.starts_with("WT") {
                *morph.entry(bare[2..].to_string()).or_default() += 1;
            }
            if let Some((prefix, number)) = upper
                .strip_prefix('G')
                .map(|number| ('G', number))
                .or_else(|| upper.strip_prefix('H').map(|number| ('H', number)))
            {
                if number.parse::<u32>().is_ok_and(|value| {
                    (prefix == 'G' && (5625..=5773).contains(&value))
                        || (prefix == 'H' && (8675..=8809).contains(&value))
                }) {
                    *numeric_morph.entry(upper.clone()).or_default() += 1;
                }
            }
            if upper.contains("LEMMA")
                || cap
                    .get(0)
                    .is_some_and(|tag| tag.as_str().chars().any(|c| is_greek(c) || is_hebrew(c)))
            {
                lemma_tags += 1;
            }
        }
    }
    let mut morph: Vec<_> = morph.into_iter().collect();
    morph.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    morph.truncate(10);
    let morph: Vec<_> = morph
        .into_iter()
        .map(|(code, count)| (masked(&code), count))
        .collect();
    let mut numeric_morph: Vec<_> = numeric_morph.into_iter().collect();
    numeric_morph.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    numeric_morph.truncate(10);
    let numeric_morph: Vec<_> = numeric_morph
        .into_iter()
        .map(|(code, count)| (masked(&code), count))
        .collect();
    let distinct_tags: Vec<_> = distinct_tags
        .into_iter()
        .map(|(name, count)| (masked(&name), count))
        .collect();
    let language = info
        .language
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let identity = format!(
        "{} {}",
        info.title.to_ascii_lowercase(),
        file_name(path).to_ascii_lowercase()
    );
    let original = [
        "greek",
        "grego",
        "grega",
        "hebrew",
        "hebraico",
        "hebreu",
        "grc",
        "grk",
        "ell",
        "heb",
        "na28",
        "ubs5",
        "wlc",
        "bhs",
        "lxx",
        "textus receptus",
    ]
    .iter()
    .any(|term| language.contains(term) || identity.contains(term))
        || matches!(language.as_str(), "gr" | "el" | "he" | "hbo" | "iw" | "gre")
        || identity
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|part| part == "tr");
    Ok(json!({
        "arquivo": file_name(path), "tipo": "biblia", "estado": "pronto",
        "titulo": info.title, "idioma": info.language, "original": original,
        "tamanho_bytes": size, "entradas": verse_count,
        "linhas_indexadas": reader.verse_count(), "charset": reader.encoding_name(),
        "direcao": if matches!(language.as_str(), "he" | "heb" | "hbo" | "iw") || language.contains("hebrew") || language.contains("hebraico") { "RTL" } else { "LTR ou não determinada" },
        "vogais_hebraicas": hebrew_vowels, "tags": tags, "tags_distintas_mascaradas": distinct_tags,
        "morfologia_wt_top10_mascarada": morph, "morfologia_numerica_candidata_top10_mascarada": numeric_morph,
        "tags_com_lema": lemma_tags, "strong_tags": strong,
        "versificacao": "não verificável por contagem de linhas"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_text_and_classifies_keys() -> Result<()> {
        let p = Patterns::new()?;
        assert_eq!(p.key_kind("G25"), "strong");
        assert_eq!(p.key_kind("25.43"), "dominio_decimal");
        assert_eq!(p.key_kind("1234a"), "numero");
        assert_eq!(p.key_kind("λόγος"), "grego");
        assert_eq!(p.key_kind("אָב"), "hebraico");
        assert_eq!(p.key_kind(""), "vazio");
        assert_eq!(masked("G25 λόγος"), "x25 xxxxx");
        assert_eq!(masked(&"a".repeat(90)).len(), 60);
        Ok(())
    }

    #[test]
    fn citation_patterns_require_boundaries() -> Result<()> {
        let p = Patterns::new()?;
        let matches: Vec<_> = p
            .citations
            .find_iter("TWOT LN DITAT G25 H430 XG25 GLN")
            .map(|m| m.as_str())
            .collect();
        assert_eq!(matches, ["TWOT", "LN", "DITAT", "G25", "H430"]);
        Ok(())
    }

    #[test]
    fn protected_dictionary_is_only_listed() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("protected.dct.twm");
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE config(name TEXT, value TEXT);
             INSERT INTO config VALUES ('secure', '1');
             CREATE TABLE content(data BLOB);",
        )?;
        drop(conn);
        let result = dictionary(&path, path.metadata()?.len(), &Patterns::new()?)?;
        assert_eq!(result["estado"], "protegido");
        assert!(result["entradas"].is_null());
        assert!(result.get("citacoes").is_none());
        Ok(())
    }

    #[test]
    fn encrypted_signature_is_only_listed() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("encrypted.dct.twm");
        std::fs::write(&path, b"TWENCBMOD")?;
        let result = dictionary(&path, 9, &Patterns::new()?)?;
        assert_eq!(result["estado"], "protegido");
        assert!(result["entradas"].is_null());
        Ok(())
    }

    #[test]
    fn dictionary_without_optional_columns_is_inventoried() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("sample.dct.twm");
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE config(name TEXT, value TEXT);
             INSERT INTO config VALUES ('content.type', 'html');
             CREATE TABLE topics(id INTEGER, subject TEXT);
             CREATE TABLE content(topic_id INTEGER, data TEXT);
             INSERT INTO topics VALUES (1, '25.43'), (2, '');
             INSERT INTO content VALUES (1, '<p>G25 TWOT</p>');",
        )?;
        drop(conn);
        let result = dictionary(&path, path.metadata()?.len(), &Patterns::new()?)?;
        assert_eq!(result["entradas"], 2);
        assert_eq!(result["chaves"]["dominio_decimal"]["contagem"], 1);
        assert_eq!(result["chaves"]["vazio"]["contagem"], 1);
        assert_eq!(result["citacoes"]["G\\d+"], 1);
        assert_eq!(result["conteudo_nao_analisado"], 1);
        Ok(())
    }
}
