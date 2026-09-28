<script setup lang="ts">
// Container (spec.md#surface-contract): owns state reads, backend calls,
// and dialog confirmations. No story (containers aren't storied).
// Replaces `main.ts`'s `renderPageList`/`handleNewPageClick`/
// `handleTodayClick`/`todaysDateTitle`. `selectPage`/`openResolution` live
// in `state/navigation.ts` since ticket 13, but this container still
// writes its own `openPersisted` independently (same as
// `ArticleContainer.vue`'s own navigation, ticket 07) since it never has a
// heading target to scroll to.
import { computed } from "vue";
import * as pagesState from "../../state/pages";
import { getPage, resolvePage } from "../../vault-api";
import { promptDialog, messageDialog } from "../../dialogs";
import PageListSurface from "./PageListSurface.vue";

const pages = computed(() => pagesState.pages.value.map((page) => ({ id: page.id, title: page.title })));

const activePageId = computed<string | null>(() => {
  const current = pagesState.openPage.value;
  return current !== null && current.kind === "persisted" ? current.id : null;
});

/** Opens a page from the list -- same "already open, skip reload" guard `main.ts`'s old `selectPage` used. A trashed page can't appear in this list, so `inTrash` is never true here in practice; the guard is kept anyway for parity. */
async function openPersisted(id: string) {
  const current = pagesState.openPage.value;
  if (current?.kind === "persisted" && current.id === id && !current.inTrash) return;
  const page = await getPage(id);
  pagesState.open({ kind: "persisted", id: page.id, title: page.title, body: page.body, html: page.html });
}

function handleSelectPage(id: string) {
  void openPersisted(id);
}

/**
 * Explicit "new page" action (issue 04): prompts for a title, persists a
 * real file immediately, and opens it straight into the editor. Exposed
 * below (see `defineExpose`) so the still-vanilla sidebar rail's own "+"
 * icon -- a separate button on a surface that hasn't migrated yet -- can
 * trigger the same flow, the same imperative-escape-hatch pattern
 * `main.ts` already uses for the article island's `scrollToHeading`.
 */
async function newPage() {
  const title = promptDialog("Title for the new page:");
  if (title === null) return; // user cancelled

  const trimmed = title.trim();
  if (!trimmed) {
    await messageDialog("Title cannot be empty.");
    return;
  }

  try {
    const summary = await pagesState.create(trimmed);
    await openPersisted(summary.id);
  } catch (err) {
    await messageDialog(String(err));
  }
}

/**
 * Today's date as an ISO-8601 `YYYY-MM-DD` string, per issue 11 / the
 * "Daily note" glossary entry -- built from the *local* calendar date, not
 * `toISOString()` (which reports the UTC date and would land on the wrong
 * day whenever local time is far enough from UTC).
 */
function todaysDateTitle(): string {
  const now = new Date();
  const year = String(now.getFullYear()).padStart(4, "0");
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/** Sidebar "Today" shortcut (issue 11): the exact same `resolve_page` flow a `[[YYYY-MM-DD]]` link chip would use -- no distinct "daily note" code path. A bare date title never carries a `#heading` suffix, so unlike `ArticleContainer.vue`'s link-click handler there is no heading target to scroll to here. */
async function handleToday() {
  const resolution = await resolvePage(todaysDateTitle());
  pagesState.open(resolution);
}

defineExpose({ newPage });
</script>

<template>
  <PageListSurface
    :pages="pages"
    :active-page-id="activePageId"
    @select-page="handleSelectPage"
    @new-page="newPage"
    @today="handleToday"
  />
</template>
