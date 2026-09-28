<script setup lang="ts">
// Container (spec.md#surface-contract): owns the search variables --
// they're local to this container, not `src/state/`, per
// spec.md#step-2-search-modal -- the debounce, and every backend call. No
// story (containers aren't storied).
import { computed, nextTick, ref, watch } from "vue";
import { modal, closeModal } from "../../state/ui";
import { searchPages, type SearchResult } from "../../vault-api";
import SearchModal from "./SearchModal.vue";
import type { SearchEntry, SearchResultEntry } from "./search-entry";

// Temporary callback root props (spec.md#islands-and-how-they-merge): page
// logic itself doesn't move into `src/state/` until step 4, so both intents
// this modal needs still live in `main.ts` and arrive here as root props.
// Each already closes the modal (via `state/ui.ts`'s `closeModal`) and
// handles its own errors internally -- see their doc comments in main.ts.
const props = defineProps<{
  /** Opens the resolved search result the same way clicking any
   * `[[Link]]` chip or a Trash-list entry would. */
  openPageByTitleInVanilla: (title: string) => Promise<void>;
  /** Creates the typed query as a brand-new page and opens it straight
   * into the editor. */
  createPageFromQueryInVanilla: (query: string) => Promise<void>;
}>();

const SEARCH_DEBOUNCE_MS = 120;

const isOpen = computed(() => modal.value === "search");

const query = ref("");
const includeTrash = ref(false);
const entries = ref<SearchEntry[]>([]);
const selectedIndex = ref(-1);

let debounceTimer: ReturnType<typeof setTimeout> | null = null;
/** Guards against a slow, now-stale search response overwriting a newer one. */
let requestId = 0;

function toEntryResult(result: SearchResult): SearchResultEntry {
  return result;
}

/**
 * Runs one search for whatever's currently in `query`, builds the
 * strict-tier results into `entries`, and appends the "Create page" action
 * whenever there's no exact title match among the results -- not only when
 * there are zero results, per the prototype spec.
 */
async function runSearch() {
  const trimmed = query.value.trim();
  if (trimmed === "") {
    entries.value = [];
    selectedIndex.value = -1;
    return;
  }

  const thisRequestId = ++requestId;
  let results: SearchResult[];
  try {
    results = await searchPages(trimmed, includeTrash.value);
  } catch (err) {
    console.error("Search failed", err);
    results = [];
  }
  if (thisRequestId !== requestId) return; // a newer search has since started

  const trimmedLower = trimmed.toLowerCase();
  const hasExactTitleMatch = results.some((r) => r.title.toLowerCase() === trimmedLower);

  const nextEntries: SearchEntry[] = results.map((result): SearchEntry => ({
    kind: "result",
    result: toEntryResult(result),
  }));
  if (!hasExactTitleMatch) {
    nextEntries.push({ kind: "create", query: trimmed });
  }
  entries.value = nextEntries;
  selectedIndex.value = nextEntries.length > 0 ? 0 : -1;
}

function scheduleSearch() {
  if (debounceTimer !== null) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    debounceTimer = null;
    void runSearch();
  }, SEARCH_DEBOUNCE_MS);
}

function handleQueryUpdate(value: string) {
  query.value = value;
  scheduleSearch();
}

function handleIncludeTrashUpdate(value: boolean) {
  includeTrash.value = value;
  void runSearch();
}

const modalRef = ref<InstanceType<typeof SearchModal> | null>(null);

// Resets the search on every open (matching the old `openSearchModal`) and
// focuses the input -- the one-shot effect `defineExpose` allows.
watch(isOpen, (open) => {
  if (!open) {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
      debounceTimer = null;
    }
    return;
  }
  query.value = "";
  includeTrash.value = false;
  entries.value = [];
  selectedIndex.value = -1;
  void nextTick(() => modalRef.value?.focus());
});

async function handleOpen(result: SearchResultEntry) {
  await props.openPageByTitleInVanilla(result.title);
}

async function handleCreate(searchQuery: string) {
  await props.createPageFromQueryInVanilla(searchQuery);
}
</script>

<template>
  <SearchModal
    v-if="isOpen"
    ref="modalRef"
    :query="query"
    :include-trash="includeTrash"
    :entries="entries"
    :selected-index="selectedIndex"
    @close="closeModal()"
    @update:query="handleQueryUpdate($event)"
    @update:include-trash="handleIncludeTrashUpdate($event)"
    @select-index="selectedIndex = $event"
    @open="handleOpen($event)"
    @create="handleCreate($event)"
  />
</template>
