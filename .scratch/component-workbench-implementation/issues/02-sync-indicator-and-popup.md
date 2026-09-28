# 02: Sync indicator + popup (pilot)

**What to build:** The sync indicator (sidebar footer button and rail icon) and its anchored popup become Vue, fed by a sync state module. This is the pilot: it proves the surface/container split, a state module, `mountIsland`, the `anchor` prop, a deep-link action, and temporary callback root props. The user sees the same indicator and popup; Settings' sync banner now reflects sync state on its own, and "Sync settings…", reconnect, and store-as-plaintext open Settings through a single action. See [spec: Step 1](../../component-workbench/spec.md#step-1-sync-indicator--popup-pilot) and [Surface contract](../../component-workbench/spec.md#surface-contract).

**Blocked by:** 01 (Foundation)

**Status:** ready-for-agent

- [ ] `state/sync.ts`: status, the `onSyncStatusChanged` subscription, and refresh — replaces `currentSyncStatus` and `applySyncIndicator`
- [ ] First cut of `state/ui.ts`: `openSettings({ section, prefillUrl, credentialKind })` and the sync popup's open/closed state (tracked separately from `modal`)
- [ ] Indicator and popup glyph, text, and CTAs come from the pure `syncIndicatorFor`
- [ ] Popup takes `anchor: DOMRect | null`, renders its own overlay with existing class names, and emits `close` on Escape and outside click (replacing `init`'s document listeners)
- [ ] Settings' sync banner reads `state/sync.ts` instead of being written by `applySyncIndicator`
- [ ] `openSyncManualForm` / `openQuickReconnect` no longer write Settings' DOM; callers use `openSettings(…)`. The still-vanilla Settings side is a temporary callback root prop
- [ ] Stories: one per icon state (`notConnected`, `syncing`, `synced`, `retrying`, `needsAttention`, `warning` via `refreshedSignInNotSaved`); popup with and without provider / last-synced; one per `SyncFailureCause` variant (11) with its text and 0–2 CTAs. Stories pass a fixed anchor rect
- [ ] Presentational SFC(s) never import `vault-api`, `dialogs.ts`, or a `src/state/` module, and mount from props alone
- [ ] Container (composable or `XContainer.vue`) owns state reads, actions, backend and dialog calls; no stories for it
- [ ] One named CSF3 story per interesting state, colocated with the SFC (no MDX, docs pages, or autodocs)
- [ ] No `<style>` block; the SFC renders the existing `styles.css` class names
- [ ] The `index.html` markup and `main.ts` code this step replaced are removed
- [ ] Escape / document listeners for the surface moved into it
- [ ] Temporary callback root props replaced by this step are deleted and named in the commit message; any new ones are named too
- [ ] `npm run build` (`vue-tsc --noEmit && vite build`) passes
- [ ] `storybook build` passes
- [ ] A manual run of the real Tauri app covers the flows this step touched (see [debugging in a sandbox](../../../docs/agents/debugging-sandbox.md))
