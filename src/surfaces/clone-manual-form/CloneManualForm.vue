<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. Renders
// `#clone-manual-overlay`'s old markup/classes verbatim (spec.md#step-9-
// clone-wizard--clone-manual-form) -- variant B's raw-git-vocabulary door
// into the same `clone_and_open_vault` command the guided wizard uses.
// Device flow and the SSH key sub-form are written inline here (first
// copies, per the ticket, alongside the clone wizard's own copies); they're
// extracted into shared components once a second copy exists in step 10.
import { ref } from "vue";
import type { DeviceCodeDisplay } from "../clone-wizard/useCloneWizard";

export type ManualCredentialKind = "accessToken" | "sshKey" | "githubOauth" | "gitlabOauth";

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

const rootEl = ref<HTMLElement | null>(null);

function handleBackdropClick(event: MouseEvent) {
  if (event.target === rootEl.value) emit("close");
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    emit("close");
  }
}
</script>

<template>
  <div
    ref="rootEl"
    class="settings-modal-overlay"
    @click="handleBackdropClick"
    @keydown="handleKeydown"
  >
    <div class="settings-modal" role="dialog" aria-modal="true" aria-label="Clone a repository (manual)">
      <div class="settings-modal-header">
        <h2>Clone a repository</h2>
        <button type="button" aria-label="Close" @click="emit('close')">✕</button>
      </div>
      <div class="settings-section">
        <label>
          Remote URL
          <input
            type="text"
            placeholder="https://example.com/user/repo.git"
            :value="remoteUrl"
            @input="emit('update:remoteUrl', ($event.target as HTMLInputElement).value)"
          />
        </label>
        <label>
          Branch
          <input type="text" value="(the repository's default branch)" disabled />
        </label>
        <label>
          Destination folder
          <input type="text" readonly placeholder="Choose a folder…" :value="destination" />
        </label>
        <button type="button" @click="emit('pickDestination')">Choose folder…</button>

        <label>
          Credential kind
          <select
            :value="credentialKind"
            @change="emit('update:credentialKind', ($event.target as HTMLSelectElement).value as ManualCredentialKind)"
          >
            <option value="accessToken">Access token</option>
            <option value="sshKey">SSH key</option>
            <option value="githubOauth">GitHub sign-in</option>
            <option value="gitlabOauth">GitLab sign-in</option>
          </select>
        </label>

        <div v-if="credentialKind === 'accessToken'" class="settings-connect-form">
          <label>
            Username
            <input
              type="text"
              :value="tokenUsername"
              @input="emit('update:tokenUsername', ($event.target as HTMLInputElement).value)"
            />
          </label>
          <label>
            Access token
            <input
              type="password"
              :value="tokenValue"
              @input="emit('update:tokenValue', ($event.target as HTMLInputElement).value)"
            />
          </label>
        </div>

        <div v-else-if="credentialKind === 'sshKey'" class="settings-connect-form">
          <div class="wizard-button-row">
            <button type="button" @click="emit('generateSshKey')">Generate a new key</button>
            <button type="button" @click="emit('importSshKey')">Import an existing key</button>
          </div>
          <p v-if="sshKeyStatus" class="settings-connect-status">{{ sshKeyStatus }}</p>
        </div>

        <div v-else-if="credentialKind === 'githubOauth'" class="settings-connect-form">
          <button type="button" @click="emit('githubSignIn')">Sign in with GitHub</button>
          <div v-if="githubDeviceCode" class="settings-connect-status">
            <p>
              Go to
              <a :href="githubDeviceCode.verificationUri" target="_blank" rel="noopener">{{
                githubDeviceCode.verificationUri
              }}</a>
              and enter code: <strong>{{ githubDeviceCode.userCode }}</strong>
            </p>
          </div>
          <p v-if="githubStatus" class="settings-connect-status">{{ githubStatus }}</p>
        </div>

        <div v-else class="settings-connect-form">
          <button type="button" @click="emit('gitlabSignIn')">Sign in with GitLab</button>
          <div v-if="gitlabDeviceCode" class="settings-connect-status">
            <p>
              Go to
              <a :href="gitlabDeviceCode.verificationUri" target="_blank" rel="noopener">{{
                gitlabDeviceCode.verificationUri
              }}</a>
              and enter code: <strong>{{ gitlabDeviceCode.userCode }}</strong>
            </p>
          </div>
          <p v-if="gitlabStatus" class="settings-connect-status">{{ gitlabStatus }}</p>
        </div>

        <button type="button" class="wizard-primary-action" :disabled="submitting" @click="emit('submit')">
          Clone
        </button>
        <p v-if="statusMessage" class="settings-connect-status">{{ statusMessage }}</p>
      </div>
    </div>
  </div>
</template>
