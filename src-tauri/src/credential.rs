// Credential storage infrastructure (ticket 02, ADR-0013): where a
// connection's secret actually lives -- the system keychain by default, or
// a consented-per-connection plaintext file when the keychain is
// unavailable -- plus the app-wide `known_hosts` file that sits beside it.
//
// No credential *kind*-specific setup/refresh flow (OAuth sign-in / access
// token / SSH key, ADR-0012) is wired to this yet -- tickets 04-07 do that.
// This module only has to move opaque secret bytes in and out of a store
// reliably, off the UI thread, with the right timeout, and tell "keychain
// locked" apart from "keychain unreachable" (ADR-0013). The non-secret
// connection record is `connection_record.rs`; the app-level index of which
// store each connection uses is `settings::Settings::connections`; ticket
// 03's `connection::Connection` is the first caller of the public API here,
// resolving a connection's secret generically (not yet per-kind) so sync
// can authenticate with it.
//
// A few items (`move_to_keychain`, `remove_all_credentials`, `known_hosts_path`)
// still have no caller -- those are ticket 13 (Settings UI) and ticket 05
// (SSH host keys) respectively -- so `#[allow(dead_code)]` stays on this
// module rather than being removed piecemeal.
#![allow(dead_code)]

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result as AnyResult};
use keyring_core::{CredentialStore, Entry};

/// Service name every connection's keychain entry is filed under. The
/// connection ID -- a random ID, never a secret itself -- is the
/// per-entry "user" (see `KEYCHAIN_SERVICE` usage below).
const KEYCHAIN_SERVICE: &str = "cerebrite-connection";

/// A sentinel "user" used only to probe whether the store is reachable and
/// unlocked, without ever actually creating a credential.
const PROBE_SENTINEL: &str = "cerebrite-keychain-probe";

/// ADR-0013: "wait up to about 2 minutes" for an interactive (connect-time)
/// keychain call -- an unlock or create-wallet prompt the user is looking
/// at and can Cancel.
const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(120);

/// ADR-0013: background (sync-time) calls "never raise a prompt" and must
/// fail fast. This is not a prompt-answering budget -- it's slack for a
/// slow but responsive D-Bus/Credential-Manager round trip before giving
/// up and reporting "keychain locked"/"keychain unreachable".
const BACKGROUND_TIMEOUT: Duration = Duration::from_secs(5);

/// Whether a keychain call is happening with the user watching (connect
/// time) or unattended (sync time) -- ADR-0013's "Interactive vs.
/// background calls". Determines the timeout; once ticket 13 wires up the
/// UI it will also determine whether a prompt may be shown at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallUrgency {
    Interactive,
    Background,
}

impl CallUrgency {
    fn timeout(self) -> Duration {
        match self {
            CallUrgency::Interactive => INTERACTIVE_TIMEOUT,
            CallUrgency::Background => BACKGROUND_TIMEOUT,
        }
    }
}

/// Errors from a credential-store operation, kept deliberately coarser
/// than `keyring_core::Error`: callers (and eventually ticket 13's UI)
/// only need to distinguish these cases, not every platform detail.
#[derive(Debug)]
pub enum CredentialError {
    /// No credential matches -- never set, or already deleted.
    NotFound,
    /// The store was reached but is locked (a Secret Service collection
    /// needing a prompt, etc). Distinct from `Unavailable` per ADR-0013:
    /// "a background... call that finds the keychain locked fails fast
    /// and distinguishably from 'keychain unreachable'".
    Locked(String),
    /// The store could not be reached at all -- no Secret Service daemon
    /// on the bus, D-Bus itself unavailable, etc.
    Unavailable(String),
    /// The call did not complete within its `CallUrgency` timeout. This is
    /// what actually fires for a background call against a locked
    /// collection: the underlying secret-service crate's unlock wait has
    /// no timeout of its own, so a locked store hangs rather than
    /// returning `Locked` -- `run_with_timeout` is what turns that hang
    /// into a fail-fast background error.
    TimedOut,
    /// A secret was rejected outright rather than silently truncated --
    /// e.g. the Windows Credential Manager ~2560-byte blob limit, enforced
    /// by `windows-native-keyring-store` itself.
    TooLarge(String),
    /// Anything else the platform store reported.
    Other(String),
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialError::NotFound => write!(f, "no credential is stored for this connection"),
            CredentialError::Locked(detail) => write!(f, "keychain locked: {detail}"),
            CredentialError::Unavailable(detail) => write!(f, "keychain unreachable: {detail}"),
            CredentialError::TimedOut => write!(f, "keychain call timed out"),
            CredentialError::TooLarge(detail) => {
                write!(f, "credential too large for this store: {detail}")
            }
            CredentialError::Other(detail) => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for CredentialError {}

impl From<keyring_core::Error> for CredentialError {
    fn from(err: keyring_core::Error) -> Self {
        use keyring_core::Error as E;
        match err {
            E::NoEntry => CredentialError::NotFound,
            E::NoStorageAccess(detail) => CredentialError::Locked(detail.to_string()),
            E::PlatformFailure(detail) => CredentialError::Unavailable(detail.to_string()),
            E::TooLong(attr, limit) => CredentialError::TooLarge(format!(
                "'{attr}' exceeds the platform limit of {limit} bytes"
            )),
            other => CredentialError::Other(other.to_string()),
        }
    }
}

/// Runs `call` on a dedicated OS thread and waits at most `timeout` for it.
/// Needed because the underlying Secret Service client waits on an
/// unlock/create-wallet prompt with *no* timeout of its own (ADR-0013) --
/// a bare call can block forever, so it must never run on the caller's
/// thread (in practice: never on the UI thread) without this wrapper.
///
/// On timeout, the spawned thread is not cancelled -- Rust has no safe way
/// to do that -- it is simply abandoned. If/when it eventually finishes it
/// finds no one listening on the channel and its result is dropped.
fn run_with_timeout<T, F>(timeout: Duration, call: F) -> Result<T, CredentialError>
where
    T: Send + 'static,
    F: FnOnce() -> keyring_core::Result<T> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("cerebrite-keychain-call".to_string())
        .spawn(move || {
            let _ = tx.send(call());
        })
        .expect("spawning keychain worker thread");

    match rx.recv_timeout(timeout) {
        Ok(result) => result.map_err(CredentialError::from),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(CredentialError::TimedOut),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(CredentialError::Other(
            "keychain worker thread panicked".to_string(),
        )),
    }
}

/// Builds the real, per-platform store: Windows Credential Manager or
/// Linux Secret Service. Connecting can itself fail fast (no daemon on the
/// bus) or, on some desktops, fail slowly -- see `KeychainBackend::probe`.
fn platform_store() -> keyring_core::Result<Arc<CredentialStore>> {
    #[cfg(target_os = "linux")]
    {
        // Pure-Rust (zbus) Secret Service store: avoids a libdbus-sys build
        // dependency, for the same cross-compilation reasons git2 already
        // needed `vendored-openssl` for (see Cargo.toml).
        let store: Arc<zbus_secret_service_keyring_store::Store> =
            zbus_secret_service_keyring_store::Store::new()?;
        Ok(store)
    }
    #[cfg(target_os = "windows")]
    {
        let store: Arc<windows_native_keyring_store::Store> =
            windows_native_keyring_store::Store::new()?;
        Ok(store)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Err(keyring_core::Error::NotSupportedByStore(
            "no keychain store is available on this platform yet".to_string(),
        ))
    }
}

/// The system keychain, wrapping whichever platform store crate is
/// compiled in (or, in tests, `keyring_core`'s in-memory `sample` store --
/// see `KeychainBackend::in_memory`). One keychain entry per connection,
/// holding all of that connection's secrets serialized together as raw
/// bytes (ADR-0013) -- this module has no opinion on how those bytes are
/// structured; that's for the credential kind that eventually calls it
/// (tickets 04-07).
pub struct KeychainBackend {
    store: Arc<CredentialStore>,
}

impl KeychainBackend {
    /// Connects to the real, per-platform store. This itself is a
    /// blocking call and should not run on the UI thread; wrap it the same
    /// way callers wrap `set_secret`/`get_secret`/`probe` if it's ever
    /// called somewhere latency-sensitive.
    pub fn platform() -> Result<Self, CredentialError> {
        Ok(Self {
            store: platform_store()?,
        })
    }

    /// Wraps an arbitrary store -- used by `in_memory` for tests, and
    /// available generally for dependency injection.
    pub fn with_store(store: Arc<CredentialStore>) -> Self {
        Self { store }
    }

    /// An in-memory, non-persistent, in-process store (`keyring_core`'s
    /// `sample` module) for tests: this sandbox is headless Linux with no
    /// Secret Service daemon running (`SecretService::connect` fails fast
    /// with `ServiceUnknown`, i.e. `platform_store` above returns
    /// `Unavailable` every time), so exercising the real backend isn't
    /// possible in CI. `sample::Store` implements the exact same
    /// `CredentialStoreApi` trait the platform stores do, so it exercises
    /// this module's logic (timeout wrapping, error mapping, the
    /// move/remove-all flows) faithfully -- everything except the actual
    /// OS integration, which has no substitute besides a real desktop
    /// session (see docs/agents/debugging-sandbox.md for how to get one
    /// for a manual smoke test).
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::with_store(
            keyring_core::sample::Store::new().expect("in-memory sample store never fails"),
        )
    }

    fn entry(&self, connection_id: &str) -> keyring_core::Result<Entry> {
        self.store.build(KEYCHAIN_SERVICE, connection_id, None)
    }

    /// Stores (creating or overwriting) `connection_id`'s secret.
    pub fn set_secret(
        &self,
        connection_id: &str,
        secret: &[u8],
        urgency: CallUrgency,
    ) -> Result<(), CredentialError> {
        let store = self.store.clone();
        let connection_id = connection_id.to_string();
        let secret = secret.to_vec();
        run_with_timeout(urgency.timeout(), move || {
            let entry = store.build(KEYCHAIN_SERVICE, &connection_id, None)?;
            entry.set_secret(&secret)
        })
    }

    /// Retrieves `connection_id`'s secret. `CredentialError::NotFound` if
    /// nothing has been stored for it (or it was deleted).
    pub fn get_secret(
        &self,
        connection_id: &str,
        urgency: CallUrgency,
    ) -> Result<Vec<u8>, CredentialError> {
        let store = self.store.clone();
        let connection_id = connection_id.to_string();
        run_with_timeout(urgency.timeout(), move || {
            let entry = store.build(KEYCHAIN_SERVICE, &connection_id, None)?;
            entry.get_secret()
        })
    }

    /// Deletes `connection_id`'s secret. A no-op (not an error) if there
    /// was none.
    pub fn delete_secret(
        &self,
        connection_id: &str,
        urgency: CallUrgency,
    ) -> Result<(), CredentialError> {
        let store = self.store.clone();
        let connection_id = connection_id.to_string();
        run_with_timeout(urgency.timeout(), move || {
            let entry = store.build(KEYCHAIN_SERVICE, &connection_id, None)?;
            match entry.delete_credential() {
                Ok(()) => Ok(()),
                Err(keyring_core::Error::NoEntry) => Ok(()),
                Err(e) => Err(e),
            }
        })
    }

    /// A cheap, side-effect-free check of whether the keychain is
    /// reachable and unlocked right now, without storing anything real.
    /// Looking up a sentinel that is never actually created distinguishes
    /// a clean "reachable, nothing there yet" (`NotFound`, i.e. success)
    /// from `Locked` (needs a prompt) and `Unavailable` (no daemon at
    /// all).
    pub fn probe(&self, urgency: CallUrgency) -> Result<(), CredentialError> {
        let store = self.store.clone();
        match run_with_timeout(urgency.timeout(), move || {
            let entry = store.build(KEYCHAIN_SERVICE, PROBE_SENTINEL, None)?;
            entry.get_secret().map(|_| ())
        }) {
            Ok(()) | Err(CredentialError::NotFound) => Ok(()),
            Err(other) => Err(other),
        }
    }
}

/// The consented, `0600` plaintext fallback (ADR-0013) -- one file per
/// connection ID in the app config dir. Never the default; only ever
/// written once a keychain probe has actually failed and the caller has
/// recorded consent. That consent lives in the connection record's
/// `credential_store` field (`connection_record::StoreKind::Plaintext`) --
/// this type has no opinion on consent, it only writes files
/// restrictively.
///
/// On Windows there is no `0600` equivalent; the file inherits the
/// standard per-user NTFS ACL of the app config dir it lives in, which
/// already excludes other user accounts.
pub struct PlaintextStore {
    dir: PathBuf,
}

impl PlaintextStore {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            dir: config_dir.join("credentials"),
        }
    }

    fn path_for(&self, connection_id: &str) -> PathBuf {
        self.dir.join(format!("{connection_id}.secret"))
    }

    /// Writes (or overwrites) `connection_id`'s secret as a `0600` file.
    pub fn set_secret(&self, connection_id: &str, secret: &[u8]) -> AnyResult<()> {
        fs::create_dir_all(&self.dir).context("creating credentials dir")?;
        let path = self.path_for(connection_id);
        fs::write(&path, secret).with_context(|| format!("writing {}", path.display()))?;
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&path)
                .with_context(|| format!("reading permissions of {}", path.display()))?
                .permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&path, perms)
                .with_context(|| format!("restricting permissions of {}", path.display()))?;
        }
        Ok(())
    }

    /// Reads `connection_id`'s secret.
    pub fn get_secret(&self, connection_id: &str) -> AnyResult<Vec<u8>> {
        let path = self.path_for(connection_id);
        fs::read(&path).with_context(|| format!("reading {}", path.display()))
    }

    /// Deletes `connection_id`'s secret file. A no-op if there was none.
    pub fn delete_secret(&self, connection_id: &str) -> AnyResult<()> {
        let path = self.path_for(connection_id);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).with_context(|| format!("removing {}", path.display())),
        }
    }
}

/// App-wide, Cerebrite-owned SSH known_hosts file, in the app config dir.
/// ADR-0013: host keys are a fact about the host, not the vault --
/// disconnecting a vault must not forget them -- and `~/.ssh/known_hosts`
/// is never read or written. Ticket 05 (SSH key connections) is what
/// actually populates entries in it; this ticket only needs the file to
/// exist at a stable, discoverable path.
pub fn known_hosts_path(config_dir: &Path) -> AnyResult<PathBuf> {
    fs::create_dir_all(config_dir).context("creating app config dir")?;
    let path = config_dir.join("known_hosts");
    if !path.exists() {
        fs::write(&path, b"").context("creating known_hosts file")?;
    }
    Ok(path)
}

/// Moves a connection's secret from the plaintext fallback into the
/// keychain (the "Move to keychain" action). The plaintext file is only
/// deleted after the keychain write succeeds, so a failure partway through
/// leaves the plaintext copy as the one source of truth rather than losing
/// the secret. The caller is responsible for updating the connection
/// record's `credential_store` field and the `settings.json` index once
/// this returns `Ok`.
pub fn move_to_keychain(
    keychain: &KeychainBackend,
    plaintext: &PlaintextStore,
    connection_id: &str,
    urgency: CallUrgency,
) -> Result<(), CredentialError> {
    let secret = plaintext
        .get_secret(connection_id)
        .map_err(|e| CredentialError::Other(e.to_string()))?;
    keychain.set_secret(connection_id, &secret, urgency)?;
    plaintext
        .delete_secret(connection_id)
        .map_err(|e| CredentialError::Other(e.to_string()))?;
    Ok(())
}

/// Deletes every credential Cerebrite has stored, across both stores, for
/// every connection ID in `connection_ids` (the "Remove all stored
/// Cerebrite credentials" Settings action). Best-effort: a failure
/// deleting one connection's secret from one store doesn't stop the rest
/// from being attempted. Returns the `(connection_id, error)` pairs for
/// whatever could not be removed; an empty vec means everything succeeded.
/// The caller is responsible for clearing the corresponding entries from
/// `settings.json`'s index for every ID that isn't in the returned list.
pub fn remove_all_credentials(
    keychain: &KeychainBackend,
    plaintext: &PlaintextStore,
    connection_ids: &[String],
    urgency: CallUrgency,
) -> Vec<(String, CredentialError)> {
    let mut failures = Vec::new();
    for id in connection_ids {
        if let Err(e) = keychain.delete_secret(id, urgency) {
            failures.push((id.clone(), e));
        }
        if let Err(e) = plaintext.delete_secret(id) {
            failures.push((id.clone(), CredentialError::Other(e.to_string())));
        }
    }
    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyring_core::api::CredentialApi;
    use keyring_core::api::CredentialStoreApi;
    use keyring_core::Error as KError;
    use std::collections::HashMap;
    use tempfile::tempdir;

    // -- KeychainBackend, against the in-memory sample store --

    #[test]
    fn set_then_get_round_trips_the_secret() {
        let backend = KeychainBackend::in_memory();
        backend
            .set_secret("conn-1", b"top secret bytes", CallUrgency::Interactive)
            .unwrap();

        let secret = backend.get_secret("conn-1", CallUrgency::Interactive).unwrap();

        assert_eq!(secret, b"top secret bytes");
    }

    #[test]
    fn get_on_an_unset_connection_is_not_found() {
        let backend = KeychainBackend::in_memory();

        let err = backend.get_secret("never-set", CallUrgency::Background).unwrap_err();

        assert!(matches!(err, CredentialError::NotFound));
    }

    #[test]
    fn delete_removes_the_secret_and_is_a_noop_if_already_gone() {
        let backend = KeychainBackend::in_memory();
        backend
            .set_secret("conn-1", b"secret", CallUrgency::Interactive)
            .unwrap();

        backend.delete_secret("conn-1", CallUrgency::Interactive).unwrap();
        assert!(matches!(
            backend.get_secret("conn-1", CallUrgency::Interactive).unwrap_err(),
            CredentialError::NotFound
        ));

        // Deleting again must not error.
        backend.delete_secret("conn-1", CallUrgency::Interactive).unwrap();
    }

    #[test]
    fn set_overwrites_an_existing_secret() {
        let backend = KeychainBackend::in_memory();
        backend.set_secret("conn-1", b"first", CallUrgency::Interactive).unwrap();
        backend.set_secret("conn-1", b"second", CallUrgency::Interactive).unwrap();

        assert_eq!(
            backend.get_secret("conn-1", CallUrgency::Interactive).unwrap(),
            b"second"
        );
    }

    #[test]
    fn probe_succeeds_against_a_reachable_unlocked_store() {
        let backend = KeychainBackend::in_memory();
        backend.probe(CallUrgency::Background).unwrap();
    }

    #[test]
    fn secrets_for_different_connections_are_independent() {
        let backend = KeychainBackend::in_memory();
        backend.set_secret("conn-a", b"a-secret", CallUrgency::Interactive).unwrap();
        backend.set_secret("conn-b", b"b-secret", CallUrgency::Interactive).unwrap();

        assert_eq!(
            backend.get_secret("conn-a", CallUrgency::Interactive).unwrap(),
            b"a-secret"
        );
        assert_eq!(
            backend.get_secret("conn-b", CallUrgency::Interactive).unwrap(),
            b"b-secret"
        );
    }

    // -- Error mapping: NoStorageAccess (locked) vs PlatformFailure
    // (unreachable) must stay distinguishable end to end, per ADR-0013.
    // The sample store can't produce these itself (it's always reachable
    // and unlocked), so a tiny fake `CredentialStoreApi` stands in for a
    // platform store that is locked, unreachable, or just slow.

    #[derive(Clone, Copy)]
    enum FlakyBehavior {
        Locked,
        Unreachable,
        Slow(Duration),
        TooLarge,
    }

    struct FlakyStore {
        behavior: FlakyBehavior,
    }

    struct FlakyCredential {
        behavior: FlakyBehavior,
    }

    impl CredentialApi for FlakyCredential {
        fn set_secret(&self, _secret: &[u8]) -> keyring_core::Result<()> {
            self.fail()
        }
        fn get_secret(&self) -> keyring_core::Result<Vec<u8>> {
            self.fail()?;
            Ok(vec![])
        }
        fn delete_credential(&self) -> keyring_core::Result<()> {
            self.fail()
        }
        fn get_credential(&self) -> keyring_core::Result<Option<Arc<keyring_core::api::Credential>>> {
            Ok(None)
        }
        fn get_specifiers(&self) -> Option<(String, String)> {
            None
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    impl FlakyCredential {
        fn fail<T>(&self) -> keyring_core::Result<T> {
            match self.behavior {
                FlakyBehavior::Locked => Err(KError::NoStorageAccess("collection is locked".into())),
                FlakyBehavior::Unreachable => {
                    Err(KError::PlatformFailure("no Secret Service on the bus".into()))
                }
                FlakyBehavior::Slow(duration) => {
                    std::thread::sleep(duration);
                    Err(KError::PlatformFailure("finally responded".into()))
                }
                FlakyBehavior::TooLarge => Err(KError::TooLong("secret".to_string(), 2560)),
            }
        }
    }

    impl CredentialStoreApi for FlakyStore {
        fn vendor(&self) -> String {
            "flaky test store".to_string()
        }
        fn id(&self) -> String {
            "flaky".to_string()
        }
        fn build(
            &self,
            _service: &str,
            _user: &str,
            _modifiers: Option<&HashMap<&str, &str>>,
        ) -> keyring_core::Result<Entry> {
            Ok(Entry::new_with_credential(Arc::new(FlakyCredential {
                behavior: self.behavior,
            })))
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    fn flaky_backend(behavior: FlakyBehavior) -> KeychainBackend {
        KeychainBackend::with_store(Arc::new(FlakyStore { behavior }))
    }

    #[test]
    fn locked_store_is_distinguishable_from_unreachable_store() {
        let locked = flaky_backend(FlakyBehavior::Locked);
        let unreachable = flaky_backend(FlakyBehavior::Unreachable);

        assert!(matches!(
            locked.get_secret("conn-1", CallUrgency::Background).unwrap_err(),
            CredentialError::Locked(_)
        ));
        assert!(matches!(
            unreachable.get_secret("conn-1", CallUrgency::Background).unwrap_err(),
            CredentialError::Unavailable(_)
        ));
    }

    #[test]
    fn too_long_is_surfaced_as_a_clear_error_not_silently_truncated() {
        let backend = flaky_backend(FlakyBehavior::TooLarge);

        let err = backend
            .set_secret("conn-1", b"a secret bigger than the blob limit", CallUrgency::Interactive)
            .unwrap_err();

        assert!(matches!(err, CredentialError::TooLarge(_)));
    }

    #[test]
    fn a_slow_background_call_times_out_fail_fast_rather_than_hanging() {
        // A locked Secret Service collection blocks on an unlock prompt
        // with no timeout of its own (ADR-0013) -- this simulates that by
        // sleeping past the deadline `run_with_timeout` is given, standing
        // in for `CallUrgency::Background`'s fail-fast budget without
        // making the test suite actually wait multiple seconds.
        let backend = flaky_backend(FlakyBehavior::Slow(Duration::from_millis(200)));

        let start = std::time::Instant::now();
        let result = run_with_timeout(Duration::from_millis(20), {
            let store = backend.store.clone();
            move || {
                let entry = store.build(KEYCHAIN_SERVICE, "conn-1", None)?;
                entry.get_secret()
            }
        });

        assert!(matches!(result, Err(CredentialError::TimedOut)));
        assert!(
            start.elapsed() < Duration::from_millis(150),
            "run_with_timeout should return as soon as its deadline passes, not wait for the slow call"
        );
    }

    // -- PlaintextStore --

    #[test]
    fn plaintext_store_round_trips_and_sets_0600_permissions() {
        let dir = tempdir().unwrap();
        let store = PlaintextStore::new(dir.path());

        store.set_secret("conn-1", b"plaintext secret").unwrap();

        assert_eq!(store.get_secret("conn-1").unwrap(), b"plaintext secret");

        #[cfg(unix)]
        {
            let perms = fs::metadata(dir.path().join("credentials/conn-1.secret"))
                .unwrap()
                .permissions();
            assert_eq!(perms.mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn plaintext_delete_is_a_noop_if_absent() {
        let dir = tempdir().unwrap();
        let store = PlaintextStore::new(dir.path());
        store.delete_secret("never-set").unwrap();
    }

    #[test]
    fn plaintext_get_on_missing_connection_errors() {
        let dir = tempdir().unwrap();
        let store = PlaintextStore::new(dir.path());
        assert!(store.get_secret("missing").is_err());
    }

    // -- known_hosts --

    #[test]
    fn known_hosts_path_creates_an_empty_file_on_first_use() {
        let dir = tempdir().unwrap();
        let config_dir = dir.path().join("config");

        let path = known_hosts_path(&config_dir).unwrap();

        assert_eq!(path, config_dir.join("known_hosts"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "");
    }

    #[test]
    fn known_hosts_path_does_not_clobber_existing_content() {
        let dir = tempdir().unwrap();
        let config_dir = dir.path().join("config");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("known_hosts"), "github.com ssh-ed25519 AAAA...\n").unwrap();

        known_hosts_path(&config_dir).unwrap();

        assert_eq!(
            fs::read_to_string(config_dir.join("known_hosts")).unwrap(),
            "github.com ssh-ed25519 AAAA...\n"
        );
    }

    // -- move_to_keychain / remove_all_credentials --

    #[test]
    fn move_to_keychain_copies_the_secret_and_deletes_the_plaintext_copy() {
        let dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(dir.path());
        let keychain = KeychainBackend::in_memory();
        plaintext.set_secret("conn-1", b"a secret").unwrap();

        move_to_keychain(&keychain, &plaintext, "conn-1", CallUrgency::Interactive).unwrap();

        assert_eq!(
            keychain.get_secret("conn-1", CallUrgency::Interactive).unwrap(),
            b"a secret"
        );
        assert!(plaintext.get_secret("conn-1").is_err());
    }

    #[test]
    fn move_to_keychain_leaves_the_plaintext_copy_when_the_keychain_write_fails() {
        let dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(dir.path());
        let keychain = flaky_backend(FlakyBehavior::Unreachable);
        plaintext.set_secret("conn-1", b"a secret").unwrap();

        let err = move_to_keychain(&keychain, &plaintext, "conn-1", CallUrgency::Interactive).unwrap_err();

        assert!(matches!(err, CredentialError::Unavailable(_)));
        // The plaintext copy must still be there -- nothing was lost.
        assert_eq!(plaintext.get_secret("conn-1").unwrap(), b"a secret");
    }

    #[test]
    fn remove_all_credentials_deletes_from_both_stores_for_every_id() {
        let dir = tempdir().unwrap();
        let plaintext = PlaintextStore::new(dir.path());
        let keychain = KeychainBackend::in_memory();
        keychain.set_secret("conn-a", b"a", CallUrgency::Interactive).unwrap();
        plaintext.set_secret("conn-b", b"b").unwrap();

        let failures = remove_all_credentials(
            &keychain,
            &plaintext,
            &["conn-a".to_string(), "conn-b".to_string()],
            CallUrgency::Interactive,
        );

        assert!(failures.is_empty(), "expected no failures, got {failures:?}");
        assert!(keychain.get_secret("conn-a", CallUrgency::Interactive).is_err());
        assert!(plaintext.get_secret("conn-b").is_err());
    }
}
