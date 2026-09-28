# 04: Vault picker

**What to build:** The vault picker becomes a Vue surface, backed by a vault state module whose open action does everything a vault open needs (including refreshing sync status) on every path. See [spec: Step 3](../../component-workbench/spec.md#step-3-vault-picker).

**Blocked by:** 03 (Search modal)

**Status:** ready-for-agent

- [ ] `state/vault.ts`: vault path, open/closed, and the open action (refreshes sync status); replaces the vault-path text writes into Settings
- [ ] `ui.vaultView: 'picker' | 'cloneWizard' | 'cloneManual' | 'workspace'` introduced
- [ ] The container may be a few inline lines in the parent
- [ ] Temporary callbacks: open the clone wizard, open the clone manual form, page/trash loads after an open
- [ ] Stories: default; translated nested-repo error; raw error (including the Android "no folder picker" rejection)
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
