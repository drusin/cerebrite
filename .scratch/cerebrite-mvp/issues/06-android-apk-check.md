Checklist for the APK built by `.github/workflows/android-smoke-apk.yml` and
installed on a real device, per
[06-android-platform-smoke-test](06-android-platform-smoke-test.md).

This build only covers **Check 1** (builds + runs + round-trip). Corpus
rebuild timing and the git2-rs check aren't in this APK yet — those still
need [06-android-smoke-test-wizard.sh](06-android-smoke-test-wizard.sh).

## Install

1. Download the APK from the workflow's GitHub Release on your phone's browser.
2. If prompted, allow your browser to "install unknown apps" — one-time,
   per-app permission on modern Android.
3. Open the downloaded file to install.

## What to check

- [ ] **Installs without a warning about a corrupt/invalid package.**
- [ ] **Launches at all** — no immediate crash or black-screen hang.
- [ ] **Round-trip works**: the screen shows `ping -> pong` shortly after launch.
      If it instead shows `ping failed: ...`, note the error text.
- [ ] **Cold-start time**: rough feel is fine — instant, ~1s, or noticeably slow?
- [ ] **WebView rendering**: text/layout renders cleanly, no visible
      Chromium/WebView version warnings or broken styling.
- [ ] **Rotate the screen** (if easy to do): app doesn't crash or blank out.
- [ ] **Background/resume**: switch to another app and back — Cerebrite's app
      doesn't crash or lose its state.
- [ ] **Uninstall cleanly** afterwards if you don't need it kept around —
      this is a throwaway smoke-test app, not the real Cerebrite app.

## Record the result

Note pass/fail on each item above, plus:

- Device model + Android version
- Anything unexpected (WebView fragmentation, slow cold start, crash logs)

Add this as the `CHECK1_NOTES` value if you later run the full
[06-android-smoke-test-wizard.sh](06-android-smoke-test-wizard.sh), or append
it directly under `## Answer` in
[06-android-platform-smoke-test.md](06-android-platform-smoke-test.md) if
you're recording Check 1 standalone.
