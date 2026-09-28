<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. Renders
// `#connect-wizard-overlay`'s old markup/classes verbatim (spec.md#step-10-
// connect-wizard) -- a compact modal layered over the still-vanilla
// Settings modal, same `.connect-wizard` z-index treatment as before. This
// is the **second modal** (search was first), so its overlay/backdrop/
// Escape mechanics are `components/Modal.vue`, not written inline again;
// and the **second copy** of device flow, the SSH key sub-form, and
// "Commit as" (the clone wizard's own copies were first), so those are
// `components/DeviceFlow.vue`/`SshKey.vue`/`CommitAs.vue` too.
import { ref } from "vue";
import type { WizardState } from "../../connect-wizard";
import Modal from "../../components/Modal.vue";
import DeviceFlow, { type DeviceCodeDisplay } from "../../components/DeviceFlow.vue";
import SshKey from "../../components/SshKey.vue";
import CommitAs, { type CommitAuthorValue } from "../../components/CommitAs.vue";

/** Just enough of `vault-api`'s `RepoInfo` to render a pickable list entry --
 * kept local rather than imported, per spec.md#surface-contract (this
 * presentational surface never imports `vault-api`). */
export interface ConnectRepoOption {
  fullName: string;
  private: boolean;
  cloneUrl: string;
}

defineProps<{
  state: WizardState;
  canGoBack: boolean;
  oauthStatus: string;
  oauthDeviceCode: DeviceCodeDisplay | null;
  repos: ConnectRepoOption[];
  repoPickerStatus: string;
  sshKeyStatus: string | null;
  createRepoError: string | null;
  authorPrefill: CommitAuthorValue | null;
  commitAuthorError: string | null;
}>();

const emit = defineEmits<{
  close: [];
  back: [];
  switchToManual: [];
  chooseHasRepo: [hasRepo: boolean];
  chooseProvider: [provider: "github" | "gitlab" | "other"];
  chooseOauthSignIn: [];
  chooseOtherWaysToConnect: [];
  chooseOtherCredentialKind: [kind: "accessToken" | "sshKey"];
  updateRepoName: [name: string];
  updateVisibility: [visibility: "private" | "public"];
  createRepo: [];
  repoSelected: [remoteUrl: string];
  submitPasteUrlToken: [remoteUrl: string, username: string, token: string];
  submitPasteUrlSshKey: [remoteUrl: string];
  generateSshKey: [];
  importSshKey: [];
  retry: [];
  commitAuthorConfirmed: [name: string, email: string];
}>();

function providerLabel(provider: "github" | "gitlab" | "other" | null): string {
  if (provider === "github") return "GitHub";
  if (provider === "gitlab") return "GitLab";
  return "this provider";
}

// The paste-URL step collects its own input (spec.md#native-dialogs:
// "surfaces with their own input UI keep doing so") and validates locally
// before emitting -- same behavior as the old inline render function, and
// the same shape the clone wizard's `CloneWizard.vue` already uses.
const pasteUrl = ref("");
const pasteTokenUsername = ref("");
const pasteTokenValue = ref("");
const pasteUrlError = ref<string | null>(null);

function submitPasteUrlToken() {
  const remoteUrl = pasteUrl.value.trim();
  const username = pasteTokenUsername.value.trim();
  const token = pasteTokenValue.value;
  if (!remoteUrl || !username || !token) {
    pasteUrlError.value = "Repository URL, username, and access token are all required.";
    return;
  }
  pasteUrlError.value = null;
  emit("submitPasteUrlToken", remoteUrl, username, token);
}

function submitPasteUrlSshKey() {
  const remoteUrl = pasteUrl.value.trim();
  if (!remoteUrl) {
    pasteUrlError.value = "Enter the repository's URL first.";
    return;
  }
  pasteUrlError.value = null;
  emit("submitPasteUrlSshKey", remoteUrl);
}
</script>

<template>
  <Modal
    label="Connect a repository"
    overlay-class="settings-modal-overlay"
    card-class="settings-modal connect-wizard"
    @close="emit('close')"
  >
    <div class="settings-modal-header">
      <h2>Connect a repository</h2>
      <button type="button" aria-label="Close" @click="emit('close')">✕</button>
    </div>

    <div class="connect-wizard-body">
      <template v-if="state.step === 'hasRepo'">
        <p class="wizard-question">Do you already have a repository?</p>
        <div class="wizard-button-row">
          <button type="button" @click="emit('chooseHasRepo', true)">Yes, I have one</button>
          <button type="button" @click="emit('chooseHasRepo', false)">No, create one</button>
        </div>
      </template>

      <template v-else-if="state.step === 'providerChoice'">
        <p class="wizard-question">Which provider is the repository on?</p>
        <div class="wizard-button-row">
          <button type="button" @click="emit('chooseProvider', 'github')">GitHub</button>
          <button type="button" @click="emit('chooseProvider', 'gitlab')">GitLab</button>
          <button v-if="state.hasRepo" type="button" @click="emit('chooseProvider', 'other')">
            Another provider
          </button>
        </div>
      </template>

      <template v-else-if="state.step === 'credentialChoice'">
        <p class="wizard-question">How do you want to connect to {{ providerLabel(state.provider) }}?</p>
        <button type="button" class="wizard-primary-action" @click="emit('chooseOauthSignIn')">
          Sign in with {{ providerLabel(state.provider) }}
        </button>
        <button type="button" class="wizard-secondary-action" @click="emit('chooseOtherWaysToConnect')">
          Other ways to connect
        </button>
      </template>

      <template v-else-if="state.step === 'otherCredentialKindChoice'">
        <p class="wizard-question">How do you want to authenticate?</p>
        <div class="wizard-button-row">
          <button type="button" @click="emit('chooseOtherCredentialKind', 'accessToken')">Access token</button>
          <button type="button" @click="emit('chooseOtherCredentialKind', 'sshKey')">SSH key</button>
        </div>
      </template>

      <template v-else-if="state.step === 'oauthSignIn'">
        <DeviceFlow :status="oauthStatus" :device-code="oauthDeviceCode" />
      </template>

      <template v-else-if="state.step === 'repoVisibility'">
        <p class="wizard-question">Name the new repository:</p>
        <input
          type="text"
          placeholder="my-notes"
          :value="state.repoName"
          @input="emit('updateRepoName', ($event.target as HTMLInputElement).value)"
        />
        <div class="wizard-button-row">
          <label>
            <input
              type="radio"
              name="connect-wizard-visibility"
              :checked="state.visibility === 'private'"
              @change="emit('updateVisibility', 'private')"
            />
            Private (recommended)
          </label>
          <label>
            <input
              type="radio"
              name="connect-wizard-visibility"
              :checked="state.visibility === 'public'"
              @change="emit('updateVisibility', 'public')"
            />
            Public
          </label>
        </div>
        <button type="button" class="wizard-primary-action" @click="emit('createRepo')">Create repository</button>
        <p v-if="createRepoError" class="error wizard-inline-error">{{ createRepoError }}</p>
      </template>

      <template v-else-if="state.step === 'repoPicker'">
        <p class="settings-connect-status">{{ repoPickerStatus }}</p>
        <ul class="wizard-repo-list">
          <li v-for="repo in repos" :key="repo.fullName">
            <button type="button" class="wizard-repo-item" @click="emit('repoSelected', repo.cloneUrl)">
              <span class="wizard-repo-name">{{ repo.fullName }}</span>
              <span class="wizard-repo-visibility">{{ repo.private ? "Private" : "Public" }}</span>
            </button>
          </li>
        </ul>
      </template>

      <template v-else-if="state.step === 'pasteUrl'">
        <p class="wizard-question">Enter the repository's URL:</p>
        <input v-model="pasteUrl" type="text" placeholder="https://example.com/user/repo.git" />

        <template v-if="state.credentialKind === 'accessToken'">
          <input v-model="pasteTokenUsername" type="text" placeholder="Username" />
          <input v-model="pasteTokenValue" type="password" placeholder="Access token" />
          <button type="button" class="wizard-primary-action" @click="submitPasteUrlToken()">Connect</button>
          <p v-if="pasteUrlError" class="error wizard-inline-error">{{ pasteUrlError }}</p>
        </template>

        <template v-else>
          <SshKey :status="sshKeyStatus" @generate="emit('generateSshKey')" @import="emit('importSshKey')" />
          <button type="button" class="wizard-primary-action" @click="submitPasteUrlSshKey()">Connect</button>
          <p v-if="pasteUrlError" class="error wizard-inline-error">{{ pasteUrlError }}</p>
        </template>
      </template>

      <template v-else-if="state.step === 'connecting'">
        <p class="settings-connect-status">Connecting…</p>
      </template>

      <template v-else-if="state.step === 'connectError'">
        <p class="error">{{ state.error ?? "Couldn't connect." }}</p>
        <button type="button" class="wizard-primary-action" @click="emit('retry')">Try again</button>
      </template>

      <template v-else-if="state.step === 'commitAuthor'">
        <CommitAs
          :prefill="authorPrefill"
          :error="commitAuthorError"
          @submit="(name, email) => emit('commitAuthorConfirmed', name, email)"
        />
      </template>
    </div>

    <div class="connect-wizard-footer">
      <button v-if="canGoBack" type="button" @click="emit('back')">Back</button>
      <button type="button" @click="emit('switchToManual')">Switch to manual setup</button>
    </div>
  </Modal>
</template>
