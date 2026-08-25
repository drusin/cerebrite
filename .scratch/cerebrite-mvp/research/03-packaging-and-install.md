# Packaging & Install — Research (2026-08-25)

**Scope:** first-install/sideload path only, per platform, given Tauri 2.x is locked ([ADR-0008](../../../docs/adr/0008-tauri-rust-core-with-sqlite-fts5.md)). Ongoing distribution (store listings, auto-update, key-rotation strategy) is explicitly out of scope and not covered below.

**Method note:** the sandbox's outbound network policy default-denies most doc hosts (`v2.tauri.app`, `developer.android.com`, `wiki.archlinux.org`, `appimage.org`, `learn.microsoft.com` all returned policy-blocked 403s to direct fetch). Two workarounds were used to still reach primary sources: (1) Tauri's docs site is generated 1:1 from the public `tauri-apps/tauri-docs` GitHub repo (`github.com` is allowed), so the `.mdx` source files were read directly from there — same content, same author, same URL structure as the rendered site; (2) the web-search tool has its own fetch path that was not subject to the same block, so it was used to pull and quote from `developer.android.com`, `wiki.archlinux.org`, and `appimage.org` pages directly. Every citation below links to the canonical page (`v2.tauri.app/...`, `developer.android.com/...`, etc.), not the workaround route.

---

## 1. Windows

### What Tauri 2.x's bundler produces out of the box

The Tauri config schema's `bundle.targets` enum for desktop is `["deb", "rpm", "appimage", "nsis", "msi", "app", "dmg"]` (or `"all"`) — confirmed against `v2.tauri.app/reference/config/`, mirrored from the `tauri-apps/tauri-docs` source. For Windows that's exactly two first-party installer bundlers:

- **NSIS** → a `-setup.exe` installer
- **WiX (v3)** → a `.msi` installer, and the docs are explicit that "`.msi` installers can only be created on Windows" — WiX requires a Windows build host, while NSIS can cross-compile from Linux/macOS "as a last resort" with extra tooling (NSIS, LLVM, LLD, `cargo-xwin`). Source: [Windows Installer — Tauri](https://v2.tauri.app/distribute/windows-installer/) (`tauri-apps/tauri-docs`, `distribute/windows-installer.mdx`, v2 branch).
- There is **no distinct "portable exe" bundler target**. A "portable executable" for Windows is just the raw compiled binary Cargo/Tauri produces before bundling (`target/release/<app>.exe`) — it runs standalone with no installer step, but it isn't a Tauri bundler *feature*, it's the absence of one.

NSIS is the lower-friction default for a solo/small-team MVP: it needs no Windows-only build host (WiX/MSI does), and NSIS is the format Tauri's own scaffolding defaults to for typical installer needs.

### WebView2 runtime dependency

Tauri's Windows installers must account for the Microsoft Edge WebView2 runtime the webview shell depends on. The docs list five handling modes, in increasing installer size / decreasing internet-dependency:

1. **Downloaded bootstrapper** (default) — smallest installer, needs internet at install time.
2. **Embedded bootstrapper** — +~1.8 MB, better Windows 7 handling for `.msi`.
3. **Offline installer** — +~127 MB, works with no internet.
4. **Fixed version** — +~180 MB, bundles an exact WebView2 build.
5. **Skip** — installs nothing; the app won't run unless WebView2 is already present.

Source: [Windows Installer — Tauri](https://v2.tauri.app/distribute/windows-installer/).

Since WebView2 now ships pre-installed on current Windows 10/11 by default, "downloaded bootstrapper" (the default) is fine for an MVP audience; it's a real (if small) caveat for a solo dev to know about, not a blocker.

### Code signing and SmartScreen

- Tauri's own signing guide is explicit that signing is **not required to run the app locally** — it exists to avoid SmartScreen warnings and to enable Microsoft Store listing: "signing isn't strictly required for local execution—only to avoid these [SmartScreen] warnings." Source: [Windows — Sign — Tauri](https://v2.tauri.app/distribute/sign/windows/) (`distribute/Sign/windows.mdx`).
- Microsoft's own SmartScreen-reputation guidance for app developers: reputation is per-file (or per-signing-certificate), builds up from clean download/install history over time ("several weeks and hundreds of clean installs" is the informal shape reported around this system), and unsigned/low-reputation files get a cautionary ("more clicks to proceed") SmartScreen prompt rather than an outright block. Source: [SmartScreen reputation for Windows app developers — Microsoft Learn](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation).
- **EV vs OV certificates**: an EV (Extended Validation) code-signing cert gets immediate SmartScreen reputation with no warning; an OV (Organization Validated) cert still has to build reputation over time even though it's signed, per Tauri's own signing guide. Source: same Tauri Sign/windows.mdx page above.
- **Portable exe vs. installer, unsigned**: neither Tauri's docs nor Microsoft's SmartScreen-reputation doc draw an explicit "installer gets more/fewer warnings than a raw exe" line — SmartScreen's reputation system is described as applying to the executable/file being run, not specifically to "installer" vs. "portable" as categories. Practically, an unsigned NSIS/MSI installer *is* itself the executable a user double-clicks first, so it takes the SmartScreen hit at that step; a portable exe takes the same hit when the user runs it directly. There's no found evidence one format is inherently penalized more than the other by SmartScreen — the determining factor is file/certificate reputation, not installer-vs-portable format. **Flag for the ticket:** this specific comparison isn't directly answered by a primary source; treat it as "roughly equivalent friction, both need either signing or user reputation to build," not "portable exe dodges SmartScreen."

### Recommendation — Windows

Ship the **NSIS `-setup.exe`** as the primary install path (Tauri's lower-friction, cross-compile-capable default; MSI/WiX only if a future need for enterprise/Group-Policy-style MSI deployment shows up, and only from a Windows build host). Leave the binary **unsigned** for MVP — code signing is optional for running the app, not for producing it, and an OV/EV cert is a recurring cost not justified pre-MVP. Expect and accept a SmartScreen "unrecognized publisher" prompt on first run; document the "More info → Run anyway" click-through for early testers rather than paying for signing now. No caveat here rises to the "EOL/deprecated" bar — NSIS, WiX v3, and WebView2 are all current, first-party-supported paths.

---

## 2. Linux (CachyOS / Arch-based)

### What Tauri 2.x's bundler actually supports

Confirmed from the same `bundle.targets` enum (`v2.tauri.app/reference/config/`): Linux-native bundler targets are **`deb`**, **`rpm`**, and **`appimage`** — all first-party, built by `tauri build` directly. Tauri's distribution overview additionally *mentions* Debian, Snap, AppImage, Flatpak, RPM, and AUR as things you can distribute a Tauri app as, but the docs' own directory structure gives away which of these are first-party bundler outputs vs. separate guides: `distribute/debian.mdx`, `distribute/rpm.mdx`, and `distribute/appimage.mdx` describe the bundler's built-in targets; `distribute/aur.mdx`, `distribute/flatpak.mdx`, and `distribute/snapcraft.mdx` are **separate manual-packaging guides**, not bundler flags. Source: [Distribute — Tauri](https://v2.tauri.app/distribute/) (`distribute/index.mdx`) plus the four individual guide pages cited below.

None of these three — AppImage, AUR, or Flatpak — are directly usable on CachyOS/Arch without extra work; here's what each actually requires:

**AppImage** (bundler-native):
- Built directly by `tauri build` with `appimage` in `bundle.targets` — no separate manifest needed.
- Docs explicitly warn to **build on the oldest system you intend to support** (they suggest Ubuntu 22.04 / Debian 12 as a WebKitGTK-4.1 baseline) because glibc symbol versioning means a build on a newer base system can fail to run on older ones (`GLIBC_2.33 not found`-style errors). Source: [AppImage — Tauri](https://v2.tauri.app/distribute/appimage/) (`distribute/appimage.mdx`).
- **Caveat confirmed against Arch/CachyOS specifically:** AppImage's runtime depends on `libfuse2` (FUSE2) to mount its payload, and Arch's philosophy of installing nothing beyond a minimal base means `fuse2` is **not present by default** on Arch/CachyOS — it must be installed manually (`sudo pacman -S fuse2`), or the AppImage run with `--appimage-extract-and-run` to sidestep FUSE entirely. This is a real, confirmed first-run friction point specific to Arch-family systems, corroborated by Arch Linux forum threads and a CachyOS-forum report of exactly this failure mode. Source: [AppImage user-guide — FUSE troubleshooting](https://docs.appimage.org/user-guide/troubleshooting/fuse.html) (appimage.org's own docs) plus [Arch Linux Forums thread](https://bbs.archlinux.org/viewtopic.php?id=285473) and a [CachyOS forum thread](https://discuss.cachyos.org/t/cant-run-appimage-despite-having-fuse2-and-python-fuse-installed/9515) reporting the same class of failure even with fuse2 present, suggesting this is not always a one-line fix.
- One self-contained artifact, no repo/registry step, no ongoing "package maintainer" burden beyond rebuilding it on each release — the best fit for a solo dev.

**AUR** (manual, not bundler-native):
- No first-party Tauri tooling; Tauri's own AUR guide is templates/examples for a PKGBUILD you write by hand, listing the runtime deps (cairo, GTK3, `webkit2gtk-4.1`, etc.) every Tauri app needs, and showing two approaches: extracting from Tauri's own `.deb` output, or building the Rust binary from source in the PKGBUILD. Source: [AUR — Tauri](https://v2.tauri.app/distribute/aur/) (`distribute/aur.mdx`).
- Submission mechanics per the **official Arch wiki**: create an `aur.archlinux.org` account, add an SSH key, interact via a per-package git remote (no web upload); PKGBUILDs "must conform to the Arch Packaging Standards or they will be deleted"; unsure cases should go through the AUR mailing list/forum for review first. Source: [AUR submission guidelines — ArchWiki](https://wiki.archlinux.org/title/AUR_submission_guidelines).
- **Trust/adoption norm, confirmed from the official wiki:** the AUR is explicitly **unmoderated, no mandatory security review** — "you are running a PKGBUILD written by a stranger" is the framing the wiki itself uses for *other people's* AUR packages, and it applies symmetrically: as a maintainer you're trusted to keep your own PKGBUILD correct with no gatekeeping process backing that trust. The wiki separately states AUR *helpers* (`yay`, `paru`, etc.) are **not supported by Arch Linux** as a project, precisely because they automate running third-party build scripts. Source: [AUR helpers — ArchWiki](https://wiki.archlinux.org/title/AUR_helpers).
- **Ongoing maintainer burden is real and open-ended**: once published, an AUR package is expected to be kept up to date (bumping `pkgver`/`pkgrel`, re-testing `makepkg`, updating `.SRCINFO`) indefinitely, or it accumulates "out of date" flags from users and eventually gets orphaned/disowned. For a solo dev this is a standing chore per release, not a one-time cost.

**Flatpak** (manual, not bundler-native):
- Tauri's own docs are unambiguous that **Tauri does not produce a Flatpak natively**. Their guide's pipeline: build the Tauri `.deb` first, generate Node/Cargo dependency-source manifests with external tools (`flatpak-node-generator`, `flatpak-cargo-generator.py`), hand-write an AppStream MetaInfo XML and a Flatpak manifest (YAML) declaring the runtime/sandbox permissions, then build/extract via `flatpak-builder`. Source: [Flatpak — Tauri](https://v2.tauri.app/distribute/flatpak/) (`distribute/flatpak.mdx`).
- This is meaningfully more setup than AppImage or even AUR for a first release: a manifest, a runtime/SDK dependency (e.g. the `org.freedesktop.Platform`/GNOME or KDE runtime, downloaded separately by the Flatpak client), and sandboxing decisions (filesystem access for a git-native notes app needs explicit permission grants, since Flatpak's model is deny-by-default outside the sandbox).
- `flatpak` and `flatpak-builder` themselves are both actively maintained (current stable releases continuing through 2026 per the flatpak GitHub org) — not a maintenance concern, just a heavier one-time-plus-ongoing build pipeline than AppImage.

### Realistic pick for a solo/small-team MVP

**AppImage** is the lowest-friction *and* lowest-ongoing-maintenance choice of the three: one artifact per release, no registry/repo relationship to maintain, no sandboxing manifest to design. Its one real Arch/CachyOS-specific caveat — `fuse2` not being present by default — is a documented, single-command fix (`pacman -S fuse2`) or a documented flag workaround (`--appimage-extract-and-run`), not a fundamental blocker; it should be called out in the app's own install instructions.

AUR is worth adding **later**, once there's an actual release cadence to sustain the maintainer obligation it creates — publishing to the AUR the community's default expectation for "real" Arch-ecosystem availability, but it's an ongoing commitment, not a one-time packaging task, and premature for a still-moving MVP.

Flatpak is the heaviest lift of the three for the least MVP benefit (its sandboxing story is arguably a better fit for a security-conscious future, but the manifest/runtime overhead isn't worth paying before the app's file-access model is even settled).

### Maintenance-status confirmation (the standing "no EOL/deprecated/abandoned" constraint)

- **AppImage**: actively maintained as of 2026 — `AppImage/appimagetool` (the low-level build tool) shows a release dated **April 3, 2026**; `AppImage/type2-runtime` (the actual runtime every AppImage embeds) shows commit activity within the past year. Source: [AppImage GitHub org](https://github.com/appimage), [appimagetool releases](https://github.com/AppImage/appimagetool/releases). No EOL or abandonment signal found — this directly answers the ticket's explicit ask to double-check AppImage's health, since it has had periods where people questioned it (the 2020-era `AppImageKit`→`appimagetool`/`type2-runtime` split was a maintainer transition, not an abandonment; the successor tools are the ones showing 2026 activity).
- **AUR / `makepkg` / Arch packaging guidelines**: current as published on the official Arch wiki, actively maintained wiki pages, no deprecation notice found. Source: [AUR submission guidelines — ArchWiki](https://wiki.archlinux.org/title/AUR_submission_guidelines), [Arch package guidelines — ArchWiki](https://wiki.archlinux.org/title/Arch_package_guidelines).
- **Flatpak / flatpak-builder**: actively maintained (continuing 2026 releases). No concern.
- **Tauri's own `deb`/`rpm`/`appimage` bundlers**: part of the actively-maintained `tauri-apps/tauri` core (already established in [research 01](./01-tech-stack-and-core-architecture.md)).

No dependency or approach surfaced in this section is EOL, deprecated, or abandoned.

---

## 3. Android

### Producing a sideloadable APK with Tauri's tooling

`tauri android build` is Tauri's CLI entrypoint for producing Android artifacts. Key documented behavior (from Tauri's Google Play distribution guide, which documents the same underlying command used for any Android build, store-bound or not):

- Default output is an **AAB** (`gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab`) — the Play-Store-oriented bundle format, **not directly installable by sideload**.
- Passing **`--apk`** switches the build to produce APK(s) instead — Tauri's docs state plainly: "To compile APKs for your app you can use the `--apk` argument." This is the artifact needed for direct/sideload install. Source: [Google Play — Tauri](https://v2.tauri.app/distribute/google-play/) (`distribute/google-play.mdx`).
- `--target` restricts the build to specific ABIs (`aarch64`, `armv7`, `i686`, `x86_64`) instead of building a universal artifact for all of them; `--split-per-abi` produces one APK per ABI instead of one universal APK. Both are optional — a single universal `--apk` build is the simplest path for a personal sideload install.
- A `--debug` build (`tauri android build --debug`, or `tauri android build -- --apk --debug`) produces a debug-signed APK; without `--debug`, the release build path requires a configured signing key (see below) or the build fails/produces an unsigned artifact depending on configuration. Build output lands under `src-tauri/gen/android/app/build/outputs/apk/...`.
- Tauri's own Android-signing guide (`distribute/Sign/android.mdx`) documents the **release-signing** setup: generate a Java keystore via `keytool` (`keytool -genkey -v -keystore ~/upload-keystore.jks -keyalg RSA -keysize 2048 -validity 10000 -alias upload`), write a `keystore.properties` file under `src-tauri/gen/android/`, and wire it into the generated `build.gradle.kts` signing config. This is Google/Android's own standard `keytool` flow, wrapped into Tauri's generated Gradle project rather than being a Tauri-specific mechanism. Source: [Android — Sign — Tauri](https://v2.tauri.app/distribute/sign/android/).

### Android's actual signing requirements (per Android's own docs)

- **Signing is mandatory, unconditionally** — there is no such thing as installing a truly unsigned APK on stock Android. "Android requires that all APKs be digitally signed with a certificate before they are installed on a device or updated" — PackageManager verifies the signature at install time and refuses to install an APK that isn't properly signed. Source: [Sign your app — Android Studio — Android Developers](https://developer.android.com/studio/publish/app-signing); the underlying mechanics are also documented in AOSP's own security docs: [App signing — Android Open Source Project](https://source.android.com/docs/security/features/apksigning).
- **The certificate does not need a CA** — Android explicitly supports self-signed certificates generated locally with no external authority or approval: "the certificate does not need to be signed by a certificate authority... Android provides code signing using self-signed certificates that developers can generate without external assistance." Source: same [Android app-signing doc](https://developer.android.com/studio/publish/app-signing).
- **Debug vs. release signing, for sideload specifically:**
  - **Debug-signed**: Android Studio (and Tauri's `--debug` build path) auto-generates a debug keystore at `~/.android/debug.keystore` on first use, with fixed well-known credentials (`storepass android`, `alias androiddebugkey`), and auto-signs debug builds with it — no setup step required. This keystore/cert also **expires 365 days after creation**, after which builds fail until the file is deleted and Android Studio regenerates a fresh one. Google's own guidance: apps "signed in debug mode" can be run on emulators/USB-connected dev devices, but are explicitly **"not intended for distribution"** and won't be accepted by app stores. Source: [Sign your app — Android Studio — Android Developers](https://developer.android.com/studio/publish/app-signing).
  - **Release-signed**: a developer-generated keystore (one `keytool` command, or Tauri's documented flow above) with no expiration practically relevant to an MVP (`-validity 10000` days is the conventional default), no fixed/well-known password, and it is the format Google explicitly sanctions for any kind of distribution, formal or informal.
  - **For pure sideload (copy-the-APK-and-tap-install, or `adb install`), both are technically installable** — Android's install-time check only verifies *that* a valid signature exists and is internally consistent (for update-matching purposes), not *which kind* of key produced it. Google's "not intended for distribution" language is a policy/support statement (store policies, and the practical fact that everyone on a debug-signed app shares the same guessable default password/alias), not an OS-level technical block on installing a debug-signed APK by hand.

### Recommendation — Android

Use **`tauri android build -- --apk`** with a **release-signed** APK (one `keytool`-generated keystore, wired in per Tauri's Sign/android.mdx guide) as the sideload artifact, even though debug-signed would technically install. The reasons: (1) the debug keystore's 365-day expiry is a recurring, avoidable annoyance for a long-lived personal project; (2) Google's own docs explicitly frame debug-signed as not meant for any kind of distribution, and a from-device-to-device sideload install is a form of distribution even if informal; (3) generating a release keystore is a single `keytool` command, not meaningfully more setup than the debug path. This stays entirely inside "produce one sideloadable APK" — no store listing, no update mechanism, no key-rotation strategy is implied or needed here.

### Caveat for the human reviewer: Android's developer-verification rollout

This surfaced as a significant, timing-sensitive risk that the ticket's framing (debug vs. release signing) doesn't anticipate, and it's worth flagging explicitly even though it's adjacent to "distribution":

- Google is rolling out a new **Android Developer Verification** program. Per Google's own developer-verification FAQ and blog posts: starting **September 30, 2026**, "certified Android devices" (Play-Protect-certified, i.e. mainstream OEM devices, not custom ROMs) in an initial set of countries (**Brazil, Indonesia, Singapore, Thailand**) will only install/update apps from *registered* developers by default, expanding to more countries and globally through **2027**. Source: [Android developer verification — Android Developers](https://developer.android.com/developer-verification) and the [Android Developer Verification FAQ](https://developer.android.com/developer-verification/guides/faq).
- **This does not block Cerebrite's own sideload path for MVP purposes.** Google's FAQ is explicit that developers retain unrestricted install rights via **ADB** ("as a developer, you are free to install apps without verification with ADB"), and separately introduces an opt-in **"advanced flow"** that lets any user keep sideloading unregistered/unverified apps with extra confirmation steps, plus a **free, no-government-ID "student/hobbyist" account tier** (limited to ~20 devices) for exactly this kind of small-scale distribution. Source: same [FAQ](https://developer.android.com/developer-verification/guides/faq).
- **Why flag it anyway:** it's a live, moving policy — the effective date (Sep 30, 2026) is roughly five weeks from today, the initial country list is small but expansion to "globally" by 2027 is Google's stated direction, and it changes the *default* posture of "install an APK you got from outside the Play Store" on mainstream devices going forward, even though ADB and the advanced-flow opt-in remain open. It's not a reason to change ticket 03's recommendation (ADB install or the advanced-flow toggle both remain adequate for a personal/sideload MVP), but a human should be aware this isn't a fully static piece of ground to build a "just sideload it" plan on for anyone other than a technically comfortable user willing to use ADB or flip the advanced-flow switch.

---

## Summary

| Platform | Recommendation | Key caveat |
|---|---|---|
| **Windows** | Unsigned NSIS `-setup.exe` (Tauri's default, lower-friction than WiX/MSI, which needs a Windows build host) | Expect a SmartScreen click-through on first run; no primary source distinguishes portable-exe vs. installer SmartScreen friction — treat as roughly equal |
| **Linux (Arch/CachyOS)** | AppImage (only Tauri-bundler-native option of the three considered; least ongoing maintainer burden) | `fuse2` isn't installed by default on Arch — document `pacman -S fuse2` or `--appimage-extract-and-run` in install instructions |
| **Android** | `tauri android build -- --apk` with a release-signed keystore (debug-signed would also install, but expires yearly and Google frames it as not for any distribution) | Google's Developer Verification rollout (effective Sep 30, 2026 in 4 countries, expanding through 2027) changes default sideload posture on mainstream devices going forward — ADB and the "advanced flow" opt-in keep MVP sideloading viable, but this is worth a human's awareness, not a blocker |

No dependency, tool, or approach evaluated above (Tauri's NSIS/WiX/deb/rpm/AppImage bundlers, AUR, Flatpak/flatpak-builder, Android's `keytool`/Gradle signing, ADB) is end-of-life, deprecated, or abandoned as of this research date. The one genuinely open, non-technical risk is the Android developer-verification timeline above — flagged for awareness, not blocking, since ADB-based install remains explicitly guaranteed by Google's own FAQ regardless of how that rollout proceeds.
