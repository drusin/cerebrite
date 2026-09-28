# 09: Trash

**What to build:** The Trash list with Empty trash becomes a Vue surface reusing `PageList`. Empty trash is confirmed in the container. See [spec: Step 8](../../component-workbench/spec.md#step-8-trash).

**Blocked by:** 08 (Sidebar page list + Recent)

**Status:** ready-for-agent

- [x] Reuses `PageList`
- [x] Empty trash confirmed via the dialogs module in the container, then calls the page action
- [x] `#empty-trash-button` becomes a class
- [x] Stories: empty, populated
- [x] Presentational SFC(s) never import `vault-api`, `dialogs.ts`, or a `src/state/` module, and mount from props alone
- [x] Container (composable or `XContainer.vue`) owns state reads, actions, backend and dialog calls; no stories for it
- [x] One named CSF3 story per interesting state, colocated with the SFC (no MDX, docs pages, or autodocs)
- [x] No `<style>` block; the SFC renders the existing `styles.css` class names
- [x] The `index.html` markup and `main.ts` code this step replaced are removed
- [x] Escape / document listeners for the surface moved into it (n/a: Trash has no overlay/Escape handling of its own)
- [x] Temporary callback root props replaced by this step are deleted and named in the commit message; any new ones are named too (none either way: Trash was mounted with no root props before or after)
- [x] `npm run build` (`vue-tsc --noEmit && vite build`) passes
- [x] `storybook build` passes
- [ ] A manual run of the real Tauri app covers the flows this step touched (see [debugging in a sandbox](../../../docs/agents/debugging-sandbox.md)) -- skipped per instructions, deferred to one pass at the end of the chain
