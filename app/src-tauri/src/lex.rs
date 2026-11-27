use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rhema_core::ModuleKind;
use rhema_lexicon::{
    BookOccurrenceCount, LexKind, LexiconError, LexiconIndex, MatchField, MorphDecodeResult,
    MorphDecoder, OccurrenceBibleInfo, OccurrencesIndex, SearchFilters, SearchResults,
    TranslationCount, VerseOccurrence, build_index, build_links, build_reverse_and_occurrences,
    validate_strong_number,
};
use rhema_library::{ModuleInfo, ModuleState};
use serde::Serialize;

use crate::reader::ApiError;
use crate::study::{self, LexiconEntryDto, StudyCache};

pub const LEX_PROGRESS_EVENT: &str = "lex-index-progress";
const MAX_QUERY_CHARS: usize = 200;
const MAX_LIMIT: usize = 100;

#[derive(Debug, Clone, Default)]
struct BuildProgressState {
    stage: Option<String>,
    module_index: usize,
    module_count: usize,
    module_id: Option<String>,
    entries_processed: usize,
    percent: u8,
    last_error: Option<String>,
}

pub struct LexState {
    data_dir: PathBuf,
    building: Mutex<bool>,
    progress: Mutex<BuildProgressState>,
    index: Mutex<Option<Arc<LexiconIndex>>>,
    occurrences: Mutex<Option<OccurrencesIndex>>,
    morph: Mutex<Option<MorphDecoder>>,
}

impl LexState {
    pub fn new(app_data: &Path) -> Self {
        Self {
            data_dir: app_data.join("lexicon"),
            building: Mutex::new(false),
            progress: Mutex::new(BuildProgressState::default()),
            index: Mutex::new(None),
            occurrences: Mutex::new(None),
            morph: Mutex::new(None),
        }
    }

    fn links_path(&self) -> PathBuf {
        self.data_dir.join("links.sqlite")
    }

    fn index_dir(&self) -> PathBuf {
        self.data_dir.join("index")
    }

    fn reverse_path(&self) -> PathBuf {
        self.data_dir.join("reverse.sqlite")
    }

    fn occurrences_path(&self) -> PathBuf {
        self.data_dir.join("occurrences.sqlite")
    }

    fn is_building(&self) -> bool {
        self.building.lock().map(|g| *g).unwrap_or(false)
    }

    fn cached_or_open(&self) -> Result<Arc<LexiconIndex>, LexiconError> {
        if let Ok(guard) = self.index.lock()
            && let Some(existing) = guard.as_ref()
        {
            return Ok(Arc::clone(existing));
        }
        let mut idx = LexiconIndex::open(self.index_dir())?;
        if self.links_path().exists() {
            idx = idx.with_links(self.links_path());
        }
        if self.reverse_path().exists() {
            idx = idx.with_reverse(self.reverse_path());
        }
        let arc = Arc::new(idx);
        if let Ok(mut guard) = self.index.lock() {
            *guard = Some(Arc::clone(&arc));
        }
        Ok(arc)
    }

    fn with_occurrences<T>(
        &self,
        f: impl FnOnce(&OccurrencesIndex) -> Result<T, LexiconError>,
    ) -> Result<T, LexiconError> {
        let mut guard = self
            .occurrences
            .lock()
            .map_err(|_| LexiconError::Invalid("estado de ocorrências indisponível".into()))?;
        if guard.is_none() {
            *guard = Some(OccurrencesIndex::open(self.occurrences_path())?);
        }
        let idx = guard
            .as_ref()
            .ok_or_else(|| LexiconError::Invalid("índice de ocorrências indisponível".into()))?;
        f(idx)
    }

    pub fn invalidate_cache(&self) {
        if let Ok(mut guard) = self.index.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.occurrences.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.morph.lock() {
            *guard = None;
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LexIndexStatusDto {
    pub state: String,
    pub stage: Option<String>,
    pub module_index: usize,
    pub module_count: usize,
    pub percent: u8,
    pub module_id: Option<String>,
    pub entries_processed: usize,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LexProgressEvent {
    pub state: String,
    pub stage: String,
    pub module_index: usize,
    pub module_count: usize,
    pub percent: u8,
    pub module_id: Option<String>,
    pub entries_processed: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LexSearchHitDto {
    pub module_id: String,
    pub module_title: String,
    pub kind: String,
    pub key: String,
    pub topic_id: Option<i64>,
    pub field: String,
    pub score: f32,
    pub provenance: Option<String>,
    pub dice: Option<f32>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LexSearchGroupDto {
    pub label: String,
    pub module_id: String,
    pub kind: String,
    pub is_suggestions: bool,
    pub is_linked: bool,
    pub hits: Vec<LexSearchHitDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LexRelatedItemDto {
    pub module_id: String,
    pub module_title: String,
    pub kind: String,
    pub key: String,
    pub topic_id: Option<i64>,
    pub title: String,
    pub reason: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LexRelatedDto {
    pub primary: LexiconEntryDto,
    pub related: Vec<LexRelatedItemDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LexSearchDto {
    pub query: String,
    pub total: usize,
    pub shown: usize,
    pub interpretations: Vec<String>,
    pub groups: Vec<LexSearchGroupDto>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OccurrenceBibleDto {
    pub module_id: String,
    pub title: String,
    pub abbreviation: Option<String>,
    pub lang: String,
    pub total_verses: u64,
    pub total_occurrences: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StrongOccurrencesDto {
    pub strong: String,
    pub selected_module_id: Option<String>,
    pub bibles: Vec<OccurrenceBibleDto>,
    pub total_verses: u64,
    pub total_occurrences: u64,
    pub by_book: Vec<BookOccurrenceCount>,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
    pub items: Vec<VerseOccurrence>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StrongTranslationsDto {
    pub strong: String,
    pub module_id: Option<String>,
    pub total: u64,
    pub items: Vec<TranslationCount>,
}

fn ready_dictionaries(catalog: &[ModuleInfo]) -> Vec<&ModuleInfo> {
    catalog
        .iter()
        .filter(|m| m.kind == ModuleKind::Dictionary && m.state == ModuleState::Ready)
        .collect()
}

pub fn collect_scan_folders(catalog: &[ModuleInfo], configured: &[PathBuf]) -> Vec<PathBuf> {
    let mut folders: BTreeSet<PathBuf> =
        configured.iter().filter(|p| p.is_dir()).cloned().collect();
    if folders.is_empty() {
        for m in catalog.iter().filter(|m| m.state == ModuleState::Ready) {
            if let Some(parent) = m.path.parent()
                && parent.is_dir()
            {
                folders.insert(parent.to_path_buf());
            }
        }
    }
    folders.into_iter().collect()
}

pub fn index_status(state: &LexState, catalog: &[ModuleInfo]) -> LexIndexStatusDto {
    if state.is_building() {
        let p = state.progress.lock().map(|g| g.clone()).unwrap_or_default();
        return LexIndexStatusDto {
            state: "building".into(),
            stage: p.stage,
            module_index: p.module_index,
            module_count: p.module_count,
            percent: p.percent,
            module_id: p.module_id,
            entries_processed: p.entries_processed,
            message: None,
        };
    }

    if let Some(err) = state
        .progress
        .lock()
        .ok()
        .and_then(|g| g.last_error.clone())
    {
        return LexIndexStatusDto {
            state: "error".into(),
            stage: None,
            module_index: 0,
            module_count: 0,
            percent: 0,
            module_id: None,
            entries_processed: 0,
            message: Some(err),
        };
    }

    if ready_dictionaries(catalog).is_empty() {
        return LexIndexStatusDto {
            state: "empty".into(),
            stage: None,
            module_index: 0,
            module_count: 0,
            percent: 0,
            module_id: None,
            entries_processed: 0,
            message: Some("Nenhum léxico pronto na biblioteca.".into()),
        };
    }

    match state.cached_or_open() {
        Ok(idx) => {
            let status = idx.status();
            if status.rebuild_needed {
                LexIndexStatusDto {
                    state: "rebuild_needed".into(),
                    stage: None,
                    module_index: 0,
                    module_count: 0,
                    percent: 0,
                    module_id: None,
                    entries_processed: 0,
                    message: status.message(),
                }
            } else if catalog
                .iter()
                .any(|m| m.kind == ModuleKind::Bible && m.state == ModuleState::Ready)
                && state.with_occurrences(|_| Ok(())).is_err()
            {
                LexIndexStatusDto {
                    state: "rebuild_needed".into(),
                    stage: None,
                    module_index: 0,
                    module_count: 0,
                    percent: 0,
                    module_id: None,
                    entries_processed: 0,
                    message: Some("Índice de ocorrências precisa ser construído.".into()),
                }
            } else {
                LexIndexStatusDto {
                    state: "ready".into(),
                    stage: None,
                    module_index: 0,
                    module_count: 0,
                    percent: 100,
                    module_id: None,
                    entries_processed: 0,
                    message: None,
                }
            }
        }
        Err(err) => LexIndexStatusDto {
            state: "rebuild_needed".into(),
            stage: None,
            module_index: 0,
            module_count: 0,
            percent: 0,
            module_id: None,
            entries_processed: 0,
            message: Some(err.to_string()),
        },
    }
}

pub fn build_all_indexes<F>(
    state: &LexState,
    folders: &[PathBuf],
    mut emit: F,
) -> Result<LexIndexStatusDto, ApiError>
where
    F: FnMut(LexProgressEvent),
{
    {
        let mut building = state
            .building
            .lock()
            .map_err(|_| ApiError::internal("estado de construção bloqueado"))?;
        if *building {
            return Ok(LexIndexStatusDto {
                state: "building".into(),
                stage: state.progress.lock().ok().and_then(|g| g.stage.clone()),
                module_index: 0,
                module_count: 0,
                percent: state.progress.lock().map(|g| g.percent).unwrap_or(0),
                module_id: None,
                entries_processed: 0,
                message: None,
            });
        }
        *building = true;
    }

    let result = run_build(state, folders, &mut emit);

    if let Ok(mut building) = state.building.lock() {
        *building = false;
    }

    match result {
        Ok(()) => {
            if let Ok(mut p) = state.progress.lock() {
                *p = BuildProgressState {
                    percent: 100,
                    ..Default::default()
                };
            }
            emit(LexProgressEvent {
                state: "ready".into(),
                stage: "done".into(),
                module_index: 0,
                module_count: 0,
                percent: 100,
                module_id: None,
                entries_processed: 0,
            });
            Ok(LexIndexStatusDto {
                state: "ready".into(),
                stage: None,
                module_index: 0,
                module_count: 0,
                percent: 100,
                module_id: None,
                entries_processed: 0,
                message: None,
            })
        }
        Err(err) => {
            if let Ok(mut p) = state.progress.lock() {
                p.last_error = Some(err.message.clone());
            }
            Err(err)
        }
    }
}

fn update_progress<F>(state: &LexState, emit: &mut F, ev: LexProgressEvent)
where
    F: FnMut(LexProgressEvent),
{
    if let Ok(mut p) = state.progress.lock() {
        p.stage = Some(ev.stage.clone());
        p.module_index = ev.module_index;
        p.module_count = ev.module_count;
        p.percent = ev.percent;
        p.module_id = ev.module_id.clone();
        p.entries_processed = ev.entries_processed;
        p.last_error = None;
    }
    emit(ev);
}

fn run_build<F>(state: &LexState, folders: &[PathBuf], emit: &mut F) -> Result<(), ApiError>
where
    F: FnMut(LexProgressEvent),
{
    if folders.is_empty() {
        return Err(ApiError::new(
            "no_lexicon_installed",
            "Nenhuma pasta de módulos configurada.",
        ));
    }
    fs::create_dir_all(&state.data_dir)
        .map_err(|e| ApiError::internal(format!("Erro ao preparar pasta do índice: {e}")))?;

    let mk = |stage: &str, module_index, module_count, percent, module_id, entries_processed| {
        LexProgressEvent {
            state: "building".into(),
            stage: stage.into(),
            module_index,
            module_count,
            percent,
            module_id,
            entries_processed,
        }
    };

    // Libera conexões SQLite abertas antes de substituir arquivos (evita bloqueio no Windows)
    state.invalidate_cache();

    // Etapa 1: ligações (opcional se não houver módulo Strong)
    update_progress(state, emit, mk("links", 0, 1, 5, None, 0));
    let links_tmp = state.data_dir.join("links.sqlite.tmp");
    let _ = fs::remove_file(&links_tmp);
    match build_links(&links_tmp, folders) {
        Ok(_) => {
            fs::rename(&links_tmp, state.links_path())
                .map_err(|e| ApiError::internal(format!("Erro ao salvar ligações: {e}")))?;
        }
        Err(err) => {
            let _ = fs::remove_file(&links_tmp);
            log::info!("build_links ignorado: {err}");
        }
    }

    // Etapa 2: índice de léxicos (tantivy, troca atômica via CURRENT)
    update_progress(state, emit, mk("lexicon", 0, 1, 15, None, 0));
    build_index(state.index_dir(), folders, &mut |p| {
        let total = p.module_count.max(1);
        let pct = 15 + (((p.module_index * 65) / total) as u8).min(65);
        update_progress(
            state,
            emit,
            mk(
                "lexicon",
                p.module_index,
                p.module_count,
                pct,
                Some(p.module_id.to_string()),
                p.entries_processed,
            ),
        );
    })
    .map_err(|e| ApiError::new("lex_build_failed", e.to_string()))?;

    // Etapa 3: índice reverso + índice de ocorrências na mesma varredura de Bíblias
    update_progress(state, emit, mk("reverse", 0, 1, 85, None, 0));
    match build_reverse_and_occurrences(state.reverse_path(), state.occurrences_path(), folders) {
        Ok((rev_res, _occ_rep)) => {
            if let Err(err) = rev_res {
                log::info!("build_reverse_index ignorado: {err}");
            }
        }
        Err(err) => {
            log::info!("build_reverse_and_occurrences ignorado: {err}");
        }
    }

    // Reabre e atualiza os handles em cache
    state.invalidate_cache();
    state
        .cached_or_open()
        .map_err(|e| ApiError::internal(format!("Erro ao abrir índice construído: {e}")))?;
    let _ = state.with_occurrences(|_| Ok(()));
    Ok(())
}

pub fn search(
    state: &LexState,
    catalog: &[ModuleInfo],
    query: &str,
    limit: Option<usize>,
) -> Result<LexSearchDto, ApiError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err(ApiError::new("invalid_query", "Informe um termo de busca."));
    }
    if trimmed.chars().count() > MAX_QUERY_CHARS {
        return Err(ApiError::new(
            "invalid_query",
            "Consulta excede o limite de 200 caracteres.",
        ));
    }
    let limit = match limit {
        Some(n) if n == 0 || n > MAX_LIMIT => {
            return Err(ApiError::new(
                "invalid_limit",
                "O limite deve estar entre 1 e 100.",
            ));
        }
        Some(n) => n,
        None => 30,
    };

    if state.is_building() {
        return Err(ApiError::new(
            "lex_index_building",
            "O índice de léxicos está sendo construído.",
        ));
    }

    let index = state.cached_or_open().map_err(|e| match e {
        LexiconError::RebuildRequired(_) => ApiError::new(
            "lex_rebuild_required",
            "Índice de léxicos precisa ser reconstruído.",
        ),
        other => ApiError::internal(other.to_string()),
    })?;

    let results = index
        .search(trimmed, limit, &SearchFilters::default())
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(map_search_results(trimmed, catalog, results))
}

fn map_search_results(query: &str, catalog: &[ModuleInfo], results: SearchResults) -> LexSearchDto {
    let title_by_file: HashMap<String, &str> = catalog
        .iter()
        .filter_map(|m| {
            m.path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|name| (name.to_string(), m.title.as_str()))
        })
        .collect();
    let title_by_id: HashMap<&str, &str> = catalog
        .iter()
        .map(|m| (m.id.as_str(), m.title.as_str()))
        .collect();
    let resolve_title = |module_id: &str| -> String {
        title_by_id
            .get(module_id)
            .or_else(|| title_by_file.get(module_id))
            .copied()
            .unwrap_or(module_id)
            .to_string()
    };

    let groups = results
        .groups
        .into_iter()
        .map(|g| {
            let is_suggestions = g.module_id == "sugestões por palavra";
            let is_linked = g.module_id.starts_with("ligados a ");
            let label = if is_suggestions || is_linked {
                g.module_id.clone()
            } else {
                resolve_title(&g.module_id)
            };
            let hits = g
                .hits
                .into_iter()
                .map(|h| {
                    let dice = (h.field == MatchField::ReverseIndex).then(|| {
                        parse_dice(h.provenance.as_deref())
                            .unwrap_or_else(|| ((h.score - 150.0) / 40.0).clamp(0.0, 1.0))
                    });
                    LexSearchHitDto {
                        module_title: resolve_title(&h.module_id),
                        module_id: h.module_id,
                        kind: kind_label(h.kind).into(),
                        key: h.key,
                        topic_id: h.topic_id,
                        field: h.field.as_str().into(),
                        score: h.score,
                        provenance: h.provenance,
                        dice,
                    }
                })
                .collect();
            LexSearchGroupDto {
                label,
                module_id: g.module_id,
                kind: kind_label(g.kind).into(),
                is_suggestions,
                is_linked,
                hits,
            }
        })
        .collect();

    LexSearchDto {
        query: query.to_string(),
        total: results.total,
        shown: results.shown,
        interpretations: results
            .interpretations
            .iter()
            .map(|q| format!("{q:?}"))
            .collect(),
        groups,
        notes: results.notes,
    }
}

fn parse_dice(provenance: Option<&str>) -> Option<f32> {
    let text = provenance?;
    let start = text.find("Dice ")? + 5;
    let end = text[start..].find(')')? + start;
    text[start..end].trim().parse().ok()
}

fn kind_label(kind: LexKind) -> &'static str {
    kind.as_str()
}

pub fn entry(
    cache: &StudyCache,
    catalog: &[ModuleInfo],
    module_id: &str,
    key: &str,
    topic_id: Option<i64>,
) -> Result<LexiconEntryDto, ApiError> {
    let module_id = module_id.trim();
    if module_id.is_empty()
        || (topic_id.is_none() && key.trim().is_empty())
        || key.chars().count() > MAX_QUERY_CHARS
    {
        return Err(ApiError::new(
            "invalid_argument",
            "Identificador ou chave inválida.",
        ));
    }
    let module = catalog
        .iter()
        .find(|m| {
            m.id == module_id
                || m.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| name == module_id)
        })
        .ok_or_else(|| ApiError::new("module_not_found", "Módulo não encontrado na biblioteca."))?;
    study::lookup_entry(cache, module, key, topic_id)
}

fn plain_from_ui_nodes(nodes: &[crate::reader::UiNode]) -> String {
    let mut out = String::new();
    for node in nodes {
        match node {
            crate::reader::UiNode::Text { value } => out.push_str(value),
            crate::reader::UiNode::Italic { children }
            | crate::reader::UiNode::Bold { children }
            | crate::reader::UiNode::Underline { children }
            | crate::reader::UiNode::Strikethrough { children }
            | crate::reader::UiNode::Caps { children }
            | crate::reader::UiNode::Superscript { children }
            | crate::reader::UiNode::Subscript { children }
            | crate::reader::UiNode::WordsOfJesus { children }
            | crate::reader::UiNode::OtQuote { children }
            | crate::reader::UiNode::Note { children }
            | crate::reader::UiNode::Heading { children, .. }
            | crate::reader::UiNode::Link { children, .. }
            | crate::reader::UiNode::VerseLink { children, .. }
            | crate::reader::UiNode::EntryLink { children, .. }
            | crate::reader::UiNode::Strong { children, .. } => {
                out.push_str(&plain_from_ui_nodes(children));
            }
            crate::reader::UiNode::Paragraph | crate::reader::UiNode::LineBreak => out.push(' '),
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn related_for(
    lex_state: &LexState,
    cache: &StudyCache,
    catalog: &[ModuleInfo],
    active_lexicon_ids: Option<&[String]>,
    strong_id: Option<&str>,
    module_id: Option<&str>,
    key: Option<&str>,
    topic_id: Option<i64>,
) -> Result<LexRelatedDto, ApiError> {
    let active = study::effective_lexicons(active_lexicon_ids, catalog);
    let (primary, mut resolved_strong, mut lemma_hint) = if let Some(sid) = strong_id {
        let dto = study::lookup_strong(cache, &active, sid)?;
        let strong = dto.key.clone();
        let orig = dto.original_word.clone();
        (dto, Some(strong), orig)
    } else if let (Some(mid), Some(k)) = (module_id, key) {
        let dto = entry(cache, catalog, mid, k, topic_id)?;
        let parsed = rhema_lexicon::parse_query(&dto.key)
            .into_iter()
            .find_map(|q| match q {
                rhema_lexicon::Query::Strong(s) => Some(s),
                _ => None,
            });
        let orig = dto.original_word.clone();
        (dto, parsed, orig)
    } else {
        return Err(ApiError::new(
            "invalid_argument",
            "Informe strong_id ou (module_id, key).",
        ));
    };

    if resolved_strong.is_none() {
        let (lemmas, _) = rhema_lexicon::split_mixed_title(&primary.title);
        if lemma_hint.is_none() {
            lemma_hint = lemmas.first().cloned();
        }
        if let (Some(lemma), Ok(idx)) = (lemmas.first(), lex_state.cached_or_open())
            && let Ok(res) = idx.search(
                lemma,
                5,
                &SearchFilters {
                    modules: Vec::new(),
                    kinds: vec![LexKind::Strong],
                },
            )
            && let Some(hit) = res.groups.first().and_then(|g| g.hits.first())
        {
            resolved_strong = Some(hit.key.clone());
        }
        if resolved_strong.is_none() {
            let plain = plain_from_ui_nodes(&primary.nodes);
            resolved_strong = rhema_lexicon::extract_explicit_strong_refs(&plain)
                .into_iter()
                .find(|r| r.starts_with('G') || r.starts_with('H'));
        }
    }

    let mut related = Vec::new();
    if let (Some(strong), Ok(idx)) = (resolved_strong, lex_state.cached_or_open())
        && let Ok(hits) = idx.related_for_strong(&strong, lemma_hint.as_deref(), 50)
    {
        let active_by_file: HashMap<String, &ModuleInfo> = active
            .iter()
            .filter_map(|m| {
                m.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| (n.to_string(), m))
            })
            .collect();
        let active_by_id: HashMap<&str, &ModuleInfo> =
            active.iter().map(|m| (m.id.as_str(), m)).collect();

        for hit in hits {
            let Some(module) = active_by_id
                .get(hit.module_id.as_str())
                .or_else(|| active_by_file.get(&hit.module_id))
                .copied()
            else {
                continue;
            };
            if module.id == primary.module_id
                && (hit.key == primary.key || hit.key == primary.title)
            {
                continue;
            }
            related.push(LexRelatedItemDto {
                module_id: module.id.clone(),
                module_title: module.title.clone(),
                kind: hit.kind.as_str().into(),
                title: hit.key.clone(),
                key: hit.key,
                topic_id: hit.topic_id,
                reason: hit.reason,
                error: None,
            });
            if related.len() >= 50 {
                break;
            }
        }
    }

    if let Some(ids) = active_lexicon_ids {
        for id in ids {
            if related.len() >= 50 {
                break;
            }
            if let Some(m) = catalog.iter().find(|m| {
                m.id == *id && m.kind == ModuleKind::Dictionary && m.state != ModuleState::Ready
            }) {
                let msg = match &m.state {
                    ModuleState::Protected => "Módulo protegido".to_string(),
                    ModuleState::UnsupportedFormat { message }
                    | ModuleState::Corrupted { message } => message.clone(),
                    ModuleState::Ready => continue,
                };
                related.push(LexRelatedItemDto {
                    module_id: m.id.clone(),
                    module_title: m.title.clone(),
                    kind: "dictionary".into(),
                    key: primary.key.clone(),
                    topic_id: None,
                    title: primary.title.clone(),
                    reason: "indisponível".into(),
                    error: Some(msg),
                });
            }
        }
    }

    Ok(LexRelatedDto { primary, related })
}

fn find_catalog_bible<'a>(
    catalog: &'a [ModuleInfo],
    info: &OccurrenceBibleInfo,
) -> Option<&'a ModuleInfo> {
    let info_canon = fs::canonicalize(&info.path).ok();
    let info_name = Path::new(&info.path)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase());
    catalog.iter().find(|m| {
        if m.kind != ModuleKind::Bible {
            return false;
        }
        if m.id == info.module_id {
            return true;
        }
        if let (Some(a), Ok(b)) = (info_canon.as_ref(), fs::canonicalize(&m.path))
            && *a == b
        {
            return true;
        }
        info_name.as_deref().is_some_and(|name| {
            m.path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().to_lowercase() == name)
        })
    })
}

fn resolve_db_module_id(
    catalog: &[ModuleInfo],
    bibles: &[OccurrenceBibleInfo],
    requested: Option<&str>,
) -> Option<String> {
    let req = requested?.trim();
    if req.is_empty() {
        return None;
    }
    if let Some(b) = bibles.iter().find(|b| b.module_id == req) {
        return Some(b.module_id.clone());
    }
    if let Some(cat_mod) = catalog.iter().find(|m| m.id == req) {
        let cat_canon = fs::canonicalize(&cat_mod.path).ok();
        let cat_name = cat_mod
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase());
        if let Some(b) = bibles.iter().find(|b| {
            if let (Some(a), Ok(bp)) = (cat_canon.as_ref(), fs::canonicalize(&b.path))
                && *a == bp
            {
                return true;
            }
            cat_name.as_deref().is_some_and(|name| {
                Path::new(&b.path)
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().to_lowercase() == name)
            })
        }) {
            return Some(b.module_id.clone());
        }
    }
    Some(req.to_string())
}

pub fn strong_occurrences(
    state: &LexState,
    catalog: &[ModuleInfo],
    strong: &str,
    module_id: Option<&str>,
    page: Option<u32>,
    page_size: Option<u32>,
) -> Result<StrongOccurrencesDto, ApiError> {
    let strong = validate_strong_number(strong)
        .map_err(|e| ApiError::new("invalid_strong", e.to_string()))?;
    let page = page.unwrap_or(1);
    if !(1..=200).contains(&page) {
        return Err(ApiError::new(
            "invalid_page",
            "A página deve estar entre 1 e 200.",
        ));
    }
    let page_size = page_size.unwrap_or(50).clamp(1, 200);
    if state.is_building() {
        return Err(ApiError::new(
            "lex_index_building",
            "O índice está sendo construído.",
        ));
    }

    let result = state
        .with_occurrences(|idx| {
            let bibles = idx.bibles_for_strong(&strong)?;
            let db_mid = resolve_db_module_id(catalog, &bibles, module_id);
            idx.occurrences(&strong, db_mid.as_deref(), page, page_size)
        })
        .map_err(|e| match e {
            LexiconError::RebuildRequired(_) => ApiError::new(
                "lex_rebuild_required",
                "O índice de ocorrências precisa ser reconstruído.",
            ),
            LexiconError::Invalid(msg) => ApiError::new("invalid_argument", msg),
            other => ApiError::internal(other.to_string()),
        })?;

    let mut id_map: HashMap<String, String> = HashMap::new();
    let bibles: Vec<OccurrenceBibleDto> = result
        .bibles
        .iter()
        .map(|b| {
            let cat = find_catalog_bible(catalog, b);
            let mapped_id = cat
                .map(|m| m.id.clone())
                .unwrap_or_else(|| b.module_id.clone());
            id_map.insert(b.module_id.clone(), mapped_id.clone());
            OccurrenceBibleDto {
                module_id: mapped_id,
                title: cat
                    .map(|m| m.title.clone())
                    .unwrap_or_else(|| b.title.clone()),
                abbreviation: cat
                    .and_then(|m| m.abbreviation.clone())
                    .or_else(|| b.abbreviation.clone()),
                lang: b.lang.clone(),
                total_verses: b.total_verses,
                total_occurrences: b.total_occurrences,
            }
        })
        .collect();

    let selected_module_id = result
        .selected_module_id
        .map(|db_id| id_map.get(&db_id).cloned().unwrap_or(db_id));

    Ok(StrongOccurrencesDto {
        strong: result.strong,
        selected_module_id,
        bibles,
        total_verses: result.total_verses,
        total_occurrences: result.total_occurrences,
        by_book: result.by_book,
        page: result.page,
        page_size: result.page_size,
        total_pages: result.total_pages,
        items: result.items,
    })
}

pub fn strong_translations(
    state: &LexState,
    catalog: &[ModuleInfo],
    strong: &str,
    module_id: Option<&str>,
    limit: Option<usize>,
) -> Result<StrongTranslationsDto, ApiError> {
    let strong = validate_strong_number(strong)
        .map_err(|e| ApiError::new("invalid_strong", e.to_string()))?;
    let limit = match limit {
        Some(n) if n == 0 || n > MAX_LIMIT => {
            return Err(ApiError::new(
                "invalid_limit",
                "O limite deve estar entre 1 e 100.",
            ));
        }
        Some(n) => n,
        None => 15,
    };
    if state.is_building() {
        return Err(ApiError::new(
            "lex_index_building",
            "O índice está sendo construído.",
        ));
    }

    let result = state
        .with_occurrences(|idx| {
            let db_mid = if module_id.is_some() {
                let bibles = idx.bibles_for_strong(&strong)?;
                resolve_db_module_id(catalog, &bibles, module_id)
            } else {
                None
            };
            idx.translations(&strong, db_mid.as_deref(), limit)
        })
        .map_err(|e| match e {
            LexiconError::RebuildRequired(_) => ApiError::new(
                "lex_rebuild_required",
                "O índice de ocorrências precisa ser reconstruído.",
            ),
            LexiconError::Invalid(msg) => ApiError::new("invalid_argument", msg),
            other => ApiError::internal(other.to_string()),
        })?;

    Ok(StrongTranslationsDto {
        strong: result.strong,
        module_id: module_id.map(str::to_string),
        total: result.total,
        items: result.items,
    })
}

pub fn morph_decode(
    state: &LexState,
    catalog: &[ModuleInfo],
    code: &str,
    lang: Option<&str>,
) -> Result<MorphDecodeResult, ApiError> {
    let mut guard = state
        .morph
        .lock()
        .map_err(|_| ApiError::internal("cache de morfologia indisponível"))?;
    if guard.is_none() {
        let dict_paths: Vec<PathBuf> = catalog
            .iter()
            .filter(|m| m.kind == ModuleKind::Dictionary && m.state == ModuleState::Ready)
            .map(|m| m.path.clone())
            .collect();
        *guard = Some(MorphDecoder::from_paths(&dict_paths));
    }
    let decoder = guard
        .as_ref()
        .ok_or_else(|| ApiError::internal("decodificador de morfologia indisponível"))?;
    decoder
        .decode(code, lang)
        .map_err(|e| ApiError::new("invalid_argument", e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gen_fixtures::{generate_synthetic_protected_twm, generate_synthetic_study_dictionary_twm};
    use tempfile::tempdir;

    fn info_for(id: &str, path: PathBuf, title: &str, state: ModuleState) -> ModuleInfo {
        ModuleInfo {
            id: id.into(),
            path,
            kind: ModuleKind::Dictionary,
            state,
            title: title.into(),
            abbreviation: None,
            language: Some("pt".into()),
            language_name: None,
            description: None,
            version: None,
            size: 1,
            has_strong: false,
        }
    }

    #[test]
    fn builds_searches_and_reads_entries_across_catalog() {
        let dir = tempdir().unwrap();
        let mods = dir.path().join("mods");
        fs::create_dir(&mods).unwrap();
        let p_dict = mods.join("strong_amg.dct.twm");
        let p_prot = mods.join("protegido.dct.twm");
        generate_synthetic_study_dictionary_twm(&p_dict).unwrap();
        generate_synthetic_protected_twm(&p_prot, "do.encrypt").unwrap();

        let catalog = vec![
            info_for(
                "id-dict",
                p_dict,
                "Dicionario Sintetico",
                ModuleState::Ready,
            ),
            info_for("id-prot", p_prot, "Protegido", ModuleState::Protected),
        ];

        let state = LexState::new(&dir.path().join("appdata"));
        assert_eq!(index_status(&state, &catalog).state, "rebuild_needed");

        let mut events = Vec::new();
        let status = build_all_indexes(&state, std::slice::from_ref(&mods), &mut |ev| {
            events.push(ev);
        })
        .unwrap();
        assert_eq!(status.state, "ready");
        assert!(events.iter().any(|e| e.stage == "lexicon"));
        assert_eq!(index_status(&state, &catalog).state, "ready");

        let res = search(&state, &catalog, "G25", Some(10)).unwrap();
        assert!(res.total >= 1);
        assert_eq!(res.groups[0].label, "Dicionario Sintetico");
        assert_eq!(res.groups[0].hits[0].key, "G25");

        let cache = StudyCache::new(4);
        let hit = &res.groups[0].hits[0];
        let loaded = entry(&cache, &catalog, &hit.module_id, &hit.key, hit.topic_id).unwrap();
        assert_eq!(loaded.module_id, "id-dict");
        assert!(!loaded.degraded);
        assert!(!loaded.nodes.is_empty());

        assert_eq!(
            entry(&cache, &catalog, "id-prot", "G25", None)
                .unwrap_err()
                .code,
            "module_protected"
        );
        assert_eq!(
            entry(&cache, &catalog, "inexistente", "G25", None)
                .unwrap_err()
                .code,
            "module_not_found"
        );
        assert_eq!(
            entry(&cache, &catalog, "id-dict", "INEXISTENTE", None)
                .unwrap_err()
                .code,
            "entry_not_found"
        );
    }

    #[test]
    fn related_returns_primary_and_orders_related_entries_with_reasons() {
        let dir = tempdir().unwrap();
        let mods = dir.path().join("mods");
        fs::create_dir(&mods).unwrap();
        let p_strong = mods.join("strong_amg.dct.twm");
        let p_strong2 = mods.join("strong_amg_2.dct.twm");
        let p_prot = mods.join("protegido.dct.twm");
        generate_synthetic_study_dictionary_twm(&p_strong).unwrap();
        generate_synthetic_study_dictionary_twm(&p_strong2).unwrap();
        generate_synthetic_protected_twm(&p_prot, "do.encrypt").unwrap();

        let catalog = vec![
            info_for(
                "id-strong",
                p_strong,
                "Strong Sintetico",
                ModuleState::Ready,
            ),
            info_for(
                "id-strong2",
                p_strong2,
                "Strong Secundario",
                ModuleState::Ready,
            ),
            info_for("id-prot", p_prot, "Protegido", ModuleState::Protected),
        ];

        let state = LexState::new(&dir.path().join("appdata"));
        build_all_indexes(&state, std::slice::from_ref(&mods), &mut |_| {}).unwrap();
        let cache = StudyCache::new(4);

        let active_ids = vec![
            "id-strong".to_string(),
            "id-strong2".to_string(),
            "id-prot".to_string(),
        ];
        let dto = related_for(
            &state,
            &cache,
            &catalog,
            Some(&active_ids),
            Some("G25"),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(dto.primary.module_id, "id-strong");
        assert_eq!(dto.related.len(), 2);
        assert_eq!(dto.related[0].module_id, "id-strong2");
        assert_eq!(dto.related[0].reason, "chave Strong");
        assert!(dto.related[0].error.is_none());
        assert_eq!(dto.related[1].module_id, "id-prot");
        assert!(dto.related[1].error.is_some());
    }

    #[test]
    fn validates_query_limit_and_keeps_no_partial_index_on_failure() {
        let dir = tempdir().unwrap();
        let state = LexState::new(&dir.path().join("appdata"));
        let catalog = Vec::new();

        assert_eq!(
            search(&state, &catalog, "", None).unwrap_err().code,
            "invalid_query"
        );
        let long = "a".repeat(201);
        assert_eq!(
            search(&state, &catalog, &long, None).unwrap_err().code,
            "invalid_query"
        );
        assert_eq!(
            search(&state, &catalog, "amor", Some(0)).unwrap_err().code,
            "invalid_limit"
        );
        assert_eq!(
            search(&state, &catalog, "amor", Some(101))
                .unwrap_err()
                .code,
            "invalid_limit"
        );

        let missing = dir.path().join("nao_existe");
        assert!(build_all_indexes(&state, &[missing], &mut |_| {}).is_err());
        assert!(!state.index_dir().join("CURRENT").exists());
    }

    #[test]
    fn strong_occurrences_and_translations_work_and_validate_inputs() {
        let dir = tempdir().unwrap();
        let mods = dir.path().join("mods");
        fs::create_dir(&mods).unwrap();
        let p_dict = mods.join("strong_amg.dct.twm");
        generate_synthetic_study_dictionary_twm(&p_dict).unwrap();

        let p_bible = mods.join("sint_pt.nt");
        let mut lines: Vec<String> = vec![".".to_string(); 7957];
        lines[0] = "luz<WG25> brilha<WG25> forte".into();
        lines[1] = "luz<WG25> brilha<WG25> clara".into();
        let mut content = lines.join("\n");
        content.push_str("\ntitle=Biblia Sintetica\nshort.title=BSIN\nlang=pt\ncharset=65001\n");
        fs::write(&p_bible, content).unwrap();

        let mut bible_info = info_for(
            "bible-cat-1",
            p_bible,
            "Biblia Sintetica",
            ModuleState::Ready,
        );
        bible_info.kind = ModuleKind::Bible;
        bible_info.has_strong = true;

        let catalog = vec![
            info_for("id-dict", p_dict, "Dicionario", ModuleState::Ready),
            bible_info,
        ];

        let state = LexState::new(&dir.path().join("appdata"));
        build_all_indexes(&state, std::slice::from_ref(&mods), &mut |_| {}).unwrap();

        // Validações
        assert_eq!(
            strong_occurrences(&state, &catalog, "X99", None, None, None)
                .unwrap_err()
                .code,
            "invalid_strong"
        );
        assert_eq!(
            strong_occurrences(&state, &catalog, "G25", None, Some(0), None)
                .unwrap_err()
                .code,
            "invalid_page"
        );
        assert_eq!(
            strong_occurrences(&state, &catalog, "G25", None, Some(201), None)
                .unwrap_err()
                .code,
            "invalid_page"
        );
        assert_eq!(
            strong_translations(&state, &catalog, "G25", None, Some(0))
                .unwrap_err()
                .code,
            "invalid_limit"
        );

        // Consulta mapeia module_id para o ID do catálogo
        let occ = strong_occurrences(&state, &catalog, "g25", None, Some(1), Some(10)).unwrap();
        assert_eq!(occ.strong, "G25");
        assert_eq!(occ.selected_module_id.as_deref(), Some("bible-cat-1"));
        assert_eq!(occ.total_verses, 2);
        assert_eq!(occ.total_occurrences, 4);
        assert_eq!(occ.by_book.len(), 1);
        assert_eq!(occ.by_book[0].book, 40);

        let tr =
            strong_translations(&state, &catalog, "G25", Some("bible-cat-1"), Some(10)).unwrap();
        assert_eq!(tr.total, 4);
        assert_eq!(tr.items.len(), 2);
    }
}
