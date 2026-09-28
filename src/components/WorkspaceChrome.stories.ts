// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per interesting layout state, per the ticket 13 checklist. Slot
// content is plain placeholder markup -- the real surfaces (Recent, "All
// pages", Trash, the sync indicator, the article view) are `App.vue`'s job
// to slot in, not this presentational component's.
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import WorkspaceChrome from "./WorkspaceChrome.vue";

const meta: Meta<typeof WorkspaceChrome> = {
  component: WorkspaceChrome,
  title: "Components/WorkspaceChrome",
};
export default meta;

type Story = StoryObj<typeof WorkspaceChrome>;

const slots = `
  <template #recent><div class="sidebar-section"><h2>Recent</h2><p class="sidebar-empty-hint">No pages opened yet.</p></div></template>
  <template #pageList><div class="sidebar-section"><h2>All pages</h2><p class="sidebar-empty-hint">No pages yet.</p></div></template>
  <template #trash><div class="sidebar-section"><h2>Trash</h2><p class="sidebar-empty-hint">Trash is empty.</p></div></template>
  <template #syncFooter><button type="button">✓ Synced</button></template>
  <template #syncRail><button type="button">✓</button></template>
  <template #main><div style="padding: 2em;"><h1>Article title</h1><p>Article content goes here.</p></div></template>
`;

export const ExpandedDesktop: Story = {
  args: { collapsed: false, compact: false, drawerOpen: false },
  render: (args) => ({
    components: { WorkspaceChrome },
    setup: () => ({ args }),
    template: `<WorkspaceChrome v-bind="args">${slots}</WorkspaceChrome>`,
  }),
};

export const CollapsedRail: Story = {
  args: { collapsed: true, compact: false, drawerOpen: false },
  render: (args) => ({
    components: { WorkspaceChrome },
    setup: () => ({ args }),
    template: `<WorkspaceChrome v-bind="args">${slots}</WorkspaceChrome>`,
  }),
};

export const CompactDrawerClosed: Story = {
  args: { collapsed: false, compact: true, drawerOpen: false },
  render: (args) => ({
    components: { WorkspaceChrome },
    setup: () => ({ args }),
    template: `<WorkspaceChrome v-bind="args">${slots}</WorkspaceChrome>`,
  }),
};

export const CompactDrawerOpen: Story = {
  args: { collapsed: false, compact: true, drawerOpen: true },
  render: (args) => ({
    components: { WorkspaceChrome },
    setup: () => ({ args }),
    template: `<WorkspaceChrome v-bind="args">${slots}</WorkspaceChrome>`,
  }),
};
