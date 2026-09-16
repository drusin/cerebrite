mod frontmatter;
mod index;
mod links;
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

/// A single grouped-by-source-page backlink entry, as returned to the
/// frontend by `get_backlinks` (issue 06).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BacklinkEntry {
    source_id: String,
    source_title: String,
    snippet: String,
    modified_at: i64,
}

impl From<index::BacklinkEntry> for BacklinkEntry {
    fn from(entry: index::BacklinkEntry) -> Self {
        BacklinkEntry {
            source_id: entry.source_id,
            source_title: entry.source_title,
            snippet: entry.snippet,
            modified_at: entry.modified_at,
        }
    }
}

/// Result of resolving a `[[Link]]` title to a page (issue 05 / ADR-0009).
///
/// `Persisted` means a real file backs this page (found by
/// case/whitespace-insensitive title match, via
/// `frontmatter::normalize_title`); `Dynamic` means no persisted page
/// matched, so the frontend should render a UI-identical page with an empty
/// body, backed by nothing on disk, identified purely by `normalized_title`
/// until the first write materializes it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PageResolution {
    #[serde(rename_all = "camelCase")]
    Persisted {
        id: String,
        title: String,
        body: String,
        html: String,
    },
    #[serde(rename_all = "camelCase")]
    Dynamic { normalized_title: String },
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

/// Saves a page's edited body: preserves the file's existing frontmatter
/// exactly, writes the new body, auto-commits the change to the vault's
/// local git repo (ADR-0006 -- local commit only, no remote push/pull), and
/// updates the derived index's row for this page so search stays in sync
/// without a full rebuild.
#[tauri::command]
fn save_page(state: State<AppState>, id: String, markdown_body: String) -> Result<(), String> {
    save_page_impl(&state, &id, &markdown_body)
}

/// Shared implementation behind `save_page` and `materialize_and_save_page`
/// (the latter calls this immediately after materializing a dynamic page,
/// so the freshly-minted frontmatter/id are preserved by `write_body`
/// rather than being overwritten by a from-scratch file write).
fn save_page_impl(state: &AppState, id: &str, markdown_body: &str) -> Result<(), String> {
    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };

    let mut db_guard = state.db.lock().unwrap();
    let conn = db_guard.as_mut().ok_or("No vault is open")?;

    let (title, path): (String, String) = conn
        .query_row(
            "SELECT title, path FROM pages WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let page_path = PathBuf::from(&path);
    frontmatter::write_body(&page_path, markdown_body).map_err(|e| e.to_string())?;

    vault::commit_all(&vault_path, &format!("Update {title}")).map_err(|e| e.to_string())?;

    // Re-parse from disk (rather than trusting `markdown_body` verbatim) so
    // the index reflects exactly what write_body persisted.
    let parsed = frontmatter::parse_and_ensure_id(&page_path).map_err(|e| e.to_string())?;
    index::update_page_content(conn, id, &parsed.title, &parsed.body).map_err(|e| e.to_string())?;

    Ok(())
}

/// Explicit "new page" action (issue 04): given a title, immediately mints a
/// frontmatter id, derives a slug-of-the-title filename, writes a
/// frontmatter-only file (empty body, no auto-inserted heading), auto-commits
/// it to the vault's local git repo like any other save, and adds it to the
/// derived index in place (no full rebuild).
///
/// Per the ticket and its referenced prototype spec (07), a title collision
/// with an existing persisted page is blocked with an in-app error rather
/// than silently disambiguated -- so this only touches the filesystem/index
/// once it's confirmed neither the title nor the derived filename is already
/// taken.
#[tauri::command]
fn create_page(state: State<AppState>, title: String) -> Result<PageSummary, String> {
    create_page_impl(&state, &title)
}

/// Shared implementation behind `create_page` and
/// `materialize_and_save_page` (issue 05): the latter is a dynamic page's
/// first write materializing it, using these exact same mechanics (mint id,
/// slugify title into filename, frontmatter-only body) before the caller
/// writes the real body via `save_page_impl`.
fn create_page_impl(state: &AppState, title: &str) -> Result<PageSummary, String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err("Title cannot be empty".to_string());
    }

    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };

    let mut db_guard = state.db.lock().unwrap();
    let conn = db_guard.as_mut().ok_or("No vault is open")?;

    let existing_titles: i64 = conn
        .query_row(
            "SELECT count(*) FROM pages WHERE title = ?1",
            params![trimmed],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if existing_titles > 0 {
        return Err(format!("A page titled '{trimmed}' already exists"));
    }

    let slug = frontmatter::slugify(trimmed);
    let file_path = vault_path.join(format!("{slug}.md"));
    if file_path.exists() {
        return Err(format!(
            "A page file for '{trimmed}' already exists ({slug}.md)"
        ));
    }

    let id = frontmatter::generate_id();
    let content = frontmatter::new_page_content(&id, trimmed);
    std::fs::write(&file_path, &content).map_err(|e| e.to_string())?;

    vault::commit_all(&vault_path, &format!("Create {trimmed}")).map_err(|e| e.to_string())?;

    index::insert_page(conn, &id, trimmed, &file_path, "").map_err(|e| e.to_string())?;

    Ok(PageSummary {
        id,
        title: trimmed.to_string(),
    })
}

/// Resolves a `[[Link]]`'s raw title (issue 05): finds an existing
/// persisted page by case/whitespace-insensitive title match
/// (`frontmatter::normalize_title`), or -- if none matches -- reports a
/// dynamic page identified by that normalized title. Used both for
/// link-click navigation and by the editor's save path to decide whether a
/// save must materialize the page first.
#[tauri::command]
fn resolve_page(state: State<AppState>, title: String) -> Result<PageResolution, String> {
    resolve_page_impl(&state, &title)
}

/// Shared implementation behind `resolve_page`, split out (like
/// `create_page_impl`/`save_page_impl`) so it's unit-testable directly
/// against a plain `AppState` without needing a full Tauri app harness.
fn resolve_page_impl(state: &AppState, title: &str) -> Result<PageResolution, String> {
    let guard = state.db.lock().unwrap();
    let conn = guard.as_ref().ok_or("No vault is open")?;

    let normalized = frontmatter::normalize_title(&title);

    let mut stmt = conn
        .prepare("SELECT id, title, body FROM pages")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;

    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let id: String = row.get(0).map_err(|e| e.to_string())?;
        let page_title: String = row.get(1).map_err(|e| e.to_string())?;
        if frontmatter::normalize_title(&page_title) == normalized {
            let body: String = row.get(2).map_err(|e| e.to_string())?;
            let html = markdown::render(&body);
            return Ok(PageResolution::Persisted {
                id,
                title: page_title,
                body,
                html,
            });
        }
    }

    Ok(PageResolution::Dynamic {
        normalized_title: normalized,
    })
}

/// Returns every backlink pointing at the page identified by `title`,
/// grouped by source page (most-recently-modified source first) with a
/// snippet of surrounding text per entry (issue 06).
///
/// `title` is normalized the same way for a persisted page's own title and
/// for a dynamic page's title-only identity, so this works identically for
/// both: a dynamic page has no row in `pages` at all, but its backlinks are
/// simply whatever rows in `backlinks` already carry its normalized title as
/// their target -- no `pages` lookup for the target side is needed.
#[tauri::command]
fn get_backlinks(state: State<AppState>, title: String) -> Result<Vec<BacklinkEntry>, String> {
    get_backlinks_impl(&state, &title)
}

/// Shared implementation behind `get_backlinks`, split out for direct unit
/// testing against a plain `AppState`.
fn get_backlinks_impl(state: &AppState, title: &str) -> Result<Vec<BacklinkEntry>, String> {
    let guard = state.db.lock().unwrap();
    let conn = guard.as_ref().ok_or("No vault is open")?;

    let normalized = frontmatter::normalize_title(title);
    let entries = index::get_backlinks(conn, &normalized).map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(BacklinkEntry::from).collect())
}

/// Materializes a dynamic page into a real persisted file the instant it
/// receives its first write (ADR-0009): reuses `create_page_impl` (mint id,
/// slug filename, frontmatter-only body -- identical to the explicit "new
/// page" action) and then writes `markdown_body` as the real body via
/// `save_page_impl`, which preserves the just-minted frontmatter/id rather
/// than overwriting the file from scratch.
#[tauri::command]
fn materialize_and_save_page(
    state: State<AppState>,
    title: String,
    markdown_body: String,
) -> Result<PageSummary, String> {
    materialize_and_save_page_impl(&state, &title, &markdown_body)
}

/// Shared implementation behind `materialize_and_save_page`, split out for
/// direct unit testing against a plain `AppState`.
fn materialize_and_save_page_impl(
    state: &AppState,
    title: &str,
    markdown_body: &str,
) -> Result<PageSummary, String> {
    let summary = create_page_impl(state, title)?;
    save_page_impl(state, &summary.id, markdown_body)?;
    Ok(summary)
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
            save_page,
            create_page,
            resolve_page,
            get_backlinks,
            materialize_and_save_page,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Builds an `AppState` over a fresh git-repo vault at `dir`, with the
    /// derived index built from whatever `.md` files already exist there --
    /// mirroring what `open_vault` does, minus the Tauri app-handle/dialog
    /// bits the `_impl` functions under test don't need.
    fn setup_vault(dir: &TempDir) -> AppState {
        vault::ensure_git_repo(dir.path()).unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        index::build_index(&mut conn, dir.path()).unwrap();

        AppState {
            vault_path: Mutex::new(Some(dir.path().to_path_buf())),
            db: Mutex::new(Some(conn)),
        }
    }

    fn write_page(dir: &TempDir, name: &str, content: &str) {
        fs::write(dir.path().join(name), content).unwrap();
    }

    #[test]
    fn resolve_page_finds_existing_persisted_page_case_and_whitespace_insensitively() {
        let dir = TempDir::new().unwrap();
        write_page(
            &dir,
            "hello.md",
            "---\nid: abc-123\ntitle: Hello World\n---\nSome body.\n",
        );
        let state = setup_vault(&dir);

        let resolution = resolve_page_impl(&state, "  hello   WORLD  ").unwrap();

        match resolution {
            PageResolution::Persisted { id, title, body, .. } => {
                assert_eq!(id, "abc-123");
                assert_eq!(title, "Hello World");
                assert_eq!(body, "Some body.\n");
            }
            PageResolution::Dynamic { .. } => panic!("expected a persisted match"),
        }
    }

    #[test]
    fn resolve_page_returns_dynamic_with_normalized_title_when_nothing_matches() {
        let dir = TempDir::new().unwrap();
        let state = setup_vault(&dir);

        let resolution = resolve_page_impl(&state, "  Some   New Thing  ").unwrap();

        assert_eq!(
            resolution,
            PageResolution::Dynamic {
                normalized_title: "some new thing".to_string()
            }
        );
    }

    #[test]
    fn resolve_page_is_dynamic_even_when_a_different_page_exists() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "other.md", "---\nid: x\ntitle: Other Page\n---\n");
        let state = setup_vault(&dir);

        let resolution = resolve_page_impl(&state, "Nonexistent Page").unwrap();

        assert_eq!(
            resolution,
            PageResolution::Dynamic {
                normalized_title: "nonexistent page".to_string()
            }
        );
    }

    #[test]
    fn materialize_and_save_page_creates_a_file_identically_to_create_page() {
        let dir = TempDir::new().unwrap();
        let state = setup_vault(&dir);

        // Sanity check: before materializing, this title has no persisted match.
        assert!(matches!(
            resolve_page_impl(&state, "Fresh Page").unwrap(),
            PageResolution::Dynamic { .. }
        ));

        let summary =
            materialize_and_save_page_impl(&state, "Fresh Page", "Some freshly typed content.\n")
                .unwrap();

        let file_path = dir.path().join("fresh-page.md");
        assert!(file_path.exists(), "materializing should create fresh-page.md");

        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains(&format!("id: {}", summary.id)));
        assert!(content.contains("title: Fresh Page"));
        assert!(content.ends_with("Some freshly typed content.\n"));

        // It's now an ordinary persisted page, findable by resolve_page.
        let resolution = resolve_page_impl(&state, "Fresh Page").unwrap();
        match resolution {
            PageResolution::Persisted { id, title, body, .. } => {
                assert_eq!(id, summary.id);
                assert_eq!(title, "Fresh Page");
                assert_eq!(body, "Some freshly typed content.\n");
            }
            PageResolution::Dynamic { .. } => panic!("expected the now-materialized page"),
        }
    }

    #[test]
    fn materialize_rejects_a_title_collision_just_like_create_page() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "taken.md", "---\nid: x\ntitle: Taken\n---\n");
        let state = setup_vault(&dir);

        let err = materialize_and_save_page_impl(&state, "Taken", "New content\n").unwrap_err();
        assert!(err.contains("already exists"));
    }

    #[test]
    fn get_backlinks_groups_by_source_most_recently_modified_first() {
        let dir = TempDir::new().unwrap();
        // Older mtime.
        write_page(&dir, "old-note.md", "---\nid: old\ntitle: Old Note\n---\nSee [[Target Page]] here.\n");
        let old_path = dir.path().join("old-note.md");
        let old_time = filetime::FileTime::from_unix_time(1_000_000, 0);
        filetime::set_file_mtime(&old_path, old_time).unwrap();

        // Newer mtime, and links to Target Page twice.
        write_page(
            &dir,
            "new-note.md",
            "---\nid: new\ntitle: New Note\n---\nFirst [[Target Page]] and again [[Target Page]].\n",
        );
        let new_path = dir.path().join("new-note.md");
        let new_time = filetime::FileTime::from_unix_time(2_000_000, 0);
        filetime::set_file_mtime(&new_path, new_time).unwrap();

        write_page(&dir, "target.md", "---\nid: target\ntitle: Target Page\n---\nNothing links from here.\n");

        let state = setup_vault(&dir);

        let entries = get_backlinks_impl(&state, "Target Page").unwrap();

        assert_eq!(entries.len(), 3, "expected 2 links from New Note + 1 from Old Note");
        // Most-recently-modified source page first.
        assert_eq!(entries[0].source_id, "new");
        assert_eq!(entries[1].source_id, "new");
        assert_eq!(entries[2].source_id, "old");
        assert!(entries[0].snippet.contains("[[Target Page]]"));
    }

    #[test]
    fn get_backlinks_works_for_a_dynamic_unmatched_target_title() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "source.md", "---\nid: s1\ntitle: Source\n---\nLinks to [[Not Yet Created]].\n");
        let state = setup_vault(&dir);

        // "Not Yet Created" has no persisted page/row -- resolve_page would
        // report it as Dynamic -- but its backlinks must still be found.
        assert!(matches!(
            resolve_page_impl(&state, "Not Yet Created").unwrap(),
            PageResolution::Dynamic { .. }
        ));

        let entries = get_backlinks_impl(&state, "Not Yet Created").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_id, "s1");
    }

    #[test]
    fn get_backlinks_is_empty_when_nothing_links_here() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "lonely.md", "---\nid: l1\ntitle: Lonely\n---\nNo links at all.\n");
        let state = setup_vault(&dir);

        let entries = get_backlinks_impl(&state, "Lonely").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn incremental_save_updates_backlinks_without_a_full_rebuild() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "source.md", "---\nid: s1\ntitle: Source\n---\nOriginally links to [[Old Target]].\n");
        write_page(&dir, "old-target.md", "---\nid: ot\ntitle: Old Target\n---\n");
        write_page(&dir, "new-target.md", "---\nid: nt\ntitle: New Target\n---\n");
        let state = setup_vault(&dir);

        assert_eq!(get_backlinks_impl(&state, "Old Target").unwrap().len(), 1);
        assert_eq!(get_backlinks_impl(&state, "New Target").unwrap().len(), 0);

        // Edit Source's body (as an ordinary save_page would) to link
        // elsewhere instead -- no full rebuild, just update_page_content.
        save_page_impl(&state, "s1", "Now links to [[New Target]] instead.\n").unwrap();

        assert_eq!(get_backlinks_impl(&state, "Old Target").unwrap().len(), 0);
        let updated = get_backlinks_impl(&state, "New Target").unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].source_id, "s1");
    }

    #[test]
    fn newly_created_page_with_links_is_immediately_a_backlink_source() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "target.md", "---\nid: t1\ntitle: Target\n---\n");
        let state = setup_vault(&dir);

        assert_eq!(get_backlinks_impl(&state, "Target").unwrap().len(), 0);

        let summary = materialize_and_save_page_impl(&state, "Fresh Source", "Links to [[Target]].\n").unwrap();

        let entries = get_backlinks_impl(&state, "Target").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_id, summary.id);
    }

    #[test]
    fn subsequent_saves_after_materializing_behave_like_an_ordinary_save_page() {
        let dir = TempDir::new().unwrap();
        let state = setup_vault(&dir);

        let summary =
            materialize_and_save_page_impl(&state, "Growing Page", "First write.\n").unwrap();

        // A later, ordinary save (as `save_page` would perform) must preserve
        // the id/frontmatter and just update the body.
        save_page_impl(&state, &summary.id, "Second write, edited.\n").unwrap();

        let file_path = dir.path().join("growing-page.md");
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains(&format!("id: {}", summary.id)));
        assert!(content.ends_with("Second write, edited.\n"));

        let resolution = resolve_page_impl(&state, "Growing Page").unwrap();
        match resolution {
            PageResolution::Persisted { body, .. } => assert_eq!(body, "Second write, edited.\n"),
            PageResolution::Dynamic { .. } => panic!("expected the persisted page"),
        }
    }
}
