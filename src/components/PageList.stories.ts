// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per interesting state, covering both variants this shared
// component renders (spec.md#shared-components).
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import PageList, { type PageListRow } from "./PageList.vue";

const meta: Meta<typeof PageList> = {
  component: PageList,
  title: "Components/PageList",
};
export default meta;

type Story = StoryObj<typeof PageList>;

const flatRows: PageListRow[] = [
  { kind: "item", key: "p1", value: "p1", label: "Meeting notes" },
  { kind: "item", key: "p2", value: "p2", label: "Project plan" },
  { kind: "item", key: "p3", value: "p3", label: "Weekly digest" },
];

export const Empty: Story = {
  args: { rows: [], emptyText: null, variant: "flat" },
};

export const EmptyWithHint: Story = {
  args: { rows: [], emptyText: "No pages opened yet.", variant: "flat" },
};

export const Populated: Story = {
  args: { rows: flatRows, variant: "flat" },
};

export const OneActive: Story = {
  args: {
    rows: flatRows.map((row) => (row.value === "p2" ? { ...row, active: true } : row)),
    variant: "flat",
  },
};

export const LongList: Story = {
  args: {
    rows: Array.from({ length: 40 }, (_, i) => ({
      kind: "item" as const,
      key: `page-${i}`,
      value: `page-${i}`,
      label: `Page ${i + 1}`,
    })),
    variant: "flat",
  },
};

const groupedRows: PageListRow[] = [
  { kind: "header", key: "h-page-1", value: "page-1", label: "Project plan" },
  { kind: "item", key: "s-page-1-0", value: "page-1", label: "…kicks off after the [[Meeting notes]] review…" },
  {
    kind: "item",
    key: "s-page-1-1",
    value: "page-1",
    label: "…see [[Meeting notes#Action items]] for follow-ups…",
    annotation: "→ Action items",
  },
  { kind: "header", key: "h-page-2", value: "page-2", label: "Weekly digest" },
  { kind: "item", key: "s-page-2-0", value: "page-2", label: "…linked from [[Meeting notes]] again this week…" },
];

export const GroupedEmpty: Story = {
  args: { rows: [], emptyText: "No backlinks yet.", variant: "grouped" },
};

export const GroupedPopulated: Story = {
  args: { rows: groupedRows, variant: "grouped" },
};
