# 02: Credential storage infrastructure

**What to build:** Per [ADR-0013](../../../docs/adr/0013-credentials-live-in-the-os-keychain-with-consented-plaintext-as-the-only-fallback.md), a place for Cerebrite to actually put a secret, with an honest fallback when that place doesn't exist. This ticket builds the storage layer only — no credential kind is wired to it yet (that's tickets 04–07); verify it end-to-end with a synthetic secret in tests and a manual smoke test.

From the user's perspective: connecting a vault to a provider (once later tickets wire it up) stores its secret in the OS keychain by default; on a machine with no keychain service, the user sees a consent dialog explaining the secret will be stored as a `0600` plaintext file instead, with an offer to set up a keychain and a "check again" button. Nothing is ever silently downgraded.

**Blocked by:** None (can start immediately)

- [ ] `keyring-core` + platform store crates integrated (Windows Credential Manager, Linux Secret Service); calls run off the UI thread with a timeout (~2 min interactive, fail-fast in background)
- [ ] One keychain entry per connection, holding all of that connection's secrets serialized together
- [ ] Plaintext fallback: `0600` file per connection ID in the app config dir, offered only after a keychain probe has actually failed, with per-connection recorded consent
- [ ] The security-claim text is exact and truthful for each store (no "encrypted"/"secure" language for the plaintext path)
- [ ] Non-secret connection record written to `.git/cerebrite/connection.json` (connection ID, credential kind, HTTPS username, provider, token expiry, which store is in use) — never committed
- [ ] `settings.json` gains an index of connection IDs and which store each uses
- [ ] App-wide Cerebrite-owned `known_hosts` file in the config dir (not `~/.ssh/known_hosts`)
- [ ] A background (sync-time) keychain call that finds the keychain locked fails fast and distinguishably from "keychain unreachable" (different underlying states, even though ticket 13 collapses their UI)
- [ ] Windows: an entry larger than the ~2560-byte Credential Manager blob limit is rejected with a clear error, not silently truncated
- [ ] "Move to keychain" and "Remove all stored Cerebrite credentials" are callable (UI wiring for these can wait for ticket 13/14, but the underlying functions exist and are tested)
