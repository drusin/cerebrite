Type: research
Status: resolved
Blocked by: 01

## Question

Given the tech stack chosen in [01-tech-stack-and-core-architecture](01-tech-stack-and-core-architecture.md), what's the easiest install/sideload path for each MVP platform?

- **Windows**: installer vs. portable executable.
- **Linux**, specifically CachyOS/Arch-based: AUR package, AppImage, Flatpak, or another approach.
- **Android**: a directly-distributed, sideloadable APK.

Ongoing distribution (store listings, auto-update infrastructure) is explicitly out of scope — this ticket only needs to answer "how does a user get the app onto their device."

## Research

Findings: [.scratch/cerebrite-mvp/research/03-packaging-and-install.md](../research/03-packaging-and-install.md).

## Answer

Per-platform sideload path, given Tauri 2.x ([01-tech-stack-and-core-architecture](01-tech-stack-and-core-architecture.md)):

1. **Windows**: unsigned NSIS `-setup.exe` — Tauri's lower-friction default bundler target (WiX/MSI needs a Windows build host; NSIS doesn't). Code signing is optional for running the app, only for suppressing SmartScreen's "unrecognized publisher" prompt — not worth an OV/EV cert pre-MVP. Document the "More info → Run anyway" click-through for early testers. WebView2 ships pre-installed on current Windows 10/11, so the default "downloaded bootstrapper" handling mode is fine.
2. **Linux (CachyOS/Arch)**: AppImage — the only one of AppImage/AUR/Flatpak that's a native Tauri bundler target (AUR and Flatpak both require hand-built packaging pipelines outside Tauri), and the lowest ongoing-maintenance option (one artifact per release, no registry relationship). One confirmed Arch-specific caveat: `fuse2` isn't installed by default on Arch/CachyOS — document `pacman -S fuse2` or the `--appimage-extract-and-run` flag in install instructions. AUR is worth revisiting post-MVP once there's a release cadence to sustain its open-ended maintainer burden; Flatpak is the heaviest lift for the least MVP benefit.
3. **Android**: `tauri android build -- --apk` with a **release-signed** keystore (one `keytool` command), not debug-signed. Signing is unconditionally mandatory on Android but the cert needs no CA — self-signed is fully sufficient. Release-signing beats debug-signing here mainly because the debug keystore expires after 365 days, a recurring annoyance for a long-lived personal project, and because Google's own docs frame debug-signed as not intended for any kind of distribution.

No dependency or approach evaluated (Tauri's NSIS/WiX/deb/rpm/AppImage bundlers, AUR, Flatpak, Android's `keytool`/Gradle signing, ADB) is end-of-life, deprecated, or abandoned, satisfying the map's standing constraint.

**Flagged for awareness, not a blocker**: Google's Android Developer Verification rollout takes effect 2026-09-30 in an initial set of countries (Brazil, Indonesia, Singapore, Thailand), expanding through 2027, and shifts the *default* posture for installing APKs outside Play on mainstream devices. It does not block this recommendation — Google's own FAQ guarantees unrestricted ADB install plus an opt-in "advanced flow" for sideloading regardless of verification status — but it's a live policy change worth knowing about since the effective date is close.
