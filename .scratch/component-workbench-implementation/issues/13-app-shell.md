# 13: App shell

**What to build:** The whole frontend is one Vue app. `App.vue` holds the `vaultView` switch, the workspace layout and sidebar chrome (collapse rail, compact drawer, `COMPACT_MEDIA_QUERY`), and the modal host. `main.ts` shrinks to a bootstrap (or nothing) and the static surface markup is gone. See [spec: Step 12](../../component-workbench/spec.md#step-12-app-shell).

**Blocked by:** 12 (Settings)

**Status:** ready-for-agent

- [ ] One `createApp`; every `mountIsland` call replaced by the template, and `mountIsland` removed
- [ ] Remaining `main.ts` reduced to a bootstrap or removed
- [ ] `index.html` surface markup removed
- [ ] Global shortcuts (Ctrl/Cmd+K, Ctrl/Cmd+, , Escape) call `ui` actions without DOM wiring
- [ ] Dead `#greet-input` rule removed
- [ ] If chrome is split into a presentational SFC, stories: expanded desktop, collapsed rail, compact with drawer closed, compact with drawer open
- [ ] **Done when:** no temporary callback root props remain
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
