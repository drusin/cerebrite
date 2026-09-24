# Known gaps

A running list of things the MVP implementation (issues 02–16, see
[`.scratch/cerebrite-mvp-implementation/issues/`](../.scratch/cerebrite-mvp-implementation/issues/))
left undone, simplified, or unverified — for a human to pick up next. Compiled
from the implementing agents' own reports and a `/code-review` pass across
the full diff. Not urgent bugs unless marked **bug**.

## Missing features

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
  them (checked `src/main.ts` again after the Settings UI commit — neither
  symbol appears there). A user still has no visual indication of whether
  sync is running, succeeded, or needs attention, even though the new
  Settings modal (`src/main.ts`, commit `0155130`) would have been a natural
  place to surface it.

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
  log (`redirects.tsv`) surviving a real sync conflict — `src-tauri/src/sync.rs`
  still has no test that touches `redirects.tsv`, only ordinary page files.
  Worth either writing the real test or correcting the comment.

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

- **(done)** `src-tauri/src/lib.rs` has since been split by feature area —
  it now declares `mod frontmatter; mod heading_slug; mod index; mod links;
  mod markdown; mod redirects; mod search; mod settings; mod sync; mod
  trash; mod vault;` and each lives in its own file (`sync.rs`, `search.rs`,
  `trash.rs`, `index.rs`, `redirects.rs`, `links.rs`, `frontmatter.rs`,
  `settings.rs`, `vault.rs`, `markdown.rs`, `heading_slug.rs`). `lib.rs`
  itself is down to ~1445 lines of `AppState`/Tauri-command glue, which is
  what's left after the split — not a monolith anymore.
- **(done)** `get_backlinks_impl` (`src-tauri/src/lib.rs`) now calls the
  shared `redirects::resolve_heading_slug` helper directly instead of
  re-deriving redirect-chain resolution inline.
- `src/main.ts` (~1010 lines) is still a single-file dumping ground — the
  sidebar/search-modal/trash-view/settings-modal logic added by tickets
  12–16 and the later Settings UI commit all still live there, unlike the
  frontend concerns tickets 06–08 did extract into their own modules
  (`vault-api.ts`, `page-editor.ts`, `wiki-link-plugin.ts`,
  `heading-slug.ts`). Worth pulling those out now that `lib.rs` has shown
  the pattern works.
- `lib.rs` still has the same "scan `pages`, normalize each title, compare"
  shape written three separate times (`resolve_page_impl`, the trashed-page
  loop inside it, and `find_page_by_normalized_title`) instead of
  consistently reusing one helper.

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

# Git provider integration (tickets 01–14)

Implemented by 14 separate commits (`750692f`..`ae06351`, one per ticket in
[`.scratch/git-provider-integration-implementation/issues/`](../.scratch/git-provider-integration-implementation/issues/)),
plus a follow-up fix commit (`e30286b`) addressing the highest-severity
findings from a `/code-review` pass across the full diff. The rest of that
review's findings — below — are left for a human to pick up.

## Blocking manual follow-up (must happen before this ships)

- **GitHub App not registered.** `src-tauri/src/github_oauth.rs` has
  placeholder `GITHUB_CLIENT_ID`/`GITHUB_APP_SLUG` constants
  (`"TODO_REGISTER_GITHUB_APP"`). A human needs to register Cerebrite as a
  GitHub App (Device Flow enabled, "Contents: Read and write" scope) and
  swap in the real values. Until then, "Sign in with GitHub" cannot work.
- **GitLab application not registered, and its live spike (ticket 07) was
  never run.** `src-tauri/src/gitlab_oauth.rs` has a placeholder
  `GITLAB_CLIENT_ID`. Three facts ticket 07 flagged as needing a live test
  against gitlab.com are still unverified: whether `write_repository` scope
  alone is enough to `git push`, whether the device grant actually returns a
  refresh token, and whether refresh works with no client secret. The code
  handles both possible outcomes defensively (see
  `gitlab_oauth::OauthSecret.refresh_token: Option<String>` and
  `SyncFailureCause::OauthReconnectRequired`), but which branch is real is
  unconfirmed.
- **No real manual QA.** Nothing in the sandbox that implemented this could
  exercise the actual running Tauri app — no wizard, popup, or OS
  keychain/Secret-Service prompt was ever clicked through by a human. Ticket
  05 (SSH) is the one exception with real end-to-end coverage, via an
  in-sandbox `sshd` fixture. Also untested against real infrastructure: the
  access-token path (ticket 04) against a real non-GitHub/GitLab host, and
  the OS keychain backends (Windows Credential Manager, Linux Secret
  Service) — this sandbox has no Secret Service daemon, so ticket 02's
  keyring integration was only exercised against an in-memory fake store.

## Bugs / correctness gaps

- **Ticket 10**: the guided clone wizard's "remote with an unrelated `vault/`
  history is refused" case (from ticket 01's four-state classification) is
  unimplemented — `classify_cloned_repo` only refuses when `vault` exists
  but isn't a directory; any actual `vault/` directory is accepted
  regardless of whether it's foreign history. Documented as a deliberate,
  narrower stand-in in the ticket-10 commit, not a full implementation of
  the checklist item.
- **Ticket 13**: the merge-conflict CTA's "resolve via ordinary git tooling"
  guidance exists only as a code comment — it's never actually surfaced to
  the user in the needs-attention popup text.
- **Ticket 06**: the GitHub device-code request sends `scope=repo`, which is
  an OAuth-App-style scope string; this is a GitHub App using Device Flow,
  which is permissioned via the app's own installation permissions
  ("Contents: Read and write"), not OAuth scopes. Worth confirming whether
  this parameter is actually inert for a GitHub App device-code request or
  whether it should be removed.
- **Ticket 06**: `refresh_if_needed`'s in-memory-only fallback branch (what
  happens when a background token refresh succeeds but the keychain write
  fails) has no test — ticket 07's equivalent path does.

## Code-health follow-ups (from `/code-review`)

- **Duplicated OAuth device-flow plumbing.** `src-tauri/src/github_oauth.rs`
  (1065 lines) and `src-tauri/src/gitlab_oauth.rs` (1291 lines) implement
  near-identical device-grant polling, HTTP/RFC3339 plumbing, and
  request/response handling. The provider-specific differences (scopes,
  revoke endpoint, refresh-token optionality) are real, but the shared
  ~2300 lines of protocol machinery is copy-pasted rather than factored into
  one `oauth_device_flow` module parameterized by endpoint/quirks — a
  protocol bugfix currently has to be applied twice.
- **Duplicated wizard-rendering glue in `src/main.ts`.** `connect-wizard.ts`
  and `clone-wizard.ts` correctly keep their *state machines* separate (the
  step ordering genuinely differs — auth happens before the repo is known
  for clone, after for connect). But the *rendering* halves left in
  `main.ts` are near line-for-line duplicates per step (e.g.
  `runOauthSignIn` vs `runCloneOauthSignIn`, `renderProviderChoiceStep` vs
  `renderCloneProviderChoiceStep`) that were never given the same
  separate-but-shared treatment.
- **OAuth token globals as a data clump.** `main.ts` has six loose
  module-level globals (`wizardAccessToken`/`wizardRefreshToken`/
  `wizardAccessTokenExpiresAt`, and their `cloneWizard*` twins) that always
  travel and get updated together — a good candidate for one
  `OauthTokenState` type reused by both wizards.
- **`main.ts` divergent change.** Grew from ~1100 to ~3800 lines across
  these tickets, mixing connect-wizard rendering, clone-wizard rendering,
  the Settings modal, the disconnect flow, and the commit-author UI in one
  file, despite the underlying wizard *logic* already being split into
  `connect-wizard.ts`/`clone-wizard.ts`. The render layer never got the
  same extraction `lib.rs` got during the MVP (see the code-health section
  above).
