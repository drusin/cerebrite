# 12: Settings + extract CredentialKindForm

**What to build:** Settings becomes a Vue modal in one step, handling its own deep links. The theme moves into state, and Change folder becomes a vault action that resets page state. This is the second credential-kind form, so `CredentialKindForm` is extracted and the clone manual form switches to it. See [spec: Step 11](../../component-workbench/spec.md#step-11-settings).

**Blocked by:** 11 (Connect wizard)

**Status:** ready-for-agent

- [ ] Sections: vault folder; color scheme (theme in `src/state/`); Commit as; Connect a repository; Sync (not-connected banner or Disconnect, four raw sub-forms all going through the plaintext-consent confirm in the container); Credentials (orphan notice, cleanup, remove all)
- [ ] `ui.modal = 'settings'`; Ctrl/Cmd+, opens it
- [ ] Change folder is a vault action that resets page state
- [ ] `openSettings({ section, prefillUrl, credentialKind })` handled by the Vue Settings; the Settings callbacks from tickets 02 and 11 deleted
- [ ] `CredentialKindForm` extracted into `src/components/` with stories; clone manual form uses it
- [ ] `settingsSshKey` becomes local
- [ ] Unused `confirmSshHostKey` deleted
- [ ] Stories: no vault path; Commit as prefilled / saved / saved with warning / error; not-connected banner vs Disconnect; each sub-form visible with its status text ("Connecting…", "Connected.", an error); GitHub device code shown and the GitHub install CTA; orphan notice shown and hidden; remove-all fully succeeded and partially failed
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
