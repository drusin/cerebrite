// Automatic background git remote sync (issue 14 / ADR-0006).
//
// The user never manually triggers sync: `run_sync` is called by `lib.rs`
// shortly after every local auto-commit (debounced/coalesced) and again on a
// periodic timer, so incoming remote changes are picked up even when the
// user isn't actively editing. See `lib.rs`'s `spawn_sync_loop` for the
// trigger machinery -- this module is deliberately Tauri-agnostic (pure
// `git2` + `std`) so it can be exercised directly against local bare-repo
// fixtures in tests, with no real network access.
//
// Conflict safety (the core of ADR-0006): a pull/merge is never allowed to
// touch the user's working-directory files until we've proven, purely in
// memory via `Repository::merge_commits`, that the merge is conflict-free.
// `merge_commits` computes a trial merge index without any checkout or
// working-tree write. If that trial merge has conflicts, we back up the
// user's current local copy of every conflicting file to
// `.cerebrite/conflict-backups/` *before* anything else happens, then abort
// the sync attempt entirely -- HEAD, the index, and the working tree are
// left exactly as they were. Actual conflict *resolution* is out of scope:
// the repo is simply left as an ordinary (not-yet-merged) local branch for
// the user to resolve with normal git tooling, and the sync status reports
// "needs attention".

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde::Serialize;

/// Relative path (from the vault root) of the conflict-backup directory,
/// mirroring the `.cerebrite/trash/` convention from ticket 10.
pub const CONFLICT_BACKUP_DIR_REL: &str = ".cerebrite/conflict-backups";

/// Sync status surfaced to the frontend (via `get_sync_status`, polled, and
/// a best-effort `sync-status-changed` event -- see `lib.rs`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum SyncStatus {
    /// No `origin` remote is configured on this vault (the common case for a
    /// freshly `git init`'d vault, per ticket 02) -- sync is a deliberate
    /// no-op, not an error.
    NoRemote,
    /// A sync attempt is currently running.
    Syncing,
    /// The last sync attempt completed with nothing left to reconcile.
    Synced,
    /// The last sync attempt hit a real (non-network) problem -- surfaced so
    /// the UI can show something other than a silently stale "Synced".
    Error { detail: String },
    /// The last sync attempt detected a conflict. A backup of the user's
    /// local version of every conflicting file was saved under
    /// `.cerebrite/conflict-backups/` before anything else touched them.
    /// Left for the user to resolve with ordinary git tooling -- no
    /// bespoke resolution UI.
    NeedsAttention { detail: String },
}

/// The result of one `run_sync` attempt.
pub struct SyncOutcome {
    pub status: SyncStatus,
    /// True when the sync changed files on disk (a fast-forward or a clean
    /// merge), so the caller should rebuild the derived index (ADR-0008).
    pub index_rebuild_needed: bool,
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Credential callbacks for both fetch and push: SSH-agent auth is the
/// primary MVP-scope case (a personal git-native vault typically syncs over
/// SSH). HTTPS-with-stored-credentials is a stretch goal, but wiring up the
/// system credential helper is cheap, so it's included as a best-effort
/// fallback -- if neither applies, `git2::Cred::default()` is tried (e.g.
/// for a bare local-path "remote", the common case in this module's own
/// tests, which needs no credentials at all).
fn make_callbacks<'a>() -> git2::RemoteCallbacks<'a> {
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|url, username_from_url, allowed_types| {
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            if let Some(user) = username_from_url {
                if let Ok(cred) = git2::Cred::ssh_key_from_agent(user) {
                    return Ok(cred);
                }
            }
        }
        if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            if let Ok(cfg) = git2::Config::open_default() {
                if let Ok(cred) = git2::Cred::credential_helper(&cfg, url, username_from_url) {
                    return Ok(cred);
                }
            }
        }
        git2::Cred::default()
    });
    callbacks
}

fn push_current_branch(remote: &mut git2::Remote, branch: &str) -> Result<()> {
    let callbacks = make_callbacks();
    let mut push_opts = git2::PushOptions::new();
    push_opts.remote_callbacks(callbacks);
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    remote
        .push(&[refspec.as_str()], Some(&mut push_opts))
        .context("pushing to remote")?;
    Ok(())
}

/// Backs up the user's current on-disk copy of every conflicting path in
/// `merged_index` under `.cerebrite/conflict-backups/`, named
/// `<relative-path>.local-backup-<unix-timestamp>`. Best-effort per file
/// (a conflict where the local side was deleted has no working file to back
/// up, and is simply skipped); returns the list of relative paths backed up.
fn backup_conflicted_files(vault_path: &Path, merged_index: &git2::Index) -> Result<Vec<String>> {
    let timestamp = now_unix_secs();
    let backup_root = vault_path.join(CONFLICT_BACKUP_DIR_REL);
    let mut backed_up = Vec::new();

    for conflict in merged_index.conflicts().context("reading merge conflicts")? {
        let conflict = conflict.context("reading one merge conflict entry")?;
        // Prefer "our" (local) side, since that's the version we must not
        // let an automatic sync silently discard; fall back to whichever
        // side is present if "ours" was deleted in this local branch.
        let entry = conflict.our.or(conflict.their).or(conflict.ancestor);
        let Some(entry) = entry else { continue };

        let rel_path = String::from_utf8_lossy(&entry.path).to_string();
        let local_file = vault_path.join(&rel_path);
        if !local_file.is_file() {
            continue;
        }

        let backup_rel = format!("{rel_path}.local-backup-{timestamp}");
        let backup_path = backup_root.join(&backup_rel);
        if let Some(parent) = backup_path.parent() {
            fs::create_dir_all(parent).context("creating conflict-backup directory")?;
        }
        fs::copy(&local_file, &backup_path).context("copying local file to conflict backup")?;
        backed_up.push(rel_path);
    }

    Ok(backed_up)
}

/// Attempts one fetch + merge-or-fast-forward + push cycle against
/// `vault_path`'s `origin` remote. A no-op (`SyncStatus::NoRemote`) when no
/// such remote is configured, or when the vault has no commits yet.
pub fn run_sync(vault_path: &Path) -> Result<SyncOutcome> {
    let repo = git2::Repository::open(vault_path).context("opening vault git repo")?;

    let mut remote = match repo.find_remote("origin") {
        Ok(remote) => remote,
        Err(_) => {
            return Ok(SyncOutcome {
                status: SyncStatus::NoRemote,
                index_rebuild_needed: false,
            })
        }
    };

    // Nothing committed yet (e.g. a brand-new vault before its first save) --
    // there is nothing meaningful to sync.
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => {
            return Ok(SyncOutcome {
                status: SyncStatus::NoRemote,
                index_rebuild_needed: false,
            })
        }
    };
    // `shorthand()` returns the literal string "HEAD" when the repo is in a
    // detached-HEAD state (no branch checked out) -- pushing that verbatim
    // would build the nonsensical refspec `refs/heads/HEAD:refs/heads/HEAD`
    // rather than erroring, silently creating a branch actually named
    // "HEAD". Detached HEAD isn't a state Cerebrite's own git usage ever
    // puts a vault into, but a user could get there with plain git tooling
    // (e.g. checking out a specific commit) -- treat it as a clean,
    // actionable error rather than attempting a nonsensical push.
    let branch = match head.shorthand() {
        Some("HEAD") | None => bail!(
            "the vault's repository is in a detached HEAD state (no branch is checked out) -- \
             check out a branch before syncing"
        ),
        Some(name) => name.to_string(),
    };
    let local_oid = head.target().context("resolving local HEAD oid")?;
    let local_commit = repo.find_commit(local_oid).context("looking up local HEAD commit")?;

    let mut fetch_opts = git2::FetchOptions::new();
    fetch_opts.remote_callbacks(make_callbacks());
    // Empty refspec list: use whatever fetch refspec is already configured on
    // the remote (the standard `+refs/heads/*:refs/remotes/origin/*`, set up
    // automatically by `git2::Repository::remote`/`clone`), same as a plain
    // `git fetch`.
    remote
        .fetch(&[] as &[&str], Some(&mut fetch_opts), None)
        .context("fetching from remote")?;

    let remote_ref_name = format!("refs/remotes/origin/{branch}");
    let remote_ref = match repo.find_reference(&remote_ref_name) {
        Ok(r) => r,
        Err(_) => {
            // The remote has no such branch yet -- nothing to merge, just
            // push to create it.
            push_current_branch(&mut remote, &branch)?;
            return Ok(SyncOutcome {
                status: SyncStatus::Synced,
                index_rebuild_needed: false,
            });
        }
    };
    let remote_oid = remote_ref.target().context("resolving remote-tracking oid")?;

    if remote_oid == local_oid {
        return Ok(SyncOutcome {
            status: SyncStatus::Synced,
            index_rebuild_needed: false,
        });
    }

    let remote_commit = repo.find_commit(remote_oid).context("looking up remote-tracking commit")?;
    let remote_annotated = repo
        .reference_to_annotated_commit(&remote_ref)
        .context("annotating remote-tracking commit")?;
    let (analysis, _preference) = repo
        .merge_analysis(&[&remote_annotated])
        .context("running merge analysis")?;

    if analysis.is_fast_forward() {
        // Local is a strict ancestor of remote: no local-only commits can be
        // lost, so a plain ref update + checkout is safe. Nothing to push.
        repo.reference(
            &format!("refs/heads/{branch}"),
            remote_oid,
            true,
            "cerebrite: fast-forward sync",
        )
        .context("fast-forwarding local branch")?;
        repo.set_head(&format!("refs/heads/{branch}"))
            .context("updating HEAD after fast-forward")?;
        let mut checkout = git2::build::CheckoutBuilder::new();
        checkout.force();
        repo.checkout_head(Some(&mut checkout))
            .context("checking out fast-forwarded HEAD")?;
        return Ok(SyncOutcome {
            status: SyncStatus::Synced,
            index_rebuild_needed: true,
        });
    }

    if analysis.is_up_to_date() {
        // Remote has nothing new to merge in; local may still be ahead.
        push_current_branch(&mut remote, &branch)?;
        return Ok(SyncOutcome {
            status: SyncStatus::Synced,
            index_rebuild_needed: false,
        });
    }

    // Both sides diverged. Compute the merge purely in memory -- this never
    // touches the working directory or HEAD -- so conflicts can be detected
    // and backed up *before* any automatic step could overwrite the user's
    // local file contents.
    let mut merged_index = repo
        .merge_commits(&local_commit, &remote_commit, None)
        .context("computing trial merge")?;

    if merged_index.has_conflicts() {
        let backed_up = backup_conflicted_files(vault_path, &merged_index)?;
        let detail = if backed_up.is_empty() {
            "Sync detected a conflict; the local version could not be safely backed up before \
             aborting."
                .to_string()
        } else {
            format!(
                "Sync detected a conflict on: {}. Your local version was copied to {}/ before \
                 anything else was touched. Resolve with ordinary git tooling.",
                backed_up.join(", "),
                CONFLICT_BACKUP_DIR_REL
            )
        };
        // Deliberately abort: HEAD, the index, and the working tree are left
        // exactly as they were before this sync attempt. No checkout, merge
        // commit, or push runs.
        return Ok(SyncOutcome {
            status: SyncStatus::NeedsAttention { detail },
            index_rebuild_needed: false,
        });
    }

    // A clean three-way merge -- safe to materialize.
    let tree_id = merged_index
        .write_tree_to(&repo)
        .context("writing merged tree")?;
    let tree = repo.find_tree(tree_id).context("looking up merged tree")?;
    let signature = repo
        .signature()
        .or_else(|_| git2::Signature::now("Cerebrite", "cerebrite@local"))
        .context("building merge commit signature")?;
    repo.commit(
        Some(&format!("refs/heads/{branch}")),
        &signature,
        &signature,
        "Merge remote changes",
        &tree,
        &[&local_commit, &remote_commit],
    )
    .context("creating merge commit")?;
    repo.set_head(&format!("refs/heads/{branch}"))
        .context("updating HEAD after merge")?;
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.checkout_head(Some(&mut checkout))
        .context("checking out merge result")?;

    push_current_branch(&mut remote, &branch)?;

    Ok(SyncOutcome {
        status: SyncStatus::Synced,
        index_rebuild_needed: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault;
    use tempfile::tempdir;

    /// Sets up a bare "remote" repo plus one working-copy clone with an
    /// initial commit already pushed, mirroring "device A already has a
    /// synced vault". Returns (bare_dir, device_a_dir).
    fn setup_remote_and_first_device() -> (tempfile::TempDir, tempfile::TempDir, String) {
        let bare_dir = tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();

        let device_a_dir = tempdir().unwrap();
        vault::ensure_git_repo(device_a_dir.path()).unwrap();
        fs::write(device_a_dir.path().join("page.md"), "---\nid: p1\n---\nOriginal.\n").unwrap();
        vault::commit_all(device_a_dir.path(), "Create page").unwrap();

        let repo_a = git2::Repository::open(device_a_dir.path()).unwrap();
        let branch = repo_a.head().unwrap().shorthand().unwrap().to_string();
        repo_a
            .remote("origin", bare_dir.path().to_str().unwrap())
            .unwrap();
        let mut remote = repo_a.find_remote("origin").unwrap();
        remote
            .push(&[format!("refs/heads/{branch}:refs/heads/{branch}").as_str()], None)
            .unwrap();

        (bare_dir, device_a_dir, branch)
    }

    /// Clones a second "device" from the bare remote, already in sync with
    /// device A's initial commit.
    fn clone_second_device(bare_dir: &tempfile::TempDir) -> tempfile::TempDir {
        let device_b_dir = tempdir().unwrap();
        git2::Repository::clone(bare_dir.path().to_str().unwrap(), device_b_dir.path()).unwrap();
        device_b_dir
    }

    #[test]
    fn detached_head_errors_cleanly_instead_of_pushing_a_branch_literally_named_head() {
        let dir = tempdir().unwrap();
        vault::ensure_git_repo(dir.path()).unwrap();
        fs::write(dir.path().join("page.md"), "---\nid: p1\n---\nHello.\n").unwrap();
        vault::commit_all(dir.path(), "Create page").unwrap();

        let repo = git2::Repository::open(dir.path()).unwrap();
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.set_head_detached(head_commit.id()).unwrap();
        assert_eq!(repo.head().unwrap().shorthand(), Some("HEAD"));

        // A remote is configured so `run_sync` gets past the "no remote"
        // early return and actually reaches branch-name derivation.
        let bare_dir = tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();
        repo.remote("origin", bare_dir.path().to_str().unwrap()).unwrap();

        let result = run_sync(dir.path());

        let err = match result {
            Ok(_) => panic!("expected a clean error, not a successful sync"),
            Err(e) => e,
        };
        let message = err.to_string();
        assert!(
            message.contains("detached HEAD"),
            "expected the error to mention detached HEAD, got: {message}"
        );
    }

    #[test]
    fn no_remote_configured_is_a_graceful_noop() {
        let dir = tempdir().unwrap();
        vault::ensure_git_repo(dir.path()).unwrap();
        fs::write(dir.path().join("page.md"), "---\nid: p1\n---\nHello.\n").unwrap();
        vault::commit_all(dir.path(), "Create page").unwrap();

        let outcome = run_sync(dir.path()).unwrap();
        assert_eq!(outcome.status, SyncStatus::NoRemote);
        assert!(!outcome.index_rebuild_needed);
    }

    #[test]
    fn no_remote_and_no_commits_yet_is_also_a_graceful_noop() {
        let dir = tempdir().unwrap();
        vault::ensure_git_repo(dir.path()).unwrap();

        let outcome = run_sync(dir.path()).unwrap();
        assert_eq!(outcome.status, SyncStatus::NoRemote);
    }

    #[test]
    fn fast_forward_pull_brings_in_remote_changes_without_conflict() {
        let (bare_dir, device_a_dir, branch) = setup_remote_and_first_device();
        let device_b_dir = clone_second_device(&bare_dir);

        // Device A makes and pushes a further change; device B hasn't
        // touched anything locally, so this should be a clean fast-forward.
        fs::write(device_a_dir.path().join("page.md"), "---\nid: p1\n---\nUpdated by A.\n").unwrap();
        vault::commit_all(device_a_dir.path(), "Update page").unwrap();
        let repo_a = git2::Repository::open(device_a_dir.path()).unwrap();
        let mut remote_a = repo_a.find_remote("origin").unwrap();
        remote_a
            .push(&[format!("refs/heads/{branch}:refs/heads/{branch}").as_str()], None)
            .unwrap();

        let outcome = run_sync(device_b_dir.path()).unwrap();

        assert_eq!(outcome.status, SyncStatus::Synced);
        assert!(outcome.index_rebuild_needed);
        let content = fs::read_to_string(device_b_dir.path().join("page.md")).unwrap();
        assert!(content.contains("Updated by A."));
    }

    #[test]
    fn local_only_commits_are_pushed_when_remote_has_nothing_new() {
        let (bare_dir, _device_a_dir, branch) = setup_remote_and_first_device();
        let device_b_dir = clone_second_device(&bare_dir);

        fs::write(
            device_b_dir.path().join("page.md"),
            "---\nid: p1\n---\nUpdated by B.\n",
        )
        .unwrap();
        vault::commit_all(device_b_dir.path(), "Update page").unwrap();

        let outcome = run_sync(device_b_dir.path()).unwrap();
        assert_eq!(outcome.status, SyncStatus::Synced);
        assert!(!outcome.index_rebuild_needed);

        let bare = git2::Repository::open_bare(bare_dir.path()).unwrap();
        let bare_branch = bare
            .find_reference(&format!("refs/heads/{branch}"))
            .unwrap()
            .target()
            .unwrap();
        let device_b_head = git2::Repository::open(device_b_dir.path())
            .unwrap()
            .head()
            .unwrap()
            .target()
            .unwrap();
        assert_eq!(bare_branch, device_b_head);
    }

    #[test]
    fn diverging_edits_to_the_same_file_are_detected_as_a_conflict_and_backed_up_before_any_merge_touches_them(
    ) {
        let (bare_dir, device_a_dir, branch) = setup_remote_and_first_device();
        let device_b_dir = clone_second_device(&bare_dir);

        // Device A edits and pushes.
        fs::write(device_a_dir.path().join("page.md"), "---\nid: p1\n---\nFrom device A.\n").unwrap();
        vault::commit_all(device_a_dir.path(), "Edit from A").unwrap();
        let repo_a = git2::Repository::open(device_a_dir.path()).unwrap();
        let mut remote_a = repo_a.find_remote("origin").unwrap();
        remote_a
            .push(&[format!("refs/heads/{branch}:refs/heads/{branch}").as_str()], None)
            .unwrap();

        // Device B, without ever syncing, edits the very same line/file
        // differently.
        fs::write(device_b_dir.path().join("page.md"), "---\nid: p1\n---\nFrom device B.\n").unwrap();
        vault::commit_all(device_b_dir.path(), "Edit from B").unwrap();
        let device_b_head_before = git2::Repository::open(device_b_dir.path())
            .unwrap()
            .head()
            .unwrap()
            .target()
            .unwrap();

        let outcome = run_sync(device_b_dir.path()).unwrap();

        match &outcome.status {
            SyncStatus::NeedsAttention { detail } => assert!(detail.contains("page.md")),
            other => panic!("expected NeedsAttention, got {other:?}"),
        }
        assert!(!outcome.index_rebuild_needed);

        // The working file is untouched -- still device B's local version --
        // and HEAD never moved, i.e. no merge/checkout ran against it.
        let content = fs::read_to_string(device_b_dir.path().join("page.md")).unwrap();
        assert_eq!(content, "---\nid: p1\n---\nFrom device B.\n");
        let device_b_head_after = git2::Repository::open(device_b_dir.path())
            .unwrap()
            .head()
            .unwrap()
            .target()
            .unwrap();
        assert_eq!(device_b_head_before, device_b_head_after);

        // A backup of device B's local version exists under
        // .cerebrite/conflict-backups/, saved before the (aborted) merge.
        let backup_dir = device_b_dir.path().join(CONFLICT_BACKUP_DIR_REL);
        let entries: Vec<_> = fs::read_dir(&backup_dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let backup_path = entries[0].as_ref().unwrap().path();
        assert!(backup_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("page.md.local-backup-"));
        let backup_content = fs::read_to_string(&backup_path).unwrap();
        assert_eq!(backup_content, "---\nid: p1\n---\nFrom device B.\n");
    }
}
