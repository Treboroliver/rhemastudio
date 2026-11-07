use serde::Serialize;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Script {
    Greek,
    Hebrew,
    Latin,
    Mixed,
    None,
}

pub fn detect_script(text: &str) -> Script {
    let mut greek = false;
    let mut hebrew = false;
    let mut latin = false;
    for c in text.nfd().filter(|c| !is_combining_mark(*c)) {
        let n = c as u32;
        if c.is_alphabetic() && matches!(n, 0x0370..=0x03ff | 0x1f00..=0x1fff) {
            greek = true;
        } else if c.is_alphabetic() && matches!(n, 0x05d0..=0x05f2) {
            hebrew = true;
        } else if c.is_ascii_alphabetic()
            || (c.is_alphabetic() && matches!(n, 0x00c0..=0x024f | 0x1e00..=0x1eff))
        {
            latin = true;
        }
    }
    match (greek, hebrew, latin) {
        (true, false, false) => Script::Greek,
        (false, true, false) => Script::Hebrew,
        (false, false, true) => Script::Latin,
        (false, false, false) => Script::None,
        _ => Script::Mixed,
    }
}

fn fold(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn fold_greek(text: &str) -> String {
    fold(text)
        .chars()
        .map(|c| if c == 'ς' { 'σ' } else { c })
        .collect()
}

pub fn fold_latin(text: &str) -> String {
    fold(text)
}

pub fn fold_hebrew(text: &str) -> String {
    fold(text)
        .chars()
        .filter_map(|c| match c {
            '*'
            | '\u{05be}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{061c}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}' => None,
            'ך' => Some('כ'),
            'ם' => Some('מ'),
            'ן' => Some('נ'),
            'ף' => Some('פ'),
            'ץ' => Some('צ'),
            _ => Some(c),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum Query {
    Strong(String),
    LouwNida(String),
    Ditat(String),
    Greek(String),
    Hebrew(String),
    Latin(String),
}

fn number_with_suffix(value: &str) -> Option<String> {
    let value = value.trim();
    let digit_end = value
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(value.len());
    if digit_end == 0 {
        return None;
    }
    let number: u32 = value[..digit_end].parse().ok()?;
    if number == 0 {
        return None;
    }
    let suffix = &value[digit_end..];
    if suffix.len() > 1 || !suffix.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Some(format!("{number}{}", suffix.to_ascii_lowercase()))
}

pub fn parse_query(input: &str) -> Vec<Query> {
    let text = input.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    if let Some(first) = text.chars().next() {
        if matches!(first, 'g' | 'G' | 'h' | 'H') {
            let rest = text[first.len_utf8()..].trim();
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(number) = rest.parse::<u32>() {
                    if number > 0 {
                        result.push(Query::Strong(format!(
                            "{}{number}",
                            first.to_ascii_uppercase()
                        )));
                        return result;
                    }
                }
                return result;
            }
        }
    }
    let prefix = text.split_whitespace().next().unwrap_or("");
    if prefix.eq_ignore_ascii_case("ditat") || prefix.eq_ignore_ascii_case("twot") {
        if let Some(value) = text.get(prefix.len()..).and_then(number_with_suffix) {
            result.push(Query::Ditat(value));
        }
        return result;
    }
    if let Some((domain, sub)) = text.split_once('.') {
        if !domain.is_empty()
            && !sub.is_empty()
            && domain.chars().all(|c| c.is_ascii_digit())
            && sub.chars().all(|c| c.is_ascii_digit())
        {
            if let (Ok(d), Ok(s)) = (domain.parse::<u32>(), sub.parse::<u32>()) {
                if d > 0 && s > 0 {
                    result.push(Query::LouwNida(format!("{d}.{s}")));
                }
            }
            return result;
        }
    }
    if let Some(value) = number_with_suffix(text) {
        if value.chars().all(|c| c.is_ascii_digit()) {
            result.push(Query::LouwNida(value.clone()));
        }
        result.push(Query::Ditat(value));
        return result;
    }
    let valid_text = text.chars().all(|c| {
        c.is_alphabetic()
            || is_combining_mark(c)
            || c.is_whitespace()
            || matches!(
                c,
                '\'' | '’' | '-' | '*' | '\u{05be}' | '\u{200e}' | '\u{200f}'
            )
    });
    if valid_text {
        match detect_script(text) {
            Script::Greek => result.push(Query::Greek(fold_greek(text))),
            Script::Hebrew => result.push(Query::Hebrew(fold_hebrew(text))),
            Script::Latin => result.push(Query::Latin(fold_latin(text))),
            _ => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_scripts() {
        assert_eq!(fold_greek("Ἄΐς"), "αισ");
        assert_eq!(fold_hebrew("*מֶ֖לֶךְ־אָב\u{200f}"), "מלכאב");
        assert_eq!(fold_hebrew("ךםןףץ"), "כמנפצ");
        assert_eq!(fold_latin("Mōisés"), "moises");
        assert_eq!(detect_script("λόγος"), Script::Greek);
        assert_eq!(detect_script("שָׁלוֹם"), Script::Hebrew);
        assert_eq!(detect_script("ação"), Script::Latin);
        assert_eq!(detect_script("xα"), Script::Mixed);
        assert_eq!(detect_script("123"), Script::None);
    }

    #[test]
    fn parses_more_than_forty_queries() {
        let cases = [
            ("G25", vec![Query::Strong("G25".into())]),
            ("g 25", vec![Query::Strong("G25".into())]),
            ("H0430", vec![Query::Strong("H430".into())]),
            ("h430", vec![Query::Strong("H430".into())]),
            (" G0001 ", vec![Query::Strong("G1".into())]),
            ("h 0003", vec![Query::Strong("H3".into())]),
            ("25.43", vec![Query::LouwNida("25.43".into())]),
            ("01.002", vec![Query::LouwNida("1.2".into())]),
            ("93.615", vec![Query::LouwNida("93.615".into())]),
            ("1.1", vec![Query::LouwNida("1.1".into())]),
            (
                "25",
                vec![Query::LouwNida("25".into()), Query::Ditat("25".into())],
            ),
            (
                "001",
                vec![Query::LouwNida("1".into()), Query::Ditat("1".into())],
            ),
            ("DITAT 57", vec![Query::Ditat("57".into())]),
            ("DITAT 100a", vec![Query::Ditat("100a".into())]),
            ("twot 57", vec![Query::Ditat("57".into())]),
            ("57a", vec![Query::Ditat("57a".into())]),
            ("001B", vec![Query::Ditat("1b".into())]),
            ("TWOT 2C", vec![Query::Ditat("2c".into())]),
            ("λόγος", vec![Query::Greek("λογοσ".into())]),
            ("ΛΌΓΟΣ", vec![Query::Greek("λογοσ".into())]),
            ("ἄνθρωπος", vec![Query::Greek("ανθρωποσ".into())]),
            ("Ἄ", vec![Query::Greek("α".into())]),
            ("שלום", vec![Query::Hebrew("שלומ".into())]),
            ("שָׁלוֹם", vec![Query::Hebrew("שלומ".into())]),
            ("אָב", vec![Query::Hebrew("אב".into())]),
            ("*אָב", vec![Query::Hebrew("אב".into())]),
            ("amor", vec![Query::Latin("amor".into())]),
            ("Graça", vec![Query::Latin("graca".into())]),
            ("Mōs", vec![Query::Latin("mos".into())]),
            ("agapao", vec![Query::Latin("agapao".into())]),
            ("", vec![]),
            ("  ", vec![]),
            ("0", vec![]),
            ("G0", vec![]),
            ("H-1", vec![]),
            ("25.", vec![]),
            (".43", vec![]),
            ("1.0", vec![]),
            ("1.2.3", vec![]),
            ("DITAT", vec![]),
            ("TWOT x", vec![]),
            ("2aa", vec![]),
            ("99999999999999999999", vec![]),
            ("@!", vec![]),
            ("αa", vec![]),
            ("G 1 2", vec![]),
        ];
        assert!(cases.len() >= 40);
        for (input, expected) in cases {
            assert_eq!(parse_query(input), expected, "{input}");
        }
    }
}
