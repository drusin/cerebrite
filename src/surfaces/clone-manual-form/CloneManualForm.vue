<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. Renders
// `#clone-manual-overlay`'s old markup/classes verbatim (spec.md#step-9-
// clone-wizard--clone-manual-form) -- variant B's raw-git-vocabulary door
// into the same `clone_and_open_vault` command the guided wizard uses.
// Device flow and the SSH key sub-form were written inline here as the
// first copies, alongside the clone wizard's own copies; ticket 11's
// connect wizard is the second copy of each, so they're now
// `components/DeviceFlow.vue`/`SshKey.vue`. Ticket 12 extracts the whole
// credential-kind selector + its four sub-forms into
// `components/CredentialKindForm.vue` (Settings' always-visible "Sync"
// section is the second copy of *that*), and this surface switches to it --
// with its shared repository-URL field hidden (`show-url-field="false"`)
// since this form already has its own single `remoteUrl` field above, and no
// `connect-label`, since the actual connect happens via this form's own
// "Clone" button below, not a per-sub-form one.
import type { DeviceCodeDisplay } from "../../components/DeviceFlow.vue";
import Modal from "../../components/Modal.vue";
import CredentialKindForm, { type CredentialKindOption } from "../../components/CredentialKindForm.vue";

export type ManualCredentialKind = CredentialKindOption;

defineProps<{
  remoteUrl: string;
  destination: string;
  credentialKind: ManualCredentialKind;
  tokenUsername: string;
  tokenValue: string;
  sshKeyStatus: string | null;
  githubDeviceCode: DeviceCodeDisplay | null;
  githubStatus: string | null;
  gitlabDeviceCode: DeviceCodeDisplay | null;
  gitlabStatus: string | null;
  submitting: boolean;
  statusMessage: string | null;
}>();

const emit = defineEmits<{
  close: [];
  "update:remoteUrl": [value: string];
  pickDestination: [];
  "update:credentialKind": [value: ManualCredentialKind];
  "update:tokenUsername": [value: string];
  "update:tokenValue": [value: string];
  generateSshKey: [];
  importSshKey: [];
  githubSignIn: [];
  gitlabSignIn: [];
  submit: [];
}>();
</script>

<template>
  <Modal
    label="Clone a repository (manual)"
    overlay-class="settings-modal-overlay"
    card-class="settings-modal"
    closable
    @close="emit('close')"
  >
    <div class="settings-modal-header">
      <h2>Clone a repository</h2>
    </div>
    <div class="settings-section">
      <label>
        Remote URL
        <input
          type="text"
          placeholder="https://example.com/user/repo.git"
          :value="remoteUrl"
          @input="
            emit('update:remoteUrl', ($event.target as HTMLInputElement).value)
          "
        />
      </label>
      <label>
        Branch
        <input type="text" value="(the repository's default branch)" disabled />
      </label>
      <label>
        Destination folder
        <input
          type="text"
          readonly
          placeholder="Choose a folder…"
          :value="destination"
        />
      </label>
      <button type="button" @click="emit('pickDestination')">
        Choose folder…
      </button>

      <CredentialKindForm
        :credential-kind="credentialKind"
        :show-url-field="false"
        :token-username="tokenUsername"
        :token-value="tokenValue"
        :ssh-key-status="sshKeyStatus"
        :github-device-code="githubDeviceCode"
        :github-status="githubStatus"
        :gitlab-device-code="gitlabDeviceCode"
        :gitlab-status="gitlabStatus"
        @update:credential-kind="emit('update:credentialKind', $event)"
        @update:token-username="emit('update:tokenUsername', $event)"
        @update:token-value="emit('update:tokenValue', $event)"
        @generate-ssh-key="emit('generateSshKey')"
        @import-ssh-key="emit('importSshKey')"
        @github-sign-in="emit('githubSignIn')"
        @gitlab-sign-in="emit('gitlabSignIn')"
      />

      <button
        type="button"
        class="wizard-primary-action"
        :disabled="submitting"
        @click="emit('submit')"
      >
        Clone
      </button>
      <p v-if="statusMessage" class="settings-connect-status">
        {{ statusMessage }}
      </p>
    </div>
  </Modal>
</template>
