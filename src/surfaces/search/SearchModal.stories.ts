// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per interesting state from the ticket 03 checklist.
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import SearchModal from "./SearchModal.vue";
import type { SearchEntry, SearchResultEntry } from "./search-entry";

const meta: Meta<typeof SearchModal> = {
  component: SearchModal,
  title: "Surfaces/Search/SearchModal",
};
export default meta;

type Story = StoryObj<typeof SearchModal>;

function result(overrides: Partial<SearchResultEntry> & Pick<SearchResultEntry, "id" | "title" | "tier">): SearchResultEntry {
  return {
    snippet: overrides.title,
    inTrash: false,
    matchedTag: null,
    ...overrides,
  };
}

export const EmptyQuery: Story = {
  args: {
    query: "",
    includeTrash: false,
    entries: [],
    selectedIndex: -1,
  },
};

export const NoResults: Story = {
  args: {
    query: "no such page",
    includeTrash: false,
    entries: [{ kind: "create", query: "no such page" }],
    selectedIndex: 0,
  },
};

export const Tier1TitleHit: Story = {
  args: {
    query: "Recipes",
    includeTrash: false,
    entries: [
      { kind: "result", result: result({ id: "1", title: "Recipes", tier: 1 }) },
      { kind: "create", query: "Recipes" },
    ],
    selectedIndex: 0,
  },
};

export const Tier2TagHit: Story = {
  args: {
    query: "cooking",
    includeTrash: false,
    entries: [
      {
        kind: "result",
        result: result({ id: "2", title: "Sunday Dinner", tier: 2, snippet: "#cooking", matchedTag: "cooking" }),
      },
      { kind: "create", query: "cooking" },
    ],
    selectedIndex: 0,
  },
};

export const Tier3SnippetHit: Story = {
  args: {
    query: "garden",
    includeTrash: false,
    entries: [
      {
        kind: "result",
        result: result({
          id: "3",
          title: "Backyard Notes",
          tier: 3,
          snippet: "watering the \u0001garden\u0002 every morning before work",
        }),
      },
      { kind: "create", query: "garden" },
    ],
    selectedIndex: 0,
  },
};

export const InTrashResult: Story = {
  args: {
    query: "Old Draft",
    includeTrash: true,
    entries: [
      { kind: "result", result: result({ id: "4", title: "Old Draft", tier: 1, inTrash: true }) },
      { kind: "create", query: "Old Draft" },
    ],
    selectedIndex: 0,
  },
};

export const IncludeTrashOn: Story = {
  args: {
    query: "Old Draft",
    includeTrash: true,
    entries: [
      { kind: "result", result: result({ id: "4", title: "Old Draft", tier: 1, inTrash: true }) },
      { kind: "create", query: "Old Draft" },
    ],
    selectedIndex: 0,
  },
};

export const IncludeTrashOff: Story = {
  args: {
    query: "Old Draft",
    includeTrash: false,
    entries: [{ kind: "create", query: "Old Draft" }],
    selectedIndex: 0,
  },
};

const KEYBOARD_SELECTION_ENTRIES: SearchEntry[] = [
  { kind: "result", result: result({ id: "5", title: "Alpha", tier: 1 }) },
  { kind: "result", result: result({ id: "6", title: "Alphabet", tier: 1 }) },
  { kind: "create", query: "Alpha" },
];

export const KeyboardSelectionMoved: Story = {
  args: {
    query: "Alpha",
    includeTrash: false,
    entries: KEYBOARD_SELECTION_ENTRIES,
    // Selection moved off the top row (index 0) via ArrowDown, onto the
    // second result.
    selectedIndex: 1,
  },
};

export const ExactTitleMatch: Story = {
  args: {
    query: "Recipes",
    includeTrash: false,
    // No trailing "Create page" row: the search found an exact title match.
    entries: [{ kind: "result", result: result({ id: "1", title: "Recipes", tier: 1 }) }],
    selectedIndex: 0,
  },
};
