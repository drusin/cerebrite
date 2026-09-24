// In-app SSH key generation and import (ticket 05). No dependency on a
// system `ssh-keygen` binary (a requirement of its own -- Android has none,
// and desktop shouldn't need one either): keys are generated and parsed in
// pure Rust via the `ssh-key` crate (RustCrypto/SSH), chosen over
// `ed25519-dalek` (archived upstream) and `russh-keys` (a full SSH client
// stack this app doesn't otherwise need) -- see ticket 05's research doc.
//
// Passphrase handling (ticket 05 checklist item 3): a Cerebrite-*generated*
// key never carries a passphrase -- see `generate_ed25519_key`'s doc
// comment for why. An *imported* key that already has one keeps it, and its
// passphrase is stored alongside the key material in the same credential
// store entry (`SshSecret`, below) rather than the app trying to strip
// or re-encrypt it. This was a real choice, not the only option (the other
// being "disable unattended sync until the passphrase is re-entered every
// session") -- picked because the OS keychain is already the trust boundary
// for the raw private key of a *generated* key; a passphrase stored in that
// same keychain entry protects nothing further that the keychain itself
// doesn't already protect, so it doesn't meaningfully weaken anything to
// keep it there too, and it's what actually keeps unattended background
// sync working for an imported key the same way it does for a generated
// one -- consistent behavior regardless of how the key arrived.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use ssh_key::{LineEnding, PrivateKey};

/// The credential-store payload for `CredentialKind::SshKey` (ticket 05):
/// the private key exactly as it should be handed to
/// `Cred::ssh_key_from_memory`, plus its passphrase if it has one.
/// Serialized as JSON and stored as opaque bytes via ticket 02's
/// `KeychainBackend`/`PlaintextStore` -- both already just move bytes, so
/// this envelope is this module's business alone, not theirs.
///
/// Deliberately *not* decrypting an imported passphrase-protected key
/// before storing it: `private_key_openssh` is stored exactly as given
/// (still encrypted, if it was), and `Cred::ssh_key_from_memory`'s own
/// `passphrase` argument is what lets libssh2 decrypt it at connect time.
/// Cerebrite never needs the decrypted key material itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshSecret {
    pub private_key_openssh: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
}

impl SshSecret {
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("SshSecret always serializes")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

#[derive(Debug)]
pub enum SshKeyError {
    /// The key material itself didn't parse (corrupt, unsupported format).
    Parse(String),
    /// The key is passphrase-protected but no passphrase (or the wrong one)
    /// was supplied at import time.
    WrongOrMissingPassphrase,
}

impl std::fmt::Display for SshKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SshKeyError::Parse(detail) => write!(f, "could not read this SSH private key: {detail}"),
            SshKeyError::WrongOrMissingPassphrase => {
                write!(f, "this key is passphrase-protected; the passphrase given did not unlock it")
            }
        }
    }
}

impl std::error::Error for SshKeyError {}

/// A freshly generated key, ready to show the user (`public_key_openssh`,
/// `fingerprint`) and, once they confirm, hand to `connect_ssh_key` as the
/// candidate `SshSecret`.
pub struct GeneratedSshKey {
    pub secret: SshSecret,
    /// The `authorized_keys`-format public key line (`ssh-ed25519 AAAA...`)
    /// -- what the user copies into the provider's "add SSH key" page.
    pub public_key_openssh: String,
    pub fingerprint_sha256: String,
}

/// Generates a fresh ed25519 keypair (ticket 05: "ed25519, the default,
/// no passphrase"). No passphrase, ever, for a Cerebrite-generated key --
/// unattended background sync must be able to use it immediately with no
/// further prompting, and per this module's doc comment, an
/// app-auto-supplied passphrase on a key the app also generated protects
/// nothing beyond what the keychain entry holding it already does.
pub fn generate_ed25519_key() -> Result<GeneratedSshKey, SshKeyError> {
    let mut rng = ssh_key::rand_core::OsRng;
    let private_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519)
        .map_err(|e| SshKeyError::Parse(e.to_string()))?;

    let private_key_openssh = private_key
        .to_openssh(LineEnding::LF)
        .map_err(|e| SshKeyError::Parse(e.to_string()))?
        .to_string();
    let public_key_openssh = private_key
        .public_key()
        .to_openssh()
        .map_err(|e| SshKeyError::Parse(e.to_string()))?;
    let fingerprint_sha256 = private_key.public_key().fingerprint(ssh_key::HashAlg::Sha256).to_string();

    Ok(GeneratedSshKey {
        secret: SshSecret {
            private_key_openssh,
            passphrase: None,
        },
        public_key_openssh,
        fingerprint_sha256,
    })
}

/// An imported key, validated (and, if it declared a passphrase, confirmed
/// to actually be unlockable with the one given) but *not* decrypted for
/// storage -- see `SshSecret`'s doc comment.
pub struct ImportedSshKey {
    pub secret: SshSecret,
    pub public_key_openssh: String,
    pub fingerprint_sha256: String,
    /// Whether the key as imported carries its own passphrase -- surfaced
    /// so the caller can tell the user their key will need that passphrase
    /// kept in the credential store for unattended sync to work (ticket 05
    /// checklist item 3), rather than silently deciding that for them.
    pub had_passphrase: bool,
}

/// Validates and wraps an imported private key. `private_key_text` is
/// whatever the user pasted/selected (an OpenSSH-format private key, PEM
/// armor included) -- parsed here only to confirm it's actually a key
/// Cerebrite can use and, if it's encrypted, that `passphrase` unlocks it;
/// the *stored* `secret.private_key_openssh` is the original text verbatim,
/// still encrypted if it was, per this module's doc comment.
pub fn import_ssh_key(private_key_text: &str, passphrase: Option<&str>) -> Result<ImportedSshKey, SshKeyError> {
    let parsed =
        PrivateKey::from_openssh(private_key_text).map_err(|e| SshKeyError::Parse(e.to_string()))?;

    let had_passphrase = parsed.is_encrypted();
    let public_key = if had_passphrase {
        let Some(passphrase) = passphrase.filter(|p| !p.is_empty()) else {
            return Err(SshKeyError::WrongOrMissingPassphrase);
        };
        // Decrypting here is purely a validation step (catch a wrong
        // passphrase at import time, with a clear error, rather than
        // surfacing it later as a confusing connect-time auth failure) --
        // the decrypted key is dropped immediately after; only the
        // original encrypted text is ever stored (see `SshSecret`).
        let decrypted = parsed
            .decrypt(passphrase)
            .map_err(|_| SshKeyError::WrongOrMissingPassphrase)?;
        decrypted.public_key().clone()
    } else {
        parsed.public_key().clone()
    };

    let public_key_openssh = public_key.to_openssh().map_err(|e| SshKeyError::Parse(e.to_string()))?;
    let fingerprint_sha256 = public_key.fingerprint(ssh_key::HashAlg::Sha256).to_string();

    Ok(ImportedSshKey {
        secret: SshSecret {
            private_key_openssh: private_key_text.to_string(),
            passphrase: if had_passphrase {
                passphrase.map(str::to_string)
            } else {
                None
            },
        },
        public_key_openssh,
        fingerprint_sha256,
        had_passphrase,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- generation --

    #[test]
    fn generates_an_ed25519_key_with_no_passphrase() {
        let generated = generate_ed25519_key().unwrap();

        assert!(generated.secret.passphrase.is_none());
        assert!(generated.secret.private_key_openssh.contains("BEGIN OPENSSH PRIVATE KEY"));
        assert!(generated.public_key_openssh.starts_with("ssh-ed25519 "));
        assert!(generated.fingerprint_sha256.starts_with("SHA256:"));
    }

    #[test]
    fn two_generated_keys_are_never_the_same() {
        let a = generate_ed25519_key().unwrap();
        let b = generate_ed25519_key().unwrap();
        assert_ne!(a.secret.private_key_openssh, b.secret.private_key_openssh);
        assert_ne!(a.public_key_openssh, b.public_key_openssh);
    }

    /// A generated key must actually be usable by libssh2 via
    /// `Cred::ssh_key_from_memory` -- ticket 05 checklist item 1's smoke
    /// test, exercised end to end through the exact code path
    /// `connection.rs`'s `credential_for_kind` uses.
    #[test]
    fn a_generated_keys_private_material_builds_valid_in_memory_ssh_credentials() {
        let generated = generate_ed25519_key().unwrap();

        let cred = git2::Cred::ssh_key_from_memory(
            "git",
            None,
            &generated.secret.private_key_openssh,
            generated.secret.passphrase.as_deref(),
        );

        assert!(
            cred.is_ok(),
            "expected a valid in-memory SSH credential, got err: {:?}",
            cred.err().map(|e| e.to_string())
        );
    }

    // -- SshSecret round-trip (what's actually persisted) --

    #[test]
    fn ssh_secret_round_trips_through_json_bytes() {
        let secret = SshSecret {
            private_key_openssh: "key-material".to_string(),
            passphrase: Some("hunter2".to_string()),
        };

        let bytes = secret.to_bytes();
        let decoded = SshSecret::from_bytes(&bytes).unwrap();

        assert_eq!(decoded, secret);
    }

    #[test]
    fn ssh_secret_without_a_passphrase_round_trips_too() {
        let secret = SshSecret {
            private_key_openssh: "key-material".to_string(),
            passphrase: None,
        };

        let decoded = SshSecret::from_bytes(&secret.to_bytes()).unwrap();

        assert_eq!(decoded, secret);
    }

    // -- import --

    #[test]
    fn imports_an_unencrypted_key_with_no_passphrase_required() {
        let generated = generate_ed25519_key().unwrap();

        let imported = import_ssh_key(&generated.secret.private_key_openssh, None).unwrap();

        assert!(!imported.had_passphrase);
        assert_eq!(imported.secret.passphrase, None);
        assert_eq!(imported.public_key_openssh, generated.public_key_openssh);
        assert_eq!(imported.fingerprint_sha256, generated.fingerprint_sha256);
    }

    #[test]
    fn imports_a_passphrase_protected_key_and_stores_the_passphrase_alongside_it() {
        let mut rng = ssh_key::rand_core::OsRng;
        let private_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
        let public_key_openssh = private_key.public_key().to_openssh().unwrap();
        let encrypted = private_key
            .encrypt(&mut rng, "correct horse battery staple")
            .unwrap();
        let encrypted_openssh = encrypted.to_openssh(LineEnding::LF).unwrap().to_string();

        let imported = import_ssh_key(&encrypted_openssh, Some("correct horse battery staple")).unwrap();

        assert!(imported.had_passphrase);
        assert_eq!(imported.secret.passphrase.as_deref(), Some("correct horse battery staple"));
        // The stored key text is the original *encrypted* blob, verbatim --
        // never decrypted for storage.
        assert_eq!(imported.secret.private_key_openssh, encrypted_openssh);
        assert_eq!(imported.public_key_openssh, public_key_openssh);
    }

    #[test]
    fn importing_a_passphrase_protected_key_with_the_wrong_passphrase_fails_clearly() {
        let mut rng = ssh_key::rand_core::OsRng;
        let private_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
        let encrypted = private_key.encrypt(&mut rng, "the-real-passphrase").unwrap();
        let encrypted_openssh = encrypted.to_openssh(LineEnding::LF).unwrap().to_string();

        let result = import_ssh_key(&encrypted_openssh, Some("wrong-guess"));

        assert!(matches!(result, Err(SshKeyError::WrongOrMissingPassphrase)));
    }

    #[test]
    fn importing_a_passphrase_protected_key_with_no_passphrase_given_fails_clearly() {
        let mut rng = ssh_key::rand_core::OsRng;
        let private_key = PrivateKey::random(&mut rng, ssh_key::Algorithm::Ed25519).unwrap();
        let encrypted = private_key.encrypt(&mut rng, "the-real-passphrase").unwrap();
        let encrypted_openssh = encrypted.to_openssh(LineEnding::LF).unwrap().to_string();

        let result = import_ssh_key(&encrypted_openssh, None);

        assert!(matches!(result, Err(SshKeyError::WrongOrMissingPassphrase)));
    }

    #[test]
    fn importing_garbage_fails_with_a_parse_error_not_a_panic() {
        let result = import_ssh_key("not an ssh key at all", None);
        assert!(matches!(result, Err(SshKeyError::Parse(_))));
    }
}
