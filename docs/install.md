# Installing Cerebrite

Cerebrite is not yet published to any app store or package registry. There is
no public release page either. Until that changes, the way to get the app is
to download one of the three sideload artifacts that CI builds, per
[ticket 16](../.scratch/cerebrite-mvp-implementation/issues/16-packaging-and-install.md).

All three are built from source in GitHub Actions and uploaded as **workflow
run artifacts** (not GitHub Releases), so you need a GitHub account with
access to this repository to download them:

1. Open the **Actions** tab of this repository.
2. Pick the workflow for your platform (see below) and open its most recent
   successful run.
3. Download the artifact from the run's **Artifacts** section at the bottom
   of the run summary page.

Artifacts expire after GitHub's default retention window (90 days on public
repos), so always grab the latest run rather than an old link.

### What's actually been verified (2026-09-16)

This sandbox has no Windows host, no display, and no code-signing keys, so
verification was necessarily partial:

- **Windows NSIS**: CI-only. Not built or run anywhere in this effort —
  verified by reading `.github/workflows/ci.yml`, not by executing it.
- **Linux AppImage**: `cargo build`/`cargo test` and the underlying app
  binary build fine locally, but the actual AppImage bundling step could
  **not** be completed in this sandbox (see the fuse2 section below for the
  end-user-facing caveat, and the ticket/PR notes for why the *build* itself
  failed here specifically).
- **Android APK**: built and verified **locally**, release-signed with a
  throwaway dev keystore, containing the real native library
  (`lib/arm64-v8a/libcerebrite_lib.so`, ~14MB) — confirmed via `unzip -l` and
  `apksigner verify --print-certs`. Not installed on a real device (none
  available here).

## Windows: NSIS installer

Workflow: `.github/workflows/ci.yml` (job `build`, `windows-latest`), artifact
name `cerebrite-windows-nsis-installer`. It contains an unsigned
`Cerebrite_<version>_x64-setup.exe`.

This installer is **not code-signed**. Windows SmartScreen will show an
"Windows protected your PC" warning when you run it. To proceed:

1. Click **More info**.
2. Click **Run anyway**.

This is expected for an unsigned installer and does not indicate a corrupted
download. Code signing (an OV/EV certificate) was deliberately deferred —
see the [research note](../.scratch/cerebrite-mvp/research/03-packaging-and-install.md)
for why it isn't worth doing pre-MVP.

WebView2 (the runtime the app embeds) ships preinstalled on current Windows
10/11, so no extra runtime download should be needed.

## Linux: AppImage

Workflow: `.github/workflows/ci.yml` (job `build`, `ubuntu-latest`), artifact
name `cerebrite-linux-appimage`. It contains `cerebrite_<version>_amd64.AppImage`.

To run it:

```sh
chmod +x cerebrite_*.AppImage
./cerebrite_*.AppImage
```

### `fuse2` caveat (Arch, CachyOS, and other modern distros)

AppImages run themselves by mounting via FUSE. Many current distributions —
notably **Arch Linux and CachyOS** — no longer ship the legacy `fuse2`
library by default (they've moved to `fuse3`), and AppImage's runtime still
needs `fuse2`/`libfuse2` specifically. If you see an error like:

```
dlopen(): error loading libfuse.so.2
AppImages require FUSE to run.
```

you have two options:

1. **Install `fuse2`** (the fix, if you're fine adding the package):
   ```sh
   # Arch / CachyOS
   sudo pacman -S fuse2
   ```
2. **Extract and run without FUSE** (no package install needed):
   ```sh
   ./cerebrite_*.AppImage --appimage-extract
   ./squashfs-root/AppRun
   ```

Note that this caveat only affects *running* an AppImage — building one (as
CI does) does not require FUSE at all. That said, Tauri's AppImage *bundler*
itself shells out to `linuxdeploy`, which is distributed as an AppImage —
so building genuinely can hit a FUSE requirement too, on a machine with no
`/dev/fuse` (this sandbox is one such machine; see the ticket/PR notes). CI's
`ubuntu-latest` runners have full FUSE support, so this does not affect the
CI-built artifact.

## Android: sideload APK

Workflow: `.github/workflows/android-build.yml` (manual trigger — see below),
artifact name `cerebrite-android-debug` today. See **Signing status** below:
this workflow currently produces a **debug-signed** APK, not the release-signed
one the ticket calls for; a release-signed CI workflow still needs a real
production keystore wired in as a secret before it can be added (see "What's
left" below). The release-signed build path itself (with a throwaway dev
keystore) has been verified locally — see `tauri android build --target
aarch64 --apk` (the `--apk` flag belongs to `tauri android build` itself, not
after a `--` separator, despite how some older docs phrase it).

To install on a device:

1. Enable "Install unknown apps" for your browser or file manager in
   Android's Settings (per-app permission, requested automatically on first
   install attempt on modern Android).
2. Download the `.apk` file to the device and open it, or `adb install
   cerebrite.apk` from a computer with the device connected over USB with
   USB debugging enabled.

### Signing status: dev keystore, not production

The Android release build path (`tauri android build -- --apk`, release
mode) is signed locally using a **throwaway development keystore** generated
with `keytool` at `src-tauri/gen/android/keystore-dev/cerebrite-dev.jks`
(gitignored, never committed) and referenced from
`src-tauri/gen/android/keystore.properties` (also gitignored). This proves
the release-signed build path works end to end, but:

- **This is not a production signing key.** Anyone can regenerate an
  equivalent throwaway keystore, so an APK signed with it carries no real
  authenticity guarantee.
- Before any real distribution, a human must generate a real keystore (kept
  in a password manager / secrets vault, never in the repo) and either:
  - copy `src-tauri/gen/android/keystore.properties.example` to
    `keystore.properties` and fill in the real values locally, or
  - wire the same values into CI as encrypted secrets and write them to
    `keystore.properties` (or export the equivalent env vars Tauri/Gradle
    read) at build time.
- Losing that real keystore means future updates can never be signed with
  the same identity again — back it up somewhere durable.

See [Tauri's Android signing docs](https://v2.tauri.app/distribute/sign/android/)
for the canonical reference on this setup.

Per Google's evolving [Android Developer Verification](https://support.google.com/googleplay/answer/16374792)
rollout (effective 2026-09-30 in an initial set of countries, expanding
through 2027), sideloading policy on mainstream devices is shifting, but
Google's own FAQ guarantees unrestricted ADB install plus an opt-in
"advanced flow" for sideloading regardless of verification status — this
does not block sideload installs as described above.

### Building the release APK locally: JDK version note

Gradle's Kotlin DSL compiler (as bundled with the Gradle version this Android
project uses) fails to parse a JDK 25 version string
(`java.lang.IllegalArgumentException: 25.0.3`) when configuring `buildSrc`.
Build with **JDK 17** (already what `.github/workflows/android-build.yml`
uses via `actions/setup-java@v4`) — set `JAVA_HOME` to a JDK 17 install
before running `tauri android build` locally if your default `java` resolves
to something newer.

## What's left for a human with real hardware/signing keys

- **Windows**: nobody has actually run the NSIS installer CI produces, on a
  real Windows machine, in this MVP effort. Do that before calling Windows
  sideloading done.
- **Linux AppImage**: get this actually building end to end on a real
  ubuntu-latest CI run (should work there — the local blocker here was
  sandbox-specific, see the ticket/PR notes) and confirm the artifact runs on
  a real Arch/CachyOS box, both with and without `fuse2` installed.
- **Android**: generate a **real production keystore** (see
  `src-tauri/gen/android/keystore.properties.example`), store it somewhere
  durable and secret, and either build release APKs locally with it or wire
  it into CI as encrypted secrets so `android-build.yml` (or a new
  release-mode workflow) can produce a properly-signed release APK instead
  of today's debug-signed one. Then install the result on a real device.
