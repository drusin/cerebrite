# 16: Packaging & install

**What to build:** The three sideload artifacts a user can actually install, per [03-packaging-and-install](../../cerebrite-mvp/issues/03-packaging-and-install.md): an unsigned NSIS installer for Windows, an AppImage for Linux (documenting the `fuse2` caveat on Arch/CachyOS), and a release-signed (not debug-signed) sideload APK for Android via `tauri android build -- --apk`.

**Blocked by:** 13, 14, 15

**Status:** ready-for-agent

- [ ] Windows: CI produces an unsigned NSIS `-setup.exe`; install instructions document the SmartScreen "Run anyway" click-through
- [ ] Linux: CI produces an AppImage; install instructions document the `fuse2` dependency (or `--appimage-extract-and-run` fallback) on Arch/CachyOS
- [ ] Android: CI produces a release-signed APK (real keystore, not debug) via `tauri android build -- --apk`
- [ ] All three artifacts are built from the feature-complete app (search, git sync, and Android parity all in place)
- [ ] No store-listing or auto-update infrastructure is built — sideload only, per the ticket's explicit out-of-scope boundary
