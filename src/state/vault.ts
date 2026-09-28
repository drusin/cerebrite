// Ticket 04: vault path + open/closed state, and the vault "open" action --
// see spec.md#step-3-vault-picker. Plain `ref()` plus actions, same shape as
// `state/sync.ts`. The open action is the single choke point every
// vault-opening path funnels through (a first-run pick, a remembered vault
// at launch, "Change folder…" in Settings, and -- as of ticket 10 -- the
// clone wizard/manual form's successful clone): it runs the backend
// `open_vault` call, updates this module's state, switches `ui.vaultView` to
// `"workspace"`, and refreshes sync status (`state/sync.ts`'s
// `refreshSyncStatus`) -- replacing main.ts's own `openVaultAndLoad` and its
// three separate call sites' `settingsVaultPathEl.textContent = path` writes
// into still-vanilla Settings.
import { computed, readonly, ref, type Ref } from "vue";
import { openVault as openVaultCommand, type VaultInfo } from "../vault-api";
import { refreshSyncStatus } from "./sync";
import { setVaultView } from "./ui";

const path: Ref<string | null> = ref(null);

/** Read-only outside this module. `null` means no vault is open. */
export const vaultPath = readonly(path);

/** Read-only outside this module -- derived from {@link vaultPath}. */
export const vaultOpen = readonly(computed(() => path.value !== null));

// Temporary callback (spec.md#islands-and-how-they-merge): page/trash state
// doesn't move into `src/state/` until step 4, so loading them after a vault
// opens still has to happen in still-vanilla code. Registered once by
// whichever container mounts the vault picker -- mirrors `state/ui.ts`'s
// `registerOpenSettingsHandler`. Delete this indirection -- and call
// page-state's load actions directly from `openVault` below -- once step 4
// lands.
export type VaultOpenedHandler = () => void | Promise<void>;
let vaultOpenedHandler: VaultOpenedHandler | null = null;

export function registerVaultOpenedHandler(handler: VaultOpenedHandler): void {
  vaultOpenedHandler = handler;
}

/**
 * Opens `folderPath` as the vault, then does everything else a vault open
 * needs on every path (via {@link applyVaultOpened} below). Rejects --
 * leaving `vaultPath`/`ui.vaultView` untouched -- if the backend
 * `open_vault` call itself fails; callers show that error themselves (e.g.
 * the vault picker's translated/raw error stories).
 */
export async function openVault(folderPath: string): Promise<VaultInfo> {
  const info = await openVaultCommand(folderPath);
  await applyVaultOpened(folderPath);
  return info;
}

/**
 * The shared "a vault is now open" tail: switches `ui.vaultView` to
 * `"workspace"`, refreshes sync status, and runs the still-vanilla
 * page/trash load (the temporary callback above) -- on every path,
 * including flows that open the vault through a *different* backend
 * command than `open_vault` above (the clone wizard/manual form's
 * `clone_and_open_vault`, ticket 10's `useCloneWizard`/
 * `CloneManualFormContainer`), which call this directly with the path the
 * clone already opened. This is what fixes the clone manual form's
 * previously-missing `refreshSyncStatus` call (spec.md#step-9-clone-
 * wizard--clone-manual-form's "incidental fix").
 */
export async function applyVaultOpened(folderPath: string): Promise<void> {
  path.value = folderPath;
  setVaultView("workspace");
  await refreshSyncStatus();
  await vaultOpenedHandler?.();
}
