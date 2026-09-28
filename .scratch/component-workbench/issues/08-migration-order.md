# Pilot surface and migration order

Type: grilling
Status: resolved
Blocked by: 07

## Question

Given the [Surface contract](07-surface-contract.md), which surface is the pilot, and in what order do the rest migrate out of `main.ts`, keeping the app working after every step?

- **Pilot:** the sync indicator + popup is the leading candidate. Its derivation `syncIndicatorFor` is pure, and after the contract only its anchor positioning is left to cut. Search and the vault picker are similarly isolated. The editor is isolated but a leaf. Decide which proves the most about the contract (surface/container split, a state module, `mountIsland`, callback root props, stories) for the least risk.
- **Order:** where the **app shell** step (the islands merge into one `App.vue`) falls, how the tangled page-navigation cluster (page list, Recent, Trash, article + backlinks, editor glue) and the Settings hub are approached, and whether they are split into smaller steps.
- **Grouping by shared components:** how the extract-on-second-copy rule (device flow, SSH key, Commit as, credential-kind, page list, `<Modal>`) shapes which surfaces go next to each other.
- **Per-step definition of done:** the Vue SFC plus its stories, the container, the state it moved out of `main.ts`, and the callback root props it deleted. Also any per-step checks (`vue-tsc`, `storybook build`, a manual run of the real app).

Background: the coupling summary in the [surface inventory](../research/03-surface-inventory.md).

## Answer

**Thirteen small steps. The easiest, most isolated surfaces go first, and the app shell goes last. Intermediate states are never rolled out: the incremental shape exists to keep diffs small and bugs local.**

- **Why "works after every step" stays:** it is kept as a **verification discipline, not a rollout requirement**. Nothing ships between steps, and the user reviews only the finished migration. But each step must be checkable in the real app, so that a bug shows up in that step's small diff and not somewhere in a rewrite of the whole of `main.ts`. The spec states it this way.
- **Ordering rule:** each step's dependencies (state modules, save actions, shared components) must already exist, and the most isolated surfaces go first.
- **App shell last** (this reverses an early-middle placement that was considered). A mid-migration shell would have to host the vanilla DOM still left inside `App.vue`, and `main.ts` looks up its DOM elements once at startup. That is the most awkward thing the migration could do. With the shell last, every surface is already a Vue island and all shared state is in `src/state/`. The shell step is then mostly composition: the `mountIsland` calls are replaced with a template, and the rest of `main.ts` and the `index.html` markup are deleted.
- **Pilot: the sync indicator + popup.** It is the most isolated surface that uses real shared state. Its pure `syncIndicatorFor` generates the stories, and it tests the `anchor` prop, a container, and a deep-link action (`openSettings({section:'sync', …})`). That action reaches the still-vanilla Settings as a temporary callback root prop.

| # | Step | Why here |
|---|---|---|
| 0 | **Foundation**: Vue `^3.5`, `@vitejs/plugin-vue`, `vue-tsc` replacing `tsc` in `build`, `mountIsland`, `src/dialogs.ts` (all native dialog calls moved behind it), and the Storybook config (themes addon, viewports, CI `storybook build`) | Tooling only, with no behaviour change. Storybook's config can land here or with the pilot's first story, whichever lets the build pass |
| 1 | **Sync indicator + popup**, plus `state/sync.ts` and the first cut of `state/ui.ts` | The pilot, see above |
| 2 | **Search modal** | Private state, two intents, one query. The first modal, written inline |
| 3 | **Vault picker**, plus `state/vault.ts` and `ui.vaultView` | Trivial. It sets up the vault state that later steps need |
| 4 | **Page state only**: `state/pages.ts` (page and trash lists, Recent, open page, and the rename/delete/restore/save actions that refresh after they run). No UI change | Removes the ad-hoc `loadPages()`/`loadTrash()` signals before any navigation UI moves |
| 5 | **Page editor**, with `commit(oldPageKey, markdown)` | Fixes the lost-edit bug. Needs step 4's save action |
| 6 | **Article + backlinks** | Backlinks are the first copy of the page list, written inline |
| 7 | **Sidebar page list + Recent** | The second copy, so `PageList` is extracted into `src/components/` |
| 8 | **Trash** | Reuses `PageList` |
| 9 | **Clone wizard + clone manual form**, with `useCloneWizard` | The least entangled of the backend-heavy surfaces, since they show up before a vault is open. Device flow, SSH key and Commit as are written inline |
| 10 | **Connect wizard**, with `useConnectWizard` | The second copies, so `DeviceFlow`, `SshKey` and `CommitAs` are extracted. The second modal, so `<Modal>` is extracted |
| 11 | **Settings**, in one step | By now its sub-parts are shared components, so what's left is mostly layout. The second credential-kind form, so `CredentialKindForm` is extracted. Change-folder becomes a `vault` action that resets page state |
| 12 | **App shell**: `App.vue` (the `vaultView` switch, workspace layout and sidebar chrome, modal host), a single `createApp`, and the rest of `main.ts` and the `index.html` markup deleted | Only composition is left |

- **Per-step definition of done:**
  - the presentational SFC(s), with **one story per interesting state**
  - the container
  - the state moved into `src/state/`
  - the temporary callback root props **deleted** (named in the commit message)
  - the `index.html` markup and `main.ts` code removed
  - any Escape or document listeners moved into the surface
- **Per-step checks:** `vue-tsc` + `vite build`, `storybook build`, and a manual run of the real Tauri app covering the flows the step touched (`docs/agents/debugging-sandbox.md`).
- **Incidental fixes, made in the step that touches them:**
  - Step 9: the clone manual form's missing `refreshSyncStatus` goes away, because the `vault` open action does it for every open path.
  - Step 11: the unused `confirmSshHostKey` is deleted.
