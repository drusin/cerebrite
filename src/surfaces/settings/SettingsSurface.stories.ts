// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per interesting state from the ticket 12 checklist -- no backend
// calls, just hand-built props.
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import SettingsSurface from "./SettingsSurface.vue";

const meta: Meta<typeof SettingsSurface> = {
  component: SettingsSurface,
  title: "Surfaces/Settings/SettingsSurface",
};
export default meta;

type Story = StoryObj<typeof SettingsSurface>;

const BASE_PROPS = {
  vaultPath: "/home/user/notes",
  theme: "system" as const,
  commitAuthorPrefill: { name: "Alice Example", email: "alice@example.com" },
  commitAuthorError: null,
  commitAuthorSavedMessage: null,
  syncConnected: false,
  credentialKind: "accessToken" as const,
  remoteUrl: "",
  tokenUsername: "",
  tokenValue: "",
  accessTokenStatus: null,
  accessTokenConnecting: false,
  sshKeyStatus: null,
  sshKeyConnecting: false,
  githubDeviceCode: null,
  githubStatus: null,
  githubInstallUrl: null,
  githubInstallContinuing: false,
  gitlabDeviceCode: null,
  gitlabStatus: null,
  orphanVisible: false,
  orphanCleaning: false,
  removeAllStatus: null,
  removeAllRemoving: false,
};

export const Default: Story = {
  args: { ...BASE_PROPS },
};

export const NoVaultPath: Story = {
  args: { ...BASE_PROPS, vaultPath: null },
};

export const CommitAuthorPrefilled: Story = {
  name: "Commit as: prefilled",
  args: { ...BASE_PROPS },
};

export const CommitAuthorSaved: Story = {
  name: "Commit as: saved",
  args: { ...BASE_PROPS, commitAuthorSavedMessage: "Saved." },
};

export const CommitAuthorSavedWithWarning: Story = {
  name: "Commit as: saved with a warning",
  args: {
    ...BASE_PROPS,
    commitAuthorSavedMessage: "Saved. This email's domain doesn't look real -- commits will still show it as-is.",
  },
};

export const CommitAuthorError: Story = {
  name: "Commit as: error",
  args: { ...BASE_PROPS, commitAuthorError: "Name and email are both required." },
};

export const SyncNotConnected: Story = {
  name: "Sync: not-connected banner",
  args: { ...BASE_PROPS, syncConnected: false },
};

export const SyncConnectedShowsDisconnect: Story = {
  name: "Sync: Disconnect button",
  args: { ...BASE_PROPS, syncConnected: true },
};

export const AccessTokenSubForm: Story = {
  name: "Sub-form: access token",
  args: { ...BASE_PROPS, credentialKind: "accessToken" },
};

export const AccessTokenConnecting: Story = {
  name: "Sub-form: access token, Connecting…",
  args: { ...BASE_PROPS, credentialKind: "accessToken", accessTokenStatus: "Connecting…", accessTokenConnecting: true },
};

export const AccessTokenConnected: Story = {
  name: "Sub-form: access token, Connected.",
  args: { ...BASE_PROPS, credentialKind: "accessToken", accessTokenStatus: "Connected." },
};

export const AccessTokenError: Story = {
  name: "Sub-form: access token, error",
  args: { ...BASE_PROPS, credentialKind: "accessToken", accessTokenStatus: "Couldn't connect: bad credentials." },
};

export const SshKeySubForm: Story = {
  name: "Sub-form: SSH key",
  args: { ...BASE_PROPS, credentialKind: "sshKey" },
};

export const SshKeyConnecting: Story = {
  name: "Sub-form: SSH key, Connecting…",
  args: {
    ...BASE_PROPS,
    credentialKind: "sshKey",
    sshKeyStatus: "Connecting…",
    sshKeyConnecting: true,
  },
};

export const SshKeyConnected: Story = {
  name: "Sub-form: SSH key, Connected.",
  args: { ...BASE_PROPS, credentialKind: "sshKey", sshKeyStatus: "Connected." },
};

export const SshKeyError: Story = {
  name: "Sub-form: SSH key, error",
  args: { ...BASE_PROPS, credentialKind: "sshKey", sshKeyStatus: "Couldn't connect: host key rejected." },
};

export const GithubDeviceCodeShown: Story = {
  name: "GitHub: device code shown",
  args: {
    ...BASE_PROPS,
    credentialKind: "githubOauth",
    githubDeviceCode: { verificationUri: "https://github.com/login/device", userCode: "ABCD-1234" },
    githubStatus: "Waiting for you to approve in the browser…",
  },
};

export const GithubInstallCta: Story = {
  name: "GitHub: install CTA",
  args: {
    ...BASE_PROPS,
    credentialKind: "githubOauth",
    githubInstallUrl: "https://github.com/apps/cerebrite/installations/new",
  },
};

export const GitlabSubForm: Story = {
  name: "Sub-form: GitLab sign-in",
  args: {
    ...BASE_PROPS,
    credentialKind: "gitlabOauth",
    gitlabStatus: "Connecting…",
  },
};

export const OrphanNoticeShown: Story = {
  name: "Credentials: orphan notice shown",
  args: { ...BASE_PROPS, orphanVisible: true },
};

export const OrphanNoticeHidden: Story = {
  name: "Credentials: orphan notice hidden",
  args: { ...BASE_PROPS, orphanVisible: false },
};

export const RemoveAllSucceeded: Story = {
  name: "Credentials: remove-all fully succeeded",
  args: { ...BASE_PROPS, removeAllStatus: "All stored credentials were removed." },
};

export const RemoveAllPartiallyFailed: Story = {
  name: "Credentials: remove-all partially failed",
  args: {
    ...BASE_PROPS,
    removeAllStatus: "Removed what it could -- 2 connection(s) could not be fully removed.",
  },
};
