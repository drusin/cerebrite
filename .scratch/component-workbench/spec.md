# Spec: component workbench

Handoff spec from the [component-workbench map](map.md). It brings together decisions made in that map's tickets and makes none of its own. Each section links the ticket that holds the reasoning. When this spec and a ticket disagree, the ticket wins, and this spec should be fixed.

## Goal

Split the frontend into isolated **surfaces**, each browsable in its interesting states in a dev-only Storybook. The frontend today is vanilla TypeScript: `src/main.ts` (~3.8k lines) patches the markup in `index.html` by hand. The work moves it to Vue 3 one surface at a time, over the 13 steps in [Migration](#migration). It ends with one Vue app, and `main.ts` and the static markup are gone.

The implementation map should turn **each migration step into one ticket**. Step sections are written to be copied into tickets.

## Standing constraints

- **Surface granularity.** One showable element is one surface: the page list, the article with its backlinks, the editor, search, the sync indicator and popup, Settings, the connect wizard, the clone wizard, the clone manual form, the vault picker, and Trash. Each is shown in its interesting states. There is no atomic widget library. Shared sub-components exist only where the extract-on-second-copy rule creates them ([Shared components](#shared-components)).
- **Manual browsing is the goal. Automated tests are designed for, not built.** Every surface must mount from props alone, so isolated interaction tests can be added later. Writing those tests is not part of this work.
- **As little backend as possible.** Stories need no fake `vault-api` ([Surface contract](#surface-contract)).
- **Incremental, with a working app after every step.** This is a **verification discipline, not a rollout requirement.** Nothing ships between steps, and the maintainer reviews the finished migration. Each step still has to be checkable in the real app, so a bug shows up inside that step's small diff and not somewhere in a rewrite of all of `main.ts` ([Pilot surface and migration order](issues/08-migration-order.md)).
- **Styling stays global.** No SFC has a `<style>` block, and all styling stays in `src/styles.css` ([ADR-0014](../../docs/adr/0014-vue-3-for-frontend-surfaces.md)).
- **Dev-only workbench.** Storybook runs in a normal desktop browser from its own npm script. It is never part of the Tauri bundle and is never deployed.
- **No dependency may be end-of-life, deprecated, or abandoned.**

## Tooling

### Vue 3

Decided in [Framework or not](issues/04-framework-decision.md) and recorded as [ADR-0014](../../docs/adr/0014-vue-3-for-frontend-surfaces.md).

- **Dependencies:** `vue` at `^3.5`. Move to 3.6 once it is stable, and never use a release candidate. Add `@vitejs/plugin-vue` to `vite.config`.
- **Type checking:** `vue-tsc --noEmit` replaces `tsc` in the `build` script, so it becomes `vue-tsc --noEmit && vite build`.
- **SFC style:** SFCs use `<script setup lang="ts">` with the Composition API only, no Options API. Props and emits are typed with `defineProps<…>()` and `defineEmits<…>()`.
- **No `<style>` blocks.** A migrated component renders the class names that already exist in `styles.css`, which is organised by surface prefix and themed through `:root` custom properties and `[data-theme]`.
- **Id selectors become classes when their surface migrates:**

  | Selector | Step |
  |---|---|
  | `#rename-page-button`, `#delete-page-button` | 6 |
  | `#new-page-button` | 7 |
  | `#empty-trash-button` | 8 |

  `#greet-input` is dead and belongs to no surface. It goes with the leftover cleanup in step 12.
- **Layout-dependent rules:** about 13 rules only apply under `.workspace.compact` or `.workspace.sidebar-collapsed`. Sidebar stories therefore need a `.workspace` wrapper decorator.

### Storybook

Decided in [Which workbench tool](issues/05-workbench-tool-decision.md). The tool comparison is in [Workbench tool landscape](issues/01-workbench-tool-landscape.md).

- **Packages:** `storybook` at `^10.6` and `@storybook/vue3-vite`, as devDependencies only. Also `@storybook/addon-themes`. The viewport is built into Storybook core.
- **Which surfaces get stories:** only surfaces that have migrated to Vue. There are no vanilla stories. A surface's stories are part of the definition of done for its step.
- **Story format:**
  - CSF3 `*.stories.ts`, colocated with the SFC.
  - One named story per interesting state. The states come from the [surface inventory](research/03-surface-inventory.md) and are listed per step below.
  - No MDX, no docs pages, and no autodocs.
- **`.storybook/preview.ts`:**
  - Imports `src/styles.css`.
  - Sets up `withThemeByDataAttribute` on `<html>`'s `data-theme` with three options: **System** (removes the attribute, so `prefers-color-scheme` applies, as `applyTheme` does), **Light**, and **Dark**.
  - Configures the viewport with a **phone** preset and a **tablet** preset next to the full-width default. `styles.css` has no width breakpoints to match, so standard device sizes are fine.
- **Where wrappers go:** a surface that needs a wrapper, such as the `.workspace` wrapper, gets it through that story file's decorators.
- **`.storybook/main.ts`:** set `core.disableTelemetry: true`.
- **Addons not added:** `addon-a11y`, `addon-docs`/autodocs, and `addon-vitest`. Vitest is not added either.
- **Scripts:**
  - `npm run storybook` starts the dev server.
  - `npm run build-storybook` (or an equivalent name) runs `storybook build` into a throwaway output directory, which is git-ignored.
- **CI:** `.github/workflows/ci.yml` adds a `storybook build` step next to `npm run build`. It is a compile check that catches stories broken as `main.ts` shrinks. Its output is never uploaded or deployed.
- **Keeping it out of the app:** nothing under `src/` that the app imports may import a `*.stories.ts` file or `@storybook/*`. That keeps Storybook out of `vite build` and so out of the Tauri bundle.
- **Fallback:** if Storybook's weight becomes a problem, the documented fallback is a hand-rolled Vite gallery page. Histoire is ruled out.

**Watch item for the future test effort (not this one):** the interaction-test path is `play` functions plus `@storybook/addon-vitest` in Vitest browser mode. `addon-vitest@10.x` accepts only Vitest `^3 || ^4`, and Vitest 5 needs Storybook 11. That effort chooses between pinning Vitest 4.1.x and moving to Storybook 11 stable.

## Surface contract

Decided in [Surface contract](issues/07-surface-contract.md).

### Two layers per surface

- **Presentational surface.** This is the `X.vue` that gets stories.
  - Plain data comes in as props, and intents go out as emits ("open page", "sync now", "delete", "close").
  - It **never** imports `vault-api`, `dialogs.ts`, or a `src/state/` module.
  - It must be mountable from props alone.
- **Container.** This is either a `useX` composable or an `XContainer.vue`.
  - It reads state, calls actions, `vault-api`, and `dialogs.ts`, and feeds the surface.
  - Containers get no stories. Testing them is left to the future test effort.
  - For trivial surfaces (the vault picker, search), the container can be a few inline lines in the parent.
- **Wizards and the clone manual form.**
  - The SFC takes `state: WizardState` or `CloneWizardState` as a prop and emits the reducer's events.
  - `useConnectWizard(api)` and `useCloneWizard(api)` run `reduceWizard` / `reduceCloneWizard` and carry out the effects: device-flow polling, repo lists, connect, and clone.
  - Stories use one reducer state per step and no backend.
  - The clone manual form uses the same split.
- **Consequence:** no fake `vault-api` is needed anywhere. Only containers touch the backend, and containers are not storied.

### Shared state: `src/state/`

- **Shape:** plain `reactive()`/`ref()` modules that export read-only state plus **actions**. There is no Pinia and no vue-router. The action-module shape maps onto Pinia setup stores if Pinia is ever needed.
- **What lives here:** the truly shared state from the inventory:
  - the open page
  - Recent
  - the vault path and whether a vault is open
  - sync status
  - theme
  - the page list and the trash list
- **Actions re-fetch what they changed.** An action does the backend call and then re-fetches, for example `renamePage()` → `api.renamePage` → `refreshPages()`. This replaces the ad-hoc `loadPages()` / `loadTrash()` "changed" signals.
- **Migration:** while migration is in progress, `main.ts` imports the same modules.
- **Local state stays local.** State that is "accidentally global" today (the search variables, all `wizard*`, `cloneWizard*`, and `cloneManual*` variables, `pendingGithubTokenPair`, `settingsSshKey`, `saveTimer`, and the `pageEditor` instance) becomes local to its surface or container. It does not go into `src/state/`.

### View and modal state: `state/ui.ts`

- **View:** `vaultView: 'picker' | 'cloneWizard' | 'cloneManual' | 'workspace'`.
- **Modal:** `modal: null | 'search' | 'settings' | 'connectWizard'`.
  - The sync popup is tracked separately, because it is anchored rather than modal.
  - Closing the connect wizard returns to `'settings'`.
- **Deep links are action arguments,** for example `openSettings({ section: 'sync', prefillUrl, credentialKind })`.
  - This replaces `openSyncManualForm` and `openQuickReconnect` writing into Settings' DOM.
  - The Settings sync banner reads sync status from state. `applySyncIndicator` no longer writes into it.
- **Shortcuts:** the global shortcuts (Ctrl/Cmd+K, Ctrl/Cmd+, and Escape) call these actions.

### Islands and how they merge

- **`mountIsland`:** `mountIsland(el, Container, rootProps)` is a thin helper around `createApp(...).mount(el)`. There is no framework-neutral `{update, destroy}` handle.
- **Shared state** that has already moved into `src/state/` is shared by importing it.
- **Temporary callback root props.** Intents whose logic still lives in `main.ts` are passed to the island as temporary callback root props.
  - Each step moves the logic it needs into a state module and **deletes** the matching callbacks.
  - The callbacks are therefore the visible, shrinking list of what hasn't moved yet.
- **Merging:** before step 12, islands are separate apps, joined only through the state modules. Step 12 (the app shell) merges them into one `createApp`.

### Imperative escape hatches

`defineExpose` is allowed **only for stateless one-shot effects:** `scrollToHeading`, and focus (for example the search input when it opens). It is never used to read state back. Data leaves a surface only through emits.

### Native dialogs

- **Where they're called:** only from containers, through `src/dialogs.ts` (next to `vault-api.ts`). It wraps `@tauri-apps/plugin-dialog` (`confirm`, `message`) and `window.prompt` / `confirm` / `alert`.
- **How:** the surface emits the intent (for example `delete`), and the container confirms and then calls the action.
- **UX:** unchanged.
- **Surfaces with their own input UI:** surfaces that already collect input in their own UI (for example the wizards' paste-URL step) keep doing so.

### Overlays

- **Rendering:** a modal or popup surface renders its own overlay and backdrop, using the existing class names.
- **Closing:** it emits `close` on Escape or a backdrop click.
- **Escape handling** moves out of `init`'s document listeners and into each surface as that surface migrates.
- **Anchored popups** take an `anchor: DOMRect | null` prop instead of reading the anchor element. Stories pass a fixed rect.

### Shared components

**A shared component is extracted when a second copy migrates.** The first migration writes the UI inline. The second extracts it into `src/components/`, with stories of its own.

| Component | Copies today | Extracted in step |
|---|---|---|
| `PageList` | page list, Recent, Trash, backlinks | 7 |
| `DeviceFlow` | ×5 | 10 |
| `SshKey` (generate/import) | ×4 | 10 |
| `CommitAs` | ×3 | 10 |
| `<Modal>` shell | search, connect wizard, Settings, … | 10 (second modal) |
| `CredentialKindForm` | Settings Sync, clone manual | 11 |

### File layout

| Path | Contents |
|---|---|
| `src/surfaces/<surface>/` | `X.vue` (presentational), `X.stories.ts`, and `useX.ts` or `XContainer.vue` |
| `src/state/` | state modules |
| `src/components/` | shared components and their stories |
| `src/mount-island.ts`, `src/dialogs.ts` | helpers (exact names are the implementer's choice) |

The existing pure modules (`sync-status.ts`, `connect-wizard.ts`, `clone-wizard.ts`, `heading-slug.ts`) and `page-editor.ts` / `wiki-link-plugin.ts` stay where they are.

## Editor wrapper and `commit`

Decided in [Editor in Vue](issues/06-editor-in-vue.md), and the flush in [Surface contract](issues/07-surface-contract.md). The prototype is on branch `prototype/editor-in-vue` at `909c4ea` (variant A).

- **Wrap `PageEditor` in a thin SFC.** Do not adopt `@milkdown/vue`: it pins `@milkdown/kit` to an exact version, pulls in `@milkdown/crepe`, and would mean writing the `Editor.make()` chain again. `PageEditor` itself stays unchanged.
- **Lifecycle:** in `onMounted`, construct `new PageEditor(root, onChange, onLinkClick)` on the SFC's root element and call `load(markdown)`. In `onUnmounted`, call `destroy()`.
- **Props:** `pageKey` and `markdown`.
  - Only a change of **`pageKey`** reloads (`watch(pageKey)` → `load()`).
  - A change to `markdown` alone does not reload, because the parent echoing back the SFC's own `change` would reset the cursor.
- **Emits:**
  - `change(pageKey, markdown)`. Ordinary changes carry `pageKey`, because the parent's current page may already have moved on.
  - `linkClick(title)`.
  - `commit(oldPageKey, markdown)`: see below.
- **The final flush, which fixes the lost-edit bug:**
  - **The bug:** Milkdown's `listener` waits 200 ms before calling `markdownUpdated`, and `destroy()` cancels that pending call. `flushPendingSaveForCurrentPage` returns early when `saveTimer === null`. So an edit made in the last ~200 ms before a page switch is dropped today.
  - **The fix:** before reloading on a `pageKey` change, and in `onBeforeUnmount`, the SFC reads its own `PageEditor.getMarkdown()`. If that differs from the last value it emitted, it emits **`commit(oldPageKey, markdown)`**.
  - The container saves a `commit` **immediately**, with no debounce. Ordinary `change` events keep the 1500 ms autosave debounce, which lives in the container.
- **`defineExpose`:** only `scrollToHeading(slug)`. `getMarkdown` is **not** exposed, because data leaves only through emits.
- **Stories:** `markdown` goes in, and `change` / `linkClick` / `commit` show up as actions. There is one named story per sample document:
  - **Plain prose:** headings, bold, italic, inline code, and a list, with no links.
  - **Wiki-links:** a page link, a link to a heading on another page, a link to a page that doesn't exist yet, and a link to a heading on the same page.
  - **Long document:** about a dozen sections, for scroll-to-heading.

## Migration

Decided in [Pilot surface and migration order](issues/08-migration-order.md). Background on each surface is in the [surface inventory](research/03-surface-inventory.md).

### Rules for every step

- **Order:** steps run **in order**, and each is blocked by the previous one. The ordering rule is that each step's dependencies (state modules, save actions, shared components) already exist, with the most isolated surfaces first.
- **The app shell goes last.** A mid-migration shell would have to host vanilla DOM inside `App.vue`, while `main.ts` looks up its elements once at startup.

**Definition of done (each step):**

- [ ] The presentational SFC(s), with **one story per interesting state**
- [ ] The container
- [ ] The state the step needs, moved into `src/state/`
- [ ] The temporary callback root props it replaced **deleted**, and named in the commit message
- [ ] The `index.html` markup and `main.ts` code it replaced removed
- [ ] Any Escape or document listeners for the surface moved into it
- [ ] The id selectors for the surface turned into classes (see [Vue 3](#vue-3))

**Checks (each step):**

- [ ] `vue-tsc --noEmit` and `vite build` pass (`npm run build`)
- [ ] `storybook build` passes
- [ ] A manual run of the real Tauri app covers the flows the step touched ([docs/agents/debugging-sandbox.md](../../docs/agents/debugging-sandbox.md))

The callbacks listed under each step below are **expected** ones, read from the inventory's cross-surface notes. The step itself names the real ones in its commit.

### Step 0: Foundation

Tooling only, with no behaviour change.

- Add Vue `^3.5` and `@vitejs/plugin-vue`, and replace `tsc` with `vue-tsc --noEmit` in `build`.
- Add the `mountIsland` helper.
- Add `src/dialogs.ts`, and route **every** native dialog call through it: the `plugin-dialog` `confirm`/`message` calls imported directly in `main.ts`, plus every `window.prompt` / `confirm` / `alert`.
- Add the Storybook config: `@storybook/vue3-vite`, `preview.ts` with `styles.css`, themes, and viewports, telemetry off, the npm scripts, and the CI `storybook build` step. This config can land here or with step 1's first story, whichever lets `storybook build` pass.
- **Stories:** none.

### Step 1: Sync indicator + popup (pilot)

The most isolated surface that uses real shared state. It exercises the surface/container split, a state module, `mountIsland`, the `anchor` prop, a deep-link action, and callback root props.

- **New state:**
  - `state/sync.ts`: status, the `onSyncStatusChanged` subscription, and refresh. It replaces `currentSyncStatus` and `applySyncIndicator`.
  - The first cut of `state/ui.ts`: `openSettings({ section, prefillUrl, credentialKind })`, and the open/closed state of the sync popup.
- **Surfaces:** the indicator (the sidebar footer button and the rail icon) and the popup.
  - The popup takes `anchor: DOMRect | null` and emits `close` on Escape and on an outside click. This replaces the document listeners in `init`.
  - The glyph, text, and CTAs come from `syncIndicatorFor` (pure).
- **Couplings cut:**
  - Settings' sync banner reads `state/sync.ts` instead of being written by `applySyncIndicator`.
  - The "Sync settings…", reconnect, and store-as-plaintext paths call `openSettings(…)` instead of `openSyncManualForm` / `openQuickReconnect` writing Settings' DOM.
- **Expected temporary callbacks:** the Settings side of `openSettings` (Settings is still vanilla).
- **Stories:**
  - one per icon state: `notConnected`, `syncing`, `synced`, `retrying`, `needsAttention`, and the `warning` severity (`refreshedSignInNotSaved`)
  - the popup with and without provider / last-synced
  - one per `SyncFailureCause` variant (11), showing its text and its 0–2 CTAs

### Step 2: Search modal

- **Why here:** its state is private, it has two intents and one query, and it is the **first modal**, written inline (no `<Modal>` yet).
- **State:** `ui.modal = 'search'`. Ctrl/Cmd+K calls the open action. The search variables become local.
- **Focus:** the input is focused through `defineExpose`.
- **Expected temporary callbacks:** open a page by title, and create a page from the query then open it (page logic moves in step 4).
- **Stories:**
  - empty query ("Type to search.")
  - no results (only "Create page: '…'")
  - tier 1 title hit
  - tier 2 hit with a `#tag` chip
  - tier 3 hit with a `<mark>` snippet
  - an in-trash result with its badge
  - include-trash on and off
  - keyboard selection moved
  - an exact title match (no Create row)

### Step 3: Vault picker

- **Why here:** it is trivial, and it sets up the vault state later steps need.
- **New state:** `state/vault.ts` (vault path, open/closed, and the open action) plus `ui.vaultView`.
  - The open action does everything a vault open needs, including refreshing sync status.
  - It replaces the vault-path text writes into Settings.
- **Expected temporary callbacks:** open the clone wizard, open the clone manual form, and the page/trash loads after an open (until step 4).
- **Stories:**
  - default
  - with the translated nested-repo error
  - with a raw error (including the Android "no folder picker" rejection)

### Step 4: Page state only

- **Why here:** it removes the ad-hoc refresh signals before any navigation UI moves.
- **New state:** `state/pages.ts`:
  - the page list and trash list
  - Recent (cap 10)
  - the open page (persisted or dynamic)
  - actions for open, rename, delete, restore, empty trash, create, save, and materialize (dynamic → persisted), each re-fetching what it changed
- **Removed:** the `loadPages()` / `loadTrash()` call sites. The five paths that mutated `recentPages` go through the actions instead.
- **UI change:** none. `main.ts`'s rendering reads and calls the module.
- **Callbacks:** delete those from steps 2–3 that page actions now cover.
- **Stories:** none (there is no surface).

### Step 5: Page editor

- **Scope:** the wrapper from [Editor wrapper and `commit`](#editor-wrapper-and-commit), and its container. The container holds the 1500 ms autosave debounce plus the immediate save on `commit`, through step 4's save action.
- **Fixes:** the lost-edit bug.
- **Removed:** `saveTimer`, `pageEditor`, `scheduleAutosave`, `flushSave`, and `flushPendingSaveForCurrentPage` leave `main.ts`.
- **Stories:** plain prose, wiki-links, long document.

### Step 6: Article + backlinks

- **Scope:**
  - The title row with rename and delete, the trash banner with Restore, and the backlinks. The editor sits inside.
  - Rename, delete, and restore are confirmed through `dialogs.ts` in the container.
  - The heading-targeted open uses `scrollToHeading`.
- **Backlinks:** the **first copy** of the page list, written inline.
- **Id selectors:** `#rename-page-button` and `#delete-page-button` become classes.
- **Stories:**
  - nothing selected
  - a persisted page (rename and delete visible)
  - a dynamic page (no rename or delete, empty body)
  - an in-trash page (banner and Restore, no delete)
  - backlinks empty
  - backlinks grouped by source, with and without a `→ Heading` label

### Step 7: Sidebar page list + Recent

- **Scope:** "All pages" with New page and Today, and Recent.
- **Shared component:** the **second copy** of the page list, so `PageList` is extracted into `src/components/` with its own stories, and backlinks switch to it.
- **Removed:** `highlightActivePage` goes, because the active item comes from state.
- **Id selectors:** `#new-page-button` becomes a class.
- **Story wrapper:** sidebar stories use a `.workspace` decorator.
- **Stories:**
  - page list: empty, populated, one active page, a long list
  - Recent: empty ("No pages opened yet."), populated with persisted and dynamic entries, an active entry

### Step 8: Trash

- **Scope:** the Trash list with Empty trash, reusing `PageList`. Empty trash is confirmed in the container.
- **Id selectors:** `#empty-trash-button` becomes a class.
- **Stories:** empty, populated.

### Step 9: Clone wizard + clone manual form

- **Why here:** the least entangled of the backend-heavy surfaces, since they appear before a vault is open.
- **Wizard:** `useCloneWizard(api)` runs `reduceCloneWizard` and its effects. The SFC takes `CloneWizardState`.
- **Manual form:** same split.
- **Views:** `ui.vaultView` covers `'cloneWizard'` and `'cloneManual'`, replacing the direct hide/show of `#vault-picker`.
- **Inline for now:** device flow, SSH key, and Commit as are written **inline** (first copies).
- **Incidental fix:** the clone manual form's missing `refreshSyncStatus` goes away, because the vault open action from step 3 handles it on every path.
- **Stories, clone wizard:** one per reducer step:
  - `providerChoice`, `credentialChoice`, `otherCredentialKindChoice`
  - `oauthSignIn`
  - `repoPicker`
  - `pasteUrl` (token and SSH)
  - `destinationPicker`
  - `cloning`, `cloneError`
  - `commitAuthor`
- **Stories, clone manual form:**
  - each of the four sub-forms
  - pre-filled from the wizard
  - validation errors
  - device code shown
  - "Signed in. Click Clone to continue."
  - cloning
  - clone error

### Step 10: Connect wizard

- **Scope:** `useConnectWizard(api)` runs `reduceWizard` and its effects. The SFC takes `WizardState`, and `ui.modal = 'connectWizard'` returns to `'settings'` when it closes.
- **Extractions:** these are the **second copies**, so `DeviceFlow`, `SshKey`, and `CommitAs` are extracted into `src/components/` (with stories), and the clone wizard and clone manual form switch to them. This is also the **second modal**, so `<Modal>` is extracted and search switches to it.
- **Expected temporary callbacks:** switch to manual (→ `openSettings({ section: 'sync', prefillUrl })` for the still-vanilla Settings), and refresh Settings' Commit as fields.
- **Stories:** one per reducer step and sub-state:
  - `hasRepo`
  - `providerChoice`, with and without "Another provider"
  - `credentialChoice`, `otherCredentialKindChoice`
  - `oauthSignIn`: requesting, code shown, error
  - `repoVisibility`, including the create error
  - `repoPicker`: loading, empty, list, error
  - `pasteUrl`: token and SSH, with validation errors
  - `connecting`, `connectError`
  - `commitAuthor`: prefilled, error

### Step 11: Settings

- **Scope:** Settings in one step. By this point, most of what's left is layout.
- **Extractions:** this is the second credential-kind form, so `CredentialKindForm` is extracted and the clone manual form switches to it.
- **Sections:**
  - vault folder
  - color scheme (theme moves into `src/state/`)
  - Commit as
  - Connect a repository
  - Sync: the not-connected banner or Disconnect, and the four raw sub-forms (all going through the plaintext-consent confirm in the container)
  - Credentials: orphan notice, cleanup, remove all
- **Change folder** becomes a `vault` action that resets page state.
- **Deep links:** `openSettings({ section, prefillUrl, credentialKind })` is now handled by the Vue Settings. Delete the Settings callbacks from steps 1 and 10.
- **Incidental fix:** delete the unused `confirmSshHostKey`.
- **Stories:**
  - no vault path
  - Commit as: prefilled, saved, saved with a warning, error
  - the not-connected banner vs the Disconnect button
  - each sub-form visible, with its status text ("Connecting…", "Connected.", an error)
  - GitHub device code shown, and the GitHub install CTA
  - the orphan notice shown and hidden
  - remove-all fully succeeded and partially failed

### Step 12: App shell

- **Scope:**
  - `App.vue`: the `vaultView` switch, the workspace layout and sidebar chrome (collapse rail, compact drawer, `COMPACT_MEDIA_QUERY`), and the modal host.
  - One `createApp`. Every `mountIsland` call is replaced by the template, and `mountIsland` goes.
- **Removed:**
  - what's left of `main.ts` (down to a bootstrap, or nothing)
  - the `index.html` surface markup
  - the global shortcut listeners' DOM wiring (they call `ui` actions)
  - the dead `#greet-input` rule
- **Stories:** the sidebar chrome states (expanded desktop, collapsed rail, compact with the drawer closed, compact with the drawer open), if chrome is split into a presentational SFC.
- **Done when:** no temporary callback root props are left.

## Out of scope

- Visual regression or screenshot diffing.
- Co-locating or scoping CSS per surface.
- Running the workbench inside the Tauri webview. Webview-specific bugs are debugged in the real app.
- Android-specific tooling beyond viewport presets.
- Deploying or publishing the workbench.
- Replacing native dialogs with in-app Vue dialogs. They stay behind `dialogs.ts` in containers.
- Writing interaction tests, adding Vitest or `addon-vitest`, and testing containers. These are left to a future test effort (see the watch item in [Storybook](#storybook)).
