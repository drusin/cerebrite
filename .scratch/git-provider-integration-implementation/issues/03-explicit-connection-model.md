# 03: Explicit per-vault Connection model & classified sync failures

**What to build:** Per [ADR-0012](../../../docs/adr/0012-explicit-per-vault-connection-with-three-credential-kinds.md), replace `sync.rs`'s implicit `ssh_key_from_agent → credential_helper → Cred::default()` chain with a Connection that names exactly one credential kind, and give `SyncStatus` enough shape to distinguish a transient failure from one that needs the user's attention.

From the user's perspective: a vault's sync either uses the one credential kind its Connection was set up with, or reports a failure of that specific kind — never a mysterious fallback that sometimes works. A dropped network connection surfaces differently from a rejected credential, even though neither has a real UI yet (that's ticket 12/13).

**Blocked by:** 02

- [ ] A `Connection` type exists (backed by ticket 02's connection record + keychain entry) naming exactly one credential kind; sync resolves credentials only from it
- [ ] `make_callbacks`'s agent/credential-helper/default chain is deleted, not reordered or kept as a further fallback
- [ ] No Connection is persisted until a test fetch using its credential succeeds
- [ ] `git2::Error` from fetch/push is classified: network-class errors → transient/retrying; everything else (auth rejected, non-fast-forward push, etc.) → needs-attention, each retaining enough detail to name a specific cause later
- [ ] `SyncStatus`'s shape carries a specific failure-cause (not just a flat string) sufficient for ticket 12/13 to pick a message and CTA per cause, without redesigning `SyncStatus` again later
- [ ] Existing `sync.rs` tests (fast-forward, local-only-push, conflict-detection) still pass unmodified in behavior, only in how credentials/errors are threaded through
- [ ] A test exercises a rejected/invalid credential and asserts it lands in the needs-attention bucket, and a simulated unreachable remote lands in transient
