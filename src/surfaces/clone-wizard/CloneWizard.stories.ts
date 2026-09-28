// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per reducer step, per the ticket 10 checklist -- no backend calls,
// just a hand-built `CloneWizardState` (plus whatever ephemeral prop each
// step also needs) fed straight in as props.
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import CloneWizard, { type CloneRepoOption } from "./CloneWizard.vue";
import { initialCloneWizardState, type CloneWizardState } from "../../clone-wizard";

const meta: Meta<typeof CloneWizard> = {
  component: CloneWizard,
  title: "Surfaces/Clone wizard/CloneWizard",
};
export default meta;

type Story = StoryObj<typeof CloneWizard>;

function state(overrides: Partial<CloneWizardState>): CloneWizardState {
  return { ...initialCloneWizardState, ...overrides };
}

const BASE_PROPS = {
  canGoBack: false,
  oauthStatus: "",
  oauthDeviceCode: null,
  repos: [],
  repoPickerStatus: "",
  sshKeyStatus: null,
  destinationStatus: null,
  authorPrefill: null,
  commitAuthorError: null,
};

export const ProviderChoice: Story = {
  args: {
    ...BASE_PROPS,
    state: state({ step: "providerChoice" }),
  },
};

export const CredentialChoice: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "credentialChoice", provider: "github" }),
  },
};

export const OtherCredentialKindChoice: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "otherCredentialKindChoice", provider: "other" }),
  },
};

export const OauthSignIn: Story = {
  args: {
    ...BASE_PROPS,
    state: state({ step: "oauthSignIn", provider: "github", credentialKind: "oauth" }),
    oauthStatus: "Waiting for you to approve in the browser…",
    oauthDeviceCode: { verificationUri: "https://github.com/login/device", userCode: "ABCD-1234" },
  },
};

const SAMPLE_REPOS: CloneRepoOption[] = [
  { fullName: "alice/notes", private: true, cloneUrl: "https://github.com/alice/notes.git" },
  { fullName: "alice/recipes", private: false, cloneUrl: "https://github.com/alice/recipes.git" },
];

export const RepoPicker: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "repoPicker", provider: "github", credentialKind: "oauth" }),
    repos: SAMPLE_REPOS,
    repoPickerStatus: "Pick a repository to clone:",
  },
};

export const PasteUrlToken: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "pasteUrl", provider: "other", credentialKind: "accessToken" }),
  },
};

export const PasteUrlSshKey: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({ step: "pasteUrl", provider: "other", credentialKind: "sshKey" }),
    sshKeyStatus: "Key ready (fingerprint SHA256:abcd1234). Add the public key to your provider, then Continue.",
  },
};

export const DestinationPicker: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({
      step: "destinationPicker",
      provider: "other",
      credentialKind: "accessToken",
      remoteUrl: "https://example.com/user/repo.git",
    }),
  },
};

export const Cloning: Story = {
  args: {
    ...BASE_PROPS,
    state: state({
      step: "cloning",
      provider: "other",
      credentialKind: "accessToken",
      remoteUrl: "https://example.com/user/repo.git",
      destination: "/home/user/notes",
    }),
  },
};

export const CloneError: Story = {
  args: {
    ...BASE_PROPS,
    canGoBack: true,
    state: state({
      step: "cloneError",
      provider: "other",
      credentialKind: "accessToken",
      remoteUrl: "https://example.com/user/repo.git",
      destination: "/home/user/notes",
      error: "Couldn't clone: that folder isn't empty.",
    }),
  },
};

export const CommitAuthor: Story = {
  args: {
    ...BASE_PROPS,
    state: state({ step: "commitAuthor" }),
    authorPrefill: { name: "Alice Example", email: "alice@example.com" },
  },
};
