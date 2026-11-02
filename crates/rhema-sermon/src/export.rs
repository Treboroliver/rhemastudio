use crate::model::{Block, Sermon};

/// Markdown portátil do sermão, com front-matter YAML.
pub fn export_markdown(sermon: &Sermon) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("title: {}\n", yaml_string(&sermon.title)));
    out.push_str(&format!("base_text: {}\n", yaml_string(&sermon.base_text)));
    out.push_str(&format!("created: {}\n", format_iso(sermon.created_at)));
    out.push_str(&format!("updated: {}\n", format_iso(sermon.updated_at)));
    out.push_str("---\n");
    for block in &sermon.blocks {
        out.push('\n');
        out.push_str(&render_block(block));
        out.push('\n');
    }
    out
}

fn render_block(block: &Block) -> String {
    match block {
        Block::Heading { level, text, .. } => {
            let level = (*level).clamp(1, 3) as usize;
            format!("{} {}", "#".repeat(level), single_line(text))
        }
        Block::Text { markdown, .. } => markdown.clone(),
        Block::Verse {
            range,
            module_abbr,
            snapshot,
            ..
        } => format!("{}\n>\n> — {range} ({module_abbr})", quote(snapshot, "")),
        Block::Lexicon {
            strong_id,
            title,
            snapshot,
            ..
        } => format!("**{strong_id} — {}**\n\n{snapshot}", single_line(title)),
        Block::Note { markdown, .. } => quote(markdown, "Nota: "),
        Block::Illustration { markdown, .. } => format!("*Ilustração:* {markdown}"),
    }
}

fn quote(text: &str, first_prefix: &str) -> String {
    let mut lines = text.lines();
    let mut out = format!("> {first_prefix}{}", lines.next().unwrap_or(""));
    for line in lines {
        out.push_str("\n> ");
        out.push_str(line);
    }
    out.split('\n')
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// String YAML entre aspas duplas (a sintaxe de string JSON é YAML válido).
fn yaml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

/// `AAAA-MM-DDTHH:MM:SSZ` a partir de milissegundos desde 1970.
pub(crate) fn format_iso(millis: i64) -> String {
    let seconds = millis.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use rhema_core::{Book, VerseRange, VerseRef};

    use super::*;

    #[test]
    fn formats_dates() {
        assert_eq!(format_iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(
            format_iso(1_709_164_800_000 + 3_723_000),
            "2024-02-29T01:02:03Z"
        );
        assert_eq!(format_iso(-1000), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn golden_export() {
        let sermon = Sermon {
            id: "00000000-0000-4000-8000-000000000000".into(),
            title: "Esperança \"viva\" 🌱".into(),
            base_text: "Jo 3:16-17".into(),
            created_at: 1_709_164_800_000,
            updated_at: 1_709_164_800_000 + 3_723_000,
            archived: false,
            blocks: vec![
                Block::Heading {
                    id: "1".into(),
                    level: 1,
                    text: "Introdução".into(),
                },
                Block::Text {
                    id: "2".into(),
                    markdown: "Texto com **negrito**.\n\nSegundo parágrafo.".into(),
                },
                Block::Heading {
                    id: "3".into(),
                    level: 3,
                    text: "Ponto\num".into(),
                },
                Block::Verse {
                    id: "4".into(),
                    range: VerseRange::single(VerseRef::new(Book::John, 3, 16)),
                    module_id: "m".into(),
                    module_abbr: "XYZ".into(),
                    snapshot: "Linha um inventada\nlinha dois inventada".into(),
                },
                Block::Lexicon {
                    id: "5".into(),
                    strong_id: "G25".into(),
                    module_id: "d".into(),
                    title: "amar".into(),
                    snapshot: "Definição inventada.".into(),
                },
                Block::Note {
                    id: "6".into(),
                    markdown: "primeira\nsegunda".into(),
                },
                Block::Illustration {
                    id: "7".into(),
                    markdown: "Uma história inventada.".into(),
                },
                Block::Heading {
                    id: "8".into(),
                    level: 2,
                    text: "Fim".into(),
                },
            ],
        };
        let expected = "\
---
title: \"Esperança \\\"viva\\\" 🌱\"
base_text: \"Jo 3:16-17\"
created: 2024-02-29T00:00:00Z
updated: 2024-02-29T01:02:03Z
---

# Introdução

Texto com **negrito**.

Segundo parágrafo.

### Ponto um

> Linha um inventada
> linha dois inventada
>
> — Jo 3:16 (XYZ)

**G25 — amar**

Definição inventada.

> Nota: primeira
> segunda

*Ilustração:* Uma história inventada.

## Fim
";
        assert_eq!(export_markdown(&sermon), expected);
    }

    #[test]
    fn empty_sermon_has_only_front_matter() {
        let sermon = Sermon::new("Vazio", "", 0);
        let out = export_markdown(&sermon);
        assert!(out.starts_with("---\ntitle: \"Vazio\"\nbase_text: \"\"\n"));
        assert!(out.ends_with("---\n"));
    }
}
