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
        Ok(Some(Self { record, secret }))
    }

    /// Builds a Connection directly from an already-known record + secret,
    /// without touching a keychain or `connection.json` -- used by
    /// `try_connect`'s pre-persist test fetch, and by tests.
    pub fn from_parts(record: ConnectionRecord, secret: Vec<u8>) -> Self {
        Self { record, secret }
    }

    pub fn record(&self) -> &ConnectionRecord {
        &self.record
    }

    pub fn credential_kind(&self) -> CredentialKind {
        self.record.credential_kind
    }

    /// Builds callbacks that resolve credentials from this Connection's one
    /// named kind, and nothing else. A credential failure here (an invalid
    /// key, a rejected token) is a hard failure of *this* connection -- it
    /// is never a cue to try a different kind, unlike the old
    /// agent/credential-helper/default chain this type replaces.
    pub fn make_callbacks(&self) -> git2::RemoteCallbacks<'_> {
        let mut callbacks = git2::RemoteCallbacks::new();
        let kind = self.record.credential_kind;
        let username = self.record.https_username.clone();
        let secret = self.secret.clone();
        callbacks.credentials(move |url, username_from_url, _allowed_types| {
            credential_for_kind(kind, &username, &secret, url, username_from_url)
        });
        callbacks
    }
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
/// `git2::Cred`. Deliberately minimal/generic per ticket 03's scope: both
/// HTTPS-transport kinds (access token, OAuth sign-in) are a token sent as
/// an HTTPS Basic-auth password and differ only in how the token was
/// obtained (ADR-0012), which tickets 04/06/07 implement; the SSH kind
/// treats the stored secret as PEM private-key material verbatim (the
/// ADR-0012 default: ed25519, no passphrase). Passphrase handling and
/// host-key pinning/TOFU are ticket 05's scope, not this one's.
fn credential_for_kind(
    kind: CredentialKind,
    https_username: &Option<String>,
    secret: &[u8],
    _url: &str,
    username_from_url: Option<&str>,
) -> Result<git2::Cred, git2::Error> {
    match kind {
        CredentialKind::AccessToken | CredentialKind::OauthSignIn => {
            let user = https_username
                .clone()
                .or_else(|| username_from_url.map(str::to_string))
                .unwrap_or_else(|| "git".to_string());
            let token = String::from_utf8_lossy(secret).into_owned();
            git2::Cred::userpass_plaintext(&user, &token)
        }
        CredentialKind::SshKey => {
            let user = username_from_url.unwrap_or("git");
            let key = String::from_utf8_lossy(secret).into_owned();
            git2::Cred::ssh_key_from_memory(user, None, &key, None)
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
    urgency: CallUrgency,
) -> Result<Connection, ConnectError> {
    let candidate = Connection::from_parts(record.clone(), secret.clone());

    crate::sync::test_fetch(remote_url, &candidate).map_err(ConnectError::Fetch)?;

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
    fn ssh_key_kind_builds_credentials_from_the_stored_key_material() {
        // `ssh_key_from_memory` defers actually parsing the key until
        // libssh2 uses it during the handshake (network-only, so it isn't
        // exercised here) -- this only checks the right `Cred` constructor
        // is used for the kind, never a fallback to another kind.
        let result = credential_for_kind(
            CredentialKind::SshKey,
            &None,
            b"-----BEGIN OPENSSH PRIVATE KEY-----\nstand-in key bytes\n-----END OPENSSH PRIVATE KEY-----\n",
            "ssh://example.test/repo.git",
            Some("git"),
        );
        assert!(result.is_ok());
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
}
