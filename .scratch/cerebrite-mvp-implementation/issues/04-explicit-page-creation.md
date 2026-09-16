# 04: Explicit page creation

**What to build:** A "new page" UI action that, given a title, immediately persists a real file: mints a frontmatter id, writes a slug-of-the-title filename, frontmatter-only body (no auto-inserted title heading), and opens it in the editor from ticket 03. Per [07-page-creation-deletion-ux](../../cerebrite-mvp/issues/07-page-creation-deletion-ux.md), this skips the dynamic-page phase entirely — title-only intent from an explicit create action is sufficient.

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] "New page" action is reachable from the UI and prompts for a title
- [ ] Submitting a title immediately creates a file with a minted frontmatter id and a slugified filename
- [ ] Title collision with an existing persisted page is blocked with an in-app error, not auto-suffixed
- [ ] The new page is immediately editable (ticket 03's editor) and appears in "All pages" (ticket 02)
- [ ] Renaming a page's title re-slugifies its filename without breaking anything (links resolve by frontmatter id, not filename)
