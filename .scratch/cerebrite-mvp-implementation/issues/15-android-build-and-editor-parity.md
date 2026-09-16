# 15: Android build & editor parity

**What to build:** Bring up the real Cerebrite app (not the throwaway smoke-test harness from [06-android-platform-smoke-test](../../cerebrite-mvp/issues/06-android-platform-smoke-test.md)) on Android via `tauri android build`, with the actual Milkdown editor from ticket 03. Confirm touch and IME interaction hold up in the real app the same way they did in the prototype and smoke test.

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] The real app builds and runs on a physical Android device via `tauri android build`
- [ ] Vault open, page browsing, editing, and save all work on-device
- [ ] Milkdown editor touch/IME behavior confirmed on-device in the real app (not just the standalone prototype)
- [ ] Sidebar renders as the Android drawer variant (ticket 12) rather than the desktop persistent sidebar
- [ ] Cold-start and derived-index rebuild time stay consistent with the smoke test's sub-second result on comparable hardware
