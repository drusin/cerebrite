// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per interesting state from the ticket 10 checklist -- no backend
// calls, just hand-built props.
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import CloneManualForm from "./CloneManualForm.vue";

const meta: Meta<typeof CloneManualForm> = {
  component: CloneManualForm,
  title: "Surfaces/Clone manual form/CloneManualForm",
};
export default meta;

type Story = StoryObj<typeof CloneManualForm>;

const BASE_PROPS = {
  remoteUrl: "",
  destination: "",
  credentialKind: "accessToken" as const,
  tokenUsername: "",
  tokenValue: "",
  sshKeyStatus: null,
  githubDeviceCode: null,
  githubStatus: null,
  gitlabDeviceCode: null,
  gitlabStatus: null,
  submitting: false,
  statusMessage: null,
};

export const AccessTokenForm: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "accessToken",
  },
};

export const SshKeyForm: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "sshKey",
  },
};

export const GithubOauthForm: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "githubOauth",
  },
};

export const GitlabOauthForm: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "gitlabOauth",
  },
};

export const PrefilledFromWizard: Story = {
  args: {
    ...BASE_PROPS,
    remoteUrl: "https://example.com/user/repo.git",
    destination: "/home/user/notes",
    credentialKind: "sshKey",
  },
};

export const ValidationError: Story = {
  args: {
    ...BASE_PROPS,
    statusMessage: "Repository URL is required.",
  },
};

export const DeviceCodeShown: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "githubOauth",
    githubDeviceCode: { verificationUri: "https://github.com/login/device", userCode: "ABCD-1234" },
    githubStatus: "Waiting for you to approve in the browser…",
  },
};

export const SignedInClickCloneToContinue: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "gitlabOauth",
    gitlabStatus: "Signed in. Click Clone to continue.",
  },
};

export const Cloning: Story = {
  args: {
    ...BASE_PROPS,
    remoteUrl: "https://example.com/user/repo.git",
    destination: "/home/user/notes",
    submitting: true,
    statusMessage: "Cloning…",
  },
};

export const CloneError: Story = {
  args: {
    ...BASE_PROPS,
    remoteUrl: "https://example.com/user/repo.git",
    destination: "/home/user/notes",
    statusMessage: "Couldn't clone: that folder isn't empty.",
  },
};
