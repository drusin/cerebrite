// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook).
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import SshKey from "./SshKey.vue";

const meta: Meta<typeof SshKey> = {
  component: SshKey,
  title: "Components/SshKey",
};
export default meta;

type Story = StoryObj<typeof SshKey>;

export const NoKeyYet: Story = {
  args: { status: null },
};

export const KeyGenerated: Story = {
  args: {
    status: "Key ready (fingerprint SHA256:abcd1234). Add the public key to your provider, then Continue.",
  },
};

export const KeyImportError: Story = {
  args: {
    status: "Error: that doesn't look like a valid OpenSSH private key.",
  },
};
