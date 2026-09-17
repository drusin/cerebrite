# Cerebrite

A git-native, markdown-based knowledge base ("second brain"): notes live as
plain markdown files, synced via git, with a graph of links and backlinks
derived from those files on demand. See [CONTEXT.md](CONTEXT.md) for the full
domain vocabulary (page, dynamic page, tag, backlink, ...) and
[docs/adr/](docs/adr/) for the architectural decisions behind it.

Point the app at a folder (an existing git repo, or a fresh one it `git
init`s for you) and it becomes your vault: every `.md` file is a page,
`[[links]]` and `#tags` connect them, and everything is plain, clean markdown
that stays readable and diffable outside the app.

Built with [Tauri 2.x](https://v2.tauri.app/): one shared Rust core (vault
indexing, SQLite/FTS5 derived index, git operations) and one TypeScript
webview frontend (a [Milkdown](https://milkdown.dev) WYSIWYG editor and the
rest of the UI). See [ADR-0008](docs/adr/0008-tauri-rust-core-with-sqlite-fts5.md)
for why.

**Status**: MVP feature-complete (see
[.scratch/cerebrite-mvp-implementation/issues/](.scratch/cerebrite-mvp-implementation/issues/)
for the ticket-by-ticket implementation record). Known gaps and unverified
areas are tracked in [docs/known-gaps.md](docs/known-gaps.md) — read that
before relying on this for real notes, particularly around Android support
and git remote sync.

## Prerequisites

- **Node.js** 22+ and npm
- **Rust** (stable toolchain — install via [rustup](https://rustup.rs/))
- Platform-specific Tauri build dependencies:
  - **Linux**: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`,
    `libayatana-appindicator3-dev`, `librsvg2-dev` (e.g. on Debian/Ubuntu:
    `sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`)
  - **Windows**: [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
    and WebView2 (preinstalled on current Windows 10/11)
  - **macOS**: Xcode Command Line Tools (untested on this platform so far —
    see [docs/known-gaps.md](docs/known-gaps.md))
- For Android: see [docs/install.md](docs/install.md#android-sideload-apk)
  and [ADR-0008](docs/adr/0008-tauri-rust-core-with-sqlite-fts5.md) — this
  needs the Android SDK/NDK and additional Rust targets, and is
  considerably more involved than the desktop targets.

See the full list of runtime/build dependencies in
[`package.json`](package.json) and [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml).

## Getting started

```sh
npm install
npm run tauri dev
```

This launches the app in development mode with hot-reload for the frontend.
On first run, you'll be prompted to pick a vault folder (it remembers your
choice for next time).

## Building

```sh
npm install
npm run tauri build
```

Produces a release build and, per platform, a native installer/package under
`src-tauri/target/release/bundle/` (an NSIS installer on Windows, an
AppImage/deb/rpm on Linux, an app bundle on macOS). To build just the one
sideload format described in [docs/install.md](docs/install.md):

```sh
npm run tauri build -- --bundles nsis      # Windows
npm run tauri build -- --bundles appimage  # Linux
```

Android builds go through `tauri android build` instead — see
[docs/install.md](docs/install.md#android-sideload-apk) for the extra setup
and signing steps that requires.

## Development workflow

- `npm run dev` — frontend only (Vite dev server), useful for quick
  frontend-only iteration without the Tauri shell.
- `npm run build` — typechecks (`tsc`) then builds the frontend bundle.
- `cd src-tauri && cargo test` — runs the Rust core's unit tests (the bulk of
  this project's test coverage — indexing, linking, tags, backlinks,
  redirects, trash, search, and sync all have Rust unit tests using tempdir
  and local bare-repo git fixtures).
- `cd src-tauri && cargo build` — compiles the Rust core alone.
- `node node_modules/typescript/bin/tsc --noEmit` — typecheck the frontend
  without emitting output.

CI (`.github/workflows/ci.yml`) runs Rust tests and builds both the Windows
and Linux sideload artifacts on every push to `main` and on every pull
request.

## Project structure

```
src/                 TypeScript webview frontend
  main.ts               app entry point, sidebar/search/trash UI wiring
  page-editor.ts         Milkdown WYSIWYG editor integration
  wiki-link-plugin.ts    [[links]] / #tags chip rendering in the editor
  vault-api.ts            typed wrappers around Tauri commands
src-tauri/           Rust core
  src/lib.rs             Tauri commands (the Rust<->JS boundary)
  src/index.rs           SQLite/FTS5 derived index (pages, backlinks, search)
  src/frontmatter.rs      YAML frontmatter parsing, slugs, title normalization
  src/links.rs            [[link]] / #tag / heading-fragment extraction
  src/redirects.rs        heading-rename redirect log (.cerebrite/redirects.tsv)
  src/trash.rs            page deletion via .cerebrite/trash/
  src/search.rs           tiered search (title -> tag -> FTS5 body)
  src/sync.rs             automatic background git remote sync
  src/vault.rs            vault open/init, local git auto-commit
docs/adr/            Architectural decision records
docs/agents/          Conventions for AI coding agents working in this repo
.scratch/             Issue tracker (specs and implementation tickets)
```

## Coding standards and conventions

Domain vocabulary is defined once in [CONTEXT.md](CONTEXT.md) — code,
comments, and commit messages should use those terms consistently (e.g.
"dynamic page", not "stub" or "placeholder"). Architectural decisions live in
[docs/adr/](docs/adr/); when in doubt about *why* something is built a
certain way (page-per-file with frontmatter ids, backlinks-only graph, clean
markdown, etc.), check there first.

## Testing approach

The Rust core carries the bulk of the test suite: unit tests for parsing,
indexing, and linking logic, plus integration-style tests using `tempfile`
for filesystem/git fixtures (including local bare-repo "remotes" for sync
tests — no network access is exercised in tests). Run `cargo test` from
`src-tauri/` before committing any Rust change. The frontend is currently
typechecked (`tsc --noEmit`) but has no automated test suite of its own; UI
changes should be reasoned through carefully and, where possible, exercised
manually with `npm run tauri dev`.
