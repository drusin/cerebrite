Type: grilling
Status: resolved
Blocked by: 07

## Question

Does Cerebrite have a tag system at MVP? If so, what's the syntax in markdown (e.g. `#tag`, `[[tag]]` reuse, frontmatter list), how do tags interact with the existing linking/backlink model, and are they indexed/searchable the same way as page links? If not, is tagging deferred to a later milestone, and what (if anything) substitutes for it at MVP?

Note: [Page creation/deletion UX](07-page-creation-deletion-ux.md) established that tagging a nonexistent page name creates a dynamic page (ADR-0009) — no file, identified by the tag's normalized text, indistinguishable in the UI from a persisted page, and materializing into a real file only on first write to it. This ticket should settle the tag *syntax* itself and confirm it resolves through the same dynamic/persisted page mechanism as an ordinary `[[link]]`.

## Answer

Tags exist at MVP as pure syntactic sugar for a page link — not a separate data model, and no separate identity anywhere downstream (derived index, backlinks, search all treat a tag reference and a `[[link]]` reference to the same page identically).

- **Inline syntax**: `#tagname` (bare word) and `#[[multi word tag]]` (bracketed, for tags containing spaces). Both resolve through the exact same dynamic/persisted page mechanism as `[[link]]` (ADR-0009) — normalized text identifies the target page, first write materializes a dynamic page into a persisted one.
- **Page-level syntax**: frontmatter `tags: [foo, bar]` (YAML list), available only on persisted pages (a dynamic page has no file to hold frontmatter yet). A frontmatter tag produces the same backlink entry as an inline `#tag` on that page — no distinction in the derived index by origin.
- Adding a frontmatter tag to a dynamic page is itself a write, so it materializes the page first, same as writing its body would (ADR-0009's existing rule, not a special case).
- No hierarchy: `#project/foo` is a flat page named "project/foo", not a nested tag under "project" — consistent with [Navigation/sidebar UI structure](08-navigation-sidebar-ui-structure.md)'s flat, folderless "All pages" and the absence of any tag-browser UI at MVP.
- WYSIWYG rendering: `#tag` gets the identical clickable-chip treatment as `[[link]]` in the Milkdown editor ([WYSIWYG editor component](02-wysiwyg-editor-component.md)) — no distinct visual style, since there's no distinct underlying concept.

Recorded in [CONTEXT.md](../../../CONTEXT.md)'s Language section as a new **Tag** glossary entry. No new ADR: this is fully explained by ADR-0009's existing "any write materializes" rule plus a syntax choice, not a new hard-to-reverse trade-off.
