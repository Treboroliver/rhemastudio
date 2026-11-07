use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use regex::Regex;
use rhema_twformat::{decode_twm_content_checked, plain_text};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

struct Row {
    id: i64,
    subject: String,
    original: Option<String>,
    kind: String,
    bytes: Option<Vec<u8>>,
}

fn open(folder: &Path, name: &str) -> Result<Connection> {
    let path = folder.join(name);
    rhema_twformat::ensure_twm_unprotected(&path)
        .with_context(|| format!("Verificação de proteção: {}", path.display()))?;
    let db = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.execute_batch("PRAGMA query_only=ON")?;
    Ok(db)
}

fn rows<F>(db: &Connection, random: bool, mut visit: F) -> Result<()>
where
    F: FnMut(Row) -> Result<()>,
{
    let has_original: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('topics') WHERE name='strong_orig_word')",
        [],
        |r| r.get(0),
    )?;
    let has_type: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('topics') WHERE name='content_type')",
        [],
        |r| r.get(0),
    )?;
    let orig = if has_original {
        "t.strong_orig_word"
    } else {
        "NULL"
    };
    let kind = if has_type {
        "COALESCE(t.content_type, (SELECT value FROM config WHERE lower(trim(name))='content.type' LIMIT 1))"
    } else {
        "(SELECT value FROM config WHERE lower(trim(name))='content.type' LIMIT 1)"
    };
    let order = if random { " ORDER BY random()" } else { "" };
    let sql = format!("SELECT t.id, t.subject, {orig}, {kind}, CAST(c.data AS BLOB) FROM topics t LEFT JOIN content c ON c.topic_id=t.id{order}");
    let mut statement = db.prepare(&sql)?;
    let mut cursor = statement.query([])?;
    while let Some(row) = cursor.next()? {
        visit(Row {
            id: row.get(0)?,
            subject: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            original: row.get(2)?,
            kind: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            bytes: row.get(4)?,
        })?;
    }
    Ok(())
}

fn text(row: &Row) -> Result<Option<String>> {
    let Some(bytes) = &row.bytes else {
        return Ok(None);
    };
    let decoded = decode_twm_content_checked(&row.kind.to_ascii_lowercase(), bytes)
        .with_context(|| format!("Formato de entrada id {} ({})", row.id, row.kind))?;
    Ok(Some(plain_text(&decoded.nodes)))
}

fn fragments(text: &str, limit: usize) -> Vec<String> {
    let chars: Vec<char> = text
        .chars()
        .take(limit)
        .map(|c| {
            if c.is_alphabetic() || is_combining_mark(c) {
                'x'
            } else if c.is_whitespace() || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    chars.chunks(60).map(|part| part.iter().collect()).collect()
}

fn greek_words(value: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut word = String::new();
    for c in value.nfd() {
        if matches!(c as u32, 0x0370..=0x03ff | 0x1f00..=0x1fff) && c.is_alphabetic() {
            word.extend(c.to_lowercase().map(|ch| if ch == 'ς' { 'σ' } else { ch }));
        } else if !is_combining_mark(c) && !word.is_empty() {
            result.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        result.push(word);
    }
    result
}

fn script_key(value: &str, greek: bool) -> String {
    value
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .filter(|c| {
            if greek {
                matches!(*c as u32, 0x0370..=0x03ff | 0x1f00..=0x1fff) && c.is_alphabetic()
            } else {
                matches!(*c as u32, 0x05d0..=0x05ea | 0x05f0..=0x05f2)
            }
        })
        .flat_map(char::to_lowercase)
        .map(|c| if greek && c == 'ς' { 'σ' } else { c })
        .collect()
}

fn mean(n: usize, total: usize) -> Option<f64> {
    if total == 0 {
        None
    } else {
        Some(n as f64 / total as f64)
    }
}

fn percent(n: usize, total: usize) -> Option<f64> {
    mean(n, total).map(|ratio| 100.0 * ratio)
}

pub fn run(folder: &Path) -> Result<()> {
    let strong_db = open(folder, "Strong_AMG2.dct.twm")?;
    let louw_db = open(folder, "Louw-Nida2.dct.twm")?;
    let ditat_db = open(folder, "DITAT.dct.twm")?;
    let ditnt_db = open(folder, "DITNT.dct.twm")?;
    let champlin_db = open(folder, "DChamplin.dct.twm")?;
    let enc_db = open(folder, "EnciclopédiaTematica.dct.twm")?;
    let ndt_db = open(folder, "NDT.dct.twm")?;
    let html_db = open(folder, "Dicionário Ilustrado da Bíblia.dct.twm")?;

    let strong_number = Regex::new(r"(?i)\b[GH]\d+\b")?;
    let label = Regex::new(r"(?i)\b(?:DITAT|TWOT)\b")?;
    let decimal = Regex::new(r"^\d{1,2}\.\d+[a-z]?$")?;
    let integer = Regex::new(r"^\d+$")?;
    let number_suffix = Regex::new(r"(?i)\b\d+[a-z]\b")?;
    let mut strong = HashMap::new();
    let mut strong_samples = Vec::new();
    let mut greek_strong_sample = Vec::new();
    let mut label_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut label_samples: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
    let mut strong_missing = 0usize;
    rows(&strong_db, true, |row| {
        if let Some(original) = &row.original {
            if strong_number.is_match(&row.subject) {
                strong.insert(row.subject.to_ascii_uppercase(), original.clone());
                if row.subject.to_ascii_uppercase().starts_with('G')
                    && greek_strong_sample.len() < 200
                {
                    greek_strong_sample.push(original.clone());
                }
            }
        }
        let Some(body) = text(&row)? else {
            strong_missing += 1;
            return Ok(());
        };
        if strong_number.is_match(&row.subject) && strong_samples.len() < 5 {
            strong_samples.push(
                json!({"chave": fragments(&row.subject, 60), "inicio": fragments(&body, 80),
                "lema_campo": fragments(row.original.as_deref().unwrap_or(""), 60)}),
            );
        }
        let mut seen = HashSet::new();
        for found in label.find_iter(&body) {
            let key = found.as_str().to_ascii_uppercase();
            seen.insert(key.clone());
            let samples = label_samples.entry(key).or_default();
            if samples.len() < 5 {
                let before = body[..found.start()]
                    .chars()
                    .rev()
                    .take(20)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>();
                let after: String = body[found.end()..].chars().take(30).collect();
                samples.push(fragments(&format!("{before}{}{after}", found.as_str()), 60));
            }
        }
        for key in seen {
            *label_counts.entry(key).or_default() += 1;
        }
        Ok(())
    })?;
    let mut louw_total = 0usize;
    let mut louw_greek = 0usize;
    let mut louw_words = 0usize;
    let mut domains = Vec::new();
    let mut entries = Vec::new();
    let mut integer_total = 0usize;
    let mut decimal_total = 0usize;
    let mut louw_missing = 0usize;
    let mut domain_with_ranges = 0usize;
    let mut domain_numbers = HashSet::new();
    let mut entry_domains = HashSet::new();
    let mut entry_greek_context = Vec::new();
    let mut louw_matches: Vec<HashSet<i64>> = vec![HashSet::new(); greek_strong_sample.len()];
    let sample_lemmas: Vec<Vec<String>> = greek_strong_sample
        .iter()
        .map(|lemma| greek_words(lemma))
        .collect();
    rows(&louw_db, true, |row| {
        louw_total += 1;
        let Some(body) = text(&row)? else {
            louw_missing += 1;
            return Ok(());
        };
        let words = greek_words(&body);
        louw_greek += usize::from(!words.is_empty());
        louw_words += words.len();
        for (index, lemma) in sample_lemmas.iter().enumerate() {
            if !lemma.is_empty() && words.windows(lemma.len()).any(|part| part == lemma) {
                louw_matches[index].insert(row.id);
            }
        }
        if integer.is_match(&row.subject) {
            integer_total += 1;
            domain_numbers.insert(row.subject.clone());
            domain_with_ranges += usize::from(body.contains(&format!("{}.", row.subject)));
            if domains.len() < 5 {
                domains.push(
                    json!({"chave": fragments(&row.subject, 60), "inicio": fragments(&body, 120)}),
                );
            }
        } else if decimal.is_match(&row.subject) {
            decimal_total += 1;
            if let Some((domain, _)) = row.subject.split_once('.') {
                entry_domains.insert(domain.to_string());
            }
            if entries.len() < 5 {
                entries.push(
                    json!({"chave": fragments(&row.subject, 60), "inicio": fragments(&body, 120)}),
                );
            }
            if entry_greek_context.len() < 5 {
                if let Some((offset, _)) = body.char_indices().find(|(_, c)| {
                    matches!(*c as u32, 0x0370..=0x03ff | 0x1f00..=0x1fff) && c.is_alphabetic()
                }) {
                    let before: String = body[..offset]
                        .chars()
                        .rev()
                        .take(20)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect();
                    let after: String = body[offset..].chars().take(40).collect();
                    entry_greek_context.push(fragments(&format!("{before}{after}"), 60));
                }
            }
        }
        Ok(())
    })?;
    let mut matched = 0usize;
    let mut links = 0usize;
    let mut no_lemma = 0usize;
    for (lemma, ids) in sample_lemmas.iter().zip(&louw_matches) {
        if lemma.is_empty() {
            no_lemma += 1;
            continue;
        }
        matched += usize::from(!ids.is_empty());
        links += ids.len();
    }
    let ditat = mapping(
        &ditat_db,
        &strong,
        'H',
        &strong_number,
        &number_suffix,
        false,
    )?;
    let ditnt = mapping(
        &ditnt_db,
        &strong,
        'G',
        &strong_number,
        &number_suffix,
        true,
    )?;
    let rvf = json!({
        "Champlin": rvf_stats(&champlin_db)?, "Enciclopedia": rvf_stats(&enc_db)?,
        "NDT": rvf_stats(&ndt_db)?, "DITAT": rvf_stats(&ditat_db)?, "DITNT": rvf_stats(&ditnt_db)?
    });
    let html = html_stats(&html_db)?;
    let report = json!({
        "A": {"amostras_80": strong_samples, "entradas_com_citacao": label_counts,
            "citacoes_mascaradas": label_samples, "sem_conteudo": strong_missing},
        "B": {"total": louw_total, "com_grego": louw_greek, "palavras_gregas": louw_words,
            "media_palavras_por_entrada": mean(louw_words, louw_total),
            "media_palavras_por_entrada_com_grego": mean(louw_words, louw_greek),
            "inteiros": integer_total, "decimais": decimal_total, "sem_conteudo": louw_missing,
            "dominios_com_faixas_no_texto": domain_with_ranges,
            "dominios_com_entradas": domain_numbers.intersection(&entry_domains).count(),
            "amostras_dominios_120": domains, "amostras_entradas_120": entries,
            "contextos_gregos_mascarados": entry_greek_context},
        "C": ditat, "D": ditnt,
        "E": {"amostra": greek_strong_sample.len(), "sem_lema_grego": no_lemma, "com_correspondencia": matched,
            "percentual": percent(matched, greek_strong_sample.len()), "pares": links,
            "media_por_lema_amostrado": mean(links, greek_strong_sample.len()),
            "media_por_lema_com_correspondencia": mean(links, matched)},
        "F_rvf": rvf, "F_html": html
    });
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

fn mapping(
    db: &Connection,
    strong: &HashMap<String, String>,
    prefix: char,
    numbers: &Regex,
    suffix: &Regex,
    greek: bool,
) -> Result<Value> {
    let mut total = 0usize;
    let mut initial = 0usize;
    let mut initial_at_start = 0usize;
    let mut in_subject = 0usize;
    let mut after_subject = 0usize;
    let mut first_positions = [0usize; 4];
    let mut body_only = 0usize;
    let mut suffix_count = 0usize;
    let mut suffix_initial_count = 0usize;
    let mut number_prefix_keys = 0usize;
    let numeric_key = Regex::new(r"^\s*\d+[a-z]?(?:\s|$)")?;
    let mut missing = 0usize;
    let mut sample = 0usize;
    let mut compared = 0usize;
    let mut equal = 0usize;
    let mut absent_strong = 0usize;
    let mut empty_key = 0usize;
    let mut examples = Vec::new();
    rows(db, true, |row| {
        total += 1;
        number_prefix_keys += usize::from(numeric_key.is_match(&row.subject));
        in_subject += usize::from(
            numbers
                .find_iter(&row.subject)
                .any(|m| m.as_str().to_ascii_uppercase().starts_with(prefix)),
        );
        let Some(body) = text(&row)? else {
            missing += 1;
            return Ok(());
        };
        if examples.len() < 10 {
            examples.push(
                json!({"chave": fragments(&row.subject, 60), "inicio": fragments(&body, 100)}),
            );
        }
        suffix_count += usize::from(suffix.is_match(&body));
        let start: String = body.chars().take(200).collect();
        suffix_initial_count += usize::from(suffix.is_match(&start));
        let found = numbers
            .find_iter(&start)
            .find(|m| m.as_str().to_ascii_uppercase().starts_with(prefix));
        if let Some(number) = found {
            initial += 1;
            initial_at_start += usize::from(start[..number.start()].trim().is_empty());
            let position = start[..number.start()].chars().count();
            first_positions[(position / 50).min(3)] += 1;
            let trimmed = start.trim_start();
            let subject = row.subject.trim();
            after_subject += usize::from(
                !subject.is_empty()
                    && trimmed.starts_with(subject)
                    && number.start() >= start.len() - trimmed.len() + subject.len(),
            );
            if sample < 200 {
                sample += 1;
                let key = script_key(&row.subject, greek);
                if key.is_empty() {
                    empty_key += 1;
                } else if let Some(original) = strong.get(&number.as_str().to_ascii_uppercase()) {
                    let original = script_key(original, greek);
                    if original.is_empty() {
                        absent_strong += 1;
                    } else {
                        compared += 1;
                        equal += usize::from(key == original);
                    }
                } else {
                    absent_strong += 1;
                }
            }
        } else if numbers
            .find_iter(&body)
            .any(|m| m.as_str().to_ascii_uppercase().starts_with(prefix))
        {
            body_only += 1;
        }
        Ok(())
    })?;
    Ok(
        json!({"total": total, "com_numero_inicio_200": initial, "numero_na_posicao_zero": initial_at_start,
        "numero_apos_200": body_only, "com_numero_sufixo_no_texto": suffix_count, "sem_conteudo": missing,
        "com_numero_sufixo_inicio_200": suffix_initial_count, "chaves_com_prefixo_numerico": number_prefix_keys,
        "numero_na_chave": in_subject, "primeiro_numero_apos_chave_repetida": after_subject,
        "posicoes_primeiros_200_faixas_0_49_50_99_100_149_150_199": first_positions,
        "amostra": sample, "comparados": compared, "iguais": equal,
        "taxa_sobre_amostra": percent(equal, sample), "taxa_comparaveis": percent(equal, compared),
        "strong_ausente_ou_sem_lema": absent_strong, "chave_sem_lema": empty_key,
        "amostras_100": examples}),
    )
}

fn rvf_stats(db: &Connection) -> Result<Value> {
    let mut total = 0usize;
    let mut converted = 0usize;
    let mut degraded = 0usize;
    let mut suspicious = 0usize;
    let mut missing = 0usize;
    let mut nanos = 0u128;
    rows(db, false, |row| {
        if !row.kind.eq_ignore_ascii_case("rvf") {
            return Ok(());
        }
        total += 1;
        let Some(bytes) = row.bytes else {
            missing += 1;
            return Ok(());
        };
        let start = Instant::now();
        let result = decode_twm_content_checked("rvf", &bytes)?;
        nanos += start.elapsed().as_nanos();
        converted += 1;
        degraded += usize::from(result.degraded);
        let text = plain_text(&result.nodes);
        suspicious += usize::from(text.chars().any(|c| {
            matches!(c as u32,
            0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff | 0x20000..=0x323af |
            0xe000..=0xf8ff | 0xf0000..=0xffffd | 0x100000..=0x10fffd | 0xfffd)
        }));
        Ok(())
    })?;
    Ok(
        json!({"entradas_rvf": total, "convertidas": converted, "degradadas": degraded,
        "suspeitas": suspicious, "sem_conteudo": missing,
        "media_ms": (converted > 0).then(|| nanos as f64 / converted as f64 / 1_000_000.0)}),
    )
}

fn html_stats(db: &Connection) -> Result<Value> {
    let tags = Regex::new(r#"(?is)<\s*(/?)\s*([a-z][a-z0-9:-]*)\b((?:[^>"']|"[^"]*"|'[^']*')*)>"#)?;
    let attrs = Regex::new(
        r#"(?i)(?:^|\s)([a-z_:][a-z0-9_:.-]*)(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s"'=<>`]+))?"#,
    )?;
    let mut tag_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut attr_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut count = 0usize;
    let mut missing = 0usize;
    let mut bytes_total = 0usize;
    let mut invalid_utf8 = 0usize;
    rows(db, false, |row| {
        if !row.kind.eq_ignore_ascii_case("html") {
            return Ok(());
        }
        let Some(bytes) = row.bytes else {
            missing += 1;
            return Ok(());
        };
        count += 1;
        bytes_total += bytes.len();
        invalid_utf8 += usize::from(std::str::from_utf8(&bytes).is_err());
        let body = String::from_utf8_lossy(&bytes);
        for capture in tags.captures_iter(&body) {
            let name = capture[2].to_ascii_lowercase();
            *tag_counts.entry(name).or_default() += 1;
            if &capture[1] == "/" {
                continue;
            }
            for attr in attrs.captures_iter(&capture[3]) {
                *attr_counts.entry(attr[1].to_ascii_lowercase()).or_default() += 1;
            }
        }
        Ok(())
    })?;
    if count == 0 {
        bail!("Nenhuma entrada HTML legível");
    }
    Ok(
        json!({"entradas": count, "sem_conteudo": missing, "utf8_invalido": invalid_utf8,
        "media_bytes": bytes_total as f64 / count as f64, "tags": tag_counts, "atributos": attr_counts,
        "metodo": "contagem lexical de tags HTML; aberturas e fechamentos são somados"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_and_mask() {
        assert_eq!(greek_words("ἄνθρωπος λόγος"), ["ανθρωποσ", "λογοσ"]);
        assert_eq!(script_key("אָב", false), "אב");
        assert_eq!(script_key("Ἄ", true), "α");
        let chunks = fragments(&"á".repeat(120), 120);
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().all(|s| s.len() == 60));
    }

    #[test]
    fn empty_safe() {
        assert!(greek_words("").is_empty());
        assert_eq!(percent(0, 0), None);
    }

    #[test]
    fn refuses_protected_dictionary() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("protected.dct.twm");
        let db = Connection::open(path)?;
        db.execute_batch("CREATE TABLE config(name TEXT, value TEXT); INSERT INTO config VALUES ('secure', '1');")?;
        drop(db);
        assert!(open(dir.path(), "protected.dct.twm").is_err());
        Ok(())
    }

    #[test]
    fn maps_synthetic_entries() -> Result<()> {
        let db = Connection::open_in_memory()?;
        db.execute_batch("CREATE TABLE config(name TEXT, value TEXT); INSERT INTO config VALUES ('content.type', 'rtf'); CREATE TABLE topics(id INTEGER, subject TEXT); CREATE TABLE content(topic_id INTEGER, data BLOB); INSERT INTO topics VALUES (1, '1 אב'), (2, '2 גד'); INSERT INTO content VALUES (1, '{\\rtf1 H1}'), (2, '{\\rtf1 H2}');")?;
        let strong = HashMap::from([
            ("H1".to_string(), "אָב".to_string()),
            ("H2".to_string(), "דג".to_string()),
        ]);
        let report = mapping(
            &db,
            &strong,
            'H',
            &Regex::new(r"(?i)\b[GH]\d+\b")?,
            &Regex::new(r"(?i)\b\d+[a-z]\b")?,
            false,
        )?;
        assert_eq!(report["amostra"], 2);
        assert_eq!(report["iguais"], 1);
        Ok(())
    }
}
