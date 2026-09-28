// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook).
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import CredentialKindForm from "./CredentialKindForm.vue";

const meta: Meta<typeof CredentialKindForm> = {
  component: CredentialKindForm,
  title: "Components/CredentialKindForm",
};
export default meta;

type Story = StoryObj<typeof CredentialKindForm>;

const BASE_PROPS = {
  remoteUrl: "",
  tokenUsername: "",
  tokenValue: "",
  sshKeyStatus: null,
  githubDeviceCode: null,
  githubStatus: null,
  gitlabDeviceCode: null,
  gitlabStatus: null,
};

export const AccessToken: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "accessToken",
    connectLabel: "Connect",
  },
};

export const SshKey: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "sshKey",
    connectLabel: "Connect",
  },
};

export const SshKeyReady: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "sshKey",
    sshKeyStatus: "Key ready (fingerprint SHA256:abc123). Add the public key to your provider, then Connect.",
    connectLabel: "Connect",
  },
};

export const GithubOauth: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "githubOauth",
  },
};

export const GithubDeviceCodeShown: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "githubOauth",
    githubDeviceCode: { verificationUri: "https://github.com/login/device", userCode: "ABCD-1234" },
    githubStatus: "Waiting for you to approve in the browser…",
  },
};

export const GitlabOauth: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "gitlabOauth",
  },
};

export const GitlabConnecting: Story = {
  args: {
    ...BASE_PROPS,
    credentialKind: "gitlabOauth",
    gitlabStatus: "Connecting…",
  },
};

export const WithoutUrlFieldOrConnectAction: Story = {
  name: "Without URL field or Connect action (clone manual form usage)",
  args: {
    ...BASE_PROPS,
    credentialKind: "sshKey",
    showUrlField: false,
  },
};
