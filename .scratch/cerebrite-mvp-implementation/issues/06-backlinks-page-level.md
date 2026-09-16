# 06: Backlinks (page-level)

**What to build:** A "Backlinks" section always appended at the bottom of a page's rendered content (per [12-backlinks-display](../../cerebrite-mvp/issues/12-backlinks-display.md)), rolling up page-level inbound links, grouped by source page (most-recently-modified source first), with a snippet of surrounding text per entry. This ticket covers page-level links only — heading-level targets and their per-entry labels are ticket 07.

**Blocked by:** 05

**Status:** ready-for-agent

- [ ] Every page renders a "Backlinks" section at the bottom, even with zero backlinks ("No backlinks yet" empty state)
- [ ] The derived index tracks, for every link, its source page and target page
- [ ] Backlink entries are grouped by source page, groups ordered by source page's most-recent-modification
- [ ] Each entry shows a snippet of text around the link
- [ ] Clicking an entry navigates to the source page
