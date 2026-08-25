Type: research
Status: resolved
Blocked by: 01

## Question

[04-stable-heading-identity-prototype](04-stable-heading-identity-prototype.md) decided that heading-rename redirects must be persisted in a small, git-synced, plaintext sidecar file (not one of the linkable pages), so they survive an app restart or a sync to another device before the background cleanup job drains them — see [ADR-0007](../../../docs/adr/0007-persisted-redirect-log-for-heading-rename-identity.md).

Given the tech stack chosen in [01-tech-stack-and-core-architecture](01-tech-stack-and-core-architecture.md), design the concrete format for this file:

1. **On-disk format**: one entry per line, keyed by old heading identity → new heading identity — what exactly goes on a line (e.g. `pageId#old-slug\tpageId#new-slug`), and why that's easy for git to merge/rebase line-by-line without spurious conflicts.
2. **Location**: where in the repo it lives (e.g. a dotfile at the repo root, or alongside the derived index's other on-disk state) — it must be obviously not a page, and not swept up by anything that iterates "linkable entities."
3. **Pruning trigger**: exactly when an entry is safe to delete (the cleanup job has rewritten every file that referenced the old key) and how that's detected without a full repo scan on every save.
4. **Conflict case**: what happens if the same heading is renamed differently on two devices before either syncs — confirm this reduces to an ordinary git merge conflict on the log file (per [ADR-0006](../../../docs/adr/0006-automatic-git-sync-with-conflict-detection-and-copy-safety-net.md)) rather than silent data loss, and note anything that needs to change if it doesn't.

Hard constraint: rebuilding the backlink + search derived index from ~500 markdown files (which now includes reading this log) must stay a sub-second operation on Windows, Linux, and Android.

## Research

Findings: [.scratch/cerebrite-mvp/research/05-redirect-log-format.md](../research/05-redirect-log-format.md).

## Answer

1. **On-disk format**: `.cerebrite/redirects.tsv`, tab-separated lines of `oldPageId#oldSlug<TAB>newPageId#newSlug<TAB>rfc3339-timestamp<TAB>deviceId`. Keyed by frontmatter page id (ADR-0005), not file path — a path can drift independently of a heading rename, which would add a second, uncoordinated staleness axis the cleanup job was never designed to track. Only the first two fields are load-bearing; timestamp/device-id are advisory, read by nobody's resolve/prune logic, there only to help a human disambiguate a conflict. Tab is a hard delimiter guaranteed absent from either field (page ids and slugs are both alphanumeric/hyphen by every common id scheme), so no escaping logic is ever needed. Lines are append-only and never edited in place — a rename is always a new line, even for a heading renamed twice before cleanup runs; pruning deletes a line whole.

2. **Location**: `.cerebrite/redirects.tsv` at the vault root — a dotfolder (mirroring `.git`/`.obsidian`) with a non-`.md` extension, so any `**/*.md` page-glob structurally cannot match it. A single root file, not per-page, since a redirect can target a different page than it originates from.

3. **Pruning trigger**: an entry is safe to delete once no link anywhere in the corpus resolves through it and it isn't the `fromKey` of any pending cleanup-job rewrite — computed as a byproduct of the derived-index rebuild that already walks all ~500 files on launch/sync (ADR-0008), not a separate scan: a hash-set membership check per link, negligible next to markdown parsing. Pruning is itself a git mutation, batched into the same rebuild cycle's commit. **Accepted residual risk**: device A can prune an entry that device B (not yet synced) still depends on — this produces no textual git conflict, since the two devices touch different files, so it degrades to a detectable *broken* link rather than silent data loss. Mitigation, not a fix: don't prune on the very first rebuild where an entry looks unreferenced — require it to survive at least one full push-and-pull round-trip first. Closing this fully would need cross-device consensus machinery that ADR-0006 deliberately keeps out of MVP scope.

4. **Conflict case, with one correction to ADR-0007's original assumption**: confirmed empirically (real `git merge`, git 2.53.0, default `ort` strategy) that the same heading renamed differently on two devices produces a genuine `CONFLICT (content)`, handled entirely by ADR-0006's existing detect + copy-safety-net machinery — no bespoke conflict logic needed. But blind end-of-file append does **not** reliably give two *unrelated* concurrent renames a clean merge, contrary to ADR-0007's stated assumption: git's line-based merge treats two insertions anchored at the same position as conflicting edits to the same region regardless of content ("changelog conflict"), verified directly. Fix: insert new lines in sorted-by-key order rather than at EOF — verified to make independent renames merge cleanly while the same-key case still conflicts correctly every time. A narrower residual false-positive remains for sort-adjacent or empty-log collisions (e.g. two unrelated renames landing in the same "gap") — always safe, never silent loss, just occasionally a spurious two-line "keep both" resolution. [ADR-0007](../../../docs/adr/0007-persisted-redirect-log-for-heading-rename-identity.md) updated to reflect this correction.

Confidence: high on all git-merge-mechanics claims (empirically reproduced, not assumed). Medium on the cross-device pruning race (reasoned from the architecture, not independently testable without a multi-device fixture). Page-id generation scheme itself remains uncommitted anywhere in the repo — the tab-delimiter choice is robust to any common scheme but should be revisited if a future id format ever permits embedded whitespace.
