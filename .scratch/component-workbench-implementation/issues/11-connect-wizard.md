# 11: Connect wizard + extract DeviceFlow, SshKey, CommitAs, Modal

**What to build:** The connect wizard becomes a Vue modal driven by its reducer; closing it returns to Settings. These are the **second copies** of device flow, SSH key, Commit as, and a modal shell, so each is extracted as a shared component with stories, and the clone wizard, clone manual form, and search switch to them. See [spec: Step 10](../../component-workbench/spec.md#step-10-connect-wizard) and [Shared components](../../component-workbench/spec.md#shared-components).

**Blocked by:** 10 (Clone wizard + clone manual form)

**Status:** ready-for-agent

- [ ] `useConnectWizard(api)` runs `reduceWizard` and its effects (device-flow polling, repo lists, connect); the SFC takes `state: WizardState` and emits the reducer's events
- [ ] `ui.modal = 'connectWizard'`; closing returns to `'settings'`
- [ ] `DeviceFlow`, `SshKey` (generate/import), and `CommitAs` extracted into `src/components/` with stories; clone wizard and clone manual form switch to them
- [ ] `<Modal>` shell extracted with stories; search switches to it
- [ ] Temporary callbacks: switch to manual (→ `openSettings({ section: 'sync', prefillUrl })` for the still-vanilla Settings); refresh Settings' Commit as fields
- [ ] Stories: `hasRepo`; `providerChoice` with and without "Another provider"; `credentialChoice`, `otherCredentialKindChoice`; `oauthSignIn` requesting / code shown / error; `repoVisibility` incl. create error; `repoPicker` loading / empty / list / error; `pasteUrl` token and SSH with validation errors; `connecting`, `connectError`; `commitAuthor` prefilled / error
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
