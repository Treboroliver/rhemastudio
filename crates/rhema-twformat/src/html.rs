use encoding_rs::{Encoding, WINDOWS_1252};
use rhema_core::Node;

const MAX_HTML_BYTES: usize = 1024 * 1024;
const MAX_TAG_BYTES: usize = 4096;
const MAX_DEPTH: usize = 128;

/// Converte HTML bruto em nós de texto e parágrafos, sem executar scripts nem baixar recursos.
pub fn convert_html(input: &[u8]) -> Vec<Node> {
    let bounded = &input[..input.len().min(MAX_HTML_BYTES)];
    let html = decode_html_bytes(bounded);
    parse_html(&html)
}

/// Converte texto simples (quando `content_type` é vazio ou `txt`/`text`) em nós.
pub(crate) fn convert_plain_bytes(input: &[u8]) -> Vec<Node> {
    let bounded = &input[..input.len().min(MAX_HTML_BYTES)];
    let text = match std::str::from_utf8(bounded) {
        Ok(s) => s.strip_prefix('\u{feff}').unwrap_or(s).to_string(),
        Err(_) => WINDOWS_1252.decode(bounded).0.into_owned(),
    };
    let mut nodes = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            nodes.push(Node::Paragraph);
        }
        let trimmed = line.trim_end_matches('\r');
        if !trimmed.is_empty() {
            nodes.push(Node::Text(trimmed.to_string()));
        }
    }
    while matches!(nodes.last(), Some(Node::Paragraph)) {
        nodes.pop();
    }
    nodes
}

fn decode_html_bytes(bytes: &[u8]) -> String {
    if let Some(enc) = detect_meta_charset(&bytes[..bytes.len().min(4096)]) {
        let (decoded, _, _) = enc.decode(bytes);
        return decoded.into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.strip_prefix('\u{feff}').unwrap_or(s).to_string(),
        Err(_) => WINDOWS_1252.decode(bytes).0.into_owned(),
    }
}

fn detect_meta_charset(head: &[u8]) -> Option<&'static Encoding> {
    let ascii: String = head
        .iter()
        .map(|&b| {
            if b.is_ascii() {
                (b as char).to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect();
    let mut pos = 0;
    while let Some(rel) = ascii[pos..].find("<meta") {
        let start = pos + rel;
        let end = ascii[start..].find('>').map_or(ascii.len(), |e| start + e);
        let tag = &ascii[start..end];
        if let Some(cs_pos) = tag.find("charset=") {
            let rest = tag[cs_pos + 8..].trim_start();
            let raw = if let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') {
                rest[1..].split(q).next().unwrap_or("")
            } else {
                rest.split(|c: char| c.is_whitespace() || c == ';' || c == '"' || c == '\'')
                    .next()
                    .unwrap_or("")
            };
            if let Some(enc) = Encoding::for_label(raw.trim().as_bytes()) {
                return Some(enc);
            }
        }
        pos = end.saturating_add(1);
    }
    None
}

fn parse_html(html: &str) -> Vec<Node> {
    let mut nodes: Vec<Node> = Vec::new();
    let mut buf = String::new();
    let mut pending_space = false;
    let mut depth = 0usize;
    let mut i = 0usize;
    let bytes = html.as_bytes();

    let flush_text = |nodes: &mut Vec<Node>, buf: &mut String| {
        if !buf.is_empty() {
            nodes.push(Node::Text(std::mem::take(buf)));
        }
    };
    let push_paragraph = |nodes: &mut Vec<Node>, buf: &mut String, pending_space: &mut bool| {
        *pending_space = false;
        let trimmed = buf.trim_end_matches(' ');
        if trimmed.len() != buf.len() {
            buf.truncate(trimmed.len());
        }
        flush_text(nodes, buf);
        if !nodes.is_empty() && !matches!(nodes.last(), Some(Node::Paragraph)) {
            nodes.push(Node::Paragraph);
        }
    };
    let push_char =
        |nodes: &mut Vec<Node>, buf: &mut String, pending_space: &mut bool, ch: char| {
            if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
                if !buf.is_empty() || matches!(nodes.last(), Some(Node::Text(_))) {
                    *pending_space = true;
                }
                return;
            }
            if *pending_space {
                buf.push(' ');
                *pending_space = false;
            }
            buf.push(ch);
        };

    while i < bytes.len() {
        if bytes[i] == b'<' {
            let rest = &html[i..];
            if let Some(comment_body) = rest.strip_prefix("<!--") {
                if let Some(end) = comment_body.find("-->") {
                    i += 4 + end + 3;
                } else {
                    break;
                }
                continue;
            }
            let next_byte = bytes.get(i + 1).copied();
            let looks_like_tag = matches!(next_byte, Some(b) if b.is_ascii_alphabetic() || b == b'/' || b == b'!' || b == b'?');
            if looks_like_tag {
                let mut search_end = (i + MAX_TAG_BYTES).min(html.len());
                while search_end < html.len() && !html.is_char_boundary(search_end) {
                    search_end += 1;
                }
                if let Some(rel_end) = html[i..search_end].find('>') {
                    let tag_inner = html[i + 1..i + rel_end].trim();
                    let closing = tag_inner.starts_with('/');
                    let name_part = tag_inner
                        .trim_start_matches('/')
                        .split(|c: char| c.is_whitespace() || c == '/')
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    i += rel_end + 1;

                    if !closing && (name_part == "script" || name_part == "style") {
                        let close_pat = if name_part == "script" {
                            "</script"
                        } else {
                            "</style"
                        };
                        let lower_rest = html[i..].to_ascii_lowercase();
                        if let Some(cpos) = lower_rest.find(close_pat) {
                            let after = i + cpos + close_pat.len();
                            if let Some(gt) = html[after..].find('>') {
                                i = after + gt + 1;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                        continue;
                    }

                    if !closing && !tag_inner.ends_with('/') {
                        depth = (depth + 1).min(MAX_DEPTH);
                    } else if closing {
                        depth = depth.saturating_sub(1);
                    }

                    if is_block_tag(&name_part) {
                        push_paragraph(&mut nodes, &mut buf, &mut pending_space);
                    }
                    continue;
                }
            }
            push_char(&mut nodes, &mut buf, &mut pending_space, '<');
            i += 1;
            continue;
        }

        if bytes[i] == b'&' {
            if let Some((ch, consumed)) = decode_entity(&html[i..]) {
                push_char(&mut nodes, &mut buf, &mut pending_space, ch);
                i += consumed;
                continue;
            }
        }

        if let Some(ch) = html[i..].chars().next() {
            push_char(&mut nodes, &mut buf, &mut pending_space, ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    flush_text(&mut nodes, &mut buf);
    while matches!(nodes.last(), Some(Node::Paragraph)) {
        nodes.pop();
    }
    nodes
}

fn is_block_tag(name: &str) -> bool {
    matches!(
        name,
        "p" | "br" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
    )
}

fn decode_entity(s: &str) -> Option<(char, usize)> {
    let semi = s[1..s.len().min(16)].find(';')?;
    let body = &s[1..1 + semi];
    let len = semi + 2;
    if let Some(hex) = body.strip_prefix("#x").or_else(|| body.strip_prefix("#X")) {
        let code = u32::from_str_radix(hex, 16).ok()?;
        return (code > 0)
            .then(|| char::from_u32(code))
            .flatten()
            .map(|c| (c, len));
    }
    if let Some(dec) = body.strip_prefix('#') {
        let code = dec.parse::<u32>().ok()?;
        return (code > 0)
            .then(|| char::from_u32(code))
            .flatten()
            .map(|c| (c, len));
    }
    let ch = match body {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "ndash" => '–',
        "mdash" => '—',
        "lsquo" => '‘',
        "rsquo" => '’',
        "ldquo" => '“',
        "rdquo" => '”',
        "hellip" => '…',
        "laquo" => '«',
        "raquo" => '»',
        "middot" => '·',
        "bull" => '•',
        "ordm" => 'º',
        "ordf" => 'ª',
        "deg" => '°',
        "copy" => '©',
        "reg" => '®',
        "aacute" => 'á',
        "Aacute" => 'Á',
        "agrave" => 'à',
        "Agrave" => 'À',
        "acirc" => 'â',
        "Acirc" => 'Â',
        "atilde" => 'ã',
        "Atilde" => 'Ã',
        "eacute" => 'é',
        "Eacute" => 'É',
        "ecirc" => 'ê',
        "Ecirc" => 'Ê',
        "iacute" => 'í',
        "Iacute" => 'Í',
        "oacute" => 'ó',
        "Oacute" => 'Ó',
        "ocirc" => 'ô',
        "Ocirc" => 'Ô',
        "otilde" => 'õ',
        "Otilde" => 'Õ',
        "uacute" => 'ú',
        "Uacute" => 'Ú',
        "uuml" => 'ü',
        "Uuml" => 'Ü',
        "ccedil" => 'ç',
        "Ccedil" => 'Ç',
        "ntilde" => 'ñ',
        "Ntilde" => 'Ñ',
        _ => return None,
    };
    Some((ch, len))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plain_text;

    #[test]
    fn converts_blocks_and_discards_script_style_comments() {
        let html = b"<!-- comentario --><h1>T&iacute;tulo</h1><style>body{color:red}</style>\
            <p>Primeiro <b>par&aacute;grafo</b> &amp; texto.</p>\
            <div>Linha 1<br/>Linha 2</div>\
            <script>alert('x')</script><ul><li>Item 1</li><li>Item 2</li></ul>";
        let nodes = convert_html(html);
        assert_eq!(
            nodes,
            vec![
                Node::Text("Título".into()),
                Node::Paragraph,
                Node::Text("Primeiro parágrafo & texto.".into()),
                Node::Paragraph,
                Node::Text("Linha 1".into()),
                Node::Paragraph,
                Node::Text("Linha 2".into()),
                Node::Paragraph,
                Node::Text("Item 1".into()),
                Node::Paragraph,
                Node::Text("Item 2".into()),
            ]
        );
    }

    #[test]
    fn decodes_named_and_numeric_entities() {
        let html = b"<p>Caf&eacute; &#233; &#x00E9; &nbsp; &lt;tag&gt; &desconhecida;</p>";
        let nodes = convert_html(html);
        assert_eq!(plain_text(&nodes), "Café é é <tag> &desconhecida;");
    }

    #[test]
    fn handles_utf8_windows1252_and_meta_charset() {
        let utf8 = "<p>Ação e graça</p>".as_bytes();
        assert_eq!(plain_text(&convert_html(utf8)), "Ação e graça");

        // Windows-1252 sem meta charset (inválido em UTF-8)
        let win1252 = b"<p>A\xe7\xe3o e gra\xe7a</p>";
        assert_eq!(plain_text(&convert_html(win1252)), "Ação e graça");

        // Meta charset explícito
        let meta =
            b"<html><head><meta charset=\"windows-1252\"></head><body><p>F\xe9</p></body></html>";
        assert_eq!(plain_text(&convert_html(meta)), "Fé");
    }

    #[test]
    fn handles_malformed_and_oversized_html_without_panic() {
        let malformed = b"<p>Inicio <b>sem fechar 5 < 10 e <script>sem fim";
        assert_eq!(
            plain_text(&convert_html(malformed)),
            "Inicio sem fechar 5 < 10 e"
        );

        let mut long_tag_utf8 = String::from("<a ");
        long_tag_utf8.push_str(&"é".repeat(MAX_TAG_BYTES));
        long_tag_utf8.push_str("fim");
        assert!(plain_text(&convert_html(long_tag_utf8.as_bytes())).ends_with("fim"));

        let mut huge = vec![b'a'; MAX_HTML_BYTES + 10_000];
        huge[..3].copy_from_slice(b"<p>");
        let nodes = convert_html(&huge);
        assert_eq!(plain_text(&nodes).len(), MAX_HTML_BYTES - 3);
    }
}
