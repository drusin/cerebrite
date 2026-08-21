# Cerebrite

A git-native, markdown-based knowledge base ("second brain"): notes live as plain markdown files, synced via git, with a graph of links and backlinks derived from those files on demand.

## Language

**Page**:
A single markdown file, identified by a stable id in its frontmatter so it survives renames and moves. The top-level linkable entity.
_Avoid_: Note, document (both used loosely elsewhere for the same file-on-disk concept; "page" is the canonical term)

**Linkable entity**:
The unit of granularity a link can target. Fixed at page, heading, and sub-heading for the MVP. Paragraph-level linking is not yet decided — see the open question below.
_Avoid_: Block (implies arbitrary/nested granularity broader than what's supported)

**Clean markdown**:
A file whose only metadata lives in its YAML frontmatter — no inline block IDs, no injected HTML comments, no other markup appended into the body to serve the tool. Editors are free to reformat the body on save; "clean" is about what content is permitted in the file, not about minimizing diffs between saves.
_Avoid_: Round-trip-safe, diff-clean (both wrongly imply a diff-minimization guarantee that clean markdown does not make)

**Backlink**:
A reference, surfaced to the reader, from one linkable entity to every other linkable entity that links to it. The only graph feature in the MVP.
_Avoid_: Graph view, graph query (both are post-MVP capabilities built on top of backlinks, not synonyms for it)

**Derived index**:
Any data computed from the markdown files rather than stored as source of truth — the backlink graph and the search index are both derived indexes. Rebuilt from scratch from the markdown files; never git-synced or committed.
_Avoid_: Cache (accurate but undersells that it's fully disposable and reconstructible, not just a performance optimization)

## Open questions

- **Paragraph-level linking**: originally envisioned as the default linkable entity, but paragraphs have no natural stable identity that doesn't violate clean markdown (an inline marker like `^block-id` is exactly the kind of body-level metadata clean markdown disallows) — unlike a heading, a paragraph has no existing text-based key to hang an identity on in the first place. A prototype resolved the analogous problem for headings (see ADR-0007), but that only covers keeping an *existing* key resolvable across a rename; it doesn't supply paragraphs with an initial key. Deferred pending an answer to that prior, harder question.
