<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. This is the
// sidebar's Recent list (spec.md#step-7-sidebar-page-list--recent). The
// active item comes from the `activeKey` prop (state/pages.ts's
// `openPage`, read by the container, compared against each entry's own
// `p:<id>`/`d:<title>` key) -- `highlightActivePage`'s DOM classing is gone.
import { computed } from "vue";
import PageList, { type PageListRow } from "../../components/PageList.vue";
import type { RecentEntry } from "./recent-entry";

const props = defineProps<{
  entries: RecentEntry[];
  /** The currently open page's Recent-entry key (`p:<id>`/`d:<title>`), or `null` if nothing is open. */
  activeKey: string | null;
}>();

const emit = defineEmits<{
  selectPersisted: [id: string];
  selectDynamic: [title: string];
}>();

const rows = computed<PageListRow[]>(() =>
  props.entries.map((entry) => ({
    kind: "item",
    key: entry.key,
    value: entry.key,
    label: entry.title,
    active: entry.key === props.activeKey,
  })),
);

function handleSelect(key: string) {
  const entry = props.entries.find((candidate) => candidate.key === key);
  if (!entry) return;
  if (entry.kind === "persisted" && entry.pageId) emit("selectPersisted", entry.pageId);
  else emit("selectDynamic", entry.title);
}
</script>

<template>
  <div class="sidebar-section">
    <div class="sidebar-header">
      <h2>Recent</h2>
    </div>
    <PageList :rows="rows" empty-text="No pages opened yet." @select="handleSelect" />
  </div>
</template>
