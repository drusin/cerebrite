Type: task
Status: claimed

## Question

Not a decision — validation work that must clear before any Android-specific implementation begins, per the risks flagged (but not treated as blocking) by [01-tech-stack-and-core-architecture](01-tech-stack-and-core-architecture.md) and [ADR-0008](../../../docs/adr/0008-tauri-rust-core-with-sqlite-fts5.md).

Build and run a minimal Tauri Android app on a **real low/mid-range physical device** (not just an emulator), and confirm:

1. **It builds and runs at all**: a bare webview + one Rust command round-trip, launching and responding on-device.
2. **Corpus rebuild time**: generate a synthetic ~500-file markdown corpus resembling realistic notes (headings, links, frontmatter) and measure cold rebuild time of the SQLite FTS5 + backlinks derived index on-device. Must land comfortably sub-second per the standing constraint in the map's Notes.
3. **`git2-rs` cross-compilation**: confirm `git2-rs` (and its bundled libgit2) cross-compiles cleanly to the Android target(s) actually shipped (e.g. `aarch64-linux-android`) and that a basic git operation (clone/pull) runs on-device.

Record what was done and the results: device(s) tested, build steps, measured rebuild time, and any rough edges found (WebView version fragmentation, cold-start time, cross-compilation gotchas). If any check fails outright, that's grounds to revisit [01-tech-stack-and-core-architecture](01-tech-stack-and-core-architecture.md) rather than proceed.

This requires a real physical device and, for the fallback path below, Android SDK/NDK and Rust — none available in the agent's sandbox, so this is HITL.

**Primary path**: trigger [`.github/workflows/android-smoke-apk.yml`](../../../.github/workflows/android-smoke-apk.yml) (`gh workflow run android-smoke-apk.yml` or via the Actions tab). It builds one APK that covers all three checks automatically on launch — no local Rust/Android SDK/adb needed, just download the APK from the run's GitHub Release to your phone and install. Follow [06-android-apk-check.md](06-android-apk-check.md) to drive it and record results. If the workflow fails at its "Build debug APK" step, that failure alone is the Check 3 answer (git2-rs/rusqlite didn't cross-compile cleanly) — nothing to install in that case.

**Fallback path**: if the CI route hits a cross-compilation blocker that needs interactive debugging, run [06-android-smoke-test-wizard.sh](06-android-smoke-test-wizard.sh) on a machine with a physical low/mid-range Android device attached, Rust, and the Android SDK/NDK installed; it walks prerequisite install, scaffolds a throwaway Tauri app locally, drives Checks 2 and 3 (Check 1 is already answered — see the partial Answer below), and appends the results + resolves this ticket + updates the map's Decisions-so-far automatically at the end.

## Answer (partial — Check 1 only)

Check 1 done via the CI-built APK (`.github/workflows/android-smoke-apk.yml`) installed and driven by hand per [06-android-apk-check.md](06-android-apk-check.md):

- Device: Pixel 9a, Android 17
- Check 1 (builds + runs + round-trip on-device): **PASS** — installs cleanly, launches, `ping -> pong` shows, cold-start feels fine, WebView rendering clean, survives rotation and background/resume. Nothing unexpected.
- Check 2 (corpus rebuild time) and Check 3 (git2-rs cross-compile + on-device op): **not yet done** — still need [06-android-smoke-test-wizard.sh](06-android-smoke-test-wizard.sh) (or an extended CI build) to cover those.

Status stays `claimed`, not `resolved`, until Checks 2 and 3 are in.
