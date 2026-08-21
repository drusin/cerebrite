Type: research
Status: open

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

This surfaces a recommendation to inform the decision, not a resolution — status stays `open` pending review, since this choice is foundational and hard to reverse.
