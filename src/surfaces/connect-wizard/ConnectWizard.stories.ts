// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per reducer step and sub-state, per the ticket 11 checklist -- no
// backend calls, just a hand-built `WizardState` (plus whatever ephemeral
// prop each step also needs) fed straight in as props, same shape as
// `../clone-wizard/CloneWizard.stories.ts`.
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import { expect, userEvent, within } from "storybook/test";
import ConnectWizard, { type ConnectRepoOption } from "./ConnectWizard.vue";
import { initialWizardState, type WizardState } from "../../connect-wizard";

const meta: Meta<typeof ConnectWizard> = {
  component: ConnectWizard,
  title: "Surfaces/Connect wizard/ConnectWizard",
};
export default meta;

type Story = StoryObj<typeof ConnectWizard>;

function state(overrides: Partial<WizardState>): WizardState {
  return { ...initialWizardState, ...overrides };
}

const BASE_PROPS = {
  canGoBack: false,
  oauthStatus: "",
  oauthDeviceCode: null,
  repos: [],
  repoPickerStatus: "",
  sshKeyStatus: null,
  createRepoError: null,
  authorPrefill: null,
  commitAuthorError: null,
};

export const HasRepo: Story = {
  args: {
    ...BASE_PROPS,
    state: state({ step: "hasRepo" }),
  },
};

export const ProviderChoicePickExisting: Story = {
  name: "ProviderChoice (with 'Another provider')",
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "providerChoice", hasRepo: true }),
  },
};

export const ProviderChoiceCreateNew: Story = {
  name: "ProviderChoice (without 'Another provider')",
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "providerChoice", hasRepo: false }),
  },
};

export const CredentialChoice: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "credentialChoice", hasRepo: true, provider: "github" }),
  },
};

export const OtherCredentialKindChoice: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "otherCredentialKindChoice", hasRepo: true, provider: "other" }),
  },
};

export const OauthSignInRequesting: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "oauthSignIn", hasRepo: true, provider: "github", credentialKind: "oauth" }),
    oauthStatus: "Requesting a device code from GitHub…",
  },
};

export const OauthSignInCodeShown: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "oauthSignIn", hasRepo: true, provider: "github", credentialKind: "oauth" }),
    oauthStatus: "Waiting for you to approve in the browser…",
    oauthDeviceCode: { verificationUri: "https://github.com/login/device", userCode: "ABCD-1234" },
  },
};

export const OauthSignInError: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "oauthSignIn", hasRepo: true, provider: "github", credentialKind: "oauth" }),
    oauthStatus: "GitHub sign-in was denied.",
  },
};

export const RepoVisibility: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "repoVisibility", hasRepo: false, provider: "github", credentialKind: "oauth" }),
  },
};

export const RepoVisibilityCreateError: Story = {
  name: "RepoVisibility (create error)",
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({
      step: "repoVisibility",
      hasRepo: false,
      provider: "github",
      credentialKind: "oauth",
      repoName: "my-notes",
    }),
    createRepoError: "Couldn't create the repository: a repository with that name already exists.",
  },
};

export const RepoPickerLoading: Story = {
  name: "RepoPicker (loading)",
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "repoPicker", hasRepo: true, provider: "github", credentialKind: "oauth" }),
    repoPickerStatus: "Loading your repositories…",
  },
};

export const RepoPickerEmpty: Story = {
  name: "RepoPicker (empty)",
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "repoPicker", hasRepo: true, provider: "github", credentialKind: "oauth" }),
    repoPickerStatus: "No repositories found for this account.",
  },
};

const SAMPLE_REPOS: ConnectRepoOption[] = [
  { fullName: "alice/notes", private: true, cloneUrl: "https://github.com/alice/notes.git" },
  { fullName: "alice/recipes", private: false, cloneUrl: "https://github.com/alice/recipes.git" },
];

export const RepoPickerList: Story = {
  name: "RepoPicker (list)",
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "repoPicker", hasRepo: true, provider: "github", credentialKind: "oauth" }),
    repos: SAMPLE_REPOS,
    repoPickerStatus: "Pick a repository:",
  },
};

export const RepoPickerError: Story = {
  name: "RepoPicker (error)",
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "repoPicker", hasRepo: true, provider: "github", credentialKind: "oauth" }),
    repoPickerStatus: "Couldn't load repositories: the network request failed.",
  },
};

export const PasteUrlToken: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "pasteUrl", hasRepo: true, provider: "other", credentialKind: "accessToken" }),
  },
};

export const PasteUrlTokenValidationError: Story = {
  name: "PasteUrlToken (validation error)",
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "pasteUrl", hasRepo: true, provider: "other", credentialKind: "accessToken" }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Connect" }));
    await expect(
      canvas.getByText("Repository URL, username, and access token are all required."),
    ).toBeInTheDocument();
  },
};

export const PasteUrlSshKey: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "pasteUrl", hasRepo: true, provider: "other", credentialKind: "sshKey" }),
    sshKeyStatus: "Key ready (fingerprint SHA256:abcd1234). Add the public key to your provider, then Connect.",
  },
};

export const PasteUrlSshKeyValidationError: Story = {
  name: "PasteUrlSshKey (validation error)",
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "pasteUrl", hasRepo: true, provider: "other", credentialKind: "sshKey" }),
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(await canvas.findByRole("button", { name: "Connect" }));
    await expect(canvas.getByText("Enter the repository's URL first.")).toBeInTheDocument();
  },
};

export const Connecting: Story = {
  args: {
    ...BASE_PROPS,
    state: state({
      step: "connecting",
      hasRepo: true,
      provider: "other",
      credentialKind: "accessToken",
      remoteUrl: "https://example.com/user/repo.git",
    }),
  },
};

export const ConnectError: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({
      step: "connectError",
      hasRepo: true,
      provider: "other",
      credentialKind: "accessToken",
      remoteUrl: "https://example.com/user/repo.git",
      error: "Couldn't connect: authentication failed.",
    }),
  },
};

export const CommitAuthorPrefilled: Story = {
  name: "CommitAuthor (prefilled)",
  args: {
    ...BASE_PROPS,
    state: state({ step: "commitAuthor" }),
    authorPrefill: { name: "Alice Example", email: "alice@example.com" },
  },
};

export const CommitAuthorError: Story = {
  name: "CommitAuthor (error)",
  args: {
    ...BASE_PROPS,
    state: state({ step: "commitAuthor" }),
    authorPrefill: { name: "Alice Example", email: "alice@example.com" },
    commitAuthorError: "Couldn't save: no vault is open.",
  },
};
