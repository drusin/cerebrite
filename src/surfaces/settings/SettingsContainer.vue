<script setup lang="ts">
// Container (spec.md#surface-contract): owns every field/status this
// surface needs (all local to this container, not `src/state/`, mirroring
// every other container's ephemeral-data pattern) and every backend/dialog
// call, shown whenever `ui.modal` is `'settings'` -- replacing the old
// `#settings-modal-overlay`'s direct `hidden`-attribute toggling and every
// handler that used to live in main.ts (spec.md#step-11-settings). No story
// (containers aren't storied).
import { computed, nextTick, ref, watch } from "vue";
import { modal, settingsDeepLink, closeModal, openConnectWizardModal } from "../../state/ui";
import { vaultPath, changeFolder } from "../../state/vault";
import { theme, setTheme } from "../../state/theme";
import { syncStatus, refreshSyncStatus } from "../../state/sync";
import { friendlyVaultOpenError } from "../../vault-open-error";
import {
  pickVaultFolder,
  commitAuthorPrefill as commitAuthorPrefillCommand,
  getCommitAuthor,
  confirmCommitAuthor,
  connectAccessToken,
  generateSshKey,
  importSshKey,
  connectSshKey,
  startGithubDeviceFlow,
  pollGithubDeviceFlow,
  checkGithubInstallation,
  connectGithubOauth,
  startGitlabDeviceFlow,
  pollGitlabDeviceFlow,
  connectGitlabOauth,
  disconnectVault,
  scanOrphanedConnections,
  cleanupOrphanedConnections,
  removeAllStoredCredentials,
  type DisconnectOutcome,
  type Provider,
  type CommitAuthor,
  type SshKeyInfo,
  type CredentialKind,
} from "../../vault-api";
import { messageDialog, promptDialog, confirmBrowser, alertBrowser, withPlaintextFallbackConsent } from "../../dialogs";
import SettingsSurface, { type ThemeOption } from "./SettingsSurface.vue";
import type { CommitAuthorValue } from "../../components/CommitAs.vue";
import type { CredentialKindOption } from "../../components/CredentialKindForm.vue";
import type { DeviceCodeDisplay } from "../../components/DeviceFlow.vue";

const isOpen = computed(() => modal.value === "settings");
const syncConnected = computed(() => syncStatus.value.state !== "noRemote");

// --- "Commit as" -------------------------------------------------------

const commitAuthorPrefillValue = ref<CommitAuthorValue | null>(null);
const commitAuthorError = ref<string | null>(null);
const commitAuthorSavedMessage = ref<string | null>(null);

/** Ticket 08's exact prefill precedence (repo-local -> global -> empty),
 * same as every other door into it -- replaces main.ts's own
 * `refreshCommitAuthorFields`. Silently does nothing if no vault is open
 * yet (the commands themselves require one). */
async function refreshCommitAuthorFields() {
  commitAuthorError.value = null;
  commitAuthorSavedMessage.value = null;
  try {
    const confirmed = await getCommitAuthor();
    const author: CommitAuthor | null = confirmed ?? (await commitAuthorPrefillCommand()).author;
    commitAuthorPrefillValue.value = author ? { name: author.name, email: author.email } : null;
  } catch {
    commitAuthorPrefillValue.value = null;
  }
}

async function handleCommitAuthorSubmit(name: string, email: string) {
  try {
    const result = await confirmCommitAuthor(name, email);
    commitAuthorError.value = null;
    commitAuthorSavedMessage.value = result.warning ? `Saved. ${result.warning}` : "Saved.";
  } catch (err) {
    commitAuthorSavedMessage.value = null;
    commitAuthorError.value = String(err);
  }
}

// --- Credentials ("Sync" section's always-visible sub-forms) -----------

const credentialKind = ref<CredentialKindOption>("accessToken");
const remoteUrl = ref("");
const tokenUsername = ref("");
const tokenValue = ref("");
const accessTokenStatus = ref<string | null>(null);
const accessTokenConnecting = ref(false);
const sshKeyStatus = ref<string | null>(null);
const sshKeyConnecting = ref(false);
const githubDeviceCode = ref<DeviceCodeDisplay | null>(null);
const githubStatus = ref<string | null>(null);
const githubInstallUrl = ref<string | null>(null);
const githubInstallContinuing = ref(false);
const gitlabDeviceCode = ref<DeviceCodeDisplay | null>(null);
const gitlabStatus = ref<string | null>(null);

let sshKey: SshKeyInfo | null = null;
/// Holds the acquired token pair between "device flow finished" and "the
/// user confirmed the app install" -- mirrors main.ts's old module-scope
/// `pendingGithubTokenPair`.
let pendingGithubTokenPair: { accessToken: string; refreshToken: string; accessTokenExpiresAt: string } | null = null;
/** Bumped on every open so a stale device-flow poll loop from a previous
 * attempt can tell it's been abandoned -- same pattern as
 * `useCloneWizard`/`CloneManualFormContainer`. */
let generation = 0;

/** Maps a connection's stored `credentialKind` (+ `provider`, for
 * `oauth_sign_in`) to this surface's own sub-form kind -- same mapping as
 * main.ts's old `syncSubformKindFor`. */
function credentialKindOptionFor(kind: CredentialKind | null, provider: string | null): CredentialKindOption {
  switch (kind) {
    case "ssh_key":
      return "sshKey";
    case "oauth_sign_in":
      return provider === "GitLab" ? "gitlabOauth" : "githubOauth";
    case "access_token":
    case null:
      return "accessToken";
  }
}

/** Applies an `openSettings({ section: 'sync', prefillUrl, credentialKind })`
 * deep link -- replaces main.ts's old `openSettingsInVanilla`. Re-asking for
 * the credential itself (rather than restoring it) is an accepted,
 * pre-existing limitation (ticket 11). */
watch(settingsDeepLink, (options) => {
  if (options.section !== "sync") return;
  if (options.prefillUrl) remoteUrl.value = options.prefillUrl;
  if (options.credentialKind !== undefined) {
    credentialKind.value = credentialKindOptionFor(options.credentialKind, options.provider ?? null);
  }
  void nextTick(() => surfaceRef.value?.scrollToSyncSection());
});

async function handleGenerateSshKey() {
  sshKeyStatus.value = "Generating…";
  try {
    sshKey = await generateSshKey();
    sshKeyStatus.value =
      `Key ready (fingerprint ${sshKey.fingerprintSha256}). Add the public key to your provider, then Connect.`;
  } catch (err) {
    sshKeyStatus.value = String(err);
  }
}

async function handleImportSshKey() {
  const privateKeyOpenssh = promptDialog("Paste the private key (OpenSSH format):");
  if (privateKeyOpenssh === null || !privateKeyOpenssh.trim()) return;
  const passphrase = promptDialog("Passphrase (leave blank if none):") ?? undefined;

  sshKeyStatus.value = "Importing…";
  try {
    sshKey = await importSshKey(privateKeyOpenssh, passphrase || undefined);
    sshKeyStatus.value =
      `Key imported (fingerprint ${sshKey.fingerprintSha256}). Add the public key to your provider, then Connect.`;
  } catch (err) {
    sshKeyStatus.value = String(err);
  }
}

/** The access-token/SSH-key sub-forms' shared "Connect" action. */
async function handleConnectAccessTokenOrSshKey() {
  if (credentialKind.value === "accessToken") {
    const username = tokenUsername.value.trim();
    const token = tokenValue.value;
    accessTokenConnecting.value = true;
    accessTokenStatus.value = "Connecting…";
    try {
      await withPlaintextFallbackConsent((allow) => connectAccessToken(remoteUrl.value.trim(), username, token, allow));
      accessTokenStatus.value = "Connected.";
      tokenValue.value = "";
    } catch (err) {
      accessTokenStatus.value = String(err);
    } finally {
      accessTokenConnecting.value = false;
      await afterConnectAttempt();
    }
    return;
  }

  if (!remoteUrl.value.trim()) {
    sshKeyStatus.value = "Enter the repository's URL first.";
    return;
  }
  if (!sshKey) {
    sshKeyStatus.value = "Generate (or import) a key first.";
    return;
  }
  sshKeyConnecting.value = true;
  sshKeyStatus.value = "Connecting…";
  try {
    await withPlaintextFallbackConsent((allow) =>
      connectSshKey(remoteUrl.value.trim(), sshKey!.privateKeyOpenssh, sshKey!.passphrase, allow),
    );
    sshKeyStatus.value = "Connected.";
  } catch (err) {
    sshKeyStatus.value = String(err);
  } finally {
    sshKeyConnecting.value = false;
    await afterConnectAttempt();
  }
}

async function afterConnectAttempt() {
  await refreshSyncStatus();
}

/** Ticket 06's device-flow "Sign in with GitHub" -- unchanged from
 * main.ts's old `handleGithubFormSubmit`/`finishGithubConnect`. */
async function handleGithubSignIn() {
  const remote = remoteUrl.value.trim();
  if (!remote) {
    githubStatus.value = "Enter the repository's URL first.";
    return;
  }

  githubInstallUrl.value = null;
  githubDeviceCode.value = null;
  pendingGithubTokenPair = null;
  const gen = ++generation;

  try {
    githubStatus.value = "Requesting a device code from GitHub…";
    const device = await startGithubDeviceFlow();
    if (gen !== generation) return;

    githubDeviceCode.value = { verificationUri: device.verificationUri, userCode: device.userCode };
    githubStatus.value = "Waiting for you to approve in the browser…";

    const deadline = Date.now() + device.expiresInSecs * 1000;
    let intervalMs = Math.max(device.intervalSecs, 1) * 1000;

    for (;;) {
      if (gen !== generation) return;
      if (Date.now() >= deadline) throw new Error("The GitHub sign-in code expired before it was confirmed.");
      await new Promise((resolve) => setTimeout(resolve, intervalMs));
      if (gen !== generation) return;

      const result = await pollGithubDeviceFlow(device.deviceCode);
      if (result.outcome === "pending") continue;
      if (result.outcome === "slowDown") {
        intervalMs += 5000;
        continue;
      }
      if (result.outcome === "denied") throw new Error("GitHub sign-in was denied.");
      if (result.outcome === "expired") throw new Error("The GitHub sign-in code expired before it was confirmed.");
      if (result.outcome === "error") throw new Error(result.message);

      pendingGithubTokenPair = {
        accessToken: result.accessToken,
        refreshToken: result.refreshToken,
        accessTokenExpiresAt: result.accessTokenExpiresAt,
      };
      break;
    }

    githubDeviceCode.value = null;
    await finishGithubConnect(remote, gen);
  } catch (err) {
    if (gen !== generation) return;
    githubDeviceCode.value = null;
    githubStatus.value = String(err);
  }
}

/** Shared tail of the GitHub sign-in flow, called once a token pair is in
 * hand: checks the app's installation on the repo and either shows the
 * install CTA or finishes the real test-fetch-then-persist connect. Also
 * the retry path `handleGithubInstallContinue` calls. */
async function finishGithubConnect(remoteUrlValue: string, gen: number) {
  if (!pendingGithubTokenPair) return;
  const { accessToken, refreshToken, accessTokenExpiresAt } = pendingGithubTokenPair;

  githubStatus.value = "Checking whether Cerebrite is installed on this repository…";
  const installation = await checkGithubInstallation(remoteUrlValue, accessToken);
  if (gen !== generation) return;
  if (installation.status === "notInstalled") {
    githubInstallUrl.value = installation.installUrl;
    githubStatus.value = null;
    return;
  }

  githubInstallUrl.value = null;
  githubStatus.value = "Connecting…";
  await withPlaintextFallbackConsent((allow) =>
    connectGithubOauth(remoteUrlValue, accessToken, refreshToken, accessTokenExpiresAt, allow),
  );
  if (gen !== generation) return;
  githubStatus.value = "Connected.";
  pendingGithubTokenPair = null;
  await afterConnectAttempt();
}

async function handleGithubInstallContinue() {
  const remote = remoteUrl.value.trim();
  githubInstallContinuing.value = true;
  const gen = generation;
  try {
    await finishGithubConnect(remote, gen);
  } catch (err) {
    if (gen !== generation) return;
    githubStatus.value = String(err);
  } finally {
    githubInstallContinuing.value = false;
  }
}

/** Ticket 07's device-flow "Sign in with GitLab" -- unchanged from
 * main.ts's old `handleGitlabFormSubmit`: no install-CTA detour, connects
 * straight away once a token pair is acquired. */
async function handleGitlabSignIn() {
  const remote = remoteUrl.value.trim();
  if (!remote) {
    gitlabStatus.value = "Enter the repository's URL first.";
    return;
  }

  gitlabDeviceCode.value = null;
  const gen = ++generation;

  try {
    gitlabStatus.value = "Requesting a device code from GitLab…";
    const device = await startGitlabDeviceFlow();
    if (gen !== generation) return;

    gitlabDeviceCode.value = { verificationUri: device.verificationUri, userCode: device.userCode };
    gitlabStatus.value = "Waiting for you to approve in the browser…";

    const deadline = Date.now() + device.expiresInSecs * 1000;
    let intervalMs = Math.max(device.intervalSecs, 1) * 1000;
    let accessToken: string;
    let refreshToken: string | undefined;
    let accessTokenExpiresAt: string;

    for (;;) {
      if (gen !== generation) return;
      if (Date.now() >= deadline) throw new Error("The GitLab sign-in code expired before it was confirmed.");
      await new Promise((resolve) => setTimeout(resolve, intervalMs));
      if (gen !== generation) return;

      const result = await pollGitlabDeviceFlow(device.deviceCode);
      if (result.outcome === "pending") continue;
      if (result.outcome === "slowDown") {
        intervalMs += 5000;
        continue;
      }
      if (result.outcome === "denied") throw new Error("GitLab sign-in was denied.");
      if (result.outcome === "expired") throw new Error("The GitLab sign-in code expired before it was confirmed.");
      if (result.outcome === "error") throw new Error(result.message);

      accessToken = result.accessToken;
      refreshToken = result.refreshToken;
      accessTokenExpiresAt = result.accessTokenExpiresAt;
      break;
    }

    gitlabDeviceCode.value = null;
    gitlabStatus.value = "Connecting…";
    await withPlaintextFallbackConsent((allow) =>
      connectGitlabOauth(remote, accessToken, refreshToken, accessTokenExpiresAt, allow),
    );
    if (gen !== generation) return;
    gitlabStatus.value = "Connected.";
    await afterConnectAttempt();
  } catch (err) {
    if (gen !== generation) return;
    gitlabDeviceCode.value = null;
    gitlabStatus.value = String(err);
  }
}

// --- Disconnect (ticket 14) ----------------------------------------------

const GITHUB_INSTALLATIONS_URL = "https://github.com/settings/installations";
const GITLAB_AUTHORIZED_APPS_URL = "https://gitlab.com/-/profile/applications";

function revocationLink(provider: Provider, kind: "token" | "sshKey"): string | null {
  if (provider.kind === "git_hub") {
    return kind === "token" ? "https://github.com/settings/tokens" : "https://github.com/settings/keys";
  }
  if (provider.kind === "git_lab") {
    return kind === "token"
      ? "https://gitlab.com/-/user_settings/personal_access_tokens"
      : "https://gitlab.com/-/user_settings/ssh_keys";
  }
  return null;
}

/** Builds the Disconnect result message honestly, per credential kind --
 * unchanged from main.ts's old `disconnectResultMessage`. */
function disconnectResultMessage(outcome: DisconnectOutcome): string {
  if (outcome.credentialKind === "oauth_sign_in" && outcome.provider.kind === "git_lab") {
    if (outcome.gitlabRevoked === true) {
      return "Disconnected. Cerebrite revoked your GitLab sign-in token.";
    }
    return (
      "Disconnected locally. Cerebrite could not confirm your GitLab sign-in token was " +
      `revoked -- it may still be valid at GitLab. Revoke it yourself at: ${GITLAB_AUTHORIZED_APPS_URL}`
    );
  }
  if (outcome.credentialKind === "oauth_sign_in" && outcome.provider.kind === "git_hub") {
    return (
      "Disconnected locally. GitHub App revocation needs a client secret Cerebrite doesn't " +
      `hold. Review or revoke Cerebrite's access yourself at: ${GITHUB_INSTALLATIONS_URL}`
    );
  }
  const linkKind = outcome.credentialKind === "ssh_key" ? "sshKey" : "token";
  const link = revocationLink(outcome.provider, linkKind);
  if (link) {
    return `Removed locally -- revoke it yourself at: ${link}`;
  }
  const what = outcome.credentialKind === "ssh_key" ? "SSH key" : "access token";
  return `Removed locally -- revoke this ${what} yourself at your git host's settings.`;
}

async function handleDisconnect() {
  const confirmed = confirmBrowser(
    "Disconnect this vault from its remote repository? Cerebrite deletes the stored " +
      "credential and connection record from this device. This can't be undone from within " +
      "Cerebrite.",
  );
  if (!confirmed) return;

  try {
    const outcome = await disconnectVault();
    alertBrowser(outcome ? disconnectResultMessage(outcome) : "Nothing was connected.");
  } catch (err) {
    alertBrowser(`Couldn't disconnect: ${err}`);
  } finally {
    await refreshSyncStatus();
  }
}

// --- Credentials section (orphan notice + remove all) --------------------

const orphanVisible = ref(false);
const orphanCleaning = ref(false);
const removeAllStatus = ref<string | null>(null);
const removeAllRemoving = ref(false);

async function refreshOrphanNotice() {
  try {
    const orphans = await scanOrphanedConnections();
    orphanVisible.value = orphans.length > 0;
  } catch {
    orphanVisible.value = false;
  }
}

async function handleOrphanCleanup() {
  orphanCleaning.value = true;
  try {
    await cleanupOrphanedConnections();
  } catch (err) {
    alertBrowser(`Couldn't clean up orphaned credentials: ${err}`);
  } finally {
    orphanCleaning.value = false;
    await refreshOrphanNotice();
  }
}

async function handleRemoveAllCredentials() {
  const confirmed = confirmBrowser(
    "Remove all stored Cerebrite credentials? This permanently deletes every credential " +
      "Cerebrite has stored on this device, for every vault it has ever connected -- not just " +
      "this one. This does not revoke anything at the provider, and can't be undone from " +
      "within Cerebrite.",
  );
  if (!confirmed) return;

  removeAllRemoving.value = true;
  removeAllStatus.value = null;
  try {
    const failed = await removeAllStoredCredentials();
    removeAllStatus.value =
      failed.length === 0
        ? "All stored credentials were removed."
        : `Removed what it could -- ${failed.length} connection(s) could not be fully removed.`;
  } catch (err) {
    removeAllStatus.value = String(err);
  } finally {
    removeAllRemoving.value = false;
    await refreshOrphanNotice();
    await refreshSyncStatus();
  }
}

// --- Vault folder ----------------------------------------------------------

async function handleChangeFolder() {
  let path: string | null;
  try {
    path = await pickVaultFolder();
  } catch (err) {
    await messageDialog(String(err));
    return;
  }
  if (!path) return; // user cancelled

  closeModal();
  try {
    await changeFolder(path);
  } catch (err) {
    await messageDialog(friendlyVaultOpenError(String(err)));
  }
}

// --- Open/close --------------------------------------------------------

// Refreshes whatever can go stale while Settings was closed, every time it
// (re)opens -- replaces main.ts's old `openSettingsModal`'s two `void`
// calls. Also covers reopening from the connect wizard (`modal` returning
// to `'settings'`), which is what retires the connect wizard's old
// `refreshCommitAuthorFieldsInVanilla` callback -- see `useConnectWizard.ts`'s
// doc comment.
watch(modal, (value) => {
  if (value !== "settings") return;
  void refreshCommitAuthorFields();
  void refreshOrphanNotice();
});

function handleThemeUpdate(next: ThemeOption) {
  void setTheme(next);
}

const surfaceRef = ref<InstanceType<typeof SettingsSurface> | null>(null);
</script>

<template>
  <SettingsSurface
    v-if="isOpen"
    ref="surfaceRef"
    :vault-path="vaultPath"
    :theme="theme"
    :commit-author-prefill="commitAuthorPrefillValue"
    :commit-author-error="commitAuthorError"
    :commit-author-saved-message="commitAuthorSavedMessage"
    :sync-connected="syncConnected"
    :credential-kind="credentialKind"
    :remote-url="remoteUrl"
    :token-username="tokenUsername"
    :token-value="tokenValue"
    :access-token-status="accessTokenStatus"
    :access-token-connecting="accessTokenConnecting"
    :ssh-key-status="sshKeyStatus"
    :ssh-key-connecting="sshKeyConnecting"
    :github-device-code="githubDeviceCode"
    :github-status="githubStatus"
    :github-install-url="githubInstallUrl"
    :github-install-continuing="githubInstallContinuing"
    :gitlab-device-code="gitlabDeviceCode"
    :gitlab-status="gitlabStatus"
    :orphan-visible="orphanVisible"
    :orphan-cleaning="orphanCleaning"
    :remove-all-status="removeAllStatus"
    :remove-all-removing="removeAllRemoving"
    @close="closeModal()"
    @change-folder="handleChangeFolder()"
    @update:theme="handleThemeUpdate($event)"
    @commit-author-submit="handleCommitAuthorSubmit"
    @open-connect-wizard="openConnectWizardModal()"
    @disconnect="handleDisconnect()"
    @update:credential-kind="credentialKind = $event"
    @update:remote-url="remoteUrl = $event"
    @update:token-username="tokenUsername = $event"
    @update:token-value="tokenValue = $event"
    @connect-access-token-or-ssh-key="handleConnectAccessTokenOrSshKey()"
    @generate-ssh-key="handleGenerateSshKey()"
    @import-ssh-key="handleImportSshKey()"
    @github-sign-in="handleGithubSignIn()"
    @github-install-continue="handleGithubInstallContinue()"
    @gitlab-sign-in="handleGitlabSignIn()"
    @orphan-cleanup="handleOrphanCleanup()"
    @remove-all-credentials="handleRemoveAllCredentials()"
  />
</template>
