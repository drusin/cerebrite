# 10: Clone wizard + clone manual form

**What to build:** The clone wizard and the clone manual form become Vue surfaces driven by their reducers, shown via `ui.vaultView` instead of hiding/showing the picker directly. Device flow, SSH key, and Commit as are written **inline** here (first copies). Opening a vault from the manual form now refreshes sync status, because it goes through the vault open action. See [spec: Step 9](../../component-workbench/spec.md#step-9-clone-wizard--clone-manual-form).

**Blocked by:** 09 (Trash)

**Status:** ready-for-agent

- [ ] `useCloneWizard(api)` runs `reduceCloneWizard` and carries out its effects (device-flow polling, repo lists, clone); the SFC takes `state: CloneWizardState` and emits the reducer's events
- [ ] Clone manual form uses the same surface/container split
- [ ] `ui.vaultView` covers `'cloneWizard'` and `'cloneManual'`; the vault picker's clone callbacks from ticket 04 are deleted
- [ ] Wizard/manual-form variables and `pendingGithubTokenPair` become local, not in `src/state/`
- [ ] Clone manual form's missing `refreshSyncStatus` is fixed via the vault open action
- [ ] Stories, clone wizard (one reducer state each, no backend): `providerChoice`, `credentialChoice`, `otherCredentialKindChoice`, `oauthSignIn`, `repoPicker`, `pasteUrl` (token and SSH), `destinationPicker`, `cloning`, `cloneError`, `commitAuthor`
- [ ] Stories, clone manual form: each of the four sub-forms; pre-filled from the wizard; validation errors; device code shown; "Signed in. Click Clone to continue."; cloning; clone error
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
