# Ways to shape a surface: plain modules, web components, Vue 3

Type: research
Status: resolved

## Question

What does it cost, and what does it buy, to shape the frontend's surfaces with each of these, adopted **incrementally** inside an existing vanilla-TS Vite + Tauri 2 app?

1. **Plain TS render modules**, for example `mount(container, props, callbacks)` returning an update/destroy handle.
2. **Native web components / Lit.**
3. **Vue 3**, mounted as islands into the existing vanilla shell and gradually taking over. This is the user's preferred framework if one is adopted.

For each, establish:
- How incremental adoption works in practice (mounting into existing DOM, sharing state with un-migrated code, coexisting with the global `styles.css`).
- Integration with Milkdown 7 (`@milkdown/kit`) and a custom ProseMirror/remark plugin. Is there an official `@milkdown/vue` or equivalent, and is it maintained?
- Tauri 2 compatibility concerns, if any, and bundle-size impact.
- How naturally each gives "props in, events out" surfaces that can be rendered in a workbench without a backend and tested in isolation.
- Maintenance status of every library involved (no deprecated/EOL/abandoned dependencies).

## Research

Full findings: branch `research/surface-shape-options` (commit 068ae70), file `.scratch/component-workbench/research/02-surface-shape-options.md`.

## Answer

All three options can be adopted one surface at a time in the current Vite 8 + Tauri 2 app with no Tauri-specific problems, and none of the libraries is deprecated. As of 2026-09: Vue 3.5.43, Lit 3.3.3, `@milkdown/vue` 7.22.2.

- **Bundle cost** (gzipped, trivial component): plain module 0.2 KB, Lit 5.7 KB, Vue 23.5 KB. The editor alone is 111.7 KB, or 135.0 KB mounted via `@milkdown/vue`.
- **Milkdown:** Vue has an official, maintained binding that accepts any `Editor.make()` chain, so the wiki-link plugin plugs in unchanged. Vue is already present transitively through `@milkdown/kit`. Lit and plain modules drive Milkdown directly, as `PageEditor` already does.
- **Global `styles.css`:** it applies as-is to plain modules and Vue. It does not reach into Lit's shadow DOM, so Lit would need `createRenderRoot() { return this; }` on every component. Lit's own docs discourage that, and it gives up slots and scoping. Lit also clashes with this repo's `useDefineForClassFields: true`: reactive properties would need `declare` or `accessor`.
- **Props in, events out:** Vue has it built in and typed, with first-class Storybook and Vitest support. The cost is `@vitejs/plugin-vue` and switching the build from `tsc` to `vue-tsc`. Lit uses properties plus `CustomEvent`s. Plain modules make it a hand-enforced convention with a hand-written `update()`.
- **Contract can be framework-neutral:** a Vue island can sit behind a plain `mount(el, props, cb) → {update, destroy}` handle. The sync-indicator pilot and the editor story are cheap under all three.

Caveat: vuejs.org, lit.dev, and milkdown.dev are blocked by the sandbox firewall. Their docs were read from the source repos on GitHub; versions and dates come from npm.
