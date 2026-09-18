# Research: SSH Authentication in Cerebrite

(Full research subagent output for ticket 03 — SSH key management.)

## Current state (verified in repo)

`src-tauri/src/sync.rs:78-98` only tries `Cred::ssh_key_from_agent`, then `credential_helper`, then `Cred::default()` — no `ssh_key`/file-path path and no `ssh_key_from_memory` path exists today. `src-tauri/Cargo.toml:36` declares `git2 = { version = "0.19", features = ["vendored-openssl"] }` — the `ssh_key_from_memory` cargo feature is **not** currently enabled.

---

## 1. What git2/libssh2 supports as built here

`git2::Cred` exposes three SSH-relevant constructors:
- `Cred::ssh_key(username, pubkey_path, privkey_path, passphrase)` — reads key files from disk via libssh2's `libssh2_userauth_publickey_fromfile`.
- `Cred::ssh_key_from_memory(username, pubkey_blob, privkey_blob, passphrase)` — passes key material as strings to libgit2's `git_credential_ssh_key_memory_new`, which calls libssh2's `libssh2_userauth_publickey_frommemory`.
- `Cred::ssh_key_from_agent(username)` — talks to a running ssh-agent over `SSH_AUTH_SOCK` (or Pageant/OpenSSH-agent named pipe on Windows).

**Key finding: `ssh_key_from_memory` is gated behind a separate, non-default Cargo feature.** docs.rs feature listing for `git2` shows `ssh_key_from_memory` as optional (docs.rs/crate/git2/latest/features); Fedora's own package split (`rust-git2+ssh_key_from_memory-devel`) confirms downstream packagers treat it as distinct/opt-in. Cerebrite's current `Cargo.toml:36` does not enable it — must be added:
```toml
git2 = { version = "0.19", features = ["vendored-openssl", "ssh_key_from_memory"] }
```
This tracks the long-standing libgit2 issue "support in-memory ssh keys" (github.com/libgit2/libgit2/issues/5067) and ssh2-rs issue "Authenticate from in-memory keypair" (github.com/rust-lang/ssh2-rs/issues/77), both documenting this as historically footgun-prone and version-sensitive.

**Backend dependency:** In-memory key auth in libssh2 is only wired up through the OpenSSL crypto backend (`libssh2_userauth_publickey_frommemory` is conditionally compiled per-backend; see libssh2's OpenSSL Backend docs at deepwiki.com/libssh2/libssh2/3.1-openssl-backend). Since Cerebrite already uses `vendored-openssl` (forcing `libssh2-sys` to statically link a bundled OpenSSL rather than system OpenSSL/WinCNG/Secure Transport), this precondition is satisfied — but not automatically; depends on `libssh2-sys`'s vendored build choosing the OpenSSL backend, which `vendored-openssl` forces. Confirmed via docs.rs/libssh2-sys and github.com/rust-lang/ssh2-rs.

**Ed25519 specifically:** libssh2 added `ssh-ed25519` user-auth and host-key support in its 1.9.x/1.10+ lineage, signing performed through OpenSSL's `EVP_PKEY_ED25519` EVP API, requiring **OpenSSL ≥ 1.1.1** (or LibreSSL ≥ 3.7.0) (libssh2.org/changes.html; docs.openssl.org/1.1.1/man7/Ed25519/). Given `vendored-openssl` pulls in a modern OpenSSL and git2 0.19 is current, ed25519 should be functional — **but must be smoke-tested in CI**, not just assumed, since it's a recurring source of "silently falls back to RSA-only" bugs historically (github.com/libssh2/libssh2/discussions/1608).

**Caveat on key file format**: libgit2/libssh2 have had trouble with the "new" OpenSSH private-key format (bcrypt-KDF encrypted) vs. classic PEM in some older libssh2 builds (github.com/rust-lang/git2-rs/issues/659). For `ssh_key_from_memory`, Cerebrite controls key generation/serialization itself (§3), so it can choose whichever blob format its vendored libssh2 actually parses — validate experimentally against the exact `libssh2-sys` version pinned by `git2 0.19`.

**Action item**: enable the `ssh_key_from_memory` feature and add an integration test that round-trips an ed25519 key generated via the chosen Rust crate through `Cred::ssh_key_from_memory` against a local test SSH server (or a real provider) before shipping.

---

## 2. Agent availability across platforms

Confirms the framing that ssh-agent presence is inconsistent enough it can never be the *primary* mechanism:

- **Windows**: OpenSSH Authentication Agent (`ssh-agent` service) ships with Windows 10+ but is **disabled by default** (`Get-Service ssh-agent` shows `StartupType: Disabled`). Must be explicitly enabled. See interworks.com write-up and a live git2-rs failure report: github.com/rust-lang/git2-rs/issues/659 ("Failed SSH authentication on Windows, success on Linux"). PuTTY/Pageant is a separate, non-default agent some users run instead.
- **Linux**: highly desktop-environment-dependent. GNOME (gnome-keyring) commonly auto-starts an SSH-agent-compatible component and sets `SSH_AUTH_SOCK` in graphical sessions (wiki.gnome.org/Projects/GnomeKeyring/Ssh) but can be disabled. KDE does **not** run an agent by default (needs KWallet + `ksshaskpass`), further complicated on Wayland/Plasma (discuss.kde.org/t/solved-plasma-6-wayland-not-starting-ssh-agent/11748). Headless/server Linux has no agent unless the user starts one.
- **macOS**: best supported — Apple-patched OpenSSH integrates with Keychain (`ssh-add --apple-use-keychain`); after an initial `ssh-add`, passphrase retrieved from Keychain on subsequent unlocks. Still requires that initial `ssh-add`.
- **Android**: **no OS-level ssh-agent concept at all.** No `SSH_AUTH_SOCK`, no analogous system daemon. Third-party workarounds exist only inside specialized environments (Termux's `ssh-android-agent`, github.com/haraldh/ssh-android-agent) but are not usable/integrable by a normal Tauri/Android app. **Structurally disqualifies** any design that requires agent-based auth — confirms agent auth must remain strictly an optional acceleration path, never the only path.

**Conclusion**: `ssh_key_from_agent` can stay as an opportunistic first attempt (cheap, no cost if absent), but the app must supply its own credential material (file-based or in-memory key) as the guaranteed path on every platform, including future Android.

---

## 3. In-app ed25519 key generation in pure Rust

Goal: generate an ed25519 keypair in pure Rust without shelling out to `ssh-keygen`, producing blobs `Cred::ssh_key_from_memory` can consume.

### Candidate: `ssh-key` (RustCrypto/SSH) — **recommended**
- Provides `PrivateKey::random(&mut OsRng, Algorithm::Ed25519)`, `.to_openssh()` for private-key serialization, `.public_key().to_string()` for the OpenSSH-format public key line, `.encrypt(&mut rng, passphrase)` for optional passphrase encryption — no system binary needed. Docs: docs.rs/ssh-key/latest/ssh_key/.
- Freshness: latest 0.6.x (0.6.7) published **October 15, 2024**, with further activity into 2025; maintained by RustCrypto member `tarcieri` under github.com/RustCrypto/SSH. Supports RFC4251/4253/OpenSSH formats, ed25519/ECDSA/RSA/DSA, `authorized_keys`/`known_hosts` parsing — reusable for §6 host-key verification. **One crate covers both key generation and known_hosts needs.**
- No EOL/deprecation signal found.

### Candidate: `ed25519-dalek`
- Lower-level; raw ed25519 keys/signatures, not SSH wire/file formats — would need manual SSH blob encoding.
- **Maintenance flag**: `dalek-cryptography/ed25519-dalek` GitHub repo is marked **ARCHIVED/MOVED**, redirecting elsewhere. Last crates.io publish seen: 2024-02-07. Ambiguous-EOL signal the standing constraint calls out. **Recommendation: avoid as a direct dependency**; acceptable only if pulled in transitively.

### Candidate: `russh-keys`
- Part of the `russh` SSH implementation ecosystem, v0.49.2. Third-party health score only 51/100 (rustio.net/crate/russh-keys); not yet folded into core `russh` (github.com/Eugeny/russh/discussions/315). Overkill for Cerebrite's need (pulls in a full SSH client stack) when git2 already uses libssh2 — **not recommended as first choice**, but not disqualified as EOL either.

**Recommendation**: use `ssh-key` (features `["ed25519", "getrandom"]`, plus `"encryption"` if passphrases are supported) for both key generation and later known_hosts parsing.

---

## 4. Should the app-generated key carry a passphrase?

Real tension: sync must run **unattended in the background** (no server, no human present to type a passphrase each cycle).

- If the app encrypts the key with a passphrase and must decrypt it automatically for background sync, that passphrase must itself be retrievable without user interaction — typically the OS credential store. At that point the passphrase provides negligible additional protection over storing the raw private key directly in that same OS-native store — it moves, rather than removes, the vulnerable point.
- Comparable tooling: **GitKraken** makes the passphrase optional at generation time and recommends adding the key to the OS ssh-agent (itself backed by OS keychain on macOS via `ssh-add --apple-use-keychain`) for hands-free operation — i.e. delegates passphrase-protection to OS-level secure storage rather than reimplementing it (help.gitkraken.com/gitkraken-desktop/authentication/).
- **Recommendation**: store the private key material directly in the platform's native secure storage (OS keychain via a `keyring`-style crate, or `tauri-plugin-stronghold`) **without an additional user-memorized passphrase** for the automated background-sync path. This is honest — the real protection boundary is "whatever protects the OS keychain" (login/device unlock/biometric), not a separate passphrase stored next to the key it protects. If a passphrase is offered at all, it should be an optional, explicit choice understood to disable unattended background sync until re-entered — never silently auto-supplied.

---

## 5. Getting the public key to the provider

Manual copy-paste is always the fallback (no OAuth token, self-hosted/unknown provider), but all four target providers expose SSH-key upload as a documented API when an OAuth token with sufficient scope is available (GitHub, GitLab, Bitbucket, Gitea/Forgejo each have a key-upload endpoint — see ticket 02's OAuth research for token acquisition). The second path is what would make SSH "effortless" for GitHub/GitLab: acquire an OAuth token via device flow or PKCE, then use it once to upload the freshly generated public key via API, without the user ever leaving the app to paste anything.

---

## 6. Host key verification

- libssh2 does **not** verify host keys safely by default — the calling application is responsible for host-key checking; naively accepting any host key (as `Cred::default()`/no-callback setups commonly do) is equivalent to disabling MITM protection entirely.
- `known_hosts` conventionally lives at `~/.ssh/known_hosts` on Linux/macOS and `%USERPROFILE%\.ssh\known_hosts` on Windows (OpenSSH-for-Windows convention) — Cerebrite has no existing `.ssh` directory management and would need to create/manage its own equivalent file (or reuse the user's existing one, which raises its own "is it there / is it writable" questions).
- Honest options for a GUI app with no terminal: (a) TOFU (trust-on-first-connect) with an explicit GUI confirmation dialog showing the fingerprint, mirroring what a terminal SSH client would prompt, recording the accepted key in a Cerebrite-managed known_hosts-equivalent file; (b) pinning well-known providers' published host keys (GitHub, GitLab, Bitbucket, Codeberg all publish their SSH host key fingerprints on public docs pages) so no prompt is needed for the two-tier "effortless" providers, falling back to TOFU-with-dialog for arbitrary self-hosted/bare-SSH hosts. Do not silently accept-and-continue with no user-visible confirmation for unknown hosts — that is the difference between SSH being secure and merely appearing to work.

---

## Summary: what an MVP "ship SSH auth" implementation needs

1. Enable the `ssh_key_from_memory` git2 feature; smoke-test ed25519 in-memory auth in CI against a real or local test SSH endpoint.
2. Generate ed25519 keys in pure Rust via the `ssh-key` crate; no passphrase for the automated path, stored via OS-native secure storage (ties to ticket 04's findings).
3. Treat `ssh_key_from_agent` as an opportunistic, non-required first attempt only — never the sole path, and never usable at all on Android.
4. Offer both manual copy-paste and (where an OAuth token is already held) automatic API-based public-key upload to the provider.
5. Implement explicit host-key handling: pin known providers' published fingerprints for the two effortless tiers; TOFU-with-GUI-confirmation-dialog for everything else, persisted to a Cerebrite-managed known_hosts-equivalent file. Never silently accept unknown host keys.
