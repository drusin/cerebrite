<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. This is the
// sidebar's "All pages" list (New page, the list itself) and "Today"
// shortcut (spec.md#step-7-sidebar-page-list--recent). The active item
// comes from the `activePageId` prop (state/pages.ts's `openPage`, read by
// the container) -- `highlightActivePage`'s DOM classing is gone.
import { computed } from "vue";
import PageList, { type PageListRow } from "../../components/PageList.vue";

export interface PageListPage {
  id: string;
  title: string;
}

const props = defineProps<{
  pages: PageListPage[];
  /** The currently open page's id, or `null` if nothing/a dynamic page is open. */
  activePageId: string | null;
}>();

const emit = defineEmits<{
  selectPage: [id: string];
  newPage: [];
  today: [];
}>();

const rows = computed<PageListRow[]>(() =>
  props.pages.map((page) => ({
    kind: "item",
    key: page.id,
    value: page.id,
    label: page.title,
    active: page.id === props.activePageId,
  })),
);
</script>

<template>
  <div class="sidebar-section">
    <div class="sidebar-header">
      <h2>All pages</h2>
      <button type="button" class="new-page-button" @click="emit('newPage')">+ New page</button>
    </div>
    <PageList :rows="rows" @select="emit('selectPage', $event)" />
  </div>

  <div class="sidebar-section">
    <button type="button" class="today-button" @click="emit('today')">Today</button>
  </div>
</template>
