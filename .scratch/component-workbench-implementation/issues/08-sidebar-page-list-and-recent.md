# 08: Sidebar page list + Recent, extract PageList

**What to build:** "All pages" (with New page and Today) and Recent become Vue surfaces. The active item comes from state. This is the **second copy** of a page list, so `PageList` is extracted as a shared component with its own stories and backlinks switch to it. See [spec: Step 7](../../component-workbench/spec.md#step-7-sidebar-page-list--recent) and [Shared components](../../component-workbench/spec.md#shared-components).

**Blocked by:** 07 (Article + backlinks)

**Status:** ready-for-agent

- [ ] `PageList` extracted into `src/components/` with its own stories; backlinks use it
- [ ] `highlightActivePage` removed
- [ ] `#new-page-button` becomes a class
- [ ] Sidebar stories use a `.workspace` wrapper decorator (layout-dependent rules)
- [ ] Stories — page list: empty, populated, one active page, a long list; Recent: empty ("No pages opened yet."), populated with persisted and dynamic entries, an active entry
- [ ] Presentational SFC(s) never import `vault-api`, `dialogs.ts`, or a `src/state/` module, and mount from props alone
- [ ] Container (composable or `XContainer.vue`) owns state reads, actions, backend and dialog calls; no stories for it
- [ ] One named CSF3 story per interesting state, colocated with the SFC (no MDX, docs pages, or autodocs)
- [ ] No `<style>` block; the SFC renders the existing `styles.css` class names
- [ ] The `index.html` markup and `main.ts` code this step replaced are removed
- [ ] Escape / document listeners for the surface moved into it
- [ ] Temporary callback root props replaced by this step are deleted and named in the commit message; any new ones are named too
- [ ] `npm run build` (`vue-tsc --noEmit && vite build`) passes
- [ ] `storybook build` passes
- [ ] A manual run of the real Tauri app covers the flows this step touched (see [debugging in a sandbox](../../../docs/agents/debugging-sandbox.md))
