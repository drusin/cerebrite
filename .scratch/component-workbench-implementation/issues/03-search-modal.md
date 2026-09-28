# 03: Search modal

**What to build:** The search modal becomes a Vue surface. Ctrl/Cmd+K opens it, the input is focused on open, results and "Create page" behave as today. It is the **first modal**, so its overlay is written inline — no shared `<Modal>` yet. See [spec: Step 2](../../component-workbench/spec.md#step-2-search-modal).

**Blocked by:** 02 (Sync indicator + popup)

**Status:** ready-for-agent

- [ ] `ui.modal = 'search'`; Ctrl/Cmd+K calls the open action
- [ ] The search variables become local to the surface/container (not in `src/state/`)
- [ ] Input focus via `defineExpose` (a stateless one-shot effect only)
- [ ] Emits `close` on Escape and backdrop click
- [ ] Temporary callbacks: open a page by title; create a page from the query then open it
- [ ] Stories: empty query ("Type to search."); no results (only "Create page: '…'"); tier 1 title hit; tier 2 hit with a `#tag` chip; tier 3 hit with a `<mark>` snippet; in-trash result with badge; include-trash on and off; keyboard selection moved; exact title match (no Create row)
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
