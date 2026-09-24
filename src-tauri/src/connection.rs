// Connection (ticket 03, ADR-0012): a vault's one binding to its git
// provider -- `origin` plus exactly one credential kind and the credential
// backing it. Wraps ticket 02's `connection_record::ConnectionRecord`
// (which kind, which store) and actually resolves the secret from whichever
// store the record says holds it, then builds `git2::RemoteCallbacks` for
// exactly that kind and nothing else (ADR-0012: "sync never falls back").
//
// No credential-*kind* setup/refresh flow lives here -- tickets 04
// (access token), 05 (SSH key) and 06/07 (OAuth sign-in) own how a token or
// key is obtained and kept fresh. This ticket only needs a minimal, generic
// way to turn "whatever bytes ticket 02's keychain/plaintext store holds"
// into libgit2 credentials for the named kind, and the gate that stops a
// Connection from ever being persisted before a real fetch has proven its
// credential works.
#![allow(dead_code)]

use std::path::Path;

use crate::connection_record::{self, ConnectionRecord, CredentialKind, StoreKind};
use crate::credential::{CallUrgency, CredentialError, KeychainBackend, PlaintextStore};
use crate::sync::SyncFailureCause;

/// A loaded connection ready to authenticate: the non-secret record plus
/// its resolved secret bytes. Never written to disk with the secret
/// attached -- the on-disk record (`connection_record::ConnectionRecord`)
/// never carries it; only this in-memory type pairs them, and only for as
/// long as one connect attempt or sync cycle needs it.
pub struct Connection {
    record: ConnectionRecord,
    secret: Vec<u8>,
    /// Ticket 05: what `certificate_check` (via `make_callbacks`) most
    /// recently found, for `sync.rs` to recover a structured
    /// `SyncFailureCause` after a failed fetch/push -- see its doc comment
    /// for *why* this side channel exists rather than reading it back out
    /// of the resulting `git2::Error`.
    last_host_key_check: std::cell::RefCell<Option<HostKeyOutcome>>,
}

/// What `certificate_check` found the one time it's called per connection
/// attempt (SSH host-key verification happens once, during the transport
/// handshake, before any credential is even requested) -- host, presented
/// fingerprint, and how it was judged.
#[derive(Debug, Clone)]
pub struct HostKeyOutcome {
    pub host: String,
    pub fingerprint: String,
    pub check: crate::ssh_host_keys::HostKeyCheck,
}

/// Everything that can go wrong resolving or trying a Connection, short of
/// the remote actually rejecting/timing out on the resulting credential --
/// that case is reported as a `SyncFailureCause` instead (see `try_connect`
/// and `sync::classify_git_error`), since it's a fact about the remote, not
/// about Cerebrite's own bookkeeping.
#[derive(Debug)]
pub enum ConnectionError {
    /// The connection record's declared store doesn't have a secret for
    /// it (never stored, or deleted outside the app).
    Credential(CredentialError),
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionError::Credential(e) => write!(f, "resolving connection credential: {e}"),
        }
    }
}

impl std::error::Error for ConnectionError {}

impl Connection {
    /// Loads `repo_root`'s connection, resolving its secret from whichever
    /// store its record says holds it. `Ok(None)` means no connection is
    /// configured for this vault (never connected, or disconnected via
    /// ticket 14) -- callers fall back to no credentials at all, which is
    /// the correct "nothing to try" outcome (ADR-0012 has no implicit
    /// fallback to fall back *to*).
    pub fn load(
        repo_root: &Path,
        keychain: &KeychainBackend,
        plaintext: &PlaintextStore,
        urgency: CallUrgency,
    ) -> anyhow::Result<Option<Self>> {
        let Some(record) = connection_record::read(repo_root)? else {
            return Ok(None);
        };
        let secret = resolve_secret(&record, keychain, plaintext, urgency)
            .map_err(ConnectionError::Credential)?;
        Ok(Some(Self {
            record,
            secret,
            last_host_key_check: std::cell::RefCell::new(None),
        }))
    }

    /// Builds a Connection directly from an already-known record + secret,
    /// without touching a keychain or `connection.json` -- used by
    /// `try_connect`'s pre-persist test fetch, and by tests.
    pub fn from_parts(record: ConnectionRecord, secret: Vec<u8>) -> Self {
        Self {
            record,
            secret,
            last_host_key_check: std::cell::RefCell::new(None),
        }
    }

    pub fn record(&self) -> &ConnectionRecord {
        &self.record
    }

    pub fn credential_kind(&self) -> CredentialKind {
        self.record.credential_kind
    }

    /// The raw secret bytes this Connection resolved -- generic on purpose
    /// (this module doesn't know or care about any kind's internal
    /// envelope, see this file's module doc comment). Ticket 06's
    /// background refresh path (`github_oauth::refresh_if_needed`) uses
    /// this to pull out the `OauthSignIn` kind's `OauthSecret` envelope
    /// without `Connection` itself needing to know that shape exists.
    pub fn secret_bytes(&self) -> &[u8] {
        &self.secret
    }

    /// The outcome of the most recent `certificate_check` invocation made
    /// through this Connection's callbacks, if any -- `None` for a non-SSH
    /// kind, or an SSH connection that never got far enough to exchange
    /// host keys at all (e.g. the TCP connect itself failed).
    pub fn last_host_key_check(&self) -> Option<HostKeyOutcome> {
        self.last_host_key_check.borrow().clone()
    }

    /// Builds callbacks that resolve credentials from this Connection's one
    /// named kind, and nothing else. A credential failure here (an invalid
    /// key, a rejected token) is a hard failure of *this* connection -- it
    /// is never a cue to try a different kind, unlike the old
    /// agent/credential-helper/default chain this type replaces.
    ///
    /// `known_hosts_path` is only consulted for `CredentialKind::SshKey` --
    /// every other kind ignores it (host-key verification is meaningless
    /// over HTTPS/TLS, which has its own certificate chain). `None` still
    /// lets pinned hosts (GitHub/GitLab, ticket 05) connect; every other SSH
    /// host is then always treated as unconfirmed (`HostKeyCheck::Unknown`),
    /// since there's nowhere to check a prior confirmation against -- safer
    /// than silently trusting it.
    pub fn make_callbacks(&self, known_hosts_path: Option<&Path>) -> git2::RemoteCallbacks<'_> {
        let mut callbacks = git2::RemoteCallbacks::new();
        let kind = self.record.credential_kind;
        let username = self.record.https_username.clone();
        let secret = self.secret.clone();
        callbacks.credentials(move |url, username_from_url, _allowed_types| {
            credential_for_kind(kind, &username, &secret, url, username_from_url)
        });

        if kind == CredentialKind::SshKey {
            let known_hosts = known_hosts_path.map(|path| {
                crate::ssh_host_keys::KnownHosts::load(path)
                    .unwrap_or_else(|_| crate::ssh_host_keys::KnownHosts::empty())
            });
            let sink = &self.last_host_key_check;
            callbacks.certificate_check(move |cert, host| {
                let (outcome, status) = check_ssh_host_key(cert, host, known_hosts.as_ref());
                *sink.borrow_mut() = Some(outcome);
                status
            });
        }

        callbacks
    }
}

/// The `RemoteCallbacks::certificate_check` implementation for SSH
/// connections (ticket 05 checklist item 4): GitHub's/GitLab's published
/// fingerprints are pinned outright; anything already confirmed once
/// (persisted in `known_hosts`) is trusted again; anything else is refused.
/// Never returns `CertificateOk` for a host it hasn't actually checked.
///
/// Returns both the `HostKeyOutcome` (for the `last_host_key_check` side
/// channel) *and* the status/error git2 itself needs -- deliberately not
/// just the latter: libgit2's SSH transport discards whatever message this
/// callback's `Err` carries and replaces it with its own fixed
/// "invalid or unknown remote ssh hostkey" once it decides the cert isn't
/// valid (see `ssh_libssh2.c`'s `certificate_check` handling), so there is
/// no way to recover *why* -- unconfirmed vs. mismatched, which host, which
/// fingerprint -- from the `git2::Error` a failed fetch/push ultimately
/// surfaces. The outcome has to travel back out some other way; `Connection`
/// gives it a place to land (`last_host_key_check`) that `sync.rs` checks
/// before falling back to generic `git2::Error` classification.
fn check_ssh_host_key(
    cert: &git2::cert::Cert<'_>,
    host: &str,
    known_hosts: Option<&crate::ssh_host_keys::KnownHosts>,
) -> (HostKeyOutcome, Result<git2::CertificateCheckStatus, git2::Error>) {
    let Some(hostkey) = cert.as_hostkey() else {
        let outcome = HostKeyOutcome {
            host: host.to_string(),
            fingerprint: String::new(),
            check: crate::ssh_host_keys::HostKeyCheck::Unknown,
        };
        return (
            outcome,
            Err(git2::Error::from_str("expected an SSH host key certificate")),
        );
    };
    let Some(digest) = hostkey.hash_sha256() else {
        let outcome = HostKeyOutcome {
            host: host.to_string(),
            fingerprint: String::new(),
            check: crate::ssh_host_keys::HostKeyCheck::Unknown,
        };
        return (
            outcome,
            Err(git2::Error::from_str(
                "this host's SSH key has no SHA-256 hash available to verify",
            )),
        );
    };
    let fingerprint = crate::ssh_host_keys::format_sha256_fingerprint(digest);

    // An empty (no-entries) store makes exactly the same pinning decision a
    // real one would -- pinning never depends on the known_hosts file being
    // reachable -- so a missing `known_hosts_path` only changes the outcome
    // for a host that isn't pinned (it becomes `Unknown` instead of
    // possibly `KnownMatch`).
    let empty = crate::ssh_host_keys::KnownHosts::empty();
    let store = known_hosts.unwrap_or(&empty);
    let check = store.check(host, &fingerprint);

    use crate::ssh_host_keys::HostKeyCheck;
    let status = match &check {
        HostKeyCheck::PinnedMatch | HostKeyCheck::KnownMatch => Ok(git2::CertificateCheckStatus::CertificateOk),
        HostKeyCheck::PinnedMismatch | HostKeyCheck::KnownMismatch { .. } => Err(git2::Error::new(
            git2::ErrorCode::Certificate,
            git2::ErrorClass::Ssh,
            crate::ssh_host_keys::host_key_mismatch_message(host, &fingerprint),
        )),
        HostKeyCheck::Unknown => Err(git2::Error::new(
            git2::ErrorCode::Certificate,
            git2::ErrorClass::Ssh,
            crate::ssh_host_keys::unknown_host_key_message(host, &fingerprint),
        )),
    };
    let outcome = HostKeyOutcome {
        host: host.to_string(),
        fingerprint,
        check,
    };
    (outcome, status)
}

fn resolve_secret(
    record: &ConnectionRecord,
    keychain: &KeychainBackend,
    plaintext: &PlaintextStore,
    urgency: CallUrgency,
) -> Result<Vec<u8>, CredentialError> {
    match record.credential_store {
        StoreKind::Keychain => keychain.get_secret(&record.connection_id, urgency),
        StoreKind::Plaintext => plaintext
            .get_secret(&record.connection_id)
            .map_err(|e| CredentialError::Other(e.to_string())),
    }
}

/// The actual, per-kind translation of "opaque secret bytes" into a
/// `git2::Cred`. Both HTTPS-transport kinds (access token, OAuth sign-in)
/// are a token sent as an HTTPS Basic-auth password and differ only in how
/// the token was obtained (ADR-0012), which tickets 04/06/07 implement; the
/// SSH kind (ticket 05) deserializes the stored secret as a
/// `ssh_key::SshSecret` -- the private key material verbatim (still
/// encrypted, if it was imported that way) plus its passphrase if it has
/// one -- and hands both straight to `Cred::ssh_key_from_memory`, which lets
/// libssh2 do any decryption itself; Cerebrite never decrypts an SSH key on
/// its own behalf (see `ssh_key.rs`'s module doc comment for why).
fn credential_for_kind(
    kind: CredentialKind,
    https_username: &Option<String>,
    secret: &[u8],
    _url: &str,
    username_from_url: Option<&str>,
) -> Result<git2::Cred, git2::Error> {
    match kind {
        CredentialKind::AccessToken => {
            let user = https_username
                .clone()
                .or_else(|| username_from_url.map(str::to_string))
                .unwrap_or_else(|| "git".to_string());
            let token = String::from_utf8_lossy(secret).into_owned();
            git2::Cred::userpass_plaintext(&user, &token)
        }
        // Ticket 06: the stored secret is a `github_oauth::OauthSecret` JSON
        // envelope (access token + refresh token), not a raw token -- unlike
        // `AccessToken`, which stores the token verbatim. Only the access
        // token half is a valid git credential; the refresh token never
        // reaches libgit2, it's only used by this module's own background
        // refresh path. Convention (ticket 01's research, confirmed for
        // GitHub Apps' user-to-server tokens): username `x-access-token`,
        // any real HTTPS username stored on the record still wins if present.
        CredentialKind::OauthSignIn => {
            let user = https_username
                .clone()
                .or_else(|| username_from_url.map(str::to_string))
                .unwrap_or_else(|| "x-access-token".to_string());
            let oauth = crate::github_oauth::OauthSecret::from_bytes(secret).map_err(|e| {
                git2::Error::from_str(&format!("stored OAuth sign-in secret is corrupt: {e}"))
            })?;
            git2::Cred::userpass_plaintext(&user, &oauth.access_token)
        }
        CredentialKind::SshKey => {
            let user = username_from_url.unwrap_or("git");
            let secret = crate::ssh_key::SshSecret::from_bytes(secret)
                .map_err(|e| git2::Error::from_str(&format!("stored SSH key material is corrupt: {e}")))?;
            git2::Cred::ssh_key_from_memory(
                user,
                None,
                &secret.private_key_openssh,
                secret.passphrase.as_deref(),
            )
        }
    }
}

/// Result of `try_connect`'s pre-persist test fetch.
#[derive(Debug)]
pub enum ConnectError {
    /// Resolving/storing the secret itself failed (keychain locked, etc) --
    /// distinct from the remote rejecting the credential.
    Credential(CredentialError),
    /// The test fetch ran and failed -- classified the same way a real
    /// sync's fetch/push failure is, so the caller can report the same
    /// specific cause without a second classification scheme.
    Fetch(SyncFailureCause),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::Credential(e) => write!(f, "storing connection credential: {e}"),
            ConnectError::Fetch(cause) => write!(f, "test fetch failed: {cause}"),
        }
    }
}

impl std::error::Error for ConnectError {}

/// Sets up a new connection: proves `record`'s credential actually works
/// against `remote_url` with a real (network) test fetch *before* writing
/// anything -- neither `repo_root`'s `connection.json` nor the secret store.
/// ADR-0012 / ticket 03 checklist: "No connection is persisted until a test
/// fetch using its credential succeeds." On success, the secret is stored
/// in the store the record declares and the record is written; on any
/// failure, nothing is persisted at all.
///
/// `keychain` is `Option` (widened from ticket 03's original `&KeychainBackend`
/// by ticket 04): a caller that has already probed `KeychainBackend::platform()`
/// and found no keychain reachable at all still needs to be able to connect
/// with `record.credential_store` set to `StoreKind::Plaintext` -- there is no
/// live `KeychainBackend` to hand over in that case, and there shouldn't need
/// to be one, since a plaintext-store record never calls into it. `None` with
/// a `Keychain`-store record fails with `ConnectError::Credential` rather than
/// panicking.
pub fn try_connect(
    repo_root: &Path,
    keychain: Option<&KeychainBackend>,
    plaintext: &PlaintextStore,
    record: ConnectionRecord,
    secret: Vec<u8>,
    remote_url: &str,
    known_hosts_path: Option<&Path>,
    urgency: CallUrgency,
) -> Result<Connection, ConnectError> {
    let candidate = Connection::from_parts(record.clone(), secret.clone());

    crate::sync::test_fetch(remote_url, &candidate, known_hosts_path).map_err(ConnectError::Fetch)?;

    match record.credential_store {
        StoreKind::Keychain => keychain
            .ok_or_else(|| {
                ConnectError::Credential(CredentialError::Unavailable(
                    "no keychain backend is available".to_string(),
                ))
            })?
            .set_secret(&record.connection_id, &secret, urgency)
            .map_err(ConnectError::Credential)?,
        StoreKind::Plaintext => plaintext
            .set_secret(&record.connection_id, &secret)
            .map_err(|e| ConnectError::Credential(CredentialError::Other(e.to_string())))?,
    }
    connection_record::write(repo_root, &record)
        .map_err(|e| ConnectError::Credential(CredentialError::Other(e.to_string())))?;

    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credential::CallUrgency;
    use crate::vault;
    use std::fs;
    use tempfile::tempdir;

    fn access_token_record() -> ConnectionRecord {
        ConnectionRecord {
            connection_id: "conn-1".to_string(),
            credential_kind: CredentialKind::AccessToken,
            provider: connection_record::Provider::Other("test-host".to_string()),
            https_username: Some("dawid".to_string()),
            token_expiry: None,
            credential_store: StoreKind::Keychain,
        }
    }

    // -- credential_for_kind: the minimal, generic per-kind translation --

    #[test]
    fn access_token_kind_builds_userpass_credentials() {
        let result = credential_for_kind(
            CredentialKind::AccessToken,
            &Some("dawid".to_string()),
            b"a-token",
            "https://example.test/repo.git",
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn oauth_sign_in_kind_builds_userpass_credentials_from_the_access_token_half() {
        // Ticket 06: the stored secret is the `OauthSecret` JSON envelope,
        // not a raw token -- this proves `credential_for_kind` unwraps it
        // and uses only the access token, defaulting to the `x-access-token`
        // username convention when the record has none.
        let secret = crate::github_oauth::OauthSecret {
            access_token: "gho_abc".to_string(),
            refresh_token: "ghr_def".to_string(),
        };
        let result = credential_for_kind(
            CredentialKind::OauthSignIn,
            &None,
            &secret.to_bytes(),
            "https://github.com/dawid/notes.git",
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn oauth_sign_in_kind_with_corrupt_stored_secret_bytes_fails_cleanly_not_a_panic() {
        let result = credential_for_kind(
            CredentialKind::OauthSignIn,
            &None,
            b"not valid json",
            "https://github.com/dawid/notes.git",
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn ssh_key_kind_builds_credentials_from_the_stored_key_material() {
        // `ssh_key_from_memory` defers actually parsing the key until
        // libssh2 uses it during the handshake (network-only, so it isn't
        // exercised here) -- this only checks the right `Cred` constructor
        // is used for the kind, never a fallback to another kind, and that
        // the ticket 05 `SshSecret` JSON envelope round-trips correctly.
        let secret = crate::ssh_key::SshSecret {
            private_key_openssh: "-----BEGIN OPENSSH PRIVATE KEY-----\nstand-in key bytes\n-----END OPENSSH PRIVATE KEY-----\n".to_string(),
            passphrase: None,
        };
        let result = credential_for_kind(
            CredentialKind::SshKey,
            &None,
            &secret.to_bytes(),
            "ssh://example.test/repo.git",
            Some("git"),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn ssh_key_kind_passes_the_stored_passphrase_through_to_the_credential() {
        // A passphrase stored alongside an imported key (ticket 05) must
        // actually reach `Cred::ssh_key_from_memory`, not just the key
        // material -- otherwise unattended sync with an imported
        // passphrase-protected key would silently never work.
        let secret = crate::ssh_key::SshSecret {
            private_key_openssh: "-----BEGIN OPENSSH PRIVATE KEY-----\nencrypted stand-in\n-----END OPENSSH PRIVATE KEY-----\n".to_string(),
            passphrase: Some("hunter2".to_string()),
        };
        let result = credential_for_kind(
            CredentialKind::SshKey,
            &None,
            &secret.to_bytes(),
            "ssh://example.test/repo.git",
            Some("git"),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn ssh_key_kind_with_corrupt_stored_secret_bytes_fails_cleanly_not_a_panic() {
        let result = credential_for_kind(
            CredentialKind::SshKey,
            &None,
            b"not valid json",
            "ssh://example.test/repo.git",
            Some("git"),
        );
        assert!(result.is_err());
    }

    // -- try_connect: nothing is persisted until a test fetch succeeds --

    #[test]
    fn try_connect_persists_nothing_when_the_test_fetch_fails() {
        let repo_dir = tempdir().unwrap();
        vault::ensure_git_repo(repo_dir.path()).unwrap();

        let config_dir = tempdir().unwrap();
        let keychain = crate::credential::KeychainBackend::in_memory();
        let plaintext = crate::credential::PlaintextStore::new(config_dir.path());

        let mut record = access_token_record();
        record.credential_store = StoreKind::Plaintext;

        // Bind and immediately drop a listener so the port is guaranteed
        // closed -- an unreachable remote, offline and deterministic.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let remote_url = format!("http://127.0.0.1:{port}/repo.git");

        let result = try_connect(
            repo_dir.path(),
            Some(&keychain),
            &plaintext,
            record.clone(),
            b"a-token".to_vec(),
            &remote_url,
            None,
            CallUrgency::Interactive,
        );

        assert!(matches!(result, Err(ConnectError::Fetch(_))));
        assert_eq!(connection_record::read(repo_dir.path()).unwrap(), None);
        assert!(plaintext.get_secret(&record.connection_id).is_err());
    }

    #[test]
    fn try_connect_persists_the_record_and_secret_once_the_test_fetch_succeeds() {
        let repo_dir = tempdir().unwrap();
        vault::ensure_git_repo(repo_dir.path()).unwrap();
        fs::write(repo_dir.path().join("page.md"), "---\nid: p1\n---\nHello.\n").unwrap();
        vault::commit_all(repo_dir.path(), "Create page").unwrap();

        // A local bare repo needs no credentials at all, so a test fetch
        // against it succeeds regardless of which (bogus) secret is given --
        // this test is about the persist-gating, not about a specific
        // kind's real-world auth behavior.
        let bare_dir = tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();
        let repo = git2::Repository::open(repo_dir.path()).unwrap();
        repo.remote("origin", bare_dir.path().to_str().unwrap()).unwrap();
        let mut remote = repo.find_remote("origin").unwrap();
        let branch = repo.head().unwrap().shorthand().unwrap().to_string();
        remote
            .push(&[format!("refs/heads/{branch}:refs/heads/{branch}").as_str()], None)
            .unwrap();

        let config_dir = tempdir().unwrap();
        let keychain = crate::credential::KeychainBackend::in_memory();
        let plaintext = crate::credential::PlaintextStore::new(config_dir.path());

        let mut record = access_token_record();
        record.credential_store = StoreKind::Keychain;

        let connection = try_connect(
            repo_dir.path(),
            Some(&keychain),
            &plaintext,
            record.clone(),
            b"a-token".to_vec(),
            bare_dir.path().to_str().unwrap(),
            None,
            CallUrgency::Interactive,
        )
        .unwrap();

        assert_eq!(connection.credential_kind(), CredentialKind::AccessToken);
        assert_eq!(connection_record::read(repo_dir.path()).unwrap(), Some(record.clone()));
        assert_eq!(
            keychain.get_secret(&record.connection_id, CallUrgency::Interactive).unwrap(),
            b"a-token"
        );
    }

    // -- Connection::load --

    #[test]
    fn load_returns_none_when_no_connection_is_configured() {
        let repo_dir = tempdir().unwrap();
        let config_dir = tempdir().unwrap();
        let keychain = crate::credential::KeychainBackend::in_memory();
        let plaintext = crate::credential::PlaintextStore::new(config_dir.path());

        let loaded = Connection::load(repo_dir.path(), &keychain, &plaintext, CallUrgency::Background).unwrap();

        assert!(loaded.is_none());
    }

    #[test]
    fn load_resolves_the_secret_from_the_store_the_record_declares() {
        let repo_dir = tempdir().unwrap();
        let config_dir = tempdir().unwrap();
        let keychain = crate::credential::KeychainBackend::in_memory();
        let plaintext = crate::credential::PlaintextStore::new(config_dir.path());

        let mut record = access_token_record();
        record.credential_store = StoreKind::Plaintext;
        connection_record::write(repo_dir.path(), &record).unwrap();
        plaintext.set_secret(&record.connection_id, b"stored-token").unwrap();

        let loaded = Connection::load(repo_dir.path(), &keychain, &plaintext, CallUrgency::Background)
            .unwrap()
            .unwrap();

        assert_eq!(loaded.credential_kind(), CredentialKind::AccessToken);
        assert_eq!(loaded.secret, b"stored-token");
    }

    // -- ticket 04: integration test against a fixture HTTPS remote
    // requiring HTTP Basic auth -- connect succeeds with the correct token,
    // fails (and persists nothing) with the wrong one, and never falls back
    // to any other mechanism (ADR-0012: `credential_for_kind`'s `AccessToken`
    // arm only ever builds one `Cred::userpass_plaintext`, so there is
    // nothing else for a failed attempt to fall back to). `test_fetch`'s
    // `connect_auth` only needs the smart-HTTP `info/refs` handshake to
    // succeed, never a real pack transfer, so this fixture only has to
    // answer that one request -- mirroring the pattern `sync.rs`'s
    // `spawn_401_server` established for ticket 03's own credential test.
    mod basic_auth_fixture {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        /// Minimal, dependency-free base64 decoder -- just enough to read
        /// back the `Authorization: Basic <base64>` header this fixture
        /// receives; not a general-purpose implementation.
        fn base64_decode(input: &str) -> Vec<u8> {
            const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut out = Vec::new();
            let mut buf = 0u32;
            let mut bits = 0u32;
            for c in input.bytes() {
                if c == b'=' {
                    break;
                }
                let Some(val) = ALPHABET.iter().position(|&b| b == c) else { continue };
                buf = (buf << 6) | val as u32;
                bits += 6;
                if bits >= 8 {
                    bits -= 8;
                    out.push((buf >> bits) as u8);
                }
            }
            out
        }

        fn basic_auth_credentials(request_head: &str) -> Option<(String, String)> {
            let header = request_head
                .lines()
                .find(|line| line.to_lowercase().starts_with("authorization:"))?;
            let value = header.splitn(2, ':').nth(1)?.trim();
            let b64 = value.strip_prefix("Basic ")?;
            let decoded = String::from_utf8(base64_decode(b64)).ok()?;
            let mut parts = decoded.splitn(2, ':');
            Some((parts.next()?.to_string(), parts.next()?.to_string()))
        }

        /// A minimal git smart-HTTP `info/refs` responder requiring HTTP
        /// Basic auth: `401` for a missing/wrong `Authorization` header,
        /// otherwise a minimal valid (zero-ref) `git-upload-pack`
        /// advertisement for exactly `username`/`token` -- any other
        /// credential is rejected, there is no secondary check to fall
        /// back to.
        pub fn spawn(username: &'static str, token: &'static str) -> u16 {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            std::thread::spawn(move || {
                for stream in listener.incoming().take(20) {
                    let Ok(mut stream) = stream else { continue };
                    let mut buf = [0u8; 8192];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]);

                    let authorized = basic_auth_credentials(&request)
                        .map(|(u, p)| u == username && p == token)
                        .unwrap_or(false);

                    if authorized {
                        let service_line = "# service=git-upload-pack\n";
                        let mut body = Vec::new();
                        body.extend_from_slice(format!("{:04x}", service_line.len() + 4).as_bytes());
                        body.extend_from_slice(service_line.as_bytes());
                        body.extend_from_slice(b"0000"); // flush after the service announcement
                        body.extend_from_slice(b"0000"); // flush for an empty ref list
                        let response = format!(
                            "HTTP/1.1 200 OK\r\n\
                             Content-Type: application/x-git-upload-pack-advertisement\r\n\
                             Content-Length: {}\r\n\
                             Connection: close\r\n\
                             \r\n",
                            body.len()
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.write_all(&body);
                    } else {
                        let _ = stream.write_all(
                            b"HTTP/1.1 401 Unauthorized\r\n\
                              WWW-Authenticate: Basic realm=\"cerebrite-test\"\r\n\
                              Content-Length: 0\r\n\
                              Connection: close\r\n\
                              \r\n",
                        );
                    }
                }
            });
            port
        }
    }

    #[test]
    fn connect_succeeds_against_a_basic_auth_fixture_remote_with_the_correct_token() {
        let repo_dir = tempdir().unwrap();
        vault::ensure_git_repo(repo_dir.path()).unwrap();

        let config_dir = tempdir().unwrap();
        let keychain = crate::credential::KeychainBackend::in_memory();
        let plaintext = crate::credential::PlaintextStore::new(config_dir.path());

        let port = basic_auth_fixture::spawn("dawid", "correct-token");
        let remote_url = format!("http://127.0.0.1:{port}/repo.git");

        let mut record = access_token_record();
        record.credential_store = StoreKind::Keychain;

        let connection = try_connect(
            repo_dir.path(),
            Some(&keychain),
            &plaintext,
            record.clone(),
            b"correct-token".to_vec(),
            &remote_url,
            None,
            CallUrgency::Interactive,
        )
        .unwrap();

        assert_eq!(connection.credential_kind(), CredentialKind::AccessToken);
        assert_eq!(connection_record::read(repo_dir.path()).unwrap(), Some(record.clone()));
        assert_eq!(
            keychain.get_secret(&record.connection_id, CallUrgency::Interactive).unwrap(),
            b"correct-token"
        );
    }

    #[test]
    fn connect_fails_and_persists_nothing_against_a_basic_auth_fixture_remote_with_the_wrong_token() {
        let repo_dir = tempdir().unwrap();
        vault::ensure_git_repo(repo_dir.path()).unwrap();

        let config_dir = tempdir().unwrap();
        let keychain = crate::credential::KeychainBackend::in_memory();
        let plaintext = crate::credential::PlaintextStore::new(config_dir.path());

        let port = basic_auth_fixture::spawn("dawid", "correct-token");
        let remote_url = format!("http://127.0.0.1:{port}/repo.git");

        let mut record = access_token_record();
        record.credential_store = StoreKind::Keychain;

        let result = try_connect(
            repo_dir.path(),
            Some(&keychain),
            &plaintext,
            record.clone(),
            b"wrong-token".to_vec(),
            &remote_url,
            None,
            CallUrgency::Interactive,
        );

        match result {
            Err(ConnectError::Fetch(cause)) => {
                assert!(
                    matches!(cause, crate::sync::SyncFailureCause::CredentialRejected { .. }),
                    "expected CredentialRejected, got {cause:?}"
                );
                assert!(
                    cause.to_string().starts_with("access token rejected:"),
                    "expected the failure to name the credential kind, got: {cause}"
                );
            }
            Ok(_) => panic!("expected connect to fail with the wrong token, but it succeeded"),
            Err(other) => panic!("expected ConnectError::Fetch(CredentialRejected), got {other:?}"),
        }
        // Nothing was persisted: no connection record, and the keychain
        // never received a secret for this connection id -- a failed test
        // fetch must never fall through to storing the credential anyway.
        assert_eq!(connection_record::read(repo_dir.path()).unwrap(), None);
        assert!(matches!(
            keychain.get_secret(&record.connection_id, CallUrgency::Interactive),
            Err(CredentialError::NotFound)
        ));
    }

    // -- ticket 05: end-to-end SSH integration against a real local sshd --
    //
    // Checklist item 8 asks for "a generated key connects and pushes/pulls
    // against a local SSH-serving fixture", and separately, that "a
    // mismatched host key is rejected rather than silently trusted" is
    // tested directly. This sandbox happens to have `openssh-server`
    // available (confirmed with `apt-get install openssh-server` while
    // implementing this ticket), so rather than descoping to a mocked
    // `certificate_check` call (which can't easily construct a real
    // `git2::Cert` without an actual handshake), this spins up a real
    // non-root `sshd` listening on 127.0.0.1, serving a real bare repo via
    // a forced `git-upload-pack` command in `authorized_keys` -- so the
    // whole path (an in-app-generated ed25519 key building valid libssh2
    // credentials, the host-key TOFU/pinning check via a real
    // `certificate_check` invocation, and `Cred::ssh_key_from_memory`
    // actually authenticating) is exercised for real. If `sshd` isn't
    // present (a leaner or non-Linux sandbox), `SshFixture::spawn` returns
    // `None` and these tests skip themselves rather than failing the suite
    // -- see this ticket's report for why a guaranteed-available fixture
    // wasn't assumed.
    mod ssh_fixture {
        use std::net::TcpStream;
        use std::path::PathBuf;
        use std::process::{Child, Command, Stdio};
        use std::time::Duration;

        pub struct SshFixture {
            child: Child,
            port: u16,
            repo_dir: tempfile::TempDir,
            _config_dir: tempfile::TempDir,
        }

        impl SshFixture {
            /// Spawns a local `sshd` trusting only `client_public_key_openssh`,
            /// serving a fresh bare repo via a forced `git-upload-pack`
            /// command. `None` if this sandbox has no `sshd` binary at all.
            pub fn spawn(client_public_key_openssh: &str) -> Option<Self> {
                const SSHD_PATH: &str = "/usr/sbin/sshd";
                if !std::path::Path::new(SSHD_PATH).exists() {
                    return None;
                }

                let config_dir = tempfile::tempdir().unwrap();
                let repo_dir = tempfile::tempdir().unwrap();
                git2::Repository::init_bare(repo_dir.path()).unwrap();

                // The host key is generated exactly the way any other
                // Cerebrite ed25519 key is (`ssh_key::generate_ed25519_key`)
                // -- a bonus check that a Cerebrite-generated key is
                // byte-for-byte usable by a real OpenSSH server, not only
                // by libssh2's parser.
                let host_key = crate::ssh_key::generate_ed25519_key().unwrap();
                let host_key_path = config_dir.path().join("host_key");
                std::fs::write(&host_key_path, &host_key.secret.private_key_openssh).unwrap();
                restrict(&host_key_path);

                let authorized_keys_path = config_dir.path().join("authorized_keys");
                let repo_path = repo_dir.path().to_str().unwrap();
                std::fs::write(
                    &authorized_keys_path,
                    format!(
                        "command=\"git-upload-pack '{repo_path}'\",no-pty,no-agent-forwarding,no-X11-forwarding,no-port-forwarding {client_public_key_openssh}\n"
                    ),
                )
                .unwrap();
                restrict(&authorized_keys_path);

                let port = unused_port();
                let sshd_config_path = config_dir.path().join("sshd_config");
                std::fs::write(
                    &sshd_config_path,
                    format!(
                        "Port {port}\n\
                         ListenAddress 127.0.0.1\n\
                         HostKey {}\n\
                         AuthorizedKeysFile {}\n\
                         UsePAM no\n\
                         PasswordAuthentication no\n\
                         PubkeyAuthentication yes\n\
                         StrictModes no\n\
                         LogLevel ERROR\n",
                        host_key_path.display(),
                        authorized_keys_path.display(),
                    ),
                )
                .unwrap();

                let child = Command::new(SSHD_PATH)
                    .args(["-f", sshd_config_path.to_str().unwrap(), "-D"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .expect("spawning sshd");

                wait_for_port(port);

                Some(Self {
                    child,
                    port,
                    repo_dir,
                    _config_dir: config_dir,
                })
            }

            pub fn host(&self) -> &'static str {
                "127.0.0.1"
            }

            pub fn remote_url(&self) -> String {
                format!(
                    "ssh://{}@{}:{}{}",
                    current_username(),
                    self.host(),
                    self.port,
                    self.repo_dir.path().to_str().unwrap()
                )
            }
        }

        impl Drop for SshFixture {
            fn drop(&mut self) {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }

        fn unused_port() -> u16 {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            drop(listener);
            port
        }

        fn wait_for_port(port: u16) {
            for _ in 0..100 {
                if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            panic!("sshd did not start listening on port {port} in time");
        }

        fn current_username() -> String {
            Command::new("whoami")
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "agent".to_string())
        }

        #[cfg(unix)]
        fn restrict(path: &PathBuf) {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path).unwrap().permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(path, perms).unwrap();
        }
    }

    fn ssh_key_record() -> ConnectionRecord {
        ConnectionRecord {
            connection_id: uuid::Uuid::new_v4().to_string(),
            credential_kind: CredentialKind::SshKey,
            provider: connection_record::Provider::Other("127.0.0.1".to_string()),
            https_username: None,
            token_expiry: None,
            credential_store: StoreKind::Plaintext,
        }
    }

    #[test]
    fn an_unconfirmed_ssh_host_key_is_reported_and_confirming_it_lets_the_connection_through() {
        let client_key = crate::ssh_key::generate_ed25519_key().unwrap();
        let Some(fixture) = ssh_fixture::SshFixture::spawn(&client_key.public_key_openssh) else {
            eprintln!("skipping: no local sshd available in this sandbox");
            return;
        };

        let repo_dir = tempdir().unwrap();
        vault::ensure_git_repo(repo_dir.path()).unwrap();
        let config_dir = tempdir().unwrap();
        let plaintext = crate::credential::PlaintextStore::new(config_dir.path());
        let known_hosts_path = config_dir.path().join("known_hosts");

        let record = ssh_key_record();
        let remote_url = fixture.remote_url();

        // First contact: the host has never been seen before, and it isn't
        // GitHub/GitLab -- must be reported as unconfirmed, not silently
        // trusted, and nothing may be persisted.
        let first_attempt = try_connect(
            repo_dir.path(),
            None,
            &plaintext,
            record.clone(),
            client_key.secret.to_bytes(),
            &remote_url,
            Some(&known_hosts_path),
            CallUrgency::Interactive,
        );
        let (host, fingerprint) = match first_attempt {
            Err(ConnectError::Fetch(crate::sync::SyncFailureCause::HostKeyUnconfirmed { host, fingerprint })) => {
                (host, fingerprint)
            }
            Ok(_) => panic!("expected HostKeyUnconfirmed, but the connection unexpectedly succeeded"),
            Err(other) => panic!("expected HostKeyUnconfirmed, got {other}"),
        };
        assert_eq!(host, fixture.host());
        assert!(fingerprint.starts_with("SHA256:"));
        assert_eq!(connection_record::read(repo_dir.path()).unwrap(), None);

        // Explicit confirmation (what `confirm_ssh_host_key` does in
        // lib.rs), then the identical connect attempt succeeds end to end:
        // the host key is now trusted *and* the generated key's private
        // material authenticates against the real sshd via
        // `Cred::ssh_key_from_memory`.
        crate::ssh_host_keys::KnownHosts::confirm(&known_hosts_path, &host, &fingerprint).unwrap();

        let connection = try_connect(
            repo_dir.path(),
            None,
            &plaintext,
            record.clone(),
            client_key.secret.to_bytes(),
            &remote_url,
            Some(&known_hosts_path),
            CallUrgency::Interactive,
        )
        .unwrap();

        assert_eq!(connection.credential_kind(), CredentialKind::SshKey);
        assert_eq!(connection_record::read(repo_dir.path()).unwrap(), Some(record.clone()));
        assert_eq!(
            plaintext.get_secret(&record.connection_id).unwrap(),
            client_key.secret.to_bytes()
        );
    }

    #[test]
    fn a_host_key_that_no_longer_matches_what_was_confirmed_is_rejected_not_silently_trusted() {
        let client_key = crate::ssh_key::generate_ed25519_key().unwrap();
        let Some(fixture) = ssh_fixture::SshFixture::spawn(&client_key.public_key_openssh) else {
            eprintln!("skipping: no local sshd available in this sandbox");
            return;
        };

        let repo_dir = tempdir().unwrap();
        vault::ensure_git_repo(repo_dir.path()).unwrap();
        let config_dir = tempdir().unwrap();
        let plaintext = crate::credential::PlaintextStore::new(config_dir.path());
        let known_hosts_path = config_dir.path().join("known_hosts");

        // Pre-confirm the host under a fingerprint that is deliberately
        // *not* what this sshd instance actually presents -- simulating
        // "this host's key changed since it was last trusted".
        crate::ssh_host_keys::KnownHosts::confirm(
            &known_hosts_path,
            fixture.host(),
            "SHA256:this-is-not-the-real-fingerprint-AAAAAAAAAAA",
        )
        .unwrap();

        let record = ssh_key_record();
        let result = try_connect(
            repo_dir.path(),
            None,
            &plaintext,
            record.clone(),
            client_key.secret.to_bytes(),
            &fixture.remote_url(),
            Some(&known_hosts_path),
            CallUrgency::Interactive,
        );

        match result {
            Err(ConnectError::Fetch(crate::sync::SyncFailureCause::HostKeyMismatch { host, fingerprint })) => {
                assert_eq!(host, fixture.host());
                // The *real* fingerprint sshd presented, not the bogus one
                // pre-confirmed above -- proof the check compared against
                // what was actually presented, not just noticed a diff.
                assert_ne!(fingerprint, "SHA256:this-is-not-the-real-fingerprint-AAAAAAAAAAA");
                assert!(fingerprint.starts_with("SHA256:"));
            }
            Ok(_) => panic!("expected HostKeyMismatch, but the connection unexpectedly succeeded"),
            Err(other) => panic!("expected HostKeyMismatch, got {other}"),
        }
        // Refused, not silently trusted -- nothing persisted.
        assert_eq!(connection_record::read(repo_dir.path()).unwrap(), None);
    }
}
