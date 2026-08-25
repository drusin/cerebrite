Checklist for the APK built by `.github/workflows/android-smoke-apk.yml` and
installed on a real device, per
[06-android-platform-smoke-test](06-android-platform-smoke-test.md).

This build covers **all three checks**. On launch the app automatically:
runs the ping/pong round-trip (Check 1), generates a synthetic ~500-file
markdown corpus and times a cold SQLite FTS5 + backlinks rebuild over it on
device (Check 2), and runs a git2-rs init → commit → local-clone round-trip
entirely on-device (Check 3). No adb, no buttons to tap — just install, open,
and read the four result lines on screen.

If the workflow run itself fails at the **"Build debug APK"** step, that
failure *is* your Check 3 answer: `git2-rs` (or `rusqlite`) didn't
cross-compile cleanly to Android. Note the failing step and the error text —
you don't need to install anything in that case.

## Install

1. Trigger the workflow (`gh workflow run android-smoke-apk.yml` or via the
   Actions tab), wait for it to finish, and open the run's published
   GitHub Release.
2. Download the APK from the release on your phone's browser.
3. If prompted, allow your browser to "install unknown apps" — one-time,
   per-app permission on modern Android.
4. Open the downloaded file to install.

## What to check

- [ ] **Installs without a warning about a corrupt/invalid package.**
- [ ] **Launches at all** — no immediate crash or black-screen hang.
- [ ] **Check 1 — round-trip**: the screen shows `ping -> pong` shortly after
      launch. If it instead shows `ping failed: ...`, note the error text.
- [ ] **Check 2 — corpus + index rebuild**: the screen shows `corpus: wrote
      500 files to ...` followed by `index rebuild: <N>ms`. Note the `<N>`.
      Sub-second (< 1000ms) is a PASS per the standing constraint; anything
      at or above 1000ms, or an `index rebuild failed: ...` line, is a FAIL.
- [ ] **Check 3 — git2-rs**: the screen shows `git: commit + local clone OK,
      1 commit(s) in clone`. A `git failed: ...` line is a FAIL — note the
      error text.
- [ ] **Cold-start time**: rough feel is fine — instant, ~1s, or noticeably
      slow? (All four checks run sequentially on first launch, so a slower
      cold start than Check-1-only is expected — note if it feels rough.)
- [ ] **WebView rendering**: text/layout renders cleanly, no visible
      Chromium/WebView version warnings or broken styling.
- [ ] **Rotate the screen** (if easy to do): app doesn't crash or blank out.
- [ ] **Background/resume**: switch to another app and back — Cerebrite's app
      doesn't crash or lose its state.
- [ ] **Uninstall cleanly** afterwards if you don't need it kept around —
      this is a throwaway smoke-test app, not the real Cerebrite app.

## Record the result

Note pass/fail on each check above, plus:

- Device model + Android version.
- The index-rebuild time in ms (Check 2).
- Anything unexpected (WebView fragmentation, slow cold start, crash logs,
  cross-compilation gotchas from the workflow log if the build itself failed).

Append the results directly under `## Answer` in
[06-android-platform-smoke-test.md](06-android-platform-smoke-test.md),
mark the ticket `resolved`, and add a one-line gist to the map's
Decisions-so-far.
