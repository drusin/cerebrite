Label: wayfinder:map

## Destination

A written MVP spec for Cerebrite — a git-native, markdown-based, Logseq-inspired knowledge base — covering scope, the data/linking model, the git-sync model, the clean-markdown requirements, architecture/tech-stack decisions, and platform targets (Windows, Linux, Android). Ready to hand to implementation planning; prod-ready iteration path, not a throwaway proof of concept.

## Notes

- Domain vocabulary and settled architectural decisions live in [CONTEXT.md](../../CONTEXT.md) and [docs/adr/](../../docs/adr/) — read both before resolving any ticket here.
- Standing constraints for every research/prototype ticket: no dependency may be end-of-life, deprecated, or abandoned; rebuilding the backlink + search derived index from ~500 markdown files must be sub-second on Windows, Linux, and Android.
- Call the `grilling` and `domain-modeling` skills for any ticket that turns out to hinge on a product/domain decision rather than a pure research question.

## Decisions so far

- [Stable heading identity prototype](issues/04-stable-heading-identity-prototype.md): viable for headings via a git-synced redirect-log sidecar file ([ADR-0007](../../docs/adr/0007-persisted-redirect-log-for-heading-rename-identity.md)), not a purely ephemeral mapping. Paragraph-level linking (ADR-0002) stays deferred — this doesn't solve paragraphs' harder, prior problem of having no natural key at all.
- [Tech stack and core architecture](issues/01-tech-stack-and-core-architecture.md): Tauri 2.x (shared Rust core, one webview UI on all three platforms) + SQLite FTS5 + a disk-backed derived index rebuilt on launch/sync with no file watcher ([ADR-0008](../../docs/adr/0008-tauri-rust-core-with-sqlite-fts5.md)). Three unresolved risks (Android production maturity, corpus benchmark, `git2-rs` cross-compile) spun out as [Android platform smoke test](issues/06-android-platform-smoke-test.md), gating Android implementation, not this decision.
- [WYSIWYG editor component](issues/02-wysiwyg-editor-component.md): Milkdown, not Tiptap — Tiptap's markdown serializer (both the deprecated community `tiptap-markdown` package and the official, actively-maintained `@tiptap/markdown`) unconditionally HTML-entity-escapes `<`/`>` and backslash-escapes `[[…]]`-style brackets in plain text, by design and not configurable, violating ADR-0003's clean-markdown-out requirement. Android ships the same WYSIWYG UI as desktop (touch/IME held up in device testing) — no plaintext-only fallback at MVP.
- [Packaging and install](issues/03-packaging-and-install.md): Windows gets an unsigned NSIS installer, Linux (CachyOS/Arch) gets an AppImage (`fuse2` caveat), Android gets a release-signed sideload APK via `tauri android build -- --apk`. Ongoing distribution stays out of scope; Google's 2026-09-30 Android developer-verification rollout flagged for awareness, not blocking.
- [Redirect log format](issues/05-redirect-log-format.md): `.cerebrite/redirects.tsv`, tab-separated, keyed by frontmatter page id, lines inserted in **sorted-by-key order** (not blind append — corrects [ADR-0007](../../docs/adr/0007-persisted-redirect-log-for-heading-rename-identity.md)'s original assumption, verified against real git merges). Pruning piggybacks on the existing derived-index rebuild; a bounded, accepted cross-device pruning race can leave a link broken (never silently wrong).
- [Android platform smoke test](issues/06-android-platform-smoke-test.md): all three checks PASS on a Pixel 9a (Android 17) via the CI-built APK — round-trip works, corpus index rebuild at 169ms (comfortably sub-second), `git2-rs` cross-compiles and runs a commit+clone round-trip on-device. Tech stack decision stands; Android implementation is unblocked.
- [Page creation/deletion UX](issues/07-page-creation-deletion-ux.md): pages are either a *dynamic page* (link/tag to nonexistent text, UI-identical, no file until first write — [ADR-0009](../../docs/adr/0009-dynamic-pages-materialize-on-first-write.md)) or a *persisted page* (real file); explicit "new page" persists immediately on title alone. Deletion moves a persisted page to a git-tracked `.cerebrite/trash/` with manual-only purge and in-app restore ([ADR-0010](../../docs/adr/0010-page-deletion-via-git-tracked-trash-folder.md)).
- [Navigation/sidebar UI structure](issues/08-navigation-sidebar-ui-structure.md): sidebar is Search (prominent Ctrl/Cmd+K entry point into [search UI](issues/11-search-ui-ux.md), no separate inline filter) → Recent (last-opened, capped 10) → All pages (flat alphabetical, no folders/namespaces, dynamic and persisted pages indistinguishable) → daily-notes slot pending [ticket 10](issues/10-daily-notes.md). Favorites/Pinned cut from MVP. Desktop: persistent collapsible sidebar; Android: hamburger-triggered drawer.

## Not yet specified

(empty — all fog graduated into tickets: [07](issues/07-page-creation-deletion-ux.md), [08](issues/08-navigation-sidebar-ui-structure.md), [09](issues/09-tag-system.md), [10](issues/10-daily-notes.md), [11](issues/11-search-ui-ux.md), [12](issues/12-backlinks-display.md))

## Out of scope

- Real-time multi-device collaboration
- Custom conflict-resolution UX beyond plain git conflicts (ADR-0006 covers the detection + copy-safety net that *is* in scope)
- Plugin/extensibility system
- Rich media beyond markdown-native embeds
- Visual graph view, structural graph queries, MCP server (post-MVP goals)
- Distribution (store listings, update infrastructure) — packaging/install itself is in scope, ongoing distribution is not
