//! Léxico (Strong) e comentários para o painel de estudo.
//!
//! Todos os módulos são abertos somente para leitura por `rhema-twformat`, que recusa módulos
//! protegidos antes de consultar qualquer conteúdo. Módulos que o catálogo não marca como
//! prontos nem chegam a ser abertos.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use rhema_core::{Book, ModuleError, ModuleKind, VerseRef};
use rhema_library::{ModuleInfo, ModuleState};
use rhema_twformat::{
    CommentaryHeader, CommentaryScope, CommentarySource, DictionarySource, normalize_strong_key,
    strong_key,
};
use serde::Serialize;

use crate::reader::{ApiError, UiNode, convert_nodes};

/// Quantidade máxima de leitores `.twm` abertos por tipo.
pub const STUDY_CACHE_CAPACITY: usize = 6;

/// Parágrafos por página de comentário.
pub const COMMENTARY_PAGE_PARAGRAPHS: usize = 60;

/// Caracteres por página de comentário; um parágrafo gigante também é dividido.
pub const COMMENTARY_PAGE_CHARS: usize = 8_000;

/// Cache LRU genérico; o primeiro item é o menos usado recentemente.
pub struct LruCache<T> {
    capacity: usize,
    entries: Mutex<Vec<(String, Arc<T>)>>,
    opens: AtomicUsize,
}

impl<T> LruCache<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Mutex::new(Vec::new()),
            opens: AtomicUsize::new(0),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<(String, Arc<T>)>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Quantas vezes um arquivo foi realmente aberto (para verificar o reuso).
    #[cfg(test)]
    pub fn open_count(&self) -> usize {
        self.opens.load(Ordering::Relaxed)
    }

    pub fn get_or_open(
        &self,
        id: &str,
        open: impl FnOnce() -> Result<T, ModuleError>,
    ) -> Result<Arc<T>, ApiError> {
        {
            let mut entries = self.lock();
            if let Some(position) = entries.iter().position(|(key, _)| key == id) {
                let entry = entries.remove(position);
                let value = Arc::clone(&entry.1);
                entries.push(entry);
                return Ok(value);
            }
        }
        let value = Arc::new(open()?);
        self.opens.fetch_add(1, Ordering::Relaxed);
        let mut entries = self.lock();
        if let Some(position) = entries.iter().position(|(key, _)| key == id) {
            entries.remove(position);
        }
        entries.push((id.to_string(), Arc::clone(&value)));
        while entries.len() > self.capacity {
            entries.remove(0);
        }
        Ok(value)
    }
}

/// Leitores `.twm` abertos. A conexão SQLite não é compartilhável entre threads, então cada
/// leitor fica atrás de um `Mutex`.
pub struct StudyCache {
    pub lexicons: LruCache<Mutex<DictionarySource>>,
    pub commentaries: LruCache<Mutex<CommentarySource>>,
}

impl StudyCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            lexicons: LruCache::new(capacity),
            commentaries: LruCache::new(capacity),
        }
    }

    fn dictionary(&self, module: &ModuleInfo) -> Result<Arc<Mutex<DictionarySource>>, ApiError> {
        self.lexicons.get_or_open(&module.id, || {
            DictionarySource::open(&module.path).map(Mutex::new)
        })
    }

    fn commentary(&self, module: &ModuleInfo) -> Result<Arc<Mutex<CommentarySource>>, ApiError> {
        self.commentaries.get_or_open(&module.id, || {
            CommentarySource::open(&module.path).map(Mutex::new)
        })
    }
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Dicionários na ordem de preferência. Sem lista salva, os prontos em português vêm primeiro.
pub fn effective_lexicons(saved: Option<&[String]>, catalog: &[ModuleInfo]) -> Vec<ModuleInfo> {
    let usable = |module: &&ModuleInfo| {
        module.kind == ModuleKind::Dictionary && module.state == ModuleState::Ready
    };
    match saved {
        Some(ids) => ids
            .iter()
            .filter_map(|id| {
                catalog
                    .iter()
                    .filter(usable)
                    .find(|module| module.id == *id)
            })
            .cloned()
            .collect(),
        None => {
            let mut modules = catalog.iter().filter(usable).cloned().collect::<Vec<_>>();
            modules.sort_by_key(|module| module.language.as_deref() != Some("pt"));
            modules
        }
    }
}

/// Comentários ativos. Sem lista salva, todos os prontos.
pub fn effective_commentaries(saved: Option<&[String]>, catalog: &[ModuleInfo]) -> Vec<ModuleInfo> {
    let usable = |module: &&ModuleInfo| {
        module.kind == ModuleKind::Commentary && module.state == ModuleState::Ready
    };
    match saved {
        Some(ids) => ids
            .iter()
            .filter_map(|id| {
                catalog
                    .iter()
                    .filter(usable)
                    .find(|module| module.id == *id)
            })
            .cloned()
            .collect(),
        None => catalog.iter().filter(usable).cloned().collect(),
    }
}

/// Erro de um módulo que o catálogo não considera pronto. Nunca abre o arquivo.
fn state_error(module: &ModuleInfo) -> Option<ApiError> {
    match &module.state {
        ModuleState::Ready => None,
        ModuleState::Protected => Some(ApiError::new(
            "module_protected",
            "Módulo protegido: não é lido pelo RhemaStudio.",
        )),
        ModuleState::UnsupportedFormat { message } => {
            Some(ApiError::new("module_unsupported", message.clone()))
        }
        ModuleState::Corrupted { message } => {
            Some(ApiError::new("module_corrupted", message.clone()))
        }
    }
}

/// Erro ao consultar uma entrada de um módulo já aberto.
fn entry_error(error: ModuleError) -> ApiError {
    match error {
        ModuleError::NotFound(_) => {
            ApiError::new("entry_not_found", "Esta entrada não existe no módulo.")
        }
        ModuleError::UnsupportedContent(kind) => ApiError::new(
            "unsupported_format",
            format!("O formato de conteúdo “{kind}” ainda não é suportado."),
        ),
        other => other.into(),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LexiconEntryDto {
    /// Chave normalizada (`G25`).
    pub key: String,
    /// Chave do verbete como está no módulo.
    pub title: String,
    pub original_word: Option<String>,
    pub module_id: String,
    pub module_title: String,
    pub degraded: bool,
    pub nodes: Vec<UiNode>,
}

/// Procura o verbete de Strong nos léxicos, na ordem recebida.
pub fn lookup_strong(
    cache: &StudyCache,
    lexicons: &[ModuleInfo],
    strong_id: &str,
) -> Result<LexiconEntryDto, ApiError> {
    let (lang, number) = normalize_strong_key(strong_id)
        .ok_or_else(|| ApiError::new("invalid_strong_id", "Número de Strong inválido."))?;
    let key = strong_key(lang, number);
    let usable = lexicons
        .iter()
        .filter(|module| {
            module.kind == ModuleKind::Dictionary && module.state == ModuleState::Ready
        })
        .collect::<Vec<_>>();
    if usable.is_empty() {
        return Err(ApiError::new(
            "no_lexicon_installed",
            "Nenhum dicionário pronto foi encontrado na biblioteca.",
        ));
    }

    let mut answered = false;
    let mut unsupported = None;
    let mut failure = None;
    for module in usable {
        let source = match cache.dictionary(module) {
            Ok(source) => source,
            Err(error) => {
                failure = Some(error);
                continue;
            }
        };
        let found = locked(&source).lookup(&key);
        match found {
            Ok(Some(entry)) => match entry.content {
                Ok(nodes) => {
                    return Ok(LexiconEntryDto {
                        key,
                        title: entry.subject,
                        original_word: entry.strong_orig_word,
                        module_id: module.id.clone(),
                        module_title: module.title.clone(),
                        degraded: entry.degraded,
                        nodes: convert_nodes(&nodes),
                    });
                }
                Err(error) => {
                    answered = true;
                    let converted = entry_error(error);
                    if converted.code == "unsupported_format" {
                        unsupported.get_or_insert(converted);
                    } else {
                        failure = Some(converted);
                    }
                }
            },
            Ok(None) => answered = true,
            Err(error) => failure = Some(error.into()),
        }
    }
    if let Some(error) = unsupported {
        return Err(error);
    }
    if answered {
        return Err(ApiError::new(
            "entry_not_found",
            "Este número não está no dicionário escolhido.",
        ));
    }
    Err(failure.unwrap_or_else(|| {
        ApiError::new(
            "entry_not_found",
            "Este número não está no dicionário escolhido.",
        )
    }))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommentaryHeaderDto {
    pub topic_id: i64,
    /// `verse`, `range`, `chapter` (faixa que cobre o capítulo todo), `chapterIntro` ou `bookIntro`.
    pub scope: &'static str,
    /// Rótulo legível no idioma da interface, como "Versículo 16".
    pub label: String,
    pub chapter: u16,
    pub from_verse: u16,
    pub to_verse: u16,
    pub size_bytes: Option<u64>,
    pub content_type: String,
    pub supported: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommentaryModuleDto {
    pub module_id: String,
    pub module_title: String,
    pub error: Option<ApiError>,
    pub entries: Vec<CommentaryHeaderDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommentaryIndexDto {
    pub book: u8,
    pub chapter: u16,
    pub verse: u16,
    pub modules: Vec<CommentaryModuleDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommentaryPageDto {
    pub module_id: String,
    pub topic_id: i64,
    pub offset: u64,
    pub next_offset: u64,
    pub has_more: bool,
    pub degraded: bool,
    pub nodes: Vec<UiNode>,
}

fn is_english(language: &str) -> bool {
    language.eq_ignore_ascii_case("en")
}

fn header_dto(header: &CommentaryHeader, english: bool) -> CommentaryHeaderDto {
    let last_verse = Book::from_number(header.bi)
        .and_then(|book| book.verses_in_chapter(header.ci))
        .unwrap_or(header.fvi);
    let to_verse = if header.tvi == 0 {
        last_verse
    } else {
        header.tvi
    };
    let (scope, label) = match header.scope {
        CommentaryScope::Book => (
            "bookIntro",
            if english {
                "Book introduction"
            } else {
                "Introdução ao livro"
            }
            .to_string(),
        ),
        CommentaryScope::Chapter => (
            "chapterIntro",
            if english {
                format!("Chapter {} introduction", header.ci)
            } else {
                format!("Introdução ao capítulo {}", header.ci)
            },
        ),
        CommentaryScope::Verse if header.fvi == to_verse => (
            "verse",
            if english {
                format!("Verse {}", header.fvi)
            } else {
                format!("Versículo {}", header.fvi)
            },
        ),
        CommentaryScope::Verse if header.fvi == 1 && to_verse >= last_verse => (
            "chapter",
            if english {
                format!("Chapter {}", header.ci)
            } else {
                format!("Capítulo {}", header.ci)
            },
        ),
        CommentaryScope::Verse => (
            "range",
            if english {
                format!("Verses {}–{}", header.fvi, to_verse)
            } else {
                format!("Versículos {}–{}", header.fvi, to_verse)
            },
        ),
    };
    CommentaryHeaderDto {
        topic_id: header.topic_id,
        scope,
        label,
        chapter: header.ci,
        from_verse: header.fvi,
        to_verse: if header.fvi == 0 { 0 } else { to_verse },
        size_bytes: header.size_bytes,
        content_type: header.content_type.clone(),
        supported: header.supported,
    }
}

/// Cabeçalhos dos comentários ativos para o versículo, do mais específico ao mais amplo.
/// O conteúdo não é lido nem convertido.
pub fn commentary_index(
    cache: &StudyCache,
    modules: &[ModuleInfo],
    book: u8,
    chapter: u16,
    verse: u16,
    language: &str,
) -> Result<CommentaryIndexDto, ApiError> {
    let book_value = Book::from_number(book)
        .ok_or_else(|| ApiError::new("invalid_reference", "Livro inválido."))?;
    let reference = VerseRef::new(book_value, chapter, verse);
    if reference.validate().is_err() {
        return Err(ApiError {
            book: Some(book),
            max: book_value.verses_in_chapter(chapter),
            ..ApiError::new("invalid_reference", "Versículo fora dos limites do livro.")
        });
    }
    let english = is_english(language);
    let build = |module: &ModuleInfo| {
        let mut dto = CommentaryModuleDto {
            module_id: module.id.clone(),
            module_title: module.title.clone(),
            error: None,
            entries: Vec::new(),
        };
        if let Some(error) = state_error(module) {
            dto.error = Some(error);
            return dto;
        }
        let result = cache.commentary(module).and_then(|source| {
            locked(&source)
                .headers_for(reference)
                .map_err(ApiError::from)
        });
        match result {
            Ok(headers) => {
                dto.entries = headers
                    .iter()
                    .map(|header| header_dto(header, english))
                    .collect();
            }
            Err(error) => dto.error = Some(error),
        }
        dto
    };
    let commentaries = modules
        .iter()
        .filter(|module| module.kind == ModuleKind::Commentary)
        .collect::<Vec<_>>();
    // Abrir um comentário grande pela primeira vez custa dezenas de ms; módulos distintos
    // são independentes, então abrem em paralelo e o tempo frio é o do mais lento.
    let modules = std::thread::scope(|scope| {
        let handles = commentaries
            .iter()
            .map(|module| scope.spawn(|| build(module)))
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .zip(&commentaries)
            .map(|(handle, module)| {
                handle.join().unwrap_or_else(|_| CommentaryModuleDto {
                    module_id: module.id.clone(),
                    module_title: module.title.clone(),
                    error: Some(ApiError::internal("falha ao consultar o módulo")),
                    entries: Vec::new(),
                })
            })
            .collect::<Vec<_>>()
    });
    Ok(CommentaryIndexDto {
        book,
        chapter,
        verse,
        modules,
    })
}

/// Uma página do conteúdo de uma entrada. Só os parágrafos pedidos são convertidos.
pub fn commentary_entry(
    cache: &StudyCache,
    module: &ModuleInfo,
    topic_id: i64,
    offset: u64,
) -> Result<CommentaryPageDto, ApiError> {
    if module.kind != ModuleKind::Commentary {
        return Err(ApiError::new(
            "module_unsupported",
            "O módulo não é um comentário.",
        ));
    }
    if let Some(error) = state_error(module) {
        return Err(error);
    }
    let source = cache.commentary(module)?;
    let page = locked(&source)
        .entry_page(
            topic_id,
            offset,
            COMMENTARY_PAGE_PARAGRAPHS,
            COMMENTARY_PAGE_CHARS,
        )
        .map_err(entry_error)?;
    Ok(CommentaryPageDto {
        module_id: module.id.clone(),
        topic_id,
        offset,
        next_offset: page.next_offset,
        has_more: page.has_more,
        degraded: page.degraded,
        nodes: convert_nodes(&page.nodes),
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::Instant;

    use gen_fixtures::{
        generate_synthetic_dictionary_twm, generate_synthetic_protected_twm,
        generate_synthetic_study_commentary_twm, generate_synthetic_study_dictionary_twm,
    };

    use super::*;

    fn module(id: &str, path: &Path, kind: ModuleKind, language: &str) -> ModuleInfo {
        ModuleInfo {
            id: id.into(),
            path: path.to_path_buf(),
            kind,
            state: ModuleState::Ready,
            title: format!("Módulo {id}"),
            abbreviation: None,
            language: Some(language.into()),
            language_name: None,
            description: None,
            version: None,
            size: 1,
            has_strong: false,
        }
    }

    fn json<T: Serialize>(value: &T) -> serde_json::Value {
        serde_json::to_value(value).expect("serializar")
    }

    fn paragraphs(nodes: &[UiNode]) -> usize {
        nodes
            .iter()
            .filter(|node| matches!(node, UiNode::Paragraph))
            .count()
    }

    fn dictionary_fixture(directory: &Path, name: &str) -> PathBuf {
        let path = directory.join(format!("{name}.dct.twm"));
        generate_synthetic_study_dictionary_twm(&path).expect("gerar dicionário");
        path
    }

    #[test]
    fn lookup_finds_normalizes_and_converts_links() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = dictionary_fixture(directory.path(), "lexicon");
        let lexicon = module("lex", &path, ModuleKind::Dictionary, "pt");
        let cache = StudyCache::new(STUDY_CACHE_CAPACITY);
        for query in ["G25", "g0025", " G25a "] {
            let entry = lookup_strong(&cache, std::slice::from_ref(&lexicon), query)
                .unwrap_or_else(|error| panic!("{query}: {error:?}"));
            assert_eq!(entry.key, "G25");
            assert_eq!(entry.title, "G25");
            assert_eq!(entry.module_id, "lex");
            assert_eq!(entry.original_word.as_deref(), Some("synthetic-lemma"));
        }
        let entry = lookup_strong(&cache, std::slice::from_ref(&lexicon), "G25").expect("G25");
        let value = json(&entry.nodes);
        let encoded = value.to_string();
        assert!(encoded.contains("\"type\":\"entryLink\"") && encoded.contains("\"num\":1"));
        assert!(encoded.contains("\"type\":\"verseLink\"") && encoded.contains("\"book\":43"));
        assert!(encoded.contains("\"type\":\"bold\""));
        assert_eq!(cache.lexicons.open_count(), 1, "o leitor é reaproveitado");

        let other = lookup_strong(&cache, std::slice::from_ref(&lexicon), "G1").expect("G1");
        assert!(
            json(&other.nodes)
                .to_string()
                .contains("\"hint\":\"person\""),
            "subtipo desconhecido vira texto com dica"
        );
        assert!(!json(&other.nodes).to_string().contains("tw://"));
        let hebrew = lookup_strong(&cache, std::slice::from_ref(&lexicon), "H430").expect("H430");
        assert_eq!(hebrew.key, "H430");
    }

    #[test]
    fn lookup_reports_stable_error_codes() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = dictionary_fixture(directory.path(), "lexicon");
        let lexicon = module("lex", &path, ModuleKind::Dictionary, "pt");
        let cache = StudyCache::new(STUDY_CACHE_CAPACITY);
        let code = |lexicons: &[ModuleInfo], id: &str| {
            lookup_strong(&cache, lexicons, id)
                .expect_err("deveria falhar")
                .code
        };
        assert_eq!(
            code(std::slice::from_ref(&lexicon), "G9999"),
            "entry_not_found"
        );
        let degraded = lookup_strong(&cache, std::slice::from_ref(&lexicon), "G26").expect("RVF");
        assert!(degraded.degraded);
        assert_eq!(
            code(std::slice::from_ref(&lexicon), "X25"),
            "invalid_strong_id"
        );
        assert_eq!(
            code(std::slice::from_ref(&lexicon), "G0"),
            "invalid_strong_id"
        );
        assert_eq!(code(&[], "G25"), "no_lexicon_installed");

        let mut commentary = lexicon.clone();
        commentary.kind = ModuleKind::Commentary;
        assert_eq!(code(&[commentary], "G25"), "no_lexicon_installed");
    }

    #[test]
    fn lookup_never_opens_protected_modules() {
        let directory = tempfile::tempdir().expect("tempdir");
        let missing = directory.path().join("never-opened.dct.twm");
        let mut locked = module("locked", &missing, ModuleKind::Dictionary, "pt");
        locked.state = ModuleState::Protected;
        let cache = StudyCache::new(STUDY_CACHE_CAPACITY);
        let error = lookup_strong(&cache, &[locked], "G25").expect_err("sem léxico utilizável");
        assert_eq!(error.code, "no_lexicon_installed");
        assert_eq!(cache.lexicons.open_count(), 0);

        // Mesmo marcado como pronto, o leitor recusa o arquivo protegido antes de ler o conteúdo.
        let protected = directory.path().join("protected.dct.twm");
        generate_synthetic_protected_twm(&protected, "do.encrypt").expect("gerar protegido");
        let stale = module("stale", &protected, ModuleKind::Dictionary, "pt");
        let error = lookup_strong(&cache, &[stale], "G25").expect_err("arquivo protegido");
        assert_eq!(error.code, "module_protected");
        assert_eq!(cache.lexicons.len(), 0);
    }

    #[test]
    fn lookup_tries_lexicons_in_order_and_falls_through() {
        let directory = tempfile::tempdir().expect("tempdir");
        let full = dictionary_fixture(directory.path(), "full");
        let small = directory.path().join("small.dct.twm");
        generate_synthetic_dictionary_twm(&small).expect("gerar dicionário pequeno");
        let first = module("small", &small, ModuleKind::Dictionary, "pt");
        let second = module("full", &full, ModuleKind::Dictionary, "en");
        let cache = StudyCache::new(STUDY_CACHE_CAPACITY);
        let both = [first.clone(), second.clone()];
        assert_eq!(
            lookup_strong(&cache, &both, "G25").expect("G25").module_id,
            "small"
        );
        assert_eq!(
            lookup_strong(&cache, &both, "H430")
                .expect("H430")
                .module_id,
            "full"
        );
        // G26 é RVF: converte com o marcador de conteúdo degradado.
        assert!(lookup_strong(&cache, &both, "G26").expect("RVF").degraded);
        assert_eq!(
            lookup_strong(&cache, &[second, first], "G25")
                .expect("G25")
                .module_id,
            "full"
        );
    }

    #[test]
    fn effective_lists_follow_defaults_and_saved_preferences() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("any.twm");
        let catalog = vec![
            module("dict-en", &path, ModuleKind::Dictionary, "en"),
            module("dict-pt", &path, ModuleKind::Dictionary, "pt"),
            module("cmt-a", &path, ModuleKind::Commentary, "pt"),
            module("cmt-b", &path, ModuleKind::Commentary, "en"),
            {
                let mut locked = module("cmt-locked", &path, ModuleKind::Commentary, "pt");
                locked.state = ModuleState::Protected;
                locked
            },
            module("bible", &path, ModuleKind::Bible, "pt"),
        ];
        let ids = |modules: Vec<ModuleInfo>| modules.into_iter().map(|m| m.id).collect::<Vec<_>>();
        assert_eq!(
            ids(effective_lexicons(None, &catalog)),
            ["dict-pt", "dict-en"]
        );
        let saved = [
            "dict-en".to_string(),
            "cmt-a".to_string(),
            "gone".to_string(),
        ];
        assert_eq!(
            ids(effective_lexicons(Some(saved.as_slice()), &catalog)),
            ["dict-en"]
        );
        assert_eq!(
            ids(effective_commentaries(None, &catalog)),
            ["cmt-a", "cmt-b"]
        );
        let saved = ["cmt-b".to_string(), "cmt-locked".to_string()];
        assert_eq!(
            ids(effective_commentaries(Some(saved.as_slice()), &catalog)),
            ["cmt-b"]
        );
        assert!(effective_commentaries(Some(&[]), &catalog).is_empty());
    }

    fn commentary_fixture(directory: &Path, paragraphs: usize) -> ModuleInfo {
        let path = directory.join("study.cmt.twm");
        generate_synthetic_study_commentary_twm(&path, paragraphs).expect("gerar comentário");
        module("cmt", &path, ModuleKind::Commentary, "pt")
    }

    #[test]
    fn commentary_index_lists_headers_specific_to_broad_without_content() {
        let directory = tempfile::tempdir().expect("tempdir");
        let commentary = commentary_fixture(directory.path(), 150);
        let cache = StudyCache::new(STUDY_CACHE_CAPACITY);
        let index = commentary_index(
            &cache,
            std::slice::from_ref(&commentary),
            43,
            3,
            16,
            "pt-BR",
        )
        .expect("índice");
        assert_eq!((index.book, index.chapter, index.verse), (43, 3, 16));
        let entries = &index.modules[0].entries;
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.topic_id)
                .collect::<Vec<_>>(),
            [40, 50, 70, 30, 60, 20, 10]
        );
        assert_eq!(
            entries.iter().map(|entry| entry.scope).collect::<Vec<_>>(),
            [
                "verse",
                "verse",
                "verse",
                "range",
                "chapter",
                "chapterIntro",
                "bookIntro"
            ]
        );
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            [
                "Versículo 16",
                "Versículo 16",
                "Versículo 16",
                "Versículos 15–17",
                "Capítulo 3",
                "Introdução ao capítulo 3",
                "Introdução ao livro"
            ]
        );
        assert!(entries[0].size_bytes.unwrap_or_default() > 1_000);
        assert!(entries[0].supported && entries[1].supported);
        assert_eq!(entries[1].content_type, "rvf");
        let encoded = json(&index).to_string();
        assert!(!encoded.contains("nodes") && !encoded.contains("Synthetic"));

        let english = commentary_index(&cache, std::slice::from_ref(&commentary), 43, 3, 16, "en")
            .expect("índice em inglês");
        assert_eq!(english.modules[0].entries[3].label, "Verses 15–17");
        assert_eq!(english.modules[0].entries[6].label, "Book introduction");
        // Um versículo fora de qualquer faixa específica só tem as introduções e a faixa do capítulo.
        let other = commentary_index(&cache, std::slice::from_ref(&commentary), 43, 3, 30, "pt")
            .expect("outro versículo");
        assert_eq!(
            other.modules[0]
                .entries
                .iter()
                .map(|e| e.scope)
                .collect::<Vec<_>>(),
            ["chapter", "chapterIntro", "bookIntro"]
        );
        let none = commentary_index(&cache, std::slice::from_ref(&commentary), 40, 1, 1, "pt")
            .expect("livro sem comentário");
        assert_eq!(none.modules[0].entries.len(), 0);
        assert!(none.modules[0].error.is_none());
        assert_eq!(cache.commentaries.open_count(), 1);
    }

    #[test]
    fn commentary_index_isolates_module_errors_and_validates_the_reference() {
        let directory = tempfile::tempdir().expect("tempdir");
        let good = commentary_fixture(directory.path(), 3);
        let mut locked = good.clone();
        locked.id = "locked".into();
        locked.path = directory.path().join("never-opened.cmt.twm");
        locked.state = ModuleState::Protected;
        let mut broken = good.clone();
        broken.id = "broken".into();
        broken.path = directory.path().join("missing.cmt.twm");
        let cache = StudyCache::new(STUDY_CACHE_CAPACITY);
        let index =
            commentary_index(&cache, &[locked, broken, good], 43, 3, 16, "pt").expect("índice");
        let codes = index
            .modules
            .iter()
            .map(|module| module.error.as_ref().map(|error| error.code))
            .collect::<Vec<_>>();
        assert_eq!(
            codes,
            [Some("module_protected"), Some("module_not_found"), None]
        );
        assert!(index.modules[2].entries.len() > 3);
        assert_eq!(
            cache.commentaries.open_count(),
            1,
            "o protegido nunca é aberto"
        );

        assert_eq!(
            commentary_index(&cache, &[], 43, 3, 99, "pt")
                .expect_err("versículo")
                .code,
            "invalid_reference"
        );
        assert_eq!(
            commentary_index(&cache, &[], 99, 1, 1, "pt")
                .expect_err("livro")
                .code,
            "invalid_reference"
        );
        assert!(
            commentary_index(&cache, &[], 43, 3, 16, "pt")
                .expect("vazio")
                .modules
                .is_empty()
        );
    }

    #[test]
    fn commentary_entry_pages_on_demand_and_isolates_unsupported_entries() {
        let directory = tempfile::tempdir().expect("tempdir");
        let commentary = commentary_fixture(directory.path(), 150);
        let cache = StudyCache::new(STUDY_CACHE_CAPACITY);
        let first = commentary_entry(&cache, &commentary, 40, 0).expect("página 1");
        assert_eq!(paragraphs(&first.nodes), COMMENTARY_PAGE_PARAGRAPHS);
        assert!(first.has_more);
        assert_eq!((first.offset, first.next_offset), (0, 60 << 32));
        let second =
            commentary_entry(&cache, &commentary, 40, first.next_offset).expect("página 2");
        let third =
            commentary_entry(&cache, &commentary, 40, second.next_offset).expect("página 3");
        assert_eq!(paragraphs(&second.nodes), 60);
        assert_eq!(paragraphs(&third.nodes), 30);
        assert!(second.has_more && !third.has_more);
        let encoded = json(&first.nodes).to_string();
        for fragment in ["bold", "italic", "underline", "superscript"] {
            assert!(
                encoded.contains(&format!("\"type\":\"{fragment}\"")),
                "{fragment}"
            );
        }
        assert!(encoded.contains("Synthetic paragraph 1 ") && !encoded.contains("paragraph 61 "));

        // A entrada RVF sintética é degradada, mas não afeta as demais.
        assert!(
            commentary_entry(&cache, &commentary, 50, 0)
                .expect("RVF")
                .degraded
        );
        assert!(commentary_entry(&cache, &commentary, 10, 0).is_ok());
        assert_eq!(
            commentary_entry(&cache, &commentary, 999, 0)
                .expect_err("tópico")
                .code,
            "entry_not_found"
        );
        let links = json(
            &commentary_entry(&cache, &commentary, 70, 0)
                .expect("links")
                .nodes,
        )
        .to_string();
        for fragment in ["verseLink", "entryLink", "\"hint\":\"person\""] {
            assert!(links.contains(fragment), "{fragment}: {links}");
        }

        let mut locked = commentary.clone();
        locked.state = ModuleState::Protected;
        assert_eq!(
            commentary_entry(&cache, &locked, 40, 0)
                .expect_err("protegido")
                .code,
            "module_protected"
        );
        let mut wrong = commentary;
        wrong.kind = ModuleKind::Dictionary;
        assert_eq!(
            commentary_entry(&cache, &wrong, 40, 0)
                .expect_err("tipo")
                .code,
            "module_unsupported"
        );
    }

    #[test]
    fn caches_are_limited_and_reuse_open_readers() {
        let directory = tempfile::tempdir().expect("tempdir");
        let cache = StudyCache::new(STUDY_CACHE_CAPACITY);
        let mut modules = Vec::new();
        for index in 0..8 {
            let path = dictionary_fixture(directory.path(), &format!("lexicon-{index}"));
            modules.push(module(
                &format!("lex-{index}"),
                &path,
                ModuleKind::Dictionary,
                "pt",
            ));
        }
        for lexicon in &modules {
            lookup_strong(&cache, std::slice::from_ref(lexicon), "G25").expect("G25");
        }
        assert_eq!(cache.lexicons.len(), STUDY_CACHE_CAPACITY);
        assert_eq!(cache.lexicons.open_count(), 8);
        let recent = &modules[7];
        lookup_strong(&cache, std::slice::from_ref(recent), "G25").expect("G25");
        assert_eq!(
            cache.lexicons.open_count(),
            8,
            "o mais recente continua aberto"
        );
        lookup_strong(&cache, std::slice::from_ref(&modules[0]), "G25").expect("G25");
        assert_eq!(
            cache.lexicons.open_count(),
            9,
            "o mais antigo foi descartado"
        );
    }

    fn find_module(folder: &Path, wanted: &str) -> Option<PathBuf> {
        std::fs::read_dir(folder)
            .ok()?
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().to_lowercase() == wanted)
            })
    }

    /// Só roda com RHEMA_TEST_MODULES. Imprime apenas números e tempos, nunca texto de módulos.
    #[test]
    fn real_lexicon_lookup_and_commentary_timings() {
        let Ok(folder) = std::env::var("RHEMA_TEST_MODULES") else {
            return;
        };
        let folder = PathBuf::from(folder);
        let library_dir = tempfile::tempdir().expect("tempdir");
        let scan = rhema_library::Library::new(library_dir.path())
            .scan(std::slice::from_ref(&folder), |_| {})
            .expect("varredura");
        let catalog = scan.modules;
        let strong = find_module(&folder, "strong_amg2.dct.twm").expect("Strong_AMG2 ausente");
        let lexicons = effective_lexicons(None, &catalog)
            .into_iter()
            .filter(|module| module.path == strong)
            .collect::<Vec<_>>();
        assert_eq!(lexicons.len(), 1, "Strong_AMG2 deve estar pronto");
        let cache = StudyCache::new(STUDY_CACHE_CAPACITY);
        for (position, key) in ["G25", "G26", "G3056", "H430"].into_iter().enumerate() {
            let started = Instant::now();
            let result = lookup_strong(&cache, &lexicons, key);
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            let outcome = match &result {
                Ok(entry) => format!("found nodes={}", entry.nodes.len()),
                Err(error) => format!("error={}", error.code),
            };
            eprintln!(
                "lookup {key}: {outcome} ms={elapsed:.3} cold={}",
                position == 0
            );
            assert!(
                cfg!(debug_assertions) || elapsed < 100.0,
                "{key}: {elapsed:.1} ms"
            );
        }

        let protected = catalog
            .iter()
            .filter(|module| module.kind == ModuleKind::Dictionary)
            .filter(|module| module.state == ModuleState::Protected)
            .count();
        let usable = effective_lexicons(None, &catalog);
        assert!(
            usable
                .iter()
                .all(|module| module.state == ModuleState::Ready)
        );
        eprintln!(
            "lexicons ready={} protected_skipped={protected}",
            usable.len()
        );

        let active = effective_commentaries(None, &catalog);
        eprintln!("active commentaries={}", active.len());
        for module in &active {
            let started = Instant::now();
            let source = CommentarySource::open(&module.path).expect("abrir comentário");
            let open_ms = started.elapsed().as_secs_f64() * 1000.0;
            let started = Instant::now();
            let headers = source
                .headers_for(VerseRef::new(Book::Genesis, 1, 1))
                .expect("cabeçalhos");
            let headers_ms = started.elapsed().as_secs_f64() * 1000.0;
            eprintln!(
                "breakdown {}: open_ms={open_ms:.1} headers_ms={headers_ms:.2} headers={}",
                module
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                headers.len()
            );
        }
        let mut biggest: Option<(ModuleInfo, CommentaryHeaderDto)> = None;
        for (book, chapter, verse, label) in [(43, 3, 16, "Jo 3:16"), (1, 1, 1, "Gn 1:1")] {
            let cold = StudyCache::new(STUDY_CACHE_CAPACITY);
            let started = Instant::now();
            let index =
                commentary_index(&cold, &active, book, chapter, verse, "pt").expect("índice");
            let cold_ms = started.elapsed().as_secs_f64() * 1000.0;
            let started = Instant::now();
            let warm =
                commentary_index(&cold, &active, book, chapter, verse, "pt").expect("índice");
            let warm_ms = started.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(warm, index);
            let counts = index
                .modules
                .iter()
                .map(|module| {
                    let file = active
                        .iter()
                        .find(|candidate| candidate.id == module.module_id)
                        .and_then(|candidate| candidate.path.file_name())
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let unsupported = module
                        .entries
                        .iter()
                        .filter(|entry| !entry.supported)
                        .count();
                    format!(
                        "{file}: headers={} unsupported={unsupported}",
                        module.entries.len()
                    )
                })
                .collect::<Vec<_>>();
            eprintln!(
                "index {label}: per_module={counts:?} cold_ms={cold_ms:.1} warm_ms={warm_ms:.3}"
            );
            // Os orçamentos valem para o build de release; em debug só registramos os tempos.
            assert!(
                cfg!(debug_assertions) || cold_ms < 100.0,
                "{label}: índice frio {cold_ms:.1} ms"
            );
            for dto in &index.modules {
                for entry in dto.entries.iter().filter(|entry| entry.supported) {
                    let bigger = biggest.as_ref().is_none_or(|(_, current)| {
                        entry.size_bytes.unwrap_or(0) > current.size_bytes.unwrap_or(0)
                    });
                    if bigger {
                        let source = active
                            .iter()
                            .find(|module| module.id == dto.module_id)
                            .cloned()
                            .expect("módulo do índice");
                        biggest = Some((source, entry.clone()));
                    }
                }
            }
        }
        if let Some((source, header)) = biggest {
            let cold = StudyCache::new(STUDY_CACHE_CAPACITY);
            let started = Instant::now();
            let page =
                commentary_entry(&cold, &source, header.topic_id, 0).expect("primeira página");
            let cold_ms = started.elapsed().as_secs_f64() * 1000.0;
            let started = Instant::now();
            let again =
                commentary_entry(&cold, &source, header.topic_id, 0).expect("primeira página");
            let warm_ms = started.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(again, page);
            eprintln!(
                "first page of largest entry: size_kb={} paragraphs={} has_more={} cold_ms={cold_ms:.1} warm_ms={warm_ms:.1}",
                header.size_bytes.unwrap_or(0) / 1024,
                paragraphs(&page.nodes),
                page.has_more
            );
            assert!(
                cfg!(debug_assertions) || cold_ms < 200.0,
                "primeira página {cold_ms:.1} ms"
            );
        }
    }
}
