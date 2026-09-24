// Ticket 09's guided connect wizard: a plain, framework-free state machine
// (pure functions, no DOM) modeling every step/branch the wizard can be in --
// deliberately factored out of main.ts so its transitions are testable in
// isolation (see this module's doc comment on testing, and main.ts's
// wizard-driving code for the side-effecting half: actual `invoke` calls,
// rendering).
//
// Scope (per the ticket): CONNECT only -- attaching a remote to an
// already-open local vault. Cloning a fresh vault from a remote is ticket
// 10's job, not this one. "Switch to manual setup" is a stub here (ticket
// 11 wires up a real standalone flow); `reset` is this module's stand-in for
// it.
//
// # Testing
//
// This project has no frontend test runner configured (`package.json` has
// no vitest/jest -- see the ticket 09 implementation notes) and this ticket
// doesn't add one speculatively. This module is instead kept dependency-free
// and side-effect-free specifically so it *can* be driven by a plain `node`
// script without any framework, and `tsc --noEmit` type-checks it as part of
// the normal `npm run build` pipeline. A one-off verification script
// (`scratchpad/verify-connect-wizard.ts`, not committed -- see the ticket's
// commit message) was run once during development to exercise every branch
// below; it is not part of this repo's build/test pipeline.

/** Which provider the user is connecting to. `"other"` covers every HTTPS git host that isn't GitHub/GitLab sign-in (Bitbucket, Gitea, Forgejo, Codeberg, a bare HTTPS remote) -- ticket 04's generic access-token path, or ticket 05's SSH path. */
export type WizardProvider = "github" | "gitlab" | "other";

/** Which mechanism actually supplies the credential. `"oauth"` is only reachable for `github`/`gitlab`; `"other"` provider always uses `accessToken`/`sshKey` (tickets 04/05), never `oauth`. */
export type WizardCredentialKind = "oauth" | "accessToken" | "sshKey";

export type WizardVisibility = "private" | "public";

/**
 * Every distinct screen the wizard can show. Kept as a flat union (rather
 * than nested state) so a caller can exhaustively switch over it when
 * rendering, the same way `DevicePollResult`'s tagged union is already
 * switched over in main.ts.
 */
export type WizardStep =
  /** "Do you already have a repository?" -- the wizard's first branching question. */
  | "hasRepo"
  /** Pick GitHub / GitLab / "another provider" -- offered for both branches; create-new hides "another provider" in the UI (there is no generic-provider create-repo API), but the step id is the same. */
  | "providerChoice"
  /** "Sign in with GitHub/GitLab" (primary) vs. "Other ways to connect" (secondary, access token or SSH key) -- ticket 09 checklist item 5. Only reachable for `provider !== "other"`. */
  | "credentialChoice"
  /** Sub-choice of "Other ways to connect": access token or SSH key. Reachable for every provider (it's the *only* path for `provider === "other"`). */
  | "otherCredentialKindChoice"
  /** The device-flow screens from tickets 06/07 are driving in the background; this step just means "waiting on that". */
  | "oauthSignIn"
  /** Create-new, after a successful sign-in: repository name + private/public choice (private is the default; public needs an explicit toggle). */
  | "repoVisibility"
  /** Pick-existing via GitHub/GitLab: a list of the signed-in user's repositories to choose from. */
  | "repoPicker"
  /** Pick-existing via any other provider, or "Other ways to connect" for GitHub/GitLab: a pasted URL plus whatever `credentialKind` needs (token, or key generate/import). */
  | "pasteUrl"
  /** The real test fetch (`connection::try_connect`, via whichever `connect_*` command matches `credentialKind`) is running. */
  | "connecting"
  /** The test fetch failed -- a clear, human-readable banner with a "try again" affordance, never a raw Rust error string. */
  | "connectError"
  /** Ticket 08's "Commit as" step -- always the wizard's last step before it reports completion. */
  | "commitAuthor"
  /** Terminal: the wizard is done and should close. */
  | "done";

export interface WizardState {
  step: WizardStep;
  hasRepo: boolean | null;
  provider: WizardProvider | null;
  credentialKind: WizardCredentialKind | null;
  visibility: WizardVisibility;
  repoName: string;
  /** The remote URL the wizard will ultimately connect with -- set once a repository is created, picked from a list, or pasted by hand. */
  remoteUrl: string | null;
  /** Human-readable failure from the last connect attempt (`connectError` step only). */
  error: string | null;
  /** Where "Edit"/back from `connectError` or `commitAuthor`-adjacent screens should return to -- tracked explicitly rather than a full history stack, since this wizard's steps form a simple tree, not an arbitrary path. */
  returnStep: WizardStep | null;
}

export const initialWizardState: WizardState = {
  step: "hasRepo",
  hasRepo: null,
  provider: null,
  credentialKind: null,
  visibility: "private",
  repoName: "",
  remoteUrl: null,
  error: null,
  returnStep: null,
};

export type WizardAction =
  | { type: "chooseHasRepo"; hasRepo: boolean }
  | { type: "chooseProvider"; provider: WizardProvider }
  | { type: "chooseOauthSignIn" }
  | { type: "chooseOtherWaysToConnect" }
  | { type: "chooseOtherCredentialKind"; kind: "accessToken" | "sshKey" }
  | { type: "oauthSignInSucceeded"; accessToken: string }
  | { type: "setRepoName"; name: string }
  | { type: "setVisibility"; visibility: WizardVisibility }
  | { type: "repoReady"; remoteUrl: string }
  | { type: "urlEntered"; remoteUrl: string }
  | { type: "startConnecting" }
  | { type: "connectSucceeded" }
  | { type: "connectFailed"; message: string }
  | { type: "retry" }
  | { type: "commitAuthorConfirmed" }
  | { type: "back" }
  | { type: "reset" };

/**
 * The pure transition function -- every step change in the wizard goes
 * through here, and nothing in here touches the DOM or calls `invoke`
 * (main.ts's job). Unknown/out-of-order actions for the current step are
 * ignored (return `state` unchanged) rather than throwing, so a stray event
 * from an already-abandoned async operation (e.g. a stale device-flow poll
 * resolving after the user clicked "back") can't corrupt the wizard.
 */
export function reduceWizard(state: WizardState, action: WizardAction): WizardState {
  switch (action.type) {
    case "chooseHasRepo":
      return { ...state, hasRepo: action.hasRepo, step: "providerChoice" };

    case "chooseProvider": {
      if (state.step !== "providerChoice") return state;
      if (action.provider === "other") {
        // No OAuth/create-repo path exists for a generic provider -- straight
        // to the credential-kind sub-choice, which is the *only* path for it.
        return { ...state, provider: "other", step: "otherCredentialKindChoice" };
      }
      return { ...state, provider: action.provider, step: "credentialChoice" };
    }

    case "chooseOauthSignIn":
      if (state.step !== "credentialChoice") return state;
      return { ...state, credentialKind: "oauth", step: "oauthSignIn" };

    case "chooseOtherWaysToConnect":
      if (state.step !== "credentialChoice") return state;
      return { ...state, step: "otherCredentialKindChoice" };

    case "chooseOtherCredentialKind":
      if (state.step !== "otherCredentialKindChoice") return state;
      return { ...state, credentialKind: action.kind, step: "pasteUrl" };

    case "oauthSignInSucceeded": {
      if (state.step !== "oauthSignIn") return state;
      // Pick-existing: go list repositories. Create-new: go ask name +
      // visibility. Either way the access token itself is held by the
      // caller (main.ts), not by this state -- the reducer only tracks
      // *which screen* to show next.
      const nextStep: WizardStep = state.hasRepo ? "repoPicker" : "repoVisibility";
      return { ...state, step: nextStep };
    }

    case "setRepoName":
      if (state.step !== "repoVisibility") return state;
      return { ...state, repoName: action.name };

    case "setVisibility":
      if (state.step !== "repoVisibility") return state;
      return { ...state, visibility: action.visibility };

    case "repoReady":
      // Reachable from `repoVisibility` (create-new, after the repo was
      // actually created) or `repoPicker` (pick-existing, after a selection).
      if (state.step !== "repoVisibility" && state.step !== "repoPicker") return state;
      return { ...state, remoteUrl: action.remoteUrl, step: "connecting", returnStep: state.step };

    case "urlEntered":
      if (state.step !== "pasteUrl") return state;
      return { ...state, remoteUrl: action.remoteUrl, step: "connecting", returnStep: "pasteUrl" };

    case "startConnecting":
      // Explicit re-entry into "connecting" for a retry from `connectError`
      // (rather than `retry` itself performing the connect -- that's a side
      // effect main.ts owns).
      if (state.step !== "connectError") return state;
      return { ...state, step: "connecting", error: null };

    case "connectSucceeded":
      if (state.step !== "connecting") return state;
      return { ...state, step: "commitAuthor", error: null };

    case "connectFailed":
      if (state.step !== "connecting") return state;
      return { ...state, step: "connectError", error: action.message };

    case "retry":
      if (state.step !== "connectError") return state;
      return { ...state, step: "connecting", error: null };

    case "commitAuthorConfirmed":
      if (state.step !== "commitAuthor") return state;
      return { ...state, step: "done" };

    case "back":
      return { ...state, step: previousStep(state) };

    case "reset":
      return { ...initialWizardState };

    default:
      return state;
  }
}

/**
 * Where `back` returns to from each step -- a fixed tree, not an arbitrary
 * history stack (this wizard never revisits the same step twice via
 * different paths), so it can be computed purely from the current step plus
 * the two branch choices (`hasRepo`/`provider`) already recorded in state.
 */
function previousStep(state: WizardState): WizardStep {
  switch (state.step) {
    case "hasRepo":
      return "hasRepo"; // nothing before the first step
    case "providerChoice":
      return "hasRepo";
    case "credentialChoice":
      return "providerChoice";
    case "otherCredentialKindChoice":
      return state.provider === "other" ? "providerChoice" : "credentialChoice";
    case "oauthSignIn":
      return "credentialChoice";
    case "repoVisibility":
      return "oauthSignIn";
    case "repoPicker":
      return "oauthSignIn";
    case "pasteUrl":
      return "otherCredentialKindChoice";
    case "connecting":
      return state.returnStep ?? "hasRepo";
    case "connectError":
      return state.returnStep ?? "hasRepo";
    case "commitAuthor":
      return "commitAuthor"; // connecting already succeeded -- nothing to undo
    case "done":
      return "done";
  }
}

/** Whether `step` is a leaf the wizard actively waits on an async operation in (drives whether main.ts should show a spinner/disable "back"). */
export function isBusyStep(step: WizardStep): boolean {
  return step === "oauthSignIn" || step === "connecting";
}

/** Whether the wizard has reached its terminal "close me" state. */
export function isDone(state: WizardState): boolean {
  return state.step === "done";
}
