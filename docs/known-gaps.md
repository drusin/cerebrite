# Known gaps

A running list of things the MVP implementation (issues 02–16, see
[`.scratch/cerebrite-mvp-implementation/issues/`](../.scratch/cerebrite-mvp-implementation/issues/))
left undone, simplified, or unverified — for a human to pick up next. Compiled
from the implementing agents' own reports and a `/code-review` pass across
the full diff. Not urgent bugs unless marked **bug**.

## Missing features

- **Page rename**: there is no way to rename a persisted page's title (and
  re-slugify its filename) from inside the app — ticket 04's checklist asked
  for this and it was never built. A page's title, once created, is
  effectively permanent without hand-editing the file and its filename.
- **In-app git remote configuration**: ticket 14's automatic sync only works
  once a vault's git repo already has a remote configured via external git
  tooling. There is no "add a remote" UI anywhere in the app.
- **Android vault picker**: `tauri-plugin-dialog`'s folder-picker API is
  desktop-only in the version this project uses. There is currently no
  working way to open/choose a vault folder on Android at all — this needs
  Storage Access Framework integration, which is a real platform-specific
  project, not a quick fix.
- **Frontend sync-status indicator**: ticket 14 built `get_sync_status` and a
  `sync-status-changed` event on the Rust side, but no UI reads either of
  them. A user currently has no visual indication of whether sync is
  running, succeeded, or needs attention.

## Bugs / correctness gaps

- **(fixed)** `create_page`'s title-collision check used to compare titles
  exactly while every other lookup in the app normalizes case/whitespace —
  fixed in commit `ce5c6e9`, now covered by a regression test.
- Ticket 12's desktop-vs-Android sidebar layout is driven by a
  `matchMedia("(max-width: 700px)")` viewport-width proxy, not real platform
  detection. Ticket 15 (Android bring-up) was expected to replace this but
  never touched it — a wide Android tablet still gets the desktop sidebar,
  and a narrow desktop window still gets the phone drawer.
- A couple of code comments assert end-to-end scenarios as "verified" that
  were never actually exercised by a test: ticket 08's multi-hop redirect
  chain colliding with a real two-device git merge, and ticket 14's redirect
  log (`redirects.tsv`) surviving a real sync conflict (its conflict tests
  only cover ordinary page files, never `redirects.tsv`). Worth either
  writing the real test or correcting the comment.

## Simplifications worth revisiting

- **Heading-slug format mismatch** (ticket 07): Milkdown's built-in
  duplicate-heading-slug suffix (`-#2`) differs from the Rust
  `HeadingSlugger`'s format (`-2`). Only bites when a page has two headings
  with literally the same text — the in-editor anchor can then disagree with
  the slug the backend computed, so a `[[Page#heading-2]]` link might not
  scroll to the right spot on that one page.
- **Heading-rename detection** (ticket 08) only fires on `save_page`, not as
  part of a full index rebuild — there's no "previous known state" to diff
  against after an app restart, since a rebuild starts from nothing per
  ADR-0008. The pairing heuristic itself is also intentionally simple
  (same-count removed/added headings paired by position, no fuzzy text
  matching) and redirect-chain resolution assumes a rename stays on the same
  page.
- **Frontmatter `tags:` has no editing UI** (ticket 09): only inline
  `#tag`/`#[[multi word tag]]` syntax is reachable from the editor. Frontmatter
  tag parsing/link-extraction is implemented and unit-tested, but a user can
  only populate a page's `tags:` list by hand-editing the file outside the
  app.
- **"Recent" sidebar list** (ticket 12) is in-memory only and resets on every
  app restart — a deliberate simplicity choice, not a bug, but worth
  revisiting if users want it to persist.
- **Rust link extractor doesn't skip code spans/fences** (ticket 06): unlike
  the frontend's mdast-based wikilink plugin, the Rust regex extractor used
  for backlinks/tags will treat `[[looks like a link]]` inside a code block
  as a real link. Accepted as a pragmatic MVP tradeoff.
- **FTS5 tokenization of hyphens** (ticket 13): the default `unicode61`
  tokenizer splits on hyphens, so a hyphenated tag like `project-management`
  tokenizes into two words for tier-3 (body) search purposes. Doesn't affect
  tier-2 tag matching, which is plain Rust string comparison.

## Code-health follow-ups (from `/code-review`)

- `src-tauri/src/lib.rs` (1400+ lines) and `src/main.ts` (~900 lines) grew
  into single-file dumping grounds touched by nearly every ticket — a
  departure from the pattern set by early tickets, which did extract
  cohesive frontend concerns into their own modules (`vault-api.ts`,
  `page-editor.ts`, `wiki-link-plugin.ts`, `heading-slug.ts`). Worth
  splitting `lib.rs` by feature area (pages, search, sync, trash) and pulling
  the sidebar/search-modal/trash-view logic out of `main.ts`.
- `lib.rs` has the same "scan `pages`, normalize each title, compare" shape
  written three separate times (`resolve_page_impl`, the trashed-page loop
  inside it, and `find_page_by_normalized_title`) instead of consistently
  reusing one helper — three different tickets each reinvented it.
- `get_backlinks_impl` re-derives redirect-chain heading-slug resolution
  inline instead of delegating to the existing `resolve_heading_slug`-style
  helper a few lines above it.

## Verification gaps (nothing wrong found, just untested with real hardware)

- **Windows**: the NSIS installer CI produces has never been run on a real
  Windows machine.
- **Linux AppImage**: the bundler itself couldn't run in the sandbox that
  built this MVP (no `/dev/fuse`); it should work on GitHub Actions'
  `ubuntu-latest` runners, but no real CI run has confirmed this yet, and no
  one has tested the resulting artifact on a real Arch/CachyOS box with or
  without `fuse2`.
- **Android**: a real, exit-0 release build was produced and inspected
  (correct signature, real native library packaged), but never installed or
  run on an actual device or emulator — no display/device was available.
  It's signed with a throwaway dev keystore, not a production one; see
  [`docs/install.md`](install.md) for what a human needs to do before real
  distribution.
- General editor touch/IME behavior on Android was reviewed at the code
  level only (tappable, not hover-gated; no keyboard-only paths to core
  actions) — never confirmed on an actual touchscreen.
