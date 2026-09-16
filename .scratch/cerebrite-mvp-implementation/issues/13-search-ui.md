# 13: Full search UI/UX

**What to build:** A single modal overlay (per [11-search-ui-ux](../../cerebrite-mvp/issues/11-search-ui-ux.md)), opened via the sidebar's Search button and Ctrl/Cmd+K. One query searches title, tags, and body together via SQLite FTS5, ranked in three strict tiers (title match → tag match → BM25 body match), one row per page with a best-section snippet. Trash excluded from default results with an explicit checkbox to include (still rendering the "in trash" state from ticket 10). Empty state offers "Create page: '‹query›'", wired to ticket 04's creation action.

**Blocked by:** 09, 12

**Status:** ready-for-agent

- [ ] Search modal opens from the sidebar button and Ctrl/Cmd+K
- [ ] One query returns results ranked in three strict tiers: title → tag → BM25 body, no cross-tier blending
- [ ] Each result is one row per page with a best-matching-section snippet (tier 3) or matched-tag chip (tier 2)
- [ ] Trashed pages excluded by default; a checkbox toggle includes them, still showing their "in trash" state
- [ ] No results with no exact title match: last row offers "Create page: '‹query›'" wired to ticket 04
