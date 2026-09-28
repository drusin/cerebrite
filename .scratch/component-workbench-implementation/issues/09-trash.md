# 09: Trash

**What to build:** The Trash list with Empty trash becomes a Vue surface reusing `PageList`. Empty trash is confirmed in the container. See [spec: Step 8](../../component-workbench/spec.md#step-8-trash).

**Blocked by:** 08 (Sidebar page list + Recent)

**Status:** ready-for-agent

- [ ] Reuses `PageList`
- [ ] Empty trash confirmed via the dialogs module in the container, then calls the page action
- [ ] `#empty-trash-button` becomes a class
- [ ] Stories: empty, populated
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
