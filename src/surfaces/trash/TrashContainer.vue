<script setup lang="ts">
// Container (spec.md#surface-contract): owns state reads, the backend
// call, and the dialog confirmation. No story (containers aren't storied).
// Replaces `main.ts`'s `renderTrashList`/`handleEmptyTrashClick`.
// `openPageByTitle`/`openResolution` live in `state/navigation.ts` since
// ticket 13, but this container still writes its own navigation
// independently (same as `PageListContainer.vue`/`RecentContainer.vue`,
// ticket 08) since it never has a heading target to scroll to.
import { computed } from "vue";
import * as pagesState from "../../state/pages";
import { resolvePage } from "../../vault-api";
import { confirmDialog, messageDialog } from "../../dialogs";
import TrashSurface from "./TrashSurface.vue";

const pages = computed(() => pagesState.trash.value.map((page) => ({ id: page.id, title: page.title })));

/** Opens a trashed page the same way a `[[Link]]` to it would -- `resolve_page` already handles the "in trash" state, rendering the article view's own banner and Restore action. */
async function handleSelect(title: string) {
  const resolution = await resolvePage(title);
  pagesState.open(resolution);
}

/** Explicit "empty trash" action (issue 10): permanently deletes every trashed page. There is no other purge path. Confirmed here (the container), through `dialogs.ts`, before calling `state/pages.ts`'s action -- `pagesState.emptyTrash()` itself clears the open page when it was a trashed one, and the article island reacts to that on its own (ticket 07). */
async function handleEmptyTrash() {
  if (!(await confirmDialog("Permanently delete all trashed pages? This cannot be undone."))) return;

  try {
    await pagesState.emptyTrash();
  } catch (err) {
    await messageDialog(String(err));
  }
}
</script>

<template>
  <TrashSurface :pages="pages" @select="handleSelect" @empty-trash="handleEmptyTrash" />
</template>
