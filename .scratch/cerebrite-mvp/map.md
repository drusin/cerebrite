Label: wayfinder:map

## Destination

A written MVP spec for Cerebrite — a git-native, markdown-based, Logseq-inspired knowledge base — covering scope, the data/linking model, the git-sync model, the clean-markdown requirements, architecture/tech-stack decisions, and platform targets (Windows, Linux, Android). Ready to hand to implementation planning; prod-ready iteration path, not a throwaway proof of concept.

## Notes

- Domain vocabulary and settled architectural decisions live in [CONTEXT.md](../../CONTEXT.md) and [docs/adr/](../../docs/adr/) — read both before resolving any ticket here.
- Standing constraints for every research/prototype ticket: no dependency may be end-of-life, deprecated, or abandoned; rebuilding the backlink + search derived index from ~500 markdown files must be sub-second on Windows, Linux, and Android.
- Call the `grilling` and `domain-modeling` skills for any ticket that turns out to hinge on a product/domain decision rather than a pure research question.

## Decisions so far

- [Stable heading identity prototype](issues/04-stable-heading-identity-prototype.md): viable for headings via a git-synced redirect-log sidecar file ([ADR-0007](../../docs/adr/0007-persisted-redirect-log-for-heading-rename-identity.md)), not a purely ephemeral mapping. Paragraph-level linking (ADR-0002) stays deferred — this doesn't solve paragraphs' harder, prior problem of having no natural key at all.

## Not yet specified

- Page creation/deletion UX
- Navigation / sidebar UI structure
- Whether a tag system exists at all
- Whether daily notes are a first-class concept
- Search UI/UX (how results are presented, ranked, filtered)
- How backlinks are displayed to the reader

## Out of scope

- Real-time multi-device collaboration
- Custom conflict-resolution UX beyond plain git conflicts (ADR-0006 covers the detection + copy-safety net that *is* in scope)
- Plugin/extensibility system
- Rich media beyond markdown-native embeds
- Visual graph view, structural graph queries, MCP server (post-MVP goals)
- Android WYSIWYG editing (Android is plaintext-only for MVP)
- Distribution (store listings, update infrastructure) — packaging/install itself is in scope, ongoing distribution is not
