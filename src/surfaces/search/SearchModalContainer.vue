<script setup lang="ts">
// Container (spec.md#surface-contract): owns the search variables --
// they're local to this container, not `src/state/`, per
// spec.md#step-2-search-modal -- the debounce, and every backend call. No
// story (containers aren't storied).
import { computed, nextTick, ref, watch } from "vue";
import { modal, closeModal } from "../../state/ui";
import { searchPages, type SearchResult } from "../../vault-api";
import * as pagesState from "../../state/pages";
import { openPageByTitle, selectPage } from "../../state/navigation";
import { messageDialog } from "../../dialogs";
import SearchModal from "./SearchModal.vue";
import type { SearchEntry, SearchResultEntry } from "./search-entry";

// Ticket 13 (app shell): this container used to take two temporary callback
// root props (`openPageByTitleInVanilla`/`createPageFromQueryInVanilla`),
// since page logic didn't move into `src/state/` until step 4 and the app
// was still several separate islands until now. Both intents are plain
// imports from `state/navigation.ts`/`state/pages.ts` today -- there's no
// more vanilla code for a callback to reach back into.

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

/** Opens the resolved search result the same way clicking any `[[Link]]` chip or a Trash-list entry would -- `resolve_page` already handles the "in trash" state, so this works identically for a persisted or a trashed hit. */
async function handleOpen(result: SearchResultEntry) {
  closeModal();
  await openPageByTitle(result.title);
}

/** Empty-state action: creates the typed query as a brand-new page and opens it straight into the editor, reusing the exact same action as the "New page" button. On failure, the modal stays open, matching the old `createPageFromQueryInVanilla`'s behavior. */
async function handleCreate(searchQuery: string) {
  try {
    const summary = await pagesState.create(searchQuery);
    closeModal();
    await selectPage(summary.id);
  } catch (err) {
    await messageDialog(String(err));
  }
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
