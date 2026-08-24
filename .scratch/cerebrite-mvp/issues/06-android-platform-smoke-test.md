Type: task
Status: open

## Question

Not a decision — validation work that must clear before any Android-specific implementation begins, per the risks flagged (but not treated as blocking) by [01-tech-stack-and-core-architecture](01-tech-stack-and-core-architecture.md) and [ADR-0008](../../../docs/adr/0008-tauri-rust-core-with-sqlite-fts5.md).

Build and run a minimal Tauri Android app on a **real low/mid-range physical device** (not just an emulator), and confirm:

1. **It builds and runs at all**: a bare webview + one Rust command round-trip, launching and responding on-device.
2. **Corpus rebuild time**: generate a synthetic ~500-file markdown corpus resembling realistic notes (headings, links, frontmatter) and measure cold rebuild time of the SQLite FTS5 + backlinks derived index on-device. Must land comfortably sub-second per the standing constraint in the map's Notes.
3. **`git2-rs` cross-compilation**: confirm `git2-rs` (and its bundled libgit2) cross-compiles cleanly to the Android target(s) actually shipped (e.g. `aarch64-linux-android`) and that a basic git operation (clone/pull) runs on-device.

Record what was done and the results: device(s) tested, build steps, measured rebuild time, and any rough edges found (WebView version fragmentation, cold-start time, cross-compilation gotchas). If any check fails outright, that's grounds to revisit [01-tech-stack-and-core-architecture](01-tech-stack-and-core-architecture.md) rather than proceed.
