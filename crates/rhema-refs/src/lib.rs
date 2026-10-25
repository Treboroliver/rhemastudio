use rhema_core::{Book, VerseRange, VerseRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    PtBr,
    En,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatStyle {
    Short,
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefError {
    UnknownBook(String),
    InvalidSyntax(String),
    InvalidChapter {
        book: Book,
        chapter: u16,
        max: u16,
    },
    InvalidVerse {
        book: Book,
        chapter: u16,
        verse: u16,
        max: u16,
    },
    InvalidRange,
    InputTooLong,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub book: Book,
    pub reference: Option<VerseRange>,
}

#[allow(non_camel_case_types)]
pub type Sugestão = Suggestion;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParseOutcome {
    pub faixas: Vec<VerseRange>,
    pub erros: Vec<RefError>,
    pub sugestões: Vec<Sugestão>,
}

#[derive(Debug, Clone, Copy)]
struct BookMatch {
    book: Book,
    consumed: usize,
}

#[derive(Debug, Clone, Copy)]
struct Position {
    chapter: u16,
    verse: Option<u16>,
}

pub fn parse(text: &str, language: Language) -> ParseOutcome {
    if text.len() > 16_384 {
        return ParseOutcome {
            erros: vec![RefError::InputTooLong],
            ..ParseOutcome::default()
        };
    }

    let normalized = normalize(text);
    let segments = normalized.split(';').collect::<Vec<_>>();
    let original_segments = text.split(';').collect::<Vec<_>>();
    let mut outcome = ParseOutcome::default();
    let mut previous_book = None;
    let mut previous_chapter = None;

    for (segment_index, segment) in segments.iter().enumerate() {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let original_segment = original_segments
            .get(segment_index)
            .copied()
            .unwrap_or(segment);
        let (book, rest) = match match_book(segment, language, original_segment) {
            Some(found) => (found.book, segment[found.consumed..].trim()),
            None if previous_book.is_some() => (previous_book.unwrap_or(Book::Genesis), segment),
            None => {
                outcome
                    .erros
                    .push(RefError::UnknownBook(segment.to_string()));
                continue;
            }
        };
        previous_book = Some(book);
        if rest.is_empty() {
            let full_name =
                normalize(book.name_pt()) == segment || normalize(book.name_en()) == segment;
            if !full_name && book.chapter_count() != 1 {
                outcome
                    .sugestões
                    .extend(book_suggestions(segment, language, 8));
            } else if let Some(range) = whole_book(book) {
                outcome.faixas.push(range);
            }
            continue;
        }

        let expressions = rest.split(',').collect::<Vec<_>>();
        let mut last_position = previous_chapter.map(|chapter| Position {
            chapter,
            verse: None,
        });
        for (index, expression) in expressions.iter().enumerate() {
            let expression = expression.trim();
            if expression.is_empty() {
                outcome.sugestões.push(Suggestion {
                    book,
                    reference: last_position
                        .and_then(|position| whole_chapter(book, position.chapter)),
                });
                continue;
            }
            if index > 0
                && expression.chars().all(|c| c.is_ascii_digit())
                && let (Some(chapter), Ok(verse)) = (previous_chapter, expression.parse::<u16>())
            {
                match single_verse(book, chapter, verse) {
                    Ok(range) => outcome.faixas.push(range),
                    Err(error) => outcome.erros.push(error),
                }
                continue;
            }
            match parse_expression(expression, book, language) {
                Ok(Some((range, position))) => {
                    if let Some(position) = position {
                        previous_chapter = Some(position.chapter);
                        last_position = Some(position);
                    }
                    outcome.faixas.push(range);
                }
                Ok(None) => {
                    if let Some(chapter) = last_position.map(|p| p.chapter) {
                        outcome.sugestões.push(Suggestion {
                            book,
                            reference: whole_chapter(book, chapter),
                        });
                    } else if rest.ends_with(':') || rest.ends_with('.') {
                        let chapter = rest.trim_end_matches([':', '.']).parse::<u16>().ok();
                        if let Some(reference) =
                            chapter.and_then(|chapter| whole_chapter(book, chapter))
                        {
                            outcome.sugestões.push(Suggestion {
                                book,
                                reference: Some(reference),
                            });
                        } else {
                            outcome
                                .sugestões
                                .extend(book_suggestions(segment, language, 8));
                        }
                    }
                }
                Err(error) => outcome.erros.push(error),
            }
        }
    }
    outcome
}

pub fn format(range: &VerseRange, language: Language, style: FormatStyle) -> String {
    let book = range.start.book;
    let name = match (language, style) {
        (Language::PtBr, FormatStyle::Short) => book.primary_abbreviation_pt(),
        (Language::En, FormatStyle::Short) => book.primary_abbreviation_en(),
        (Language::PtBr, FormatStyle::Full) => book.name_pt(),
        (Language::En, FormatStyle::Full) => book.name_en(),
    };
    if let Some((start_chapter, end_chapter)) = whole_book_chapters(range)
        && start_chapter == 1
        && end_chapter == book.chapter_count()
    {
        return name.to_string();
    }
    if range.start.chapter == range.end.chapter
        && range.start.verse == 1
        && book.verses_in_chapter(range.start.chapter) == Some(range.end.verse)
    {
        return format!("{name} {}", range.start.chapter);
    }
    if range.is_single_verse() {
        return format!("{name} {}:{}", range.start.chapter, range.start.verse);
    }
    let start = format!("{}:{}", range.start.chapter, range.start.verse);
    let end = if range.start.chapter == range.end.chapter {
        range.end.verse.to_string()
    } else {
        format!("{}:{}", range.end.chapter, range.end.verse)
    };
    format!("{name} {start}-{end}")
}

pub fn suggest_books(prefix: &str, language: Language, limit: usize) -> Vec<Book> {
    if limit == 0 || prefix.len() > 1024 {
        return Vec::new();
    }
    let prefix = normalize(prefix);
    if prefix.is_empty() {
        return Vec::new();
    }
    let mut suggestions = Vec::new();
    for book in Book::ALL {
        let matches = aliases(book, language)
            .iter()
            .any(|alias| normalize(alias).starts_with(&prefix));
        if matches {
            suggestions.push(book);
        }
        if suggestions.len() >= limit {
            break;
        }
    }
    suggestions
}

fn parse_expression(
    expression: &str,
    book: Book,
    language: Language,
) -> Result<Option<(VerseRange, Option<Position>)>, RefError> {
    let (left, right) = match split_range(expression) {
        Some((left, right)) => (left.trim(), Some(right.trim())),
        None => (expression.trim(), None),
    };
    let Some(start_position) = parse_position(left, book, language)? else {
        return Ok(None);
    };
    validate_position(book, start_position)?;

    let start_ref = match start_position.verse {
        Some(verse) => VerseRef::new(book, start_position.chapter, verse),
        None => first_verse(book, start_position.chapter),
    };
    let end_ref = if let Some(right) = right {
        let Some(end_position) = parse_endpoint(right, book, language, start_position)? else {
            return Ok(None);
        };
        validate_position(book, end_position)?;
        match end_position.verse {
            Some(verse) => VerseRef::new(book, end_position.chapter, verse),
            None => last_verse(book, end_position.chapter),
        }
    } else if let Some(verse) = start_position.verse {
        VerseRef::new(book, start_position.chapter, verse)
    } else {
        last_verse(book, start_position.chapter)
    };
    let range = VerseRange::new(start_ref, end_ref).map_err(|_| RefError::InvalidRange)?;
    Ok(Some((range, Some(start_position))))
}

fn parse_endpoint(
    input: &str,
    book: Book,
    language: Language,
    start: Position,
) -> Result<Option<Position>, RefError> {
    if input.contains(':') || input.contains('.') {
        return parse_position(input, book, language);
    }
    if input.is_empty() {
        return Ok(None);
    }
    let number = parse_number(input)?;
    Ok(Some(match start.verse {
        Some(_) => Position {
            chapter: start.chapter,
            verse: Some(number),
        },
        None => Position {
            chapter: number,
            verse: None,
        },
    }))
}

fn parse_position(
    input: &str,
    book: Book,
    _language: Language,
) -> Result<Option<Position>, RefError> {
    let input = input.trim();
    if input.is_empty() || input.ends_with(':') || input.ends_with('.') {
        return Ok(None);
    }
    let mut parts = input.split([':', '.']);
    let first = parts.next().unwrap_or_default();
    let Some(chapter) = parse_number(first).ok() else {
        return Ok(None);
    };
    let verse_part = parts.next();
    if parts.next().is_some() {
        return Err(RefError::InvalidSyntax(input.to_string()));
    }
    if verse_part.is_none() && book.chapter_count() == 1 {
        return Ok(Some(Position {
            chapter: 1,
            verse: Some(chapter),
        }));
    }
    let verse = verse_part.map(parse_number).transpose()?;
    Ok(Some(Position { chapter, verse }))
}

fn parse_number(value: &str) -> Result<u16, RefError> {
    match value.parse::<u16>() {
        Ok(number) => Ok(number),
        Err(_) if value.chars().all(|ch| ch.is_ascii_digit()) => Ok(u16::MAX),
        Err(_) => Err(RefError::InvalidSyntax(value.to_string())),
    }
}

fn validate_position(book: Book, position: Position) -> Result<(), RefError> {
    let max_chapter = book.chapter_count();
    if position.chapter == 0 || position.chapter > max_chapter {
        return Err(RefError::InvalidChapter {
            book,
            chapter: position.chapter,
            max: max_chapter,
        });
    }
    if let Some(verse) = position.verse {
        let max = book.verses_in_chapter(position.chapter).unwrap_or(0);
        if verse == 0 || verse > max {
            return Err(RefError::InvalidVerse {
                book,
                chapter: position.chapter,
                verse,
                max,
            });
        }
    }
    Ok(())
}

fn split_range(input: &str) -> Option<(&str, &str)> {
    input
        .char_indices()
        .find(|(_, ch)| matches!(ch, '-' | '–' | '—'))
        .map(|(index, ch)| {
            let start = &input[..index];
            let end = &input[index + ch.len_utf8()..];
            (start, end)
        })
}

fn single_verse(book: Book, chapter: u16, verse: u16) -> Result<VerseRange, RefError> {
    validate_position(
        book,
        Position {
            chapter,
            verse: Some(verse),
        },
    )?;
    let verse = VerseRef::new(book, chapter, verse);
    Ok(VerseRange::single(verse))
}

fn first_verse(book: Book, chapter: u16) -> VerseRef {
    VerseRef::new(book, chapter, 1)
}

fn last_verse(book: Book, chapter: u16) -> VerseRef {
    VerseRef::new(book, chapter, book.verses_in_chapter(chapter).unwrap_or(1))
}

fn whole_chapter(book: Book, chapter: u16) -> Option<VerseRange> {
    if chapter == 0 || chapter > book.chapter_count() {
        return None;
    }
    VerseRange::new(first_verse(book, chapter), last_verse(book, chapter)).ok()
}

fn whole_book(book: Book) -> Option<VerseRange> {
    VerseRange::new(first_verse(book, 1), last_verse(book, book.chapter_count())).ok()
}

fn whole_book_chapters(range: &VerseRange) -> Option<(u16, u16)> {
    if range.start.verse == 1
        && range.end.verse == range.end.book.verses_in_chapter(range.end.chapter)?
        && range.start.book == range.end.book
    {
        Some((range.start.chapter, range.end.chapter))
    } else {
        None
    }
}

fn match_book(input: &str, language: Language, original: &str) -> Option<BookMatch> {
    if language == Language::PtBr
        && original.trim_start().to_lowercase().starts_with("jó")
        && input.starts_with("jo")
        && input[2..]
            .chars()
            .next()
            .is_none_or(|ch| ch.is_whitespace() || ch.is_ascii_digit())
    {
        return Some(BookMatch {
            book: Book::Job,
            consumed: 2,
        });
    }
    let mut candidates = Vec::new();
    for book in Book::ALL {
        for alias in aliases(book, language) {
            let alias = normalize(&alias);
            if input.starts_with(&alias) {
                let next = input[alias.len()..].chars().next();
                if next.is_none_or(|ch| ch.is_whitespace() || ch.is_ascii_digit()) {
                    candidates.push(BookMatch {
                        book,
                        consumed: alias.len(),
                    });
                }
            }
        }
    }
    candidates.sort_by_key(|candidate| {
        let preferred = language == Language::PtBr && candidate.book == Book::John;
        (
            std::cmp::Reverse(candidate.consumed),
            std::cmp::Reverse(preferred),
        )
    });
    candidates.into_iter().next()
}

fn aliases(book: Book, language: Language) -> Vec<String> {
    let mut aliases = vec![match language {
        Language::PtBr => book.name_pt().to_string(),
        Language::En => book.name_en().to_string(),
    }];
    let abbreviations = match language {
        Language::PtBr => book.abbreviations_pt(),
        Language::En => book.abbreviations_en(),
    };
    aliases.extend(abbreviations.iter().map(|value| (*value).to_string()));
    aliases.extend(
        extra_aliases(book, language)
            .iter()
            .map(|value| (*value).to_string()),
    );
    if matches!(
        book,
        Book::FirstSamuel
            | Book::SecondSamuel
            | Book::FirstKings
            | Book::SecondKings
            | Book::FirstChronicles
            | Book::SecondChronicles
            | Book::FirstCorinthians
            | Book::SecondCorinthians
            | Book::FirstThessalonians
            | Book::SecondThessalonians
            | Book::FirstTimothy
            | Book::SecondTimothy
            | Book::FirstPeter
            | Book::SecondPeter
            | Book::FirstJohn
            | Book::SecondJohn
            | Book::ThirdJohn
    ) {
        let compact = aliases
            .iter()
            .filter(|alias| alias.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
            .map(|alias| alias.replace(' ', ""))
            .collect::<Vec<_>>();
        aliases.extend(compact);
        let ordinal = match book {
            Book::FirstSamuel
            | Book::FirstKings
            | Book::FirstChronicles
            | Book::FirstCorinthians
            | Book::FirstThessalonians
            | Book::FirstTimothy
            | Book::FirstPeter
            | Book::FirstJohn => Some("primeira de"),
            Book::SecondSamuel
            | Book::SecondKings
            | Book::SecondChronicles
            | Book::SecondCorinthians
            | Book::SecondThessalonians
            | Book::SecondTimothy
            | Book::SecondPeter
            | Book::SecondJohn => Some("segunda de"),
            Book::ThirdJohn => Some("terceira de"),
            _ => None,
        };
        if let Some(ordinal) = ordinal.filter(|_| language == Language::PtBr) {
            let base = match book {
                Book::FirstSamuel | Book::SecondSamuel => "Samuel",
                Book::FirstKings | Book::SecondKings => "Reis",
                Book::FirstChronicles | Book::SecondChronicles => "Crônicas",
                Book::FirstCorinthians | Book::SecondCorinthians => "Coríntios",
                Book::FirstThessalonians | Book::SecondThessalonians => "Tessalonicenses",
                Book::FirstTimothy | Book::SecondTimothy => "Timóteo",
                Book::FirstPeter | Book::SecondPeter => "Pedro",
                Book::FirstJohn | Book::SecondJohn | Book::ThirdJohn => "João",
                _ => "",
            };
            aliases.push(format!("{ordinal} {base}"));
            aliases.push(format!(
                "{} {base}",
                match ordinal {
                    "primeira de" => "1a",
                    "segunda de" => "2a",
                    _ => "3a",
                }
            ));
            aliases.push(format!(
                "{}{base}",
                match ordinal {
                    "primeira de" => "1",
                    "segunda de" => "2",
                    _ => "3",
                }
            ));
            aliases.push(format!(
                "{} {base}",
                match ordinal {
                    "primeira de" => "I",
                    "segunda de" => "II",
                    _ => "III",
                }
            ));
            aliases.push(format!(
                "{}{base}",
                match ordinal {
                    "primeira de" => "I",
                    "segunda de" => "II",
                    _ => "III",
                }
            ));
        }
    }
    aliases
}

fn extra_aliases(book: Book, language: Language) -> &'static [&'static str] {
    match (book, language) {
        (Book::Jonah, Language::PtBr) => &["Jn", "Jonas"],
        (Book::John, Language::En) => &["Jn", "John"],
        (Book::John, Language::PtBr) => &["João"],
        (Book::Psalms, Language::PtBr) => &["Salmo", "Salmos"],
        (Book::Psalms, Language::En) => &["Psalm"],
        (Book::Jude, Language::PtBr) => &["Judas", "Jd"],
        (Book::FirstJohn, Language::PtBr) => &["1Jo", "1 João"],
        (Book::SecondJohn, Language::PtBr) => &["2Jo", "2 João"],
        (Book::ThirdJohn, Language::PtBr) => &["3Jo", "3 João"],
        (Book::FirstCorinthians, Language::En) => &["1 Cor", "1 Corinthians"],
        (Book::SecondCorinthians, Language::En) => &["2 Cor", "2 Corinthians"],
        (Book::FirstJohn, Language::En) => &["1 John"],
        (Book::SecondJohn, Language::En) => &["2 John"],
        (Book::ThirdJohn, Language::En) => &["3 John"],
        (Book::Jude, Language::En) => &["Jude"],
        _ => &[],
    }
}

fn book_suggestions(prefix: &str, language: Language, limit: usize) -> Vec<Suggestion> {
    suggest_books(prefix, language, limit)
        .into_iter()
        .map(|book| Suggestion {
            book,
            reference: None,
        })
        .collect()
}

fn normalize(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut pending_space = false;
    for ch in input.chars() {
        let folded = fold_char(ch);
        if folded.is_whitespace() {
            pending_space = !output.is_empty();
            continue;
        }
        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        output.push(folded);
    }
    output
}

fn fold_char(ch: char) -> char {
    match ch {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
        'ç' | 'Ç' => 'c',
        'ñ' | 'Ñ' => 'n',
        'ª' => 'a',
        'º' => 'o',
        other => other.to_ascii_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn parses_reference_lists_partial_inputs_and_chapter_ranges() {
        let outcome = parse("Jo 3:16-18; 4:1,3", Language::PtBr);
        assert!(outcome.erros.is_empty());
        assert_eq!(outcome.faixas.len(), 3);
        assert_eq!(
            format(&outcome.faixas[0], Language::PtBr, FormatStyle::Short),
            "Jo 3:16-18"
        );
        assert_eq!(parse("Jo 3:16-4:2", Language::PtBr).faixas.len(), 1);
        assert_eq!(parse("Sl 23:1-6; 27:1", Language::PtBr).faixas.len(), 2);
        let partial = parse("Jo 3:", Language::PtBr);
        assert!(partial.erros.is_empty());
        assert!(!partial.sugestões.is_empty());
    }

    #[test]
    fn parses_accents_ordinals_and_single_chapter_books() {
        for input in [
            "Êxodo 1:1",
            "exodo 1:1",
            "JOAO 3:16",
            "1 João 1:1",
            "1ª João 1:1",
            "I João 1:1",
            "Primeira de João 1:1",
            "Jo 3.16",
        ] {
            assert_eq!(parse(input, Language::PtBr).erros.len(), 0, "{input}");
        }
        assert_eq!(
            format(
                &parse("Jd 3", Language::PtBr).faixas[0],
                Language::PtBr,
                FormatStyle::Short
            ),
            "Jd 1:3"
        );
        assert_eq!(parse("Judas", Language::PtBr).faixas.len(), 1);
        assert_eq!(parse("Psalm 23", Language::En).faixas.len(), 1);
    }

    #[test]
    fn language_disambiguates_jn_and_book_suggestions_are_bounded() {
        assert_eq!(
            parse("Jn 1:1", Language::PtBr).faixas[0].start.book,
            Book::Jonah
        );
        assert_eq!(
            parse("Jn 1:1", Language::En).faixas[0].start.book,
            Book::John
        );
        assert!(suggest_books("jo", Language::PtBr, 2).len() <= 2);
    }

    #[test]
    fn formats_short_and_full_names_and_round_trips() {
        let range = parse("Jo 3:16-18", Language::PtBr).faixas[0];
        assert_eq!(
            format(&range, Language::PtBr, FormatStyle::Full),
            "João 3:16-18"
        );
        for book in Book::ALL {
            let range = VerseRange::single(VerseRef::new(book, 1, 1));
            for language in [Language::PtBr, Language::En] {
                let formatted = format(&range, language, FormatStyle::Short);
                assert_eq!(parse(&formatted, language).faixas, vec![range]);
            }
        }
    }

    #[test]
    fn reports_canonical_limits_and_long_input() {
        assert!(matches!(
            parse("Jo 22", Language::PtBr).erros.first(),
            Some(RefError::InvalidChapter { max: 21, .. })
        ));
        assert!(matches!(
            parse("Jo 3:37", Language::PtBr).erros.first(),
            Some(RefError::InvalidVerse { max: 36, .. })
        ));
        assert!(matches!(
            parse("Jo 999999999999999999", Language::PtBr).erros.first(),
            Some(RefError::InvalidChapter { max: 21, .. })
        ));
        assert_eq!(
            parse(&"x".repeat(20_000), Language::PtBr).erros,
            [RefError::InputTooLong]
        );
    }

    #[test]
    fn benchmark_typical_reference_parse() {
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let result = parse("Jo 3:16-18; 4:1,3", Language::PtBr);
            assert!(result.erros.is_empty());
        }
        eprintln!(
            "M2.1 parse típico: {:.3} µs/op",
            start.elapsed().as_secs_f64() * 1_000.0 / 10_000.0
        );
    }

    proptest! {
        #[test]
        fn arbitrary_string_never_panics(text in any::<String>()) {
            let _ = parse(&text, Language::PtBr);
            let _ = parse(&text, Language::En);
        }

        #[test]
        fn formatted_verse_ranges_round_trip(book_num in 1u8..=66, chapter_seed in 0u16..=u16::MAX, verse_seed in 0u16..=u16::MAX) {
            let book = Book::from_number(book_num).unwrap_or(Book::Genesis);
            let chapter = chapter_seed % book.chapter_count() + 1;
            let max_verse = book.verses_in_chapter(chapter).unwrap_or(1);
            let verse = verse_seed % max_verse + 1;
            let range = VerseRange::single(VerseRef::new(book, chapter, verse));
            let text = format(&range, Language::PtBr, FormatStyle::Short);
            prop_assert_eq!(parse(&text, Language::PtBr).faixas, vec![range]);
        }
    }

    #[test]
    fn data_table_has_two_hundred_cases_and_expected_results() {
        let cases = include_str!("../tests/refs_cases.tsv");
        let mut rows = cases.lines();
        assert_eq!(rows.next(), Some("input\tlanguage\texpected"));
        let mut count = 0;
        let mut failures = Vec::new();
        for row in rows {
            let parts = row.split('\t').collect::<Vec<_>>();
            if parts.len() != 3 {
                failures.push(row.to_string());
                continue;
            }
            let language = if parts[1] == "en" {
                Language::En
            } else {
                Language::PtBr
            };
            let input = parts[0];
            let parsed = parse(input, language);
            let actual = if !parsed.erros.is_empty() {
                "error"
            } else if !parsed.faixas.is_empty() {
                "ok"
            } else {
                "partial"
            };
            count += 1;
            if actual != parts[2] {
                failures.push(format!("{input} => {actual}, esperado {}", parts[2]));
            }
        }
        assert_eq!(count, 200, "quantidade inesperada de casos tabulares");
        assert!(failures.is_empty(), "Falhas na tabela: {failures:?}");
    }
}
