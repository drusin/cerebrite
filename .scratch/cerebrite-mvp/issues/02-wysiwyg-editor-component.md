Type: prototype
Status: open
Blocked by: 01

## Question

Given the tech stack chosen in [01-tech-stack-and-core-architecture](01-tech-stack-and-core-architecture.md) (Tauri, single webview UI on Windows, Linux, *and* Android), pick between Milkdown and Tiptap (both ProseMirror-based) as Cerebrite's WYSIWYG editor, and answer the question [01](01-tech-stack-and-core-architecture.md) deliberately left open: does Android ship WYSIWYG at MVP, or fall back to plaintext?

Build a concrete prototype, not just a comparison on paper:

1. **Clean-markdown-out**: embed the candidate(s) and confirm save emits no metadata outside YAML frontmatter — no stray inline markup injected into the body, per [ADR-0003](../../../docs/adr/0003-clean-markdown-excludes-round-trip-fidelity.md) (round-trip fidelity is not required). If neither satisfies this, evaluate a minimal custom editor as a fallback and estimate the effort gap vs. adopting a library.
2. **Android WebView behavior**: run the same webview UI on Android and actually test touch and IME (on-screen keyboard) interaction with the editor — this is flagged by the tech-stack research as an untested soft spot for ProseMirror-family editors in mobile WebViews generally, not confirmed one way or the other for Cerebrite's case. If it's rough (broken selection, IME composition issues, unusable touch targets), that's the basis for falling back to plaintext-only on Android at MVP; if it holds up, Android ships the same WYSIWYG UI as desktop.

The answer settles both the editor pick and the MVP scope question for Android's editing surface — update the map's scope accordingly once resolved.
