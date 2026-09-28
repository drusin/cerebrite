---
status: accepted
---

# Vue 3 for frontend surfaces, with styling kept global

The frontend was vanilla TypeScript: a ~3.8k-line `main.ts` that hand-patches DOM living in `index.html`. To make each **surface** (sidebar, article, editor, search, sync indicator, Settings, the wizards, …) browsable in isolation in a dev-only workbench, the surfaces are rebuilt as **Vue 3** components. The migration goes one surface at a time as Vue islands mounted into the existing shell, and ends in a single Vue app. Vue won because automatic re-rendering and typed props/emits remove the hand-patching that made `main.ts` complex. The duplicated UI (the device-flow poll ×5, SSH key handling ×4, the "Commit as" form ×3, page lists ×4) turns into shared components. Storybook and Vitest support Vue as a first-class framework, and it is the framework the maintainer knows. The cost is about 23 KB gzipped (irrelevant for an app loaded from disk, where the editor alone is 112 KB), `@vitejs/plugin-vue`, and `vue-tsc` in place of `tsc`.

## Consequences

- Components are `.vue` SFCs with `<script setup lang="ts">`, Composition API only, and typed `defineProps`/`defineEmits`. Vue stays on the stable 3.5 line until 3.6 is released and never runs a pre-release.
- **SFCs carry no `<style>` blocks.** Styling stays in the global `src/styles.css`, which is already organised by surface class prefix and themed through `:root` custom properties and `[data-theme]`. A migrated component renders the same class names and needs no CSS work. The few id selectors become classes as their surface migrates. This is a scope choice, not a prohibition: `<style scoped>` can be introduced later, one component at a time.

## Considered options

- **Plain TS render modules** (`mount(el, props, callbacks) → {update, destroy}`). They cost nothing to add, but every `update()` stays hand-written DOM patching, the thing being moved away from.
- **Web components / Lit.** Rejected. Global styles do not reach into shadow DOM, and the opt-out (`createRenderRoot() { return this; }`) gives up slots and scoping. Lit also conflicts with `useDefineForClassFields: true`, and Milkdown has no Lit binding.
- **Vue islands forever**, with only touched surfaces migrating. Rejected. It leaves two paradigms in place permanently, and the most tangled surfaces (page navigation, Settings) are exactly the ones that would stay vanilla.
