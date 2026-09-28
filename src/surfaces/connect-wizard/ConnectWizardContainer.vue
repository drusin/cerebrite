<script setup lang="ts">
// Container (spec.md#surface-contract): thin wrapper mounting
// `useConnectWizard` and feeding it to the presentational `ConnectWizard.vue`,
// shown whenever `ui.modal` is `'connectWizard'` -- replacing the old
// `#connect-wizard-overlay`'s direct `hidden`-attribute toggling
// (spec.md#step-10-connect-wizard). No story (containers aren't storied).
import { watch } from "vue";
import { modal } from "../../state/ui";
import { useConnectWizard } from "./useConnectWizard";
import ConnectWizard from "./ConnectWizard.vue";

// Temporary callback root prop (spec.md#islands-and-how-they-merge):
// Settings hasn't migrated to Vue yet, so its "Commit as" fields are still
// vanilla DOM this composable can't reach directly -- see
// `useConnectWizard.ts`'s `ConnectWizardOptions` doc comment. Delete this
// indirection once Settings migrates (ticket 12).
const props = defineProps<{
  refreshCommitAuthorFieldsInVanilla: () => Promise<void>;
}>();

const wizard = useConnectWizard({
  refreshCommitAuthorFieldsInVanilla: () => props.refreshCommitAuthorFieldsInVanilla(),
});

// (Re)starts the wizard fresh every time it's opened -- same reset the old
// `openConnectWizard()` did on every open.
watch(modal, (value) => {
  if (value === "connectWizard") wizard.open();
});
</script>

<template>
  <ConnectWizard
    v-if="modal === 'connectWizard'"
    :state="wizard.state.value"
    :can-go-back="wizard.canGoBack.value"
    :oauth-status="wizard.oauthStatus.value"
    :oauth-device-code="wizard.oauthDeviceCode.value"
    :repos="wizard.repos.value"
    :repo-picker-status="wizard.repoPickerStatus.value"
    :ssh-key-status="wizard.sshKeyStatus.value"
    :create-repo-error="wizard.createRepoError.value"
    :author-prefill="wizard.authorPrefill.value"
    :commit-author-error="wizard.commitAuthorError.value"
    @close="wizard.close()"
    @back="wizard.back()"
    @switch-to-manual="wizard.switchToManual()"
    @choose-has-repo="wizard.chooseHasRepo($event)"
    @choose-provider="wizard.chooseProvider($event)"
    @choose-oauth-sign-in="wizard.chooseOauthSignIn()"
    @choose-other-ways-to-connect="wizard.chooseOtherWaysToConnect()"
    @choose-other-credential-kind="wizard.chooseOtherCredentialKind($event)"
    @update-repo-name="wizard.setRepoName($event)"
    @update-visibility="wizard.setVisibility($event)"
    @create-repo="wizard.createRepo()"
    @repo-selected="wizard.repoSelected($event)"
    @submit-paste-url-token="
      (remoteUrl: string, username: string, token: string) => wizard.submitPasteUrlToken(remoteUrl, username, token)
    "
    @submit-paste-url-ssh-key="wizard.submitPasteUrlSshKey($event)"
    @generate-ssh-key="wizard.generateSshKeyStep()"
    @import-ssh-key="wizard.importSshKeyStep()"
    @retry="wizard.retry()"
    @commit-author-confirmed="(name: string, email: string) => wizard.confirmCommitAuthorStep(name, email)"
  />
</template>
