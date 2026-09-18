# SSH key management on desktop and Android

Type: research
Status: open

## Question

What does shipping SSH authentication actually entail, given `sync.rs` today only tries `ssh_key_from_agent` — a path that does not exist on Android and is not guaranteed on a fresh desktop either?

Establish:

- **What `git2`/libssh2 supports as built here**: `Cred::ssh_key` (key file paths), `Cred::ssh_key_from_memory`, and `ssh_key_from_agent`. Confirm `ssh_key_from_memory` is actually compiled in for this project's `git2 0.19` + `vendored-openssl` configuration — it depends on the libssh2 backend and is a known footgun. Confirm **ed25519** support specifically, since that is the key type providers now recommend and libssh2's support has historically lagged.
- **Agent availability**: which desktop platforms reliably have an ssh-agent running with keys loaded (Linux varies by distro/session, Windows needs the OpenSSH agent service enabled, macOS is generally fine), and confirm Android has none — so agent auth can only ever be an optimization, never the mechanism.
- **In-app key generation**: what it takes to generate an ed25519 keypair in Rust and hand the private half to libssh2, avoiding any dependency on a system `ssh-keygen` binary. Name candidate crates and check them against the no-EOL-dependency constraint.
- **Passphrases**: whether an app-generated key should carry one at all, given the app would have to store the passphrase somewhere to run background sync unattended — which may just move the secret rather than protect it.
- **Getting the public key to the provider**: manual copy-paste into a settings page, versus uploading it through the provider API if an OAuth token is already in hand. The second is what would make SSH "effortless" for GitHub/GitLab; establish which providers expose a key-upload endpoint and what scope it needs.
- **Host key verification**: what libssh2 does by default (it is not automatically safe), where a `known_hosts` file would live on each platform, and what the honest options are for a GUI app that cannot show a terminal prompt. Do not let this one slide — it is the difference between SSH being secure and SSH merely appearing to work.
