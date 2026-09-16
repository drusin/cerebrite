Type: grilling
Status: resolved
Blocked by: 07

## Question

What's the top-level navigation / sidebar structure of the Cerebrite UI? What lists or trees does it show (all pages, recent pages, favorites/pinned, tags if any), how is it organized (flat list, folders, namespaces), and how does it need to adapt across the three platform targets (Windows, Linux, Android — e.g. collapsible on desktop vs. drawer on mobile)?

Note: [Page creation/deletion UX](07-page-creation-deletion-ux.md) settled that dynamic pages (link/tag to nonexistent text, no file yet — ADR-0009) are included in search/navigation the same as persisted pages once the index is built. Any "all pages" list here needs to account for that (e.g. does it distinguish dynamic from persisted pages visually, or are they truly indistinguishable?).

## Answer

Sidebar sections, top to bottom:

1. **Search** — a prominent, always-visible entry point (button + Ctrl/Cmd+K shortcut) that opens the full search UI [ticket 11](11-search-ui-ux.md) specs. Page-title fuzzy-matching is just the first-keystroke behavior of that one search surface, not a separate inline filter box — one search surface, not two.
2. **Recent** — last-*opened* order (not last-edited: reflects the reader's own navigation trail), capped at 10 entries, no "show more."
3. **All pages** — a flat alphabetical list of page titles. No folders or namespace grouping: there's no directory structure backing pages (links resolve via frontmatter id, not path/filename), and page titles stay plain, opaque strings — no special parsing of e.g. `/` in titles.
4. **Daily-notes entry point** — slot reserved pending [ticket 10](10-daily-notes.md)'s outcome.

Dynamic and persisted pages are visually **indistinguishable** in the All-pages list — no icon/badge/dimming marker — consistent with ADR-0009/CONTEXT.md's "renders identically" principle.

Favorites/Pinned is **cut from MVP scope**: it adds a curated-state UI surface (and a new piece of persisted state) for marginal benefit in what is a single-user tool, where Recent + Search already cover fast access.

Platform adaptation:
- **Desktop (Windows/Linux)**: persistent, user-collapsible sidebar (standard desktop-app pattern).
- **Android**: drawer, hidden by default, hamburger-triggered — not a persistent sidebar, to preserve editor screen space on phone-sized screens.

No CONTEXT.md changes and no new ADR: checked against the three ADR criteria (hard to reverse / surprising / real trade-off) via the `domain-modeling` skill — the "no namespace grouping" call is a UI choice, not a data-model one (titles stay plain strings regardless; folder-style grouping could be added later purely additively, no migration), so it fails "hard to reverse." Purely a UI-structure decision, captured as this ticket's resolution only.
