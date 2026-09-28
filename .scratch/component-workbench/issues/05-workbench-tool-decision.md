# Which workbench tool

Type: grilling
Status: resolved
Blocked by: 01, 04

## Question

Given the tool landscape (ticket 01) and the framework decision (ticket 04), which workbench does the spec adopt? The decision must fit the standing constraints: dev-only, theme toggle, viewport presets, and a path to isolated interaction tests later.

## Answer

**Storybook 10.x (`^10.6`) with `@storybook/vue3-vite`.** The hand-rolled Vite gallery stays documented as the fallback if Storybook's weight becomes a problem. Histoire is ruled out ([tool landscape](01-workbench-tool-landscape.md)).

- **Framework package: `vue3-vite`, not `html-vite`.** The research chose `html-vite` so that vanilla and Vue stories could share one Storybook. There will be no vanilla stories (see below), so that reason is gone. `vue3-vite` gives native Vue stories: `args` map to props, emits show up as actions, controls are inferred from `defineProps` types, and Vue unmounts cleanly between stories. With `html-vite`, each story would `createApp().mount()` by hand, and the research couldn't verify cleanup there.
- **Only migrated Vue surfaces get stories.** A surface still in `main.ts` can't be storied without first being extracted, and extracting it into a vanilla module would be thrown away when it moves to Vue. So a surface's stories are part of the **definition of done for its migration step**: migrated means a Vue SFC plus its stories. The editor is not an exception, because both options in [Editor in Vue](06-editor-in-vue.md) put `PageEditor` behind a Vue component.
- **Story format:** CSF3 `*.stories.ts` colocated next to the SFC in `src/`, with one named story per interesting state (`Syncing`, `Conflict`, `Offline`, …) taken from the [surface inventory](../research/03-surface-inventory.md). No MDX or docs pages.
- **Addons:**
  - `@storybook/addon-themes` with `withThemeByDataAttribute`, offering **System / Light / Dark**. System removes `data-theme` so `prefers-color-scheme` applies, matching `applyTheme` in `main.ts`.
  - Core viewport, configured with a **phone** preset and a **tablet** preset alongside the full-width default. `styles.css` has no width breakpoints to match, so standard device sizes are fine.
  - Not added: `addon-a11y` and `addon-docs`/autodocs.
  - `addon-vitest` is deferred to the future interaction-test effort.
- **Preview:** `.storybook/preview.ts` imports the global `src/styles.css`. Wrappers a surface needs, such as the `.workspace` wrapper sidebar stories need, go in that surface's story decorators.
- **Scripts and CI:**
  - `npm run storybook` runs the dev server.
  - CI runs `storybook build` into a throwaway directory as a compile check, so stories that break as `main.ts` shrinks get caught. The output is never uploaded or deployed.
  - Telemetry is disabled (`core.disableTelemetry: true`).
- **Interaction-test path (future):** `play` functions plus `@storybook/addon-vitest` (Vitest browser mode). **Watch item:** `addon-vitest@10.x` accepts only Vitest `^3 || ^4`, while Vitest 5 needs Storybook 11. The test effort chooses between pinning Vitest 4.1.x and moving to Storybook 11 stable, whichever is current then. Vitest isn't added until then.
- **No ADR:** the workbench is dev-only and cheap to swap.
