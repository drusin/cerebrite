// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook).
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import CommitAs from "./CommitAs.vue";

const meta: Meta<typeof CommitAs> = {
  component: CommitAs,
  title: "Components/CommitAs",
};
export default meta;

type Story = StoryObj<typeof CommitAs>;

export const Prefilled: Story = {
  args: {
    prefill: { name: "Alice Example", email: "alice@example.com" },
    error: null,
  },
};

export const Empty: Story = {
  args: {
    prefill: null,
    error: null,
  },
};

export const Error: Story = {
  args: {
    prefill: { name: "Alice Example", email: "alice@example.com" },
    error: "Couldn't save: no vault is open.",
  },
};
