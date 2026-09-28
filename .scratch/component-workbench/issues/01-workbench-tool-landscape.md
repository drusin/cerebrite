# Workbench tool landscape for a Vite 8 app, vanilla and Vue

Type: research
Status: resolved

## Question

Which component-workbench tools are viable for this repo (Vite 8, TypeScript 6, vanilla DOM today, possibly Vue 3 later), and how do they compare?

Candidates at minimum: Storybook (`@storybook/html-vite` and `@storybook/vue3-vite`), Histoire, and a hand-rolled Vite multi-page "gallery". Include anything else current and relevant.

For each, establish:
- Maturity and maintenance status in 2026 (not deprecated, EOL, or abandoned) and whether it officially supports Vite 8.
- Support for vanilla TS/DOM stories *and* Vue 3 stories, and whether one setup can host both during a gradual migration.
- Out-of-the-box theme (light/dark) switching and viewport presets.
- The path to isolated interaction tests later (for example Storybook's test runner / Vitest addon, Vitest browser mode).
- Setup weight: dependency count, config complexity, dev-server startup.
- Any known trouble with things this app uses: Milkdown/ProseMirror, `@tauri-apps/api` imports being present in the module graph.

## Research

Full findings: branch `research/workbench-tool-landscape` (commit ed7b61e), file `.scratch/component-workbench/research/01-workbench-tool-landscape.md`.

## Answer

The research recommends **Storybook 10.x with `@storybook/html-vite`** (plus `@storybook/addon-themes`), with a hand-rolled Vite gallery as the fallback. Histoire is not viable.

- **Storybook:** 10.6.0 (2026-09-02) is current, with 11.0 in alpha, and Vite 8 has been officially supported since 10.3.0.
  - **Themes and viewports:** addon-themes can toggle `data-theme` on `<html>`, which is what `styles.css` already switches on. Viewport presets are built in.
  - **Milkdown:** Milkdown's own repo uses the same html-vite setup.
  - **Smoke test** (scratch project, Vite 8.3.1, Milkdown 7.22.1): the editor rendered with `@tauri-apps/api` imported, which is harmless unless called. The dev server was ready in about 4 s, and the setup adds about 81 packages.
- **Vanilla and Vue together:** one html-vite Storybook can host both, confirmed by mounting a Vue component with the Vue plugin inside a plain story. Vue cleanup when switching stories was not verified.
- **Interaction tests later:** `play` functions plus `@storybook/addon-vitest`. Watch item: 10.6 accepts only Vitest 3 or 4, while Vitest 5.0.2 is the latest, so pin 4.1.x (which supports Vite 8) or wait for Storybook 11. `sb.mock` can module-mock `vault-api` if a fake is ever needed.
- **Histoire:** the latest release is `1.0.0-beta.1` (2026-01-07) with nothing since. It peers on Vite `^7.3.0` only, so it installs only with `--legacy-peer-deps` and pulls in a second copy of Vite. It has no vanilla/HTML support.
- **Hand-rolled gallery:** needs no new dependencies, but the theme toggle, viewports, and story index would all be built by hand, with tests via Vitest browser mode.
- **Others:** Ladle is React-only and Vitebook is deprecated.

Caveat: storybook.js.org, histoire.dev, vite.dev, and v2.tauri.app are blocked by the sandbox firewall, so their docs were read from source repos on GitHub.
