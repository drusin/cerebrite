Type: research
Status: resolved

## Question

What language(s) and framework(s) should Cerebrite use to deliver WYSIWYG desktop apps (Windows, Linux) and a plaintext-editing Android app, from one codebase or a small set of codebases?

Resolve, as one coherent recommendation:

1. **Primary language and UI framework strategy**: a single cross-platform framework (e.g. Flutter, Kotlin Multiplatform, Tauri + web UI, .NET MAUI) vs. a shared core library (e.g. Rust or Kotlin) with separate native UIs per platform. Weigh against the constraint that Windows/Linux need a real WYSIWYG editing surface while Android for MVP only needs plaintext editing.
2. **Full-text search library**: an embedded, no-server, cross-platform search library or approach (e.g. SQLite FTS5, Tantivy) compatible with the chosen stack.
3. **Derived-index storage and rebuild trigger**: whether the backlink + search derived index is cached to disk (e.g. SQLite) or kept purely in-memory, and whether it rebuilds via a file watcher on every change or on-demand/on-launch. Lean toward validating a disk-backed cache rather than pure in-memory.

Hard constraints:
- No dependency (language, framework, or library) may be end-of-life, deprecated, or abandoned.
- Rebuilding the backlink + search derived index from ~500 markdown files must be a sub-second operation on all three platforms (Windows, Linux, Android).

See [CONTEXT.md](../../../CONTEXT.md) and [docs/adr/](../../../docs/adr/) for the settled domain model and constraints this stack must support.

## Research

Findings: [.scratch/cerebrite-mvp/research/01-tech-stack-and-core-architecture.md](../research/01-tech-stack-and-core-architecture.md) (also on the throwaway branch `worktree-agent-a6742d4c2842a48f7`, where the research was originally produced).

This surfaced a recommendation to inform the decision, not a resolution — status stayed `open` pending review, since this choice is foundational and hard to reverse.

## Answer

Locked, as recommended, with one deliberate scope expansion:

1. **UI framework / shared core**: Tauri 2.x — one TypeScript webview frontend, one shared Rust core, run as a webview shell on Windows, Linux, and Android alike. Kotlin Multiplatform + Compose Multiplatform stays the documented fallback; Flutter, .NET MAUI, and Electron ruled out per the research.
2. **Search**: SQLite FTS5 via `rusqlite`, in the shared Rust core. Tantivy noted as the upgrade path if search needs ever outgrow ~500 files.
3. **Derived-index storage/rebuild**: single on-disk SQLite DB (FTS5 + a plain backlinks table), rebuilt on app launch and after every completed git sync — no file-system watcher for MVP.
4. **Editor**: a ProseMirror-family editor (Milkdown or Tiptap) in the shared webview — **scope change**: evaluated across all three platforms, not desktop-only. Because Tauri runs the identical webview UI everywhere, "WYSIWYG on Android" is a question of editor behavior in an Android WebView (touch/IME), not a platform split — so the map's prior "Android is plaintext-only for MVP" assumption no longer holds by default. Whether Android ships WYSIWYG at MVP or falls back to plaintext is decided by the rescoped [02-wysiwyg-editor-component](02-wysiwyg-editor-component.md) (retyped from research to prototype), not fixed here.
5. **Flagged risks not treated as blocking**: Tauri's Android production maturity beyond "it builds," no direct rebuild-time benchmark on real Android hardware, and unverified `git2-rs` Android cross-compilation. Spun out as [06-android-platform-smoke-test](06-android-platform-smoke-test.md), a hard gate before Android-specific implementation work starts, not before this decision.

Captured as [ADR-0008](../../../docs/adr/0008-tauri-rust-core-with-sqlite-fts5.md).
