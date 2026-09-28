<script setup lang="ts">
// Container (spec.md#surface-contract): owns every backend/dialog call for
// the sync popup; no story (containers aren't storied). Replaces
// main.ts's `refreshSyncPopupDetails`/`handleSyncNowClick`/
// `renderSyncPopupCtas`/`handleSyncCtaClick`/`openQuickReconnect`/
// `handleUnlockAndRetryClick`/`handleSetUpKeychainClick`/
// `handleStoreAsPlaintextClick`/`handleOpenConflictBackupsClick`/
// `openSyncSettingsFromPopup`.
import { computed, ref, watch } from "vue";
import { syncStatus } from "../../state/sync";
import {
  syncPopupOpen,
  syncPopupAnchor,
  closeSyncPopup,
  openSettings,
  registerOpenSettingsHandler,
  type OpenSettingsOptions,
} from "../../state/ui";
import { syncIndicatorFor, type SyncCtaId } from "../../sync-status";
import {
  getSyncDetails,
  triggerSyncNow,
  unlockKeychainAndRetrySync,
  revealConflictBackups,
  type CredentialKind,
} from "../../vault-api";
import { confirmDialog, messageDialog } from "../../dialogs";
import SyncPopup from "./SyncPopup.vue";

// Temporary callback root prop (state/ui.ts's doc comment has the full
// rationale): the Settings side of `openSettings` -- Settings is still
// vanilla, so the actual modal-opening/prefilling/scrolling logic still
// lives in main.ts and arrives here as a root prop to register.
const props = defineProps<{
  openSettingsInVanilla: (options: OpenSettingsOptions) => void;
}>();
registerOpenSettingsHandler(props.openSettingsInVanilla);

const indicator = computed(() => syncIndicatorFor(syncStatus.value));

// Popup-only detail (`get_sync_details`) -- not "truly shared state" per
// spec.md#shared-state-srcstate (only `syncStatus` itself is), so it stays
// local to this container instead of living in `state/sync.ts`.
const provider = ref<string | null>(null);
const lastSyncedAt = ref<number | null>(null);
const remoteUrl = ref<string | null>(null);
const credentialKind = ref<CredentialKind | null>(null);
const syncNowPending = ref(false);

async function refreshDetails() {
  try {
    const details = await getSyncDetails();
    provider.value = details.provider;
    lastSyncedAt.value = details.lastSyncedAt;
    remoteUrl.value = details.remoteUrl;
    credentialKind.value = details.credentialKind;
  } catch {
    provider.value = null;
    lastSyncedAt.value = null;
    remoteUrl.value = null;
    credentialKind.value = null;
  }
}

watch(syncPopupOpen, (open) => {
  if (open) void refreshDetails();
});

async function handleSyncNow() {
  syncNowPending.value = true;
  try {
    await triggerSyncNow();
  } catch (err) {
    await messageDialog(String(err));
  } finally {
    syncNowPending.value = false;
  }
}

/** "Reconnect" (every credential-rejected/expired/host-key cause): jumps
 * straight into the matching credential-kind sub-form, pre-filled with the
 * already-known repository URL, rather than restarting the guided connect
 * wizard -- same as the old `openQuickReconnect`. */
function handleReconnect() {
  closeSyncPopup();
  openSettings({
    section: "sync",
    prefillUrl: remoteUrl.value ?? "",
    credentialKind: credentialKind.value ?? undefined,
    provider: provider.value,
  });
}

async function handleUnlockAndRetry() {
  try {
    await unlockKeychainAndRetrySync();
  } catch (err) {
    await messageDialog(`Couldn't unlock the keychain: ${err}`);
  }
}

async function handleSetUpKeychain() {
  await messageDialog(
    "No keychain could be reached. Set up or unlock your system's credential " +
      "store (e.g. start your desktop's Secret Service/keychain daemon), then " +
      'use "Sync now" below to retry.',
  );
}

/** "Store as plaintext instead" for `keychainUnavailable`. Uses
 * `confirmDialog` (not the old, effectively-broken `confirmBrowser` --
 * see dialogs.ts's module doc comment) since this call site is being
 * rewritten here, not just moved verbatim. */
async function handleStoreAsPlaintext() {
  const confirmed = await confirmDialog(
    "Store this connection's credential as a plaintext file instead of the " +
      "system keychain? This is less secure than the keychain, and should " +
      "only be used when no keychain is available. You'll need to re-enter " +
      "the credential.",
  );
  if (!confirmed) return;
  handleReconnect();
}

async function handleOpenConflictBackups() {
  try {
    await revealConflictBackups();
  } catch (err) {
    await messageDialog(`Couldn't open the conflict-backups folder: ${err}`);
  }
}

function handleCta(id: SyncCtaId) {
  switch (id) {
    case "reconnect":
      return handleReconnect();
    case "unlockAndRetry":
      return void handleUnlockAndRetry();
    case "setUpKeychain":
      return void handleSetUpKeychain();
    case "storeAsPlaintext":
      return void handleStoreAsPlaintext();
    case "retrySync":
      return void handleSyncNow();
    case "openConflictBackups":
      return void handleOpenConflictBackups();
  }
}

function handleSettingsLink() {
  closeSyncPopup();
  openSettings({ section: "sync" });
}
</script>

<template>
  <SyncPopup
    v-if="syncPopupOpen"
    :indicator="indicator"
    :anchor="syncPopupAnchor"
    :provider="provider"
    :last-synced-at="lastSyncedAt"
    :sync-now-pending="syncNowPending"
    @close="closeSyncPopup()"
    @sync-now="handleSyncNow()"
    @cta="handleCta($event)"
    @settings-link="handleSettingsLink()"
  />
</template>
