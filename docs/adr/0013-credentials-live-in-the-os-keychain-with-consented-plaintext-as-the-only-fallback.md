---
status: accepted
---

# Credentials live in the OS keychain, with consented plaintext as the only fallback

Every [connection](../../CONTEXT.md) (ADR-0012) holds a secret: an OAuth access token and its rotating refresh token, a pasted access token, or an SSH private key (plus an imported key's passphrase). Sync runs unattended, so the secret must be readable without the user present, which rules out anything that asks for a password every launch. Migrating stored secrets after release is painful, so the store is decided now rather than left to whatever the first implementation happened to call.

A connection's secret lives in exactly one **credential store**: the **system keychain** by default, or an **unencrypted file** only when the keychain is unavailable and the user has consented for that connection.

- **System keychain.** This is the only default, reached through `keyring-core` and one store crate per platform: Windows Credential Manager, Linux Secret Service (KWallet's `ksecretd` or gnome-keyring), and later Android Keystore. Each connection has one entry, keyed by a random connection ID, which holds all of that connection's secrets serialized together as raw bytes. On a fresh CachyOS install the keychain works out of the box on the default KDE edition and on GNOME, Cinnamon and Budgie. It very likely works on Hyprland, Niri, MangoWM and Qtile too, since CachyOS pulls in gnome-keyring and the login manager's PAM unlocks it. It is absent on Xfce, MATE, LXQt, LXDE, Sway, Wayfire, i3, bspwm and Openbox, where the call fails fast with `ServiceUnknown`.
- **Unencrypted file.** A file with `0600` permissions in the app config dir, one per connection ID. It is offered only after a keychain probe has actually failed, and it is never a preference the user picks up front. The same dialog offers "Set up a keychain instead" (install gnome-keyring, or enable KeePassXC's Secret Service integration, then log in again) with a "Check again" button.

The security claim is stated, never implied. The consent dialog and the connection's details both say: *"Stored unencrypted in `<path>`. Anyone or anything that can read your home folder can use this credential to push to your repository."* The words "encrypted" and "secure" never appear for this store. The keychain gets a neutral line of text, not a padlock. The docs, not the UI, note that on Linux any process running as the user can read an unlocked keyring.

The store never changes on its own. If the keychain becomes unreachable after a connection was saved (the user switched from GNOME to sway, or the keyring is locked), sync fails with a failure that names the keychain. A plaintext connection moves to the keychain only through an explicit "Move to keychain" action. Chromium and Electron apps do the opposite: they silently fall back to a hard-coded-key "basic" store. That is the behaviour this ADR exists to rule out.

## Details

- **Interactive vs. background calls.** All keychain calls run off the UI thread.
  - *Connect time (user present):* wait up to about 2 minutes for an unlock or create-wallet prompt, with a visible "Waiting for your system keychain…" and a Cancel button.
  - *Background sync:* never raise a prompt. A locked collection fails fast as "keychain locked", with an "Unlock and retry" action that takes the interactive path.
  - The `secret-service` crate waits on unlock prompts with **no timeout**, so a bare call can block forever. Both paths therefore need their own time limit.
- **Only the secret goes in the store.** A non-secret **connection record** sits in `.git/cerebrite/connection.json`, inside the repository's git dir. It is never committed, moves with the folder, and is never picked up by a fresh clone. It holds the connection ID, credential kind, HTTPS username, provider, token expiry and which credential store is in use. `origin` stays in `.git/config`, and so does the vault's [author](../../CONTEXT.md), which belongs to the vault rather than its connection. `settings.json` stays app-level and gains only an index of connection IDs and their stores.
- **Host keys** (pinned and TOFU) are stored app-wide in a Cerebrite-owned `known_hosts` file in the config dir. A host's identity is a fact about the host, not the vault. Disconnecting a vault does not forget them. `~/.ssh/known_hosts` is never read or written.
- **Windows size limit.** A Credential Manager blob holds at most 2560 bytes, and the store rejects anything larger rather than chunking it. Every token, ed25519 key, ECDSA key and RSA-2048 key fits. An imported RSA key of 3072 bits or more is refused on Windows, with the reason and a one-click switch to generating a key.
- **Rotating refresh tokens.** GitHub and GitLab both invalidate the old refresh token on use. Before each refresh, Cerebrite probes that the store is writable and skips the refresh if the probe fails. If a write fails after a successful refresh anyway, the new pair is kept in memory, sync continues, and a named warning says the sign-in could not be saved and will be lost when the app quits.
- **Disconnect** deletes the secret, the connection record and the index entry.
  - *GitLab:* it also revokes the refresh token at the provider, because GitLab's `/oauth/revoke` accepts a public client with no secret.
  - *Everything else:* nothing can be revoked cleanly, because GitHub's revoke endpoints need the client secret and Cerebrite never issued access tokens or uploaded SSH keys. The dialog names what is still valid and where to remove it: GitHub's "Authorized GitHub Apps" page, the host's token page, or the host's SSH key page. It never claims a revocation that did not happen.
- **Orphans.** Deleting a repository outside the app leaves its keychain entry behind. At startup, Cerebrite uses the index to offer to remove entries whose repository no longer exists, and Settings has "Remove all stored Cerebrite credentials".

## Considered options

- **Refuse without a keychain.** Rejected. It is honest, but it locks out about half the CachyOS editions, which are this project's own target users.
- **Master passphrase (Stronghold + Argon2).** Rejected. It gives real protection, but it adds a product concept Cerebrite doesn't have, and unattended sync would need the passphrase at every launch.
- **App-managed encrypted file without a keychain or passphrase key.** Rejected as security theatre: the key would sit next to the ciphertext.
- **Envelope encryption for every secret** (a data key in the keychain, the ciphertext in a file). Deferred. It would remove the Windows size limit, but it adds a second thing to lose, and only imported legacy RSA keys need it today.
- **Git credential helpers / `keyutils`.** Rejected. Helpers are what ADR-0012 removed, and the kernel keyring is cleared at reboot.
- **Connection record in `settings.json`, keyed by vault path.** Rejected. It breaks when the folder is moved, and it goes stale when a repository is deleted and re-cloned.
