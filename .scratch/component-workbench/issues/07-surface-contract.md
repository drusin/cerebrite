# Surface contract

Type: grilling
Status: open
Blocked by: 04

## Question

Surfaces are Vue 3 components migrated one at a time as islands, ending in one Vue app ([Framework or not: how surfaces are built](04-framework-decision.md)). What is a surface's contract, and how does the app compose them along the way?

- **Island boundary:** does each island sit behind a framework-neutral `mount(el, props, callbacks) → {update, destroy}` handle that `main.ts` calls, or does `main.ts` talk to Vue directly (for example by mutating a shared `reactive()`/`ref()` that islands render from)?
- **Props and emits:** do surfaces take plain data in and emit intents out ("open page", "sync now") and never call `vault-api` themselves? Or may some surfaces own their backend calls? The wizards and clone manual run device flow, repo lists, and connect/clone inside the surface today.
- **Shared state:** where the truly shared state from the inventory lives once it leaves `main.ts` (open page, Recent, vault path and open/closed, sync status, theme, page and trash lists): Pinia, a plain `reactive()` module, or props threaded from a root. Also, what replaces the ad-hoc `loadPages()`/`loadTrash()` "changed" signals.
- **Composition and routing:** how the shell switches between the vault picker, the clone wizard, and the workspace, and how modals (search, Settings, connect wizard, sync popup) are opened. Also, when and how the islands merge into one `createApp`.
- **Shared sub-components:** whether the duplicated device-flow, SSH-key, "Commit as", credential-kind, and page-list UI becomes shared components as part of the contract, or later.

Keep isolated interaction tests possible: a surface must be mountable with props alone.

Background: the coupling summary in the [surface inventory](../research/03-surface-inventory.md).
