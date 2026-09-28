<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. Renders
// `#clone-wizard-overlay`'s old markup/classes verbatim (spec.md#step-9-
// clone-wizard--clone-manual-form) -- full-screen, not a modal (there is no
// vault/window yet to anchor a modal on top of). Device flow, the SSH key
// sub-form, and "Commit as" were written inline here as the first copies;
// ticket 11's connect wizard is the second copy of each, so they're now
// `components/DeviceFlow.vue`/`SshKey.vue`/`CommitAs.vue` and this surface
// switches to them below.
import { ref } from "vue";
import { isCloneWizardBusyStep, type CloneWizardState } from "../../clone-wizard";
import type { WizardProvider } from "../../connect-wizard";
import DeviceFlow, { type DeviceCodeDisplay } from "../../components/DeviceFlow.vue";
import SshKey from "../../components/SshKey.vue";
import CommitAs, { type CommitAuthorValue } from "../../components/CommitAs.vue";

export type { CommitAuthorValue };

/** Just enough of `vault-api`'s `RepoInfo` to render a pickable list entry --
 * kept local rather than imported, per spec.md#surface-contract (this
 * presentational surface never imports `vault-api`). */
export interface CloneRepoOption {
  fullName: string;
  private: boolean;
  cloneUrl: string;
}

const props = defineProps<{
  state: CloneWizardState;
  canGoBack: boolean;
  oauthStatus: string;
  oauthDeviceCode: DeviceCodeDisplay | null;
  repos: CloneRepoOption[];
  repoPickerStatus: string;
  sshKeyStatus: string | null;
  destinationStatus: string | null;
  authorPrefill: CommitAuthorValue | null;
  commitAuthorError: string | null;
}>();

const emit = defineEmits<{
  close: [];
  back: [];
  switchToManual: [];
  chooseProvider: [provider: WizardProvider];
  chooseOauthSignIn: [];
  chooseOtherWaysToConnect: [];
  chooseOtherCredentialKind: [kind: "accessToken" | "sshKey"];
  repoSelected: [remoteUrl: string];
  submitPasteUrlToken: [remoteUrl: string, username: string, token: string];
  submitPasteUrlSshKey: [remoteUrl: string];
  generateSshKey: [];
  importSshKey: [];
  pickDestination: [];
  retry: [];
  commitAuthorConfirmed: [name: string, email: string];
}>();

function providerLabel(provider: WizardProvider | null): string {
  if (provider === "github") return "GitHub";
  if (provider === "gitlab") return "GitLab";
  return "this provider";
}

// The paste-URL step collects its own input (spec.md#native-dialogs:
// "surfaces with their own input UI keep doing so") and validates locally
// before emitting -- same behavior as the old inline render function.
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

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && !isCloneWizardBusyStep(props.state.step)) {
    event.preventDefault();
    emit("close");
  }
}
</script>

<template>
  <section class="clone-wizard-overlay" @keydown="handleKeydown($event)">
    <div id="clone-wizard" class="clone-wizard" role="dialog" aria-modal="true" aria-label="Clone a repository">
      <div class="clone-wizard-header">
        <h1>Clone a repository</h1>
        <button type="button" aria-label="Cancel" @click="emit('close')">✕</button>
      </div>

      <div class="clone-wizard-body">
        <template v-if="state.step === 'providerChoice'">
          <p class="wizard-question">Which provider is the repository on?</p>
          <div class="wizard-button-row">
            <button type="button" @click="emit('chooseProvider', 'github')">GitHub</button>
            <button type="button" @click="emit('chooseProvider', 'gitlab')">GitLab</button>
            <button type="button" @click="emit('chooseProvider', 'other')">Another provider</button>
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
            <button type="button" class="wizard-primary-action" @click="submitPasteUrlToken()">Continue</button>
            <p v-if="pasteUrlError" class="error wizard-inline-error">{{ pasteUrlError }}</p>
          </template>

          <template v-else>
            <SshKey :status="sshKeyStatus" @generate="emit('generateSshKey')" @import="emit('importSshKey')" />
            <button type="button" class="wizard-primary-action" @click="submitPasteUrlSshKey()">Continue</button>
            <p v-if="pasteUrlError" class="error wizard-inline-error">{{ pasteUrlError }}</p>
          </template>
        </template>

        <template v-else-if="state.step === 'destinationPicker'">
          <p class="wizard-question">Pick an empty folder to clone into:</p>
          <p class="settings-connect-status">{{ destinationStatus }}</p>
          <button type="button" class="wizard-primary-action" @click="emit('pickDestination')">Choose folder…</button>
        </template>

        <template v-else-if="state.step === 'cloning'">
          <p class="settings-connect-status">Cloning…</p>
        </template>

        <template v-else-if="state.step === 'cloneError'">
          <p class="error">{{ state.error ?? "Couldn't clone." }}</p>
          <button type="button" class="wizard-primary-action" @click="emit('retry')">
            Pick a different folder and try again
          </button>
        </template>

        <template v-else-if="state.step === 'commitAuthor'">
          <CommitAs
            :prefill="authorPrefill"
            :error="commitAuthorError"
            @submit="(name, email) => emit('commitAuthorConfirmed', name, email)"
          />
        </template>
      </div>

      <div class="clone-wizard-footer">
        <button v-if="canGoBack" type="button" @click="emit('back')">Back</button>
        <button type="button" @click="emit('switchToManual')">Switch to manual setup</button>
      </div>
    </div>
  </section>
</template>
