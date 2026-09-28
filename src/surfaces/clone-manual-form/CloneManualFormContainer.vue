<script setup lang="ts">
// Container (spec.md#surface-contract): owns every field/status this form
// needs (all local to this container, not `src/state/`, per spec.md#step-9-
// clone-wizard--clone-manual-form) and every backend call, shown whenever
// `ui.vaultView` is `'cloneManual'` -- replacing the old `#clone-manual-
// overlay`'s direct `hidden`-attribute toggling. No story (containers
// aren't storied).
import { computed, ref, watch } from "vue";
import { vaultView, setVaultView, openSettings } from "../../state/ui";
import { applyVaultOpened } from "../../state/vault";
import { takeCloneManualPrefill } from "../../clone-manual-handoff";
import { promptDialog } from "../../dialogs";
import {
  pickVaultFolder,
  generateSshKey,
  importSshKey,
  startGithubDeviceFlow,
  pollGithubDeviceFlow,
  checkGithubInstallation,
  startGitlabDeviceFlow,
  pollGitlabDeviceFlow,
  cloneAndOpenVault,
  type SshKeyInfo,
  type CloneCredential,
} from "../../vault-api";
import CloneManualForm, { type ManualCredentialKind } from "./CloneManualForm.vue";
import type { DeviceCodeDisplay } from "../../components/DeviceFlow.vue";

const isOpen = computed(() => vaultView.value === "cloneManual");

const remoteUrl = ref("");
const destination = ref("");
const credentialKind = ref<ManualCredentialKind>("accessToken");
const tokenUsername = ref("");
const tokenValue = ref("");
const sshKeyStatus = ref<string | null>(null);
const githubDeviceCode = ref<DeviceCodeDisplay | null>(null);
const githubStatus = ref<string | null>(null);
const gitlabDeviceCode = ref<DeviceCodeDisplay | null>(null);
const gitlabStatus = ref<string | null>(null);
const submitting = ref(false);
const statusMessage = ref<string | null>(null);

let sshKey: SshKeyInfo | null = null;
let githubToken: { accessToken: string; refreshToken: string; accessTokenExpiresAt: string } | null = null;
let gitlabToken: { accessToken: string; refreshToken?: string; accessTokenExpiresAt: string } | null = null;
/** Bumped on every open/close so a stale device-flow poll loop from a
 * previous attempt can tell it's been abandoned -- same pattern as
 * `useCloneWizard`'s own generation counter. */
let generation = 0;

function resetStatuses() {
  sshKeyStatus.value = null;
  githubStatus.value = null;
  gitlabStatus.value = null;
  githubDeviceCode.value = null;
  gitlabDeviceCode.value = null;
  statusMessage.value = null;
}

/** Resets every field fresh on every open -- carrying over the guided
 * wizard's "Switch to manual setup" handoff (`clone-manual-handoff.ts`)
 * when present, same as the old `openCloneManualDialog`'s optional
 * `prefillRemoteUrl`/`prefillDestination` arguments. */
watch(
  vaultView,
  (view) => {
    if (view !== "cloneManual") return;
    generation += 1;
    const prefill = takeCloneManualPrefill();
    remoteUrl.value = prefill?.remoteUrl ?? "";
    destination.value = prefill?.destination ?? "";
    credentialKind.value = "accessToken";
    tokenUsername.value = "";
    tokenValue.value = "";
    sshKey = null;
    githubToken = null;
    gitlabToken = null;
    resetStatuses();
  },
);

function close() {
  generation += 1; // invalidates any in-flight poll loop/clone
  setVaultView("picker");
}

async function handlePickDestination() {
  try {
    const path = await pickVaultFolder();
    if (!path) return; // user cancelled
    destination.value = path;
  } catch (err) {
    statusMessage.value = String(err);
  }
}

async function handleGenerateSshKey() {
  sshKeyStatus.value = "Generating…";
  try {
    sshKey = await generateSshKey();
    sshKeyStatus.value = `Key ready (fingerprint ${sshKey.fingerprintSha256}). Add the public key to your provider, then Clone.`;
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
    sshKeyStatus.value = `Key imported (fingerprint ${sshKey.fingerprintSha256}). Add the public key to your provider, then Clone.`;
  } catch (err) {
    sshKeyStatus.value = String(err);
  }
}

/** The device-flow sign-in shared shape (tickets 06/07), landing in
 * `githubToken`/`gitlabToken` instead of dispatching a wizard action -- this
 * form has no reducer of its own to drive. */
async function runOauthSignIn(provider: "github" | "gitlab") {
  const label = provider === "github" ? "GitHub" : "GitLab";
  const deviceCode = provider === "github" ? githubDeviceCode : gitlabDeviceCode;
  const status = provider === "github" ? githubStatus : gitlabStatus;
  const gen = generation;

  deviceCode.value = null;
  status.value = `Requesting a device code from ${label}…`;
  try {
    const device = provider === "github" ? await startGithubDeviceFlow() : await startGitlabDeviceFlow();
    if (gen !== generation) return;

    deviceCode.value = { verificationUri: device.verificationUri, userCode: device.userCode };
    status.value = "Waiting for you to approve in the browser…";

    const deadline = Date.now() + device.expiresInSecs * 1000;
    let intervalMs = Math.max(device.intervalSecs, 1) * 1000;
    let accessToken: string;
    let refreshToken: string | undefined;
    let accessTokenExpiresAt: string;

    for (;;) {
      if (gen !== generation) return;
      if (Date.now() >= deadline) throw new Error(`The ${label} sign-in code expired before it was confirmed.`);
      await new Promise((resolve) => setTimeout(resolve, intervalMs));
      if (gen !== generation) return;

      const result =
        provider === "github" ? await pollGithubDeviceFlow(device.deviceCode) : await pollGitlabDeviceFlow(device.deviceCode);
      if (result.outcome === "pending") continue;
      if (result.outcome === "slowDown") {
        intervalMs += 5000;
        continue;
      }
      if (result.outcome === "denied") throw new Error(`${label} sign-in was denied.`);
      if (result.outcome === "expired") throw new Error(`The ${label} sign-in code expired before it was confirmed.`);
      if (result.outcome === "error") throw new Error(result.message);

      accessToken = result.accessToken;
      refreshToken = result.refreshToken;
      accessTokenExpiresAt = result.accessTokenExpiresAt;
      break;
    }

    if (provider === "github") {
      githubToken = { accessToken, refreshToken: refreshToken ?? "", accessTokenExpiresAt };
    } else {
      gitlabToken = { accessToken, refreshToken, accessTokenExpiresAt };
    }
    deviceCode.value = null;
    status.value = "Signed in. Click Clone to continue.";
  } catch (err) {
    if (gen !== generation) return;
    deviceCode.value = null;
    status.value = String(err);
  }
}

/** Ticket 10 checklist item 4: the same test-fetch-before-save gate as every
 * other door into these mechanisms -- builds whichever `CloneCredential` the
 * selected kind needs (identical union `useCloneWizard`'s own `performClone`
 * builds) and calls `cloneAndOpenVault` directly. */
async function handleSubmit() {
  const url = remoteUrl.value.trim();
  const dest = destination.value.trim();

  if (!url) {
    statusMessage.value = "Repository URL is required.";
    return;
  }
  if (!dest) {
    statusMessage.value = "Choose a destination folder first.";
    return;
  }

  let credential: CloneCredential;
  try {
    if (credentialKind.value === "accessToken") {
      const username = tokenUsername.value.trim();
      const token = tokenValue.value;
      if (!username || !token) throw new Error("Username and access token are both required.");
      credential = { kind: "accessToken", username, token };
    } else if (credentialKind.value === "sshKey") {
      if (!sshKey) throw new Error("Generate or import an SSH key first.");
      credential = { kind: "sshKey", privateKeyOpenssh: sshKey.privateKeyOpenssh, passphrase: sshKey.passphrase };
    } else if (credentialKind.value === "githubOauth") {
      if (!githubToken) throw new Error("Sign in with GitHub first.");
      const installation = await checkGithubInstallation(url, githubToken.accessToken);
      if (installation.status === "notInstalled") {
        throw new Error(
          `Cerebrite isn't installed on this repository yet. Install it at ${installation.installUrl}, then try again.`,
        );
      }
      credential = { kind: "githubOauth", ...githubToken };
    } else {
      if (!gitlabToken) throw new Error("Sign in with GitLab first.");
      credential = { kind: "gitlabOauth", ...gitlabToken };
    }
  } catch (err) {
    statusMessage.value = String(err);
    return;
  }

  submitting.value = true;
  statusMessage.value = "Cloning…";
  const gen = generation;
  try {
    const result = await cloneAndOpenVault(url, dest, credential);
    if (gen !== generation) return;
    // Routes through the shared post-open tail (view switch to 'workspace' --
    // which also closes this form, since it's only shown for 'cloneManual' --
    // plus sync refresh and page/trash load) instead of duplicating it
    // inline. This is what fixes this call site's previously-missing
    // `refreshSyncStatus` call (spec.md#step-9-clone-wizard--clone-manual-
    // form's "incidental fix").
    await applyVaultOpened(result.vault.path);
    // Opens Settings so the user can confirm ticket 08's "Commit as" step --
    // Settings already shows it, prefilled, on open (still-vanilla Settings'
    // own `openSettingsModal` refreshes those fields every time it opens).
    openSettings();
  } catch (err) {
    if (gen !== generation) return;
    statusMessage.value = `Couldn't clone: ${String(err)}`;
  } finally {
    if (gen === generation) submitting.value = false;
  }
}
</script>

<template>
  <CloneManualForm
    v-if="isOpen"
    :remote-url="remoteUrl"
    :destination="destination"
    :credential-kind="credentialKind"
    :token-username="tokenUsername"
    :token-value="tokenValue"
    :ssh-key-status="sshKeyStatus"
    :github-device-code="githubDeviceCode"
    :github-status="githubStatus"
    :gitlab-device-code="gitlabDeviceCode"
    :gitlab-status="gitlabStatus"
    :submitting="submitting"
    :status-message="statusMessage"
    @close="close()"
    @update:remote-url="remoteUrl = $event"
    @pick-destination="handlePickDestination()"
    @update:credential-kind="credentialKind = $event"
    @update:token-username="tokenUsername = $event"
    @update:token-value="tokenValue = $event"
    @generate-ssh-key="handleGenerateSshKey()"
    @import-ssh-key="handleImportSshKey()"
    @github-sign-in="runOauthSignIn('github')"
    @gitlab-sign-in="runOauthSignIn('gitlab')"
    @submit="handleSubmit()"
  />
</template>
