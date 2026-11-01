//! Conversor tolerante de conteúdo RVF (`content_type = rvf`) para [`Node`].
//!
//! A especificação observada está em `docs/02-apendice-schemas.md`. O RVF é um fluxo
//! misto: linhas de texto (`-8 …`, `-9 …`, cabeçalhos de item), tabelas de estilo em
//! formato binário do Delphi e texto dos itens em UTF-16LE.

use std::collections::BTreeMap;

use encoding_rs::{UTF_16LE, UTF_8, WINDOWS_1252};
use rhema_core::{LinkTarget, Node};

const MAX_STYLES: usize = 4096;
const MAX_DEPTH: usize = 8;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RvfStats {
    pub items: usize,
    pub text_styles: usize,
    pub utf16_items: usize,
    pub ansi_items: usize,
    /// Links por tipo (`bíblia`, `interno`, `livro`, `verbete`, `arquivo`, `outro`).
    pub links: BTreeMap<String, usize>,
    /// Construções não reconhecidas, por tipo.
    pub unknown: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RvfConversion {
    pub nodes: Vec<Node>,
    /// A estrutura não foi reconhecida por completo; o texto é o melhor esforço.
    pub degraded: bool,
    pub stats: RvfStats,
}

#[derive(Debug, Clone, Copy, Default)]
struct TextStyle {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
}

struct ItemHeader {
    style: i64,
    paragraph: i64,
    tag: Option<Vec<u8>>,
    end: usize,
}

struct Item<'a> {
    header: ItemHeader,
    text: &'a [u8],
}

/// Converte RVF. Nunca entra em pânico; estruturas desconhecidas preservam o texto.
pub fn convert_rvf(input: &[u8]) -> RvfConversion {
    let mut stats = RvfStats::default();
    if !input.starts_with(b"-8 ") {
        *stats
            .unknown
            .entry("sem-cabecalho".to_string())
            .or_default() += 1;
        return degraded_text(input, stats);
    }

    let markers = find_markers(input);
    let styles = markers
        .iter()
        .enumerate()
        .find(|(_, marker)| marker.values[5] == 1)
        .map(|(index, marker)| {
            let stop = markers
                .get(index + 1)
                .map_or(input.len(), |next| next.start);
            let region = &input[marker.end.min(stop)..stop];
            parse_text_styles(skip_line(region))
        })
        .unwrap_or_default();
    stats.text_styles = styles.len();

    let body_start = markers.last().map_or_else(
        || skip_line(input).as_ptr() as usize - input.as_ptr() as usize,
        |m| m.end,
    );
    let items = scan_items(input, body_start);
    stats.items = items.len();
    if items.is_empty() {
        *stats.unknown.entry("sem-itens".to_string()).or_default() += 1;
        return degraded_text(input, stats);
    }

    let unicode_majority =
        items.iter().filter(|item| has_odd_zero(item.text)).count() * 2 >= items.len();

    let mut nodes: Vec<Node> = Vec::new();
    let mut open_link: Option<(Vec<u8>, usize)> = None;
    for item in &items {
        let header = &item.header;
        if header.style < 0 {
            *stats
                .unknown
                .entry(format!("estilo-{}", header.style))
                .or_default() += 1;
            continue;
        }
        let style = usize::try_from(header.style)
            .ok()
            .and_then(|index| styles.get(index))
            .copied()
            .unwrap_or_else(|| {
                *stats
                    .unknown
                    .entry("estilo-fora-da-tabela".to_string())
                    .or_default() += 1;
                TextStyle::default()
            });
        let text = decode_item_text(item.text, unicode_majority, &mut stats);
        if header.paragraph >= 0
            && !nodes.is_empty()
            && !matches!(nodes.last(), Some(Node::Paragraph))
        {
            nodes.push(Node::Paragraph);
            open_link = None;
        }
        if text.is_empty() {
            continue;
        }
        let node = styled(&text, style);
        match &header.tag {
            Some(tag) => {
                if let Some((open_tag, index)) = &open_link {
                    if open_tag == tag {
                        if let Some(Node::Link { children, .. }) = nodes.get_mut(*index) {
                            children.push(node);
                            continue;
                        }
                    }
                }
                let (kind, target) = classify_link(tag);
                *stats.links.entry(kind.to_string()).or_default() += 1;
                nodes.push(Node::Link {
                    target,
                    children: vec![node],
                });
                open_link = Some((tag.clone(), nodes.len() - 1));
            }
            None => {
                open_link = None;
                nodes.push(node);
            }
        }
    }
    while matches!(nodes.last(), Some(Node::Paragraph)) {
        nodes.pop();
    }

    let degraded = styles.is_empty() || nodes.is_empty();
    RvfConversion {
        nodes,
        degraded,
        stats,
    }
}

fn styled(text: &str, style: TextStyle) -> Node {
    let mut node = Node::Text(text.to_string());
    if style.strike {
        node = Node::Strikethrough(vec![node]);
    }
    if style.underline {
        node = Node::Underline(vec![node]);
    }
    if style.italic {
        node = Node::Italic(vec![node]);
    }
    if style.bold {
        node = Node::Bold(vec![node]);
    }
    node
}

fn classify_link(tag: &[u8]) -> (&'static str, LinkTarget) {
    let text = match std::str::from_utf8(tag) {
        Ok(text) => text.to_string(),
        Err(_) => WINDOWS_1252.decode(tag).0.into_owned(),
    };
    if text.starts_with("tw://bible") {
        let target = crate::rtf::parse_link_target(&text);
        let kind = if matches!(target, LinkTarget::Bible(_)) {
            "bíblia"
        } else {
            "outro"
        };
        return (kind, target);
    }
    let kind = if text.starts_with("tw://[self]") {
        "interno"
    } else if text.starts_with("tw://bk.") {
        "livro"
    } else if text.starts_with("tw://dct.") {
        "verbete"
    } else if text.starts_with("file:") {
        "arquivo"
    } else {
        "outro"
    };
    (kind, LinkTarget::Other(text))
}

fn decode_item_text(bytes: &[u8], unicode_majority: bool, stats: &mut RvfStats) -> String {
    let utf16 = bytes.len() >= 2
        && bytes.len().is_multiple_of(2)
        && (has_odd_zero(bytes) || (unicode_majority && !bytes.contains(&0) && !is_ascii(bytes)));
    let text = if utf16 {
        stats.utf16_items += 1;
        UTF_16LE.decode_without_bom_handling(bytes).0.into_owned()
    } else {
        stats.ansi_items += 1;
        match UTF_8.decode_without_bom_handling_and_without_replacement(bytes) {
            Some(text) => text.into_owned(),
            None => WINDOWS_1252
                .decode_without_bom_handling(bytes)
                .0
                .into_owned(),
        }
    };
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'))
        .collect::<String>()
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

fn is_ascii(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii)
}

fn has_odd_zero(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes.iter().skip(1).step_by(2).any(|byte| *byte == 0)
}

struct Marker {
    start: usize,
    end: usize,
    values: [i64; 6],
}

/// Linhas `-9 a b c d e f` que separam as tabelas de estilo e o corpo.
fn find_markers(input: &[u8]) -> Vec<Marker> {
    let mut markers = Vec::new();
    let mut position = 0;
    while let Some(offset) = memchr::memmem::find(&input[position..], b"-9 ") {
        let start = position + offset;
        position = start + 3;
        let mut cursor = start + 3;
        let mut values = [0i64; 6];
        let mut ok = true;
        for (index, slot) in values.iter_mut().enumerate() {
            match parse_int(input, cursor) {
                Some((value, next)) => {
                    *slot = value;
                    cursor = next;
                    if index < 5 {
                        if input.get(cursor) == Some(&b' ') {
                            cursor += 1;
                        } else {
                            ok = false;
                            break;
                        }
                    }
                }
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if ok && input[cursor..].starts_with(b"\r\n") {
            markers.push(Marker {
                start,
                end: cursor + 2,
                values,
            });
            position = cursor + 2;
        }
    }
    markers
}

fn skip_line(input: &[u8]) -> &[u8] {
    match memchr::memmem::find(input, b"\r\n") {
        Some(index) => &input[index + 2..],
        None => &input[input.len()..],
    }
}

fn parse_int(input: &[u8], start: usize) -> Option<(i64, usize)> {
    let mut cursor = start;
    if input.get(cursor) == Some(&b'-') {
        cursor += 1;
    }
    let digits = cursor;
    while input.get(cursor).is_some_and(u8::is_ascii_digit) && cursor - digits < 10 {
        cursor += 1;
    }
    if cursor == digits {
        return None;
    }
    let value = std::str::from_utf8(&input[start..cursor])
        .ok()?
        .parse()
        .ok()?;
    Some((value, cursor))
}

/// Cabeçalho de item: `estilo linhas parágrafo opções extra tag` terminado em CRLF; a tag é
/// um inteiro (sem tag) ou texto entre aspas.
fn parse_header(input: &[u8], start: usize) -> Option<ItemHeader> {
    let mut cursor = start;
    let mut values = [0i64; 5];
    for slot in &mut values {
        let (value, next) = parse_int(input, cursor)?;
        *slot = value;
        cursor = next;
        if input.get(cursor) != Some(&b' ') {
            return None;
        }
        cursor += 1;
    }
    let tag = if input.get(cursor) == Some(&b'"') {
        let relative = memchr::memmem::find(&input[cursor + 1..], b"\"\r\n")?;
        let tag = input[cursor + 1..cursor + 1 + relative].to_vec();
        cursor += relative + 4;
        Some(tag)
    } else {
        let (_, next) = parse_int(input, cursor)?;
        if !input[next..].starts_with(b"\r\n") {
            return None;
        }
        cursor = next + 2;
        None
    };
    Some(ItemHeader {
        style: values[0],
        paragraph: values[2],
        tag,
        end: cursor,
    })
}

fn header_allowed_at(input: &[u8], start: usize, body_start: usize) -> bool {
    start == body_start
        || input[start - 1] == 0
        || input[start - 1] == b'\n'
        || (start >= 2 && input[start - 1] == b' ' && input[start - 2] == b')')
}

fn scan_items(input: &[u8], body_start: usize) -> Vec<Item<'_>> {
    let mut headers: Vec<(usize, ItemHeader)> = Vec::new();
    let mut position = body_start.min(input.len());
    while position < input.len() {
        let byte = input[position];
        if (byte.is_ascii_digit() || byte == b'-') && header_allowed_at(input, position, body_start)
        {
            if let Some(header) = parse_header(input, position) {
                let next = header.end;
                headers.push((position, header));
                position = next;
                continue;
            }
        }
        position += 1;
    }
    let starts = headers.iter().map(|(start, _)| *start).collect::<Vec<_>>();
    headers
        .into_iter()
        .enumerate()
        .map(|(index, (_, header))| {
            let stop = starts.get(index + 1).copied().unwrap_or(input.len());
            let text = trim_terminator(&input[header.end.min(stop)..stop]);
            Item { header, text }
        })
        .collect()
}

/// Remove o terminador `) ` (um por item; ocorre repetido em itens vazios) e o `)` final.
fn trim_terminator(mut text: &[u8]) -> &[u8] {
    loop {
        if text.ends_with(b") ") && (text.len() - 2).is_multiple_of(2) {
            text = &text[..text.len() - 2];
        } else if text.len() % 2 == 1 && (text.ends_with(b")") || text.ends_with(b" ")) {
            text = &text[..text.len() - 1];
        } else {
            return text;
        }
    }
}

enum Value {
    Set(Vec<String>),
    Other,
}

fn take<'a>(blob: &'a [u8], position: &mut usize, count: usize) -> Option<&'a [u8]> {
    let end = position.checked_add(count)?;
    let slice = blob.get(*position..end)?;
    *position = end;
    Some(slice)
}

fn take_u32_len(blob: &[u8], position: &mut usize) -> Option<usize> {
    let bytes = take(blob, position, 4)?;
    usize::try_from(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])).ok()
}

fn read_value(blob: &[u8], position: &mut usize, depth: usize) -> Option<Value> {
    if depth > MAX_DEPTH {
        return None;
    }
    let kind = *take(blob, position, 1)?.first()?;
    match kind {
        0 | 8 | 9 | 0x0d => {}
        2 => {
            take(blob, position, 1)?;
        }
        3 => {
            take(blob, position, 2)?;
        }
        4 | 0x0f => {
            take(blob, position, 4)?;
        }
        5 => {
            take(blob, position, 10)?;
        }
        0x10 | 0x11 | 0x13 => {
            take(blob, position, 8)?;
        }
        6 | 7 => {
            let length = usize::from(*take(blob, position, 1)?.first()?);
            take(blob, position, length)?;
        }
        0x0a | 0x0c | 0x14 => {
            let length = take_u32_len(blob, position)?;
            take(blob, position, length)?;
        }
        0x12 => {
            let length = take_u32_len(blob, position)?;
            take(blob, position, length.checked_mul(2)?)?;
        }
        0x0b => {
            let mut names = Vec::new();
            loop {
                let length = usize::from(*take(blob, position, 1)?.first()?);
                if length == 0 {
                    break;
                }
                let name = take(blob, position, length)?;
                names.push(String::from_utf8_lossy(name).into_owned());
            }
            return Some(Value::Set(names));
        }
        1 => {
            while *blob.get(*position)? != 0 {
                read_value(blob, position, depth + 1)?;
            }
            *position += 1;
        }
        0x0e => {
            while *blob.get(*position)? != 0 {
                if blob[*position] == 1 {
                    *position += 1;
                }
                read_properties(blob, position, depth + 1, &mut |_, _| {})?;
            }
            *position += 1;
        }
        _ => return None,
    }
    Some(Value::Other)
}

fn read_properties(
    blob: &[u8],
    position: &mut usize,
    depth: usize,
    visit: &mut dyn FnMut(&str, &Value),
) -> Option<()> {
    loop {
        let length = usize::from(*take(blob, position, 1)?.first()?);
        if length == 0 {
            return Some(());
        }
        let name = String::from_utf8_lossy(take(blob, position, length)?).into_owned();
        let value = read_value(blob, position, depth)?;
        visit(&name, &value);
    }
}

/// Tabela de estilos de texto: fluxo binário do Delphi com uma coleção de itens.
fn parse_text_styles(blob: &[u8]) -> Vec<TextStyle> {
    let mut styles = Vec::new();
    let start = if blob.get(4) == Some(&0x0e) {
        Some(4)
    } else {
        blob.iter().take(16).position(|byte| *byte == 0x0e)
    };
    let Some(start) = start else {
        return styles;
    };
    let mut position = start + 1;
    while styles.len() < MAX_STYLES && blob.get(position) == Some(&1) {
        position += 1;
        let mut style = TextStyle::default();
        let result = read_properties(blob, &mut position, 0, &mut |name, value| {
            if let (true, Value::Set(names)) = (name == "Style", value) {
                for flag in names {
                    match flag.as_str() {
                        "fsBold" => style.bold = true,
                        "fsItalic" => style.italic = true,
                        "fsUnderline" => style.underline = true,
                        "fsStrikeOut" => style.strike = true,
                        _ => {}
                    }
                }
            }
        });
        if result.is_none() {
            break;
        }
        styles.push(style);
    }
    styles
}

/// Melhor esforço quando a estrutura não é reconhecida: texto legível, sem formatação.
fn degraded_text(input: &[u8], stats: RvfStats) -> RvfConversion {
    let decoded = if has_odd_zero(input) {
        UTF_16LE.decode_without_bom_handling(input).0.into_owned()
    } else {
        WINDOWS_1252
            .decode_without_bom_handling(input)
            .0
            .into_owned()
    };
    let mut text = String::new();
    for c in decoded.chars() {
        if c.is_control() && !matches!(c, '\n') {
            if !text.ends_with(' ') {
                text.push(' ');
            }
        } else {
            text.push(c);
        }
    }
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    RvfConversion {
        nodes: if text.is_empty() {
            Vec::new()
        } else {
            vec![Node::Text(text)]
        },
        degraded: true,
        stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    /// Tabela de estilos sintética: 0 = normal, 1 = negrito, 2 = itálico + sublinhado.
    fn style_blob() -> Vec<u8> {
        let mut blob = vec![0x20, 0, 0, 0, 0x0e];
        for flags in [&[][..], &["fsBold"][..], &["fsItalic", "fsUnderline"][..]] {
            blob.push(1);
            blob.extend([9]);
            blob.extend(b"StyleName");
            blob.extend([6, 1, b'S']);
            blob.push(5);
            blob.extend(b"Style");
            blob.push(0x0b);
            for flag in flags {
                blob.push(flag.len() as u8);
                blob.extend(flag.bytes());
            }
            blob.push(0);
            blob.push(4);
            blob.extend(b"Size");
            blob.extend([2, 12]);
            blob.push(0);
        }
        blob.push(0);
        blob
    }

    fn document(items: &[(&str, &[u8])]) -> Vec<u8> {
        let mut data = b"-8 1 3\r\n-9 2 0 0 2 0 1\r\nStyles\r\n".to_vec();
        data.extend(style_blob());
        data.extend(b"-9 0 0 0 0 0 7\r\n");
        for (index, (header, text)) in items.iter().enumerate() {
            if index > 0 {
                data.extend(b") ");
            }
            data.extend(header.as_bytes());
            data.extend(b"\r\n");
            data.extend(*text);
        }
        data.extend(b")");
        data
    }

    #[test]
    fn converts_styles_paragraphs_and_links() {
        let one = utf16("um ");
        let two = utf16("dois");
        let three = utf16("tres");
        let data = document(&[
            ("0 1 1 8 0 0", &one),
            ("1 1 -1 9 0 0", &two),
            ("2 1 2 8 0 \"tw://bible.*?id=43.3.16|_X_|\"", &three),
        ]);
        let conversion = convert_rvf(&data);
        assert!(!conversion.degraded);
        assert_eq!(conversion.stats.items, 3);
        assert_eq!(conversion.stats.text_styles, 3);
        assert_eq!(conversion.stats.links.get("bíblia"), Some(&1));
        let plain = conversion
            .nodes
            .iter()
            .filter(|node| !matches!(node, Node::Paragraph))
            .map(Node::to_plain_text)
            .collect::<String>();
        assert_eq!(plain, "um doistres");
        assert!(matches!(conversion.nodes[1], Node::Bold(_)));
        assert!(conversion.nodes.contains(&Node::Paragraph));
        assert!(matches!(conversion.nodes.last(), Some(Node::Link { .. })));
    }

    #[test]
    fn adjacent_items_with_the_same_tag_share_one_link() {
        let a = utf16("a");
        let b = utf16("b");
        let data = document(&[
            ("0 1 1 8 0 \"tw://[self]?tid=5\"", &a),
            ("0 1 -1 9 0 \"tw://[self]?tid=5\"", &b),
        ]);
        let conversion = convert_rvf(&data);
        assert_eq!(conversion.nodes.len(), 1);
        assert_eq!(conversion.stats.links.get("interno"), Some(&1));
    }

    #[test]
    fn unknown_styles_keep_text_and_are_counted() {
        let text = utf16("texto");
        let negative = utf16("x");
        let data = document(&[("9 1 1 8 0 0", &text), ("-2 1 -1 9 0 0", &negative)]);
        let conversion = convert_rvf(&data);
        assert_eq!(
            conversion.stats.unknown.get("estilo-fora-da-tabela"),
            Some(&1)
        );
        assert_eq!(conversion.stats.unknown.get("estilo--2"), Some(&1));
        assert_eq!(conversion.nodes[0].to_plain_text(), "texto");
    }

    #[test]
    fn ansi_items_fall_back_to_windows_1252() {
        let data = document(&[("0 1 1 8 0 0", &[0x6e, 0xe3, 0x6f, 0x21][..])]);
        let conversion = convert_rvf(&data);
        assert_eq!(conversion.nodes[0].to_plain_text(), "não!");
        assert_eq!(conversion.stats.ansi_items, 1);
    }

    #[test]
    fn unrecognised_structure_is_degraded_not_an_error() {
        for input in [
            &b"plain text"[..],
            b"",
            b"-8 1 3\r\nabc",
            &[0xff, 0xfe, 0, 0][..],
        ] {
            let conversion = convert_rvf(input);
            assert!(conversion.degraded);
        }
        let conversion = convert_rvf(b"-8 1 3\r\n-9 2 0 0 2 0 1\r\nx\r\nabc");
        assert!(conversion.degraded);
        assert_eq!(
            conversion.nodes[0].to_plain_text(),
            "-8 1 3 -9 2 0 0 2 0 1 x abc"
        );
    }

    #[test]
    fn missing_style_table_is_degraded_but_keeps_text() {
        let mut data = b"-8 1 3\r\n-9 0 0 0 0 0 7\r\n".to_vec();
        data.extend(b"0 1 1 8 0 0\r\n");
        data.extend(utf16("ok"));
        let conversion = convert_rvf(&data);
        assert!(conversion.degraded);
        assert_eq!(conversion.nodes[0].to_plain_text(), "ok");
    }

    #[test]
    fn truncated_style_blob_does_not_panic() {
        let blob = style_blob();
        for cut in 0..blob.len() {
            let _ = parse_text_styles(&blob[..cut]);
        }
    }

    proptest::proptest! {
        #[test]
        fn arbitrary_bytes_never_panic(input in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..2048)) {
            let _ = convert_rvf(&input);
            let mut prefixed = b"-8 1 3\r\n-9 2 0 0 2 0 1\r\n".to_vec();
            prefixed.extend(&input);
            let _ = convert_rvf(&prefixed);
        }
    }
}
