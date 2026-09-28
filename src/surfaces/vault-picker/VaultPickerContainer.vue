<script setup lang="ts">
// Container (spec.md#surface-contract): owns the picker's state read
// (`ui.vaultView`), its error text, and every backend call -- including the
// remembered-vault auto-open at launch, self-contained here since the
// container owns backend calls anyway (this replaces main.ts's old
// `init()` doing that itself before anything rendered). No story
// (containers aren't storied).
import { onMounted, ref } from "vue";
import { vaultView, setVaultView } from "../../state/ui";
import { openVault, registerVaultOpenedHandler } from "../../state/vault";
import { refreshPages, refreshTrash } from "../../state/pages";
import { getSettings, pickVaultFolder } from "../../vault-api";
import { friendlyVaultOpenError } from "../../vault-open-error";
import VaultPicker from "./VaultPicker.vue";

// Temporary callback root props (spec.md#islands-and-how-they-merge): the
// guided clone wizard and the standalone manual clone form don't migrate off
// `main.ts` until step 9 -- they still arrive here as root props, same
// pattern as the search modal's (ticket 03). Page/trash loading (ticket 04's
// third callback, `loadPagesAndTrashInVanilla`) is gone as of ticket 05:
// `state/pages.ts`'s own `refreshPages`/`refreshTrash` cover it, so this
// container calls them directly below instead of routing through `main.ts`.
const props = defineProps<{
  /** Opens the full-screen guided clone wizard. */
  openCloneWizardInVanilla: () => void;
  /** Opens the standalone "git clone" manual form. */
  openCloneManualInVanilla: () => void;
}>();

/** Runs once a vault successfully opens, on every path (registered with
 * `state/vault.ts`, which calls it from its `openVault` action). */
async function loadPagesAndTrash(): Promise<void> {
  await refreshPages();
  await refreshTrash();
}
registerVaultOpenedHandler(loadPagesAndTrash);

const error = ref<string | null>(null);

async function handleSelect() {
  let path: string | null;
  try {
    path = await pickVaultFolder();
  } catch (err) {
    // Some platforms (Android currently) have no folder picker at all --
    // `pickVaultFolder` itself rejects rather than resolving `null`, so this
    // needs the same error path as `openVault` failing below.
    error.value = friendlyVaultOpenError(String(err));
    return;
  }
  if (!path) return; // user cancelled

  try {
    await openVault(path);
    error.value = null;
  } catch (err) {
    error.value = friendlyVaultOpenError(String(err));
  }
}

function handleOpenCloneWizard() {
  setVaultView("cloneWizard");
  props.openCloneWizardInVanilla();
}

function handleOpenCloneManual() {
  // Unlike the clone wizard (a full-screen takeover that replaces this
  // picker), the manual form is a modal overlay on top of it -- same as
  // today, the picker stays visible (dimmed by the overlay backdrop)
  // underneath, so `vaultView` doesn't change here. `"cloneManual"` stays
  // unused until step 9 actually migrates the manual form onto this state.
  props.openCloneManualInVanilla();
}

// Auto-opens the remembered vault at launch -- same effect as main.ts's old
// `init()` awaiting `openVaultAndLoad(settings.vaultPath)` before showing
// anything, moved here since the container owns backend calls.
onMounted(async () => {
  const settings = await getSettings();
  if (!settings.vaultPath) return;
  try {
    await openVault(settings.vaultPath);
  } catch (err) {
    error.value = friendlyVaultOpenError(String(err));
  }
});
</script>

<template>
  <VaultPicker
    v-if="vaultView === 'picker'"
    :error="error"
    @select="handleSelect()"
    @open-clone-wizard="handleOpenCloneWizard()"
    @open-clone-manual="handleOpenCloneManual()"
  />
</template>
