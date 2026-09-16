# 08: Heading-rename redirect log

**What to build:** When a heading is renamed or moved, existing `[[Page#old-slug]]` links keep resolving immediately via `.cerebrite/redirects.tsv` (per [ADR-0007](../../../docs/adr/0007-persisted-redirect-log-for-heading-rename-identity.md) and [05-redirect-log-format](../../cerebrite-mvp/issues/05-redirect-log-format.md)): tab-separated `oldPageId#oldSlug\tnewPageId#newSlug\ttimestamp\tdeviceId` lines, inserted in **sorted-by-key order** (not append), at the vault root. A background cleanup job rewrites every file that referenced the old heading identity to point at the new one directly; an entry is pruned once no link resolves through it and it isn't the source of a pending rewrite, computed as a byproduct of the derived-index rebuild.

**Blocked by:** 07

**Status:** ready-for-agent

- [ ] Renaming a heading writes a sorted-by-key-inserted line to `.cerebrite/redirects.tsv`
- [ ] A link addressing the old heading slug still resolves correctly immediately after the rename
- [ ] A background job rewrites referencing files to the new heading text, and this is verified to collapse a multi-hop rename chain into one direct rewrite
- [ ] Pruning happens as part of the existing derived-index rebuild cycle (launch/sync), not a separate scan, and the prune itself is a git commit
- [ ] `.cerebrite/redirects.tsv` is git-synced but structurally excluded from anything that iterates "linkable entities" (page glob, search index, etc.)
- [ ] Renaming the same heading differently on two (simulated) devices before sync produces an ordinary git merge conflict on the log file, not silent data loss
