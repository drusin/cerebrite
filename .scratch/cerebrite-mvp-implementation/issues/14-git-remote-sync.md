# 14: Git remote sync automation

**What to build:** Automatic background pull/push against the vault's git remote — the user never manually triggers sync (per [ADR-0001](../../../docs/adr/0001-git-native-markdown-as-permanent-source-of-truth.md) and [ADR-0006](../../../docs/adr/0006-automatic-git-sync-with-conflict-detection-and-copy-safety-net.md)). When a file lands in a conflicted state, the app detects it and saves a copy of the user's locally-conflicting version before any automatic sync step could touch it — actual conflict *resolution* stays out of scope, left to ordinary git tooling.

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] Vault can be configured with a git remote; background sync pulls and pushes without user action
- [ ] A completed sync triggers a derived-index rebuild (per [ADR-0008](../../../docs/adr/0008-tauri-rust-core-with-sqlite-fts5.md))
- [ ] A file entering a conflicted state is detected, and a copy of the local version is saved before the automatic sync step can touch it
- [ ] No bespoke conflict-resolution UI is built — conflicts are left as ordinary git conflicts for the user to resolve with normal tooling
- [ ] Verified against the redirect log's sorted-by-key merge behavior (ticket 08): a genuine same-heading-renamed-on-two-devices conflict surfaces as an ordinary git conflict, not silent loss
