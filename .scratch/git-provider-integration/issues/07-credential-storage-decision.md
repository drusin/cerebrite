# Where credentials live at rest

Type: grilling
Status: resolved
Blocked by: 04, 06

## Question

Given the mechanisms that ticket 06 settled on and the storage options ticket 04 surveyed, where do Cerebrite's credentials actually live — and what is the honest security claim?

Hard to reverse (migrating stored secrets after release is genuinely painful), surprising without context, and a real trade-off between portability and protection. This should produce an ADR.

Settle:

- **The chosen mechanism**, per platform, including what happens on Windows, on a Linux box with no secret service, and what the Android path will be when it is eventually built.
- **The fallback when it is unavailable.** A keychain that fails on a user's machine must not leave sync permanently broken with no explanation. Refuse and explain, degrade to a less protected store with visible consent, or something else — but decided, not accidental.
- **The security claim.** State plainly what an attacker with read access to the user's home directory gets, and make sure the UI never promises more than that. If the real answer is "a file-permission-protected token", say so rather than implying encryption that does not exist.
- **Scope of what is stored.** Just the credential, or also the remote URL, the chosen mechanism, and the provider identity? Some of that is not secret and may belong in `settings.json` next to `vaultPath` and `theme`; splitting it deliberately is better than putting everything in the most awkward store.
- **Revocation and clearing.** How a user disconnects a vault from its provider and is genuinely confident the credential is gone.

## Answer

Written up as **[ADR-0013](../../../docs/adr/0013-credentials-live-in-the-os-keychain-with-consented-plaintext-as-the-only-fallback.md)**. One new glossary entry was added to **[CONTEXT.md](../../../CONTEXT.md)**: `Credential store`.

### The chosen mechanism, per platform

**The OS keychain is the only default.** It is reached through `keyring-core` and one store crate per platform:
- **Windows:** Credential Manager.
- **Linux:** Secret Service (KWallet's `ksecretd` or gnome-keyring).
- **Android, later:** Keystore via `android-native-keyring-store`.

Git credential helpers and `keyutils` are out.

**CachyOS, checked against the installer's current package lists:**
- **Works out of the box** with a password login: KDE (the default), GNOME, Cinnamon, Budgie.
- **Very likely works:** Hyprland, Niri, MangoWM, Qtile. CachyOS's settings packages pull in gnome-keyring, and the login manager's PAM unlocks it. This is inferred from the package lists, not tested.
- **Shaky:** COSMIC, which has an open unlock bug.
- **Nothing installed:** Xfce, MATE, LXQt, LXDE, Sway, Wayfire, i3, bspwm, Openbox. The call fails fast with `ServiceUnknown`.
- **Autologin:** every edition prompts on first use.

**The hang risk is a locked collection with an unanswered prompt**, not a missing daemon. The `secret-service` crate waits on the unlock prompt with no timeout. So keychain calls run off the UI thread, in one of two modes:
- **Interactive (connect time):** wait up to about 2 minutes, show "Waiting for your system keychain…", and offer Cancel.
- **Background (sync):** never prompt. A locked keychain fails fast as "keychain locked", with an "Unlock and retry" action.

### Fallback when it is unavailable

**A plaintext file with consent.** It is a `0600` file in the app config dir, one per connection ID.
- It is offered only after a keychain probe has actually failed, never as an up-front preference.
- Consent is given per connection and recorded.
- The same dialog offers "Set up a keychain instead" (gnome-keyring or KeePassXC's Secret Service) and a "Check again" button.

**The store never changes on its own.**
- If the keychain becomes unreachable later, sync fails with a failure that names the keychain.
- A plaintext connection moves to the keychain only through an explicit "Move to keychain" action.

### The security claim

- **Unencrypted file:** *"Stored unencrypted in `<path>`. Anyone or anything that can read your home folder can use this credential to push to your repository."* It is shown in the consent dialog and permanently on the connection's details. The words "encrypted" and "secure" never appear for this store.
- **Keychain:** a neutral line of text, no padlock. The Linux caveat, that any process running as the user can read an unlocked keyring, goes in the docs.

### Scope of what is stored

**Secrets:** one keychain entry per connection, holding all its secrets serialized together as raw bytes.

**Non-secret connection record:** `.git/cerebrite/connection.json`, in the repository's git dir. It is never committed, moves with the folder, and is never picked up by a fresh clone. It holds:
- the connection ID;
- the credential kind;
- the HTTPS username;
- the provider;
- the token expiry;
- which credential store is in use.

`origin` stays in `.git/config`, and so does the vault's author: see [Git author identity in the connect journey](11-git-author-identity-product.md), which moved it out of the connection record.

**App level:** `settings.json` gains only an index of connection IDs and their stores.

**Host keys:** an app-wide, Cerebrite-owned `known_hosts` in the config dir, never `~/.ssh/known_hosts`. Disconnecting a vault does not forget them.

**Windows size limit:** a Credential Manager blob holds at most 2560 bytes, and the store rejects larger ones. An imported RSA key of 3072 bits or more is refused on Windows, with a one-click switch to generating a key. Envelope encryption is deferred.

**Rotating refresh tokens:**
- Before each refresh, Cerebrite probes that the store is writable.
- If a write fails after a successful refresh anyway, the new pair is kept in memory, sync continues, and a named warning is shown. The pair is lost when the app quits.

### Revocation and clearing

**Disconnect** deletes the secret, the connection record and the index entry.

**Revocation at the provider:**
- **GitLab:** revokes the refresh token, because `/oauth/revoke` accepts a public client with no secret. This was verified in GitLab/Doorkeeper source code; the docs don't say it.
- **GitHub:** both revoke endpoints need the client secret, and the only secret-free route is the leaked-credential endpoint, which is off-label. So Disconnect directs the user to Settings → Applications → Authorized GitHub Apps.
- **Access tokens and SSH keys:** the dialog names the host's token page or SSH key page.
- **In every case:** the dialog never claims a revocation that did not happen.

**Orphans:** at startup, the index drives an offer to clean up entries whose repository no longer exists. Settings has "Remove all stored Cerebrite credentials".

### Handed on

- To [Surfacing sync state and auth failure](09-sync-and-auth-failure-surfacing.md): three more named failures, plus the credential-loss fog item, now graduated into that ticket:
  - "keychain locked" (with Unlock and retry);
  - "keychain unreachable";
  - "couldn't save your refreshed sign-in" (lost on quit).
- To [The connect and clone onboarding journey](08-connect-and-clone-journey.md): three screens.
  - the keychain wait screen;
  - the plaintext consent dialog, with "Set up a keychain instead";
  - the Disconnect flow, with its per-kind "what's still valid" step.
