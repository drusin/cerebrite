# Surface contract

Type: grilling
Status: resolved
Blocked by: 04

## Question

Surfaces are Vue 3 components migrated one at a time as islands, ending in one Vue app ([Framework or not: how surfaces are built](04-framework-decision.md)). What is a surface's contract, and how does the app compose them along the way?

- **Island boundary:** does each island sit behind a framework-neutral `mount(el, props, callbacks) → {update, destroy}` handle that `main.ts` calls, or does `main.ts` talk to Vue directly (for example by mutating a shared `reactive()`/`ref()` that islands render from)?
- **Props and emits:** do surfaces take plain data in and emit intents out ("open page", "sync now") and never call `vault-api` themselves? Or may some surfaces own their backend calls? The wizards and clone manual run device flow, repo lists, and connect/clone inside the surface today.
- **Shared state:** where the truly shared state from the inventory lives once it leaves `main.ts` (open page, Recent, vault path and open/closed, sync status, theme, page and trash lists): Pinia, a plain `reactive()` module, or props threaded from a root. Also, what replaces the ad-hoc `loadPages()`/`loadTrash()` "changed" signals.
- **Composition and routing:** how the shell switches between the vault picker, the clone wizard, and the workspace, and how modals (search, Settings, connect wizard, sync popup) are opened. Also, when and how the islands merge into one `createApp`.
- **Shared sub-components:** whether the duplicated device-flow, SSH-key, "Commit as", credential-kind, and page-list UI becomes shared components as part of the contract, or later.

- **Final edits on swap/unmount:** [Editor in Vue](06-editor-in-vue.md) found that edits made in the last ~200 ms before a page switch are lost: Milkdown's listener debounce is cancelled on `destroy()`, and `flushPendingSaveForCurrentPage` skips the flush when no autosave timer is pending yet. Decide how the contract handles this. For example, a surface sends its final `change` before a swap or unmount, instead of the parent pulling it with `getMarkdown()`. Also decide whether imperative escape hatches (`defineExpose` for `scrollToHeading`/`getMarkdown`) are allowed at all.

Keep isolated interaction tests possible: a surface must be mountable with props alone.

Background: the coupling summary in the [surface inventory](../research/03-surface-inventory.md).

## Answer

**Every surface is split into a presentational SFC and a container. Shared state lives in plain `reactive()` action modules. Islands join through those modules until an app-shell step merges them into one app.**

- **Two layers.**
  - The **presentational surface** is the SFC that gets stories. It takes plain data as props and emits intents. It never imports `vault-api`, `dialogs.ts`, or a state module.
  - The **container** (a `useX` composable or an `XContainer.vue`) reads state, calls actions and `vault-api`, and feeds the surface. Containers get no stories. For trivial surfaces (vault picker, search) the container can be a few lines inline in the parent.
  - **Wizards:** the wizard SFC takes `state: WizardState` / `CloneWizardState` as a prop and emits the reducer's events. A `useConnectWizard(api)` / `useCloneWizard(api)` composable runs `reduceWizard` / `reduceCloneWizard` plus the effects (device-flow poll, repo lists, connect/clone). Stories are one reducer state per step, with no backend. The clone manual form follows the same split.
  - As a result, **no fake `vault-api` is needed for stories.** Only containers touch the backend, and they aren't storied. Testing containers is left to the future test effort.
- **Shared state: plain `reactive()`/`ref()` modules in `src/state/`,** exporting read-only state plus **actions**. The truly shared state from the inventory is the open page, Recent, vault path and open/closed, sync status, theme, and the page and trash lists.
  - An action does the backend call and then re-fetches what it changed, e.g. `renamePage()` → `api.renamePage` → `refreshPages()`. This replaces the ad-hoc `loadPages()` / `loadTrash()` "changed" signals.
  - `main.ts` imports the same modules during migration.
  - Pinia is rejected for now: it needs a shared instance installed in every island and adds a dependency. The action-module shape maps onto Pinia setup stores if it's ever needed.
- **View and modal state: a `ui` state module,** not vue-router.
  - `vaultView: 'picker' | 'cloneWizard' | 'cloneManual' | 'workspace'`.
  - `modal: null | 'search' | 'settings' | 'connectWizard'`. The sync popup is tracked separately because it is anchored, not modal.
  - **Deep links are action arguments,** e.g. `openSettings({ section: 'sync', prefillUrl, credentialKind })`. This replaces `openSyncManualForm` and `openQuickReconnect` writing into Settings' DOM. The sync banner reads sync status from state instead of `applySyncIndicator` writing into it. Together these cut two of the sync popup's three couplings.
  - The connect wizard is its own `modal` value and returns to `'settings'` when it closes.
  - The global shortcuts (Ctrl/Cmd+K, Ctrl/Cmd+,, Escape) call these actions.
- **Island boundary:** `mountIsland(el, Container, rootProps)` is a thin helper around `createApp(...).mount(el)`. There is no framework-neutral `{update, destroy}` handle.
  - Anything already moved into a state module is shared by importing it.
  - Intents whose logic still lives in `main.ts` are passed as **temporary callback root props**. Each migration step moves the logic it needs into a state module and deletes the matching callback, so the callbacks are the visible, shrinking list of what hasn't moved yet.
- **Merging into one app is a named migration step: the app shell.** It migrates the `vaultView` switch, the workspace layout and sidebar chrome, and the modal host into `App.vue`. After that, migrated surfaces render as its children, and any vanilla regions left are mounted inside it until they migrate. Before that step, islands are separate apps joined only by the state modules. Where this step falls is part of the migration order.
- **Editor, final edits:** the editor surface owns the flush.
  - Before it reloads on a `pageKey` change, and in `onBeforeUnmount`, it reads its own `PageEditor.getMarkdown()`. If that differs from the last value it emitted, it emits **`commit(oldPageKey, markdown)`**, and the container saves immediately with no debounce.
  - Ordinary `change` events carry `pageKey` too, because the parent's current page may already have moved on.
  - This closes the lost-edit bug found in [Editor in Vue](06-editor-in-vue.md), and it can be tested in isolation: mount, type, change `pageKey`, and assert that `commit` fired with the old key.
- **`defineExpose` is allowed only for stateless one-shot effects:** `scrollToHeading`, and focus (e.g. the search input on open). It is never used to read state back. Data leaves a surface only through emits.
- **Native dialogs are called from containers only,** through a small `src/dialogs.ts` wrapper (next to `vault-api.ts`) around `@tauri-apps/plugin-dialog` and `window.prompt` / `confirm` / `alert`. A surface emits the intent (e.g. `delete`), and the container confirms and then calls the action. The UX is unchanged. Surfaces that already collect input in their own UI (e.g. the wizards' paste-URL step) keep doing so. Replacing native dialogs with in-app Vue dialogs is out of scope.
- **Overlays:** a modal or popup surface renders its own overlay and backdrop with the existing class names, and emits `close` on Escape or a backdrop click. Escape handling moves out of `init`'s document listeners into each surface as it migrates. Anchored popups take an `anchor: DOMRect | null` prop instead of reading the anchor element, and stories pass a fixed rect.
- **Shared sub-components are extracted when a second copy migrates.** The first migration writes the UI inline, and the second extracts it into `src/components/` with its own stories. The named candidates are device flow (×5), SSH key generate/import (×4), Commit as (×3), the credential-kind selector with sub-forms (×2), the page list (×4: pages, Recent, Trash, backlinks), and a `<Modal>` shell (extracted with the second modal).
- **File layout:**
  - `src/surfaces/<surface>/` holds `X.vue` (presentational), `X.stories.ts`, and `useX.ts` or `XContainer.vue`.
  - `src/state/` holds the state modules, and `src/components/` holds the shared components.
  - The existing pure modules (`sync-status.ts`, `connect-wizard.ts`, `clone-wizard.ts`, `heading-slug.ts`) stay where they are.
