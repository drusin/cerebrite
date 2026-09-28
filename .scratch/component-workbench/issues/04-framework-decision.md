# Framework or not: how surfaces are built

Type: grilling
Status: resolved
Blocked by: 02, 03

## Question

Given the shape options (ticket 02) and the coupling picture (ticket 03), are surfaces built as plain TS render modules, web components, or Vue 3 components?

The standing lean is "no hard opinion, Vue 3 if a framework". The decision must weigh the migration cost against how much simpler isolation, stories, and future tests become. This is hard to reverse and likely ADR-worthy.

## Answer

**Surfaces are built as Vue 3 components.** Recorded as [ADR-0014](../../../docs/adr/0014-vue-3-for-frontend-surfaces.md).

- **End state: one Vue app.** Surfaces migrate one at a time as Vue islands (`createApp(...).mount(...)` into the existing shell), so the app keeps working after every step. Islands are only the migration mechanism. Once enough surfaces have moved, they collapse into a single app, and `main.ts` shrinks to a bootstrap or disappears. "Islands forever" was rejected because it would leave two paradigms in place for good, with the tangled page-navigation cluster and Settings still hand-patched.
- **Conventions:**
  - `.vue` SFCs with `<script setup lang="ts">`. Composition API only, no Options API.
  - Typed `defineProps<…>()` and `defineEmits<…>()`.
  - **No `<style>` blocks.** Styling stays in the global `src/styles.css`.
  - The build adds `@vitejs/plugin-vue`, and `vue-tsc --noEmit` replaces `tsc` in `build`.
- **Styling stays global, deliberately.** This was re-checked during this grilling, and global styles still keep the scope small. `styles.css` is class-based and already organised by surface prefix (`.clone-wizard-*`, `.settings-*`, `.search-*`, `.sync-popup`, `.page-list`, …). Theming is `:root` custom properties plus `[data-theme]`. A component that renders the same class names gets its styling with no CSS work, and scoping would be a second migration of its own (Milkdown's ProseMirror DOM would also need `:deep()`). The friction left is small:
  - The five id selectors (`#new-page-button`, `#rename-page-button`, `#delete-page-button`, `#empty-trash-button`, and the dead `#greet-input`) **become classes when their surface migrates**.
  - About 13 layout rules only apply under `.workspace.compact` or `.workspace.sidebar-collapsed`, so sidebar stories need a `.workspace` wrapper.
  - Scoped CSS can still be adopted later, one component at a time.
- **Version policy:** `vue ^3.5`, currently 3.5.43. Move to 3.6 once it is stable. Never run a release candidate.
- **Rejected:**
  - **Lit:** the global stylesheet doesn't reach its shadow DOM, the opt-out (`createRenderRoot`) gives up most of what Lit adds, it clashes with `useDefineForClassFields: true`, and there is no Milkdown binding.
  - **Plain TS render modules:** zero cost, but every `update()` is hand-patched, which is what made `main.ts` complex. The repeated patterns (device flow ×5, SSH ×4, Commit as ×3, page lists ×4) want shared components with automatic re-render.
- **Left open:** how the editor integrates with Vue (a thin wrapper around `PageEditor`, or `@milkdown/vue`) moved to a prototype ticket, [Editor in Vue: wrap `PageEditor` or adopt `@milkdown/vue`](06-editor-in-vue.md). The surface contract (whether a framework-neutral handle stays, where shared state lives, and how islands merge) is [Surface contract](07-surface-contract.md).
