// Ticket 10's guided clone wizard: a plain, framework-free state machine
// (pure functions, no DOM), following the exact same architecture ticket
// 09's `connect-wizard.ts` established -- see that module's doc comment for
// the rationale (no vitest/jest in this repo, so this is kept
// dependency-free and side-effect-free specifically so it can be driven by
// a plain `node` script, and type-checked by `tsc --noEmit`).
//
// Scope: CLONE only -- device #2, no local vault exists yet. This is a
// *distinct* state machine from `connect-wizard.ts`'s, not a variant of it,
// because the step ordering is genuinely different: clone must authenticate
// *before* any repository picture exists to attach to (there is no vault to
// read a remote/credential out of yet), whereas connect authenticates
// *after* picking create-new/pick-existing against an already-open vault.
// Concretely: connect's tree is hasRepo -> providerChoice -> credential ->
// oauth/paste -> repoVisibility/repoPicker -> connecting -> commitAuthor;
// clone's is providerChoice -> credential -> oauth/paste -> repoPicker/
// pasteUrl -> **destinationPicker** -> cloning -> commitAuthor. The two
// trees share almost every leaf concept (provider/credential-kind choice,
// oauth sign-in, a repo picker, a paste-URL form, "Commit as") but diverge
// enough in sequencing that folding them into one reducer would need a
// three-way step/mode split for very little real sharing -- the *types*
// below intentionally reuse connect-wizard's naming (`WizardProvider`,
// `WizardCredentialKind`) rather than redefining equivalent ones, but the
// step graph itself is its own.
//
// Full-screen, not a modal (per issue 08 Variant C / ticket 10): there is no
// vault/window yet to anchor a modal on top of, unlike ticket 09's connect
// wizard which layers over the already-open workspace's Settings modal.

import type { WizardProvider, WizardCredentialKind } from "./connect-wizard";

export type { WizardProvider, WizardCredentialKind };

/** Every distinct screen the clone wizard can show. */
export type CloneWizardStep =
  /** "Which provider is the repository on?" -- the wizard's first screen (reached from the first-run folder-picker's "I already have a repository" entry point). */
  | "providerChoice"
  /** "Sign in with GitHub/GitLab" (primary) vs. "Other ways to connect" (secondary) -- only reachable for `provider !== "other"`, mirrors connect-wizard's `credentialChoice`. */
  | "credentialChoice"
  /** Sub-choice of "Other ways to connect": access token or SSH key. The only path for `provider === "other"`. */
  | "otherCredentialKindChoice"
  /** The device-flow screens from tickets 06/07 are driving in the background. */
  | "oauthSignIn"
  /** Pick-existing via GitHub/GitLab, using the token from a completed sign-in (ticket 09's `list_repositories`, reused as-is). */
  | "repoPicker"
  /** A pasted URL plus whatever `credentialKind` needs (token, or key generate/import) -- reachable for every provider (the only path for `provider === "other"`, and "Other ways to connect" for GitHub/GitLab). */
  | "pasteUrl"
  /** Pick an empty local destination folder to clone into -- reachable only once a repository (remote URL) is known, since clone's ordering puts this *after* authentication+repo-picture, unlike connect (which never needs a destination at all: the vault already exists). */
  | "destinationPicker"
  /** The real clone (`clone_and_open_vault`) is running -- per the ticket, the clone itself is the test that gates persistence. */
  | "cloning"
  /** The clone failed -- a clear, human-readable banner with a "try again" affordance, never a raw Rust error string. Nothing was persisted. */
  | "cloneError"
  /** Ticket 08's "Commit as" step, prefilled by the backend from the cloned repo's config or the provider -- always the wizard's last step before it reports completion. */
  | "commitAuthor"
  /** Terminal: the wizard is done, the vault is open, and the caller should tear down the full-screen surface in favor of the ordinary workspace. */
  | "done";

export interface CloneWizardState {
  step: CloneWizardStep;
  provider: WizardProvider | null;
  credentialKind: WizardCredentialKind | null;
  /** The remote URL the wizard will clone -- set once a repository is picked from a list or pasted by hand. */
  remoteUrl: string | null;
  /** The local folder to clone into -- set once the destination picker returns a choice. */
  destination: string | null;
  /** Human-readable failure from the last clone attempt (`cloneError` step only). */
  error: string | null;
  /** Where `back`/retry should return to -- tracked explicitly, same rationale as connect-wizard's `returnStep` (a simple tree, not an arbitrary history stack). */
  returnStep: CloneWizardStep | null;
}

export const initialCloneWizardState: CloneWizardState = {
  step: "providerChoice",
  provider: null,
  credentialKind: null,
  remoteUrl: null,
  destination: null,
  error: null,
  returnStep: null,
};

export type CloneWizardAction =
  | { type: "chooseProvider"; provider: WizardProvider }
  | { type: "chooseOauthSignIn" }
  | { type: "chooseOtherWaysToConnect" }
  | { type: "chooseOtherCredentialKind"; kind: "accessToken" | "sshKey" }
  | { type: "oauthSignInSucceeded" }
  | { type: "repoSelected"; remoteUrl: string }
  | { type: "urlEntered"; remoteUrl: string }
  | { type: "destinationChosen"; destination: string }
  | { type: "cloneSucceeded" }
  | { type: "cloneFailed"; message: string }
  | { type: "retry" }
  | { type: "commitAuthorConfirmed" }
  | { type: "back" }
  | { type: "reset" };

/**
 * The pure transition function. Unknown/out-of-order actions for the
 * current step are ignored (return `state` unchanged) rather than throwing
 * -- same defensive stance as `connect-wizard.ts`'s `reduceWizard`, so a
 * stray event from an already-abandoned async operation can't corrupt the
 * wizard.
 */
export function reduceCloneWizard(state: CloneWizardState, action: CloneWizardAction): CloneWizardState {
  switch (action.type) {
    case "chooseProvider": {
      if (state.step !== "providerChoice") return state;
      if (action.provider === "other") {
        // No OAuth path exists for a generic provider -- straight to the
        // credential-kind sub-choice, the only path for it.
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

    case "oauthSignInSucceeded":
      if (state.step !== "oauthSignIn") return state;
      return { ...state, step: "repoPicker" };

    case "repoSelected":
      if (state.step !== "repoPicker") return state;
      return { ...state, remoteUrl: action.remoteUrl, step: "destinationPicker" };

    case "urlEntered":
      if (state.step !== "pasteUrl") return state;
      return { ...state, remoteUrl: action.remoteUrl, step: "destinationPicker" };

    case "destinationChosen":
      if (state.step !== "destinationPicker") return state;
      return { ...state, destination: action.destination, step: "cloning", returnStep: "destinationPicker" };

    case "cloneSucceeded":
      if (state.step !== "cloning") return state;
      return { ...state, step: "commitAuthor", error: null };

    case "cloneFailed":
      if (state.step !== "cloning") return state;
      return { ...state, step: "cloneError", error: action.message };

    case "retry":
      // Retrying a failed clone re-picks the destination rather than
      // blindly re-cloning into the same one -- the destination may itself
      // be the reason it failed (e.g. it ended up non-empty from a partial
      // attempt on a machine that didn't clean it up), and the ticket's
      // refusal case explicitly tells the user to pick a new folder.
      if (state.step !== "cloneError") return state;
      return { ...state, step: "destinationPicker", error: null };

    case "commitAuthorConfirmed":
      if (state.step !== "commitAuthor") return state;
      return { ...state, step: "done" };

    case "back":
      return { ...state, step: previousStep(state) };

    case "reset":
      return { ...initialCloneWizardState };

    default:
      return state;
  }
}

/**
 * Where `back` returns to from each step -- a fixed tree, computed purely
 * from the current step plus `provider`, same approach as connect-wizard's
 * own `previousStep`.
 */
function previousStep(state: CloneWizardState): CloneWizardStep {
  switch (state.step) {
    case "providerChoice":
      return "providerChoice"; // nothing before the first step
    case "credentialChoice":
      return "providerChoice";
    case "otherCredentialKindChoice":
      return state.provider === "other" ? "providerChoice" : "credentialChoice";
    case "oauthSignIn":
      return "credentialChoice";
    case "repoPicker":
      return "oauthSignIn";
    case "pasteUrl":
      return "otherCredentialKindChoice";
    case "destinationPicker":
      return state.provider !== "other" && state.credentialKind === "oauth" ? "repoPicker" : "pasteUrl";
    case "cloning":
      return state.returnStep ?? "destinationPicker";
    case "cloneError":
      return state.returnStep ?? "destinationPicker";
    case "commitAuthor":
      return "commitAuthor"; // cloning already succeeded -- nothing to undo
    case "done":
      return "done";
  }
}

/** Whether `step` is a leaf the wizard actively waits on an async operation in (drives whether the UI should show a spinner/disable "back"). */
export function isCloneWizardBusyStep(step: CloneWizardStep): boolean {
  return step === "oauthSignIn" || step === "cloning";
}

/** Whether the wizard has reached its terminal "close me" state. */
export function isCloneWizardDone(state: CloneWizardState): boolean {
  return state.step === "done";
}
