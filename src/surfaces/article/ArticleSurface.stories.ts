// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per interesting state from the ticket 07 checklist. The `editor`
// slot gets a plain placeholder here -- the real editor
// (`surfaces/page-editor/PageEditorSurface.vue`) already has its own
// stories, and this surface never imports it (it only declares the slot).
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import ArticleSurface from "./ArticleSurface.vue";
import type { BacklinkEntry } from "./backlink-entry";

const meta: Meta<typeof ArticleSurface> = {
  component: ArticleSurface,
  title: "Surfaces/Article/ArticleSurface",
};
export default meta;

type Story = StoryObj<typeof ArticleSurface>;

const editorPlaceholder = {
  template: `<ArticleSurface v-bind="args"><template #editor><div>(editor content)</div></template></ArticleSurface>`,
};

export const NothingSelected: Story = {
  args: {
    title: null,
    deletable: false,
    inTrash: false,
    backlinks: [],
  },
  render: (args) => ({
    components: { ArticleSurface },
    setup: () => ({ args }),
    ...editorPlaceholder,
  }),
};

export const PersistedPage: Story = {
  args: {
    title: "Meeting notes",
    deletable: true,
    inTrash: false,
    backlinks: [],
  },
  render: (args) => ({
    components: { ArticleSurface },
    setup: () => ({ args }),
    ...editorPlaceholder,
  }),
};

export const DynamicPage: Story = {
  args: {
    title: "Not written yet",
    deletable: false,
    inTrash: false,
    backlinks: [],
  },
  render: (args) => ({
    components: { ArticleSurface },
    setup: () => ({ args }),
    ...editorPlaceholder,
  }),
};

export const InTrashPage: Story = {
  args: {
    title: "Old draft",
    deletable: false,
    inTrash: true,
    backlinks: [],
  },
  render: (args) => ({
    components: { ArticleSurface },
    setup: () => ({ args }),
    ...editorPlaceholder,
  }),
};

export const BacklinksEmpty: Story = {
  args: {
    title: "A lonely page",
    deletable: true,
    inTrash: false,
    backlinks: [],
  },
  render: (args) => ({
    components: { ArticleSurface },
    setup: () => ({ args }),
    ...editorPlaceholder,
  }),
};

const groupedBacklinks: BacklinkEntry[] = [
  {
    sourceId: "page-1",
    sourceTitle: "Project plan",
    snippet: "…kicks off after the [[Meeting notes]] review…",
    modifiedAt: Date.now(),
    targetHeadingSlug: null,
  },
  {
    sourceId: "page-1",
    sourceTitle: "Project plan",
    snippet: "…see [[Meeting notes#Action items]] for follow-ups…",
    modifiedAt: Date.now(),
    targetHeadingSlug: "action-items",
  },
  {
    sourceId: "page-2",
    sourceTitle: "Weekly digest",
    snippet: "…linked from [[Meeting notes]] again this week…",
    modifiedAt: Date.now(),
    targetHeadingSlug: null,
  },
];

export const BacklinksGroupedBySource: Story = {
  args: {
    title: "Meeting notes",
    deletable: true,
    inTrash: false,
    backlinks: groupedBacklinks,
  },
  render: (args) => ({
    components: { ArticleSurface },
    setup: () => ({ args }),
    ...editorPlaceholder,
  }),
};
