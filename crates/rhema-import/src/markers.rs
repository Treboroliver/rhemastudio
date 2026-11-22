use rhema_core::{Book, StrongLang};
use rhema_refs::Language;

use crate::ir::{EntryKey, InlineLink, KeyType, Span, SpanStyle};

const KNOWN_TYPES: &[&str] = &[
    "Headword",
    "Bible",
    "HebrewStrongs",
    "GreekStrongs",
    "louw",
    "TWOT",
    "GreekGK",
    "HebrewGK",
];

/// Resultado do processamento de marcadores de um parágrafo.
#[derive(Debug, Default)]
pub struct ParagraphMarkers {
    pub spans: Vec<Span>,
    pub tags: Vec<(EntryKey, bool)>, // (chave, estava no início do parágrafo)
    pub links: Vec<InlineLink>,
    pub unbalanced_brackets: usize,
    pub unclosed_fields: usize,
    pub unmarked_bible_refs: usize,
}

/// Classifica o nome do tipo de marcador e extrai sufixo de idioma opcional (`Headword+el`).
pub fn parse_key_type(raw_type: &str) -> (KeyType, Option<String>) {
    let trimmed = raw_type.trim();
    let (base, lang) = match trimmed.split_once('+') {
        Some((b, l)) if !l.trim().is_empty() => (b.trim(), Some(l.trim().to_ascii_lowercase())),
        _ => (trimmed, None),
    };

    let key_type = match base.to_ascii_lowercase().as_str() {
        "headword" => KeyType::Headword,
        "bible" | "bíblia" | "biblia" => KeyType::Bible,
        "hebrewstrongs" => KeyType::HebrewStrongs,
        "greekstrongs" => KeyType::GreekStrongs,
        "louw" => KeyType::Louw,
        "twot" => KeyType::Twot,
        "greekgk" => KeyType::GreekGk,
        "hebrewgk" => KeyType::HebrewGk,
        _ => KeyType::Unknown(base.to_string()),
    };

    (key_type, lang)
}

/// Sugere o tipo conhecido mais próximo para um tipo desconhecido (distância <= 2).
pub fn suggest_known_type(unknown: &str) -> Option<&'static str> {
    let probe = unknown.trim().to_ascii_lowercase();
    if probe.is_empty() {
        return None;
    }
    let mut best: Option<(&'static str, usize)> = None;
    for &known in KNOWN_TYPES {
        let dist = levenshtein(&probe, &known.to_ascii_lowercase());
        if dist <= 2 && best.is_none_or(|(_, d)| dist < d) {
            best = Some((known, dist));
        }
    }
    best.map(|(name, _)| name)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// Normaliza uma chave Strong de etiqueta (`H1234`, `01234`, `G25`, `g0025`).
pub fn parse_strong_number(kind: &KeyType, value: &str) -> Option<(StrongLang, u32)> {
    let default_lang = match kind {
        KeyType::HebrewStrongs => Some(StrongLang::Hebrew),
        KeyType::GreekStrongs => Some(StrongLang::Greek),
        _ => None,
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let first = trimmed.chars().next()?;
    let (lang, rest) = match first.to_ascii_uppercase() {
        'H' => (StrongLang::Hebrew, &trimmed[1..]),
        'G' => (StrongLang::Greek, &trimmed[1..]),
        _ => (default_lang?, trimmed),
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let num = digits.parse::<u32>().ok()?;
    if num == 0 {
        return None;
    }
    Some((lang, num))
}

/// Tenta resolver uma referência bíblica com `rhema-refs` (pt-BR primeiro, en como fallback).
pub fn resolve_bible_ref(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.to_ascii_lowercase().starts_with("tw://bible.") {
        return trimmed.contains("id=");
    }
    let pt = rhema_refs::parse(trimmed, Language::PtBr);
    if !pt.faixas.is_empty() && pt.erros.is_empty() {
        return true;
    }
    let en = rhema_refs::parse(trimmed, Language::En);
    !en.faixas.is_empty() && en.erros.is_empty()
}

/// Processa os runs de um parágrafo, extraindo etiquetas `[[@...]]`, links `[[...]]`,
/// campos `{{field-on:...}}`/`{{field-off:...}}` e contando referências não marcadas.
pub fn parse_paragraph_runs(
    runs: &[(String, SpanStyle)],
    detect_unmarked_refs: bool,
) -> ParagraphMarkers {
    let mut out = ParagraphMarkers::default();
    if runs.is_empty() {
        return out;
    }

    let total_len: usize = runs.iter().map(|(t, _)| t.len()).sum();
    let mut full = String::with_capacity(total_len);
    let mut ranges: Vec<(usize, usize, SpanStyle)> = Vec::with_capacity(runs.len());
    for (text, style) in runs {
        if text.is_empty() {
            continue;
        }
        let start = full.len();
        full.push_str(text);
        ranges.push((start, full.len(), *style));
    }

    let bytes = full.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut text_start = 0usize;
    let mut seen_visible_text = false;
    let mut open_fields: Vec<String> = Vec::new();

    let flush_text = |from: usize,
                      to: usize,
                      open_fields: &[String],
                      seen_visible: &mut bool,
                      out: &mut ParagraphMarkers| {
        if from >= to {
            return;
        }
        let slice = &full[from..to];
        if !slice.trim().is_empty() {
            *seen_visible = true;
            if detect_unmarked_refs {
                out.unmarked_bible_refs += count_unmarked_bible_refs(slice);
            }
        }
        let field = open_fields.last().cloned();
        emit_styled_text(slice, from, &ranges, field, &mut out.spans);
    };

    while i < len {
        // Verifica `[[`
        if bytes[i] == b'[' && i + 1 < len && bytes[i + 1] == b'[' {
            if let Some(close_rel) = find_double_bracket_close(&full[i + 2..]) {
                let inner = &full[i + 2..i + 2 + close_rel];
                if let Some(parsed) = parse_bracket_inner(inner) {
                    flush_text(
                        text_start,
                        i,
                        &open_fields,
                        &mut seen_visible_text,
                        &mut out,
                    );
                    match parsed {
                        BracketToken::Tag(key) => {
                            out.tags.push((key, !seen_visible_text));
                        }
                        BracketToken::Link(link) => {
                            seen_visible_text = true;
                            let style = style_at(i, &ranges);
                            let field = open_fields.last().cloned();
                            out.links.push(link.clone());
                            out.spans.push(Span::Link { link, style, field });
                        }
                    }
                    i = i + 2 + close_rel + 2;
                    text_start = i;
                    continue;
                }
            }
            // `[[` sem `]]` válido (ou com outro `[[` antes)
            out.unbalanced_brackets += 1;
            i += 2;
            continue;
        }

        // Verifica `]]` solto
        if bytes[i] == b']' && i + 1 < len && bytes[i + 1] == b']' {
            out.unbalanced_brackets += 1;
            i += 2;
            continue;
        }

        // Verifica `{{field-on:...}}` ou `{{field-off:...}}`
        if bytes[i] == b'{' && i + 1 < len && bytes[i + 1] == b'{' {
            if let Some(close_rel) = full[i + 2..].find("}}") {
                let inner = full[i + 2..i + 2 + close_rel].trim();
                if let Some((is_on, name)) = parse_field_directive(inner) {
                    flush_text(
                        text_start,
                        i,
                        &open_fields,
                        &mut seen_visible_text,
                        &mut out,
                    );
                    if is_on {
                        open_fields.push(name);
                    } else if let Some(pos) = open_fields
                        .iter()
                        .rposition(|f| f.eq_ignore_ascii_case(&name))
                    {
                        open_fields.remove(pos);
                    } else {
                        open_fields.pop();
                    }
                    i = i + 2 + close_rel + 2;
                    text_start = i;
                    continue;
                }
            }
            i += 2;
            continue;
        }

        i += 1;
    }

    flush_text(
        text_start,
        len,
        &open_fields,
        &mut seen_visible_text,
        &mut out,
    );
    out.unclosed_fields = open_fields.len();
    out
}

enum BracketToken {
    Tag(EntryKey),
    Link(InlineLink),
}

fn find_double_bracket_close(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    let mut j = 0;
    while j + 1 < bytes.len() {
        if bytes[j] == b'[' && bytes[j + 1] == b'[' {
            // Outro `[[` aberto antes de fechar -> desbalanceado
            return None;
        }
        if bytes[j] == b']' && bytes[j + 1] == b']' {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn parse_bracket_inner(inner: &str) -> Option<BracketToken> {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(after_at) = trimmed.strip_prefix('@') {
        let (raw_type, raw_val) = after_at.split_once(':')?;
        let (kind, lang) = parse_key_type(raw_type);
        let value = raw_val.trim().to_string();
        return Some(BracketToken::Tag(EntryKey { kind, value, lang }));
    }

    // Link: opcionalmente `label >> TIPO: VALOR` ou `TIPO: VALOR`
    let (label, target_part) = match trimmed.split_once(">>") {
        Some((lbl, tgt)) => (Some(lbl.trim().to_string()), tgt.trim()),
        None => (None, trimmed),
    };
    let (raw_type, raw_val) = target_part.split_once(':')?;
    let (kind, lang) = parse_key_type(raw_type);
    let value = raw_val.trim().to_string();
    Some(BracketToken::Link(InlineLink {
        kind,
        value,
        lang,
        label,
    }))
}

fn parse_field_directive(inner: &str) -> Option<(bool, String)> {
    let (cmd, name) = inner.split_once(':')?;
    let cmd = cmd.trim().to_ascii_lowercase();
    let name = name.trim().to_string();
    if name.is_empty() {
        return None;
    }
    if cmd == "field-on" {
        Some((true, name))
    } else if cmd == "field-off" {
        Some((false, name))
    } else {
        None
    }
}

fn style_at(byte_offset: usize, ranges: &[(usize, usize, SpanStyle)]) -> SpanStyle {
    for &(start, end, style) in ranges {
        if byte_offset >= start && byte_offset < end {
            return style;
        }
    }
    SpanStyle::default()
}

fn emit_styled_text(
    slice: &str,
    slice_start: usize,
    ranges: &[(usize, usize, SpanStyle)],
    field: Option<String>,
    out: &mut Vec<Span>,
) {
    let slice_end = slice_start + slice.len();
    for &(r_start, r_end, style) in ranges {
        if r_end <= slice_start {
            continue;
        }
        if r_start >= slice_end {
            break;
        }
        let s = r_start.max(slice_start) - slice_start;
        let e = r_end.min(slice_end) - slice_start;
        if s < e {
            let chunk = &slice[s..e];
            if let Some(Span::Text {
                text,
                style: prev_style,
                field: prev_field,
            }) = out.last_mut()
            {
                if *prev_style == style && *prev_field == field {
                    text.push_str(chunk);
                    continue;
                }
            }
            out.push(Span::Text {
                text: chunk.to_string(),
                style,
                field: field.clone(),
            });
        }
    }
}

/// Detecta e conta referências bíblicas não marcadas no corpo do texto (ex.: `(Gn 31.28)`),
/// exigindo abreviação de livro + capítulo + separador `.` ou `:` + versículo.
pub fn count_unmarked_bible_refs(text: &str) -> usize {
    let mut count = 0usize;
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;

    while i < len {
        // Procura separador `.` ou `:` cercado de dígitos (capítulo.versículo ou capítulo:versículo)
        if (bytes[i] == b'.' || bytes[i] == b':')
            && i > 0
            && i + 1 < len
            && bytes[i - 1].is_ascii_digit()
            && bytes[i + 1].is_ascii_digit()
        {
            // Recua dígitos do capítulo
            let mut cap_start = i - 1;
            while cap_start > 0 && bytes[cap_start - 1].is_ascii_digit() {
                cap_start -= 1;
            }
            // Avança dígitos do versículo (e faixa opcional `-12`)
            let mut end = i + 1;
            while end < len && bytes[end].is_ascii_digit() {
                end += 1;
            }
            // Precisa de espaço antes do capítulo
            if cap_start >= 2 && bytes[cap_start - 1] == b' ' {
                let mut bk_end = cap_start - 1;
                while bk_end > 0 && bytes[bk_end - 1] == b' ' {
                    bk_end -= 1;
                }
                // Recua palavra do livro (letras + possível prefixo numérico `1`/`2`/`3` colado ou separado)
                let mut bk_start = bk_end;
                while bk_start > 0 {
                    let prev_char = text[..bk_start].chars().next_back();
                    match prev_char {
                        Some(c) if c.is_alphabetic() => bk_start -= c.len_utf8(),
                        _ => break,
                    }
                }
                // Prefixo numérico colado (1Co) ou separado por espaço (1 Co)
                if bk_start > 0 && matches!(bytes[bk_start - 1], b'1' | b'2' | b'3') {
                    let prev_is_boundary = bk_start == 1
                        || !text[..bk_start - 1]
                            .chars()
                            .next_back()
                            .is_some_and(char::is_alphanumeric);
                    if prev_is_boundary {
                        bk_start -= 1;
                    }
                } else if bk_start >= 2
                    && bytes[bk_start - 1] == b' '
                    && matches!(bytes[bk_start - 2], b'1' | b'2' | b'3')
                {
                    let prev_is_boundary = bk_start == 2
                        || !text[..bk_start - 2]
                            .chars()
                            .next_back()
                            .is_some_and(char::is_alphanumeric);
                    if prev_is_boundary {
                        bk_start -= 2;
                    }
                }

                if bk_start < bk_end {
                    let book_token = text[bk_start..bk_end].trim();
                    if looks_like_book_token(book_token) {
                        let candidate = &text[bk_start..end];
                        let parsed = rhema_refs::parse(candidate, Language::PtBr);
                        if !parsed.faixas.is_empty() && parsed.erros.is_empty() {
                            count += 1;
                            i = end;
                            continue;
                        }
                    }
                }
            }
            i = end;
            continue;
        }
        i += 1;
    }

    count
}

fn looks_like_book_token(token: &str) -> bool {
    let char_count = token.chars().count();
    if !(2..=20).contains(&char_count) {
        return false;
    }
    // Exige primeira letra maiúscula (ou dígito 1/2/3 seguido de maiúscula) para evitar falsos positivos em prosa
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let valid_start = if matches!(first, '1' | '2' | '3') {
        chars
            .find(|c| !c.is_whitespace())
            .is_some_and(char::is_uppercase)
    } else {
        first.is_uppercase()
    };
    if !valid_start {
        return false;
    }
    // Checagem rápida contra abreviações primárias ou nomes em pt-BR
    Book::ALL.iter().any(|b| {
        b.primary_abbreviation_pt().eq_ignore_ascii_case(token)
            || b.name_pt().eq_ignore_ascii_case(token)
            || b.primary_abbreviation_en().eq_ignore_ascii_case(token)
    }) || token.len() <= 5
}
