mod author;
mod connection;
mod connection_record;
mod credential;
mod frontmatter;
mod github_oauth;
mod gitlab_oauth;
mod heading_slug;
mod index;
mod links;
mod markdown;
mod redirects;
mod search;
mod settings;
mod ssh_host_keys;
mod ssh_key;
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
#[cfg(desktop)]
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

/// Result of an explicit "rename page" action (the checklist item ticket 04
/// left undone, see `docs/known-gaps.md`): the renamed page's fresh
/// `PageSummary`, plus the ids of every *other* page whose body was rewritten
/// to keep its inbound `[[Old Title]]`-style links pointing at the new
/// title. The frontend uses `affected_page_ids` to know whether the
/// currently-open page (if any) needs its in-memory editor content reloaded
/// so a pending autosave doesn't silently revert the rewrite.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenamePageResult {
    id: String,
    title: String,
    affected_page_ids: Vec<String>,
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

/// Derives a vault's git repository root from its `vault/` page directory
/// (ADR-0011): `state.vault_path` always points at `<repo_root>/vault`
/// (that's what `vault::ensure_git_repo` guarantees), so the repo root a
/// commit/sync must operate against is always exactly its parent directory.
fn repo_root_of(vault_path: &Path) -> Result<PathBuf, String> {
    vault_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| format!("'{}' has no parent directory to use as the repository root", vault_path.display()))
}

/// Returns the app's persisted settings (remembered vault path, forced color
/// scheme) so the frontend can auto-open the vault and apply the theme
/// without prompting the user again.
#[tauri::command]
fn get_settings(app: AppHandle) -> settings::Settings {
    settings::load(&app)
}

/// Persists the forced color-scheme preference (Settings UI). Applying it to
/// the page is the frontend's job; this only saves it for next launch.
#[tauri::command]
fn set_theme(app: AppHandle, theme: settings::Theme) -> Result<(), String> {
    settings::set_theme(&app, theme).map_err(|e| e.to_string())
}

/// Opens a native folder picker and returns the chosen path, or `None` if the
/// user cancelled.
///
/// Desktop only: `tauri-plugin-dialog` 2.7.3 gates its entire folder-picker
/// surface (`pick_folder`/`pick_folders` and their blocking variants) behind
/// `#[cfg(desktop)]` -- there is currently no folder picker at all on
/// Android (only `pick_file`/`pick_files`, backed by Android's
/// ACTION_OPEN_DOCUMENT, work cross-platform). This was discovered by
/// actually cross-compiling to aarch64-linux-android for ticket 15, not
/// assumed: the original code (`app.dialog().file().blocking_pick_folder()`)
/// failed to *compile* for Android at all.
///
/// This means picking a vault folder -- and therefore opening a vault at
/// all -- has no working UI path on Android yet. Shipping real folder
/// access there needs a native Storage Access Framework (SAF) integration
/// (an `ACTION_OPEN_DOCUMENT_TREE` picker plus routing all of vault.rs's
/// `std::fs`/`git2`/`walkdir` calls through SAF's `content://` URIs instead
/// of plain paths), which is a substantial platform-specific undertaking
/// that needs a real device to build and verify against -- well beyond a
/// config-level fix, and explicitly left as follow-up rather than guessed
/// at blind. The `#[cfg(mobile)]` arm below keeps the app compiling and
/// fails the picker loudly (a clear "not supported yet" error) rather than
/// silently returning `None` as if the user simply cancelled.
#[cfg(desktop)]
#[tauri::command]
async fn pick_vault_folder(app: AppHandle) -> Option<String> {
    // Non-blocking `pick_folder` (callback bridged to `.await` via a oneshot
    // channel) rather than `blocking_pick_folder`: an async tauri command
    // must not block its executor thread on user interaction.
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder);
    });
    rx.await.ok().flatten().map(|p| p.to_string())
}

#[cfg(mobile)]
#[tauri::command]
async fn pick_vault_folder(_app: AppHandle) -> Result<Option<String>, String> {
    Err("Selecting a vault folder isn't supported on Android yet (no Storage Access Framework integration -- see pick_vault_folder in lib.rs).".into())
}

/// Opens `path` as the vault: ensures it's a git repo, persists it as the
/// remembered vault, and rebuilds the derived SQLite index from scratch.
#[tauri::command]
fn open_vault(app: AppHandle, state: State<AppState>, path: String) -> Result<VaultInfo, String> {
    let picked_path = PathBuf::from(&path);
    if !picked_path.is_dir() {
        return Err(format!("'{path}' is not a directory"));
    }

    // Per ADR-0011, `picked_path` is the *repository* the user chose, not
    // the vault itself: `ensure_git_repo` turns it into a repository root
    // (initializing/discovering/refusing as appropriate) and returns the
    // `vault/` subdirectory that all page I/O below operates under.
    let vault_path = vault::ensure_git_repo(&picked_path).map_err(|e| e.to_string())?;
    let repo_root = repo_root_of(&vault_path)?;
    settings::set_vault_path(&app, &picked_path).map_err(|e| e.to_string())?;
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
    redirects::cleanup_and_prune(&vault_path, &repo_root, &conn).map_err(|e| e.to_string())?;

    *state.vault_path.lock().unwrap() = Some(vault_path.clone());
    *state.db.lock().unwrap() = Some(conn);
    *state.device_id.lock().unwrap() = Some(device_id);

    // (Re)start the background sync loop for this vault (issue 14). Storing
    // the new sender drops the previous one, if any, which cleanly stops
    // whatever loop was running for a previously-open vault.
    let (tx, rx) = mpsc::channel();
    *state.sync_tx.lock().unwrap() = Some(tx);
    spawn_sync_loop(app.clone(), vault_path.clone(), repo_root, rx);

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
fn spawn_sync_loop(app: AppHandle, vault_path: PathBuf, repo_root: PathBuf, rx: std::sync::mpsc::Receiver<()>) {
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

        perform_sync(&app, &vault_path, &repo_root);
    });
}

/// Runs one sync attempt and reconciles its outcome into `AppState`: updates
/// `sync_status` (polled by the frontend via `get_sync_status`), best-effort
/// emits a `sync-status-changed` event, and rebuilds the derived index
/// (ADR-0008) when the sync actually changed files on disk.
fn perform_sync(app: &AppHandle, vault_path: &Path, repo_root: &Path) {
    let state = app.state::<AppState>();
    *state.sync_status.lock().unwrap() = sync::SyncStatus::Syncing;
    let _ = app.emit("sync-status-changed", &sync::SyncStatus::Syncing);

    // Ticket 03 / ADR-0012: sync authenticates with this vault's one
    // configured Connection (its named credential kind) and nothing else --
    // `None` when no connection has been set up, which is fine for a
    // transport that needs no credentials and fails cleanly for one that
    // does. A failure resolving the credential itself (e.g. a locked
    // keychain) is reported the same way a fetch/push failure would be,
    // rather than silently falling back to no credentials.
    let keychain = credential::KeychainBackend::platform();
    let config_dir = app.path().app_config_dir().ok();
    // Ticket 05: resolved once per sync attempt so SSH connections can
    // check presented host keys against it -- `None` only when the app
    // config dir itself couldn't be resolved, in which case `make_callbacks`
    // still lets pinned hosts (GitHub/GitLab) through and treats every other
    // SSH host as unconfirmed rather than silently trusting it.
    let known_hosts_path = config_dir
        .as_ref()
        .and_then(|dir| credential::known_hosts_path(dir).ok());
    let plaintext = config_dir.as_ref().map(|dir| credential::PlaintextStore::new(dir));
    let connection_load = keychain.as_ref().ok().and_then(|keychain| {
        plaintext
            .as_ref()
            .and_then(|plaintext| {
                connection::Connection::load(repo_root, keychain, plaintext, credential::CallUrgency::Background)
                    .transpose()
            })
    });

    let outcome = match connection_load {
        Some(Err(e)) => sync::SyncOutcome {
            status: sync::SyncFailureCause::Other { detail: e.to_string() }.into_status(),
            index_rebuild_needed: false,
        },
        Some(Ok(connection)) => {
            // Ticket 06: an OAuth sign-in connection's access token is
            // short-lived (8 hours) and must be refreshed unattended in this
            // background path -- refresh happens here, *before* the sync
            // attempt, so a connection whose token was about to expire uses
            // the fresh one rather than racing the old one's expiry mid-sync.
            // A no-op for every other credential kind, and for an OAuth
            // connection that isn't yet close to expiring.
            // Ticket 07: GitLab's own OAuth sign-in connections need the
            // same unattended pre-sync refresh -- both `refresh_if_needed`
            // functions are no-ops for a connection they don't own (guarded
            // by `record.provider`, not just `credential_kind`, since both
            // share `CredentialKind::OauthSignIn`), so calling both in
            // sequence is safe regardless of which provider this connection
            // is actually signed in with.
            let connection = match (keychain.as_ref().ok(), plaintext.as_ref()) {
                (keychain, Some(plaintext)) => {
                    let connection = github_oauth::refresh_if_needed(repo_root, keychain, plaintext, connection);
                    gitlab_oauth::refresh_if_needed(repo_root, keychain, plaintext, connection)
                }
                _ => connection,
            };
            match sync::run_sync(repo_root, Some(&connection), known_hosts_path.as_deref()) {
                Ok(outcome) => outcome,
                Err(e) => sync::SyncOutcome {
                    status: sync::SyncFailureCause::Other { detail: e.to_string() }.into_status(),
                    index_rebuild_needed: false,
                },
            }
        }
        None => match sync::run_sync(repo_root, None, known_hosts_path.as_deref()) {
            Ok(outcome) => outcome,
            Err(e) => sync::SyncOutcome {
                status: sync::SyncFailureCause::Other { detail: e.to_string() }.into_status(),
                index_rebuild_needed: false,
            },
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

/// Ticket 08 checklist item 3/8: the "Commit as" prefill for the currently
/// open vault -- repo-local `.git/config` -> global `~/.gitconfig` -> empty.
/// Deliberately never includes the provider tier: this command has no
/// access token to call `GET /user` with, and is meant to be callable
/// generically during vault setup (pick/create/clone, before any connection
/// exists at all) as well as from Settings. The one-time provider-suggested
/// tier only ever shows up as `OauthConnectResult::provider_suggested_author`
/// on `connect_github_oauth`/`connect_gitlab_oauth` below, where a real
/// access token is already in hand.
#[tauri::command]
fn commit_author_prefill(state: State<AppState>) -> Result<author::PrefillResult, String> {
    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };
    let repo_root = repo_root_of(&vault_path)?;
    let repo_local = author::read_repo_local(&repo_root).map_err(|e| e.to_string())?;
    let global = author::read_global().unwrap_or(None);
    Ok(author::prefill(repo_local, global, None))
}

/// Ticket 08 checklist item 6/8: reads the currently confirmed "Commit as"
/// author (repo-local only -- `None` means nothing has been confirmed for
/// this vault yet), for Settings to show the current value.
#[tauri::command]
fn get_commit_author(state: State<AppState>) -> Result<Option<author::CommitAuthor>, String> {
    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };
    let repo_root = repo_root_of(&vault_path)?;
    author::read_repo_local(&repo_root).map_err(|e| e.to_string())
}

/// Ticket 08 checklist item 4/6/7: validates `name`/`email` (ticket 11's
/// minimal checks -- a warning, never a hard block, for an unrealistic-
/// looking domain) and, if they pass, writes them to the vault's repo-local
/// `.git/config` -- never the global config. Callable both as the
/// vault-setup "Commit as" step (pick/create/clone, before the first
/// commit) and from Settings' editable "Commit as" field; the write itself
/// is identical either way.
#[tauri::command]
fn confirm_commit_author(state: State<AppState>, name: String, email: String) -> Result<author::ValidationOutcome, String> {
    let outcome = author::validate(&name, &email).map_err(|e| e.to_string())?;

    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };
    let repo_root = repo_root_of(&vault_path)?;
    author::confirm(&repo_root, &author::CommitAuthor { name, email }).map_err(|e| e.to_string())?;

    Ok(outcome)
}

/// Ticket 04's minimal/raw "connect with an access token" entry point -- the
/// generic HTTPS path for any git host that isn't GitHub/GitLab sign-in
/// (Bitbucket Cloud, Gitea, Forgejo, Codeberg, a bare HTTPS remote). Reuses
/// ticket 03's `connection::try_connect` persist-gate as-is: it runs a real
/// test fetch with `Cred::userpass_plaintext(username, token)` *before*
/// writing anything, so a wrong/expired token returns an error (its message
/// names the credential kind -- see `sync::SyncFailureCause`'s `Display`)
/// and leaves no connection record or stored secret behind. On success the
/// vault has exactly one Connection (`CredentialKind::AccessToken`); the
/// background sync loop (`perform_sync` above) picks it up on its own with
/// no further prompting.
///
/// Store choice is a call this minimal command has to make on its own (no
/// Settings-UI consent flow yet -- that's ticket 13): prefer the platform
/// keychain, actually probed (not just "did `platform()` construct")
/// so a reachable-but-locked keychain isn't mistaken for a usable one;
/// fall back to the consented plaintext store (ADR-0013) whenever it isn't.
/// A dedicated "keychain unavailable, store in plaintext instead?" prompt is
/// deliberately out of this ticket's scope.
#[tauri::command]
fn connect_access_token(
    app: AppHandle,
    state: State<AppState>,
    remote_url: String,
    username: String,
    token: String,
) -> Result<(), String> {
    let remote_url = remote_url.trim().to_string();
    let username = username.trim().to_string();
    if remote_url.is_empty() || username.is_empty() || token.is_empty() {
        return Err("Repository URL, username, and access token are all required".to_string());
    }

    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };
    let repo_root = repo_root_of(&vault_path)?;

    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let plaintext = credential::PlaintextStore::new(&config_dir);

    let keychain = credential::KeychainBackend::platform().ok();
    let store_kind = match &keychain {
        Some(kc) if kc.probe(credential::CallUrgency::Interactive).is_ok() => connection_record::StoreKind::Keychain,
        _ => connection_record::StoreKind::Plaintext,
    };

    let record = connection_record::ConnectionRecord {
        connection_id: uuid::Uuid::new_v4().to_string(),
        credential_kind: connection_record::CredentialKind::AccessToken,
        provider: infer_provider(&remote_url),
        https_username: Some(username),
        token_expiry: None,
        credential_store: store_kind,
    };

    let connection = connection::try_connect(
        &repo_root,
        keychain.as_ref(),
        &plaintext,
        record.clone(),
        token.into_bytes(),
        &remote_url,
        None, // access tokens are HTTPS-only; SSH host-key checking doesn't apply
        credential::CallUrgency::Interactive,
    )
    .map_err(|e| e.to_string())?;

    // Index bookkeeping (ADR-0013 / ticket 02's `settings::set_connection_store`):
    // records which store this connection's secret ended up in, app-wide, so
    // a later cleanup/"remove all credentials" pass can find it without
    // walking every vault on disk.
    settings::set_connection_store(&app, &record.connection_id, store_kind).map_err(|e| e.to_string())?;

    // The vault now has a working connection -- nudge the background sync
    // loop to try immediately rather than waiting for its next periodic
    // tick, same as any other state-changing action already does via
    // `notify_sync`.
    notify_sync(&state);

    debug_assert_eq!(connection.credential_kind(), connection_record::CredentialKind::AccessToken);
    Ok(())
}

/// What `generate_ssh_key`/`import_ssh_key` hand back to the frontend: the
/// public half to show (with a copy button, per the ticket) plus everything
/// `connect_ssh_key` needs to actually connect. The private key material
/// necessarily passes through the frontend here -- same trust boundary
/// ticket 04's access-token form already crosses (the user types/pastes a
/// secret into a form field that round-trips through Tauri's IPC) -- held
/// only in memory until the user confirms Connect.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SshKeyInfo {
    private_key_openssh: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    passphrase: Option<String>,
    public_key_openssh: String,
    fingerprint_sha256: String,
    had_passphrase: bool,
}

/// Ticket 05: generates a fresh ed25519 key in-app (the default: no
/// passphrase, ready for unattended sync immediately). Generation alone
/// persists nothing -- exactly like `connect_access_token`'s token isn't
/// stored until a test fetch succeeds, the key returned here only becomes a
/// real Connection once the caller sends it to `connect_ssh_key` and that
/// succeeds.
#[tauri::command]
fn generate_ssh_key() -> Result<SshKeyInfo, String> {
    let generated = ssh_key::generate_ed25519_key().map_err(|e| e.to_string())?;
    Ok(SshKeyInfo {
        private_key_openssh: generated.secret.private_key_openssh,
        passphrase: generated.secret.passphrase,
        public_key_openssh: generated.public_key_openssh,
        fingerprint_sha256: generated.fingerprint_sha256,
        had_passphrase: false,
    })
}

/// Ticket 05's import path: validates `private_key_openssh` (and, if it's
/// passphrase-protected, that `passphrase` actually unlocks it) without
/// persisting anything yet -- same "validate, don't yet store" shape as
/// `generate_ssh_key`. See `ssh_key.rs`'s module doc comment for why an
/// imported passphrase is kept (not stripped or silently dropped) and
/// stored alongside the key once `connect_ssh_key` does persist it.
#[tauri::command]
fn import_ssh_key(private_key_openssh: String, passphrase: Option<String>) -> Result<SshKeyInfo, String> {
    let imported =
        ssh_key::import_ssh_key(&private_key_openssh, passphrase.as_deref()).map_err(|e| e.to_string())?;
    Ok(SshKeyInfo {
        private_key_openssh: imported.secret.private_key_openssh,
        passphrase: imported.secret.passphrase,
        public_key_openssh: imported.public_key_openssh,
        fingerprint_sha256: imported.fingerprint_sha256,
        had_passphrase: imported.had_passphrase,
    })
}

/// Ticket 05's "connect with an SSH key" entry point, mirroring
/// `connect_access_token`'s shape exactly: reuses `connection::try_connect`
/// as-is, so a real test fetch (including this connection's host-key check,
/// via `known_hosts_path`) must succeed *before* anything is persisted.
///
/// A failure here can be an ordinary rejected-credential/unreachable-remote
/// error (same as ticket 04), or -- ticket 05's own case -- an unconfirmed
/// or mismatched SSH host key (`SyncFailureCause::HostKeyUnconfirmed`/
/// `HostKeyMismatch`, surfaced through this error's message). This minimal
/// command-level flow doesn't parse that out into a separate confirm-dialog
/// affordance (no wizard UI exists yet to show one in -- tickets 09-11); the
/// caller sees a message naming the host and fingerprint, confirms it via
/// `confirm_ssh_host_key`, and calls `connect_ssh_key` again, which then
/// finds the host already trusted.
#[tauri::command]
fn connect_ssh_key(
    app: AppHandle,
    state: State<AppState>,
    remote_url: String,
    private_key_openssh: String,
    passphrase: Option<String>,
) -> Result<(), String> {
    let remote_url = remote_url.trim().to_string();
    if remote_url.is_empty() || private_key_openssh.trim().is_empty() {
        return Err("Repository URL and an SSH private key are both required".to_string());
    }

    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };
    let repo_root = repo_root_of(&vault_path)?;

    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let plaintext = credential::PlaintextStore::new(&config_dir);
    let known_hosts_path = credential::known_hosts_path(&config_dir).map_err(|e| e.to_string())?;

    let keychain = credential::KeychainBackend::platform().ok();
    let store_kind = match &keychain {
        Some(kc) if kc.probe(credential::CallUrgency::Interactive).is_ok() => connection_record::StoreKind::Keychain,
        _ => connection_record::StoreKind::Plaintext,
    };

    let secret = ssh_key::SshSecret {
        private_key_openssh,
        passphrase,
    };

    let record = connection_record::ConnectionRecord {
        connection_id: uuid::Uuid::new_v4().to_string(),
        credential_kind: connection_record::CredentialKind::SshKey,
        provider: infer_provider(&remote_url),
        https_username: None,
        token_expiry: None,
        credential_store: store_kind,
    };

    let connection = connection::try_connect(
        &repo_root,
        keychain.as_ref(),
        &plaintext,
        record.clone(),
        secret.to_bytes(),
        &remote_url,
        Some(&known_hosts_path),
        credential::CallUrgency::Interactive,
    )
    .map_err(|e| e.to_string())?;

    settings::set_connection_store(&app, &record.connection_id, store_kind).map_err(|e| e.to_string())?;
    notify_sync(&state);

    debug_assert_eq!(connection.credential_kind(), connection_record::CredentialKind::SshKey);
    Ok(())
}

/// Ticket 05's TOFU confirmation -- the "simple confirm dialog/command" the
/// ticket allows for this ticket's minimal UI. Persists `fingerprint` for
/// `host` to Cerebrite's own known_hosts-equivalent file
/// (`credential::known_hosts_path`) *only* when called -- nothing upstream
/// of this command ever calls it automatically; the certificate_check
/// callback in `connection.rs` always rejects an unconfirmed or mismatched
/// host key rather than accepting it silently. The caller is responsible
/// for having actually shown `fingerprint` to the user and gotten explicit
/// confirmation first -- this command trusts that it did.
#[tauri::command]
fn confirm_ssh_host_key(app: AppHandle, host: String, fingerprint: String) -> Result<(), String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let path = credential::known_hosts_path(&config_dir).map_err(|e| e.to_string())?;
    ssh_host_keys::KnownHosts::confirm(&path, &host, &fingerprint).map_err(|e| e.to_string())
}

/// Ticket 06 step 1: requests a fresh device/user code pair from GitHub and
/// hands it back for the frontend to display (the code, and the URL to
/// visit). Uses the placeholder `github_oauth::GITHUB_CLIENT_ID` until a
/// real GitHub App is registered -- see `github_oauth.rs`'s module doc
/// comment for the full "blocked on manual follow-up" note. A real
/// (unregistered) client id makes GitHub reject this with
/// `incorrect_client_credentials`, surfaced here as an ordinary `Err`.
#[tauri::command]
fn start_github_device_flow() -> Result<github_oauth::DeviceCodeInfo, String> {
    let endpoints = github_oauth::GitHubEndpoints::production();
    github_oauth::request_device_code(&endpoints, github_oauth::GITHUB_CLIENT_ID).map_err(|e| e.to_string())
}

/// Ticket 06 step 2: one poll of GitHub's token endpoint, called repeatedly
/// by the frontend on a timer (the same "frontend owns the poll loop"
/// pattern `get_sync_status` already uses) at the interval
/// `start_github_device_flow`'s response named -- rather than one long
/// blocking command, so the frontend can show live progress and let the
/// user cancel.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
enum DevicePollResult {
    Success {
        access_token: String,
        refresh_token: String,
        access_token_expires_at: String,
    },
    Pending,
    SlowDown,
    Denied,
    Expired,
    Error {
        message: String,
    },
}

#[tauri::command]
fn poll_github_device_flow(device_code: String) -> DevicePollResult {
    let endpoints = github_oauth::GitHubEndpoints::production();
    match github_oauth::poll_once(&endpoints, github_oauth::GITHUB_CLIENT_ID, &device_code) {
        github_oauth::PollOutcome::Success(pair) => DevicePollResult::Success {
            access_token: pair.access_token,
            refresh_token: pair.refresh_token,
            access_token_expires_at: pair.access_token_expires_at,
        },
        github_oauth::PollOutcome::Pending => DevicePollResult::Pending,
        github_oauth::PollOutcome::SlowDown => DevicePollResult::SlowDown,
        github_oauth::PollOutcome::Denied => DevicePollResult::Denied,
        github_oauth::PollOutcome::Expired => DevicePollResult::Expired,
        github_oauth::PollOutcome::Error(e) => DevicePollResult::Error { message: e.to_string() },
    }
}

/// Ticket 06 step 3: once a token is in hand, checks whether the GitHub App
/// is installed on `remote_url`'s repository -- if not, the frontend walks
/// the user to `InstallationStatus::NotInstalled`'s `install_url` before
/// calling `connect_github_oauth` (a token for an app that isn't installed
/// on the repo would fail the test fetch anyway, but this gives the user a
/// clear next step instead of an opaque auth failure).
#[tauri::command]
fn check_github_installation(remote_url: String, access_token: String) -> Result<github_oauth::InstallationStatus, String> {
    let (owner, repo) =
        github_oauth::owner_repo_from_remote_url(&remote_url).ok_or_else(|| "not a GitHub repository URL".to_string())?;
    let endpoints = github_oauth::GitHubEndpoints::production();
    github_oauth::check_installation(&endpoints, &access_token, &owner, &repo).map_err(|e| e.to_string())
}

/// Ticket 08 checklist item 5: what a successful `connect_github_oauth`/
/// `connect_gitlab_oauth` hands back alongside "connected" -- the one-time
/// "switch to the provider's address?" offer. `provider_suggested_author`
/// is populated *only* when the vault already had a confirmed repo-local
/// author that differs from what the provider suggests; when there's no
/// confirmed author yet, that's the ordinary prefill case
/// (`commit_author_prefill`), not a switch offer, so it stays `None`. The
/// frontend surfaces both values and defaults to a no-op -- ticket 11: "The
/// default is to keep the current author. It never switches silently."
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OauthConnectResult {
    provider_suggested_author: Option<author::CommitAuthor>,
}

/// Ticket 06 step 4-6: finishes GitHub sign-in the same way
/// `connect_access_token` finishes an access-token connection -- reuses
/// `connection::try_connect` as-is, so a real test fetch with
/// `Cred::userpass_plaintext("x-access-token", access_token)` (via
/// `connection::credential_for_kind`'s `OauthSignIn` arm) must succeed
/// *before* anything is persisted. The full `OauthSecret` envelope (access
/// token + refresh token) is what actually gets stored -- see
/// `github_oauth::OauthSecret`'s doc comment -- so the background refresh
/// path (`github_oauth::refresh_if_needed`, wired into `perform_sync`) has
/// the refresh token to work with later.
#[tauri::command]
fn connect_github_oauth(
    app: AppHandle,
    state: State<AppState>,
    remote_url: String,
    access_token: String,
    refresh_token: String,
    access_token_expires_at: String,
) -> Result<OauthConnectResult, String> {
    let remote_url = remote_url.trim().to_string();
    if remote_url.is_empty() || access_token.is_empty() || refresh_token.is_empty() {
        return Err("Repository URL, access token, and refresh token are all required".to_string());
    }

    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };
    let repo_root = repo_root_of(&vault_path)?;

    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let plaintext = credential::PlaintextStore::new(&config_dir);

    let keychain = credential::KeychainBackend::platform().ok();
    let store_kind = match &keychain {
        Some(kc) if kc.probe(credential::CallUrgency::Interactive).is_ok() => connection_record::StoreKind::Keychain,
        _ => connection_record::StoreKind::Plaintext,
    };

    let record = connection_record::ConnectionRecord {
        connection_id: uuid::Uuid::new_v4().to_string(),
        credential_kind: connection_record::CredentialKind::OauthSignIn,
        provider: infer_provider(&remote_url),
        https_username: Some("x-access-token".to_string()),
        token_expiry: Some(access_token_expires_at),
        credential_store: store_kind,
    };

    let secret = github_oauth::OauthSecret {
        access_token: access_token.clone(),
        refresh_token,
    };

    let connection = connection::try_connect(
        &repo_root,
        keychain.as_ref(),
        &plaintext,
        record.clone(),
        secret.to_bytes(),
        &remote_url,
        None, // OAuth sign-in is HTTPS-only; SSH host-key checking doesn't apply
        credential::CallUrgency::Interactive,
    )
    .map_err(|e| e.to_string())?;

    settings::set_connection_store(&app, &record.connection_id, store_kind).map_err(|e| e.to_string())?;
    notify_sync(&state);

    debug_assert_eq!(connection.credential_kind(), connection_record::CredentialKind::OauthSignIn);

    // Ticket 08 checklist item 5: the one-time "switch to the provider's
    // address?" offer, only when the vault already had a *different*
    // confirmed author -- a fetch failure here (no real GitHub App
    // registered yet, see `github_oauth.rs`'s module doc comment) just
    // means no offer is made, never a failed connect.
    let existing_author = author::read_repo_local(&repo_root).unwrap_or(None);
    let endpoints = github_oauth::GitHubEndpoints::production();
    let provider_identity = author::fetch_github_identity(&endpoints.api_base_url, &access_token).ok();
    let provider_suggested_author = match (&existing_author, &provider_identity) {
        (Some(existing), Some(suggested)) if existing != suggested => Some(suggested.clone()),
        _ => None,
    };

    Ok(OauthConnectResult { provider_suggested_author })
}

/// Ticket 07 step 1: requests a fresh device/user code pair from GitLab and
/// hands it back for the frontend to display (the code, and the URL to
/// visit). Uses the placeholder `gitlab_oauth::GITLAB_CLIENT_ID` until a
/// real GitLab application is registered -- see `gitlab_oauth.rs`'s module
/// doc comment for the full "blocked on manual follow-up" note, including
/// the still-pending live spike ticket 07 was supposed to run first. A real
/// (unregistered) client id makes GitLab reject this.
#[tauri::command]
fn start_gitlab_device_flow() -> Result<gitlab_oauth::DeviceCodeInfo, String> {
    let endpoints = gitlab_oauth::GitLabEndpoints::production();
    gitlab_oauth::request_device_code(&endpoints, gitlab_oauth::GITLAB_CLIENT_ID).map_err(|e| e.to_string())
}

/// Ticket 07 step 2: one poll of GitLab's token endpoint, called repeatedly
/// by the frontend on a timer -- same "frontend owns the poll loop" pattern
/// `poll_github_device_flow` uses. Unlike GitHub's `DevicePollResult`,
/// `refreshToken` may be absent on success (ticket 07's defensive dual
/// path: GitLab's device grant may not return one at all).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
enum GitlabDevicePollResult {
    Success {
        access_token: String,
        refresh_token: Option<String>,
        access_token_expires_at: String,
    },
    Pending,
    SlowDown,
    Denied,
    Expired,
    Error {
        message: String,
    },
}

#[tauri::command]
fn poll_gitlab_device_flow(device_code: String) -> GitlabDevicePollResult {
    let endpoints = gitlab_oauth::GitLabEndpoints::production();
    match gitlab_oauth::poll_once(&endpoints, gitlab_oauth::GITLAB_CLIENT_ID, &device_code) {
        gitlab_oauth::PollOutcome::Success(pair) => GitlabDevicePollResult::Success {
            access_token: pair.access_token,
            refresh_token: pair.refresh_token,
            access_token_expires_at: pair.access_token_expires_at,
        },
        gitlab_oauth::PollOutcome::Pending => GitlabDevicePollResult::Pending,
        gitlab_oauth::PollOutcome::SlowDown => GitlabDevicePollResult::SlowDown,
        gitlab_oauth::PollOutcome::Denied => GitlabDevicePollResult::Denied,
        gitlab_oauth::PollOutcome::Expired => GitlabDevicePollResult::Expired,
        gitlab_oauth::PollOutcome::Error(e) => GitlabDevicePollResult::Error { message: e.to_string() },
    }
}

/// Ticket 07 step 3-5: finishes GitLab sign-in the same way
/// `connect_github_oauth` finishes a GitHub one -- reuses
/// `connection::try_connect` as-is, so a real test fetch with
/// `Cred::userpass_plaintext("oauth2", access_token)` (via
/// `connection::credential_for_kind`'s `OauthSignIn`/`GitLab` arm) must
/// succeed *before* anything is persisted. Unlike GitHub, there is no
/// per-repo "app installation" step to check first -- a GitLab OAuth
/// application reaches every repo the authorizing user can, so this goes
/// straight from a token pair to the test-fetch-then-persist gate.
/// `refresh_token` is `Option` (ticket 07's defensive dual path): when
/// `None`, the stored `OauthSecret` simply has no refresh token to work
/// with later, and `sync::classify_git_error_for` reports the eventual
/// 2-hour expiry as `SyncFailureCause::OauthReconnectRequired` rather than
/// a silent break.
#[tauri::command]
fn connect_gitlab_oauth(
    app: AppHandle,
    state: State<AppState>,
    remote_url: String,
    access_token: String,
    refresh_token: Option<String>,
    access_token_expires_at: String,
) -> Result<OauthConnectResult, String> {
    let remote_url = remote_url.trim().to_string();
    if remote_url.is_empty() || access_token.is_empty() {
        return Err("Repository URL and access token are both required".to_string());
    }

    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };
    let repo_root = repo_root_of(&vault_path)?;

    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let plaintext = credential::PlaintextStore::new(&config_dir);

    let keychain = credential::KeychainBackend::platform().ok();
    let store_kind = match &keychain {
        Some(kc) if kc.probe(credential::CallUrgency::Interactive).is_ok() => connection_record::StoreKind::Keychain,
        _ => connection_record::StoreKind::Plaintext,
    };

    let record = connection_record::ConnectionRecord {
        connection_id: uuid::Uuid::new_v4().to_string(),
        credential_kind: connection_record::CredentialKind::OauthSignIn,
        provider: infer_provider(&remote_url),
        https_username: Some(gitlab_oauth::GITLAB_HTTPS_USERNAME.to_string()),
        token_expiry: Some(access_token_expires_at),
        credential_store: store_kind,
    };

    let secret = gitlab_oauth::OauthSecret {
        access_token: access_token.clone(),
        refresh_token,
    };

    let connection = connection::try_connect(
        &repo_root,
        keychain.as_ref(),
        &plaintext,
        record.clone(),
        secret.to_bytes(),
        &remote_url,
        None, // OAuth sign-in is HTTPS-only; SSH host-key checking doesn't apply
        credential::CallUrgency::Interactive,
    )
    .map_err(|e| e.to_string())?;

    settings::set_connection_store(&app, &record.connection_id, store_kind).map_err(|e| e.to_string())?;
    notify_sync(&state);

    debug_assert_eq!(connection.credential_kind(), connection_record::CredentialKind::OauthSignIn);

    // Ticket 08 checklist item 5, GitLab half -- see `connect_github_oauth`'s
    // matching comment. GitLab's `GET /user` may not carry `commit_email` at
    // all (ticket 11's GitLab contingency: only confirmed once the pending
    // live spike settles which scope actually returns it), in which case
    // `fetch_gitlab_identity` returns `Ok(None)` and no offer is made --
    // same "no offer" outcome as a hard fetch failure.
    let existing_author = author::read_repo_local(&repo_root).unwrap_or(None);
    // GitLab's REST API root -- not otherwise modeled in `gitlab_oauth.rs`
    // (unlike GitHub's, which has an `api_base_url` on `GitHubEndpoints` for
    // its own installation check), so it's named here directly.
    const GITLAB_API_BASE_URL: &str = "https://gitlab.com/api/v4";
    let provider_identity = author::fetch_gitlab_identity(GITLAB_API_BASE_URL, &access_token)
        .ok()
        .flatten();
    let provider_suggested_author = match (&existing_author, &provider_identity) {
        (Some(existing), Some(suggested)) if existing != suggested => Some(suggested.clone()),
        _ => None,
    };

    Ok(OauthConnectResult { provider_suggested_author })
}

/// Provider hosts recognized as `Provider::GitHub`/`Provider::GitLab`;
/// anything else is `Provider::Other(host)` -- ticket 04's generic
/// access-token path covers exactly that "anything else" tier (Bitbucket,
/// Gitea, Forgejo, Codeberg, a bare HTTPS host).
fn infer_provider(remote_url: &str) -> connection_record::Provider {
    let without_scheme = remote_url.splitn(2, "://").nth(1).unwrap_or(remote_url);
    let after_auth = without_scheme.rsplit('@').next().unwrap_or(without_scheme);
    let host = after_auth
        .split(['/', ':'])
        .next()
        .unwrap_or(after_auth)
        .to_lowercase();
    match host.as_str() {
        "github.com" => connection_record::Provider::GitHub,
        "gitlab.com" => connection_record::Provider::GitLab,
        _ => connection_record::Provider::Other(host),
    }
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

    let repo_root = repo_root_of(&vault_path)?;
    vault::commit_all(&repo_root, &format!("Update {title}")).map_err(|e| e.to_string())?;
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

    let normalized = frontmatter::normalize_title(trimmed);
    if find_page_by_normalized_title(conn, &normalized).is_some() {
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

    let repo_root = repo_root_of(&vault_path)?;
    vault::commit_all(&repo_root, &format!("Create {trimmed}")).map_err(|e| e.to_string())?;
    notify_sync(state);

    index::insert_page(conn, &id, trimmed, &file_path, "").map_err(|e| e.to_string())?; // no frontmatter tags on a brand-new page

    Ok(PageSummary {
        id,
        title: trimmed.to_string(),
    })
}

/// Explicit "rename page" action (the checklist item ticket 04 left undone --
/// see `docs/known-gaps.md`): re-slugifies the title into a new filename,
/// rewrites every other page's (and this page's own) `[[Old Title]]`-style
/// links to the new title so nothing is silently left dangling, and commits
/// every touched file in one commit.
///
/// A title/filename collision is blocked with an in-app error exactly like
/// `create_page`, excluding the page being renamed itself so a pure
/// case/whitespace change to its own title (e.g. "My Page" -> "my page")
/// isn't mistaken for a collision with itself.
#[tauri::command]
fn rename_page(state: State<AppState>, id: String, new_title: String) -> Result<RenamePageResult, String> {
    rename_page_impl(&state, &id, &new_title)
}

/// Shared implementation behind `rename_page`, split out for direct unit
/// testing against a plain `AppState`.
fn rename_page_impl(state: &AppState, id: &str, new_title: &str) -> Result<RenamePageResult, String> {
    let trimmed = new_title.trim();
    if trimmed.is_empty() {
        return Err("Title cannot be empty".to_string());
    }

    let vault_path = {
        let guard = state.vault_path.lock().unwrap();
        guard.as_ref().ok_or("No vault is open")?.clone()
    };

    let mut db_guard = state.db.lock().unwrap();
    let conn = db_guard.as_mut().ok_or("No vault is open")?;

    let (old_title, old_path): (String, String) = conn
        .query_row(
            "SELECT title, path FROM pages WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    let normalized_new = frontmatter::normalize_title(trimmed);
    if let Some((existing_id, _)) = find_page_by_normalized_title(conn, &normalized_new) {
        if existing_id != id {
            return Err(format!("A page titled '{trimmed}' already exists"));
        }
    }

    let old_page_path = PathBuf::from(&old_path);
    let new_slug = frontmatter::slugify(trimmed);
    let new_file_path = vault_path.join(format!("{new_slug}.md"));
    if new_file_path != old_page_path && new_file_path.exists() {
        return Err(format!(
            "A page file for '{trimmed}' already exists ({new_slug}.md)"
        ));
    }

    // Snapshot every page's current id/title/path/body/tags before mutating
    // anything, so the link-rewrite pass below operates on a consistent view
    // even though it (and the frontmatter/filesystem writes before it) will
    // change several of these rows as it goes.
    let pages: Vec<(String, String, PathBuf, String, Vec<String>)> = {
        let mut stmt = conn
            .prepare("SELECT id, title, path, body, tags FROM pages")
            .map_err(|e| e.to_string())?;
        let mapped = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(4)?;
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, tags_json))
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for row in mapped {
            let (page_id, page_title, page_path, page_body, tags_json): (String, String, String, String, String) =
                row.map_err(|e| e.to_string())?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            out.push((page_id, page_title, PathBuf::from(page_path), page_body, tags));
        }
        out
    };

    // Rewrite this page's own frontmatter title, then move its file if the
    // slug changed.
    frontmatter::write_title(&old_page_path, trimmed).map_err(|e| e.to_string())?;
    if new_file_path != old_page_path {
        std::fs::rename(&old_page_path, &new_file_path).map_err(|e| e.to_string())?;
    }

    // Fix up the renamed page's own self-referential links (if any), then
    // update its index row for the new title/path regardless of whether its
    // body actually changed.
    let (_, _, _, self_body, self_tags) = pages
        .iter()
        .find(|(page_id, ..)| page_id == id)
        .cloned()
        .expect("the page being renamed must be present in its own snapshot");
    let (new_self_body, self_changed) = links::rewrite_links_to_title(&self_body, &old_title, trimmed);
    if self_changed {
        frontmatter::write_body(&new_file_path, &new_self_body).map_err(|e| e.to_string())?;
    }
    index::update_page_path_and_title(conn, id, trimmed, &new_file_path, &new_self_body, &self_tags)
        .map_err(|e| e.to_string())?;

    // Rewrite inbound links in every other page whose body mentions the old
    // title, on disk and in the index.
    let mut affected_page_ids = Vec::new();
    for (page_id, page_title, page_path, page_body, tags) in &pages {
        if page_id == id {
            continue;
        }
        let (new_body, changed) = links::rewrite_links_to_title(page_body, &old_title, trimmed);
        if !changed {
            continue;
        }
        frontmatter::write_body(page_path, &new_body).map_err(|e| e.to_string())?;
        index::update_page_content(conn, page_id, page_title, &new_body, tags).map_err(|e| e.to_string())?;
        affected_page_ids.push(page_id.clone());
    }

    let repo_root = repo_root_of(&vault_path)?;
    vault::commit_all(&repo_root, &format!("Rename {old_title} to {trimmed}")).map_err(|e| e.to_string())?;
    notify_sync(state);

    Ok(RenamePageResult {
        id: id.to_string(),
        title: trimmed.to_string(),
        affected_page_ids,
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
    let repo_root = repo_root_of(&vault_path)?;
    trash::trash_page(&vault_path, &repo_root, &page_path, id, &format!("Trash {title}")).map_err(|e| e.to_string())?;
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

    let repo_root = repo_root_of(&vault_path)?;
    let restored_path =
        trash::restore_page(&vault_path, &repo_root, trashed_filename, &format!("Restore {}", parsed.title))
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
    let repo_root = repo_root_of(&vault_path)?;
    trash::empty_trash(&vault_path, &repo_root).map_err(|e| e.to_string())?;
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
            get_settings,
            set_theme,
            pick_vault_folder,
            open_vault,
            list_pages,
            get_page,
            save_page,
            create_page,
            rename_page,
            resolve_page,
            get_backlinks,
            materialize_and_save_page,
            trash_page,
            restore_page,
            empty_trash,
            list_trashed_pages,
            search_pages,
            get_sync_status,
            commit_author_prefill,
            get_commit_author,
            confirm_commit_author,
            connect_access_token,
            generate_ssh_key,
            import_ssh_key,
            connect_ssh_key,
            confirm_ssh_host_key,
            start_github_device_flow,
            poll_github_device_flow,
            check_github_installation,
            connect_github_oauth,
            start_gitlab_device_flow,
            poll_gitlab_device_flow,
            connect_gitlab_oauth,
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
    /// Per ADR-0011, `write_page` (called before `setup_vault` by every test
    /// below) writes fixture pages under `dir.path()/vault` -- the fixed
    /// page directory -- while `setup_vault` itself `git init`s `dir.path()`
    /// as the repository root and adopts the already-populated `vault/`
    /// `ensure_git_repo` finds there (no migration triggers, since these
    /// fixtures never create a root-level `.cerebrite/`).
    fn setup_vault(dir: &TempDir) -> AppState {
        let vault_path = vault::ensure_git_repo(dir.path()).unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        index::build_index(&mut conn, &vault_path).unwrap();

        // Ticket 08: `commit_all`/`run_sync`'s merge path no longer fall
        // back to `Cerebrite <cerebrite@local>` -- every fixture that goes
        // on to save/create/rename/trash/restore a page (all of which
        // commit) needs a confirmed repo-local author first, same as
        // `vault.rs`/`sync.rs`/`trash.rs`/`redirects.rs`/`connection.rs`'s
        // own test fixtures.
        author::confirm_test_author(dir.path());

        AppState {
            vault_path: Mutex::new(Some(vault_path)),
            db: Mutex::new(Some(conn)),
            device_id: Mutex::new(Some("test-device".to_string())),
            sync_status: Mutex::new(sync::SyncStatus::NoRemote),
            sync_tx: Mutex::new(None),
        }
    }

    fn write_page(dir: &TempDir, name: &str, content: &str) {
        let vault_dir = dir.path().join("vault");
        fs::create_dir_all(&vault_dir).unwrap();
        fs::write(vault_dir.join(name), content).unwrap();
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

        let file_path = dir.path().join("vault").join("fresh-page.md");
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
    fn create_page_rejects_a_title_collision_that_only_differs_by_case_or_whitespace() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "taken.md", "---\nid: x\ntitle: Taken\n---\n");
        let state = setup_vault(&dir);

        let err = create_page_impl(&state, "  taken  ").unwrap_err();
        assert!(err.contains("already exists"));
    }

    #[test]
    fn get_backlinks_groups_by_source_most_recently_modified_first() {
        let dir = TempDir::new().unwrap();
        // Older mtime.
        write_page(&dir, "old-note.md", "---\nid: old\ntitle: Old Note\n---\nSee [[Target Page]] here.\n");
        let old_path = dir.path().join("vault").join("old-note.md");
        let old_time = filetime::FileTime::from_unix_time(1_000_000, 0);
        filetime::set_file_mtime(&old_path, old_time).unwrap();

        // Newer mtime, and links to Target Page twice.
        write_page(
            &dir,
            "new-note.md",
            "---\nid: new\ntitle: New Note\n---\nFirst [[Target Page]] and again [[Target Page]].\n",
        );
        let new_path = dir.path().join("vault").join("new-note.md");
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

        let file_path = dir.path().join("vault").join("growing-page.md");
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
        let entries = redirects::load(&dir.path().join("vault")).unwrap();
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

        assert!(redirects::load(&dir.path().join("vault")).unwrap().is_empty());
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

        assert!(!dir.path().join("vault").join("hello.md").exists());
        assert!(dir.path().join("vault/.cerebrite/trash/hello.md").exists());
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
        assert!(dir.path().join("vault").join("hello.md").exists());
        assert!(!dir.path().join("vault/.cerebrite/trash/hello.md").exists());

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

        assert!(!dir.path().join("vault/.cerebrite/trash/hello.md").exists());
        assert!(list_trashed_pages_impl(&state).unwrap().is_empty());
    }

    #[test]
    fn rename_page_updates_title_slug_and_frontmatter() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "old-title.md", "---\nid: p1\ntitle: Old Title\n---\nBody.\n");
        let state = setup_vault(&dir);

        let result = rename_page_impl(&state, "p1", "New Title").unwrap();

        assert_eq!(result.id, "p1");
        assert_eq!(result.title, "New Title");
        assert!(result.affected_page_ids.is_empty());

        assert!(!dir.path().join("vault").join("old-title.md").exists());
        let new_path = dir.path().join("vault").join("new-title.md");
        assert!(new_path.exists());
        let content = fs::read_to_string(&new_path).unwrap();
        assert!(content.contains("id: p1"));
        assert!(content.contains("title: New Title"));
        assert!(content.ends_with("Body.\n"));

        let resolution = resolve_page_impl(&state, "New Title").unwrap();
        match resolution {
            PageResolution::Persisted { id, title, .. } => {
                assert_eq!(id, "p1");
                assert_eq!(title, "New Title");
            }
            PageResolution::Dynamic { .. } => panic!("expected the renamed page"),
        }
    }

    #[test]
    fn rename_page_rewrites_inbound_links_in_other_pages() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "target.md", "---\nid: t1\ntitle: Old Title\n---\n");
        write_page(
            &dir,
            "source.md",
            "---\nid: s1\ntitle: Source\n---\nSee [[Old Title]] and [[Old Title#Some Heading]] and #[[Old Title]].\n",
        );
        let state = setup_vault(&dir);

        let result = rename_page_impl(&state, "t1", "New Title").unwrap();
        assert_eq!(result.affected_page_ids, vec!["s1".to_string()]);

        let content = fs::read_to_string(dir.path().join("vault").join("source.md")).unwrap();
        assert!(content.contains("See [[New Title]] and [[New Title#Some Heading]] and #[[New Title]]."));

        // The index reflects the rewrite too -- old title has no more
        // backlinks, new title does.
        assert!(get_backlinks_impl(&state, "Old Title").unwrap().is_empty());
        // Three occurrences in source.md: the plain link, the heading link,
        // and the bracket-tag form -- each is its own backlink row.
        assert_eq!(get_backlinks_impl(&state, "New Title").unwrap().len(), 3);
    }

    #[test]
    fn rename_page_rewrites_its_own_self_referential_links() {
        let dir = TempDir::new().unwrap();
        write_page(
            &dir,
            "old-title.md",
            "---\nid: p1\ntitle: Old Title\n---\nSee also [[Old Title]] (itself).\n",
        );
        let state = setup_vault(&dir);

        rename_page_impl(&state, "p1", "New Title").unwrap();

        let content = fs::read_to_string(dir.path().join("vault").join("new-title.md")).unwrap();
        assert!(content.contains("See also [[New Title]] (itself).\n"));
    }

    #[test]
    fn rename_page_converts_a_bare_tag_to_bracket_form_when_needed() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "todo.md", "---\nid: t1\ntitle: Todo\n---\n");
        write_page(&dir, "source.md", "---\nid: s1\ntitle: Source\n---\nFiled under #todo.\n");
        let state = setup_vault(&dir);

        rename_page_impl(&state, "t1", "To Do").unwrap();

        let content = fs::read_to_string(dir.path().join("vault").join("source.md")).unwrap();
        assert_eq!(content, "---\nid: s1\ntitle: Source\n---\nFiled under #[[To Do]].\n");
    }

    #[test]
    fn rename_page_rejects_a_title_collision_with_a_different_page() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "one.md", "---\nid: p1\ntitle: One\n---\n");
        write_page(&dir, "two.md", "---\nid: p2\ntitle: Two\n---\n");
        let state = setup_vault(&dir);

        let err = rename_page_impl(&state, "p1", "  two  ").unwrap_err();
        assert!(err.contains("already exists"));
        // Nothing should have moved.
        assert!(dir.path().join("vault").join("one.md").exists());
    }

    #[test]
    fn rename_page_allows_a_pure_casing_change_to_its_own_title() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "my-page.md", "---\nid: p1\ntitle: My Page\n---\n");
        let state = setup_vault(&dir);

        let result = rename_page_impl(&state, "p1", "MY PAGE").unwrap();
        assert_eq!(result.title, "MY PAGE");
    }

    #[test]
    fn rename_page_does_not_collide_with_a_trashed_page_of_the_same_title() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "taken.md", "---\nid: t1\ntitle: Taken\n---\n");
        write_page(&dir, "other.md", "---\nid: p1\ntitle: Other\n---\n");
        let state = setup_vault(&dir);
        trash_page_impl(&state, "t1").unwrap();

        let result = rename_page_impl(&state, "p1", "Taken").unwrap();
        assert_eq!(result.title, "Taken");
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
        let repo_root = repo_root_of(&vault_path)?;
        trash::empty_trash(&vault_path, &repo_root).map_err(|e| e.to_string())
    }

    fn list_trashed_pages_impl(state: &AppState) -> Result<Vec<trash::TrashedPage>, String> {
        let vault_path = state.vault_path.lock().unwrap().as_ref().unwrap().clone();
        trash::list_trashed_pages(&vault_path).map_err(|e| e.to_string())
    }
}
