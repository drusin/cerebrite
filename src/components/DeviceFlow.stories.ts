// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook).
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import DeviceFlow from "./DeviceFlow.vue";

const meta: Meta<typeof DeviceFlow> = {
  component: DeviceFlow,
  title: "Components/DeviceFlow",
};
export default meta;

type Story = StoryObj<typeof DeviceFlow>;

export const Requesting: Story = {
  args: {
    status: "Requesting a device code from GitHub…",
    deviceCode: null,
  },
};

export const CodeShown: Story = {
  args: {
    status: "Waiting for you to approve in the browser…",
    deviceCode: { verificationUri: "https://github.com/login/device", userCode: "ABCD-1234" },
  },
};

export const Error: Story = {
  args: {
    status: "GitHub sign-in was denied.",
    deviceCode: null,
  },
};
