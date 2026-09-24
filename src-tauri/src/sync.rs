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

use crate::connection::Connection;
use crate::connection_record::CredentialKind;

/// Relative path (from the vault root) of the conflict-backup directory,
/// mirroring the `.cerebrite/trash/` convention from ticket 10.
pub const CONFLICT_BACKUP_DIR_REL: &str = ".cerebrite/conflict-backups";

/// The specific cause behind a `Transient` or `NeedsAttention` `SyncStatus`
/// (ticket 03 / ADR-0012). Kept as an enum rather than a flat string so a
/// future UI (ticket 12/13) can pick a message and CTA per cause -- "your
/// GitLab sign-in was rejected" vs. "trying again..." -- without
/// re-deriving that distinction from prose, and without `SyncStatus`
/// needing another shape change once that UI lands.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "cause", rename_all = "camelCase")]
pub enum SyncFailureCause {
    /// A network/transport-class failure: DNS, connect, TLS, or timeout.
    /// Likely to resolve itself on the next automatic retry, so this is
    /// the one cause that lands in `SyncStatus::Transient` rather than
    /// `NeedsAttention`.
    NetworkUnreachable { detail: String },
    /// The connection's one credential was rejected by the remote.
    /// `credential_kind` names *which* kind was in use when the remote
    /// rejected it (ticket 04: an expired/rejected access token must
    /// surface identifiably as "access token", not a generic message) --
    /// `None` only when no `Connection` was available to classify against
    /// (e.g. `classify_git_error` called directly in a test, or a fetch
    /// failure that happens before any credential is even attempted).
    CredentialRejected {
        detail: String,
        credential_kind: Option<CredentialKind>,
    },
    /// A push was rejected because it was not a fast-forward on the
    /// remote -- someone else pushed first.
    NonFastForwardPush { detail: String },
    /// A merge conflict was detected. Kept here (alongside the network/auth
    /// causes) so every "not now, and here's specifically why" reason is
    /// one `SyncFailureCause`, even though conflict handling itself
    /// (backing up the user's local file) is unrelated to credentials.
    Conflict { detail: String },
    /// Any other git2/libgit2 failure, or a non-git problem (e.g. a
    /// detached HEAD) that isn't one of the above.
    Other { detail: String },
    /// An SSH host presented a key that's never been trusted before (not
    /// GitHub's/GitLab's pinned key either, ticket 05) -- not a broken
    /// connection, but it always blocks until the user explicitly confirms
    /// the fingerprint via `ssh_host_keys::KnownHosts::confirm`. Sync itself
    /// never confirms one on its own.
    HostKeyUnconfirmed { host: String, fingerprint: String },
    /// An SSH host's key no longer matches what was pinned (GitHub/GitLab)
    /// or previously confirmed (TOFU) for it -- ticket 05 checklist item 6,
    /// the textbook host-key-changed/MITM signature. Unlike
    /// `HostKeyUnconfirmed`, re-trusting this host needs the same explicit
    /// confirmation as first contact, never an automatic retry.
    HostKeyMismatch { host: String, fingerprint: String },
    /// An OAuth sign-in connection's access token was rejected *and* this
    /// connection has no refresh token to renew it with (ticket 07: GitLab's
    /// device grant may not hand one back at all -- one of the three facts
    /// the still-pending live spike must confirm; see
    /// `gitlab_oauth::refresh_if_needed`'s doc comment). Distinct from the
    /// generic `CredentialRejected` so a future UI can point the user
    /// straight at signing in again rather than implying the same
    /// credential might work on retry -- the fix here is never "try again",
    /// it's "reconnect".
    OauthReconnectRequired {
        detail: String,
        credential_kind: CredentialKind,
    },
}

impl std::fmt::Display for SyncFailureCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncFailureCause::NetworkUnreachable { detail } => write!(f, "network problem: {detail}"),
            SyncFailureCause::CredentialRejected { detail, credential_kind } => match credential_kind {
                Some(kind) => write!(f, "{} rejected: {detail}", credential_kind_label(*kind)),
                None => write!(f, "credential rejected: {detail}"),
            },
            SyncFailureCause::NonFastForwardPush { detail } => write!(f, "push rejected: {detail}"),
            SyncFailureCause::Conflict { detail } => write!(f, "conflict: {detail}"),
            SyncFailureCause::Other { detail } => write!(f, "{detail}"),
            SyncFailureCause::HostKeyUnconfirmed { host, fingerprint } => write!(
                f,
                "unknown SSH host key for {host} ({fingerprint}) -- confirm this fingerprint before connecting"
            ),
            SyncFailureCause::HostKeyMismatch { host, fingerprint } => write!(
                f,
                "SSH host key for {host} no longer matches what was trusted (now {fingerprint}) -- refusing to connect until re-confirmed"
            ),
            SyncFailureCause::OauthReconnectRequired { detail, credential_kind } => write!(
                f,
                "{} expired and can't be refreshed automatically -- reconnect required: {detail}",
                credential_kind_label(*credential_kind)
            ),
        }
    }
}

/// A human-facing name for a credential kind, used to make a rejected
/// credential's `SyncFailureCause` identifiable (ticket 04) rather than a
/// generic "credential rejected" message.
fn credential_kind_label(kind: CredentialKind) -> &'static str {
    match kind {
        CredentialKind::AccessToken => "access token",
        CredentialKind::SshKey => "SSH key",
        CredentialKind::OauthSignIn => "sign-in",
    }
}

impl SyncFailureCause {
    /// Whether this cause belongs in `SyncStatus::Transient` (worth
    /// retrying automatically) rather than `SyncStatus::NeedsAttention`
    /// (the user has to do something first). Only a network-class problem
    /// is transient -- everything else (a rejected credential, a
    /// non-fast-forward push, a conflict) needs a person or a different
    /// credential before retrying would help.
    fn is_transient(&self) -> bool {
        matches!(self, SyncFailureCause::NetworkUnreachable { .. })
    }

    /// Wraps this cause in whichever `SyncStatus` variant it belongs in.
    pub fn into_status(self) -> SyncStatus {
        if self.is_transient() {
            SyncStatus::Transient { cause: self }
        } else {
            SyncStatus::NeedsAttention { cause: self }
        }
    }
}

/// Classifies a `git2::Error` from a fetch or push into a `SyncFailureCause`
/// (ticket 03 checklist item 4). Network-class errors (DNS, connect, TLS,
/// timeout) are `NetworkUnreachable`; a rejected credential is
/// `CredentialRejected`; a non-fast-forward push is `NonFastForwardPush`;
/// anything else falls back to `Other`, still keeping the original detail
/// so a cause can be named more specifically later without re-plumbing.
pub fn classify_git_error(err: &git2::Error) -> SyncFailureCause {
    let detail = err.message().to_string();
    match err.code() {
        git2::ErrorCode::Auth => {
            return SyncFailureCause::CredentialRejected { detail, credential_kind: None }
        }
        git2::ErrorCode::NotFastForward => return SyncFailureCause::NonFastForwardPush { detail },
        git2::ErrorCode::Timeout => return SyncFailureCause::NetworkUnreachable { detail },
        _ => {}
    }
    match err.class() {
        git2::ErrorClass::Net | git2::ErrorClass::Ssh | git2::ErrorClass::Ssl | git2::ErrorClass::Os => {
            SyncFailureCause::NetworkUnreachable { detail }
        }
        git2::ErrorClass::Http => {
            // libgit2's smart-HTTP client doesn't always surface a
            // credential-rejection HTTP status (401/403) as `ErrorCode::Auth`
            // -- older/odd server behavior sometimes leaves it a generic
            // HTTP-class error instead. Treat those as rejected credentials
            // rather than network trouble: the connection *was* reached.
            let lower = detail.to_lowercase();
            if detail.contains("401")
                || lower.contains("unauthorized")
                || detail.contains("403")
                || lower.contains("authentication")
            {
                SyncFailureCause::CredentialRejected { detail, credential_kind: None }
            } else {
                SyncFailureCause::Other { detail }
            }
        }
        _ => SyncFailureCause::Other { detail },
    }
}

/// Classifies a git2 error the same way `classify_git_error` does, but when
/// the result is a rejected credential and `connection` is known, tags it
/// with which credential kind was in use -- so a needs-attention failure can
/// name it ("access token rejected: ...") instead of a generic message
/// (ticket 04 checklist: "surfaces ... as a needs-attention failure naming
/// this credential kind").
///
/// Ticket 05: also checks `connection`'s `last_host_key_check` *first* --
/// when an SSH connection's `certificate_check` rejected the host key
/// (unconfirmed or mismatched), the resulting `git2::Error`'s own message is
/// useless for telling those two cases apart (libgit2 overwrites it with a
/// fixed "invalid or unknown remote ssh hostkey" string regardless of what
/// the callback set -- see `connection.rs`'s `check_ssh_host_key` doc
/// comment), so the structured cause has to come from that side channel
/// instead of `err` at all. Falls through to the ordinary classification
/// below whenever host-key checking wasn't what failed (a non-SSH
/// connection, or an SSH one where the host key itself checked out fine and
/// something else -- credentials, network -- failed afterward).
pub fn classify_git_error_for(err: &git2::Error, connection: Option<&Connection>) -> SyncFailureCause {
    if let Some(cause) = connection.and_then(host_key_failure_cause) {
        return cause;
    }
    match (classify_git_error(err), connection) {
        (SyncFailureCause::CredentialRejected { detail, .. }, Some(connection)) => {
            let kind = connection.credential_kind();
            if kind == CredentialKind::OauthSignIn && !oauth_connection_is_refreshable(connection) {
                return SyncFailureCause::OauthReconnectRequired { detail, credential_kind: kind };
            }
            SyncFailureCause::CredentialRejected {
                detail,
                credential_kind: Some(kind),
            }
        }
        (cause, _) => cause,
    }
}

/// Whether `connection` (an `OauthSignIn` connection) has a refresh token
/// to renew itself with -- ticket 07's defensive dual path. GitHub
/// connections (ticket 06) always carry a mandatory refresh token by
/// construction, so this is only ever meaningfully `false` for a GitLab
/// connection whose device grant never returned one. A corrupt/unparsable
/// stored secret defaults to `true` (i.e. an ordinary `CredentialRejected`
/// rather than `OauthReconnectRequired`) -- that failure mode is already
/// reported elsewhere (`connection::credential_for_kind`'s own error), and
/// this classifier shouldn't invent a second, different-shaped story for
/// the same underlying problem.
fn oauth_connection_is_refreshable(connection: &Connection) -> bool {
    use crate::connection_record::Provider;
    match &connection.record().provider {
        Provider::GitLab => crate::gitlab_oauth::OauthSecret::from_bytes(connection.secret_bytes())
            .map(|secret| secret.is_refreshable())
            .unwrap_or(true),
        // GitHub (ticket 06) and any other provider: always treated as
        // refreshable (GitHub always issues a refresh token for its
        // device-flow tokens).
        _ => true,
    }
}

/// Recovers a `SyncFailureCause` from `connection`'s
/// `last_host_key_check` (ticket 05), if the most recent
/// `certificate_check` invocation rejected the host key. `None` when there
/// was no SSH host-key check at all, or it passed (pinned/previously
/// confirmed) and whatever actually failed happened afterward -- that case
/// falls through to ordinary `git2::Error` classification instead.
fn host_key_failure_cause(connection: &Connection) -> Option<SyncFailureCause> {
    let outcome = connection.last_host_key_check()?;
    use crate::ssh_host_keys::HostKeyCheck;
    match outcome.check {
        HostKeyCheck::PinnedMismatch | HostKeyCheck::KnownMismatch { .. } => Some(SyncFailureCause::HostKeyMismatch {
            host: outcome.host,
            fingerprint: outcome.fingerprint,
        }),
        HostKeyCheck::Unknown => Some(SyncFailureCause::HostKeyUnconfirmed {
            host: outcome.host,
            fingerprint: outcome.fingerprint,
        }),
        HostKeyCheck::PinnedMatch | HostKeyCheck::KnownMatch => None,
    }
}

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
    /// The last sync attempt hit a failure likely to resolve itself on the
    /// next automatic retry (ticket 03 / ADR-0012) -- a network-class
    /// problem, not something the user needs to act on (yet).
    Transient { cause: SyncFailureCause },
    /// The last sync attempt hit a failure that will keep failing until the
    /// user does something -- a rejected credential, a non-fast-forward
    /// push, or a merge conflict. Surfaced so the UI can show something
    /// other than a silently stale "Synced".
    NeedsAttention { cause: SyncFailureCause },
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

/// Builds callbacks for a fetch/push attempt that has no `Connection` at
/// all (no connection is configured for this vault). This registers *no*
/// credentials callback -- ADR-0012 deleted the old
/// agent/credential-helper/`Cred::default()` chain outright, it was not
/// reordered or kept as a further fallback. A transport that needs
/// credentials without one configured fails cleanly; a transport that
/// needs none (a local bare-repo path, the case this module's own tests
/// use) is unaffected.
fn callbacks_without_connection<'a>() -> git2::RemoteCallbacks<'a> {
    git2::RemoteCallbacks::new()
}

fn push_current_branch(
    remote: &mut git2::Remote,
    branch: &str,
    connection: Option<&Connection>,
    known_hosts_path: Option<&Path>,
) -> Result<(), git2::Error> {
    let callbacks = connection
        .map(|c| c.make_callbacks(known_hosts_path))
        .unwrap_or_else(callbacks_without_connection);
    let mut push_opts = git2::PushOptions::new();
    push_opts.remote_callbacks(callbacks);
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    remote.push(&[refspec.as_str()], Some(&mut push_opts))
}

/// A test fetch against `remote_url` using `connection`'s credential and
/// nothing else, run *without* opening any local repository. Used by
/// `connection::try_connect` to prove a credential works before anything is
/// persisted -- ticket 03 checklist: "No connection is persisted until a
/// test fetch using its credential succeeds."
pub fn test_fetch(
    remote_url: &str,
    connection: &Connection,
    known_hosts_path: Option<&Path>,
) -> Result<(), SyncFailureCause> {
    let mut remote =
        git2::Remote::create_detached(remote_url).map_err(|e| classify_git_error(&e))?;
    let callbacks = connection.make_callbacks(known_hosts_path);
    remote
        .connect_auth(git2::Direction::Fetch, Some(callbacks), None)
        .map_err(|e| classify_git_error_for(&e, Some(connection)))?;
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
/// `vault_path`'s `origin` remote, authenticating with `connection`'s one
/// credential kind and nothing else (ADR-0012) -- `None` when no connection
/// is configured for this vault, which is fine for a transport that needs
/// no credentials (a local bare-repo path, as in this module's own tests)
/// and fails cleanly for one that does. A no-op (`SyncStatus::NoRemote`)
/// when no `origin` remote is configured, or when the vault has no commits
/// yet.
pub fn run_sync(
    vault_path: &Path,
    connection: Option<&Connection>,
    known_hosts_path: Option<&Path>,
) -> Result<SyncOutcome> {
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

    let fetch_callbacks = connection
        .map(|c| c.make_callbacks(known_hosts_path))
        .unwrap_or_else(callbacks_without_connection);
    let mut fetch_opts = git2::FetchOptions::new();
    fetch_opts.remote_callbacks(fetch_callbacks);
    // Empty refspec list: use whatever fetch refspec is already configured on
    // the remote (the standard `+refs/heads/*:refs/remotes/origin/*`, set up
    // automatically by `git2::Repository::remote`/`clone`), same as a plain
    // `git fetch`.
    if let Err(e) = remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None) {
        return Ok(SyncOutcome {
            status: classify_git_error_for(&e, connection).into_status(),
            index_rebuild_needed: false,
        });
    }

    let remote_ref_name = format!("refs/remotes/origin/{branch}");
    let remote_ref = match repo.find_reference(&remote_ref_name) {
        Ok(r) => r,
        Err(_) => {
            // The remote has no such branch yet -- nothing to merge, just
            // push to create it.
            if let Err(e) = push_current_branch(&mut remote, &branch, connection, known_hosts_path) {
                return Ok(SyncOutcome {
                    status: classify_git_error_for(&e, connection).into_status(),
                    index_rebuild_needed: false,
                });
            }
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
        if let Err(e) = push_current_branch(&mut remote, &branch, connection, known_hosts_path) {
            return Ok(SyncOutcome {
                status: classify_git_error_for(&e, connection).into_status(),
                index_rebuild_needed: false,
            });
        }
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
            status: SyncStatus::NeedsAttention {
                cause: SyncFailureCause::Conflict { detail },
            },
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

    if let Err(e) = push_current_branch(&mut remote, &branch, connection, known_hosts_path) {
        // The merge already landed locally (HEAD and the working tree were
        // updated above) -- only the push failed, so this still needs an
        // index rebuild even though the push itself didn't succeed.
        return Ok(SyncOutcome {
            status: classify_git_error_for(&e, connection).into_status(),
            index_rebuild_needed: true,
        });
    }

    Ok(SyncOutcome {
        status: SyncStatus::Synced,
        index_rebuild_needed: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::Connection;
    use crate::connection_record::{ConnectionRecord, CredentialKind, Provider, StoreKind};
    use crate::vault;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use tempfile::tempdir;

    // -- classify_git_error --

    #[test]
    fn auth_error_classifies_as_credential_rejected_needs_attention() {
        let err = git2::Error::new(git2::ErrorCode::Auth, git2::ErrorClass::Http, "authentication required");

        let cause = classify_git_error(&err);

        assert!(matches!(cause, SyncFailureCause::CredentialRejected { .. }));
        assert!(matches!(cause.into_status(), SyncStatus::NeedsAttention { .. }));
    }

    #[test]
    fn network_class_error_classifies_as_transient() {
        let err = git2::Error::new(git2::ErrorCode::GenericError, git2::ErrorClass::Net, "could not connect to host");

        let cause = classify_git_error(&err);

        assert!(matches!(cause, SyncFailureCause::NetworkUnreachable { .. }));
        assert!(matches!(cause.into_status(), SyncStatus::Transient { .. }));
    }

    #[test]
    fn timeout_error_classifies_as_transient() {
        let err = git2::Error::new(git2::ErrorCode::Timeout, git2::ErrorClass::Net, "timed out");

        assert!(matches!(classify_git_error(&err), SyncFailureCause::NetworkUnreachable { .. }));
    }

    #[test]
    fn non_fast_forward_error_classifies_as_needs_attention() {
        let err = git2::Error::new(
            git2::ErrorCode::NotFastForward,
            git2::ErrorClass::Reference,
            "non-fast-forward update rejected",
        );

        let cause = classify_git_error(&err);

        assert!(matches!(cause, SyncFailureCause::NonFastForwardPush { .. }));
        assert!(matches!(cause.into_status(), SyncStatus::NeedsAttention { .. }));
    }

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

        let result = run_sync(dir.path(), None, None);

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

        let outcome = run_sync(dir.path(), None, None).unwrap();
        assert_eq!(outcome.status, SyncStatus::NoRemote);
        assert!(!outcome.index_rebuild_needed);
    }

    #[test]
    fn no_remote_and_no_commits_yet_is_also_a_graceful_noop() {
        let dir = tempdir().unwrap();
        vault::ensure_git_repo(dir.path()).unwrap();

        let outcome = run_sync(dir.path(), None, None).unwrap();
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

        let outcome = run_sync(device_b_dir.path(), None, None).unwrap();

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

        let outcome = run_sync(device_b_dir.path(), None, None).unwrap();
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

        let outcome = run_sync(device_b_dir.path(), None, None).unwrap();

        match &outcome.status {
            SyncStatus::NeedsAttention {
                cause: SyncFailureCause::Conflict { detail },
            } => assert!(detail.contains("page.md")),
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

    // -- ticket 03 checklist: rejected credential -> NeedsAttention,
    // unreachable remote -> Transient, exercised through `run_sync` itself
    // rather than only unit-testing `classify_git_error`. --

    fn access_token_connection(secret: &str) -> Connection {
        Connection::from_parts(
            ConnectionRecord {
                connection_id: "test-conn".to_string(),
                credential_kind: CredentialKind::AccessToken,
                provider: Provider::Other("test-host".to_string()),
                https_username: Some("git".to_string()),
                token_expiry: None,
                credential_store: StoreKind::Plaintext,
            },
            secret.as_bytes().to_vec(),
        )
    }

    /// Binds a TCP port and immediately drops the listener, so the port is
    /// guaranteed closed (`ECONNREFUSED` on connect) without depending on
    /// real network access -- used to simulate an unreachable remote.
    fn unreachable_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    }

    /// A minimal HTTP server that answers every request with a bare `401
    /// Unauthorized`, standing in for a real git host rejecting a
    /// credential over the smart-HTTP transport -- run for a bounded number
    /// of connections (libgit2 retries a handful of times before giving up)
    /// so the test thread always exits rather than blocking on `accept`
    /// forever.
    fn spawn_401_server() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(20) {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 4096];
                // Best-effort: read whatever the client sent so far (don't
                // block forever waiting for a body that never comes).
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(
                    b"HTTP/1.1 401 Unauthorized\r\n\
                      WWW-Authenticate: Basic realm=\"cerebrite-test\"\r\n\
                      Content-Length: 0\r\n\
                      Connection: close\r\n\
                      \r\n",
                );
            }
        });
        port
    }

    #[test]
    fn unreachable_remote_lands_in_transient() {
        let dir = tempdir().unwrap();
        vault::ensure_git_repo(dir.path()).unwrap();
        fs::write(dir.path().join("page.md"), "---\nid: p1\n---\nHello.\n").unwrap();
        vault::commit_all(dir.path(), "Create page").unwrap();

        let repo = git2::Repository::open(dir.path()).unwrap();
        let port = unreachable_port();
        repo.remote("origin", &format!("http://127.0.0.1:{port}/repo.git")).unwrap();

        let outcome = run_sync(dir.path(), None, None).unwrap();

        match outcome.status {
            SyncStatus::Transient { cause } => {
                assert!(matches!(cause, SyncFailureCause::NetworkUnreachable { .. }))
            }
            other => panic!("expected Transient, got {other:?}"),
        }
    }

    #[test]
    fn rejected_credential_lands_in_needs_attention() {
        let dir = tempdir().unwrap();
        vault::ensure_git_repo(dir.path()).unwrap();
        fs::write(dir.path().join("page.md"), "---\nid: p1\n---\nHello.\n").unwrap();
        vault::commit_all(dir.path(), "Create page").unwrap();

        let repo = git2::Repository::open(dir.path()).unwrap();
        let port = spawn_401_server();
        repo.remote("origin", &format!("http://127.0.0.1:{port}/repo.git")).unwrap();

        let connection = access_token_connection("invalid-token");
        let outcome = run_sync(dir.path(), Some(&connection), None).unwrap();

        match outcome.status {
            SyncStatus::NeedsAttention { cause } => {
                assert!(
                    matches!(cause, SyncFailureCause::CredentialRejected { .. }),
                    "expected CredentialRejected, got {cause:?}"
                )
            }
            other => panic!("expected NeedsAttention, got {other:?}"),
        }
    }

    // -- ticket 04: a rejected credential names its kind, not just "credential
    // rejected" -- exercised through `run_sync` with a real access-token
    // `Connection`, mirroring `rejected_credential_lands_in_needs_attention`
    // above.

    #[test]
    fn rejected_access_token_names_its_credential_kind() {
        let dir = tempdir().unwrap();
        vault::ensure_git_repo(dir.path()).unwrap();
        fs::write(dir.path().join("page.md"), "---\nid: p1\n---\nHello.\n").unwrap();
        vault::commit_all(dir.path(), "Create page").unwrap();

        let repo = git2::Repository::open(dir.path()).unwrap();
        let port = spawn_401_server();
        repo.remote("origin", &format!("http://127.0.0.1:{port}/repo.git")).unwrap();

        let connection = access_token_connection("invalid-token");
        let outcome = run_sync(dir.path(), Some(&connection), None).unwrap();

        match outcome.status {
            SyncStatus::NeedsAttention { cause } => {
                match &cause {
                    SyncFailureCause::CredentialRejected { credential_kind, .. } => {
                        assert_eq!(*credential_kind, Some(CredentialKind::AccessToken));
                    }
                    other => panic!("expected CredentialRejected, got {other:?}"),
                }
                assert!(
                    cause.to_string().starts_with("access token rejected:"),
                    "expected the message to name the credential kind, got: {cause}"
                );
            }
            other => panic!("expected NeedsAttention, got {other:?}"),
        }
    }

    // -- ticket 07: a GitLab OAuth sign-in connection with no refresh token
    // surfaces a rejected credential as `OauthReconnectRequired`, not a
    // generic `CredentialRejected` -- the defensive dual path for the
    // still-unverified "does GitLab's device grant return a refresh token"
    // fact. A GitLab connection that *does* have a refresh token is treated
    // exactly like any other rejected credential (an ordinary
    // `CredentialRejected`), since a stale/revoked refresh token is a
    // different problem than "there was never anything to refresh with".

    fn gitlab_oauth_connection(access_token: &str, refresh_token: Option<&str>) -> Connection {
        let secret = crate::gitlab_oauth::OauthSecret {
            access_token: access_token.to_string(),
            refresh_token: refresh_token.map(str::to_string),
        };
        Connection::from_parts(
            ConnectionRecord {
                connection_id: "test-conn".to_string(),
                credential_kind: CredentialKind::OauthSignIn,
                provider: Provider::GitLab,
                https_username: Some(crate::gitlab_oauth::GITLAB_HTTPS_USERNAME.to_string()),
                token_expiry: None,
                credential_store: StoreKind::Plaintext,
            },
            secret.to_bytes(),
        )
    }

    #[test]
    fn a_rejected_gitlab_oauth_connection_with_no_refresh_token_surfaces_as_reconnect_required() {
        let dir = tempdir().unwrap();
        vault::ensure_git_repo(dir.path()).unwrap();
        fs::write(dir.path().join("page.md"), "---\nid: p1\n---\nHello.\n").unwrap();
        vault::commit_all(dir.path(), "Create page").unwrap();

        let repo = git2::Repository::open(dir.path()).unwrap();
        let port = spawn_401_server();
        repo.remote("origin", &format!("http://127.0.0.1:{port}/repo.git")).unwrap();

        let connection = gitlab_oauth_connection("expired-token", None);
        let outcome = run_sync(dir.path(), Some(&connection), None).unwrap();

        match outcome.status {
            SyncStatus::NeedsAttention { cause } => {
                match &cause {
                    SyncFailureCause::OauthReconnectRequired { credential_kind, .. } => {
                        assert_eq!(*credential_kind, CredentialKind::OauthSignIn);
                    }
                    other => panic!("expected OauthReconnectRequired, got {other:?}"),
                }
                assert!(
                    cause.to_string().contains("reconnect required"),
                    "expected the message to call out reconnecting, got: {cause}"
                );
            }
            other => panic!("expected NeedsAttention, got {other:?}"),
        }
    }

    #[test]
    fn a_rejected_gitlab_oauth_connection_with_a_refresh_token_is_an_ordinary_credential_rejection() {
        let dir = tempdir().unwrap();
        vault::ensure_git_repo(dir.path()).unwrap();
        fs::write(dir.path().join("page.md"), "---\nid: p1\n---\nHello.\n").unwrap();
        vault::commit_all(dir.path(), "Create page").unwrap();

        let repo = git2::Repository::open(dir.path()).unwrap();
        let port = spawn_401_server();
        repo.remote("origin", &format!("http://127.0.0.1:{port}/repo.git")).unwrap();

        // A refresh token is present, but stale/revoked (that's a separate
        // problem from "there was never one to try") -- rejection here is
        // an ordinary `CredentialRejected`, not `OauthReconnectRequired`.
        let connection = gitlab_oauth_connection("expired-token", Some("glrt_stale"));
        let outcome = run_sync(dir.path(), Some(&connection), None).unwrap();

        match outcome.status {
            SyncStatus::NeedsAttention { cause } => {
                assert!(
                    matches!(cause, SyncFailureCause::CredentialRejected { .. }),
                    "expected CredentialRejected, got {cause:?}"
                )
            }
            other => panic!("expected NeedsAttention, got {other:?}"),
        }
    }
}
