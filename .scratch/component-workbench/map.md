Label: wayfinder:map

# Component workbench

## Destination

A written spec, ready to hand to a separate implementation effort (as the MVP and git-provider maps were), for splitting the frontend into isolated **surfaces** that can each be browsed in their interesting states in a dev-only workbench (Storybook or similar): which tool, what a surface's contract looks like, how little backend is faked, and the incremental migration order out of `main.ts`, starting with a named pilot surface.

## Notes

- Where things stand: the frontend is vanilla TypeScript with no UI framework. `src/main.ts` (~3.8k lines) holds module-level state and ~135 direct DOM lookups; all markup lives in `index.html`, with surfaces as hidden `<section>`/`<div>` elements that get toggled. Every Tauri `invoke` goes through `src/vault-api.ts`, the obvious seam. `connect-wizard.ts`, `clone-wizard.ts`, and `sync-status.ts` are already pure reducers/derivations. There's no test runner.
- Call `grilling` + `domain-modeling` for any ticket that hinges on a decision. `CONTEXT.md` is a product glossary, so dev-tooling terms don't belong there unless they become user-facing.

### Standing constraints

Settled in charting; not open for relitigation.

- **Surface granularity.** One showable element is a surface (sidebar/page list, page article + backlinks, page editor, search modal, sync indicator + popup, settings modal, connect wizard, clone wizard, vault picker, trash), shown in its interesting states. No atomic widget library.
- **Manual browsing is the goal; automated tests are designed for.** Surface shape must permit isolated interaction tests later; building them is not part of this effort.
- **As little backend as feasible.** Surfaces should prefer plain props/data in and events out. A fake `vault-api` is allowed only where that keeps code simpler than avoiding it.
- **Framework: Vue 3.** Decided in [Framework or not: how surfaces are built](issues/04-framework-decision.md) ([ADR-0014](../../docs/adr/0014-vue-3-for-frontend-surfaces.md)).
- **Incremental migration.** One surface at a time, and the app keeps working after every step.
- **The page editor (Milkdown + wiki-link plugin) is a first-class story**, fed sample markdown directly.
- **Styling stays global.** The workbench loads `styles.css` and offers a light/dark toggle plus phone/tablet viewport presets.
- **Dev-only.** Runs in a normal desktop browser via its own npm script, is never in the Tauri bundle, and is never deployed.
- **No dependency may be end-of-life, deprecated, or abandoned** (carried over from earlier maps).

## Decisions so far

<!-- one line per resolved ticket; zoom the link for the detail -->

- [Workbench tool landscape for a Vite 8 app, vanilla and Vue](issues/01-workbench-tool-landscape.md): Storybook 10.x `html-vite` is the one mature, maintained, Vite-8-official option. It hosts vanilla and Vue stories in one setup, its theme addon toggles the `data-theme` attribute this app already uses, and a smoke test rendered Milkdown fine. Vitest must be pinned to 4.1.x for `addon-vitest` until Storybook 11. Histoire is ruled out (stalled beta, Vite 7 only, no vanilla support); a hand-rolled gallery is the fallback.
- [Ways to shape a surface: plain modules, web components, Vue 3](issues/02-surface-shape-options.md): All three can be adopted one surface at a time and are maintained. Vue costs about 23 KB gzipped plus `vue-tsc`, but brings typed props/emits, an official `@milkdown/vue`, and is already a transitive dependency. Lit fights the global stylesheet (shadow DOM) and this repo's `useDefineForClassFields`. With plain modules, props-in/events-out is a hand-enforced convention. A Vue island can sit behind a framework-neutral `mount → {update, destroy}` handle.
- [Surface inventory and coupling map of the current frontend](issues/03-surface-inventory.md): The editor (already a callback class, and needing no page index), the sync indicator + popup, search, and the vault picker are nearly isolated. The wizards keep their state private but are backend-heavy. The page-navigation cluster (lists, article, Recent, editor glue) and Settings (a hub) are tangled. Native dialogs are a second backend seam outside `vault-api`.
- [Framework or not: how surfaces are built](issues/04-framework-decision.md): Vue 3 (`^3.5`, no release candidates). Surfaces migrate as islands and end in one Vue app. SFCs use `<script setup lang="ts">` with typed props/emits and have no `<style>` blocks: styling stays in the global `styles.css` (surface-prefixed classes, token theming), and id selectors become classes as surfaces migrate. `vue-tsc` replaces `tsc`. Lit and plain modules are rejected. See [ADR-0014](../../docs/adr/0014-vue-3-for-frontend-surfaces.md).

## Not yet specified

- **Minimal fake backend.** Which surfaces genuinely need a stand-in for `vault-api` (the inventory points at the wizards and clone manual, where device flow, repo lists, and connect/clone calls run inside the surface), and whether that's a fake module swapped at build time, injected dependencies, or per-story stubs. It must also cover the second seam: `@tauri-apps/plugin-dialog` plus `window.prompt`/`confirm`/`alert`. Hangs on [Surface contract](issues/07-surface-contract.md) (whether surfaces may own backend calls at all).
- **Pilot surface and migration order.** The inventory confirms the sync indicator + popup as a strong candidate (three couplings to Settings to cut). Search and the vault picker are similarly isolated. The page-navigation cluster and Settings are the hard end. The end state is one Vue app. The order hangs on [Surface contract](issues/07-surface-contract.md).
- **Future interaction-test path.** Which runner (Storybook test / Vitest browser mode / other) the chosen workbench points to, and so what the spec must keep possible. Hangs on [Which workbench tool](issues/05-workbench-tool-decision.md).
- **Spec assembly.** Pulling the decisions into `spec.md` for handoff.

## Out of scope

- Visual regression / screenshot diffing.
- Co-locating or scoping CSS per surface.
- Running the workbench inside the Tauri webview; webview-specific bugs are debugged in the real app (`docs/agents/debugging-sandbox.md`).
- Android-specific tooling beyond viewport presets.
- Deploying or publishing the workbench.
- Actually building the workbench or migrating surfaces (that belongs to the implementation effort this spec hands off to).
