# Secret storage options at rest

Type: research
Status: open

## Question

Where can a Tauri 2 app actually put a secret on Windows, Linux, and Android — and how well does each option hold up?

Whatever mechanism wins, Cerebrite ends up holding something secret: an OAuth token, a personal access token, an SSH private key, or a key passphrase. Today `settings.json` is plaintext in the app config dir and `Cargo.toml` carries no keychain dependency at all, so this is greenfield.

Survey the options and report the real trade-offs, not the marketing:

- **OS-native keychains** via the `keyring` crate or a Tauri plugin: Windows Credential Manager, Linux Secret Service/libsecret, Android Keystore. For Linux specifically, establish what happens on a headless box, a minimal window manager, or a distro with no secret-service daemon — this is the known weak point and the project targets Arch/CachyOS. Check the crate's maintenance status and Android support against the no-EOL-dependency constraint.
- **App-managed encrypted file**: what it would be encrypted *with*, given there is no user password anywhere in Cerebrite today. Be blunt about whether this reduces to obfuscation.
- **Delegating entirely**: leaning on git's credential helpers and ssh-agent so Cerebrite never holds a secret itself. Establish what this costs on a machine with neither configured, and whether it is reachable at all on Android.
- **Plaintext with informed consent**: what comparable open-source desktop apps actually do, and whether file-permission-protected plaintext in the app config dir is a defensible position for a single-user local-first tool. Include this honestly rather than dismissing it — it may be the right answer, and pretending otherwise produces security theatre.

For each option, state what an attacker with read access to the user's home directory gets, and what the user experiences when the mechanism fails or is unavailable. Failure modes matter more than the happy path here.
