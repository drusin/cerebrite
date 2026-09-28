// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per interesting state from the ticket 09 checklist. Wrapped in a
// `.workspace` decorator: about 13 of styles.css's rules only apply under
// `.workspace`/`.workspace.compact`/`.workspace.sidebar-collapsed`
// (spec.md#vue-3).
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import TrashSurface from "./TrashSurface.vue";

const meta: Meta<typeof TrashSurface> = {
  component: TrashSurface,
  title: "Surfaces/Trash/TrashSurface",
  decorators: [
    (story) => ({
      components: { story },
      template: `<div class="workspace"><aside class="sidebar"><story /></aside></div>`,
    }),
  ],
};
export default meta;

type Story = StoryObj<typeof TrashSurface>;

export const Empty: Story = {
  args: { pages: [] },
};

export const Populated: Story = {
  args: {
    pages: [
      { id: "page-1", title: "Old meeting notes" },
      { id: "page-2", title: "Abandoned draft" },
      { id: "page-3", title: "Duplicate page" },
    ],
  },
};
