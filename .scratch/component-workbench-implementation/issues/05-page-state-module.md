# 05: Page state module

**What to build:** Page, trash, Recent, and open-page state move into one state module whose actions do the backend call and re-fetch what they changed. The ad-hoc "changed" signals disappear before any navigation UI moves. **No UI change**: `main.ts` rendering reads and calls the module. See [spec: Step 4](../../component-workbench/spec.md#step-4-page-state-only).

**Blocked by:** 04 (Vault picker)

**Status:** ready-for-agent

- [ ] `state/pages.ts` holds the page list, trash list, Recent (cap 10), and the open page (persisted or dynamic), exported read-only
- [ ] Actions: open, rename, delete, restore, empty trash, create, save, materialize (dynamic → persisted), each re-fetching what it changed
- [ ] All `loadPages()` / `loadTrash()` call sites removed
- [ ] The five paths that mutated `recentPages` go through actions
- [ ] Temporary callbacks from tickets 03–04 that page actions now cover are deleted and named in the commit message
- [ ] No stories (no surface)
- [ ] `npm run build` (`vue-tsc --noEmit && vite build`) passes
- [ ] `storybook build` passes
- [ ] A manual run of the real Tauri app covers the flows this step touched (see [debugging in a sandbox](../../../docs/agents/debugging-sandbox.md))
