# 05: Linking & dynamic pages

**What to build:** `[[Link]]` syntax in the editor, rendered as a clickable chip. Linking or tagging text that doesn't match an existing page opens a UI-identical dynamic page (per [ADR-0009](../../../docs/adr/0009-dynamic-pages-materialize-on-first-write.md)) — no file, no frontmatter id, identified purely by the link's normalized text. The dynamic page materializes into a real persisted file (using the same creation mechanics as ticket 04) the instant the user performs any write on it; merely viewing it does not.

**Blocked by:** 04

**Status:** ready-for-agent

- [ ] Typing `[[Some Page]]` in the editor renders it as a clickable chip
- [ ] Clicking a chip that targets a nonexistent title opens a dynamic page, rendered identically to a persisted page (title, empty body, backlinks panel placeholder)
- [ ] Viewing a dynamic page does not create a file
- [ ] The first write to a dynamic page (body edit, or frontmatter tag write per ticket 09) materializes it: mints a frontmatter id, creates the file, and it behaves as an ordinary persisted page from then on
- [ ] A dynamic page that nothing links to any more simply stops appearing (no explicit delete needed) on the next derived-index rebuild
