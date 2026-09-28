<script setup lang="ts">
// Container (spec.md#surface-contract): thin wrapper mounting `useCloneWizard`
// and feeding it to the presentational `CloneWizard.vue`, shown whenever
// `ui.vaultView` is `'cloneWizard'` -- replacing the old `#clone-wizard-
// overlay`'s direct `hidden`-attribute toggling (spec.md#step-9-clone-
// wizard--clone-manual-form). No story (containers aren't storied).
import { watch } from "vue";
import { vaultView } from "../../state/ui";
import { useCloneWizard } from "./useCloneWizard";
import CloneWizard from "./CloneWizard.vue";

const wizard = useCloneWizard();

// (Re)starts the wizard fresh every time this view becomes active -- same
// reset the old `openCloneWizard()` did on every open.
watch(
  vaultView,
  (view) => {
    if (view === "cloneWizard") wizard.open();
  },
);
</script>

<template>
  <CloneWizard
    v-if="vaultView === 'cloneWizard'"
    :state="wizard.state.value"
    :can-go-back="wizard.canGoBack.value"
    :oauth-status="wizard.oauthStatus.value"
    :oauth-device-code="wizard.oauthDeviceCode.value"
    :repos="wizard.repos.value"
    :repo-picker-status="wizard.repoPickerStatus.value"
    :ssh-key-status="wizard.sshKeyStatus.value"
    :destination-status="wizard.destinationStatus.value"
    :author-prefill="wizard.authorPrefill.value"
    :commit-author-error="wizard.commitAuthorError.value"
    @close="wizard.close()"
    @back="wizard.back()"
    @switch-to-manual="wizard.switchToManual()"
    @choose-provider="wizard.chooseProvider($event)"
    @choose-oauth-sign-in="wizard.chooseOauthSignIn()"
    @choose-other-ways-to-connect="wizard.chooseOtherWaysToConnect()"
    @choose-other-credential-kind="wizard.chooseOtherCredentialKind($event)"
    @repo-selected="wizard.repoSelected($event)"
    @submit-paste-url-token="
      (remoteUrl: string, username: string, token: string) => wizard.submitPasteUrlToken(remoteUrl, username, token)
    "
    @submit-paste-url-ssh-key="wizard.submitPasteUrlSshKey($event)"
    @generate-ssh-key="wizard.generateSshKeyStep()"
    @import-ssh-key="wizard.importSshKeyStep()"
    @pick-destination="wizard.pickDestination()"
    @retry="wizard.retry()"
    @commit-author-confirmed="(name: string, email: string) => wizard.confirmCommitAuthorStep(name, email)"
  />
</template>
