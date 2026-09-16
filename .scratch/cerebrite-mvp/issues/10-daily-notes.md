Type: grilling
Status: resolved

## Question

Are daily notes a first-class concept in Cerebrite at MVP (a page auto-created/keyed by date, a dedicated entry point in the UI), or are they just an ordinary page a user can create by naming it a date? If first-class, what's the file naming/frontmatter convention, and how does "today's note" get surfaced in navigation?

## Answer

Daily notes are first-class but lightweight — see [Daily note](../../../CONTEXT.md) glossary entry.

**Recognition**: purely a title-pattern convention — a page whose title exactly matches ISO-8601 `YYYY-MM-DD` (e.g. `2026-09-16`) is a daily note. No frontmatter flag, no new page type; it's an ordinary page under [ADR-0009](../../../docs/adr/0009-dynamic-pages-materialize-on-first-write.md) distinguished only by what its title looks like. A hand-typed date-like title in any other format (e.g. "Sep 16, 2026") gets zero special treatment.

**Creation/persistence**: clicking the sidebar's "Today" shortcut navigates to today's date-titled page as a normal *dynamic page* — no file until first write, same lifecycle as any other page. Nothing eagerly persists an empty file for days the user never wrote in.

**Sidebar surfacing** (fills the slot reserved in [Navigation/sidebar UI structure](08-navigation-sidebar-ui-structure.md)): a single "Today" shortcut only. No calendar, no chronological list of past daily notes, no prev/next-day navigation — all cut from MVP. Past daily notes are just ordinary pages, already reachable via Recent, All pages, and Search; a second date-scoped browsing surface would duplicate that coverage for a single-user tool.

No ADR: not hard to reverse (a stored frontmatter flag could be added later with no migration, since existing date-titled pages already satisfy the pattern), so it fails the ADR criteria despite being a genuine trade-off (title-convention vs. frontmatter flag vs. dedicated type) — checked via the `domain-modeling` skill. CONTEXT.md gets a **Daily note** glossary entry (unlike ticket 08's UI-only call) because "daily note" is a term future tickets/code will need to reference.
