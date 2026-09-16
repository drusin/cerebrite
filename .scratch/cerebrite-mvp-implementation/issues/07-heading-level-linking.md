# 07: Heading-level linking

**What to build:** `[[Page#Heading]]` links, addressed by the heading's slug (per [ADR-0005](../../../docs/adr/0005-page-per-file-with-frontmatter-id-and-slug-addressed-headings.md)). Clicking such a link jumps directly to the heading, not just the top of the page. Backlink entries (ticket 06) gain a per-entry target-heading label (e.g. "→ Setup") when the link targets a heading, omitted when it targets the page itself.

**Blocked by:** 06

**Status:** ready-for-agent

- [ ] `[[Page#Heading]]` and `[[Page#Sub-heading]]` syntax parses and resolves via slug
- [ ] Click-through scrolls/jumps to the target heading within the destination page
- [ ] Backlinks section shows the target-heading label per entry when applicable
- [ ] A link to a heading that doesn't (yet) exist behaves like a dynamic-page link at the page level (heading-specific dynamic targets are out of scope — only whole pages materialize per ADR-0009)
