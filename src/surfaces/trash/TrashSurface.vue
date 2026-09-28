<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. This is the
// sidebar's Trash list plus "Empty trash" (spec.md#step-8-trash -- called
// "Step 9" in the map/ticket but "Step 8" in spec.md's own numbering).
// Reuses `PageList` (its **third** copy -- extracted in ticket 08),
// unchanged from `main.ts`'s old `renderTrashList`: click an entry to open
// it, same as a `[[Link]]` would. There is no active-item highlight here
// (a trashed page, once opened, shows its own "in trash" banner in the
// article view instead -- ticket 07's `ArticleSurface.vue` -- so unlike
// "All pages"/Recent, Trash never needs to mark one of its own rows as
// active).
import { computed } from "vue";
import PageList, { type PageListRow } from "../../components/PageList.vue";

export interface TrashedPage {
  id: string;
  title: string;
}

const props = defineProps<{
  pages: TrashedPage[];
}>();

const emit = defineEmits<{
  /** Opens a trashed page, by title -- same as the container's old
   * `openPageByTitle`/`resolvePage` navigation (see `TrashContainer.vue`),
   * so a same-titled persisted page is never opened by mistake. */
  select: [title: string];
  emptyTrash: [];
}>();

const rows = computed<PageListRow[]>(() =>
  props.pages.map((page) => ({
    // `key` is the row's own unique identity (the trashed page's id, which
    // never collides), while `value` -- what `select` emits -- stays the
    // title, matching `resolvePage`'s by-title lookup.
    kind: "item",
    key: page.id,
    value: page.title,
    label: page.title,
  })),
);
</script>

<template>
  <div class="sidebar-section">
    <div class="sidebar-header sidebar-trash-header">
      <h2>Trash</h2>
      <button type="button" class="empty-trash-button" @click="emit('emptyTrash')">Empty trash</button>
    </div>
    <PageList :rows="rows" @select="emit('select', $event)" />
  </div>
</template>
