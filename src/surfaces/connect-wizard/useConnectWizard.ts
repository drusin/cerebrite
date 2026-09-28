// Container (spec.md#surface-contract, "Wizards and the clone manual
// form"): runs `reduceWizard` and carries out its effects -- device-flow
// polling, repo list/create, and the real `connect_*` test-fetch-then-
// persist calls -- mirroring exactly how `main.ts`'s old ticket-09 section
// drove the same reducer (see `connect-wizard.ts`'s own doc comment for the
// reducer itself, unchanged by this ticket) and how ticket 10's
// `useCloneWizard.ts` already wraps its own sibling reducer the same way.
// Ephemeral data the reducer doesn't track (access tokens, fetched repo
// lists, generated/imported SSH key material) lives in plain local
// variables/refs here, reset on every `open()` -- mirroring the old
// module-level `wizard*` variables, now local to this composable instead of
// `src/state/` (spec.md#shared-state-statets). No story (containers aren't
// storied).
import { computed, ref, shallowRef } from "vue";
import {
  reduceWizard,
  initialWizardState,
  isDone as isWizardDone,
  isSwitchedToManual as isWizardSwitchedToManual,
  isBusyStep,
  type WizardState,
  type WizardAction,
  type WizardProvider,
} from "../../connect-wizard";
import {
  generateSshKey,
  importSshKey,
  startGithubDeviceFlow,
  pollGithubDeviceFlow,
  checkGithubInstallation,
  createGithubRepository,
  listGithubRepositories,
  startGitlabDeviceFlow,
  pollGitlabDeviceFlow,
  createGitlabRepository,
  listGitlabRepositories,
  connectAccessToken,
  connectSshKey,
  connectGithubOauth,
  connectGitlabOauth,
  commitAuthorPrefill,
  getCommitAuthor,
  confirmCommitAuthor,
  type RepoInfo,
  type SshKeyInfo,
  type CommitAuthor,
} from "../../vault-api";
import { promptDialog, withPlaintextFallbackConsent } from "../../dialogs";
import { openSettings, closeConnectWizardModal } from "../../state/ui";
import type { DeviceCodeDisplay } from "../../components/DeviceFlow.vue";
import type { CommitAuthorValue } from "../../components/CommitAs.vue";

function providerLabel(provider: WizardProvider | null): string {
  if (provider === "github") return "GitHub";
  if (provider === "gitlab") return "GitLab";
  return "this provider";
}

export interface ConnectWizardOptions {
  /** Ticket 11's temporary callback root prop (see `ConnectWizardContainer.vue`'s
   * doc comment) -- refreshes Settings' still-vanilla "Commit as" fields
   * after this wizard's own "Commit as" step saves a new one. */
  refreshCommitAuthorFieldsInVanilla: () => Promise<void>;
}

export function useConnectWizard(options: ConnectWizardOptions) {
  const state = ref<WizardState>({ ...initialWizardState });

  const oauthStatus = ref("");
  const oauthDeviceCode = shallowRef<DeviceCodeDisplay | null>(null);
  const repos = ref<RepoInfo[]>([]);
  const repoPickerStatus = ref("");
  const sshKeyStatus = ref<string | null>(null);
  const createRepoError = ref<string | null>(null);
  const authorPrefill = ref<CommitAuthorValue | null>(null);
  const commitAuthorError = ref<string | null>(null);

  // Ephemeral data the reducer doesn't track -- see this module's doc
  // comment. Reset in `open()`.
  let accessToken: string | null = null;
  let refreshToken: string | undefined;
  let accessTokenExpiresAt = "";
  let sshKey: SshKeyInfo | null = null;
  /** Holds the pasted-URL access-token form's values between `pasteUrl` and
   * `connecting`, since the reducer only tracks `remoteUrl` -- mirrors the
   * old `wizardPendingAccessTokenConnect`. */
  let pendingAccessTokenConnect: { remoteUrl: string; username: string; token: string } | null = null;
  /** Bumped on every open/close so a stale device-flow poll loop or
   * in-flight connect from a previous attempt can tell it's been abandoned. */
  let generation = 0;

  const canGoBack = computed(
    () => !isBusyStep(state.value.step) && state.value.step !== "hasRepo" && state.value.step !== "done",
  );

  function invalidate(): void {
    generation += 1;
  }

  function dispatch(action: WizardAction): void {
    state.value = reduceWizard(state.value, action);
    if (isWizardSwitchedToManual(state.value)) {
      handleSwitchedToManual();
      return;
    }
    runStepEffect();
    if (isWizardDone(state.value)) {
      close();
    }
  }

  /** Ticket 11's "Switch to manual setup" escape hatch: tears down this
   * wizard (invalidating any in-flight poll/connect) and reuses
   * `state/ui.ts`'s existing `openSettings` deep-link action to jump
   * Settings to its always-visible "Sync" section, carrying over
   * `remoteUrl` if one was already committed -- same accepted "may re-ask
   * for the credential" limitation the old vanilla wizard had. */
  function handleSwitchedToManual(): void {
    const remoteUrl = state.value.remoteUrl ?? "";
    invalidate();
    closeConnectWizardModal();
    openSettings({ section: "sync", prefillUrl: remoteUrl });
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
      case "connecting":
        void performConnect(gen);
        break;
      case "commitAuthor":
        void primeCommitAuthor();
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
      dispatch({ type: "oauthSignInSucceeded", accessToken });
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
      repoPickerStatus.value = list.length === 0 ? "No repositories found for this account." : "Pick a repository:";
    } catch (err) {
      if (gen !== generation) return;
      repoPickerStatus.value = `Couldn't load repositories: ${String(err)}`;
    }
  }

  async function generateSshKeyStep(): Promise<void> {
    sshKeyStatus.value = "Generating…";
    try {
      sshKey = await generateSshKey();
      sshKeyStatus.value = `Key ready (fingerprint ${sshKey.fingerprintSha256}). Add the public key to your provider, then Connect.`;
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
      sshKeyStatus.value = `Key imported (fingerprint ${sshKey.fingerprintSha256}). Add the public key to your provider, then Connect.`;
    } catch (err) {
      sshKeyStatus.value = String(err);
    }
  }

  function setRepoName(name: string): void {
    dispatch({ type: "setRepoName", name });
  }

  function setVisibility(visibility: "private" | "public"): void {
    dispatch({ type: "setVisibility", visibility });
  }

  async function createRepo(): Promise<void> {
    const name = state.value.repoName.trim();
    createRepoError.value = null;
    if (!name) {
      createRepoError.value = "Give the repository a name.";
      return;
    }
    if (!accessToken) {
      createRepoError.value = "Sign-in is required before creating a repository.";
      return;
    }
    const gen = generation;
    try {
      const isPrivate = state.value.visibility === "private";
      const repo =
        state.value.provider === "gitlab"
          ? await createGitlabRepository(name, isPrivate, accessToken)
          : await createGithubRepository(name, isPrivate, accessToken);
      if (gen !== generation) return;
      dispatch({ type: "repoReady", remoteUrl: repo.cloneUrl });
    } catch (err) {
      if (gen !== generation) return;
      createRepoError.value = `Couldn't create the repository: ${String(err)}`;
    }
  }

  function repoSelected(remoteUrl: string): void {
    dispatch({ type: "repoReady", remoteUrl });
  }

  function submitPasteUrlToken(remoteUrl: string, username: string, token: string): void {
    pendingAccessTokenConnect = { remoteUrl, username, token };
    dispatch({ type: "urlEntered", remoteUrl });
  }

  function submitPasteUrlSshKey(remoteUrl: string): void {
    dispatch({ type: "urlEntered", remoteUrl });
  }

  /** Ticket 09 checklist item 7: the real test fetch every `connect_*`
   * command already runs before persisting anything -- routes to whichever
   * one matches `credentialKind`/`provider`, wrapped in the same
   * plaintext-fallback consent gate every other connect door uses, and
   * turns a failure into `connectFailed` (a clear in-wizard error banner)
   * or a success into `connectSucceeded` (-> "Commit as"). */
  async function performConnect(gen: number): Promise<void> {
    const remoteUrl = state.value.remoteUrl;
    if (!remoteUrl) {
      dispatch({ type: "connectFailed", message: "No repository URL to connect to." });
      return;
    }
    try {
      if (state.value.credentialKind === "oauth") {
        if (!accessToken) throw new Error("Sign-in is required before connecting.");
        if (state.value.provider === "github") {
          const installation = await checkGithubInstallation(remoteUrl, accessToken);
          if (installation.status === "notInstalled") {
            throw new Error(
              `Cerebrite isn't installed on this repository yet. Install it at ${installation.installUrl}, then try again.`,
            );
          }
          await withPlaintextFallbackConsent((allow) =>
            connectGithubOauth(remoteUrl, accessToken!, refreshToken ?? "", accessTokenExpiresAt, allow),
          );
        } else {
          await withPlaintextFallbackConsent((allow) =>
            connectGitlabOauth(remoteUrl, accessToken!, refreshToken, accessTokenExpiresAt, allow),
          );
        }
      } else if (state.value.credentialKind === "accessToken") {
        if (!pendingAccessTokenConnect) throw new Error("Missing access token details.");
        await withPlaintextFallbackConsent((allow) =>
          connectAccessToken(
            pendingAccessTokenConnect!.remoteUrl,
            pendingAccessTokenConnect!.username,
            pendingAccessTokenConnect!.token,
            allow,
          ),
        );
      } else if (state.value.credentialKind === "sshKey") {
        if (!sshKey) throw new Error("Generate or import an SSH key first.");
        await withPlaintextFallbackConsent((allow) =>
          connectSshKey(remoteUrl, sshKey!.privateKeyOpenssh, sshKey!.passphrase, allow),
        );
      } else {
        throw new Error("No connection method was chosen.");
      }
      if (gen !== generation) return;
      dispatch({ type: "connectSucceeded" });
    } catch (err) {
      if (gen !== generation) return;
      dispatch({ type: "connectFailed", message: `Couldn't connect: ${String(err)}` });
    }
  }

  /** Ticket 08's exact "Commit as" prefill -- confirmed repo-local ->
   * global -> empty, same as every other door into it. */
  async function primeCommitAuthor(): Promise<void> {
    commitAuthorError.value = null;
    try {
      const confirmed = await getCommitAuthor();
      const author: CommitAuthor | null = confirmed ?? (await commitAuthorPrefill()).author;
      authorPrefill.value = author ? { name: author.name, email: author.email } : null;
    } catch {
      // No vault open -- leave blank; `confirmCommitAuthorStep` below
      // surfaces a clear error if that's somehow still true at submit time.
      authorPrefill.value = null;
    }
  }

  /** Ticket 09's last step, reusing ticket 08's exact `confirmCommitAuthor`
   * command -- never a parallel author-writing path. */
  async function confirmCommitAuthorStep(name: string, email: string): Promise<void> {
    commitAuthorError.value = null;
    try {
      await confirmCommitAuthor(name, email);
      await options.refreshCommitAuthorFieldsInVanilla();
      dispatch({ type: "commitAuthorConfirmed" });
    } catch (err) {
      commitAuthorError.value = String(err);
    }
  }

  function chooseHasRepo(hasRepo: boolean): void {
    dispatch({ type: "chooseHasRepo", hasRepo });
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

  function retry(): void {
    dispatch({ type: "retry" });
  }

  function back(): void {
    dispatch({ type: "back" });
  }

  function switchToManual(): void {
    dispatch({ type: "switchToManual" });
  }

  /** Closes the wizard without finishing -- per the ticket, `ui.modal`
   * returns to `'settings'` (the vanilla Settings modal was showing
   * underneath the whole time; see `closeConnectWizardModal`'s doc
   * comment). */
  function close(): void {
    invalidate();
    closeConnectWizardModal();
  }

  /** Resets every field (state and ephemeral data alike) and (re)starts
   * effects for the first step -- called by the container whenever
   * `ui.modal` becomes `'connectWizard'`. */
  function open(): void {
    invalidate();
    state.value = { ...initialWizardState };
    accessToken = null;
    refreshToken = undefined;
    accessTokenExpiresAt = "";
    sshKey = null;
    pendingAccessTokenConnect = null;
    repos.value = [];
    oauthStatus.value = "";
    oauthDeviceCode.value = null;
    repoPickerStatus.value = "";
    sshKeyStatus.value = null;
    createRepoError.value = null;
    authorPrefill.value = null;
    commitAuthorError.value = null;
  }

  return {
    state,
    canGoBack,
    oauthStatus,
    oauthDeviceCode,
    repos,
    repoPickerStatus,
    sshKeyStatus,
    createRepoError,
    authorPrefill,
    commitAuthorError,
    open,
    close,
    chooseHasRepo,
    chooseProvider,
    chooseOauthSignIn,
    chooseOtherWaysToConnect,
    chooseOtherCredentialKind,
    setRepoName,
    setVisibility,
    createRepo,
    repoSelected,
    submitPasteUrlToken,
    submitPasteUrlSshKey,
    generateSshKeyStep,
    importSshKeyStep,
    retry,
    confirmCommitAuthorStep,
    back,
    switchToManual,
  };
}
