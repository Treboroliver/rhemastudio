use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rhema_core::ModuleKind;
use rhema_lexicon::{
    LexKind, LexiconError, LexiconIndex, MatchField, SearchFilters, SearchResults, build_index,
    build_links, build_reverse_index,
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
}

impl LexState {
    pub fn new(app_data: &Path) -> Self {
        Self {
            data_dir: app_data.join("lexicon"),
            building: Mutex::new(false),
            progress: Mutex::new(BuildProgressState::default()),
            index: Mutex::new(None),
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

    pub fn invalidate_cache(&self) {
        if let Ok(mut guard) = self.index.lock() {
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
pub struct LexSearchDto {
    pub query: String,
    pub total: usize,
    pub shown: usize,
    pub interpretations: Vec<String>,
    pub groups: Vec<LexSearchGroupDto>,
    pub notes: Vec<String>,
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

    // Etapa 3: índice reverso palavra -> Strong (opcional se não houver Bíblias com Strong)
    update_progress(state, emit, mk("reverse", 0, 1, 85, None, 0));
    if let Err(err) = build_reverse_index(state.reverse_path(), folders) {
        log::info!("build_reverse_index ignorado: {err}");
    }

    // Reabre e atualiza os handles em cache
    state.invalidate_cache();
    state
        .cached_or_open()
        .map_err(|e| ApiError::internal(format!("Erro ao abrir índice construído: {e}")))?;
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
) -> Result<LexiconEntryDto, ApiError> {
    let module_id = module_id.trim();
    let key = key.trim();
    if module_id.is_empty() || key.is_empty() || key.chars().count() > MAX_QUERY_CHARS {
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
    study::lookup_entry(cache, module, key)
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
        let loaded = entry(&cache, &catalog, &res.groups[0].hits[0].module_id, "G25").unwrap();
        assert_eq!(loaded.module_id, "id-dict");
        assert!(!loaded.degraded);
        assert!(!loaded.nodes.is_empty());

        assert_eq!(
            entry(&cache, &catalog, "id-prot", "G25").unwrap_err().code,
            "module_protected"
        );
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
}
