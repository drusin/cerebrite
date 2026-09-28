# 01: Foundation — Vue, Storybook, dialogs, mountIsland

**What to build:** The tooling every later step stands on, with **no behaviour change** to the app. Vue 3 builds and type-checks alongside the vanilla code; every native dialog goes through one wrapper so containers can call it later; a dev-only Storybook compiles in CI. See [spec: Tooling](../../component-workbench/spec.md#tooling) and [Step 0](../../component-workbench/spec.md#step-0-foundation).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] `vue` at `^3.5` (no release candidates) and `@vitejs/plugin-vue` added; Vite config uses the plugin
- [ ] `build` script is `vue-tsc --noEmit && vite build`
- [ ] A thin `mountIsland(el, Container, rootProps)` helper around `createApp(...).mount(el)` exists (no `{update, destroy}` handle)
- [ ] A dialogs module next to `vault-api.ts` wraps `@tauri-apps/plugin-dialog` `confirm`/`message` and `window.prompt`/`confirm`/`alert`; **every** such call in `main.ts` goes through it, and dialog UX is unchanged
- [ ] Storybook `^10.6` with `@storybook/vue3-vite` and `@storybook/addon-themes` as devDependencies only; no `addon-a11y`, `addon-docs`, `addon-vitest`, or Vitest
- [ ] Storybook main config sets `core.disableTelemetry: true`
- [ ] Storybook preview imports the global stylesheet, sets up `withThemeByDataAttribute` on `<html>` `data-theme` with System (attribute removed) / Light / Dark, and adds phone and tablet viewport presets next to the full-width default
- [ ] `npm run storybook` starts the dev server; `npm run build-storybook` (or equivalent) builds into a git-ignored throwaway directory
- [ ] CI runs `storybook build` next to `npm run build`; output is never uploaded or deployed
- [ ] Nothing the app imports under `src/` imports a `*.stories.ts` file or `@storybook/*`
- [ ] If `storybook build` cannot pass with zero stories, the Storybook config may land with ticket 02's first story instead — note which in the commit
- [ ] No dependency is end-of-life, deprecated, or abandoned
- [ ] `npm run build` (`vue-tsc --noEmit && vite build`) passes
- [ ] `storybook build` passes
- [ ] A manual run of the real Tauri app covers the flows this step touched (see [debugging in a sandbox](../../../docs/agents/debugging-sandbox.md))
