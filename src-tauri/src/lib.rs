mod frontmatter;
mod heading_slug;
mod index;
mod links;
mod markdown;
mod redirects;
mod search;
mod sync;
mod trash;
mod vault;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

/// Shared app state: the currently open vault path and its derived-index
/// connection. `None` until a vault has been opened.
pub struct AppState {
    vault_path: Mutex<Option<PathBuf>>,
    db: Mutex<Option<Connection>>,
    /// This device's stable id (ticket 08), used to attribute entries this
    /// device appends to `.cerebrite/redirects.tsv`. `None` until a vault is
    /// open (or, in tests that construct `AppState` directly, always --
    /// callers fall back to a placeholder id in that case).
    device_id: Mutex<Option<String>>,
    /// Latest known background-sync status (issue 14), polled by the
    /// frontend via `get_sync_status` and also best-effort emitted as a
    /// `sync-status-changed` event. Starts `NoRemote` (the honest default
    /// before a vault -- and its remote, if any -- has even been checked).
    sync_status: Mutex<sync::SyncStatus>,
    /// Sender the background sync loop listens on: every local auto-commit
    /// sends a ping (see `notify_sync`), which the loop debounces/coalesces;
    /// it also wakes on its own periodic timer regardless. Replacing this
    /// (on a fresh `open_vault`) drops the old sender, which cleanly stops
    /// the previous vault's loop thread.
    sync_tx: Mutex<Option<Sender<()>>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            vault_path: Mutex::new(None),
            db: Mutex::new(None),
            device_id: Mutex::new(None),
            sync_status: Mutex::new(sync::SyncStatus::NoRemote),
            sync_tx: Mutex::new(None),
        }
    }
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

/// One trashed page (issue 10), as surfaced to the frontend's "Trash" list:
/// enough to display it and to drive an in-app "Restore" action (which needs
/// `trashed_filename`, the manifest key).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashedPageSummary {
    id: String,
    title: String,
    trashed_filename: String,
    original_relative_path: String,
}

impl From<trash::TrashedPage> for TrashedPageSummary {
    fn from(page: trash::TrashedPage) -> Self {
        TrashedPageSummary {
            id: page.id,
            title: page.title,
            trashed_filename: page.trashed_filename,
            original_relative_path: page.original_relative_path,
        }
    }
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
    /// The target heading's slug (ticket 07), present only when the link
    /// that produced this entry targeted a heading rather than the page
    /// itself -- the frontend renders a "→ Heading" label when this is set.
    target_heading_slug: Option<String>,
}

impl From<index::BacklinkEntry> for BacklinkEntry {
    fn from(entry: index::BacklinkEntry) -> Self {
        BacklinkEntry {
            source_id: entry.source_id,
            source_title: entry.source_title,
            snippet: entry.snippet,
            modified_at: entry.modified_at,
            target_heading_slug: entry.target_heading_slug,
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
        /// The target heading's slug (ticket 07), if the resolved `[[Link]]`
        /// included a `#Heading` fragment -- the frontend uses this to
        /// scroll to that heading after navigating to the page.
        heading_slug: Option<String>,
        /// True when this page currently sits in `.cerebrite/trash/` (issue
        /// 10): links to a trashed page must keep resolving here (not fall
        /// through to `Dynamic`) since the id/content are still on disk --
        /// the frontend renders an "in trash" indicator with an inline
        /// restore action instead of the ordinary page chrome.
        #[serde(default)]
        in_trash: bool,
        /// The trashed file's filename under `.cerebrite/trash/` (the trash
        /// manifest key) -- present only when `in_trash` is true, since
        /// that's what the "Restore" action needs.
        #[serde(default)]
        trashed_filename: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Dynamic {
        normalized_title: String,
        /// Per the ticket: a link to a heading that doesn't (yet) exist
        /// behaves like a page-level dynamic link -- there is no
        /// heading-specific dynamic target, so this is carried through only
        /// for consistency/debugging, never acted on by the frontend for a
        /// dynamic page.
        heading_slug: Option<String>,
    },
}

/// One search result row (ticket 13), as returned to the frontend's search
/// modal by `search_pages`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    id: String,
    title: String,
    tier: u8,
    snippet: String,
    in_trash: bool,
    matched_tag: Option<String>,
}

impl From<search::SearchResult> for SearchResult {
    fn from(result: search::SearchResult) -> Self {
        SearchResult {
            id: result.id,
            title: result.title,
            tier: result.tier,
            snippet: result.snippet,
            in_trash: result.in_trash,
            matched_tag: result.matched_tag,
        }
    }
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
    let device_id = vault::load_or_create_device_id(&app).map_err(|e| e.to_string())?;

    let db_file = derived_index_path(&app)?;
    // Rebuild from scratch every launch/open (ADR-0008): drop any stale file
    // rather than trying to reconcile it.
    let _ = std::fs::remove_file(&db_file);
    let mut conn = Connection::open(&db_file).map_err(|e| e.to_string())?;
    let page_count = index::build_index(&mut conn, &vault_path).map_err(|e| e.to_string())?;

    // Byproduct of the full rebuild (ticket 08), never a separate scan:
    // rewrite files whose heading links resolve through a redirect chain to
    // a now-current slug, then prune redirect entries nothing references
    // any more.
    redirects::cleanup_and_prune(&vault_path, &conn).map_err(|e| e.to_string())?;

    *state.vault_path.lock().unwrap() = Some(vault_path.clone());
    *state.db.lock().unwrap() = Some(conn);
    *state.device_id.lock().unwrap() = Some(device_id);

    // (Re)start the background sync loop for this vault (issue 14). Storing
    // the new sender drops the previous one, if any, which cleanly stops
    // whatever loop was running for a previously-open vault.
    let (tx, rx) = mpsc::channel();
    *state.sync_tx.lock().unwrap() = Some(tx);
    spawn_sync_loop(app.clone(), vault_path.clone(), rx);

    Ok(VaultInfo {
        path: vault_path.to_string_lossy().to_string(),
        page_count,
    })
}

/// Pings the background sync loop after a local auto-commit. Fire-and-forget:
/// a missing sender (no vault open yet, or sync loop not started, e.g. in
/// plain `_impl` unit tests that build `AppState` directly) is not an error.
fn notify_sync(state: &AppState) {
    if let Some(tx) = state.sync_tx.lock().unwrap().as_ref() {
        let _ = tx.send(());
    }
}

/// Runs the background sync loop for one open vault (issue 14): wakes either
/// when `rx` receives a ping (a local auto-commit just happened) or after a
/// fixed 60s timeout (so incoming remote changes are still picked up while
/// the user is idle), whichever comes first.
///
/// A ping wakeup debounces briefly and drains any further pings that arrive
/// in that window, so several commits in quick succession (e.g. a burst of
/// saves) coalesce into a single sync attempt rather than one per commit.
///
/// Exits cleanly once `rx` disconnects, which happens when `open_vault`
/// replaces `state.sync_tx` (e.g. a different vault is opened) and drops the
/// only sender this loop was listening on.
fn spawn_sync_loop(app: AppHandle, vault_path: PathBuf, rx: std::sync::mpsc::Receiver<()>) {
    thread::spawn(move || loop {
        match rx.recv_timeout(Duration::from_secs(60)) {
            Ok(()) => {
                // Debounce/coalesce: a burst of saves should trigger one
                // sync, not one per commit.
                thread::sleep(Duration::from_millis(500));
                while rx.try_recv().is_ok() {}
            }
            Err(RecvTimeoutError::Timeout) => {
                // Periodic tick -- catches incoming remote changes even when
                // the user isn't actively editing.
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }

        perform_sync(&app, &vault_path);
    });
}

/// Runs one sync attempt and reconciles its outcome into `AppState`: updates
/// `sync_status` (polled by the frontend via `get_sync_status`), best-effort
/// emits a `sync-status-changed` event, and rebuilds the derived index
/// (ADR-0008) when the sync actually changed files on disk.
fn perform_sync(app: &AppHandle, vault_path: &Path) {
    let state = app.state::<AppState>();
    *state.sync_status.lock().unwrap() = sync::SyncStatus::Syncing;
    let _ = app.emit("sync-status-changed", &sync::SyncStatus::Syncing);

    let outcome = match sync::run_sync(vault_path) {
        Ok(outcome) => outcome,
        Err(e) => sync::SyncOutcome {
            status: sync::SyncStatus::Error { detail: e.to_string() },
            index_rebuild_needed: false,
        },
    };

    if outcome.index_rebuild_needed {
        let mut db_guard = state.db.lock().unwrap();
        if let Some(conn) = db_guard.as_mut() {
            let _ = index::build_index(conn, vault_path);
        }
    }

    *state.sync_status.lock().unwrap() = outcome.status.clone();
    let _ = app.emit("sync-status-changed", &outcome.status);
}

/// Polled by the frontend for a lightweight sync-status indicator (issue
/// 14): "synced" / "syncing" / "no remote configured" / "sync needs
/// attention". A `sync-status-changed` event is also emitted on every
/// transition for a frontend that prefers to react immediately rather than
/// poll.
#[tauri::command]
fn get_sync_status(state: State<AppState>) -> sync::SyncStatus {
    state.sync_status.lock().unwrap().clone()
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

    let device_id = state
        .device_id
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| "unknown-device".to_string());

    let mut db_guard = state.db.lock().unwrap();
    let conn = db_guard.as_mut().ok_or("No vault is open")?;

    let (title, path, old_body): (String, String, String) = conn
        .query_row(
            "SELECT title, path, body FROM pages WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| e.to_string())?;

    let page_path = PathBuf::from(&path);
    frontmatter::write_body(&page_path, markdown_body).map_err(|e| e.to_string())?;

    // Re-parse from disk (rather than trusting `markdown_body` verbatim) so
    // the index reflects exactly what write_body persisted.
    let parsed = frontmatter::parse_and_ensure_id(&page_path).map_err(|e| e.to_string())?;

    // Rename detection (ticket 08): compare this page's heading slugs before
    // and after the save. Any detected rename gets a sorted-inserted entry in
    // `.cerebrite/redirects.tsv` *before* committing, so the same commit that
    // saves the page's new content also carries the redirect log update.
    let old_slugs = markdown::heading_slugs(&old_body);
    let new_slugs = markdown::heading_slugs(&parsed.body);
    for (old_slug, new_slug) in redirects::detect_renames(&old_slugs, &new_slugs) {
        let redirect_entry = redirects::RedirectEntry {
            old_key: redirects::make_key(id, &old_slug),
            new_key: redirects::make_key(id, &new_slug),
            timestamp: redirects::now_rfc3339(),
            device_id: device_id.clone(),
        };
        redirects::insert_sorted(&vault_path, redirect_entry).map_err(|e| e.to_string())?;
    }

    vault::commit_all(&vault_path, &format!("Update {title}")).map_err(|e| e.to_string())?;
    notify_sync(state);

    index::update_page_content(conn, id, &parsed.title, &parsed.body, &parsed.tags).map_err(|e| e.to_string())?;

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
    notify_sync(state);

    index::insert_page(conn, &id, trimmed, &file_path, "").map_err(|e| e.to_string())?; // no frontmatter tags on a brand-new page

    Ok(PageSummary {
        id,
        title: trimmed.to_string(),
    })
}

/// Resolves a `[[Link]]`'s raw title (issue 05), which may carry a trailing
/// `#Heading` fragment (ticket 07): finds an existing persisted page by
/// case/whitespace-insensitive title match (`frontmatter::normalize_title`)
/// against the part before the `#`, or -- if none matches -- reports a
/// dynamic page identified by that normalized title. Used both for
/// link-click navigation and by the editor's save path to decide whether a
/// save must materialize the page first.
#[tauri::command]
fn resolve_page(state: State<AppState>, title: String) -> Result<PageResolution, String> {
    resolve_page_impl(&state, &title)
}

/// Splits a raw `[[Link]]` title into its page-level target text and (if
/// present) its target heading's slug -- shared by `resolve_page_impl` so
/// navigation and the underlying link-extraction logic (`links.rs`) treat
/// the `#Heading` fragment identically.
fn split_title_and_heading_slug(raw_title: &str) -> (&str, Option<String>) {
    let mut parts = raw_title.splitn(2, '#');
    let page_title = parts.next().unwrap_or("").trim();
    let heading_slug = parts
        .next()
        .map(str::trim)
        .filter(|fragment| !fragment.is_empty())
        .map(heading_slug::slugify_heading);
    (page_title, heading_slug)
}

/// Shared implementation behind `resolve_page`, split out (like
/// `create_page_impl`/`save_page_impl`) so it's unit-testable directly
/// against a plain `AppState` without needing a full Tauri app harness.
fn resolve_page_impl(state: &AppState, title: &str) -> Result<PageResolution, String> {
    let vault_path = state.vault_path.lock().unwrap().clone();

    let guard = state.db.lock().unwrap();
    let conn = guard.as_ref().ok_or("No vault is open")?;

    let (page_title, heading_slug) = split_title_and_heading_slug(title);
    let normalized = frontmatter::normalize_title(page_title);

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
            let resolved_heading_slug = resolve_heading_slug_via_redirects(
                vault_path.as_deref(),
                &id,
                &body,
                heading_slug.as_deref(),
            );
            return Ok(PageResolution::Persisted {
                id,
                title: page_title,
                body,
                html,
                heading_slug: resolved_heading_slug,
                in_trash: false,
                trashed_filename: None,
            });
        }
    }

    // No persisted (non-trashed) match -- but per ADR-0010, a page currently
    // sitting in `.cerebrite/trash/` must still resolve (rendered "in trash"
    // with a restore prompt) rather than falling through to `Dynamic`, since
    // its id/content are still on disk. Only emptying the trash makes a link
    // to it truly dangling/dynamic.
    if let Some(vault_path) = vault_path.as_deref() {
        if let Ok(trashed_pages) = trash::list_trashed_pages(vault_path) {
            for trashed in trashed_pages {
                if frontmatter::normalize_title(&trashed.title) == normalized {
                    let file_path = trash::trash_dir(vault_path).join(&trashed.trashed_filename);
                    if let Ok(parsed) = frontmatter::parse_and_ensure_id(&file_path) {
                        let html = markdown::render(&parsed.body);
                        return Ok(PageResolution::Persisted {
                            id: trashed.id,
                            title: trashed.title,
                            body: parsed.body,
                            html,
                            heading_slug,
                            in_trash: true,
                            trashed_filename: Some(trashed.trashed_filename),
                        });
                    }
                }
            }
        }
    }

    Ok(PageResolution::Dynamic {
        normalized_title: normalized,
        heading_slug,
    })
}

/// Resolves `slug` against `page_id`'s current heading slugs (parsed from
/// `body`), following the vault's redirect log (ticket 08) when `slug`
/// doesn't currently exist on the page -- so a `[[Page#old-slug]]` link keeps
/// resolving immediately after a rename, without the file itself being
/// rewritten yet. Returns `slug` unchanged if there's no vault open, no
/// redirect log, or nothing to resolve through (`slug` is `None`).
fn resolve_heading_slug_via_redirects(
    vault_path: Option<&Path>,
    page_id: &str,
    body: &str,
    slug: Option<&str>,
) -> Option<String> {
    let slug = slug?;
    let Some(vault_path) = vault_path else {
        return Some(slug.to_string());
    };
    let current_slugs = markdown::heading_slugs(body);
    if current_slugs.iter().any(|s| s == slug) {
        return Some(slug.to_string());
    }
    let entries = redirects::load(vault_path).unwrap_or_default();
    Some(redirects::resolve_heading_slug(&entries, page_id, slug, &current_slugs))
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
    let vault_path = state.vault_path.lock().unwrap().clone();

    let guard = state.db.lock().unwrap();
    let conn = guard.as_ref().ok_or("No vault is open")?;

    let normalized = frontmatter::normalize_title(title);
    let entries = index::get_backlinks(conn, &normalized).map_err(|e| e.to_string())?;

    // Resolve each entry's target_heading_slug through the redirect log
    // (ticket 08), same as `resolve_page` -- a backlink recorded against an
    // old heading slug should still show the target's current one. Only
    // meaningful when the target itself is a persisted page (a dynamic
    // target has no id/body to resolve against).
    let target_page = find_page_by_normalized_title(conn, &normalized);

    let entries = if let (Some(vault_path), Some((target_id, target_body))) = (&vault_path, &target_page) {
        let current_slugs = markdown::heading_slugs(target_body);
        let redirect_entries = redirects::load(vault_path).unwrap_or_default();
        entries
            .into_iter()
            .map(|mut entry| {
                if let Some(slug) = &entry.target_heading_slug {
                    if !current_slugs.iter().any(|s| s == slug) {
                        entry.target_heading_slug = Some(redirects::resolve_heading_slug(
                            &redirect_entries,
                            target_id,
                            slug,
                            &current_slugs,
                        ));
                    }
                }
                entry
            })
            .collect()
    } else {
        entries
    };

    Ok(entries.into_iter().map(BacklinkEntry::from).collect())
}

/// Scans `pages` for a row whose title normalizes to `normalized`, returning
/// its id/body -- the same case/whitespace-insensitive match `resolve_page`
/// uses, needed here to find the *target* page's current heading slugs for
/// redirect resolution.
fn find_page_by_normalized_title(conn: &Connection, normalized: &str) -> Option<(String, String)> {
    let mut stmt = conn.prepare("SELECT id, title, body FROM pages").ok()?;
    let mut rows = stmt.query([]).ok()?;
    while let Some(row) = rows.next().ok()? {
        let id: String = row.get(0).ok()?;
        let title: String = row.get(1).ok()?;
        if frontmatter::normalize_title(&title) == normalized {
            let body: String = row.get(2).ok()?;
            return Some((id, body));
        }
    }
    None
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

/// Explicit "delete page" action (issue 10 / ADR-0010): moves a persisted
/// page's file into `.cerebrite/trash/` (a git-tracked move, auto-committed
/// like any other edit) rather than deleting it, and removes it from the
/// derived index (`index::remove_page`) so it stops appearing in "All
/// pages"/search and its outbound links stop counting as backlinks
/// elsewhere. The frontmatter (and its id) is untouched -- a pure move.
#[tauri::command]
fn trash_page(state: State<AppState>, id: String) -> Result<(), String> {
    trash_page_impl(&state, &id)
}

fn trash_page_impl(state: &AppState, id: &str) -> Result<(), String> {
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
    trash::trash_page(&vault_path, &page_path, id, &format!("Trash {title}")).map_err(|e| e.to_string())?;
    notify_sync(state);

    // Only drop the DB rows once the filesystem move + commit succeeded.
    index::remove_page(conn, id).map_err(|e| e.to_string())?;

    Ok(())
}

/// Explicit "restore" action (issue 10): moves a trashed page's file back to
/// its original path (per the trash manifest), re-adds it to the derived
/// index, and git-commits the restore. The file's frontmatter/id are
/// untouched -- a pure move.
#[tauri::command]
fn restore_page(state: State<AppState>, trashed_filename: String) -> Result<PageSummary, String> {
    restore_page_impl(&state, &trashed_filename)
}

fn restore_page_impl(state: &AppState, trashed_filename: &str) -> Result<PageSummary, String> {
    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };

    let trashed_path = trash::trash_dir(&vault_path).join(trashed_filename);
    let parsed = frontmatter::parse_and_ensure_id(&trashed_path).map_err(|e| e.to_string())?;

    let restored_path =
        trash::restore_page(&vault_path, trashed_filename, &format!("Restore {}", parsed.title))
            .map_err(|e| e.to_string())?;
    notify_sync(state);

    let mut db_guard = state.db.lock().unwrap();
    let conn = db_guard.as_mut().ok_or("No vault is open")?;
    index::insert_restored_page(conn, &parsed.id, &parsed.title, &restored_path, &parsed.body, &parsed.tags)
        .map_err(|e| e.to_string())?;

    Ok(PageSummary {
        id: parsed.id,
        title: parsed.title,
    })
}

/// Explicit "empty trash" action (issue 10): permanently deletes every file
/// under `.cerebrite/trash/` (real filesystem removal -- there is no other
/// purge path) and git-commits the removal. Trashed pages already have no
/// rows in the derived index (removed at trash time), so no index cleanup is
/// needed here.
#[tauri::command]
fn empty_trash(state: State<AppState>) -> Result<(), String> {
    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };
    trash::empty_trash(&vault_path).map_err(|e| e.to_string())?;
    notify_sync(&state);
    Ok(())
}

/// Lists every page currently sitting in `.cerebrite/trash/` (issue 10), for
/// the frontend's "Trash" view.
#[tauri::command]
fn list_trashed_pages(state: State<AppState>) -> Result<Vec<TrashedPageSummary>, String> {
    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };
    trash::list_trashed_pages(&vault_path)
        .map(|pages| pages.into_iter().map(TrashedPageSummary::from).collect())
        .map_err(|e| e.to_string())
}

/// Full search (ticket 13): one query against title, tags, and body, ranked
/// in three strict tiers (see search.rs). `include_trash`, when true, also
/// searches trashed pages directly off disk (they aren't indexed) and flags
/// them `in_trash` in the results.
#[tauri::command]
fn search_pages(state: State<AppState>, query: String, include_trash: bool) -> Result<Vec<SearchResult>, String> {
    search_pages_impl(&state, &query, include_trash)
}

fn search_pages_impl(state: &AppState, query: &str, include_trash: bool) -> Result<Vec<SearchResult>, String> {
    let vault_path = state.vault_path.lock().unwrap().clone();

    let guard = state.db.lock().unwrap();
    let conn = guard.as_ref().ok_or("No vault is open")?;

    search::search_pages(conn, vault_path.as_deref(), query, include_trash)
        .map(|results| results.into_iter().map(SearchResult::from).collect())
        .map_err(|e| e.to_string())
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
            trash_page,
            restore_page,
            empty_trash,
            list_trashed_pages,
            search_pages,
            get_sync_status,
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
            device_id: Mutex::new(Some("test-device".to_string())),
            sync_status: Mutex::new(sync::SyncStatus::NoRemote),
            sync_tx: Mutex::new(None),
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
                normalized_title: "some new thing".to_string(),
                heading_slug: None,
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
                normalized_title: "nonexistent page".to_string(),
                heading_slug: None,
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
    fn resolve_page_carries_the_heading_slug_for_a_persisted_target() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "hello.md", "---\nid: abc-123\ntitle: Hello World\n---\nBody.\n");
        let state = setup_vault(&dir);

        let resolution = resolve_page_impl(&state, "Hello World#Some Heading").unwrap();

        match resolution {
            PageResolution::Persisted { id, heading_slug, .. } => {
                assert_eq!(id, "abc-123");
                assert_eq!(heading_slug.as_deref(), Some("some-heading"));
            }
            PageResolution::Dynamic { .. } => panic!("expected a persisted match"),
        }
    }

    #[test]
    fn resolve_page_carries_the_heading_slug_for_a_dynamic_target() {
        let dir = TempDir::new().unwrap();
        let state = setup_vault(&dir);

        let resolution = resolve_page_impl(&state, "Nonexistent#Setup").unwrap();

        assert_eq!(
            resolution,
            PageResolution::Dynamic {
                normalized_title: "nonexistent".to_string(),
                heading_slug: Some("setup".to_string()),
            }
        );
    }

    #[test]
    fn resolve_page_without_a_heading_fragment_has_no_heading_slug() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "hello.md", "---\nid: abc-123\ntitle: Hello World\n---\nBody.\n");
        let state = setup_vault(&dir);

        let resolution = resolve_page_impl(&state, "Hello World").unwrap();

        match resolution {
            PageResolution::Persisted { heading_slug, .. } => assert_eq!(heading_slug, None),
            PageResolution::Dynamic { .. } => panic!("expected a persisted match"),
        }
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

    #[test]
    fn saving_a_heading_rename_writes_a_redirect_entry_that_resolve_page_honors() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "guide.md", "---\nid: guide\ntitle: Guide\n---\n## Setup\n\nBody.\n");
        let state = setup_vault(&dir);

        // A link written against the pre-rename slug resolves fine right now.
        let resolution = resolve_page_impl(&state, "Guide#Setup").unwrap();
        match resolution {
            PageResolution::Persisted { heading_slug, .. } => {
                assert_eq!(heading_slug.as_deref(), Some("setup"))
            }
            PageResolution::Dynamic { .. } => panic!("expected a persisted match"),
        }

        // Rename the heading via an ordinary save.
        save_page_impl(&state, "guide", "## Getting Started\n\nBody.\n").unwrap();

        // The redirect log now has exactly one sorted entry for this rename.
        let entries = redirects::load(dir.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].old_key, "guide#setup");
        assert_eq!(entries[0].new_key, "guide#getting-started");

        // The old link text still resolves -- to the new slug -- without the
        // file itself being rewritten.
        let resolution = resolve_page_impl(&state, "Guide#Setup").unwrap();
        match resolution {
            PageResolution::Persisted { heading_slug, .. } => {
                assert_eq!(heading_slug.as_deref(), Some("getting-started"))
            }
            PageResolution::Dynamic { .. } => panic!("expected a persisted match"),
        }
    }

    #[test]
    fn renaming_a_heading_that_is_only_edited_not_removed_is_not_a_false_positive_rename() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "guide.md", "---\nid: guide\ntitle: Guide\n---\n## Setup\n\nBody.\n");
        let state = setup_vault(&dir);

        // Adding a second, unrelated heading (no removal) must not be
        // mistaken for a rename of "Setup".
        save_page_impl(&state, "guide", "## Setup\n\nBody.\n\n## Usage\n\nMore.\n").unwrap();

        assert!(redirects::load(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn get_backlinks_resolves_a_stale_target_heading_slug_through_the_redirect_log() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "target.md", "---\nid: target\ntitle: Target\n---\n## Setup\n\nBody.\n");
        write_page(
            &dir,
            "source.md",
            "---\nid: source\ntitle: Source\n---\nSee [[Target#Setup]] here.\n",
        );
        let state = setup_vault(&dir);

        save_page_impl(&state, "target", "## Getting Started\n\nBody.\n").unwrap();

        let entries = get_backlinks_impl(&state, "Target").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].target_heading_slug.as_deref(), Some("getting-started"));
    }

    #[test]
    fn trash_page_removes_it_from_list_pages_and_moves_the_file() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "hello.md", "---\nid: h1\ntitle: Hello\n---\nBody.\n");
        let state = setup_vault(&dir);

        trash_page_impl(&state, "h1").unwrap();

        assert!(!dir.path().join("hello.md").exists());
        assert!(dir.path().join(".cerebrite/trash/hello.md").exists());
        let ids: i64 = {
            let guard = state.db.lock().unwrap();
            guard
                .as_ref()
                .unwrap()
                .query_row("SELECT count(*) FROM pages WHERE id = 'h1'", [], |row| row.get(0))
                .unwrap()
        };
        assert_eq!(ids, 0);
    }

    #[test]
    fn trash_page_excludes_its_outbound_backlinks_from_the_targets_section() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "source.md", "---\nid: s1\ntitle: Source\n---\nLinks to [[Target]].\n");
        write_page(&dir, "target.md", "---\nid: t1\ntitle: Target\n---\n");
        let state = setup_vault(&dir);

        assert_eq!(get_backlinks_impl(&state, "Target").unwrap().len(), 1);

        trash_page_impl(&state, "s1").unwrap();

        assert!(get_backlinks_impl(&state, "Target").unwrap().is_empty());
    }

    #[test]
    fn resolve_page_finds_a_trashed_page_and_flags_it_in_trash() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "hello.md", "---\nid: h1\ntitle: Hello World\n---\nBody.\n");
        let state = setup_vault(&dir);

        trash_page_impl(&state, "h1").unwrap();

        let resolution = resolve_page_impl(&state, "Hello World").unwrap();
        match resolution {
            PageResolution::Persisted { id, title, body, in_trash, trashed_filename, .. } => {
                assert_eq!(id, "h1");
                assert_eq!(title, "Hello World");
                assert_eq!(body, "Body.\n");
                assert!(in_trash);
                assert_eq!(trashed_filename.as_deref(), Some("hello.md"));
            }
            PageResolution::Dynamic { .. } => panic!("expected a trashed-but-resolving match"),
        }
    }

    #[test]
    fn restore_page_reinstates_the_page_in_the_index_and_original_path() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "hello.md", "---\nid: h1\ntitle: Hello World\n---\nBody.\n");
        let state = setup_vault(&dir);

        trash_page_impl(&state, "h1").unwrap();
        let summary = restore_page_impl(&state, "hello.md").unwrap();

        assert_eq!(summary.id, "h1");
        assert_eq!(summary.title, "Hello World");
        assert!(dir.path().join("hello.md").exists());
        assert!(!dir.path().join(".cerebrite/trash/hello.md").exists());

        // Ordinary resolve_page again -- no longer in trash.
        let resolution = resolve_page_impl(&state, "Hello World").unwrap();
        match resolution {
            PageResolution::Persisted { in_trash, .. } => assert!(!in_trash),
            PageResolution::Dynamic { .. } => panic!("expected the restored persisted page"),
        }
    }

    #[test]
    fn empty_trash_permanently_deletes_a_trashed_page() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "hello.md", "---\nid: h1\ntitle: Hello World\n---\nBody.\n");
        let state = setup_vault(&dir);

        trash_page_impl(&state, "h1").unwrap();
        assert_eq!(list_trashed_pages_impl(&state).unwrap().len(), 1);

        empty_trash_impl(&state).unwrap();

        assert!(!dir.path().join(".cerebrite/trash/hello.md").exists());
        assert!(list_trashed_pages_impl(&state).unwrap().is_empty());
    }

    #[test]
    fn search_pages_impl_ranks_tiers_and_excludes_trash_by_default() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "widget.md", "---\nid: w\ntitle: Widget\n---\nBody.\n");
        write_page(&dir, "hidden.md", "---\nid: h\ntitle: Hidden Widget Note\n---\nBody.\n");
        let state = setup_vault(&dir);

        trash_page_impl(&state, "h").unwrap();

        let default_results = search_pages_impl(&state, "widget", false).unwrap();
        assert_eq!(default_results.len(), 1);
        assert_eq!(default_results[0].id, "w");
        assert_eq!(default_results[0].tier, 1);

        let with_trash = search_pages_impl(&state, "widget", true).unwrap();
        assert_eq!(with_trash.len(), 2);
        let hidden = with_trash.iter().find(|r| r.id == "h").unwrap();
        assert!(hidden.in_trash);
    }

    // Small `_impl`-free wrappers, mirroring the pattern used elsewhere in
    // this test module, so trash/empty-trash tests don't need a full Tauri
    // `State<AppState>` harness.
    fn empty_trash_impl(state: &AppState) -> Result<(), String> {
        let vault_path = state.vault_path.lock().unwrap().as_ref().unwrap().clone();
        trash::empty_trash(&vault_path).map_err(|e| e.to_string())
    }

    fn list_trashed_pages_impl(state: &AppState) -> Result<Vec<trash::TrashedPage>, String> {
        let vault_path = state.vault_path.lock().unwrap().as_ref().unwrap().clone();
        trash::list_trashed_pages(&vault_path).map_err(|e| e.to_string())
    }
}
