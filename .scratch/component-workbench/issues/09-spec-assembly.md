# Assemble the handoff spec

Type: task
Status: open
Blocked by: 08

## Question

AFK. Bring the resolved decisions together in `.scratch/component-workbench/spec.md`, a spec a separate implementation effort can plan from (as the MVP and git-provider maps were handed off). No new decisions: if a gap turns up, raise it as a new ticket instead of deciding it here.

It must cover:
- **Tooling:** Vue 3 (per [ADR-0014](../../../docs/adr/0014-vue-3-for-frontend-surfaces.md)), `vue-tsc`, Storybook 10 `vue3-vite` with themes and viewports, the dev-only npm script, and CI `storybook build`.
- **The surface contract** (from [Surface contract](07-surface-contract.md)).
- **The editor wrapper and `commit`** (from [Editor in Vue](06-editor-in-vue.md)).
- **The 13-step migration order**, with the per-step definition of done and checks (from [Pilot surface and migration order](08-migration-order.md)).
- **The out-of-scope list** from the map.

The spec should be written so that the implementation map can turn each migration step into one ticket.
