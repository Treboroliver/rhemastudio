use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rhema_sermon::{Sermon, SermonError, SermonStore, SermonSummary, VersionInfo, export_markdown};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

pub const DB_FILE_NAME: &str = "rhema.db";

/// Banco de sermões; `store` é `None` se a abertura falhou (o motivo fica em `init_error`).
pub struct SermonState {
    store: Option<Arc<SermonStore>>,
    init_error: Option<String>,
}

impl SermonState {
    pub fn open(data_dir: &Path) -> Self {
        match SermonStore::open(&data_dir.join(DB_FILE_NAME)) {
            Ok(store) => Self {
                store: Some(Arc::new(store)),
                init_error: None,
            },
            Err(error) => Self {
                store: None,
                init_error: Some(error.to_string()),
            },
        }
    }

    fn store(&self) -> Result<&Arc<SermonStore>, SermonApiError> {
        self.store.as_ref().ok_or_else(|| SermonApiError {
            code: "sermon_unavailable",
            message: self
                .init_error
                .clone()
                .unwrap_or_else(|| "Banco de sermões indisponível.".to_string()),
        })
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SermonApiError {
    pub code: &'static str,
    pub message: String,
}

impl From<SermonError> for SermonApiError {
    fn from(error: SermonError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
        }
    }
}

impl SermonApiError {
    fn export(message: impl ToString) -> Self {
        Self {
            code: "sermon_export_failed",
            message: message.to_string(),
        }
    }
}

type ApiResult<T> = Result<T, SermonApiError>;

#[tauri::command]
pub fn sermon_list(
    state: State<'_, SermonState>,
    include_archived: Option<bool>,
    query: Option<String>,
) -> ApiResult<Vec<SermonSummary>> {
    let include_archived = include_archived.unwrap_or(false);
    let store = state.store()?;
    Ok(
        match query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
            Some(query) => store.search_by_title(query, include_archived)?,
            None => store.list(include_archived)?,
        },
    )
}

#[tauri::command]
pub fn sermon_create(
    state: State<'_, SermonState>,
    title: String,
    base_text: Option<String>,
) -> ApiResult<Sermon> {
    Ok(state
        .store()?
        .create(&title, base_text.as_deref().unwrap_or(""))?)
}

#[tauri::command]
pub fn sermon_get(state: State<'_, SermonState>, id: String) -> ApiResult<Sermon> {
    Ok(state.store()?.get(&id)?)
}

#[tauri::command]
pub fn sermon_save(state: State<'_, SermonState>, sermon: Sermon) -> ApiResult<Sermon> {
    Ok(state.store()?.save(&sermon)?)
}

#[tauri::command]
pub fn sermon_duplicate(state: State<'_, SermonState>, id: String) -> ApiResult<Sermon> {
    Ok(state.store()?.duplicate(&id)?)
}

/// Arquiva (padrão) ou, com `archived = false`, desarquiva.
#[tauri::command]
pub fn sermon_archive(
    state: State<'_, SermonState>,
    id: String,
    archived: Option<bool>,
) -> ApiResult<()> {
    let store = state.store()?;
    if archived.unwrap_or(true) {
        store.archive(&id)?;
    } else {
        store.unarchive(&id)?;
    }
    Ok(())
}

#[tauri::command]
pub fn sermon_delete(state: State<'_, SermonState>, id: String) -> ApiResult<()> {
    Ok(state.store()?.delete(&id)?)
}

#[tauri::command]
pub fn sermon_versions(state: State<'_, SermonState>, id: String) -> ApiResult<Vec<VersionInfo>> {
    Ok(state.store()?.versions(&id)?)
}

#[tauri::command]
pub fn sermon_restore(
    state: State<'_, SermonState>,
    id: String,
    version_id: i64,
) -> ApiResult<Sermon> {
    Ok(state.store()?.restore_version(&id, version_id)?)
}

#[tauri::command]
pub fn sermon_export_markdown(state: State<'_, SermonState>, id: String) -> ApiResult<String> {
    Ok(export_markdown(&state.store()?.get(&id)?))
}

/// Abre o diálogo nativo de salvar e grava o Markdown; devolve o caminho ou `None` se cancelado.
#[tauri::command]
pub async fn sermon_save_export(
    app: AppHandle,
    state: State<'_, SermonState>,
    id: String,
) -> ApiResult<Option<String>> {
    let sermon = state.store()?.get(&id)?;
    let markdown = export_markdown(&sermon);
    let chosen = app
        .dialog()
        .file()
        .set_title("Exportar sermão")
        .set_file_name(suggested_file_name(&sermon.title))
        .add_filter("Markdown", &["md"])
        .blocking_save_file();
    let Some(chosen) = chosen else {
        return Ok(None);
    };
    let path = chosen.into_path().map_err(SermonApiError::export)?;
    write_atomic(&path, markdown.as_bytes()).map_err(SermonApiError::export)?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

pub(crate) fn suggested_file_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(80)
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').trim();
    if cleaned.is_empty() {
        "sermao.md".to_string()
    } else {
        format!("{cleaned}.md")
    }
}

/// Arquivo temporário vizinho + rename, para nunca deixar o destino pela metade.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut temporary: PathBuf = path.to_path_buf();
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".rhema-tmp");
    temporary.set_file_name(name);
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_serialize_with_stable_codes() {
        let error = SermonApiError::from(SermonError::NotFound);
        let json = serde_json::to_value(&error).expect("json");
        assert_eq!(json["code"], "sermon_not_found");
        assert!(json["message"].is_string());
        assert_eq!(
            SermonApiError::from(SermonError::TitleEmpty).code,
            "sermon_title_empty"
        );
    }

    #[test]
    fn state_open_failure_is_reported_not_fatal() {
        let dir = tempfile::tempdir().expect("pasta");
        let blocker = dir.path().join("arquivo");
        fs::write(&blocker, b"x").expect("arquivo");
        let state = SermonState::open(&blocker);
        let error = state.store().err().expect("indisponível");
        assert_eq!(error.code, "sermon_unavailable");
    }

    #[test]
    fn state_uses_rhema_db_in_the_data_folder() {
        let dir = tempfile::tempdir().expect("pasta");
        let state = SermonState::open(dir.path());
        let store = state.store().expect("aberto");
        let sermon = store.create("Teste", "").expect("criar");
        assert!(dir.path().join(DB_FILE_NAME).exists());
        assert_eq!(store.get(&sermon.id).expect("ler").title, "Teste");
    }

    #[test]
    fn file_names_are_sanitised() {
        assert_eq!(
            suggested_file_name("Jo 3:16 / graça?"),
            "Jo 3_16 _ graça_.md"
        );
        assert_eq!(suggested_file_name("  ..  "), "sermao.md");
        assert_eq!(suggested_file_name(&"a".repeat(300)).chars().count(), 83);
    }

    #[test]
    fn atomic_write_replaces_the_file_and_leaves_no_temporary() {
        let dir = tempfile::tempdir().expect("pasta");
        let path = dir.path().join("saida.md");
        write_atomic(&path, b"um").expect("gravar");
        write_atomic(&path, b"dois").expect("gravar");
        assert_eq!(fs::read(&path).expect("ler"), b"dois");
        assert_eq!(fs::read_dir(dir.path()).expect("listar").count(), 1);
    }
}
