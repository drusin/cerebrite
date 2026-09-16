Type: grilling
Status: open
Blocked by: 07

## Question

Does Cerebrite have a tag system at MVP? If so, what's the syntax in markdown (e.g. `#tag`, `[[tag]]` reuse, frontmatter list), how do tags interact with the existing linking/backlink model, and are they indexed/searchable the same way as page links? If not, is tagging deferred to a later milestone, and what (if anything) substitutes for it at MVP?

Note: [Page creation/deletion UX](07-page-creation-deletion-ux.md) established that tagging a nonexistent page name creates a dynamic page (ADR-0009) — no file, identified by the tag's normalized text, indistinguishable in the UI from a persisted page, and materializing into a real file only on first write to it. This ticket should settle the tag *syntax* itself and confirm it resolves through the same dynamic/persisted page mechanism as an ordinary `[[link]]`.
