# Assemble the handoff spec

Type: task
Status: resolved
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

## Answer

**Done: [spec.md](../spec.md).** It covers tooling (Vue 3, `vue-tsc`, Storybook 10 `vue3-vite` with themes and viewports, the npm scripts, and the CI `storybook build`), the surface contract, the editor wrapper and `commit`, and the 13 migration steps. Each step section can be copied into one implementation ticket: scope, state moved, expected temporary callbacks, and the stories list taken from the surface inventory. Every step shares the same definition of done and the same checks. The spec ends with the out-of-scope list.

No gaps needed a new ticket. A few placements are direct consequences of existing decisions, not new ones:
- **Theme** moves into `src/state/` in step 11, since Settings holds the theme radios.
- **Search** switches to `<Modal>`, and the clone surfaces switch to `DeviceFlow`, `SshKey`, `CommitAs`, and `CredentialKindForm`, in the step that extracts each one (extract on second copy).
- **The editor exposes only `scrollToHeading`** (`getMarkdown` is not exposed), because data leaves a surface only through emits.
- **The dead `#greet-input` rule** has no owning surface, so it is deleted in step 12's cleanup.
