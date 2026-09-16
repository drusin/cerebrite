# 01: Project scaffold & walking skeleton

**What to build:** A Tauri 2.x application — one shared Rust core, one TypeScript webview frontend — that launches a window on Windows and Linux and proves the Rust↔JS command boundary works end to end (a trivial round-trip command, e.g. ping/pong). CI builds both targets on every push.

Per [ADR-0008](../../../docs/adr/0008-tauri-rust-core-with-sqlite-fts5.md): this is the shared-core architecture every later ticket builds on. No feature logic yet — just the skeleton and the build pipeline.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `tauri android build` config exists (even if not yet wired into CI) so ticket 15 isn't starting from zero
- [ ] Windows and Linux CI jobs both build a runnable artifact
- [ ] App launches to a visible window on both platforms
- [ ] A Rust command is invocable from the TS frontend and its response renders in the UI
- [ ] No feature/domain code (page parsing, editor, git) — skeleton only
