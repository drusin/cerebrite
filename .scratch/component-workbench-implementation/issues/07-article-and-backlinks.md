# 07: Article + backlinks

**What to build:** The article view — title row with rename and delete, the trash banner with Restore, the editor inside, and the backlinks — becomes a Vue surface. Rename, delete, and restore are confirmed in the container via the dialogs module; heading-targeted opens scroll to the heading. Backlinks are the **first copy** of a page list, written inline. See [spec: Step 6](../../component-workbench/spec.md#step-6-article--backlinks).

**Blocked by:** 06 (Page editor)

**Status:** ready-for-agent

- [ ] Surface emits rename/delete/restore intents; container confirms and calls page actions
- [ ] Heading-targeted open uses the editor's `scrollToHeading`
- [ ] Backlinks rendered inline (no shared component yet)
- [ ] `#rename-page-button` and `#delete-page-button` become classes
- [ ] Stories: nothing selected; persisted page (rename and delete visible); dynamic page (no rename/delete, empty body); in-trash page (banner and Restore, no delete); backlinks empty; backlinks grouped by source, with and without a `→ Heading` label
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
