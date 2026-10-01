<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. Renders
// `#settings-modal-overlay`'s old markup/classes verbatim (spec.md#step-11-
// settings) -- the whole of Settings migrates in this one step, since by
// this point most of what's left is layout. Uses `components/Modal.vue`
// (the shared overlay/backdrop/Escape shell, extracted in ticket 11),
// `components/CommitAs.vue` (the third copy of "Commit as" -- the clone
// wizard and connect wizard's own copies were first and second), and
// `components/CredentialKindForm.vue` (the second copy of the
// credential-kind selector + its four sub-forms -- extracted in this same
// ticket, see its own doc comment).
import { ref } from "vue";
import Modal from "../../components/Modal.vue";
import CommitAs, { type CommitAuthorValue } from "../../components/CommitAs.vue";
import CredentialKindForm, { type CredentialKindOption } from "../../components/CredentialKindForm.vue";
import type { DeviceCodeDisplay } from "../../components/DeviceFlow.vue";

/** Kept local rather than imported from `vault-api`'s `Theme`, per
 * spec.md#surface-contract (this presentational surface never imports
 * `vault-api`) -- same "just enough of the backend type" pattern
 * `ConnectWizard.vue`'s own `ConnectRepoOption` uses. */
export type ThemeOption = "system" | "light" | "dark";

defineProps<{
  vaultPath: string | null;
  theme: ThemeOption;

  commitAuthorPrefill: CommitAuthorValue | null;
  commitAuthorError: string | null;
  /** "Saved." or "Saved. <warning>" -- `null` once a fresh edit invalidates
   * the last save's confirmation, or while nothing has been saved yet. */
  commitAuthorSavedMessage: string | null;

  /** `true` once this vault has *some* connection configured (regardless of
   * whether it's currently healthy) -- shows "Disconnect…" instead of the
   * not-connected banner. */
  syncConnected: boolean;
  credentialKind: CredentialKindOption;
  remoteUrl: string;
  tokenUsername: string;
  tokenValue: string;
  accessTokenStatus: string | null;
  accessTokenConnecting: boolean;
  sshKeyStatus: string | null;
  sshKeyConnecting: boolean;
  githubDeviceCode: DeviceCodeDisplay | null;
  githubStatus: string | null;
  /** Set once GitHub sign-in succeeds but the app isn't installed on the
   * repository yet -- shows the install CTA in place of the sign-in status. */
  githubInstallUrl: string | null;
  githubInstallContinuing: boolean;
  gitlabDeviceCode: DeviceCodeDisplay | null;
  gitlabStatus: string | null;

  orphanVisible: boolean;
  orphanCleaning: boolean;
  removeAllStatus: string | null;
  removeAllRemoving: boolean;
}>();

const emit = defineEmits<{
  close: [];
  changeFolder: [];
  "update:theme": [value: ThemeOption];
  commitAuthorSubmit: [name: string, email: string];
  openConnectWizard: [];
  disconnect: [];
  "update:credentialKind": [value: CredentialKindOption];
  "update:remoteUrl": [value: string];
  "update:tokenUsername": [value: string];
  "update:tokenValue": [value: string];
  connectAccessTokenOrSshKey: [];
  generateSshKey: [];
  importSshKey: [];
  githubSignIn: [];
  githubInstallContinue: [];
  gitlabSignIn: [];
  orphanCleanup: [];
  removeAllCredentials: [];
}>();

/** `CommitAs.vue` emits `submit(name, email)` as two payload args -- a
 * template `@submit="$emit(...)"` binding can only see the first
 * (`$event`), so this forwards both explicitly. */
function handleCommitAuthorSubmit(name: string, email: string): void {
  emit("commitAuthorSubmit", name, email);
}

// Imperative escape hatch (spec.md#imperative-escape-hatches): a stateless
// one-shot effect only, never used to read state back -- the container
// calls this once, after applying an `openSettings({ section: 'sync' })`
// deep link, same shape as the article surface's `scrollToHeading`.
const syncSectionEl = ref<HTMLElement | null>(null);
function scrollToSyncSection(): void {
  syncSectionEl.value?.scrollIntoView({ block: "start" });
}
defineExpose({ scrollToSyncSection });
</script>

<template>
  <Modal
    label="Settings"
    overlay-class="settings-modal-overlay"
    card-class="settings-modal"
    closable
    @close="$emit('close')"
  >
    <div class="settings-modal-header">
      <h2>Settings</h2>
    </div>

    <div class="settings-section">
      <h3>Vault folder</h3>
      <p class="settings-vault-path">{{ vaultPath ?? "" }}</p>
      <button type="button" @click="$emit('changeFolder')">Change folder…</button>
    </div>

    <div class="settings-section">
      <h3>Color scheme</h3>
      <div class="settings-theme-options">
        <label>
          <input
            type="radio"
            name="settings-theme"
            value="system"
            :checked="theme === 'system'"
            @change="$emit('update:theme', 'system')"
          />
          System
        </label>
        <label>
          <input
            type="radio"
            name="settings-theme"
            value="light"
            :checked="theme === 'light'"
            @change="$emit('update:theme', 'light')"
          />
          Light
        </label>
        <label>
          <input
            type="radio"
            name="settings-theme"
            value="dark"
            :checked="theme === 'dark'"
            @change="$emit('update:theme', 'dark')"
          />
          Dark
        </label>
      </div>
    </div>

    <div class="settings-section">
      <h3>Commit as</h3>
      <CommitAs
        :prefill="commitAuthorPrefill"
        :error="commitAuthorError"
        submit-label="Save"
        @submit="handleCommitAuthorSubmit"
      />
      <p v-if="commitAuthorSavedMessage" class="settings-connect-status">{{ commitAuthorSavedMessage }}</p>
      <p class="settings-commit-author-note">Affects future commits only -- past history is never rewritten.</p>
    </div>

    <div class="settings-section">
      <h3>Connect a repository</h3>
      <p class="settings-wizard-intro">
        Guided setup: sign in, pick or create a repository, and confirm how commits are attributed.
      </p>
      <button type="button" @click="$emit('openConnectWizard')">Connect…</button>
    </div>

    <div class="settings-section" id="sync-section" ref="syncSectionEl">
      <h3>Sync</h3>
      <p v-if="!syncConnected" class="sync-section-not-connected">
        Not connected — connect a repository.
        <button type="button" @click="$emit('openConnectWizard')">Connect…</button>
      </p>
      <p v-else class="sync-section-connected">
        <button type="button" @click="$emit('disconnect')">Disconnect…</button>
      </p>
      <p class="settings-wizard-intro">Connect this vault to a remote repository directly, using raw git fields.</p>
      <label>
        Branch
        <input type="text" value="(the repository's current branch)" disabled />
      </label>

      <CredentialKindForm
        :credential-kind="credentialKind"
        :remote-url="remoteUrl"
        :token-username="tokenUsername"
        :token-value="tokenValue"
        :access-token-status="accessTokenStatus"
        :ssh-key-status="sshKeyStatus"
        :github-device-code="githubDeviceCode"
        :github-status="githubStatus"
        :gitlab-device-code="gitlabDeviceCode"
        :gitlab-status="gitlabStatus"
        connect-label="Connect"
        :connect-disabled="credentialKind === 'sshKey' ? sshKeyConnecting : accessTokenConnecting"
        @update:credential-kind="$emit('update:credentialKind', $event)"
        @update:remote-url="$emit('update:remoteUrl', $event)"
        @update:token-username="$emit('update:tokenUsername', $event)"
        @update:token-value="$emit('update:tokenValue', $event)"
        @generate-ssh-key="$emit('generateSshKey')"
        @import-ssh-key="$emit('importSshKey')"
        @github-sign-in="$emit('githubSignIn')"
        @gitlab-sign-in="$emit('gitlabSignIn')"
        @connect="$emit('connectAccessTokenOrSshKey')"
      />

      <div v-if="credentialKind === 'githubOauth' && githubInstallUrl" class="settings-connect-status">
        <p>
          GitHub is signed in, but Cerebrite isn't installed on this repository yet.
          <a :href="githubInstallUrl" target="_blank" rel="noopener">Install the app</a>, then continue.
        </p>
        <button type="button" :disabled="githubInstallContinuing" @click="$emit('githubInstallContinue')">
          I've installed it, continue
        </button>
      </div>
    </div>

    <div class="settings-section" id="credentials-section">
      <h3>Credentials</h3>
      <p v-if="orphanVisible" class="settings-orphan-notice">
        Cerebrite has stored credentials for a repository that no longer exists on this device.
        <button type="button" :disabled="orphanCleaning" @click="$emit('orphanCleanup')">Clean up</button>
      </p>
      <p class="settings-wizard-intro">
        Permanently deletes every credential Cerebrite has stored on this device, for every vault it has ever
        connected -- not just this one. This does not revoke anything at the provider.
      </p>
      <button type="button" :disabled="removeAllRemoving" @click="$emit('removeAllCredentials')">
        Remove all stored Cerebrite credentials…
      </button>
      <p v-if="removeAllStatus" class="settings-connect-status">{{ removeAllStatus }}</p>
    </div>
  </Modal>
</template>
