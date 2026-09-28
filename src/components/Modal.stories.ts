// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook).
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import Modal from "./Modal.vue";

const meta: Meta<typeof Modal> = {
  component: Modal,
  title: "Components/Modal",
};
export default meta;

type Story = StoryObj<typeof Modal>;

export const SearchSized: Story = {
  args: {
    label: "Search",
    overlayClass: "search-modal-overlay",
    cardClass: "search-modal",
  },
  render: (args) => ({
    components: { Modal },
    setup: () => ({ args }),
    template: `<Modal v-bind="args"><p style="padding: 1em;">Search modal content goes here.</p></Modal>`,
  }),
};

export const SettingsSized: Story = {
  args: {
    label: "Connect a repository",
    overlayClass: "settings-modal-overlay",
    cardClass: "settings-modal connect-wizard",
  },
  render: (args) => ({
    components: { Modal },
    setup: () => ({ args }),
    template: `<Modal v-bind="args"><p style="padding: 1em;">Settings-sized modal content goes here.</p></Modal>`,
  }),
};
