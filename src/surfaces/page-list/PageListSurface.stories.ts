// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per interesting state from the ticket 08 checklist. Wrapped in a
// `.workspace` decorator: about 13 of styles.css's rules only apply under
// `.workspace`/`.workspace.compact`/`.workspace.sidebar-collapsed`
// (spec.md#vue-3).
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import PageListSurface from "./PageListSurface.vue";

const meta: Meta<typeof PageListSurface> = {
  component: PageListSurface,
  title: "Surfaces/PageList/PageListSurface",
  decorators: [
    (story) => ({
      components: { story },
      template: `<div class="workspace"><aside class="sidebar"><story /></aside></div>`,
    }),
  ],
};
export default meta;

type Story = StoryObj<typeof PageListSurface>;

export const Empty: Story = {
  args: { pages: [], activePageId: null },
};

export const Populated: Story = {
  args: {
    pages: [
      { id: "page-1", title: "Meeting notes" },
      { id: "page-2", title: "Project plan" },
      { id: "page-3", title: "Weekly digest" },
    ],
    activePageId: null,
  },
};

export const OneActivePage: Story = {
  args: {
    pages: [
      { id: "page-1", title: "Meeting notes" },
      { id: "page-2", title: "Project plan" },
      { id: "page-3", title: "Weekly digest" },
    ],
    activePageId: "page-2",
  },
};

export const LongList: Story = {
  args: {
    pages: Array.from({ length: 40 }, (_, i) => ({ id: `page-${i}`, title: `Page ${i + 1}` })),
    activePageId: "page-12",
  },
};
