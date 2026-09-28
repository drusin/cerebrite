// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per interesting state from the ticket 08 checklist. Wrapped in a
// `.workspace` decorator: about 13 of styles.css's rules only apply under
// `.workspace`/`.workspace.compact`/`.workspace.sidebar-collapsed`
// (spec.md#vue-3).
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import RecentSurface from "./RecentSurface.vue";
import type { RecentEntry } from "./recent-entry";

const meta: Meta<typeof RecentSurface> = {
  component: RecentSurface,
  title: "Surfaces/Recent/RecentSurface",
  decorators: [
    (story) => ({
      components: { story },
      template: `<div class="workspace"><aside class="sidebar"><story /></aside></div>`,
    }),
  ],
};
export default meta;

type Story = StoryObj<typeof RecentSurface>;

export const Empty: Story = {
  args: { entries: [], activeKey: null },
};

const mixedEntries: RecentEntry[] = [
  { key: "p:page-1", title: "Meeting notes", kind: "persisted", pageId: "page-1" },
  { key: "d:Untitled idea", title: "Untitled idea", kind: "dynamic" },
  { key: "p:page-2", title: "Project plan", kind: "persisted", pageId: "page-2" },
];

export const Populated: Story = {
  args: { entries: mixedEntries, activeKey: null },
};

export const ActiveEntry: Story = {
  args: { entries: mixedEntries, activeKey: "d:Untitled idea" },
};
