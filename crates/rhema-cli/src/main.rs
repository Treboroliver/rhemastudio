use std::collections::{BTreeMap, HashMap};
use std::fs::{create_dir, write, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use rusqlite::{Connection, OpenFlags};

use rhema_core::{Book, VerseRef};
use rhema_library::{Library, ModuleInfo, ModuleState};
use rhema_refs::{FormatStyle, Language, ParseOutcome};
use rhema_twformat::{CommentarySource, DictionarySource, OntReader};

mod inventory;
mod lexmap;

#[derive(Parser)]
#[command(
    name = "rhema-cli",
    about = "RhemaStudio CLI — Ferramenta de inspeção e utilitários para módulos theWord"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Ferramentas do núcleo lexical
    Lex {
        #[command(subcommand)]
        command: LexCommands,
    },

    /// Verifica amostras e mapeamentos lexicais sem alterar módulos
    LexmapCheck {
        /// Pasta que contém os dicionários a analisar
        folder: PathBuf,
    },

    /// Inventaria módulos locais sem alterar seus arquivos
    Inventory {
        /// Pastas para percorrer recursivamente
        #[arg(required = true)]
        folders: Vec<PathBuf>,
    },

    /// Gerencia o catálogo de módulos
    Library {
        #[command(subcommand)]
        command: LibraryCommands,
    },

    /// Inspeciona a estrutura, DDL, metadados e tags de um módulo
    Inspect {
        /// Caminho para o arquivo (.twm, .ont, .ot, .nt, etc.)
        file: PathBuf,
    },

    /// Exibe o texto bruto de um versículo a partir de um módulo bíblico (.ont/.ot/.nt)
    Dump {
        /// Caminho para o arquivo (.ont, .ot, .nt)
        file: PathBuf,

        /// Número do livro canônico (1 a 66; ex: 43 para João)
        #[arg(long, conflicts_with = "reference")]
        book: Option<u8>,

        /// Número do capítulo
        #[arg(long, conflicts_with = "reference")]
        chapter: Option<u16>,

        /// Número do versículo
        #[arg(long, conflicts_with = "reference")]
        verse: Option<u16>,

        /// Referência bíblica (ex.: "Jo 3:16")
        #[arg(long = "ref")]
        reference: Option<String>,

        /// Exibe a árvore de nós interpretada
        #[arg(long, conflicts_with = "plain")]
        parsed: bool,

        /// Exibe texto sem tags e sem notas
        #[arg(long)]
        plain: bool,
    },

    /// Exibe comentários associados a uma referência bíblica
    DumpCmt {
        /// Caminho para um módulo de comentário (.cmt.twm)
        file: PathBuf,
        #[arg(long)]
        book: u8,
        #[arg(long)]
        chapter: u16,
        #[arg(long)]
        verse: u16,
        #[arg(long, conflicts_with = "plain")]
        parsed: bool,
        #[arg(long)]
        plain: bool,
    },

    /// Busca uma entrada de dicionário por chave
    DumpDct {
        /// Caminho para um módulo de dicionário (.dct.twm)
        file: PathBuf,
        #[arg(long)]
        key: String,
        #[arg(long, conflicts_with = "plain")]
        parsed: bool,
        #[arg(long)]
        plain: bool,
    },

    /// Resolve uma referência bíblica sem abrir um módulo
    Ref {
        /// Texto da referência
        text: String,
        /// Idioma das abreviações e sugestões
        #[arg(long, default_value = "pt-BR", value_parser = ["pt-BR", "en"])]
        lang: String,
    },

    /// Grava amostras distribuídas do conteúdo RTF de um módulo SQLite em pasta temporária
    RtfSample {
        /// Caminho para arquivo de módulo SQLite (.twm)
        file: PathBuf,

        /// Quantidade de linhas amostradas entre início, meio e fim da tabela
        #[arg(long, default_value_t = 10)]
        count: usize,
    },
}

#[derive(Subcommand)]
enum LexCommands {
    /// Interpreta uma consulta lexical
    Parse { text: String },
    /// Constrói ligações persistentes a partir das pastas de módulos
    LinksBuild {
        #[arg(required = true)]
        folders: Vec<PathBuf>,
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Lista ligações para um número Strong
    Links {
        strong: String,
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Constrói o índice lexical (tantivy) a partir das pastas de módulos
    IndexBuild {
        #[arg(required = true)]
        folders: Vec<PathBuf>,
        #[arg(long)]
        index_dir: Option<PathBuf>,
    },
    /// Busca no índice lexical (apenas chaves e campos)
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long = "module")]
        modules: Vec<String>,
        #[arg(long)]
        index_dir: Option<PathBuf>,
    },
    /// Mede a latência de consultas variadas no índice lexical
    Bench {
        #[arg(long)]
        index_dir: Option<PathBuf>,
        #[arg(long, default_value_t = 200)]
        count: usize,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Constrói o índice reverso palavra -> Strong a partir das Bíblias com tags
    WordsBuild {
        #[arg(required = true)]
        folders: Vec<PathBuf>,
        #[arg(long)]
        cache: Option<PathBuf>,
    },
    /// Sugere números Strong para uma palavra usando o índice reverso
    Words {
        word: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        cache: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum LibraryCommands {
    /// Varre pastas e atualiza o catálogo local de módulos
    Scan {
        /// Pastas a percorrer (profundidade máxima de seis níveis)
        #[arg(required = true)]
        folders: Vec<PathBuf>,

        /// Imprime o catálogo em JSON
        #[arg(long)]
        json: bool,

        /// Pasta de dados/cache do catálogo
        #[arg(long)]
        cache: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Lex { command } => match command {
            LexCommands::Parse { text } => {
                println!(
                    "{}",
                    serde_json::to_string(&rhema_lexicon::parse_query(&text))?
                );
                Ok(())
            }
            LexCommands::LinksBuild { folders, db } => {
                let path = lex_cache_path(db)?;
                let report = rhema_lexicon::build_links(path, &folders)?;
                println!("{}", serde_json::to_string(&report)?);
                Ok(())
            }
            LexCommands::Links { strong, db } => {
                let path = lex_cache_path(db)?;
                println!(
                    "{}",
                    serde_json::to_string(&rhema_lexicon::links_for_strong(path, &strong)?)?
                );
                Ok(())
            }
            LexCommands::IndexBuild { folders, index_dir } => {
                let dir = lex_index_dir(index_dir)?;
                let report = rhema_lexicon::build_index(&dir, &folders, &mut |_| {})?;
                println!("{}", serde_json::to_string_pretty(&report)?);
                Ok(())
            }
            LexCommands::Search {
                query,
                limit,
                modules,
                index_dir,
            } => {
                let dir = lex_index_dir(index_dir)?;
                let index = open_lex_index(&dir)?;
                let filters = rhema_lexicon::SearchFilters {
                    modules: modules
                        .iter()
                        .map(|m| {
                            index
                                .resolve_module(m)
                                .map_or_else(|| m.clone(), str::to_string)
                        })
                        .collect(),
                    kinds: Vec::new(),
                };
                let mut results = index.search(&query, limit, &filters)?;
                if let Some(msg) = index.status().message() {
                    results.notes.push(msg);
                }
                println!("total={} (mostrando {})", results.total, results.shown);
                println!("{}", serde_json::to_string_pretty(&results)?);
                Ok(())
            }
            LexCommands::Bench {
                index_dir,
                count,
                limit,
            } => {
                let dir = lex_index_dir(index_dir)?;
                let t0 = std::time::Instant::now();
                let index = open_lex_index(&dir)?;
                let open_ms = t0.elapsed().as_secs_f64() * 1000.0;
                let queries = rhema_lexicon::bench_queries(count);
                let filters = rhema_lexicon::SearchFilters::default();
                let mut times = Vec::with_capacity(queries.len());
                let mut non_empty = 0usize;
                for q in &queries {
                    let start = std::time::Instant::now();
                    let res = index.search(q, limit, &filters)?;
                    times.push(start.elapsed().as_secs_f64() * 1000.0);
                    if res.total > 0 {
                        non_empty += 1;
                    }
                }
                times.sort_by(f64::total_cmp);
                let pct = |p: usize| -> f64 {
                    if times.is_empty() {
                        0.0
                    } else {
                        let idx = ((times.len() - 1) * p) / 100;
                        times[idx]
                    }
                };
                let mean = if times.is_empty() {
                    0.0
                } else {
                    times.iter().sum::<f64>() / times.len() as f64
                };
                let out = serde_json::json!({
                    "index_dir": dir,
                    "open_ms": (open_ms * 100.0).round() / 100.0,
                    "queries": times.len(),
                    "non_empty": non_empty,
                    "p50_ms": (pct(50) * 100.0).round() / 100.0,
                    "p95_ms": (pct(95) * 100.0).round() / 100.0,
                    "max_ms": (times.last().copied().unwrap_or(0.0) * 100.0).round() / 100.0,
                    "mean_ms": (mean * 100.0).round() / 100.0,
                });
                println!("{}", serde_json::to_string_pretty(&out)?);
                Ok(())
            }
            LexCommands::WordsBuild { folders, cache } => {
                let path = lex_reverse_path(cache)?;
                let report = rhema_lexicon::build_reverse_index(path, &folders)?;
                println!("{}", serde_json::to_string_pretty(&report)?);
                Ok(())
            }
            LexCommands::Words { word, limit, cache } => {
                let path = lex_reverse_path(cache)?;
                let suggestions = rhema_lexicon::suggest_strongs(path, &word, limit)?;
                println!("{}", serde_json::to_string_pretty(&suggestions)?);
                Ok(())
            }
        },
        Commands::LexmapCheck { folder } => lexmap::run(&folder),
        Commands::Inventory { folders } => inventory::run(&folders),
        Commands::Library { command } => match command {
            LibraryCommands::Scan {
                folders,
                json,
                cache,
            } => scan_library(&folders, cache.as_deref(), json),
        },
        Commands::Inspect { file } => inspect_file(&file),
        Commands::Dump {
            file,
            book,
            chapter,
            verse,
            reference,
            parsed,
            plain,
        } => {
            if let Some(reference) = reference {
                let outcome = rhema_refs::parse(&reference, Language::PtBr);
                if !outcome.erros.is_empty() {
                    bail!("Referência inválida: {outcome:?}");
                }
                let range = outcome
                    .faixas
                    .first()
                    .ok_or_else(|| anyhow::anyhow!("Referência não resolvida: {outcome:?}"))?;
                dump_verse(
                    &file,
                    range.start.book.number(),
                    range.start.chapter,
                    range.start.verse,
                    parsed,
                    plain,
                )
            } else {
                let (Some(book), Some(chapter), Some(verse)) = (book, chapter, verse) else {
                    bail!("Informe --ref ou os três argumentos --book, --chapter e --verse");
                };
                dump_verse(&file, book, chapter, verse, parsed, plain)
            }
        }
        Commands::DumpCmt {
            file,
            book,
            chapter,
            verse,
            parsed,
            plain,
        } => dump_commentary(&file, book, chapter, verse, parsed, plain),
        Commands::DumpDct {
            file,
            key,
            parsed,
            plain,
        } => dump_dictionary(&file, &key, parsed, plain),
        Commands::Ref { text, lang } => {
            let language = if lang == "en" {
                Language::En
            } else {
                Language::PtBr
            };
            let outcome = rhema_refs::parse(&text, language);
            print_parse_outcome(&outcome, language);
            if outcome.faixas.is_empty() && outcome.erros.is_empty() && outcome.sugestões.is_empty()
            {
                bail!("A referência está incompleta; nenhuma faixa foi resolvida");
            }
            if !outcome.erros.is_empty() {
                bail!("A referência contém erros");
            }
            Ok(())
        }
        Commands::RtfSample { file, count } => rtf_sample(&file, count),
    }
}

fn lex_cache_path(path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = path {
        return Ok(path);
    }
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .or_else(|| {
            std::env::var_os("HOME").map(|home| {
                PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .into_os_string()
            })
        })
        .ok_or_else(|| anyhow::anyhow!("Informe --db: pasta de dados local indisponível"))?;
    let dir = PathBuf::from(base).join("RhemaStudio");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("lexicon-links.sqlite"))
}

fn lex_index_dir(path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = path {
        return Ok(path);
    }
    rhema_lexicon::default_index_dir()
        .ok_or_else(|| anyhow::anyhow!("Informe --index-dir: pasta de dados local indisponível"))
}

fn lex_reverse_path(path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = path {
        return Ok(path);
    }
    rhema_lexicon::default_reverse_path()
        .ok_or_else(|| anyhow::anyhow!("Informe --cache: pasta de dados local indisponível"))
}

fn open_lex_index(dir: &Path) -> Result<rhema_lexicon::LexiconIndex> {
    let mut index = rhema_lexicon::LexiconIndex::open(dir)?;
    if let Some(links) = rhema_lexicon::default_data_dir()
        .map(|d| d.join("lexicon-links.sqlite"))
        .filter(|p| p.exists())
    {
        index = index.with_links(links);
    }
    if let Some(rev) = rhema_lexicon::default_reverse_path().filter(|p| p.exists()) {
        index = index.with_reverse(rev);
    }
    Ok(index)
}

fn scan_library(folders: &[PathBuf], cache_dir: Option<&Path>, json: bool) -> Result<()> {
    let cache_dir = match cache_dir {
        Some(path) => path.to_path_buf(),
        None => default_library_cache_dir()?,
    };
    let library = Library::new(cache_dir);
    let started = std::time::Instant::now();
    let result = library.scan(folders, |_| {})?;
    let elapsed = started.elapsed();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "elapsed_ms": elapsed.as_secs_f64() * 1000.0,
                "cache_hits": result.cache_hits,
                "modules": result.modules,
                "warnings": result.warnings,
            }))?
        );
    } else {
        println!("Nome\tTipo\tEstado\tIdioma\tTamanho (bytes)");
        for module in &result.modules {
            println!(
                "{}\t{}\t{}\t{}\t{}",
                table_cell(&module.title),
                module_kind_label(module),
                module_state_label(&module.state),
                table_cell(module.language.as_deref().unwrap_or("-")),
                module.size
            );
        }
        for warning in &result.warnings {
            eprintln!("Aviso: {}: {}", warning.path.display(), warning.message);
        }
        println!(
            "Varredura: {:.2} ms; {} módulo(s) do cache.",
            elapsed.as_secs_f64() * 1000.0,
            result.cache_hits
        );
    }
    Ok(())
}

fn table_cell(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if matches!(character, '\t' | '\r' | '\n') {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn module_kind_label(module: &ModuleInfo) -> &'static str {
    match module.kind {
        rhema_core::ModuleKind::Bible => "Bíblia",
        rhema_core::ModuleKind::Commentary => "Comentário",
        rhema_core::ModuleKind::Dictionary => "Dicionário",
        rhema_core::ModuleKind::Book => "Livro",
        rhema_core::ModuleKind::Devotional => "Devocional",
        rhema_core::ModuleKind::Unknown => "Outro",
    }
}

fn module_state_label(state: &ModuleState) -> &'static str {
    match state {
        ModuleState::Ready => "Pronto",
        ModuleState::Protected => "Protegido",
        ModuleState::UnsupportedFormat { .. } => "FormatoNaoSuportado (em breve)",
        ModuleState::Corrupted { .. } => "Corrompido",
    }
}

fn default_library_cache_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .context("LOCALAPPDATA não está definido; informe --cache")?;
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".local").join("share"))
        })
        .context("Diretório de dados do usuário indisponível; informe --cache")?;
    Ok(base.join("RhemaStudio").join("library"))
}

fn print_parse_outcome(outcome: &ParseOutcome, language: Language) {
    for range in &outcome.faixas {
        println!(
            "{}",
            rhema_refs::format(range, language, FormatStyle::Short)
        );
    }
    for error in &outcome.erros {
        println!("Erro: {error:?}");
    }
    for suggestion in &outcome.sugestões {
        match suggestion.reference {
            Some(reference) => println!(
                "Sugestão: {}",
                rhema_refs::format(&reference, language, FormatStyle::Short)
            ),
            None => println!(
                "Sugestão: {}",
                match language {
                    Language::PtBr => suggestion.book.name_pt(),
                    Language::En => suggestion.book.name_en(),
                }
            ),
        }
    }
}

fn rtf_sample(path: &Path, count: usize) -> Result<()> {
    if count == 0 {
        bail!("--count deve ser maior que zero");
    }
    if !path.exists() {
        bail!("Arquivo não encontrado: {}", path.display());
    }
    rhema_twformat::ensure_twm_unprotected(path)?;
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| {
        format!(
            "Falha ao abrir SQLite somente para leitura: {}",
            path.display()
        )
    })?;
    let row_count: i64 = conn
        .query_row("SELECT count(*) FROM content", [], |row| row.get(0))
        .with_context(|| "Tabela content não encontrada ou inválida")?;
    if row_count == 0 {
        bail!("A tabela content não contém linhas para amostrar");
    }

    let output_dir = create_sample_directory()?;
    let mut statement =
        conn.prepare("SELECT topic_id, data, data2 FROM content ORDER BY topic_id")?;
    let mut rows = statement.query([])?;
    let sample_count = count.min(row_count as usize);
    let sample_offsets = (0..sample_count)
        .map(|index| {
            if sample_count <= 1 {
                0
            } else {
                index * (row_count as usize - 1) / (sample_count - 1)
            }
        })
        .collect::<Vec<_>>();
    let mut next_sample = 0usize;
    let mut row_index = 0usize;
    let mut written = 0usize;
    let mut selected_samples = Vec::new();
    let mut binary_blobs = 0usize;
    let mut binary_bytes = 0usize;
    let mut binary_examples = Vec::new();

    while let Some(row) = rows.next()? {
        if sample_offsets.get(next_sample) != Some(&row_index) {
            row_index += 1;
            continue;
        }
        let topic_id = row.get::<_, i64>(0)?;
        let data = row.get::<_, rusqlite::types::Value>(1)?;
        let data2 = row.get::<_, rusqlite::types::Value>(2)?;
        for (column, value) in [("data", data), ("data2", data2)] {
            match value {
                rusqlite::types::Value::Null => {}
                rusqlite::types::Value::Text(text) => {
                    let extension = if looks_like_rtf(text.as_bytes()) {
                        "rtf"
                    } else {
                        "txt"
                    };
                    let filename = format!("{topic_id}_{column}.{extension}");
                    write(output_dir.join(&filename), text.as_bytes())?;
                    written += 1;
                    selected_samples.push(filename);
                }
                rusqlite::types::Value::Blob(bytes) if looks_like_rtf(&bytes) => {
                    let filename = format!("{topic_id}_{column}.rtf");
                    write(output_dir.join(&filename), &bytes)?;
                    written += 1;
                    selected_samples.push(filename);
                }
                rusqlite::types::Value::Blob(bytes) => {
                    binary_blobs += 1;
                    binary_bytes += bytes.len();
                    if binary_examples.len() < 3 {
                        let hex = bytes
                            .iter()
                            .take(32)
                            .map(|byte| format!("{byte:02X}"))
                            .collect::<Vec<_>>()
                            .join(" ");
                        binary_examples.push(format!(
                            "topic_id={topic_id} {column}: {} bytes, início em hex: {hex}",
                            bytes.len()
                        ));
                    }
                }
                value => println!("topic_id={topic_id} {column}: {value:?}"),
            }
        }
        next_sample += 1;
        row_index += 1;
    }
    if next_sample != sample_count {
        bail!("A amostragem de conteúdo terminou antes de alcançar as linhas esperadas");
    }

    println!(
        "Amostradas {} linhas de {}; {} amostras de texto gravadas em {}",
        sample_count,
        row_count,
        written,
        output_dir.display()
    );
    println!("BLOBs binários não gravados: {binary_blobs} ({binary_bytes} bytes)");
    for example in binary_examples {
        println!("{example}");
    }
    for filename in selected_samples.iter().take(2) {
        println!("Exemplo: {}", output_dir.join(filename).display());
    }
    if let Some(filename) = selected_samples.get(selected_samples.len() / 2) {
        println!("Exemplo central: {}", output_dir.join(filename).display());
    }
    if let Some(filename) = selected_samples.last() {
        println!("Exemplo final: {}", output_dir.join(filename).display());
    }
    Ok(())
}

fn looks_like_rtf(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .copied()
        .skip_while(u8::is_ascii_whitespace)
        .take(5)
        .eq(b"{\\rtf".iter().copied())
}

fn create_sample_directory() -> Result<PathBuf> {
    let temp_root = std::env::temp_dir();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Relógio do sistema anterior à época Unix")?
        .as_millis();
    for attempt in 0..100 {
        let path = temp_root.join(format!(
            "rhemastudio-rtf-samples-{}-{timestamp}-{attempt}",
            std::process::id()
        ));
        match create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bail!("Não foi possível criar pasta temporária exclusiva para amostras RTF")
}

fn dump_verse(
    file: &Path,
    book_num: u8,
    chapter: u16,
    verse_num: u16,
    parsed: bool,
    plain: bool,
) -> Result<()> {
    let book = Book::from_number(book_num).ok_or_else(|| {
        anyhow::anyhow!(
            "Número de livro inválido: {} (deve estar entre 1 e 66)",
            book_num
        )
    })?;

    let reader = OntReader::open(file)?;
    println!("Módulo: {} ({})", reader.info().title, file.display());
    println!(
        "Referência: {} (Livro #{}) {}:{}",
        book.name_pt(),
        book_num,
        chapter,
        verse_num
    );
    println!("Codificação: {}", reader.encoding_name());
    println!("Versículos no módulo: {}", reader.verse_count());
    if reader.is_degraded() {
        println!("Aviso: Módulo operando em modo degradado");
    }

    println!("------------------------------------------------------------");
    if parsed {
        println!("{:#?}", reader.verse_nodes(book, chapter, verse_num)?);
    } else if plain {
        println!(
            "{}",
            rhema_twformat::plain_text(&reader.verse_nodes(book, chapter, verse_num)?)
        );
    } else {
        println!("{}", reader.verse(book, chapter, verse_num)?);
    }
    println!("------------------------------------------------------------");

    Ok(())
}

fn dump_commentary(
    file: &Path,
    book_num: u8,
    chapter: u16,
    verse_num: u16,
    parsed: bool,
    _plain: bool,
) -> Result<()> {
    let book = Book::from_number(book_num).ok_or_else(|| {
        anyhow::anyhow!(
            "Número de livro inválido: {} (deve estar entre 1 e 66)",
            book_num
        )
    })?;
    let source = CommentarySource::open(file)?;
    let entries = source.entries_for(VerseRef::new(book, chapter, verse_num))?;
    let mut output = format!("{} — {}:{}\n", book.name_pt(), chapter, verse_num);
    if entries.is_empty() {
        output.push_str("Nenhum comentário encontrado.\n");
    }
    for entry in entries {
        output.push_str(&format!("Entrada {} ({:?})\n", entry.topic_id, entry.scope));
        match entry.content {
            Ok(nodes) if parsed => output.push_str(&format!("{nodes:#?}\n")),
            Ok(nodes) => output.push_str(&rhema_twformat::plain_text(&nodes)),
            Err(error) => output.push_str(&format!(
                "Aviso: conteúdo desta entrada indisponível: {error}\n"
            )),
        }
        output.push('\n');
    }
    print_limited(&output);
    Ok(())
}

fn dump_dictionary(file: &Path, key: &str, parsed: bool, _plain: bool) -> Result<()> {
    let source = DictionarySource::open(file)?;
    let mut output = String::new();
    match source.lookup(key)? {
        Some(entry) => {
            output.push_str(&format!("{} (id {})\n", entry.subject, entry.topic_id));
            match entry.content {
                Ok(nodes) if parsed => output.push_str(&format!("{nodes:#?}\n")),
                Ok(nodes) => output.push_str(&rhema_twformat::plain_text(&nodes)),
                Err(error) => output.push_str(&format!(
                    "Aviso: conteúdo desta entrada indisponível: {error}\n"
                )),
            }
        }
        None => output.push_str("Entrada não encontrada.\n"),
    }
    print_limited(&output);
    Ok(())
}

fn print_limited(output: &str) {
    print!("{}", limited_output(output));
}

fn limited_output(output: &str) -> String {
    const MAX_CHARS: usize = 1500;
    const MAX_LINES: usize = 40;
    const WARNING: &str = "Aviso: saída truncada.\n";
    let mut result = String::new();
    let mut truncated = false;

    for (line_count, line) in output.lines().enumerate() {
        if line_count >= MAX_LINES - 1 {
            truncated = true;
            break;
        }
        let remaining = MAX_CHARS - WARNING.chars().count() - result.chars().count() - 1;
        let line_length = line.chars().count();
        if line_length > remaining {
            result.extend(line.chars().take(remaining));
            result.push('\n');
            truncated = true;
            break;
        }
        result.push_str(line);
        result.push('\n');
    }
    if truncated {
        result.push_str(WARNING);
    }
    result
}

fn inspect_file(path: &Path) -> Result<()> {
    if !path.exists() {
        bail!("Arquivo não encontrado: {}", path.display());
    }

    let file_size = std::fs::metadata(path)?.len();
    println!("============================================================");
    println!("INSPEÇÃO DE MÓDULO: {}", path.display());
    println!(
        "Tamanho do arquivo: {} bytes ({:.2} MB)",
        file_size,
        file_size as f64 / (1024.0 * 1024.0)
    );
    println!("============================================================\n");

    let mut header = [0u8; 16];
    let mut f = File::open(path)?;
    let read_bytes = f.read(&mut header)?;
    drop(f);

    let is_sqlite = read_bytes >= 15 && &header[0..15] == b"SQLite format 3";
    let is_encrypted = read_bytes >= 9 && &header[0..9] == b"TWENCBMOD";

    if is_encrypted {
        println!("Formato detectado: Módulo theWord Encriptado (Assinatura TWENCBMOD / DRM)");
        println!("Aviso: O módulo utiliza criptografia proprietária do theWord com proteção DRM.");
        println!("Por diretriz do projeto (AGENTS.md / docs/02), o RhemaStudio não implementa engenharia reversa de DRM.");
        return Ok(());
    }

    if is_sqlite
        || path
            .extension()
            .is_some_and(|ext| ext == "twm" || ext == "mybible")
    {
        inspect_sqlite(path)?;
    } else {
        inspect_bible_file(path)?;
    }

    Ok(())
}

fn inspect_sqlite(path: &Path) -> Result<()> {
    println!("Formato detectado: SQLite (banco de dados)");
    println!("Abrindo em modo estritamente SOMENTE LEITURA...\n");
    rhema_twformat::ensure_twm_unprotected(path)?;

    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("Falha ao abrir SQLite: {}", path.display()))?;

    conn.execute_batch("PRAGMA query_only = ON;")?;

    // 1. sqlite_master DDL
    println!("------------------------------------------------------------");
    println!("1. ESTRUTURA DO BANCO (sqlite_master / DDL)");
    println!("------------------------------------------------------------");

    let mut stmt = conn.prepare(
        "SELECT type, name, tbl_name, sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY type, name;"
    )?;
    let ddl_rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    let mut table_names = Vec::new();
    for row in ddl_rows {
        let (obj_type, name, _tbl_name, sql) = row?;
        println!("[{}] {}", obj_type.to_uppercase(), name);
        if obj_type == "table" && !name.starts_with("sqlite_") {
            table_names.push(name.clone());
        }
        for line in sql.lines() {
            println!("    {}", line);
        }
        println!();
    }

    // 2. Tabela de Metadados (config / details / info / properties / etc.)
    println!("------------------------------------------------------------");
    println!("2. METADADOS DO MÓDULO");
    println!("------------------------------------------------------------");

    let mut metadata_found = false;
    for &candidate in &[
        "config",
        "details",
        "info",
        "properties",
        "meta",
        "module_info",
    ] {
        if table_names
            .iter()
            .any(|t| t.eq_ignore_ascii_case(candidate))
        {
            let actual_name = table_names
                .iter()
                .find(|t| t.eq_ignore_ascii_case(candidate))
                .unwrap();
            println!("Encontrada tabela de metadados: \"{}\"", actual_name);

            // Verificar colunas
            let pragma_query = format!("PRAGMA table_info(\"{}\");", actual_name);
            let mut p_stmt = conn.prepare(&pragma_query)?;
            let col_names: Vec<String> = p_stmt
                .query_map([], |r| r.get::<_, String>(1))?
                .filter_map(Result::ok)
                .collect();

            let select_query = format!("SELECT * FROM \"{}\";", actual_name);
            let mut m_stmt = conn.prepare(&select_query)?;
            let col_count = m_stmt.column_count();

            let mut rows = m_stmt.query([])?;
            let mut row_idx = 0;
            while let Some(r) = rows.next()? {
                row_idx += 1;
                print!("  [#{}]: ", row_idx);
                let mut parts = Vec::new();
                for i in 0..col_count {
                    let col_name = col_names.get(i).map(|s| s.as_str()).unwrap_or("col");
                    let val = match r.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => "NULL".to_string(),
                        rusqlite::types::ValueRef::Integer(v) => v.to_string(),
                        rusqlite::types::ValueRef::Real(v) => v.to_string(),
                        rusqlite::types::ValueRef::Text(v) => {
                            let s = String::from_utf8_lossy(v);
                            if s.len() > 80 {
                                format!("{}... (len: {})", &s[..80], s.len())
                            } else {
                                s.to_string()
                            }
                        }
                        rusqlite::types::ValueRef::Blob(v) => format!("[BLOB {} bytes]", v.len()),
                    };
                    parts.push(format!("{} = {:?}", col_name, val));
                }
                println!("{}", parts.join(" | "));
            }
            metadata_found = true;
            println!();
        }
    }

    if !metadata_found {
        println!("(Nenhuma tabela convencional de metadados [config/details/info] identificada)\n");
    }

    // Análise Específica: bible_refs (se presente em comentários)
    if table_names
        .iter()
        .any(|t| t.eq_ignore_ascii_case("bible_refs"))
    {
        println!("------------------------------------------------------------");
        println!("ANÁLISE DE REFERÊNCIAS BÍBLICAS (bible_refs)");
        println!("------------------------------------------------------------");
        let min_max: (i64, i64, i64, i64, i64, i64, i64, i64) = conn.query_row(
            "SELECT MIN(bi), MAX(bi), MIN(ci), MAX(ci), MIN(fvi), MAX(fvi), MIN(tvi), MAX(tvi) FROM bible_refs;",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
        )?;
        println!("  Range de Livros (bi):      {} a {}", min_max.0, min_max.1);
        println!("  Range de Capítulos (ci):   {} a {}", min_max.2, min_max.3);
        println!("  Range Versículo De (fvi):  {} a {}", min_max.4, min_max.5);
        println!("  Range Versículo Até (tvi): {} a {}", min_max.6, min_max.7);

        // Amostras de categorias: Livro intro (ci=0), Capítulo intro (ci>0, fvi=0), Versículo único (fvi=tvi), Faixa (fvi<tvi)
        println!("\n  Exemplos de padrões de referência em bible_refs:");
        let mut b_stmt = conn.prepare(
            "SELECT topic_id, bi, ci, fvi, tvi, content_type FROM bible_refs WHERE ci = 0 LIMIT 2;",
        )?;
        let mut b_rows = b_stmt.query([])?;
        while let Some(r) = b_rows.next()? {
            println!("    [Introdução de Livro]: topic_id={}, bi={}, ci={}, fvi={}, tvi={}, content_type={}",
                r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?);
        }

        let mut c_stmt = conn.prepare(
            "SELECT topic_id, bi, ci, fvi, tvi, content_type FROM bible_refs WHERE ci > 0 AND fvi = 0 LIMIT 2;"
        )?;
        let mut c_rows = c_stmt.query([])?;
        while let Some(r) = c_rows.next()? {
            println!("    [Introdução de Capítulo]: topic_id={}, bi={}, ci={}, fvi={}, tvi={}, content_type={}",
                r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?);
        }

        let mut v_stmt = conn.prepare(
            "SELECT topic_id, bi, ci, fvi, tvi, content_type FROM bible_refs WHERE ci > 0 AND fvi > 0 AND fvi = tvi LIMIT 2;"
        )?;
        let mut v_rows = v_stmt.query([])?;
        while let Some(r) = v_rows.next()? {
            println!(
                "    [Versículo Único]: topic_id={}, bi={}, ci={}, fvi={}, tvi={}, content_type={}",
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, String>(5)?
            );
        }

        let mut r_stmt = conn.prepare(
            "SELECT topic_id, bi, ci, fvi, tvi, content_type FROM bible_refs WHERE ci > 0 AND fvi > 0 AND fvi < tvi LIMIT 2;"
        )?;
        let mut r_rows = r_stmt.query([])?;
        while let Some(r) = r_rows.next()? {
            println!("    [Faixa de Versículos (Perícope)]: topic_id={}, bi={}, ci={}, fvi={}, tvi={}, content_type={}",
                r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?);
        }
        println!();
    }

    // Análise Específica: topics (se presente em dicionários)
    if table_names.iter().any(|t| t.eq_ignore_ascii_case("topics")) {
        println!("------------------------------------------------------------");
        println!("ANÁLISE DE TÓPICOS / CHAVES DE DICIONÁRIO (topics)");
        println!("------------------------------------------------------------");

        // Verificar chaves que parecem Strong Grego (G... ou numéricas)
        let mut g_stmt = conn.prepare(
            "SELECT id, pid, subject, strong_orig_word FROM topics WHERE subject LIKE 'G%' OR subject GLOB '[0-9]*' LIMIT 5;"
        )?;
        let mut g_rows = g_stmt.query([])?;
        println!("  Amostra de entradas Gregas / Numéricas em topics:");
        while let Some(r) = g_rows.next()? {
            println!(
                "    id={}, pid={}, subject={:?}, strong_orig_word={:?}",
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?
            );
        }

        // Verificar chaves que parecem Strong Hebraico (H...)
        let mut h_stmt = conn.prepare(
            "SELECT id, pid, subject, strong_orig_word FROM topics WHERE subject LIKE 'H%' LIMIT 5;"
        )?;
        let mut h_rows = h_stmt.query([])?;
        println!("  Amostra de entradas Hebraicas em topics:");
        while let Some(r) = h_rows.next()? {
            println!(
                "    id={}, pid={}, subject={:?}, strong_orig_word={:?}",
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?
            );
        }
        println!();
    }

    // 3. Amostra de cada tabela (até 5 linhas)
    println!("------------------------------------------------------------");
    println!("3. AMOSTRAS DAS TABELAS (até 5 linhas cada)");
    println!("------------------------------------------------------------");

    for tbl in &table_names {
        let count_query = format!("SELECT count(*) FROM \"{}\";", tbl);
        let total_rows: i64 = conn.query_row(&count_query, [], |r| r.get(0)).unwrap_or(-1);

        println!("Tabela: \"{}\" (Total de linhas: {})", tbl, total_rows);

        let pragma_query = format!("PRAGMA table_info(\"{}\");", tbl);
        let mut p_stmt = conn.prepare(&pragma_query)?;
        let columns: Vec<(String, String)> = p_stmt
            .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?
            .filter_map(Result::ok)
            .collect();

        println!(
            "  Colunas: {}",
            columns
                .iter()
                .map(|(c, t)| format!("{}: {}", c, t))
                .collect::<Vec<_>>()
                .join(", ")
        );

        let sample_query = format!("SELECT * FROM \"{}\" LIMIT 5;", tbl);
        let mut s_stmt = conn.prepare(&sample_query)?;
        let col_count = s_stmt.column_count();

        let mut rows = s_stmt.query([])?;
        let mut sample_idx = 0;
        while let Some(r) = rows.next()? {
            sample_idx += 1;
            print!("    Linha {}: ", sample_idx);
            let mut parts = Vec::new();
            for i in 0..col_count {
                let col_name = columns.get(i).map(|(c, _)| c.as_str()).unwrap_or("col");
                let val = match r.get_ref(i)? {
                    rusqlite::types::ValueRef::Null => "NULL".to_string(),
                    rusqlite::types::ValueRef::Integer(v) => v.to_string(),
                    rusqlite::types::ValueRef::Real(v) => v.to_string(),
                    rusqlite::types::ValueRef::Text(v) => {
                        let s = String::from_utf8_lossy(v);
                        if s.len() > 100 {
                            format!("{}... (len: {})", &s[..100], s.len())
                        } else {
                            s.to_string()
                        }
                    }
                    rusqlite::types::ValueRef::Blob(v) => format!("[BLOB {} bytes]", v.len()),
                };
                parts.push(format!("{}: {:?}", col_name, val));
            }
            println!("{}", parts.join(", "));
        }
        println!();
    }

    Ok(())
}

fn inspect_bible_file(path: &Path) -> Result<()> {
    println!("Formato detectado: Bíblia em texto plano (.ont / .ot / .nt / .ontx)");

    let mut raw_bytes = Vec::new();
    let mut file = File::open(path)?;
    file.read_to_end(&mut raw_bytes)?;

    // 1. Identificar Codificação
    let mut encoding_name = "UTF-8";
    let (text, _, had_errors) = if raw_bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        encoding_name = "UTF-8 com BOM";
        encoding_rs::UTF_8.decode(&raw_bytes[3..])
    } else {
        let (t, _, err) = encoding_rs::UTF_8.decode(&raw_bytes);
        if err {
            encoding_name = "Windows-1252 (fallback)";
            let (w1252_text, _, _) = encoding_rs::WINDOWS_1252.decode(&raw_bytes);
            (w1252_text, encoding_rs::WINDOWS_1252, false)
        } else {
            (t, encoding_rs::UTF_8, false)
        }
    };

    println!(
        "Codificação identificada: {} (erros de decodificação: {})",
        encoding_name, had_errors
    );

    // 2. Linhas de versículos vs Propriedades
    let mut verse_lines = 0usize;
    let mut empty_lines = 0usize;
    let mut property_lines = Vec::new();
    let mut properties = HashMap::new();

    let mut tag_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut tag_families: BTreeMap<String, usize> = BTreeMap::new();

    let mut in_properties = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            empty_lines += 1;
            continue;
        }

        // Bloco de propriedades ao final costuma iniciar com chave=valor
        // Geralmente após os versículos
        if verse_lines >= 7000 && trimmed.contains('=') && !trimmed.starts_with('<') {
            in_properties = true;
        }

        if in_properties {
            property_lines.push(trimmed.to_string());
            if let Some((k, v)) = trimmed.split_once('=') {
                properties.insert(k.trim().to_string(), v.trim().to_string());
            }
        } else {
            verse_lines += 1;

            count_markup_tags(line, &mut tag_counts, &mut tag_families);
        }
    }

    println!("\n------------------------------------------------------------");
    println!("CONTAGEM DE LINHAS");
    println!("------------------------------------------------------------");
    println!("Total de linhas de versículos: {}", verse_lines);
    println!("Total de linhas de propriedades: {}", property_lines.len());
    println!("Linhas vazias ignoradas: {}", empty_lines);

    // Comparação com cânon
    println!("\nComparação com contagem canônica esperada:");
    println!("  .ont esperado: 31.102 | Encontrado: {}", verse_lines);
    println!("  .ot esperado:  23.145");
    println!("  .nt esperado:   7.957");

    // 3. Bloco de propriedades
    println!("\n------------------------------------------------------------");
    println!("PROPRIEDADES DO MÓDULO (chave=valor)");
    println!("------------------------------------------------------------");
    for (k, v) in &properties {
        let character_count = v.chars().count();
        if matches!(
            k.to_ascii_lowercase().as_str(),
            "about" | "creator" | "description"
        ) {
            println!(
                "  {} = [texto de metadado omitido; {} caracteres]",
                k, character_count
            );
        } else if character_count > 80 {
            println!(
                "  {} = [valor longo omitido; {} caracteres]",
                k, character_count
            );
        } else {
            println!("  {} = {}", k, v);
        }
    }

    // 4. Todas as tags distintas
    println!("\n------------------------------------------------------------");
    println!("DISTRIBUIÇÃO DE FAMÍLIAS DE TAGS ENCONTRADAS");
    println!("------------------------------------------------------------");
    let total_tags: usize = tag_families.values().sum();
    println!("Total geral de tags no arquivo: {}\n", total_tags);

    for (fam, count) in &tag_families {
        println!("  {:35} : {:8} ocorrências", fam, count);
    }

    println!("\n------------------------------------------------------------");
    println!("AMOSTRA DE TAGS DISTINTAS (Top 30 mais frequentes)");
    println!("------------------------------------------------------------");
    let mut sorted_tags: Vec<(&String, &usize)> = tag_counts.iter().collect();
    sorted_tags.sort_by(|a, b| b.1.cmp(a.1));

    for (tag, count) in sorted_tags.iter().take(30) {
        println!("  {:25} : {}x", tag, count);
    }

    Ok(())
}

fn count_markup_tags(
    line: &str,
    tag_counts: &mut BTreeMap<String, usize>,
    tag_families: &mut BTreeMap<String, usize>,
) {
    let mut cursor = 0usize;
    while cursor < line.len() {
        let Some(start_offset) = line[cursor..].find('<') else {
            break;
        };
        let start = cursor + start_offset;
        let Some(end_offset) = line[start..].find('>') else {
            break;
        };
        let end = start + end_offset;
        let tag = &line[start..=end];
        let body = tag
            .strip_prefix('<')
            .and_then(|value| value.strip_suffix('>'))
            .unwrap_or_default();
        let name = body.strip_prefix('/').unwrap_or(body);
        let valid = name
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_');
        if valid {
            *tag_counts.entry(tag.to_string()).or_default() += 1;
            let family = if name.starts_with("WG") {
                "<WG####> (Strong Grego)".to_string()
            } else if name.starts_with("WH") {
                "<WH####> (Strong Hebraico)".to_string()
            } else if name.starts_with("WT") {
                "<WT...> (Morfologia)".to_string()
            } else if name.starts_with("TS") {
                "<TS...> (Título de Seção)".to_string()
            } else if name.starts_with("Ts") {
                "<Ts...> (Fim Título de Seção)".to_string()
            } else if name.starts_with("RX") {
                "<RX...> (Referência Cruzada)".to_string()
            } else if name.starts_with("RF") {
                "<RF...> (Início Nota Rodapé)".to_string()
            } else if name.starts_with("Rf") {
                "<Rf> (Fim Nota Rodapé)".to_string()
            } else {
                tag.to_string()
            };
            *tag_families.entry(family).or_default() += 1;
        }
        cursor = end + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{count_markup_tags, limited_output};
    use std::collections::BTreeMap;

    #[test]
    fn twm_dump_output_obeys_line_and_character_caps() {
        let output = (0..100)
            .map(|line| format!("entry {line}: {}", "x".repeat(80)))
            .collect::<Vec<_>>()
            .join("\n");
        let limited = limited_output(&output);
        assert!(limited.chars().count() <= 1500);
        assert!(limited.lines().count() <= 40);
        assert!(limited.contains("Aviso: saída truncada."));

        let long_line = limited_output(&"x".repeat(2000));
        assert!(long_line.chars().count() <= 1500);
        assert!(long_line.lines().count() <= 40);
        assert!(long_line.contains("Aviso: saída truncada."));
    }

    #[test]
    fn tag_inspection_ignores_text_spans_absorbing_nested_markers() {
        let mut tags = BTreeMap::new();
        let mut families = BTreeMap::new();
        count_markup_tags(
            "<word containing <WG123> then <WH456>",
            &mut tags,
            &mut families,
        );

        assert_eq!(tags.get("<WH456>"), Some(&1));
        assert!(!tags.contains_key("<WG123>"));
    }
}
