Type: research
Status: open
Blocked by: 01

## Question

[04-stable-heading-identity-prototype](04-stable-heading-identity-prototype.md) decided that heading-rename redirects must be persisted in a small, git-synced, plaintext sidecar file (not one of the linkable pages), so they survive an app restart or a sync to another device before the background cleanup job drains them — see [ADR-0007](../../../docs/adr/0007-persisted-redirect-log-for-heading-rename-identity.md).

Given the tech stack chosen in [01-tech-stack-and-core-architecture](01-tech-stack-and-core-architecture.md), design the concrete format for this file:

1. **On-disk format**: one entry per line, keyed by old heading identity → new heading identity — what exactly goes on a line (e.g. `pageId#old-slug\tpageId#new-slug`), and why that's easy for git to merge/rebase line-by-line without spurious conflicts.
2. **Location**: where in the repo it lives (e.g. a dotfile at the repo root, or alongside the derived index's other on-disk state) — it must be obviously not a page, and not swept up by anything that iterates "linkable entities."
3. **Pruning trigger**: exactly when an entry is safe to delete (the cleanup job has rewritten every file that referenced the old key) and how that's detected without a full repo scan on every save.
4. **Conflict case**: what happens if the same heading is renamed differently on two devices before either syncs — confirm this reduces to an ordinary git merge conflict on the log file (per [ADR-0006](../../../docs/adr/0006-automatic-git-sync-with-conflict-detection-and-copy-safety-net.md)) rather than silent data loss, and note anything that needs to change if it doesn't.

Hard constraint: rebuilding the backlink + search derived index from ~500 markdown files (which now includes reading this log) must stay a sub-second operation on Windows, Linux, and Android.
