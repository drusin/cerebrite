# Secret storage options at rest

Type: research
Status: resolved

## Question

Where can a Tauri 2 app actually put a secret on Windows, Linux, and Android — and how well does each option hold up?

Whatever mechanism wins, Cerebrite ends up holding something secret: an OAuth token, a personal access token, an SSH private key, or a key passphrase. Today `settings.json` is plaintext in the app config dir and `Cargo.toml` carries no keychain dependency at all, so this is greenfield.

Survey the options and report the real trade-offs, not the marketing:

- **OS-native keychains** via the `keyring` crate or a Tauri plugin: Windows Credential Manager, Linux Secret Service/libsecret, Android Keystore. For Linux specifically, establish what happens on a headless box, a minimal window manager, or a distro with no secret-service daemon — this is the known weak point and the project targets Arch/CachyOS. Check the crate's maintenance status and Android support against the no-EOL-dependency constraint.
- **App-managed encrypted file**: what it would be encrypted *with*, given there is no user password anywhere in Cerebrite today. Be blunt about whether this reduces to obfuscation.
- **Delegating entirely**: leaning on git's credential helpers and ssh-agent so Cerebrite never holds a secret itself. Establish what this costs on a machine with neither configured, and whether it is reachable at all on Android.
- **Plaintext with informed consent**: what comparable open-source desktop apps actually do, and whether file-permission-protected plaintext in the app config dir is a defensible position for a single-user local-first tool. Include this honestly rather than dismissing it — it may be the right answer, and pretending otherwise produces security theatre.

For each option, state what an attacker with read access to the user's home directory gets, and what the user experiences when the mechanism fails or is unavailable. Failure modes matter more than the happy path here.

## Answer

**No option is free of trade-offs; a layered approach is indicated for ticket 07 to decide on: OS-native keychain as the default, with an explicit, user-visible fallback (never silent) to permission-protected plaintext when the keychain is unavailable.**

- **OS keychains** (via `keyring-core` + explicit per-platform store crates, not the monolithic `keyring` crate or unproven third-party Tauri plugins): strong and reliable on Windows (Credential Manager/DPAPI, always present); on Linux, strong on GNOME/KDE-full desktops but **absent by default** on headless/minimal-WM setups (i3, sway, fresh Arch/CachyOS without a full DE) — directly matching this project's stated user base, and some Secret Service implementations **hang indefinitely** rather than fail fast when no daemon is present, so any implementation must impose a timeout. Android Keystore is the strongest of the four platforms (non-exportable, hardware-backed) but only relevant once Android work starts; note Jetpack's convenience wrapper (`EncryptedSharedPreferences`) was deprecated in April 2025 — target raw Keystore/`android-native-keyring-store` instead.
- **App-managed encrypted file**: security theater unless the decryption key comes from the OS keychain (redundant with the option above) or a genuine new user passphrase — which would introduce a master-password UX concept Cerebrite doesn't have today. Don't build this without one of those two.
- **Delegating to git's credential helpers/ssh-agent**: removes Cerebrite's own exposure, but git ships no default persistent helper on Linux and Git Credential Manager isn't in the official Arch/CachyOS repos (AUR only) — real setup burden for exactly the non-git-native users this project targets. Dead end on Android (no system-level credential-helper or ssh-agent infrastructure at all).
- **Plaintext with informed consent**: what comparable tools (e.g. the Obsidian Git plugin) already ship; a legitimate, honestly-labeled **fallback**, not default, for a single-user local-first tool — an attacker with home-dir read access gets the secret immediately and in full, which the UI must state plainly rather than imply otherwise.

Full findings, per-option attacker/failure-mode analysis, and sources: [research report](04-secret-storage-options-research.md).
