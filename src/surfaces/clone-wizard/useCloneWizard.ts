// Container (spec.md#surface-contract, "Wizards and the clone manual
// form"): runs `reduceCloneWizard` and carries out its effects -- device-flow
// polling, the repo list, and the clone itself -- mirroring exactly how
// `main.ts`'s old ticket-10 section drove the same reducer (see
// `clone-wizard.ts`'s own doc comment for the reducer itself, unchanged by
// this ticket). Ephemeral data the reducer doesn't track (access tokens,
// fetched repo lists, generated/imported SSH key material, the clone's own
// result) lives in plain local variables/refs here, reset on every `open()`
// -- mirroring the old module-level `cloneWizard*` variables, now local to
// this composable instead of `src/state/` (spec.md#shared-state-statets).
// No story (containers aren't storied).
import { computed, ref, shallowRef } from "vue";
import {
  reduceCloneWizard,
  initialCloneWizardState,
  isCloneWizardDone,
  isCloneWizardSwitchedToManual,
  isCloneWizardBusyStep,
  type CloneWizardState,
  type CloneWizardAction,
  type WizardProvider,
} from "../../clone-wizard";
import { ensureGithubAppInstalled } from "../../github-app-install";
import { openInBrowser } from "../../open-external";
import {
  pickVaultFolder,
  generateSshKey,
  importSshKey,
  startGithubDeviceFlow,
  pollGithubDeviceFlow,
  checkGithubInstallation,
  listGithubRepositories,
  startGitlabDeviceFlow,
  pollGitlabDeviceFlow,
  listGitlabRepositories,
  confirmCommitAuthor,
  cloneAndOpenVault,
  type RepoInfo,
  type SshKeyInfo,
  type CloneCredential,
  type CommitAuthor,
} from "../../vault-api";
import { promptDialog } from "../../dialogs";
import { setVaultView } from "../../state/ui";
import { applyVaultOpened } from "../../state/vault";
import { setCloneManualPrefill } from "../../clone-manual-handoff";
import type { DeviceCodeDisplay } from "../../components/DeviceFlow.vue";

/** Same fallback labels `connect-wizard.ts`'s driving code (main.ts) uses --
 * duplicated here rather than imported/shared, since main.ts's copy stays
 * put until step 10 migrates the connect wizard too (spec.md#shared-
 * components: `DeviceFlow` isn't extracted until its *second* copy). */
function providerLabel(provider: WizardProvider | null): string {
  if (provider === "github") return "GitHub";
  if (provider === "gitlab") return "GitLab";
  return "this provider";
}

export type { DeviceCodeDisplay };

export function useCloneWizard() {
  const state = ref<CloneWizardState>({ ...initialCloneWizardState });

  const oauthStatus = ref("");
  const oauthDeviceCode = shallowRef<DeviceCodeDisplay | null>(null);
  const repos = ref<RepoInfo[]>([]);
  const repoPickerStatus = ref("");
  const sshKeyStatus = ref<string | null>(null);
  const destinationStatus = ref<string | null>(null);
  const authorPrefill = ref<CommitAuthor | null>(null);
  const commitAuthorError = ref<string | null>(null);

  // Ephemeral data the reducer doesn't track -- see this module's doc
  // comment. Reset in `open()`.
  let accessToken: string | null = null;
  let refreshToken: string | undefined;
  let accessTokenExpiresAt = "";
  let sshKey: SshKeyInfo | null = null;
  /** Holds the pasted-URL access-token form's values between `pasteUrl` and
   * `destinationPicker`, since the reducer only tracks `remoteUrl` --
   * mirrors the old `cloneWizardPendingAccessTokenConnect`. */
  let pendingAccessTokenConnect: { remoteUrl: string; username: string; token: string } | null = null;
  let clonedPath: string | null = null;
  /** Bumped on every open/close so a stale device-flow poll loop or
   * in-flight clone from a previous attempt can tell it's been abandoned. */
  let generation = 0;

  const canGoBack = computed(
    () => !isCloneWizardBusyStep(state.value.step) && state.value.step !== "providerChoice",
  );

  function invalidate(): void {
    generation += 1;
  }

  function dispatch(action: CloneWizardAction): void {
    state.value = reduceCloneWizard(state.value, action);
    if (isCloneWizardSwitchedToManual(state.value)) {
      handleSwitchedToManual();
      return;
    }
    runStepEffect();
    if (isCloneWizardDone(state.value)) {
      close();
      void finishIntoWorkspace();
    }
  }

  /** Ticket 11's "Switch to manual setup" escape hatch: tears down this
   * wizard (invalidating any in-flight poll/clone) and hands off to the
   * standalone manual form via `clone-manual-handoff.ts`, carrying over
   * `remoteUrl`/`destination` if either was already committed -- same
   * accepted "may re-ask for values" limitation as before. */
  function handleSwitchedToManual(): void {
    const remoteUrl = state.value.remoteUrl ?? "";
    const destination = state.value.destination ?? "";
    invalidate();
    setCloneManualPrefill({ remoteUrl, destination });
    setVaultView("cloneManual");
  }

  async function finishIntoWorkspace(): Promise<void> {
    if (!clonedPath) return;
    // `clone_and_open_vault` already opened the vault server-side, so this
    // runs the shared post-open tail directly (view switch + sync refresh +
    // page/trash load) rather than `state/vault.ts`'s `openVault`, which
    // would re-invoke the backend `open_vault` command needlessly.
    await applyVaultOpened(clonedPath);
  }

  function runStepEffect(): void {
    const gen = generation;
    switch (state.value.step) {
      case "oauthSignIn":
        oauthStatus.value = `Requesting a device code from ${providerLabel(state.value.provider)}…`;
        oauthDeviceCode.value = null;
        void runOauthSignIn(gen);
        break;
      case "repoPicker":
        repoPickerStatus.value = "Loading your repositories…";
        repos.value = [];
        void loadRepoPicker(gen);
        break;
      case "cloning":
        void performClone(gen);
        break;
      default:
        break;
    }
  }

  async function runOauthSignIn(gen: number): Promise<void> {
    const provider = state.value.provider;
    const stale = () => gen !== generation;
    try {
      const device = provider === "gitlab" ? await startGitlabDeviceFlow() : await startGithubDeviceFlow();
      if (stale()) return;

      oauthDeviceCode.value = { verificationUri: device.verificationUri, userCode: device.userCode };
      oauthStatus.value = "Waiting for you to approve in the browser…";
      void openInBrowser(device.verificationUri);

      const deadline = Date.now() + device.expiresInSecs * 1000;
      let intervalMs = Math.max(device.intervalSecs, 1) * 1000;
      let token: string;
      let refresh: string | undefined;
      let expiresAt: string;

      for (;;) {
        if (stale()) return;
        if (Date.now() >= deadline) {
          throw new Error(`The ${providerLabel(provider)} sign-in code expired before it was confirmed.`);
        }
        await new Promise((resolve) => setTimeout(resolve, intervalMs));
        if (stale()) return;

        const result =
          provider === "gitlab" ? await pollGitlabDeviceFlow(device.deviceCode) : await pollGithubDeviceFlow(device.deviceCode);
        if (result.outcome === "pending") continue;
        if (result.outcome === "slowDown") {
          intervalMs += 5000;
          continue;
        }
        if (result.outcome === "denied") throw new Error(`${providerLabel(provider)} sign-in was denied.`);
        if (result.outcome === "expired") {
          throw new Error(`The ${providerLabel(provider)} sign-in code expired before it was confirmed.`);
        }
        if (result.outcome === "error") throw new Error(result.message);

        token = result.accessToken;
        refresh = result.refreshToken;
        expiresAt = result.accessTokenExpiresAt;
        break;
      }

      if (stale()) return;
      accessToken = token;
      refreshToken = refresh;
      accessTokenExpiresAt = expiresAt;
      oauthDeviceCode.value = null;
      if (provider === "github") await ensureGithubAppInstalled(token, (status) => (oauthStatus.value = status), stale);
      if (stale()) return;
      dispatch({ type: "oauthSignInSucceeded" });
    } catch (err) {
      if (stale()) return;
      oauthStatus.value = String(err);
      oauthDeviceCode.value = null;
    }
  }

  async function loadRepoPicker(gen: number): Promise<void> {
    if (!accessToken) {
      repoPickerStatus.value = "Sign-in is required before listing repositories.";
      return;
    }
    try {
      const list =
        state.value.provider === "gitlab" ? await listGitlabRepositories(accessToken) : await listGithubRepositories(accessToken);
      if (gen !== generation) return;
      repos.value = list;
      repoPickerStatus.value = list.length === 0 ? "No repositories found for this account." : "Pick a repository to clone:";
    } catch (err) {
      if (gen !== generation) return;
      repoPickerStatus.value = `Couldn't load repositories: ${String(err)}`;
    }
  }

  async function generateSshKeyStep(): Promise<void> {
    sshKeyStatus.value = "Generating…";
    try {
      sshKey = await generateSshKey();
      sshKeyStatus.value = `Key ready (fingerprint ${sshKey.fingerprintSha256}). Add the public key to your provider, then Continue.`;
    } catch (err) {
      sshKeyStatus.value = String(err);
    }
  }

  async function importSshKeyStep(): Promise<void> {
    const privateKeyOpenssh = promptDialog("Paste the private key (OpenSSH format):");
    if (privateKeyOpenssh === null || !privateKeyOpenssh.trim()) return;
    const passphrase = promptDialog("Passphrase (leave blank if none):") ?? undefined;

    sshKeyStatus.value = "Importing…";
    try {
      sshKey = await importSshKey(privateKeyOpenssh, passphrase || undefined);
      sshKeyStatus.value = `Key imported (fingerprint ${sshKey.fingerprintSha256}). Add the public key to your provider, then Continue.`;
    } catch (err) {
      sshKeyStatus.value = String(err);
    }
  }

  function submitPasteUrlToken(remoteUrl: string, username: string, token: string): void {
    pendingAccessTokenConnect = { remoteUrl, username, token };
    dispatch({ type: "urlEntered", remoteUrl });
  }

  function submitPasteUrlSshKey(remoteUrl: string): void {
    dispatch({ type: "urlEntered", remoteUrl });
  }

  function pickDestination(): void {
    const gen = generation;
    void (async () => {
      try {
        const path = await pickVaultFolder();
        if (!path) return; // user cancelled
        if (gen !== generation) return;
        dispatch({ type: "destinationChosen", destination: path });
      } catch (err) {
        if (gen !== generation) return;
        destinationStatus.value = String(err);
      }
    })();
  }

  /** Ticket 10 checklist item 4/5/7: the real clone -- `clone_and_open_vault`
   * authenticates with whichever credential this wizard obtained, clones,
   * classifies the four post-clone states, and only *then* persists the
   * Connection + remembered vault path. */
  async function performClone(gen: number): Promise<void> {
    const remoteUrl = state.value.remoteUrl;
    const destination = state.value.destination;
    if (!remoteUrl || !destination) {
      dispatch({ type: "cloneFailed", message: "No repository or destination folder to clone into." });
      return;
    }
    try {
      let credential: CloneCredential;
      if (state.value.credentialKind === "oauth") {
        if (!accessToken) throw new Error("Sign-in is required before cloning.");
        if (state.value.provider === "github") {
          const installation = await checkGithubInstallation(remoteUrl, accessToken);
          if (installation.status === "notInstalled") {
            throw new Error(
              `Cerebrite isn't installed on this repository yet. Install it at ${installation.installUrl}, then try again.`,
            );
          }
          credential = {
            kind: "githubOauth",
            accessToken,
            refreshToken: refreshToken ?? "",
            accessTokenExpiresAt,
          };
        } else {
          credential = { kind: "gitlabOauth", accessToken, refreshToken, accessTokenExpiresAt };
        }
      } else if (state.value.credentialKind === "accessToken") {
        if (!pendingAccessTokenConnect) throw new Error("Missing access token details.");
        credential = {
          kind: "accessToken",
          username: pendingAccessTokenConnect.username,
          token: pendingAccessTokenConnect.token,
        };
      } else if (state.value.credentialKind === "sshKey") {
        if (!sshKey) throw new Error("Generate or import an SSH key first.");
        credential = { kind: "sshKey", privateKeyOpenssh: sshKey.privateKeyOpenssh, passphrase: sshKey.passphrase };
      } else {
        throw new Error("No authentication method was chosen.");
      }

      const result = await cloneAndOpenVault(remoteUrl, destination, credential);
      if (gen !== generation) return;
      clonedPath = result.vault.path;
      authorPrefill.value = result.authorPrefill.author;
      dispatch({ type: "cloneSucceeded" });
    } catch (err) {
      if (gen !== generation) return;
      dispatch({ type: "cloneFailed", message: `Couldn't clone: ${String(err)}` });
    }
  }

  /** Ticket 10's last step, reusing ticket 08's exact `confirmCommitAuthor`
   * command -- prefilled from the clone's own `authorPrefill` (computed by
   * the backend at clone time) rather than a second `commitAuthorPrefill`
   * round trip. Leaves the wizard on `commitAuthor` and shows an inline
   * error on failure, same as before. */
  async function confirmCommitAuthorStep(name: string, email: string): Promise<void> {
    commitAuthorError.value = null;
    try {
      await confirmCommitAuthor(name, email);
      dispatch({ type: "commitAuthorConfirmed" });
    } catch (err) {
      commitAuthorError.value = String(err);
    }
  }

  function chooseProvider(provider: WizardProvider): void {
    dispatch({ type: "chooseProvider", provider });
  }

  function chooseOauthSignIn(): void {
    dispatch({ type: "chooseOauthSignIn" });
  }

  function chooseOtherWaysToConnect(): void {
    dispatch({ type: "chooseOtherWaysToConnect" });
  }

  function chooseOtherCredentialKind(kind: "accessToken" | "sshKey"): void {
    dispatch({ type: "chooseOtherCredentialKind", kind });
  }

  function repoSelected(remoteUrl: string): void {
    dispatch({ type: "repoSelected", remoteUrl });
  }

  function retry(): void {
    dispatch({ type: "retry" });
  }

  function back(): void {
    dispatch({ type: "back" });
  }

  function switchToManual(): void {
    dispatch({ type: "switchToManual" });
  }

  /** Cancels the wizard and returns to the first-run folder-picker screen --
   * there is no vault to fall back into. */
  function close(): void {
    invalidate();
    if (!isCloneWizardDone(state.value)) setVaultView("picker");
  }

  /** Resets every field (state and ephemeral data alike) and (re)starts
   * effects for the first step -- called by the container whenever
   * `ui.vaultView` becomes `'cloneWizard'`. */
  function open(): void {
    invalidate();
    state.value = { ...initialCloneWizardState };
    accessToken = null;
    refreshToken = undefined;
    accessTokenExpiresAt = "";
    repos.value = [];
    sshKey = null;
    pendingAccessTokenConnect = null;
    authorPrefill.value = null;
    commitAuthorError.value = null;
    clonedPath = null;
    oauthStatus.value = "";
    oauthDeviceCode.value = null;
    repoPickerStatus.value = "";
    sshKeyStatus.value = null;
    destinationStatus.value = null;
    runStepEffect();
  }

  return {
    state,
    canGoBack,
    oauthStatus,
    oauthDeviceCode,
    repos,
    repoPickerStatus,
    sshKeyStatus,
    destinationStatus,
    authorPrefill,
    commitAuthorError,
    open,
    close,
    chooseProvider,
    chooseOauthSignIn,
    chooseOtherWaysToConnect,
    chooseOtherCredentialKind,
    repoSelected,
    submitPasteUrlToken,
    submitPasteUrlSshKey,
    generateSshKeyStep,
    importSshKeyStep,
    pickDestination,
    retry,
    confirmCommitAuthorStep,
    back,
    switchToManual,
  };
}
