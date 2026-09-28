// Ticket 02 (the pilot): the first `src/state/` module -- see
// spec.md#shared-state-srcstate. Plain `ref()` plus actions, no Pinia.
// Replaces main.ts's own `currentSyncStatus` variable and the
// `applySyncIndicator` function that wrote it (and the DOM) on every
// `get_sync_status` poll and `sync-status-changed` event -- this module now
// owns that state, and every surface (the sync indicator/popup, and
// Settings' still-vanilla sync banner) reads it instead.
import { readonly, ref, type Ref } from "vue";
import { getSyncStatus, onSyncStatusChanged, type SyncStatus } from "../vault-api";

const status: Ref<SyncStatus> = ref({ state: "noRemote" });

/** Read-only outside this module. Changed only by {@link refreshSyncStatus}
 * and the `sync-status-changed` subscription below. */
export const syncStatus = readonly(status);

/** Polls `get_sync_status` once -- called on app start and again once a
 * vault opens, same as main.ts's original `refreshSyncStatus`. Silently
 * ignored if no vault is open yet, or the command is otherwise unavailable,
 * matching the original's behavior (the background sync loop and the
 * `sync-status-changed` subscription below keep this fresh afterwards, so
 * there's no need to surface or retry a failed poll here). */
export async function refreshSyncStatus(): Promise<void> {
  try {
    status.value = await getSyncStatus();
  } catch {
    // No vault open yet, or the command is otherwise unavailable -- leave
    // the last-known status as-is.
  }
}

// Modules only evaluate once, so this module-scope call is the equivalent
// of main.ts's former one-time `void onSyncStatusChanged(applySyncIndicator)`
// subscription in `init()`.
void onSyncStatusChanged((next) => {
  status.value = next;
});
