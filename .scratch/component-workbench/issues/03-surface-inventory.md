# Surface inventory and coupling map of the current frontend

Type: task
Status: resolved

## Question

AFK. Before the framework and contract decisions can be made, produce a factual inventory of the frontend as it stands in `src/main.ts` + `index.html` (+ `page-editor.ts`, `wiki-link-plugin.ts`, the wizard/sync modules).

For each surface (sidebar/page list, recent list, page article + backlinks, page editor, search modal, sync indicator + popup, settings modal and its sub-forms, connect wizard, clone wizard + manual clone form, vault picker, trash, orphan notice, plus any not listed here):
- The DOM it owns (ids/sections in `index.html`) and the `main.ts` functions that render and handle it.
- The module-level state it reads and writes, and which of that state other surfaces also touch.
- Which `vault-api.ts` calls it makes.
- Its interesting states (the ones a story would show).
- Cross-surface calls (surface A opening or refreshing surface B).

Close with a coupling summary: which surfaces are already nearly isolated (pilot candidates), which are tangled, and which state is truly shared app state versus accidentally global. Record the facts only; no redesign.

Write the inventory to `.scratch/component-workbench/research/03-surface-inventory.md` and link it here.

## Inventory

Full inventory: [`.scratch/component-workbench/research/03-surface-inventory.md`](../research/03-surface-inventory.md).

## Answer

It covers 13 surfaces plus the app shell. Here are the facts later decisions hinge on:

- **Isolated already:**
  - The **page editor** is a `PageEditor(root, onChange, onLinkClick)` class with `load`, `scrollToHeading`, `getMarkdown`, and `destroy`, and no backend. The wiki-link plugin is pure syntax and needs **no page index**, since chips don't show whether their target exists and links are resolved only on click, through the callback.
  - The **sync indicator and popup** has one piece of state that only it writes, plus the pure `syncIndicatorFor`. The couplings to cut are three: it writes directly into Settings' Sync banner, quick reconnect sets Settings' credential-kind select, and it positions itself from the anchor element.
  - The **search modal** keeps all its state private. It emits two intents ("open title" and "create page") and has one backend query.
  - The **vault picker** is trivial.
- **Wizards and clone manual:** their state is private and the two wizards have pure reducers. Only a few calls go outward. But they are the heaviest `vault-api` users: device flow, repo lists, and connect/clone calls all happen inside the surface.
- **Tangled:**
  - The **page-navigation cluster** (page list, Recent, Trash, article, backlinks, editor glue) shares `currentPage`, `recentPages`, `pageEditor`, and `saveTimer`. `loadPages()` and `loadTrash()` serve as ad-hoc "changed" signals from about eight call sites.
  - **Settings** is a hub: it has six entry paths, is written into from outside, and its change-folder action resets the navigation cluster.
- **Shared app state:**
  - **Truly shared:** the open page, Recent, the vault path and whether a vault is open, the sync status, the theme, and the page and trash lists.
  - **Accidentally global:** every wizard, clone-manual, and search variable, the raw-form token and key holders, and the editor instance and its autosave timer.
- **A second backend seam:** `@tauri-apps/plugin-dialog` is imported directly in `main.ts` (not through `vault-api`), and `window.prompt`, `window.confirm`, and `window.alert` are used as dialogs in about 15 places.
- **Duplication:** the device-flow poll loop appears 5 times, SSH generate/import 4 times, the "Commit as" form 3 times, and the credential-kind sub-form toggle twice.
- **Incidental finding:** `confirmSshHostKey` has no caller. The clone-manual success path skips `refreshSyncStatus`, which the other vault-open paths call.
