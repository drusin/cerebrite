Type: grilling
Status: resolved
Blocked by: 01, 07

## Question

How is search presented to the user? Quick-switcher-style modal vs. dedicated search page/pane, how are SQLite FTS5 results ranked and grouped (by page, by matching block/heading, recency), what result metadata is shown (snippet/highlight, backlinks count), and how are results filtered (by tag if [tag system](09-tag-system.md) exists, by date, by page vs. block match)?

Note: [Page creation/deletion UX](07-page-creation-deletion-ux.md) settled that dynamic pages (no file yet — ADR-0009) surface in search the same as persisted pages once the index is built, and that a page sitting in trash still resolves (in an "in trash" state) rather than disappearing outright — this ticket should account for both when defining what a search result actually represents.

## Answer

Search is one surface, not two, following on from [ticket 08](08-navigation-sidebar-ui-structure.md)'s "one search surface, not two." A subagent research pass (Logseq/Obsidian/Notion/VS Code/Sublime/Spotlight) confirmed blending is the right call at this scale: tools that split quick-switch from content search (VS Code, Sublime) do so because content hits are line-level and need a persistent panel with replace — not applicable to Cerebrite's ~500-file corpus. Obsidian's split (Quick Switcher vs. Search-in-files) is a widely-complained-about ergonomic gap, with the top switcher plugins existing mainly to re-merge the two.

**Presentation**: a single modal overlay (quick-switcher style), generously sized to host a scrollable results list — not a dedicated pane/page. Opened via a prominent, always-visible sidebar button *and* Ctrl/Cmd+K (per [ticket 08](08-navigation-sidebar-ui-structure.md)) — the shortcut is not the only entry point.

**What's searchable**: page title, frontmatter/inline tags ([tag system](09-tag-system.md)), and full body text via SQLite FTS5 — all three in one query, not separate modes.

**Ranking — three strict tiers**, each internally sorted by its own relevance, no cross-tier blending:
1. Exact/prefix title match
2. Tag match (inline `#tag` or frontmatter `tags:`)
3. BM25-ranked body-text match

No recency blending — Recent already has its own dedicated sidebar surface ([ticket 08](08-navigation-sidebar-ui-structure.md)).

**Result shape**: one row per page. A tier-3 hit shows a snippet from the single best-matching heading/sub-heading section, deep-linking via its [ADR-0005](../../../docs/adr/0005-page-per-file-with-frontmatter-id-and-slug-addressed-headings.md) slug; a strong secondary match in a different section shows as a small secondary snippet in the *same* row, never a second row. A tier-2 hit shows the matched-tag chip. No backlink count or last-modified date on the row — that's page-detail information, not scan-a-results-list information, and no other surface in the app currently supports sorting/scanning by recency or connectedness.

**Trash**: excluded from default results (consistent with trash being "off to the side by design," manual-purge-only, per [ADR-0010](../../../docs/adr/0010-page-deletion-via-git-tracked-trash-folder.md)); an explicit checkbox toggle includes trashed pages, still resolving to their "in trash" state per [ticket 07](07-page-creation-deletion-ux.md).

**No other filters at MVP**: no tag-scope filter (redundant with tier-2 ranking — typing the tag already surfaces those pages at the top) and no date-range filter (no existing sort-by-date affordance anywhere else in the app to hang it on — Recent is opened-order, not edited-order). Both are straightforward post-MVP additions if real usage shows a gap.

**Empty state**: when there's no exact title match, the last row offers "Create page: '‹query›'", wired to the existing explicit-new-page action from [ticket 07](07-page-creation-deletion-ux.md) (persists immediately on the typed title) — matches the equivalent affordance in Notion, Logseq, Obsidian, and Roam.

Checked against the three ADR criteria via the `domain-modeling` skill: no CONTEXT.md change and no new ADR. Nothing here touches data model, storage, or file format (FTS5 itself was already decided in ADR-0008) — it's pure UI/ranking behavior, same category as [ticket 08](08-navigation-sidebar-ui-structure.md)'s "no ADR needed" precedent. No new or changed glossary terms.
