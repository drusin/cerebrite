# 09: Tag system

**What to build:** `#tagname` and `#[[multi word tag]]` inline syntax, and a `tags: [foo, bar]` YAML frontmatter list on persisted pages, per [09-tag-system](../../cerebrite-mvp/issues/09-tag-system.md). Both resolve through the exact same dynamic/persisted page mechanism as `[[link]]` (ticket 05) — no separate identity, no separate index entry, same backlink as an ordinary link would produce. Same WYSIWYG chip rendering as a link, no distinct visual style. Flat — no hierarchy (`#project/foo` is a page literally titled "project/foo").

**Blocked by:** 07

**Status:** ready-for-agent

- [ ] `#tagname` and `#[[multi word tag]]` parse in the editor and render as the identical chip treatment as `[[link]]`
- [ ] A frontmatter `tags:` list on a persisted page produces the same backlink entries as inline tags
- [ ] Adding a frontmatter tag to a dynamic page materializes it, same as any other write (ticket 05's rule, not a special case)
- [ ] Tags contribute to the derived index and backlinks identically to links — no separate "is this a tag" flag surfaced anywhere downstream
