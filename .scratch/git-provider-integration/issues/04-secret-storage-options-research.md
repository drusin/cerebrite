# Secret Storage Landscape for Cerebrite (Tauri 2, Windows + Linux now, Android later)

(Full research subagent output for ticket 04 — secret storage options.)

## Summary

There is no free lunch: every option trades off attacker-resistance against "what happens when the mechanism isn't there." OS keychains (via the `keyring`/`keyring-core` crate family, now actively maintained under `open-source-cooperative` — not EOL) give real protection on Windows and reasonable protection on a *properly-configured* desktop Linux session, but degrade to hard failures on headless/minimal-WM Linux (i3, sway, fresh Arch without a login manager) where no Secret Service daemon exists — this is a real, well-documented gap, not FUD. An app-managed encrypted file is close to meaningless (mere obfuscation) unless the key comes from somewhere the attacker doesn't also have (OS keychain or a user passphrase Cerebrite doesn't currently have). Delegating to git's own credential helpers/ssh-agent removes Cerebrite's exposure entirely but pushes the same "unconfigured machine" problem onto the user, and is essentially a dead end on Android. Plaintext-with-consent is exactly what several respected git-based tools (Obsidian Git plugin) already ship, and is a legitimate, honestly-labeled fallback for a single-user local-first tool — but it should be presented as a fallback, not a default.

---

## 1. OS-native keychains (`keyring` crate / Tauri plugins)

### Crate/plugin maintenance status (checked against "no EOL" constraint)

- **`keyring` crate** — historically maintained by hwchen, now under the **open-source-cooperative** org. As of v4.0 (2024), the project underwent a major architectural split: `keyring` is now mostly a CLI/sample-code wrapper, real logic lives in **`keyring-core`** plus per-platform store crates (`windows-native-keyring-store`, `secret-service`, `apple-native`, `android-native-keyring-store`, `keyutils`). Latest tag in snapshot: 4.1.6. Docs: `docs.rs/keyring/4.0.1`, repo: `github.com/open-source-cooperative/keyring-rs`. **Verdict: actively maintained, not deprecated** — target `keyring-core` + explicit store crates rather than the monolithic `keyring` crate, per maintainers' current guidance.
- **`zbus`** (D-Bus crate backing the Linux `secret-service` store) is actively maintained, pure-Rust, async, high download volume (`github.com/dbus2/zbus`). Not a risk factor.
- **`windows-native-keyring-store`** wraps Windows Credential Manager (`CRED_TYPE_GENERIC`), uses DPAPI under the hood.
- **`android-native-keyring-store`** exists and is functional but requires JNI glue (Kotlin/Java `System.loadLibrary` + `ndk-context` to pass the Android `Context` into Rust). Previously an open feature request, now implemented — treat as "supported, immature" rather than "supported, proven."
- **Tauri plugin layer**: no *official* `tauri-apps`-maintained keyring plugin.
  - `HuakunShen/tauri-plugin-keyring` — thin wrapper over `keyring`; last updated ~2 years ago at search time — **stale, flag as risk if chosen**.
  - `charlesportwoodii/tauri-plugin-keyring` — v2-targeted, claims Windows/macOS/Linux/Android/iOS support, last update ~3 months ago at search time, D-Bus Secret Service default on Linux with `keyutils` alternative. More current, but single-maintainer — do bus-factor due diligence.
  - Official Tauri secret-adjacent plugin is `tauri-plugin-stronghold` (§2) — actively maintained in `tauri-apps/plugins-workspace`, but a vault, not a keychain wrapper.

### Windows: Credential Manager
- **Mechanism**: DPAPI-encrypted "generic credentials" via Win32 Credential Manager APIs, wrapped by `windows-native-keyring-store`.
- **(a) Attacker with home-dir read access only**: DPAPI blobs live under `%APPDATA%\Microsoft\Credentials\`, masterkey under `%APPDATA%\Microsoft\Protect\<SID>`. Filesystem read access alone is **not sufficient** — masterkeys are wrapped with a key derived from the user's login password (PBKDF2). Needs files *and* password/NT hash/live session (tools: SharpDPAPI, dpapick, Impacket). Meaningfully better than plaintext against a "stole your backup" attacker, but not against a live-session/credential-dumping attacker (mimikatz-class).
- **(b) Failure mode**: first-class OS service on every supported Windows install, no extra daemon required, essentially never "isn't there." Strongest, most reliable of the four options on Windows specifically.

### Linux: Secret Service / D-Bus (via `secret-service` crate, backed by `zbus`)
- **Mechanism**: talks over D-Bus **session** bus to whatever Secret Service implementation is registered (typically `gnome-keyring-daemon` or `kwalletd`). "Login" keyring auto-unlocks at login via `pam_gnome_keyring.so` only if the keyring password matches login password and it's a normal password login (not auto-login).
- **Concrete headless/minimal-WM failure mode (confirmed)**: on i3/sway/fresh Arch/CachyOS without a full DE, generally **no D-Bus session bus and no Secret Service daemon started automatically**. User must explicitly install `gnome-keyring`/run KWallet standalone, start a D-Bus session, and launch `gnome-keyring-daemon` — nothing in a stock minimal setup does this by default (ArchWiki `GNOME/Keyring`).
  - When there's no Secret Service on the bus, `keyring`/`secret-service` calls surface as `keyring::Error::NoStorageAccess` or `PlatformFailure` — a clean, catchable Rust error, but some implementations **hang indefinitely** waiting for an unlock prompt that can never appear in headless contexts (`github.com/jaraco/keyring/issues/477`). Any implementation must set a timeout and hard error.
- **(a) Attacker with home-dir read access**: the Secret Service database (`~/.local/share/keyrings/*.keyring`) is encrypted with a key derived from the login keyring password — but if it auto-unlocks with the account password (common default) and the user is logged in, plaintext secrets are obtainable live via D-Bus by *any process running as that user* while the session is unlocked (explicitly GNOME's stated trust model). Pure offline-file attacker without the login password gets ciphertext; a co-resident-process attacker on an unlocked session gets plaintext just by asking.
- **(b) Failure mode summary**: strong on GNOME/KDE-full desktops, **absent by default** on minimal WMs and headless boxes — directly matching this project's stated CachyOS/Arch-minimal user base as a real, non-hypothetical risk.

### Android Keystore
- **Mechanism**: hardware-backed (TEE or StrongBox) key storage. Per Android's own docs, keys generated in the Keystore are **non-exportable by design**, even via attestation. Strongest guarantee of the four platforms when hardware backing is available.
- **Caveat**: Jetpack's `EncryptedSharedPreferences`/`EncryptedFile` (the common convenience wrapper) was **officially deprecated by Google in April 2025**; current recommendation is Jetpack DataStore + Google Tink. Does not deprecate Keystore itself — future Android work should go through `android-native-keyring-store`/raw Keystore APIs or Tink, not the deprecated convenience wrapper.
- Not evaluatable in detail today since Android is out of scope, but this is the one platform where the OS-native mechanism is unambiguously the right default with no realistic "daemon not running" failure mode — the reverse of the Linux desktop situation.

---

## 2. App-managed encrypted file

- Cerebrite has **no user password, PIN, or master passphrase anywhere today**. An "encrypted `settings.json`" can only get its key from: (a) a key stored in another file next to it, (b) a key derived from machine-specific/hardcoded identifiers, or (c) a key from the OS keychain / a new user-supplied passphrase.
- **Be blunt**: options (a) and (b) are **security theater**. If the decryption key sits in a plaintext file in the same config directory (or is derivable from public machine identifiers an attacker with filesystem access can also read), an attacker with home-dir read access gets the plaintext exactly as fast as unencrypted storage — encryption just adds one extra `decrypt()` call to their script. Worse than useless if it creates a false sense of security.
- **What would make it real**:
  - OS keychain holding the *encryption key* (not the secret itself) — but then you've reintroduced dependency on §1's keychain, inheriting all its Linux-headless failure modes, plus now two things that can fail.
  - A genuine user-supplied passphrase (Argon2/PBKDF2-derived key) — exactly what `tauri-plugin-stronghold`'s `Builder::with_argon2(&salt_path)` does. Stronghold is **officially maintained** by `tauri-apps/plugins-workspace`, not an EOL risk. But requires a new UX concept — a master password the user must set and remember — that doesn't exist in the product today, a real product decision not just a technical one. Without that passphrase, Stronghold degrades to the same "key stored next to ciphertext" problem, since its salt file is plaintext-on-disk.
  - **Conclusion**: only a genuine improvement over plaintext if paired with (i) OS keychain-derived key (redundant with §1) or (ii) a new user passphrase (new UX surface, only as strong as what the user picks/remembers).

---

## 3. Delegating entirely to git (credential helpers + `ssh-agent`)

- **Git credential helpers** (`git config credential.helper`): three common built-ins per git's own docs:
  - `cache` — in-memory only, default 15-min TTL, never touches disk, lost on restart — not viable as sole mechanism for "set it once" UX.
  - `store` — writes to `~/.git-credentials` in **plaintext**, no expiry. Git's own "durable" out-of-the-box option, and it's plaintext.
  - `manager` (Git Credential Manager, GCM) — cross-platform, backs onto Windows Credential Manager/macOS Keychain/Linux Secret Service; what GitHub Desktop and GitKraken both defer to rather than implementing their own storage.
- **Cost on an unconfigured machine (the common case here)**: git ships **no default persistent helper** on Linux; GCM is **not in official Arch/CachyOS repos** — must be pulled from AUR or `dotnet tool install -g git-credential-manager`. A real, non-trivial install step for a "non-git-native user" (this project's stated audience). On Windows, Git for Windows ships GCM built-in since Git 2.39+, much smoother there than on Linux.
- **`ssh-agent`**: on Linux, if no agent running/key not loaded, OpenSSH prompts for passphrase on every use — bad recurring-prompt UX for background sync. On Windows, the built-in OpenSSH Authentication Agent service persists added keys **in the registry**, DPAPI-encrypted, survives reboots — a different (arguably worse from "keys shouldn't outlive intent," though more convenient) security model than Linux's in-memory-only agent.
- **Android**: **no system-level git-credential-helper or ssh-agent infrastructure by default** — dead end for the "Android-possible, not Android-now" requirement; future Android must fall back to a Keystore-backed in-app store (§1).
- **(a) Attacker with home-dir read**: if `credential.helper=store` ends up configured (git's simplest durable default), attacker gets plaintext from `~/.git-credentials` — no different from Cerebrite storing plaintext itself, except now it's git's blast radius. If GCM is configured, inherits §1's OS-keychain trade-offs.
- **(b) Failure mode**: on an unconfigured machine, "delegate to git" effectively means "prompt via terminal/GIT_ASKPASS or fail" — bad experience for a GUI note-taking app, pushes real setup burden onto exactly the non-git-native users this project targets.

---

## 4. Plaintext with informed consent

- What comparable open-source desktop apps do: the Obsidian Git plugin (a close functional analog — git-sync for a notes app) stores credentials in plaintext in its settings, an accepted, honestly-labeled position for a single-user local-first tool.
- File-permission-protected plaintext (`chmod 600`-equivalent) in the app config dir is a defensible **fallback** position — not a default — for a single-user local-first tool, provided the UI is explicit about what protection it does and does not offer (no encryption, protected only by OS file permissions and whatever full-disk protection the user already has).
- **(a) Attacker with home-dir read access**: gets the secret immediately, in full — the honest, stated cost of this option.
- **(b) Failure mode**: none — it never fails to store or retrieve, which is exactly its appeal as a fallback beneath a keychain that might not be present.

---

## Recommendation input for ticket 07 (not decided here)

Layered approach worth considering downstream: OS-native keychain as the default (via `keyring-core` + explicit per-platform store crates, not the monolithic `keyring` crate or an unproven third-party Tauri plugin), with an explicit, user-visible fallback to permission-protected plaintext when the keychain is unavailable (e.g. headless/minimal-WM Linux) — never a silent, unlabeled degrade. Do not build an app-managed encrypted file without either an OS-keychain-held key or a genuine new user passphrase; anything else is theater. Delegating entirely to git's own helpers is not a good primary mechanism for this project's stated non-git-native audience, though it remains available to power users who already have it configured.
