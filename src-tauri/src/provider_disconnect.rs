// Disconnect & credential revocation (ticket 14, implementing the decision
// recorded in
// `.scratch/git-provider-integration/issues/07-credential-storage-decision.md`'s
// "Revocation and clearing" section).
//
// `disconnect_connection` deletes the three on-disk artifacts a vault's
// connection leaves behind: the stored secret (whichever store
// `ConnectionRecord::credential_store` names), `.git/cerebrite/connection.json`
// (`connection_record::delete`), and -- best effort -- the git `origin`
// remote itself. Removing `origin` too (not just the credential) is what
// makes a subsequent sync attempt report `SyncStatus::NoRemote` rather than
// an unauthenticated-fetch failure against a remote that's still configured
// but now has no credential backing it (ticket 14 checklist's last item);
// `settings.json`'s index entry is the caller's responsibility to clear
// (`settings::remove_connection`), since that needs an `AppHandle` this
// module deliberately stays free of, matching `connection.rs`/`sync.rs`'s
// precedent of keeping the Tauri-agnostic core separate from the command
// layer in `lib.rs`.
//
// Revocation *at the provider* is the other half, and ticket 14's core trust
// constraint governs it: this module never reports a revocation happened
// unless it actually did.
// - **GitLab** OAuth sign-in connections with a refresh token: revoked via
//   `gitlab_oauth::revoke_token` (`POST /oauth/revoke`, a public-client call
//   needing no secret) *before* the secret is deleted below -- the refresh
//   token has to still be readable to revoke it. `DisconnectOutcome::gitlab_revoked`
//   is `Some(true)` only once GitLab actually confirmed it with a 2xx.
// - **GitHub** App installations, and plain **access tokens**/**SSH keys**,
//   need action at the provider Cerebrite cannot take on the user's behalf
//   (GitHub App revocation needs a client secret Cerebrite deliberately
//   doesn't hold; there is no revoke-by-Cerebrite mechanism for a bare
//   access token or an SSH key at all) -- this module does nothing for
//   those beyond the local deletions, and the frontend's Disconnect dialog
//   copy (`main.ts`) is responsible for linking the user to the right
//   provider page rather than ever implying a revocation happened.
#![allow(dead_code)]

use std::path::Path;

use anyhow::Context;
use serde::Serialize;

use crate::connection_record::{self, ConnectionRecord, CredentialKind, Provider, StoreKind};
use crate::credential::{CallUrgency, KeychainBackend, PlaintextStore};
use crate::gitlab_oauth::{self, GitLabEndpoints};

/// What a `disconnect_connection` call actually did -- enough for the
/// frontend to render honest, per-kind Disconnect-dialog copy without ever
/// having to *infer* whether a revocation happened (ticket 14's core trust
/// constraint: never claim one unless it's true).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectOutcome {
    pub connection_id: String,
    pub credential_kind: CredentialKind,
    pub provider: Provider,
    /// `Some(true)`: this was a GitLab OAuth sign-in connection with a
    /// refresh token, and GitLab's `/oauth/revoke` confirmed it (2xx).
    /// `Some(false)`: same, but GitLab couldn't be reached, rejected the
    /// call, or the stored secret/refresh token couldn't be read in the
    /// first place -- the token may still be valid at GitLab, and the
    /// frontend must say so, never "revoked".
    /// `None`: not applicable -- any other credential kind/provider, or a
    /// GitLab OAuth connection that never had a refresh token to revoke to
    /// begin with (ticket 07's defensive dual path).
    pub gitlab_revoked: Option<bool>,
}

/// Deletes `repo_root`'s connection: the stored secret, the connection
/// record, and -- best effort -- the `origin` remote (see this module's doc
/// comment). `Ok(None)` if there was no connection configured for this repo
/// at all (nothing to do -- not an error, mirrors `connection_record::read`'s
/// `Ok(None)`). The caller still owns clearing `settings.json`'s index entry
/// via `settings::remove_connection` once this returns `Ok(Some(_))`.
///
/// Code-review follow-up (ticket 14): deleting the stored secret used to be
/// `let _ = ...`'d away, so a keychain that refused the delete (locked,
/// permission denied, daemon unreachable) still resulted in `Ok` -- the
/// caller reported "disconnected" while the credential was, in fact, still
/// sitting in the store. That's now a hard error, returned *before* the
/// connection record is deleted: the record staying in place is what makes
/// the failure recoverable (the user can just try Disconnect again) rather
/// than leaving an orphaned secret with nothing left referencing it.
/// Removing the `origin` remote remains best-effort past that point -- a
/// vault whose `origin` can't be removed (wrong permissions, a corrupt
/// `.git`) still ends up fully disconnected credential-wise; it just won't
/// report `NoRemote` on its very next sync attempt the way a normally
/// removable remote would.
pub fn disconnect_connection(
    repo_root: &Path,
    keychain: Option<&KeychainBackend>,
    plaintext: &PlaintextStore,
    gitlab_endpoints: &GitLabEndpoints,
) -> anyhow::Result<Option<DisconnectOutcome>> {
    let Some(record) = connection_record::read(repo_root)? else {
        return Ok(None);
    };

    // GitLab revoke first -- needs the secret to still be in its store.
    let gitlab_revoked = maybe_revoke_gitlab(&record, keychain, plaintext, gitlab_endpoints);

    delete_stored_secret(&record, keychain, plaintext)
        .with_context(|| format!("deleting stored secret for connection {}", record.connection_id))?;

    connection_record::delete(repo_root)?;

    // Best effort: a vault whose `origin` can't be removed (unlikely --
    // wrong permissions, a corrupt .git) still ends up fully disconnected
    // credential-wise; it just won't report `NoRemote` on its very next
    // sync attempt the way a normally-removable remote would.
    if let Ok(repo) = git2::Repository::open(repo_root) {
        let _ = repo.remote_delete("origin");
    }

    Ok(Some(DisconnectOutcome {
        connection_id: record.connection_id,
        credential_kind: record.credential_kind,
        provider: record.provider,
        gitlab_revoked,
    }))
}

/// Ticket 14 checklist item 2: attempts GitLab's `/oauth/revoke` for a
/// GitLab OAuth sign-in connection's refresh token. A no-op (`None`) for any
/// other kind/provider. Returns `Some(false)` (not `None`) for a GitLab
/// OAuth connection whose secret/refresh token couldn't be read -- ticket
/// 14's honesty rule cuts both ways: it's also wrong to stay silent about an
/// *attempted* revocation that couldn't even get as far as calling GitLab.
fn maybe_revoke_gitlab(
    record: &ConnectionRecord,
    keychain: Option<&KeychainBackend>,
    plaintext: &PlaintextStore,
    endpoints: &GitLabEndpoints,
) -> Option<bool> {
    if record.credential_kind != CredentialKind::OauthSignIn || record.provider != Provider::GitLab {
        return None;
    }
    let secret_bytes = match record.credential_store {
        StoreKind::Keychain => keychain.and_then(|kc| kc.get_secret(&record.connection_id, CallUrgency::Interactive).ok()),
        StoreKind::Plaintext => plaintext.get_secret(&record.connection_id).ok(),
    };
    let Some(secret_bytes) = secret_bytes else {
        return Some(false);
    };
    let Ok(secret) = gitlab_oauth::OauthSecret::from_bytes(&secret_bytes) else {
        return Some(false);
    };
    // No refresh token was ever issued (ticket 07's defensive dual path) --
    // there is nothing to revoke, which is not the same thing as a failed
    // revocation attempt.
    let Some(refresh_token) = secret.refresh_token else {
        return None;
    };
    Some(gitlab_oauth::revoke_token(endpoints, gitlab_oauth::GITLAB_CLIENT_ID, &refresh_token).is_ok())
}

/// Deletes `record`'s stored secret. A hard error (not swallowed -- see
/// `disconnect_connection`'s doc comment) so a keychain that refuses the
/// delete is never mistaken for one that succeeded. `StoreKind::Keychain`
/// with no reachable `keychain` backend is itself an error, for the same
/// reason: the secret is presumably still there, unreachable, not gone.
fn delete_stored_secret(
    record: &ConnectionRecord,
    keychain: Option<&KeychainBackend>,
    plaintext: &PlaintextStore,
) -> anyhow::Result<()> {
    match record.credential_store {
        StoreKind::Keychain => keychain
            .ok_or_else(|| anyhow::anyhow!("no keychain backend is available"))?
            .delete_secret(&record.connection_id, CallUrgency::Interactive)
            .map_err(anyhow::Error::from),
        StoreKind::Plaintext => plaintext.delete_secret(&record.connection_id),
    }
}

/// Ticket 14 checklist item 6's cleanup action: deletes a stored secret
/// directly by connection id and store kind -- used for orphaned entries
/// whose repository no longer exists on disk, where there is no
/// `connection.json` left to read the way `disconnect_connection` needs.
/// Also used by "Remove all stored Cerebrite credentials" (checklist item 7)
/// for whichever of its entries turn out to already be orphaned.
///
/// Code-review follow-up (ticket 14): returns the outcome instead of
/// swallowing it, for the same reason `delete_stored_secret` does -- a
/// caller that treats this as always-successful can end up removing the
/// `settings.json` index entry (the only remaining record of the secret's
/// existence) while the secret itself is still sitting in the store.
pub fn delete_orphaned_secret(
    keychain: Option<&KeychainBackend>,
    plaintext: &PlaintextStore,
    connection_id: &str,
    store: StoreKind,
) -> anyhow::Result<()> {
    match store {
        StoreKind::Keychain => keychain
            .ok_or_else(|| anyhow::anyhow!("no keychain backend is available"))?
            .delete_secret(connection_id, CallUrgency::Interactive)
            .map_err(anyhow::Error::from),
        StoreKind::Plaintext => plaintext.delete_secret(connection_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credential::KeychainBackend;
    use tempfile::tempdir;

    fn mock_endpoints(port: u16) -> GitLabEndpoints {
        let base = format!("http://127.0.0.1:{port}");
        GitLabEndpoints {
            device_code_url: base.clone(),
            token_url: base.clone(),
            api_base_url: base.clone(),
            revoke_url: base,
        }
    }

    fn unreachable_endpoints() -> GitLabEndpoints {
        // Ordinary offline stand-in: bind then immediately drop, so the
        // port is guaranteed closed.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        mock_endpoints(port)
    }

    fn local_bare_remote(repo_dir: &Path) -> tempfile::TempDir {
        crate::vault::ensure_git_repo(repo_dir).unwrap();
        crate::author::confirm_test_author(repo_dir);
        std::fs::write(repo_dir.join("page.md"), "---\nid: p1\n---\nHello.\n").unwrap();
        crate::vault::commit_all(repo_dir, "Create page").unwrap();

        let bare_dir = tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();
        let repo = git2::Repository::open(repo_dir).unwrap();
        repo.remote("origin", bare_dir.path().to_str().unwrap()).unwrap();
        let mut remote = repo.find_remote("origin").unwrap();
        let branch = repo.head().unwrap().shorthand().unwrap().to_string();
        remote
            .push(&[format!("refs/heads/{branch}:refs/heads/{branch}").as_str()], None)
            .unwrap();
        bare_dir
    }

    fn access_token_record(store: StoreKind) -> ConnectionRecord {
        ConnectionRecord {
            connection_id: "conn-1".to_string(),
            credential_kind: CredentialKind::AccessToken,
            provider: Provider::Other("test-host".to_string()),
            https_username: Some("dawid".to_string()),
            token_expiry: None,
            credential_store: store,
        }
    }

    // -- disconnect_connection: the three-artifact deletion (checklist item 1/8) --

    #[test]
    fn disconnect_connection_is_a_noop_returning_none_when_nothing_is_configured() {
        let repo_dir = tempdir().unwrap();
        crate::vault::ensure_git_repo(repo_dir.path()).unwrap();
        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());
        let endpoints = mock_endpoints(0);

        let result = disconnect_connection(repo_dir.path(), None, &plaintext, &endpoints).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn disconnect_connection_removes_the_secret_the_record_and_the_origin_remote() {
        let repo_dir = tempdir().unwrap();
        let _bare = local_bare_remote(repo_dir.path());

        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());
        let record = access_token_record(StoreKind::Plaintext);
        plaintext.set_secret(&record.connection_id, b"a-token").unwrap();
        connection_record::write(repo_dir.path(), &record).unwrap();

        let endpoints = mock_endpoints(0);
        let outcome = disconnect_connection(repo_dir.path(), None, &plaintext, &endpoints)
            .unwrap()
            .expect("a connection was configured");

        assert_eq!(outcome.connection_id, "conn-1");
        assert_eq!(outcome.credential_kind, CredentialKind::AccessToken);
        assert_eq!(outcome.gitlab_revoked, None);

        // All three artifacts are gone.
        assert!(plaintext.get_secret("conn-1").is_err());
        assert_eq!(connection_record::read(repo_dir.path()).unwrap(), None);
        let repo = git2::Repository::open(repo_dir.path()).unwrap();
        assert!(repo.find_remote("origin").is_err());
    }

    #[test]
    fn disconnect_connection_removes_a_keychain_backed_secret_too() {
        let repo_dir = tempdir().unwrap();
        let _bare = local_bare_remote(repo_dir.path());

        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());
        let keychain = KeychainBackend::in_memory();
        let record = access_token_record(StoreKind::Keychain);
        keychain
            .set_secret(&record.connection_id, b"a-token", CallUrgency::Interactive)
            .unwrap();
        connection_record::write(repo_dir.path(), &record).unwrap();

        let endpoints = mock_endpoints(0);
        disconnect_connection(repo_dir.path(), Some(&keychain), &plaintext, &endpoints)
            .unwrap()
            .unwrap();

        assert!(keychain.get_secret("conn-1", CallUrgency::Interactive).is_err());
    }

    // -- code-review follow-up (ticket 14): a failed secret deletion must be
    // a hard error, not swallowed -- and must leave the connection record
    // (and the secret) in place so the failure is recoverable.

    #[test]
    fn disconnect_connection_fails_and_leaves_the_record_and_secret_in_place_when_the_keychain_is_unreachable() {
        let repo_dir = tempdir().unwrap();
        let _bare = local_bare_remote(repo_dir.path());

        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());
        // A Keychain-store record with no keychain backend at all --
        // `delete_stored_secret` can't even attempt the delete, which must
        // surface as an error rather than a silent no-op success.
        let record = access_token_record(StoreKind::Keychain);
        connection_record::write(repo_dir.path(), &record).unwrap();

        let endpoints = mock_endpoints(0);
        let result = disconnect_connection(repo_dir.path(), None, &plaintext, &endpoints);

        assert!(result.is_err(), "expected disconnect to fail, but it reported success");
        // The record must still be there -- otherwise a retry has nothing
        // left to disconnect, even though the (unreachable, still-live)
        // secret was never actually deleted.
        assert_eq!(connection_record::read(repo_dir.path()).unwrap(), Some(record));
        // And the origin remote -- the last step -- must not have run either.
        let repo = git2::Repository::open(repo_dir.path()).unwrap();
        assert!(repo.find_remote("origin").is_ok());
    }

    #[test]
    fn delete_orphaned_secret_reports_failure_instead_of_claiming_success() {
        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());

        let result = delete_orphaned_secret(None, &plaintext, "conn-1", StoreKind::Keychain);

        assert!(result.is_err());
    }

    // -- checklist item 8: a subsequent sync reports NoRemote, not a stale
    // credential error -- exercised via `sync::run_sync` directly against
    // the exact repo/remote `disconnect_connection` just tore down.

    #[test]
    fn a_sync_attempt_after_disconnect_reports_no_remote_not_a_stale_credential_error() {
        let repo_dir = tempdir().unwrap();
        let _bare = local_bare_remote(repo_dir.path());

        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());
        let record = access_token_record(StoreKind::Plaintext);
        plaintext.set_secret(&record.connection_id, b"a-token").unwrap();
        connection_record::write(repo_dir.path(), &record).unwrap();

        let endpoints = mock_endpoints(0);
        disconnect_connection(repo_dir.path(), None, &plaintext, &endpoints).unwrap();

        // Mirrors `lib.rs`'s `perform_sync`: no connection record left, so
        // no `Connection` is loaded and `run_sync` is called with `None`.
        assert_eq!(connection_record::read(repo_dir.path()).unwrap(), None);
        let outcome = crate::sync::run_sync(repo_dir.path(), None, None).unwrap();
        assert_eq!(outcome.status, crate::sync::SyncStatus::NoRemote);
    }

    // -- GitLab revocation honesty (checklist items 2/5) --

    #[test]
    fn disconnect_connection_reports_gitlab_revoked_true_on_a_confirmed_2xx() {
        let repo_dir = tempdir().unwrap();
        let _bare = local_bare_remote(repo_dir.path());
        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());

        let mut record = access_token_record(StoreKind::Plaintext);
        record.credential_kind = CredentialKind::OauthSignIn;
        record.provider = Provider::GitLab;
        let secret = gitlab_oauth::OauthSecret {
            access_token: "glpat_abc".to_string(),
            refresh_token: Some("glrt_def".to_string()),
        };
        plaintext.set_secret(&record.connection_id, &secret.to_bytes()).unwrap();
        connection_record::write(repo_dir.path(), &record).unwrap();

        let port = mock_server_spawn(vec![(200, String::new())]);
        let endpoints = mock_endpoints(port);

        let outcome = disconnect_connection(repo_dir.path(), None, &plaintext, &endpoints)
            .unwrap()
            .unwrap();

        assert_eq!(outcome.gitlab_revoked, Some(true));
    }

    #[test]
    fn disconnect_connection_reports_gitlab_revoked_false_never_true_when_gitlab_rejects_it() {
        let repo_dir = tempdir().unwrap();
        let _bare = local_bare_remote(repo_dir.path());
        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());

        let mut record = access_token_record(StoreKind::Plaintext);
        record.credential_kind = CredentialKind::OauthSignIn;
        record.provider = Provider::GitLab;
        let secret = gitlab_oauth::OauthSecret {
            access_token: "glpat_abc".to_string(),
            refresh_token: Some("glrt_def".to_string()),
        };
        plaintext.set_secret(&record.connection_id, &secret.to_bytes()).unwrap();
        connection_record::write(repo_dir.path(), &record).unwrap();

        let port = mock_server_spawn(vec![(400, r#"{"error":"invalid_request"}"#.to_string())]);
        let endpoints = mock_endpoints(port);

        let outcome = disconnect_connection(repo_dir.path(), None, &plaintext, &endpoints)
            .unwrap()
            .unwrap();

        assert_eq!(outcome.gitlab_revoked, Some(false));
        // The local artifacts are still fully torn down even though the
        // remote revocation failed -- Disconnect must always complete
        // locally regardless of GitLab's answer.
        assert!(plaintext.get_secret(&record.connection_id).is_err());
        assert_eq!(connection_record::read(repo_dir.path()).unwrap(), None);
    }

    #[test]
    fn disconnect_connection_reports_gitlab_revoked_false_when_gitlab_is_unreachable() {
        let repo_dir = tempdir().unwrap();
        let _bare = local_bare_remote(repo_dir.path());
        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());

        let mut record = access_token_record(StoreKind::Plaintext);
        record.credential_kind = CredentialKind::OauthSignIn;
        record.provider = Provider::GitLab;
        let secret = gitlab_oauth::OauthSecret {
            access_token: "glpat_abc".to_string(),
            refresh_token: Some("glrt_def".to_string()),
        };
        plaintext.set_secret(&record.connection_id, &secret.to_bytes()).unwrap();
        connection_record::write(repo_dir.path(), &record).unwrap();

        let endpoints = unreachable_endpoints();

        let outcome = disconnect_connection(repo_dir.path(), None, &plaintext, &endpoints)
            .unwrap()
            .unwrap();

        assert_eq!(outcome.gitlab_revoked, Some(false));
    }

    #[test]
    fn disconnect_connection_reports_gitlab_revoked_none_when_there_was_no_refresh_token() {
        // Ticket 07's defensive dual path: GitLab's device grant may never
        // have issued one -- there is nothing to revoke, which must not be
        // reported the same way as a failed revocation attempt.
        let repo_dir = tempdir().unwrap();
        let _bare = local_bare_remote(repo_dir.path());
        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());

        let mut record = access_token_record(StoreKind::Plaintext);
        record.credential_kind = CredentialKind::OauthSignIn;
        record.provider = Provider::GitLab;
        let secret = gitlab_oauth::OauthSecret {
            access_token: "glpat_abc".to_string(),
            refresh_token: None,
        };
        plaintext.set_secret(&record.connection_id, &secret.to_bytes()).unwrap();
        connection_record::write(repo_dir.path(), &record).unwrap();

        // No mock server at all is spawned -- proves revoke is never even
        // attempted for this branch.
        let endpoints = unreachable_endpoints();

        let outcome = disconnect_connection(repo_dir.path(), None, &plaintext, &endpoints)
            .unwrap()
            .unwrap();

        assert_eq!(outcome.gitlab_revoked, None);
    }

    #[test]
    fn disconnect_connection_never_attempts_gitlab_revoke_for_a_github_connection() {
        let repo_dir = tempdir().unwrap();
        let _bare = local_bare_remote(repo_dir.path());
        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());

        let mut record = access_token_record(StoreKind::Plaintext);
        record.credential_kind = CredentialKind::OauthSignIn;
        record.provider = Provider::GitHub;
        let secret = crate::github_oauth::OauthSecret {
            access_token: "gho_abc".to_string(),
            refresh_token: "ghr_def".to_string(),
        };
        plaintext.set_secret(&record.connection_id, &secret.to_bytes()).unwrap();
        connection_record::write(repo_dir.path(), &record).unwrap();

        let endpoints = unreachable_endpoints();

        let outcome = disconnect_connection(repo_dir.path(), None, &plaintext, &endpoints)
            .unwrap()
            .unwrap();

        assert_eq!(outcome.gitlab_revoked, None);
        assert_eq!(outcome.provider, Provider::GitHub);
    }

    // -- delete_orphaned_secret (checklist item 6) --

    #[test]
    fn delete_orphaned_secret_removes_a_plaintext_secret_with_no_repo_involved() {
        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());
        plaintext.set_secret("conn-orphan", b"leftover").unwrap();

        delete_orphaned_secret(None, &plaintext, "conn-orphan", StoreKind::Plaintext).unwrap();

        assert!(plaintext.get_secret("conn-orphan").is_err());
    }

    #[test]
    fn delete_orphaned_secret_removes_a_keychain_secret_with_no_repo_involved() {
        let config_dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(config_dir.path());
        let keychain = KeychainBackend::in_memory();
        keychain
            .set_secret("conn-orphan", b"leftover", CallUrgency::Interactive)
            .unwrap();

        delete_orphaned_secret(Some(&keychain), &plaintext, "conn-orphan", StoreKind::Keychain).unwrap();

        assert!(keychain.get_secret("conn-orphan", CallUrgency::Interactive).is_err());
    }

    // A tiny copy of `gitlab_oauth`'s private test `mock_server::spawn`,
    // since that module's test helper isn't `pub` -- same hand-rolled
    // HTTP/1.1 responder, never touches real GitLab.
    fn mock_server_spawn(responses: Vec<(u16, String)>) -> u16 {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let counter = Arc::new(AtomicUsize::new(0));
        std::thread::spawn(move || {
            for stream in listener.incoming().take(responses.len().max(1) + 5) {
                let Ok(mut stream) = stream else { continue };
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf).unwrap_or(0);

                let idx = counter.fetch_add(1, Ordering::SeqCst);
                let Some((status, body)) = responses.get(idx) else {
                    break;
                };
                let response = format!(
                    "HTTP/1.1 {status} Status\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        port
    }
}
