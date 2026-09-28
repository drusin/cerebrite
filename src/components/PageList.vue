<script setup lang="ts">
// Shared component (spec.md#shared-components): "a shared component is
// extracted when a second copy migrates" -- backlinks (ticket 07) wrote
// this inline as the first copy; the sidebar's "All pages"/Recent (ticket
// 08) is the second, so it's extracted here and backlinks switches to it
// too (see `ArticleSurface.vue`). Trash (ticket 09) reuses it as well.
// Presentational: never imports `vault-api`/`dialogs.ts`/a `src/state/`
// module, and mounts from props alone.
export interface PageListRow {
  /** A clickable header row grouping the following items under one source
   * page (backlinks only -- Recent/"All pages"/Trash have no headers). */
  kind: "item" | "header";
  /** Vue `:key` -- must be unique across every row, unlike `value` below
   * (a backlinks header and its own snippets all share one `value`, the
   * source page's id). */
  key: string;
  /** Emitted via `select` when this row is clicked. */
  value: string;
  label: string;
  /** Highlights this row as the currently open page. Only meaningful for
   * `kind: "item"` -- a header is never "active". */
  active?: boolean;
  /** e.g. "→ Heading" (backlinks only). */
  annotation?: string | null;
}

const props = withDefaults(
  defineProps<{
    rows: PageListRow[];
    /** Shown instead of the list when `rows` is empty. Left unset (the
     * default), an empty list renders nothing -- "All pages" and Trash
     * have no empty-state copy today. */
    emptyText?: string | null;
    /** Which existing class names to render with: "flat" (the default) is
     * the plain `.page-list` hover/active styling (Recent, "All pages",
     * Trash); "grouped" is the `.backlinks-list` header/snippet styling. */
    variant?: "flat" | "grouped";
  }>(),
  { emptyText: null, variant: "flat" },
);

const emit = defineEmits<{ select: [value: string] }>();
</script>

<template>
  <p v-if="rows.length === 0 && emptyText" :class="props.variant === 'grouped' ? 'backlinks-empty' : 'sidebar-empty-hint'">
    {{ emptyText }}
  </p>
  <ul v-else-if="rows.length > 0" :class="props.variant === 'grouped' ? 'backlinks-list' : 'page-list'">
    <li
      v-for="row in rows"
      :key="row.key"
      :class="props.variant === 'grouped' ? (row.kind === 'header' ? 'backlink-group-header' : 'backlink-snippet') : undefined"
    >
      <button type="button" :class="{ active: row.kind === 'item' && row.active }" @click="emit('select', row.value)">
        {{ row.label }}
      </button>
      <span v-if="row.kind === 'item' && row.annotation" class="backlink-heading-label">{{ row.annotation }}</span>
    </li>
  </ul>
</template>
