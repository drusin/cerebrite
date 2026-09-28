# 06: Page editor wrapper + lost-edit fix

**What to build:** The Milkdown editor is hosted by a thin Vue SFC wrapping the unchanged `PageEditor`, with a container that autosaves. Edits made in the last ~200 ms before switching page or unmounting are **no longer lost**. See [spec: Editor wrapper and commit](../../component-workbench/spec.md#editor-wrapper-and-commit) and [Step 5](../../component-workbench/spec.md#step-5-page-editor). A prototype lives on branch `prototype/editor-in-vue` at `909c4ea` (variant A).

**Blocked by:** 05 (Page state module)

**Status:** ready-for-agent

- [ ] Wraps `PageEditor` (unchanged); does not adopt `@milkdown/vue`
- [ ] `onMounted` constructs `new PageEditor(root, onChange, onLinkClick)` and calls `load(markdown)`; `onUnmounted` calls `destroy()`
- [ ] Props `pageKey` and `markdown`; only a `pageKey` change reloads — a `markdown`-only change does not (avoids cursor reset from echoed changes)
- [ ] Emits `change(pageKey, markdown)`, `linkClick(title)`, and `commit(oldPageKey, markdown)`
- [ ] Before reloading on a `pageKey` change and in `onBeforeUnmount`, the SFC reads `getMarkdown()` and emits `commit` if it differs from the last emitted value
- [ ] Container keeps the 1500 ms autosave debounce for `change` and saves `commit` immediately, through the page save action
- [ ] `defineExpose` exposes only `scrollToHeading(slug)`; `getMarkdown` is not exposed
- [ ] `saveTimer`, `pageEditor`, `scheduleAutosave`, `flushSave`, and `flushPendingSaveForCurrentPage` removed from `main.ts`
- [ ] Manual check: type, then switch page within ~200 ms — the edit is saved
- [ ] Stories (`change` / `linkClick` / `commit` as actions): Plain prose (headings, bold, italic, inline code, list, no links); Wiki-links (page link, heading on another page, not-yet-existing page, heading on same page); Long document (~12 sections)
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
