// SSH host-key verification (ticket 05): libssh2 does not verify host keys
// safely on its own -- the calling application is responsible (see ticket
// 05's research doc, .scratch/git-provider-integration/issues/
// 03-ssh-key-management-research.md, section 6). This module is that
// responsibility: GitHub's and GitLab's published fingerprints are pinned
// outright (no prompt, no TOFU window); every other host goes through
// trust-on-first-connect (TOFU), and is only ever trusted once the user has
// explicitly confirmed the shown fingerprint -- never silently. Confirmed
// hosts are persisted to Cerebrite's own known_hosts-equivalent file
// (`credential::known_hosts_path`, from ticket 02).
//
// Deliberately *not* OpenSSH's `known_hosts` format: that stores the full
// public key blob (so a real SSH client can also do host-key *type*
// negotiation and signature verification), which this app has no need to
// reproduce -- Cerebrite only ever needs "does this host's fingerprint match
// what we last saw", not "connect as a general-purpose SSH client". A
// one-line-per-host `<host> <sha256-fingerprint>` file is simpler to read,
// write, and reason about, and is never touched by any tool but Cerebrite
// itself (see `credential.rs`'s `known_hosts_path` doc comment: the real
// `~/.ssh/known_hosts` is never read or written).
#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// GitHub's and GitLab's publicly published SSH host key fingerprints
/// (SHA-256, `ssh-keygen -lf`/OpenSSH display format). Confirmed live
/// against the providers' own docs while implementing ticket 05:
/// - https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/githubs-ssh-key-fingerprints
/// - https://docs.gitlab.com/user/gitlab_com/#ssh-host-keys-fingerprints
///
/// All three key types each host publishes are pinned, not just ed25519 --
/// which type libssh2 is actually offered during host-key negotiation isn't
/// under this app's control, so pinning only the type Cerebrite itself
/// generates keys as would leave RSA/ECDSA connections to these two
/// providers falling through to a TOFU prompt they don't need.
const PINNED_HOST_KEYS: &[(&str, &str)] = &[
    ("github.com", "SHA256:uNiVztksCsDhcc0u9e8BujQXVUpKZIDTMczCvj3tD2s"), // RSA
    ("github.com", "SHA256:p2QAMXNIC1TJYWeIOttrVc98/R1BUFWu3/LiyKgUfQM"), // ECDSA
    ("github.com", "SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU"), // ED25519
    ("gitlab.com", "SHA256:ROQFvPThGrW4RuWLoL9tq9I9zJ42fK4XywyRtbOz/EQ"), // RSA
    ("gitlab.com", "SHA256:HbW3g8zUjNSksFbqTiUWPWg2Bq1x8xdGUrliXFzSnUw"), // ECDSA
    ("gitlab.com", "SHA256:eUXGGm1YGsMAS7vkcx6JOJdOGHPem5gQp4taiCfCLB8"), // ED25519
];

pub fn is_pinned_host(host: &str) -> bool {
    PINNED_HOST_KEYS.iter().any(|(h, _)| *h == host)
}

fn pinned_fingerprint_matches(host: &str, fingerprint: &str) -> bool {
    PINNED_HOST_KEYS.iter().any(|(h, fp)| *h == host && *fp == fingerprint)
}

/// Outcome of checking a presented host-key fingerprint against pinning and
/// the TOFU store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyCheck {
    /// Matches one of `PINNED_HOST_KEYS` for this host.
    PinnedMatch,
    /// This host is pinned (GitHub/GitLab) but the presented fingerprint
    /// matches none of its published keys -- never TOFU for a pinned host;
    /// always reject rather than silently falling back to trust-on-first-use.
    PinnedMismatch,
    /// Matches the fingerprint previously confirmed for this host.
    KnownMatch,
    /// A fingerprint *was* previously confirmed for this host, but it
    /// doesn't match what's being presented now -- ticket 05 checklist item
    /// 6, the textbook host-key-changed/MITM signature.
    KnownMismatch { previous_fingerprint: String },
    /// Never confirmed before, and not a pinned host -- needs an explicit
    /// TOFU confirmation before this connection may proceed.
    Unknown,
}

/// Cerebrite's TOFU store: `credential::known_hosts_path`'s content, loaded
/// into memory. Read-only except via `confirm`, which is the *only* writer
/// and is only ever meant to be called once a user has explicitly confirmed
/// a shown fingerprint (see this module's doc comment) -- this type itself
/// has no opinion on when that's appropriate; it just persists the decision.
pub struct KnownHosts {
    entries: HashMap<String, String>,
}

impl KnownHosts {
    pub fn load(path: &Path) -> Result<Self> {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e).context("reading known_hosts"),
        };
        let mut entries = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((host, fingerprint)) = line.split_once(' ') {
                entries.insert(host.trim().to_string(), fingerprint.trim().to_string());
            }
        }
        Ok(Self { entries })
    }

    pub fn empty() -> Self {
        Self { entries: HashMap::new() }
    }

    /// Checks `fingerprint` for `host` against pinning first, then this
    /// store's previously-confirmed entries.
    pub fn check(&self, host: &str, fingerprint: &str) -> HostKeyCheck {
        if is_pinned_host(host) {
            return if pinned_fingerprint_matches(host, fingerprint) {
                HostKeyCheck::PinnedMatch
            } else {
                HostKeyCheck::PinnedMismatch
            };
        }
        match self.entries.get(host) {
            Some(known) if known == fingerprint => HostKeyCheck::KnownMatch,
            Some(known) => HostKeyCheck::KnownMismatch {
                previous_fingerprint: known.clone(),
            },
            None => HostKeyCheck::Unknown,
        }
    }

    /// Persists explicit confirmation of `fingerprint` for `host`, creating
    /// the file (and its parent dir) if this is the first entry ever
    /// written. Overwrites any previous entry for the host -- re-confirming
    /// after a `KnownMismatch` (the host's key legitimately rotated) is
    /// itself an explicit user action, not an automatic one; nothing in
    /// this module calls `confirm` on its own.
    pub fn confirm(path: &Path, host: &str, fingerprint: &str) -> Result<()> {
        let mut me = Self::load(path)?;
        me.entries.insert(host.to_string(), fingerprint.to_string());

        let mut hosts: Vec<&String> = me.entries.keys().collect();
        hosts.sort();
        let mut content = String::new();
        for host in hosts {
            content.push_str(host);
            content.push(' ');
            content.push_str(&me.entries[host]);
            content.push('\n');
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("creating known_hosts parent dir")?;
        }
        fs::write(path, content).context("writing known_hosts")?;
        Ok(())
    }
}

/// Formats a raw SHA-256 host-key digest (as `git2::CertHostkey::hash_sha256`
/// hands it back) the way OpenSSH tooling displays it: `SHA256:<base64,
/// no padding>`. Matches what `ssh-keygen -lf`/a real SSH client would show
/// a user, so a fingerprint Cerebrite surfaces in a TOFU dialog is directly
/// comparable to one the user might check out-of-band.
pub fn format_sha256_fingerprint(digest: &[u8]) -> String {
    format!("SHA256:{}", base64_no_pad(digest))
}

const BASE64_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_no_pad(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(BASE64_ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(BASE64_ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(BASE64_ALPHABET[(n & 0x3f) as usize] as char);
        }
    }
    out
}

/// Human-readable messages for the `git2::Error` `connection.rs`'s
/// `certificate_check` callback returns on rejection. These are *not* how
/// `sync.rs` recovers which host/fingerprint failed and how (that's
/// `Connection::last_host_key_check`, a side channel) -- libgit2's SSH
/// transport discards whatever message this callback's `Err` carries and
/// substitutes its own fixed "invalid or unknown remote ssh hostkey" once it
/// decides the cert isn't valid (confirmed empirically while implementing
/// ticket 05: see `connection.rs`'s `check_ssh_host_key` doc comment). Kept
/// only because *something* descriptive has to go in the `Err` git2 itself
/// requires; the callback's real communication path is the side channel.
pub fn unknown_host_key_message(host: &str, fingerprint: &str) -> String {
    format!("unknown SSH host key for {host}: {fingerprint}")
}

pub fn host_key_mismatch_message(host: &str, fingerprint: &str) -> String {
    format!("SSH host key mismatch for {host}: presented {fingerprint}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // -- pinning --

    #[test]
    fn githubs_and_gitlabs_published_ed25519_fingerprints_are_pinned() {
        assert!(pinned_fingerprint_matches(
            "github.com",
            "SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU"
        ));
        assert!(pinned_fingerprint_matches(
            "gitlab.com",
            "SHA256:eUXGGm1YGsMAS7vkcx6JOJdOGHPem5gQp4taiCfCLB8"
        ));
    }

    #[test]
    fn a_pinned_host_with_a_non_matching_fingerprint_is_a_pinned_mismatch_never_tofu() {
        let known_hosts = KnownHosts::empty();
        let check = known_hosts.check("github.com", "SHA256:not-the-real-key");
        assert_eq!(check, HostKeyCheck::PinnedMismatch);
    }

    #[test]
    fn a_pinned_host_with_the_right_fingerprint_matches_without_needing_the_tofu_store() {
        let known_hosts = KnownHosts::empty();
        let check = known_hosts.check("github.com", "SHA256:+DiY3wvvV6TuJJhbpZisF/zLDA0zPMSvHdkr4UvCOqU");
        assert_eq!(check, HostKeyCheck::PinnedMatch);
    }

    // -- TOFU / KnownHosts --

    #[test]
    fn an_unpinned_host_never_seen_before_is_unknown() {
        let known_hosts = KnownHosts::empty();
        assert_eq!(
            known_hosts.check("git.example.test", "SHA256:abc"),
            HostKeyCheck::Unknown
        );
    }

    #[test]
    fn confirm_persists_and_a_later_load_matches_the_same_fingerprint() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("known_hosts");

        KnownHosts::confirm(&path, "git.example.test", "SHA256:abc").unwrap();

        let known_hosts = KnownHosts::load(&path).unwrap();
        assert_eq!(
            known_hosts.check("git.example.test", "SHA256:abc"),
            HostKeyCheck::KnownMatch
        );
    }

    #[test]
    fn a_host_previously_confirmed_under_a_different_fingerprint_is_a_known_mismatch() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("known_hosts");
        KnownHosts::confirm(&path, "git.example.test", "SHA256:original").unwrap();

        let known_hosts = KnownHosts::load(&path).unwrap();
        assert_eq!(
            known_hosts.check("git.example.test", "SHA256:changed"),
            HostKeyCheck::KnownMismatch {
                previous_fingerprint: "SHA256:original".to_string()
            }
        );
    }

    #[test]
    fn confirm_overwrites_a_previous_entry_for_the_same_host_rather_than_duplicating_it() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("known_hosts");
        KnownHosts::confirm(&path, "git.example.test", "SHA256:original").unwrap();
        KnownHosts::confirm(&path, "git.example.test", "SHA256:rotated").unwrap();

        let known_hosts = KnownHosts::load(&path).unwrap();
        assert_eq!(
            known_hosts.check("git.example.test", "SHA256:rotated"),
            HostKeyCheck::KnownMatch
        );
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 1);
    }

    #[test]
    fn confirming_one_host_never_affects_another() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("known_hosts");
        KnownHosts::confirm(&path, "a.example.test", "SHA256:a").unwrap();
        KnownHosts::confirm(&path, "b.example.test", "SHA256:b").unwrap();

        let known_hosts = KnownHosts::load(&path).unwrap();
        assert_eq!(known_hosts.check("a.example.test", "SHA256:a"), HostKeyCheck::KnownMatch);
        assert_eq!(known_hosts.check("b.example.test", "SHA256:b"), HostKeyCheck::KnownMatch);
    }

    #[test]
    fn loading_a_missing_file_is_an_empty_store_not_an_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist");
        let known_hosts = KnownHosts::load(&path).unwrap();
        assert_eq!(known_hosts.check("anything", "SHA256:x"), HostKeyCheck::Unknown);
    }

    // -- fingerprint formatting: cross-checked against real `ssh-keygen -lf`
    // / well-known SHA-256 base64 output for a trivial input, to catch a
    // padding or alphabet mistake in the hand-rolled base64 encoder above.

    #[test]
    fn formats_a_raw_digest_like_openssh_does() {
        // "abc"'s SHA-256 digest, base64 (no padding) is a value that's easy
        // to independently verify: `printf abc | sha256sum` then base64.
        let digest = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ];
        assert_eq!(
            format_sha256_fingerprint(&digest),
            "SHA256:ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0"
        );
    }

    // -- error message builders (human-readable text only; see their doc
    // comment for why the structured host/fingerprint travels via
    // `Connection::last_host_key_check` instead) --

    #[test]
    fn message_builders_name_the_host_and_fingerprint() {
        assert!(unknown_host_key_message("git.example.test", "SHA256:abc").contains("git.example.test"));
        assert!(unknown_host_key_message("git.example.test", "SHA256:abc").contains("SHA256:abc"));
        assert!(host_key_mismatch_message("github.com", "SHA256:bogus").contains("github.com"));
        assert!(host_key_mismatch_message("github.com", "SHA256:bogus").contains("SHA256:bogus"));
    }
}
