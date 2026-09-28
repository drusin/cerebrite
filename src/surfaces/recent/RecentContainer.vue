<script setup lang="ts">
// Container (spec.md#surface-contract): owns state reads and backend
// calls. No story (containers aren't storied). Replaces `main.ts`'s
// `renderRecentList` -- `selectPage`/`openPageByTitle` stay in `main.ts` a
// while longer (Trash and the search modal's still-vanilla callback both
// still need them), so this container's own navigation is written
// independently, same as `PageListContainer.vue`.
import { computed } from "vue";
import * as pagesState from "../../state/pages";
import { getPage, resolvePage } from "../../vault-api";
import RecentSurface from "./RecentSurface.vue";

const entries = computed(() => [...pagesState.recent.value]);

/** Same `p:<id>`/`d:<title>` scheme `state/pages.ts` uses for a Recent entry's own `key`. */
const activeKey = computed<string | null>(() => {
  const current = pagesState.openPage.value;
  if (current === null) return null;
  return current.kind === "persisted" ? `p:${current.id}` : `d:${current.title}`;
});

async function handleSelectPersisted(id: string) {
  const current = pagesState.openPage.value;
  if (current?.kind === "persisted" && current.id === id && !current.inTrash) return;
  const page = await getPage(id);
  pagesState.open({ kind: "persisted", id: page.id, title: page.title, body: page.body, html: page.html });
}

/** A dynamic Recent entry's title is already a normalized page title (never raw `[[Link]]` text), so unlike `ArticleContainer.vue`'s link-click handler there is never a `#heading` suffix to scroll to here. */
async function handleSelectDynamic(title: string) {
  const resolution = await resolvePage(title);
  pagesState.open(resolution);
}
</script>

<template>
  <RecentSurface
    :entries="entries"
    :active-key="activeKey"
    @select-persisted="handleSelectPersisted"
    @select-dynamic="handleSelectDynamic"
  />
</template>
