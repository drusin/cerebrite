# SSH key management on desktop and Android

Type: research
Status: resolved

## Question

What does shipping SSH authentication actually entail, given `sync.rs` today only tries `ssh_key_from_agent` — a path that does not exist on Android and is not guaranteed on a fresh desktop either?

Establish:

- **What `git2`/libssh2 supports as built here**: `Cred::ssh_key` (key file paths), `Cred::ssh_key_from_memory`, and `ssh_key_from_agent`. Confirm `ssh_key_from_memory` is actually compiled in for this project's `git2 0.19` + `vendored-openssl` configuration — it depends on the libssh2 backend and is a known footgun. Confirm **ed25519** support specifically, since that is the key type providers now recommend and libssh2's support has historically lagged.
- **Agent availability**: which desktop platforms reliably have an ssh-agent running with keys loaded (Linux varies by distro/session, Windows needs the OpenSSH agent service enabled, macOS is generally fine), and confirm Android has none — so agent auth can only ever be an optimization, never the mechanism.
- **In-app key generation**: what it takes to generate an ed25519 keypair in Rust and hand the private half to libssh2, avoiding any dependency on a system `ssh-keygen` binary. Name candidate crates and check them against the no-EOL-dependency constraint.
- **Passphrases**: whether an app-generated key should carry one at all, given the app would have to store the passphrase somewhere to run background sync unattended — which may just move the secret rather than protect it.
- **Getting the public key to the provider**: manual copy-paste into a settings page, versus uploading it through the provider API if an OAuth token is already in hand. The second is what would make SSH "effortless" for GitHub/GitLab; establish which providers expose a key-upload endpoint and what scope it needs.
- **Host key verification**: what libssh2 does by default (it is not automatically safe), where a `known_hosts` file would live on each platform, and what the honest options are for a GUI app that cannot show a terminal prompt. Do not let this one slide — it is the difference between SSH being secure and SSH merely appearing to work.

## Answer

**Shipping SSH auth is viable but needs real work beyond the current `ssh_key_from_agent`-only path — it is never usable as the sole mechanism because agent availability is inconsistent (Windows agent disabled by default; Linux depends on DE; Android has no agent concept at all).**

- `Cred::ssh_key_from_memory` is gated behind a Cargo feature not currently enabled (`git2 = { features = ["vendored-openssl", "ssh_key_from_memory"] }` needed) and is a historically footgun-prone path — must be smoke-tested in CI, including ed25519 specifically (supported since libssh2 1.9.x/1.10+ given OpenSSL ≥1.1.1, which `vendored-openssl` provides, but not to be assumed untested).
- In-app ed25519 key generation: use the **`ssh-key`** crate (RustCrypto/SSH, actively maintained) — covers keygen, OpenSSH serialization, and `known_hosts` parsing in one dependency. Avoid `ed25519-dalek` as a direct dependency (its GitHub repo is archived/moved). `russh-keys` is possible but overkill and lower health-score.
- **No passphrase** on app-generated keys for the automated background-sync path — a passphrase the app must auto-supply just relocates the secret to wherever the passphrase itself is stored (typically the same OS keychain), providing negligible extra protection. Store the raw private key directly in OS-native secure storage instead (ties to ticket 04's findings). An optional user passphrase should explicitly disable unattended sync until re-entered.
- Public key delivery: manual copy-paste always available as fallback; all four target providers expose a key-upload API when an OAuth token is already held (ties to ticket 02) — this is what makes SSH "effortless" for GitHub/GitLab.
- **Host key verification is not automatically safe in libssh2** — the app must implement it explicitly: pin known providers' published host-key fingerprints for the two effortless tiers, and use TOFU with an explicit GUI confirmation dialog (never silent accept) for arbitrary self-hosted/bare-SSH hosts, persisted to a Cerebrite-managed `known_hosts`-equivalent file.

Full findings and sources: [research report](03-ssh-key-management-research.md).
