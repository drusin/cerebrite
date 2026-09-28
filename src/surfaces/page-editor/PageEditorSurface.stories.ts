// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook).
// `change`/`linkClick`/`commit` show up in the Actions panel automatically
// (Storybook's Vue3 framework wires declared `defineEmits` up on its own).
// One named story per sample document, per the ticket 06 checklist.
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import PageEditorSurface from "./PageEditorSurface.vue";

const meta: Meta<typeof PageEditorSurface> = {
  component: PageEditorSurface,
  title: "Surfaces/Page editor/PageEditorSurface",
};
export default meta;

type Story = StoryObj<typeof PageEditorSurface>;

export const PlainProse: Story = {
  args: {
    pageKey: "d:Plain prose",
    markdown: `# Plain prose

Just paragraphs, **bold**, *italic*, and \`code\`. No links at all.

A second paragraph so there's something to edit.

- a list
- with two items
`,
  },
};

export const WikiLinks: Story = {
  args: {
    pageKey: "d:Wiki-links",
    markdown: `# Wiki-links

Plain page link: [[Plain prose]].

Link to a heading on another page: [[Long document#Section 7]].

Link to a page that doesn't exist yet (dynamic page): [[Not written yet]].

Link to a heading on *this* page: [[Wiki-links#Targets]].

## Targets

The heading above is the target of the last link.
`,
  },
};

const longSections = Array.from({ length: 12 }, (_, i) => {
  const n = i + 1;
  return `## Section ${n}\n\nParagraph ${n}. ${"Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(6)}\n\n- point ${n}.a\n- point ${n}.b links to [[Plain prose]]\n`;
}).join("\n");

export const LongDocument: Story = {
  args: {
    pageKey: "p:long-document",
    markdown: `# Long document

About a dozen sections, for scroll-to-heading.

${longSections}`,
  },
};
