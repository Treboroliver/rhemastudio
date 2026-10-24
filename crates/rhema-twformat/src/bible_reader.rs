use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use memmap2::Mmap;
use rhema_core::{
    Book, ModuleError, ModuleInfo, ModuleKind, Testament, VerseRef, NEW_TESTAMENT_VERSES,
    OLD_TESTAMENT_VERSES, TOTAL_CANONICAL_VERSES,
};

use crate::detect::detect_signature;
use crate::markup::tokenize_markup;

/// Escopo do texto bíblico contido no módulo
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BibleScope {
    /// Bíblia completa (Antigo e Novo Testamento — 31.102 versículos)
    Full,
    /// Somente Antigo Testamento (23.145 versículos)
    OldTestament,
    /// Somente Novo Testamento (7.957 versículos)
    NewTestament,
    /// Versificação não-padrão ou arquivo parcial aberto em modo degradado
    Degraded { total_verses: usize },
}

/// Leitor de alta performance para Bíblias em texto plano (.ont, .ot, .nt) do theWord.
///
/// Utiliza `memmap2` para mapear o arquivo em memória sem alocar o texto inteiro em RAM,
/// mantendo um índice de offsets de início e fim de cada versículo. A decodificação de cada
/// linha ocorre sob demanda conforme a propriedade `charset` do módulo.
pub struct OntReader {
    mmap: Mmap,
    info: ModuleInfo,
    verse_offsets: Vec<(usize, usize)>,
    properties: HashMap<String, String>,
    encoding: &'static encoding_rs::Encoding,
    scope: BibleScope,
    is_degraded: bool,
}

impl OntReader {
    /// Abre um arquivo de Bíblia (.ont, .ot, .nt) em modo somente leitura.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ModuleError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ModuleError::NotFound(path.display().to_string()));
        }

        let file = File::open(path).map_err(|e| ModuleError::Io(e.to_string()))?;
        let metadata = file
            .metadata()
            .map_err(|e| ModuleError::Io(e.to_string()))?;

        if metadata.len() == 0 {
            return Err(ModuleError::Corrupted(
                "Arquivo do módulo está vazio (0 bytes)".to_string(),
            ));
        }

        let mmap = unsafe {
            Mmap::map(&file).map_err(|e| {
                ModuleError::Io(format!("Falha ao mapear arquivo em memória: {}", e))
            })?
        };

        let bytes = &mmap[..];

        // 1. Detecção de Assinatura (DRM / SQLite / etc.)
        let sample_len = bytes.len().min(32);
        detect_signature(&bytes[..sample_len])?;

        // 2. Indexação de Linhas de Ultra-Alta Performance via memchr SIMD
        let mut raw_lines: Vec<(usize, usize)> = Vec::with_capacity(31200);
        let mut start = 0;

        for newline_pos in memchr::memchr_iter(b'\n', bytes) {
            let mut line_end = newline_pos;
            if line_end > start && bytes[line_end - 1] == b'\r' {
                line_end -= 1;
            }
            raw_lines.push((start, line_end));
            start = newline_pos + 1;
        }

        if start < bytes.len() {
            let mut line_end = bytes.len();
            if line_end > start && bytes[line_end - 1] == b'\r' {
                line_end -= 1;
            }
            raw_lines.push((start, line_end));
        }

        if raw_lines.is_empty() {
            return Err(ModuleError::Corrupted(
                "Nenhuma linha de versículo encontrada no módulo".to_string(),
            ));
        }

        // 3. Delimitação entre Versículos e Bloco de Propriedades (ao final do arquivo)
        // Varredura de trás para frente reconhecendo propriedades, comentários (#),
        // linhas de continuação indentadas e separadores em branco.
        let mut last_verse_idx = raw_lines.len().saturating_sub(1);
        let mut found_verse_boundary = false;

        for (idx, &(l_start, l_end)) in raw_lines.iter().enumerate().rev() {
            let line_bytes = &bytes[l_start..l_end];

            if is_property_line(line_bytes) {
                continue;
            }

            // Encontrou a última linha de texto bíblico
            last_verse_idx = idx;
            found_verse_boundary = true;
            break;
        }

        let verse_count = if found_verse_boundary {
            last_verse_idx + 1
        } else {
            raw_lines.len()
        };

        let verse_offsets: Vec<(usize, usize)> = raw_lines[..verse_count].to_vec();
        let prop_lines = &raw_lines[verse_count..];

        // 4. Detecção de Codificação a partir da propriedade charset (ou fallback)
        let raw_props_map = parse_raw_ascii_properties(bytes, prop_lines);
        let encoding = resolve_encoding(&raw_props_map, bytes);

        // 5. Decodificação Completa das Propriedades
        let properties = decode_properties(bytes, prop_lines, encoding);

        // 6. Determinação de Escopo e Modo Degradado
        let (scope, is_degraded) = match verse_count {
            TOTAL_CANONICAL_VERSES => (BibleScope::Full, false),
            OLD_TESTAMENT_VERSES => (BibleScope::OldTestament, false),
            NEW_TESTAMENT_VERSES => (BibleScope::NewTestament, false),
            other => {
                tracing::warn!(
                    "Módulo {} aberto em modo degradado: {} linhas de versículos (esperado: 31.102, 23.145 ou 7.957)",
                    path.display(),
                    other
                );
                (
                    BibleScope::Degraded {
                        total_verses: other,
                    },
                    true,
                )
            }
        };

        // 7. Construção de ModuleInfo
        let filename = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Bíblia".to_string());

        let title = properties
            .get("title")
            .cloned()
            .or_else(|| properties.get("short.title").cloned())
            .unwrap_or_else(|| filename.clone());

        let abbreviation = properties
            .get("abbrev")
            .cloned()
            .or_else(|| properties.get("short.title").cloned());

        let language = properties.get("lang").cloned();

        let has_strong = properties
            .get("keywords")
            .map(|k| k.to_ascii_lowercase().contains("strong"))
            .unwrap_or(false)
            || properties.get("strong").map(|s| s == "1").unwrap_or(false)
            || properties.contains_key("untranslated.strong.format")
            || properties.contains_key("verse.rule");

        let info = ModuleInfo {
            id: filename,
            path: path.to_path_buf(),
            kind: ModuleKind::Bible,
            title,
            abbreviation,
            language,
            has_strong,
            props: properties.clone(),
        };

        Ok(Self {
            mmap,
            info,
            verse_offsets,
            properties,
            encoding,
            scope,
            is_degraded,
        })
    }

    /// Retorna as informações e metadados do módulo.
    pub fn info(&self) -> &ModuleInfo {
        &self.info
    }

    /// Retorna o escopo do texto bíblico (Completo, AT, NT ou Degradado).
    pub fn scope(&self) -> BibleScope {
        self.scope
    }

    /// Indica se o módulo foi aberto em modo degradado devido a contagem não-canônica.
    pub fn is_degraded(&self) -> bool {
        self.is_degraded
    }

    /// Retorna a quantidade total de versículos indexados.
    pub fn verse_count(&self) -> usize {
        self.verse_offsets.len()
    }

    /// Retorna o mapa de propriedades lidas do final do módulo.
    pub fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }

    /// Retorna o nome da codificação de caracteres ativa.
    pub fn encoding_name(&self) -> &'static str {
        self.encoding.name()
    }

    /// Obtém o texto bruto de uma linha pelo seu índice sequencial (0-indexed).
    pub fn line_by_index(&self, index: usize) -> Result<String, ModuleError> {
        if index >= self.verse_offsets.len() {
            return Err(ModuleError::Corrupted(format!(
                "Índice de linha fora dos limites: {} (total: {})",
                index,
                self.verse_offsets.len()
            )));
        }

        let (start, end) = self.verse_offsets[index];
        if start > end || end > self.mmap.len() {
            return Err(ModuleError::Corrupted(
                "Offset de versículo corrompido ou inconsistente".to_string(),
            ));
        }

        let line_bytes = &self.mmap[start..end];
        let (decoded, _, _) = self.encoding.decode(line_bytes);
        Ok(decoded.into_owned())
    }

    /// Obtém o texto bruto do versículo especificado por livro, capítulo e versículo.
    pub fn verse(&self, book: Book, chapter: u16, verse: u16) -> Result<String, ModuleError> {
        self.verse_by_ref(VerseRef::new(book, chapter, verse))
    }

    /// Retorna os nós tipados do versículo, preservando marcação desconhecida.
    pub fn verse_nodes(
        &self,
        book: Book,
        chapter: u16,
        verse: u16,
    ) -> Result<Vec<rhema_core::Node>, ModuleError> {
        self.verse(book, chapter, verse)
            .map(|text| tokenize_markup(&text))
    }

    /// Obtém o texto bruto do versículo através de um `VerseRef`.
    pub fn verse_by_ref(&self, r: VerseRef) -> Result<String, ModuleError> {
        if !r.is_valid() {
            return Err(ModuleError::VerseNotFound(r));
        }

        match self.scope {
            BibleScope::Full => {
                let idx = r.cumulative_index().ok_or(ModuleError::VerseNotFound(r))?;
                self.line_by_index(idx)
            }
            BibleScope::OldTestament => {
                if r.book.testament() != Testament::Old {
                    return Err(ModuleError::BookNotInModule(
                        r.book,
                        "Módulo contém apenas o Antigo Testamento".to_string(),
                    ));
                }
                let idx = r.cumulative_index().ok_or(ModuleError::VerseNotFound(r))?;
                self.line_by_index(idx)
            }
            BibleScope::NewTestament => {
                if r.book.testament() != Testament::New {
                    return Err(ModuleError::BookNotInModule(
                        r.book,
                        "Módulo contém apenas o Novo Testamento".to_string(),
                    ));
                }
                let full_idx = r.cumulative_index().ok_or(ModuleError::VerseNotFound(r))?;
                let idx = full_idx.saturating_sub(OLD_TESTAMENT_VERSES);
                self.line_by_index(idx)
            }
            BibleScope::Degraded { total_verses } => {
                if let Some(idx) = r.cumulative_index() {
                    if idx < total_verses {
                        return self.line_by_index(idx);
                    }
                }
                Err(ModuleError::VerseNotFound(r))
            }
        }
    }

    /// Retorna todos os versículos de um capítulo completo como pares `(número_versículo, texto_bruto)`.
    pub fn chapter(&self, book: Book, chapter: u16) -> Result<Vec<(u16, String)>, ModuleError> {
        let max_v = book
            .verses_in_chapter(chapter)
            .ok_or_else(|| ModuleError::VerseNotFound(VerseRef::new(book, chapter, 1)))?;

        let mut verses = Vec::with_capacity(max_v as usize);
        for v in 1..=max_v {
            let text = self.verse(book, chapter, v)?;
            verses.push((v, text));
        }

        Ok(verses)
    }

    /// Retorna os nós tipados de cada versículo do capítulo.
    pub fn chapter_nodes(
        &self,
        book: Book,
        chapter: u16,
    ) -> Result<Vec<(u16, Vec<rhema_core::Node>)>, ModuleError> {
        self.chapter(book, chapter).map(|verses| {
            verses
                .into_iter()
                .map(|(number, text)| (number, tokenize_markup(&text)))
                .collect()
        })
    }
}

/// Identifica se uma linha em bytes corresponde ao início ou continuação de propriedade `chave=valor`
fn is_property_line(bytes: &[u8]) -> bool {
    let trimmed = trim_ascii(bytes);
    if trimmed.is_empty() {
        return true;
    }

    if trimmed.starts_with(b"<") {
        return false;
    }

    // Linha de comentário (#...)
    if trimmed.starts_with(b"#") {
        return true;
    }

    // Linha de continuação com indentação (começa com espaço ou tab)
    if bytes.first().is_some_and(|b| b.is_ascii_whitespace()) {
        return true;
    }

    // Se contém '=' e o lado esquerdo é um identificador de propriedade theWord
    if let Some(eq_pos) = trimmed.iter().position(|&b| b == b'=') {
        if eq_pos > 0 {
            let left = trim_ascii(&trimmed[..eq_pos]);
            return !left.is_empty()
                && left.iter().all(|&b| {
                    b.is_ascii_alphanumeric()
                        || b == b'.'
                        || b == b'_'
                        || b == b'#'
                        || b == b'-'
                        || b == b' '
                });
        }
    }

    false
}

/// Remove espaços ASCII nas extremidades de uma fatia de bytes
fn trim_ascii(mut b: &[u8]) -> &[u8] {
    while let Some((&first, rest)) = b.split_first() {
        if first.is_ascii_whitespace() {
            b = rest;
        } else {
            break;
        }
    }
    while let Some((&last, rest)) = b.split_last() {
        if last.is_ascii_whitespace() {
            b = rest;
        } else {
            break;
        }
    }
    b
}

/// Extrai preliminarmente chaves e valores ASCII simples das propriedades
fn parse_raw_ascii_properties(
    bytes: &[u8],
    prop_lines: &[(usize, usize)],
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for &(start, end) in prop_lines {
        let line = trim_ascii(&bytes[start..end]);
        if let Some(eq_pos) = line.iter().position(|&b| b == b'=') {
            let k = String::from_utf8_lossy(trim_ascii(&line[..eq_pos]))
                .trim()
                .to_ascii_lowercase();
            let v = String::from_utf8_lossy(trim_ascii(&line[eq_pos + 1..]))
                .trim()
                .to_string();
            if !k.is_empty() {
                map.insert(k, v);
            }
        }
    }
    map
}

/// Determina a codificação a partir da propriedade `charset`
fn resolve_encoding(
    props: &HashMap<String, String>,
    bytes: &[u8],
) -> &'static encoding_rs::Encoding {
    if let Some(charset_str) = props.get("charset") {
        let charset = charset_str.trim().to_ascii_lowercase();
        match charset.as_str() {
            "0" | "1252" => return encoding_rs::WINDOWS_1252,
            "65001" | "utf-8" | "utf8" => return encoding_rs::UTF_8,
            "1250" => return encoding_rs::WINDOWS_1250,
            "1251" => return encoding_rs::WINDOWS_1251,
            "1253" => return encoding_rs::WINDOWS_1253,
            "1255" => return encoding_rs::WINDOWS_1255,
            _ => {}
        }
    }

    let encoding = if std::str::from_utf8(bytes).is_ok() {
        encoding_rs::UTF_8
    } else {
        encoding_rs::WINDOWS_1252
    };
    let charset = props
        .get("charset")
        .map(String::as_str)
        .unwrap_or("ausente");
    tracing::warn!(
        "Charset '{}' ausente ou desconhecido; fallback selecionado: {}",
        charset,
        encoding.name()
    );
    encoding
}

/// Decodifica completamente o bloco de propriedades com o charset especificado
fn decode_properties(
    bytes: &[u8],
    prop_lines: &[(usize, usize)],
    encoding: &'static encoding_rs::Encoding,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut current_key: Option<String> = None;
    let mut current_val = String::new();

    for &(start, end) in prop_lines {
        let line_bytes = trim_ascii(&bytes[start..end]);
        if line_bytes.is_empty() {
            continue;
        }

        if let Some(eq_pos) = line_bytes.iter().position(|&b| b == b'=') {
            // Salvar chave anterior se pendente
            if let Some(k) = current_key.take() {
                map.insert(k, current_val.trim().to_string());
                current_val.clear();
            }

            let (k_cow, _, _) = encoding.decode(trim_ascii(&line_bytes[..eq_pos]));
            let (v_cow, _, _) = encoding.decode(trim_ascii(&line_bytes[eq_pos + 1..]));

            let k = k_cow.trim().to_string();
            let mut v = v_cow.trim().to_string();

            // Tratar continuação com barra invertida final
            if v.ends_with('\\') {
                v.pop();
                current_key = Some(k);
                current_val = v;
            } else {
                map.insert(k, v);
            }
        } else if current_key.is_some() {
            // Linha de continuação
            let (cont_cow, _, _) = encoding.decode(line_bytes);
            let cont = cont_cow.trim();
            current_val.push(' ');
            current_val.push_str(cont);
            if !current_val.ends_with('\\') {
                if let Some(k) = current_key.take() {
                    map.insert(k, current_val.trim().to_string());
                    current_val.clear();
                }
            } else {
                current_val.pop();
            }
        }
    }

    if let Some(k) = current_key.take() {
        map.insert(k, current_val.trim().to_string());
    }

    map
}
