<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. This was the
// **first modal** (spec.md#step-2-search-modal), written with its own
// inline overlay/backdrop; ticket 11 is the *second* modal (the connect
// wizard), so the shared overlay/backdrop/Escape shell is extracted into
// `components/Modal.vue` and this surface switches to it here. Renders
// `#search-modal-overlay`'s old markup/classes verbatim -- `Modal` is given
// those exact class names as props, so nothing visual changes.
import { computed, ref, watch } from "vue";
import Modal from "../../components/Modal.vue";
import type { SearchEntry, SearchResultEntry } from "./search-entry";

const props = defineProps<{
  query: string;
  includeTrash: boolean;
  entries: SearchEntry[];
  selectedIndex: number;
}>();

const emit = defineEmits<{
  close: [];
  "update:query": [value: string];
  "update:includeTrash": [value: boolean];
  selectIndex: [index: number];
  /** Opens a resolved result (a temporary callback root prop on the
   * container resolves the title -- see its own doc comment). */
  open: [result: SearchResultEntry];
  /** Creates a page from the typed query, then opens it (also a temporary
   * callback root prop on the container). */
  create: [query: string];
}>();

const resultsListEl = ref<HTMLElement | null>(null);
const inputEl = ref<HTMLInputElement | null>(null);

// Imperative escape hatch (spec.md#imperative-escape-hatches): a stateless
// one-shot effect only, never used to read state back -- the container
// calls this once, right after opening.
defineExpose({
  focus(): void {
    inputEl.value?.focus();
  },
});

const emptyHint = computed<string | null>(() => {
  if (props.query.trim() === "") return "Type to search.";
  if (props.entries.length === 0) return "No results.";
  return null;
});

function searchResultRowLabel(result: SearchResultEntry): string {
  return result.inTrash ? `${result.title} (in trash)` : result.title;
}

// FTS5 snippets built with `char(1)`/`char(2)` markers (search.rs's
// `body_fts_matches`) around each matched span -- turned into plain text
// segments here (never `v-html`) so the template can wrap the marked one(s)
// in `<mark>`.
const SNIPPET_MARK_START = "\u0001";
const SNIPPET_MARK_END = "\u0002";

function snippetParts(snippet: string): { text: string; marked: boolean }[] {
  const segments = snippet.split(SNIPPET_MARK_START);
  const parts: { text: string; marked: boolean }[] = [{ text: segments[0] ?? "", marked: false }];
  for (const segment of segments.slice(1)) {
    const [marked, ...restParts] = segment.split(SNIPPET_MARK_END);
    parts.push({ text: marked ?? "", marked: true });
    parts.push({ text: restParts.join(SNIPPET_MARK_END), marked: false });
  }
  return parts;
}

function activateEntry(entry: SearchEntry) {
  if (entry.kind === "result") emit("open", entry.result);
  else emit("create", entry.query);
}

function moveSelection(delta: number) {
  if (props.entries.length === 0) return;
  const next = props.selectedIndex < 0 ? 0 : props.selectedIndex + delta;
  emit("selectIndex", Math.max(0, Math.min(props.entries.length - 1, next)));
}

function activateSelected() {
  const index = props.selectedIndex >= 0 ? props.selectedIndex : 0;
  const entry = props.entries[index];
  if (entry) activateEntry(entry);
}

// Escape is handled by `Modal` itself now (see the template's `@close`) --
// this only handles the keys `Modal` doesn't know about. Bound on `Modal`
// via Vue's attribute/listener fallthrough (a non-prop listener on a
// component falls through to its single root element -- `Modal`'s own
// overlay div), so both listeners fire on the same bubbled keydown.
function handleKeydown(event: KeyboardEvent) {
  switch (event.key) {
    case "ArrowDown":
      event.preventDefault();
      moveSelection(1);
      break;
    case "ArrowUp":
      event.preventDefault();
      moveSelection(-1);
      break;
    case "Enter":
      event.preventDefault();
      activateSelected();
      break;
  }
}

// Keeps the selected row in view as arrow keys move the selection, the same
// way the old `moveSearchSelection` did with `scrollIntoView`.
watch(
  () => props.selectedIndex,
  (index) => {
    if (index < 0) return;
    const rows = resultsListEl.value?.querySelectorAll<HTMLButtonElement>(".search-result");
    rows?.[index]?.scrollIntoView({ block: "nearest" });
  },
);
</script>

<template>
  <Modal
    label="Search"
    overlay-class="search-modal-overlay"
    card-class="search-modal"
    @close="emit('close')"
    @keydown="handleKeydown"
  >
    <div class="search-modal-header">
      <input
        ref="inputEl"
        class="search-input"
        type="text"
        placeholder="Search title, tags, and body…"
        autocomplete="off"
        spellcheck="false"
        :value="query"
        @input="emit('update:query', ($event.target as HTMLInputElement).value)"
      />
      <label class="search-include-trash">
        <input
          type="checkbox"
          :checked="includeTrash"
          @change="emit('update:includeTrash', ($event.target as HTMLInputElement).checked)"
        />
        Include trash
      </label>
    </div>
    <ul ref="resultsListEl" class="search-results-list">
      <li v-for="(entry, index) in entries" :key="entry.kind === 'result' ? entry.result.id : 'create'">
        <button
          type="button"
          class="search-result"
          :class="{ active: index === selectedIndex, 'create-page-action': entry.kind === 'create' }"
          @click="activateEntry(entry)"
        >
          <template v-if="entry.kind === 'result'">
            <span class="search-result-title">
              <span>{{ searchResultRowLabel(entry.result) }}</span>
              <span v-if="entry.result.inTrash" class="search-result-in-trash-badge">In trash</span>
            </span>
            <span v-if="entry.result.tier === 2 && entry.result.matchedTag" class="search-result-tag-chip">
              #{{ entry.result.matchedTag }}
            </span>
            <span v-else-if="entry.result.tier === 3 && entry.result.snippet" class="search-result-snippet">
              <template v-for="(part, partIndex) in snippetParts(entry.result.snippet)" :key="partIndex">
                <mark v-if="part.marked">{{ part.text }}</mark>
                <template v-else>{{ part.text }}</template>
              </template>
            </span>
          </template>
          <template v-else>Create page: '{{ entry.query }}'</template>
        </button>
      </li>
    </ul>
    <p v-if="emptyHint" class="search-empty-hint">{{ emptyHint }}</p>
  </Modal>
</template>
