mod frontmatter;
mod index;
mod markdown;
mod vault;

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

/// Shared app state: the currently open vault path and its derived-index
/// connection. `None` until a vault has been opened.
#[derive(Default)]
pub struct AppState {
    vault_path: Mutex<Option<PathBuf>>,
    db: Mutex<Option<Connection>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultInfo {
    path: String,
    page_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageSummary {
    id: String,
    title: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageContent {
    id: String,
    title: String,
    body: String,
    html: String,
}

fn derived_index_path(app: &AppHandle) -> Result<PathBuf, String> {
    vault::app_data_dir(app)
        .map(|dir| dir.join("index.sqlite3"))
        .map_err(|e| e.to_string())
}

/// Returns the previously remembered vault path, if any, so the frontend can
/// auto-open it instead of prompting the user again.
#[tauri::command]
fn get_remembered_vault(app: AppHandle) -> Option<String> {
    vault::load_remembered_vault(&app).map(|p| p.to_string_lossy().to_string())
}

/// Opens a native folder picker and returns the chosen path, or `None` if the
/// user cancelled.
#[tauri::command]
fn pick_vault_folder(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .map(|p| p.to_string())
}

/// Opens `path` as the vault: ensures it's a git repo, persists it as the
/// remembered vault, and rebuilds the derived SQLite index from scratch.
#[tauri::command]
fn open_vault(app: AppHandle, state: State<AppState>, path: String) -> Result<VaultInfo, String> {
    let vault_path = PathBuf::from(&path);
    if !vault_path.is_dir() {
        return Err(format!("'{path}' is not a directory"));
    }

    vault::ensure_git_repo(&vault_path).map_err(|e| e.to_string())?;
    vault::persist_vault_path(&app, &vault_path).map_err(|e| e.to_string())?;

    let db_file = derived_index_path(&app)?;
    // Rebuild from scratch every launch/open (ADR-0008): drop any stale file
    // rather than trying to reconcile it.
    let _ = std::fs::remove_file(&db_file);
    let mut conn = Connection::open(&db_file).map_err(|e| e.to_string())?;
    let page_count = index::build_index(&mut conn, &vault_path).map_err(|e| e.to_string())?;

    *state.vault_path.lock().unwrap() = Some(vault_path.clone());
    *state.db.lock().unwrap() = Some(conn);

    Ok(VaultInfo {
        path: vault_path.to_string_lossy().to_string(),
        page_count,
    })
}

/// Lists every persisted page's id/title, flat and alphabetical.
#[tauri::command]
fn list_pages(state: State<AppState>) -> Result<Vec<PageSummary>, String> {
    let guard = state.db.lock().unwrap();
    let conn = guard.as_ref().ok_or("No vault is open")?;

    let mut stmt = conn
        .prepare("SELECT id, title FROM pages ORDER BY title COLLATE NOCASE ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(PageSummary {
                id: row.get(0)?,
                title: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut pages = Vec::new();
    for row in rows {
        pages.push(row.map_err(|e| e.to_string())?);
    }
    Ok(pages)
}

/// Fetches one page's title, raw markdown body, and rendered HTML.
#[tauri::command]
fn get_page(state: State<AppState>, id: String) -> Result<PageContent, String> {
    let guard = state.db.lock().unwrap();
    let conn = guard.as_ref().ok_or("No vault is open")?;

    let (title, body): (String, String) = conn
        .query_row(
            "SELECT title, body FROM pages WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let html = markdown::render(&body);

    Ok(PageContent {
        id,
        title,
        body,
        html,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_remembered_vault,
            pick_vault_folder,
            open_vault,
            list_pages,
            get_page,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
