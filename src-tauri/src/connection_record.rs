// Non-secret connection record (ADR-0013 / ticket 02): everything about a
// vault's connection except the secret itself. Lives at
// `.git/cerebrite/connection.json`, inside the repository's git dir --
// never committed (nothing under `.git/` ever is, since git doesn't track
// its own metadata directory), never picked up by a fresh clone, and moves
// with the folder if the user renames or relocates it.
//
// Deliberately *not* `.cerebrite/` at the repo root, which vault.rs and
// ADR-0011 already use for tracked, git-synced state (the redirect log,
// the trash folder): a connection record must never travel to a second
// device via git, so it lives inside `.git/` instead.
//
// Per ticket 11 (which amended the original decision in
// `.scratch/git-provider-integration/issues/07-credential-storage-decision.md`),
// the vault's commit author does NOT live here -- it lives in repo-local
// git config (`user.name` / `user.email`), because a vault with no
// connection at all still needs an author, and a connection's lifetime
// (connect/disconnect) must not affect it. Do not add an author field to
// this struct.
//
// No credential *kind* is wired up by this ticket -- tickets 04 (access
// token), 05 (SSH key) and 06/07 (OAuth sign-in) do that. This is just the
// shape the record can hold, and the read/write/delete plumbing for it.
//
// `write`/`read`/`delete` have no caller yet -- wiring a real connection
// into a vault starts at ticket 04 -- so silence the dead-code warning
// deliberately rather than leaving the noise; remove once a caller exists.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const CONNECTION_SUBDIR: &str = "cerebrite";
const CONNECTION_FILE: &str = "connection.json";

/// How a connection authenticates -- ADR-0012's three credential kinds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    OauthSignIn,
    AccessToken,
    SshKey,
}

/// The provider `origin` points at. `Other` covers any HTTPS host that
/// isn't GitHub or GitLab -- ADR-0012's access-token kind works against
/// "any HTTPS host" (Gitea, Forgejo, Codeberg, Bitbucket, ...).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "host")]
pub enum Provider {
    GitHub,
    GitLab,
    Other(String),
}

/// Which credential store currently holds this connection's secret
/// (ADR-0013). Also indexed app-wide in `settings.json` -- see
/// `settings::Settings::connections` -- so it can be discovered without
/// opening every vault.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum StoreKind {
    Keychain,
    Plaintext,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRecord {
    pub connection_id: String,
    pub credential_kind: CredentialKind,
    pub provider: Provider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub https_username: Option<String>,
    /// RFC 3339 timestamp; absent for kinds with no expiring token (SSH
    /// key, or an access token the host never expires).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_expiry: Option<String>,
    pub credential_store: StoreKind,
}

fn record_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".git").join(CONNECTION_SUBDIR).join(CONNECTION_FILE)
}

/// Writes (or overwrites) `repo_root`'s connection record.
pub fn write(repo_root: &Path, record: &ConnectionRecord) -> Result<()> {
    let path = record_path(repo_root);
    let dir = path.parent().expect("record_path always has a parent");
    fs::create_dir_all(dir).context("creating .git/cerebrite")?;
    let json = serde_json::to_string_pretty(record).context("serializing connection record")?;
    fs::write(&path, json).context("writing connection record")?;
    Ok(())
}

/// Reads `repo_root`'s connection record, if any. `Ok(None)` covers both
/// "never connected" and "disconnected" (ticket 14 deletes this file via
/// `delete` below).
pub fn read(repo_root: &Path) -> Result<Option<ConnectionRecord>> {
    let path = record_path(repo_root);
    match fs::read_to_string(&path) {
        Ok(content) => {
            let record = serde_json::from_str(&content).context("parsing connection record")?;
            Ok(Some(record))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).context("reading connection record"),
    }
}

/// Deletes `repo_root`'s connection record, if it exists (ticket 14's
/// Disconnect). A no-op if there is none.
pub fn delete(repo_root: &Path) -> Result<()> {
    let path = record_path(repo_root);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).context("removing connection record"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_record() -> ConnectionRecord {
        ConnectionRecord {
            connection_id: "11111111-1111-1111-1111-111111111111".to_string(),
            credential_kind: CredentialKind::AccessToken,
            provider: Provider::GitHub,
            https_username: Some("dawid".to_string()),
            token_expiry: None,
            credential_store: StoreKind::Keychain,
        }
    }

    #[test]
    fn read_returns_none_when_no_record_exists() {
        let dir = tempdir().unwrap();
        assert_eq!(read(dir.path()).unwrap(), None);
    }

    #[test]
    fn write_then_read_round_trips() {
        let dir = tempdir().unwrap();
        let record = sample_record();

        write(dir.path(), &record).unwrap();

        assert_eq!(read(dir.path()).unwrap(), Some(record));
    }

    #[test]
    fn record_lives_under_dot_git_not_the_tracked_dot_cerebrite() {
        let dir = tempdir().unwrap();
        write(dir.path(), &sample_record()).unwrap();

        assert!(dir.path().join(".git/cerebrite/connection.json").exists());
        assert!(!dir.path().join(".cerebrite").exists());
    }

    #[test]
    fn write_overwrites_an_existing_record() {
        let dir = tempdir().unwrap();
        write(dir.path(), &sample_record()).unwrap();

        let mut updated = sample_record();
        updated.credential_store = StoreKind::Plaintext;
        write(dir.path(), &updated).unwrap();

        assert_eq!(read(dir.path()).unwrap(), Some(updated));
    }

    #[test]
    fn delete_removes_the_record_and_is_a_noop_if_absent() {
        let dir = tempdir().unwrap();
        write(dir.path(), &sample_record()).unwrap();

        delete(dir.path()).unwrap();
        assert_eq!(read(dir.path()).unwrap(), None);

        // Deleting again must not error.
        delete(dir.path()).unwrap();
    }

    #[test]
    fn serialized_record_has_no_author_field() {
        // Ticket 11 moved author identity out of the connection record and
        // into repo-local git config. Guard against it creeping back in.
        let json = serde_json::to_string(&sample_record()).unwrap();
        assert!(!json.to_lowercase().contains("author"));
    }
}
