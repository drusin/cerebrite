# 05: SSH key connection

**What to build:** Cerebrite-managed SSH key auth, per [ticket 03's research](../../git-provider-integration/issues/03-ssh-key-management.md) — generated or imported, with real host-key verification instead of libssh2's unsafe default.

From the user's perspective: connecting via SSH, the user either generates a new key in-app (the default, ed25519, no passphrase) or imports an existing one. The generated key's public half is shown with a copy button and a link to the host's "add SSH key" page. On first contact with a host, its key is either already pinned (GitHub/GitLab) or the user is shown a TOFU confirmation dialog naming the fingerprint before it's trusted. A test fetch runs before the Connection is saved. As with ticket 04, this ticket's UI can be minimal — the wizard integration is tickets 09–11.

**Blocked by:** 02, 03

- [ ] `git2`'s `ssh_key_from_memory` Cargo feature is enabled and smoke-tested (including ed25519 specifically) in this project's `vendored-openssl` configuration
- [ ] In-app ed25519 key generation via the `ssh-key` crate, no dependency on a system `ssh-keygen` binary
- [ ] Generated keys carry no passphrase by default (raw private key stored via ticket 02's keychain infra); importing a passphrase-protected key stores the passphrase alongside it so unattended sync keeps working, or the user is told unattended sync is disabled until the passphrase is re-entered
- [ ] Host-key verification: GitHub/GitLab's published fingerprints are pinned; any other host triggers a TOFU dialog showing the fingerprint, persisted to ticket 02's Cerebrite-owned `known_hosts` file only on explicit confirmation — never silent accept
- [ ] A test fetch using the SSH credential runs before the Connection is saved
- [ ] A host whose key the app no longer accepts (changed fingerprint) surfaces through ticket 03's classification as a needs-attention failure
- [ ] Manual smoke test against a real SSH-only host (e.g. a bare SSH test repo) covering both generate and import paths
- [ ] Integration test: generated key connects and pushes/pulls against a local SSH-serving fixture (or equivalent), and a mismatched host key is rejected rather than silently trusted
