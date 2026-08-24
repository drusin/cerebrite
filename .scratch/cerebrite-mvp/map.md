Label: wayfinder:map

## Destination

A written MVP spec for Cerebrite — a git-native, markdown-based, Logseq-inspired knowledge base — covering scope, the data/linking model, the git-sync model, the clean-markdown requirements, architecture/tech-stack decisions, and platform targets (Windows, Linux, Android). Ready to hand to implementation planning; prod-ready iteration path, not a throwaway proof of concept.

## Notes

- Domain vocabulary and settled architectural decisions live in [CONTEXT.md](../../CONTEXT.md) and [docs/adr/](../../docs/adr/) — read both before resolving any ticket here.
- Standing constraints for every research/prototype ticket: no dependency may be end-of-life, deprecated, or abandoned; rebuilding the backlink + search derived index from ~500 markdown files must be sub-second on Windows, Linux, and Android.
- Call the `grilling` and `domain-modeling` skills for any ticket that turns out to hinge on a product/domain decision rather than a pure research question.

## Decisions so far

- [Stable heading identity prototype](issues/04-stable-heading-identity-prototype.md): viable for headings via a git-synced redirect-log sidecar file ([ADR-0007](../../docs/adr/0007-persisted-redirect-log-for-heading-rename-identity.md)), not a purely ephemeral mapping. Paragraph-level linking (ADR-0002) stays deferred — this doesn't solve paragraphs' harder, prior problem of having no natural key at all.
- [Tech stack and core architecture](issues/01-tech-stack-and-core-architecture.md): Tauri 2.x (shared Rust core, one webview UI on all three platforms) + SQLite FTS5 + a disk-backed derived index rebuilt on launch/sync with no file watcher ([ADR-0008](../../docs/adr/0008-tauri-rust-core-with-sqlite-fts5.md)). Scope change: WYSIWYG-on-Android is now an open editor-behavior question, not ruled out — see [WYSIWYG editor component](issues/02-wysiwyg-editor-component.md). Three unresolved risks (Android production maturity, corpus benchmark, `git2-rs` cross-compile) spun out as [Android platform smoke test](issues/06-android-platform-smoke-test.md), gating Android implementation, not this decision.

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
- Distribution (store listings, update infrastructure) — packaging/install itself is in scope, ongoing distribution is not
