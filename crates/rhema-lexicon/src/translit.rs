use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

use crate::fold_latin;

/// Transliteração simples do grego para letras latinas sem acentos.
///
/// Esquema: α a, β b, γ g (n antes de γ κ ξ χ), δ d, ε e, ζ z, η e, θ th, ι i, κ k, λ l,
/// μ m, ν n, ξ x, ο o, π p, ρ r, σ/ς s, τ t, υ u, φ ph, χ ch, ψ ps, ω o. Espírito áspero
/// nas duas primeiras letras da palavra acrescenta `h` (ou `rh` em ρ inicial); demais
/// acentos, espírito brando e iota subscrito são descartados. Caracteres não gregos são
/// ignorados.
pub fn translit_greek(text: &str) -> String {
    text.split_whitespace()
        .map(word)
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn word(text: &str) -> String {
    let mut letters = Vec::new();
    let mut rough = false;
    for c in text.nfd() {
        if c == '\u{0314}' && letters.len() <= 2 {
            rough = true;
        } else if !is_combining_mark(c) {
            letters.extend(c.to_lowercase());
        }
    }
    let mut out = String::new();
    for (i, &c) in letters.iter().enumerate() {
        let next = letters.get(i + 1).copied();
        let piece = match c {
            'α' => "a",
            'β' => "b",
            'γ' if matches!(next, Some('γ' | 'κ' | 'ξ' | 'χ')) => "n",
            'γ' => "g",
            'δ' => "d",
            'ε' => "e",
            'ζ' => "z",
            'η' => "e",
            'θ' => "th",
            'ι' => "i",
            'κ' => "k",
            'λ' => "l",
            'μ' => "m",
            'ν' => "n",
            'ξ' => "x",
            'ο' => "o",
            'π' => "p",
            'ρ' if rough && i == 0 => "rh",
            'ρ' => "r",
            'σ' | 'ς' => "s",
            'τ' => "t",
            'υ' => "u",
            'φ' => "ph",
            'χ' => "ch",
            'ψ' => "ps",
            'ω' => "o",
            _ => "",
        };
        out.push_str(piece);
    }
    if rough && !out.is_empty() && letters.first() != Some(&'ρ') {
        out.insert(0, 'h');
    }
    out
}

/// Normalização usada no campo `translit`: `fold_latin` e somente letras/dígitos ASCII.
pub(crate) fn translit_norm(text: &str) -> String {
    fold_latin(text)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else if c.is_whitespace() {
                ' '
            } else {
                '\0'
            }
        })
        .filter(|c| *c != '\0')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transliterates_greek_examples() {
        let cases = [
            ("ἀγαπάω", "agapao"),
            ("λόγος", "logos"),
            ("χάρις", "charis"),
            ("ἁμαρτία", "hamartia"),
            ("υἱός", "huios"),
            ("ῥῆμα", "rhema"),
            ("ἄγγελος", "angelos"),
            ("ψυχή", "psuche"),
            ("ᾠδή", "ode"),
            ("ΘΕΟΣ", "theos"),
            ("πνεῦμα ἅγιον", "pneuma hagion"),
            ("", ""),
            ("abc", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(translit_greek(input), expected, "{input}");
        }
    }

    #[test]
    fn normalizes_module_transliterations() {
        assert_eq!(translit_norm("agapáō"), "agapao");
        assert_eq!(translit_norm("’ĕlôhîym"), "elohiym");
        assert_eq!(translit_norm("  Chá-ris  x "), "charis x");
        assert_eq!(translit_norm(""), "");
    }
}
