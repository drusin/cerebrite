# 02: Vault open + read-only page browsing

**What to build:** The user points the app at a folder — an existing git repo, or a fresh one the app inits — and Cerebrite treats it as their vault. The Rust core walks all `.md` files, parses each page's YAML frontmatter (including its stable id, per [ADR-0005](../../../docs/adr/0005-page-per-file-with-frontmatter-id-and-slug-addressed-headings.md)), and builds the on-disk SQLite derived index (FTS5 + backlinks table, per [ADR-0008](../../../docs/adr/0008-tauri-rust-core-with-sqlite-fts5.md)) from scratch on launch. The UI shows a flat, alphabetical "All pages" list; clicking a page renders its markdown body read-only.

**Blocked by:** 01 (can start immediately once the scaffold exists)

**Status:** ready-for-agent

- [ ] User can select a vault folder on first run (or reopen a previously selected one)
- [ ] A folder that isn't yet a git repo gets `git init`'d (per [ADR-0001](../../../docs/adr/0001-git-native-markdown-as-permanent-source-of-truth.md), markdown is always synced via git)
- [ ] Every `.md` file's frontmatter id and title are parsed into the derived index
- [ ] Derived index rebuild over a ~500-file corpus stays sub-second on Windows and Linux (validated already on Android in [06-android-platform-smoke-test](../../cerebrite-mvp/issues/06-android-platform-smoke-test.md))
- [ ] "All pages" sidebar list shows every persisted page, flat alphabetical order, no folders/namespaces
- [ ] Selecting a page renders its body (read-only) in the main view
