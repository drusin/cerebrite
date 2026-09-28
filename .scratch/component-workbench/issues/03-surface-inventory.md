# Surface inventory and coupling map of the current frontend

Type: task
Status: open

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
