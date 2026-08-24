# Tauri + Rust core, with SQLite FTS5 for the derived index

Cerebrite ships on Windows, Linux, and Android from one stack rather than a stack per platform: [Tauri](https://v2.tauri.app) 2.x, with a single TypeScript webview frontend and a shared Rust core, run as a thin webview shell on all three platforms. Cerebrite's hard problems — parsing markdown/frontmatter, walking the page tree, maintaining the backlink [derived index](../../CONTEXT.md), driving git — are Rust-shaped, and a single webview UI avoids maintaining two UI toolkits for a text-heavy app with no near-term native-look-and-feel requirement.

Because the same webview UI runs everywhere, WYSIWYG is not inherently desktop-only: whether Android ships WYSIWYG at MVP or falls back to plaintext is a question of editor behavior (touch/IME) inside an Android WebView, not a platform-architecture split, and is left to the editor evaluation itself (see the issue tracker) rather than decided here.

Full-text search is SQLite FTS5 via `rusqlite`, compiled into the shared Rust core. The same SQLite file also holds a plain backlinks table, so one dependency covers both derived-index needs at Cerebrite's scale (~500 files) without the added power (and added weight) of a dedicated engine like Tantivy. The derived index is rebuilt on app launch and again after every completed git sync, deliberately without a persistent file-system watcher: a full rebuild is already sub-second at this scale, so the on-disk cache exists to make "just rebuild" cheap and always-correct rather than to avoid slowness, and a watcher would fight Android's Doze/background-execution limits for no real benefit, since git sync is the only channel through which files change outside a direct in-app edit.

## Considered options

- **Kotlin Multiplatform + Compose Multiplatform**: a credible, actively-maintained alternative, but it would split the stack across two UI toolkits and a Rust↔Kotlin boundary, and its rich-text-editor ecosystem is markedly less mature than the web-based ProseMirror one (Milkdown/Tiptap) this decision leans on. Remains the documented fallback if Tauri's Android story turns out weaker than expected.
- **Flutter**: passes the maintenance bar, but its desktop (Windows/Linux) story is the weaker of the group, underscored by Google recently handing desktop stewardship to Canonical.
- **.NET MAUI**: ruled out outright — Microsoft does not support or plan to support Linux desktop.
- **Electron**: ruled out outright — no mobile target at all, so it can't cover Android from the same codebase.
- **Tantivy** (search): more powerful than a ~500-file personal KB needs, and doesn't double as backlink storage the way one SQLite file does. Kept as the upgrade path if search needs ever outgrow FTS5.

## Consequences

This decision is deliberately not blocked on three risks the research flagged — Tauri's Android production maturity beyond "it builds," no direct rebuild-time benchmark on real Android hardware, and unverified `git2-rs` Android cross-compilation — but they must clear (via a dedicated smoke-test task) before Android-specific implementation work starts, since a bad outcome there is what would send this decision back to the Compose Multiplatform fallback.
