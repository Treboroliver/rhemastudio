use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::{fold_greek, fold_hebrew, fold_latin, parse_query, Query};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StrongEntry {
    pub number: String,
    pub lemma: String,
    pub transliteration: Option<String>,
    pub cited_numbers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LouwNidaEntry {
    pub key: String,
    pub domain: u32,
    pub entry: Option<u32>,
    pub subdomain: Option<String>,
    pub lexemes: Vec<String>,
    pub header_recognized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DitatSubentry {
    pub number: String,
    pub lemma: String,
    pub strong_numbers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DitatEntry {
    pub number: String,
    pub lemma: String,
    pub transliteration: Option<String>,
    pub strong_numbers: Vec<String>,
    pub subentries: Vec<DitatSubentry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DitntEntry {
    pub lemmas: Vec<String>,
    pub initial_strong: Vec<String>,
    pub all_strong: Vec<String>,
    pub initial_count: usize,
    pub total_count: usize,
}

const PATTERNS: &[&str] = &[
    r"(?i)\b([GH])\s*(\d+)\b",
    r"(?i)\bde\s+(\d+(?:\s*(?:,|e|ou)\s*\d+)*)\b",
    r"\d+",
    r"(?i)\bsubdom[ií]nio\s*:",
    r"\(\s*\d{1,2}\.\d+[^)]*\)|(?:-\s*){3,}",
    r"(?i)^\s*(\d+[a-z]?)\b",
    r"(?i)^(?:\s*<H\d+>\s*,?\s*)+",
    r"(?i)\b(\d+[a-z])\s+[\s*†\u{200e}\u{200f}]*([\p{Hebrew}\p{M}]+)[^<>]{0,80}(?:<H\d+>)(?:\s*,?\s*<H\d+>)*",
];

fn patterns() -> Result<&'static HashMap<&'static str, Regex>, &'static regex::Error> {
    static CACHE: OnceLock<Result<HashMap<&'static str, Regex>, regex::Error>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            PATTERNS
                .iter()
                .map(|pattern| Regex::new(pattern).map(|compiled| (*pattern, compiled)))
                .collect()
        })
        .as_ref()
}

pub(crate) fn validate_patterns() -> Result<(), String> {
    patterns().map(|_| ()).map_err(ToString::to_string)
}

fn regex(pattern: &str) -> Option<&'static Regex> {
    patterns().ok()?.get(pattern)
}

fn first_parenthesis(text: &str) -> Option<String> {
    let start = text.find('(')? + 1;
    let end = text[start..].find(')')? + start;
    let value = text[start..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn words(text: &str, greek: bool) -> Vec<String> {
    let mut result = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        let value = c as u32;
        if (greek && matches!(value, 0x0370..=0x03ff | 0x1f00..=0x1fff) && c.is_alphabetic())
            || (!greek && matches!(value, 0x05d0..=0x05ea | 0x05f0..=0x05f2))
            || (unicode_normalization::char::is_combining_mark(c) && !word.is_empty())
        {
            word.push(c);
        } else if !word.is_empty() {
            result.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        result.push(word);
    }
    result
}

fn strong_numbers(text: &str, prefix: char) -> Vec<String> {
    let Some(pattern) = regex(r"(?i)\b([GH])\s*(\d+)\b") else {
        return Vec::new();
    };
    pattern
        .captures_iter(text)
        .filter_map(|cap| {
            if !cap[1].eq_ignore_ascii_case(&prefix.to_string()) {
                return None;
            }
            let n: u32 = cap[2].parse().ok()?;
            (n > 0).then(|| format!("{prefix}{n}"))
        })
        .collect()
}

pub fn extract_strong(key: &str, original: &str, text: &str) -> Option<StrongEntry> {
    let number = match parse_query(key).first()? {
        Query::Strong(n) => n.clone(),
        _ => return None,
    };
    let language = number.chars().next()?;
    let citation = regex(r"(?i)\bde\s+(\d+(?:\s*(?:,|e|ou)\s*\d+)*)\b")?;
    let digits = regex(r"\d+")?;
    let cited_numbers = citation
        .captures_iter(text)
        .flat_map(|capture| digits.find_iter(capture.get(1).map_or("", |m| m.as_str())))
        .filter_map(|value| {
            let number: u32 = value.as_str().parse().ok()?;
            (number > 0).then(|| format!("{language}{number}"))
        })
        .collect();
    Some(StrongEntry {
        number,
        lemma: original.trim().to_string(),
        transliteration: first_parenthesis(text),
        cited_numbers,
    })
}

pub fn extract_louw_nida(key: &str, text: &str) -> Option<LouwNidaEntry> {
    let query = parse_query(key);
    let normalized = match query.first()? {
        Query::LouwNida(n) => n,
        _ => return None,
    };
    let (domain, entry) = if let Some((a, b)) = normalized.split_once('.') {
        (a.parse().ok()?, Some(b.parse().ok()?))
    } else {
        (normalized.parse().ok()?, None)
    };
    if entry.is_none() {
        return Some(LouwNidaEntry {
            key: key.to_string(),
            domain,
            entry,
            subdomain: None,
            lexemes: Vec::new(),
            header_recognized: true,
        });
    }
    let subdomain_marker = regex(r"(?i)\bsubdom[ií]nio\s*:")?;
    let range = regex(r"\(\s*\d{1,2}\.\d+[^)]*\)|(?:-\s*){3,}")?;
    let subdomain = subdomain_marker.find(text).and_then(|marker| {
        let rest = text[marker.end()..].trim_start();
        let mut chars = rest.char_indices();
        let (_, first) = chars.next()?;
        if !first.is_ascii_alphabetic() {
            return None;
        }
        let mut end = first.len_utf8();
        if let Some((i, next)) = chars.next() {
            if matches!(next, '\'' | '’') {
                end = i + next.len_utf8();
            }
        }
        rest[end..]
            .trim_start()
            .starts_with('-')
            .then(|| rest[..end].to_string())
    });
    let header = subdomain_marker.find(text).and_then(|marker| {
        let rest = &text[marker.end()..];
        range.find(rest).map(|range| &rest[range.end()..])
    });
    let start = header.map(|s| {
        s.trim_start_matches(|c: char| {
            c.is_whitespace() || matches!(c, '\u{200e}' | '\u{200f}' | '\u{200b}')
        })
    });
    let valid_start = start.and_then(|s| s.chars().next()).is_some_and(|c| {
        c.is_alphabetic() && matches!(c as u32, 0x0370..=0x03ff | 0x1f00..=0x1fff)
    });
    let lexemes = if let Some(rest) = start.filter(|_| valid_start) {
        let end = rest
            .char_indices()
            .find(|(_, c)| {
                c.is_alphabetic() && !matches!(*c as u32, 0x0370..=0x03ff | 0x1f00..=0x1fff)
            })
            .map_or(rest.len(), |(i, _)| i);
        rest[..end]
            .split(';')
            .filter_map(|segment| {
                let lemma = segment.split(',').next().unwrap_or("");
                let parts = words(lemma, true);
                (!parts.is_empty()).then(|| parts.join(" "))
            })
            .collect()
    } else {
        Vec::new()
    };
    Some(LouwNidaEntry {
        key: key.to_string(),
        domain,
        entry,
        subdomain,
        lexemes,
        header_recognized: valid_start,
    })
}

pub fn extract_ditat(key: &str, text: &str) -> Option<DitatEntry> {
    let number_pattern = regex(r"(?i)^\s*(\d+[a-z]?)\b")?;
    let number = number_pattern
        .captures(key)?
        .get(1)?
        .as_str()
        .to_ascii_lowercase();
    let lemma = words(key, false).first()?.clone();
    let transliteration = first_parenthesis(key);
    let start = text
        .trim_start()
        .strip_prefix(key.trim())
        .unwrap_or(text.trim_start())
        .trim_start();
    let header = regex(r"(?i)^(?:\s*<H\d+>\s*,?\s*)+")?;
    let strong = header
        .find(start)
        .map(|m| strong_numbers(m.as_str(), 'H'))
        .unwrap_or_default();
    let sub_pattern = regex(
        r"(?i)\b(\d+[a-z])\s+[\s*†\u{200e}\u{200f}]*([\p{Hebrew}\p{M}]+)[^<>]{0,80}(?:<H\d+>)(?:\s*,?\s*<H\d+>)*",
    )?;
    let subentries = sub_pattern
        .captures_iter(start)
        .filter_map(|cap| {
            let whole = cap.get(0)?.as_str();
            Some(DitatSubentry {
                number: cap.get(1)?.as_str().to_ascii_lowercase(),
                lemma: cap.get(2)?.as_str().to_string(),
                strong_numbers: strong_numbers(whole, 'H'),
            })
        })
        .collect();
    Some(DitatEntry {
        number,
        lemma,
        transliteration,
        strong_numbers: strong,
        subentries,
    })
}

pub fn extract_ditnt(key: &str, text: &str) -> DitntEntry {
    let lemmas = words(key, true);
    let first: String = text.chars().take(400).collect();
    let initial_strong = strong_numbers(&first, 'G');
    let all_strong = strong_numbers(text, 'G');
    DitntEntry {
        lemmas,
        initial_count: initial_strong.len(),
        total_count: all_strong.len(),
        initial_strong,
        all_strong,
    }
}

/// Limpa marcações de título (`[[@Headword:X]]`, tags HTML e entidades) sem alterar o módulo.
pub fn clean_title(title: &str) -> String {
    let mut no_headword = String::with_capacity(title.len());
    let mut rest = title;
    while let Some(start) = rest.find("[[@") {
        no_headword.push_str(&rest[..start]);
        let after = &rest[start + 3..];
        if let Some(end) = after.find("]]") {
            rest = &after[end + 2..];
        } else {
            no_headword.push_str(&rest[start..]);
            rest = "";
            break;
        }
    }
    no_headword.push_str(rest);

    let mut no_tags = String::with_capacity(no_headword.len());
    let mut in_tag = false;
    for ch in no_headword.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                no_tags.push(' ');
            }
            _ if !in_tag => no_tags.push(ch),
            _ => {}
        }
    }

    let decoded = decode_title_entities(&no_tags);
    let cleaned = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        if let Some(pos) = title.find("[[@Headword:") {
            let after = &title[pos + 11..];
            if let Some(end) = after.find("]]") {
                return after[..end].trim().to_string();
            }
        }
    }
    cleaned
}

fn decode_title_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'&' {
            if let Some(semi) = s[i + 1..s.len().min(i + 16)].find(';') {
                let body = &s[i + 1..i + 1 + semi];
                let ch = if let Some(hex) =
                    body.strip_prefix("#x").or_else(|| body.strip_prefix("#X"))
                {
                    u32::from_str_radix(hex, 16)
                        .ok()
                        .filter(|&c| c > 0)
                        .and_then(char::from_u32)
                } else if let Some(dec) = body.strip_prefix('#') {
                    dec.parse::<u32>()
                        .ok()
                        .filter(|&c| c > 0)
                        .and_then(char::from_u32)
                } else {
                    match body {
                        "amp" => Some('&'),
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "quot" => Some('"'),
                        "apos" => Some('\''),
                        "nbsp" => Some(' '),
                        "aacute" => Some('á'),
                        "Aacute" => Some('Á'),
                        "agrave" => Some('à'),
                        "Agrave" => Some('À'),
                        "acirc" => Some('â'),
                        "Acirc" => Some('Â'),
                        "atilde" => Some('ã'),
                        "Atilde" => Some('Ã'),
                        "eacute" => Some('é'),
                        "Eacute" => Some('É'),
                        "ecirc" => Some('ê'),
                        "Ecirc" => Some('Ê'),
                        "iacute" => Some('í'),
                        "Iacute" => Some('Í'),
                        "oacute" => Some('ó'),
                        "Oacute" => Some('Ó'),
                        "ocirc" => Some('ô'),
                        "Ocirc" => Some('Ô'),
                        "otilde" => Some('õ'),
                        "Otilde" => Some('Õ'),
                        "uacute" => Some('ú'),
                        "Uacute" => Some('Ú'),
                        "ccedil" => Some('ç'),
                        "Ccedil" => Some('Ç'),
                        _ => None,
                    }
                };
                if let Some(ch) = ch {
                    out.push(ch);
                    i += semi + 2;
                    continue;
                }
            }
        }
        if let Some(ch) = s[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    out
}

pub fn extract_generic(title: &str) -> String {
    fold_latin(&clean_title(title))
}

/// Extrai citações explícitas de Strong (`G3056`, `H430`, `{G3056}`, `Strong's #3056`, `Strong 3056`).
/// Números soltos sem prefixo `G`/`H` nem marcador `Strong` são ignorados.
pub fn extract_explicit_strong_refs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    if let Some(gh) = regex(r"(?i)\b([GH])\s*(\d+)\b") {
        for cap in gh.captures_iter(text) {
            if cap[2].len() <= 5 {
                if let Ok(n) = cap[2].parse::<u32>() {
                    if n > 0 {
                        let prefix = cap[1].to_ascii_uppercase();
                        let code = format!("{prefix}{n}");
                        if seen.insert(code.clone()) {
                            out.push(code);
                        }
                    }
                }
            }
        }
    }
    if let Ok(strong_bare) = Regex::new(r"(?i)\bstrong(?:'s|s)?\s*#?\s*(\d{1,5})\b") {
        for cap in strong_bare.captures_iter(text) {
            if let Ok(n) = cap[1].parse::<u32>() {
                if n > 0 {
                    let code = format!("S{n}");
                    if seen.insert(code.clone()) {
                        out.push(code);
                    }
                }
            }
        }
    }
    out
}

/// Separa um título de dicionário comum em `(lemas_gregos_ou_hebraicos, parte_latina)`.
pub fn split_mixed_title(title: &str) -> (Vec<String>, String) {
    let cleaned = clean_title(title);
    let mut lemmas = words(&cleaned, true);
    lemmas.extend(words(&cleaned, false));
    let mut latin = String::with_capacity(cleaned.len());
    for c in cleaned.chars() {
        let v = c as u32;
        let is_gk_hb = matches!(v, 0x0370..=0x03ff | 0x1f00..=0x1fff | 0x0590..=0x05ff)
            || unicode_normalization::char::is_combining_mark(c);
        if is_gk_hb {
            latin.push(' ');
        } else {
            latin.push(c);
        }
    }
    let latin_norm = fold_latin(
        &latin
            .split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
    );
    (lemmas, latin_norm)
}

pub(crate) fn normalized_greek(lemma: &str) -> String {
    fold_greek(lemma).trim().to_string()
}
pub(crate) fn normalized_hebrew(lemma: &str) -> String {
    fold_hebrew(lemma).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_extractors_from_synthetic_data() {
        let s =
            extract_strong("G25", "ἄνθρωπος", "G25 ἄνθρωπος (anthrōpos) de 1 e 2").expect("strong");
        assert_eq!(s.cited_numbers, ["G1", "G2"]);
        assert_eq!(s.transliteration.as_deref(), Some("anthrōpos"));
        let l = extract_louw_nida(
            "25.43",
            "Domínio: 25 Subdomínio: A (25.1 a 25.9) ἄνθρωπος; λόγος Significado: σάρξ",
        )
        .expect("louw");
        assert_eq!(l.lexemes, ["ἄνθρωπος", "λόγος"]);
        assert_eq!(l.domain, 25);
        assert_eq!(l.entry, Some(43));
        assert_eq!(l.subdomain.as_deref(), None);
        assert_eq!(
            extract_louw_nida(
                "25.43",
                "Domínio: 25 Subdomínio: A - teste (25.1 a 25.9) λόγος Significado: x"
            )
            .expect("louw")
            .subdomain
            .as_deref(),
            Some("A")
        );
        assert_eq!(
            extract_louw_nida(
                "25.43",
                "Domínio: 25 Subdomínio: A (25.1 a 25.9) λόγος, ὁ; σάρξ, ἡ Significado: πίστις"
            )
            .expect("louw")
            .lexemes,
            ["λόγος", "σάρξ"]
        );
        assert_eq!(
            extract_louw_nida(
                "25.43",
                "Domínio: 25 Subdomínio: A (25.1 a 25.9) λόγος descrição: σάρξ"
            )
            .expect("louw")
            .lexemes,
            ["λόγος"]
        );
        assert!(
            !extract_louw_nida(
                "25.43",
                "Domínio: 25 Subdomínio: A (25.1 a 25.9) descrição: σάρξ"
            )
            .expect("louw")
            .header_recognized
        );
        assert!(extract_louw_nida("25", "")
            .expect("domain")
            .subdomain
            .is_none());
        let d = extract_ditat(
            "100 אָב (av)",
            "100 אָב (av) <H1>, <H2> texto 100a אָב (av) <H3> texto",
        )
        .expect("ditat");
        assert_eq!(d.strong_numbers, ["H1", "H2"]);
        assert_eq!(d.subentries[0].number, "100a");
        assert_eq!(d.subentries[0].strong_numbers, ["H3"]);
        let nt = extract_ditnt("λόγος, πίστις", "G25 G26");
        assert_eq!(nt.lemmas, ["λόγος", "πίστις"]);
        assert_eq!(nt.total_count, 2);
        assert_eq!(extract_generic("<b>Coração</b>"), "coracao");
        assert_eq!(clean_title("[[@Headword:CREDO]] CREDO"), "CREDO");
        assert_eq!(clean_title("<b>F&eacute;</b> &amp; &#233;"), "Fé & é");
        assert_eq!(extract_generic("[[@Headword:AMOR]] AMOR "), "amor");
        assert_eq!(
            extract_explicit_strong_refs(
                "Veja G3056, {H430}, Strong's #123, Strong 45 e numero 999."
            ),
            vec!["G3056", "H430", "S123", "S45"]
        );
        assert_eq!(
            split_mixed_title("ἀγαπάω (amar)"),
            (vec!["ἀγαπάω".to_string()], "amar".to_string())
        );
        assert!(extract_strong("", "", "").is_none());
        assert!(extract_ditat("?", "").is_none());
    }
}
