<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. This is the
// article view (spec.md#step-6-article--backlinks): the title row with
// rename/delete, the trash banner with Restore, the editor (via the
// `editor` slot -- the container fills it with
// `surfaces/page-editor/PageEditorContainer.vue`, which this file never
// imports, so this surface stays mountable from props alone), and
// backlinks -- the first copy of a "page list"-shaped UI, written inline
// per spec.md#shared-components (extracted into `PageList` on its second
// copy, ticket 08).
import { computed } from "vue";
import { humanizeHeadingSlug } from "../../heading-slug";
import type { BacklinkEntry } from "./backlink-entry";

const props = defineProps<{
  /** `null` = nothing selected ("Select a page from the list."). */
  title: string | null;
  /** Offers "Rename page"/"Delete page" -- only true for a persisted,
   * non-trashed page (a dynamic page has nothing to rename/delete yet, and
   * a trashed page shows the banner + Restore below instead). */
  deletable: boolean;
  /** Shows the trash banner + Restore instead of rename/delete. */
  inTrash: boolean;
  /** Pre-grouped by source page, most-recently-modified source first (same
   * order `get_backlinks` returns) -- this surface only groups consecutive
   * same-source entries under one header, it never re-sorts. */
  backlinks: BacklinkEntry[];
}>();

const emit = defineEmits<{
  rename: [];
  delete: [];
  restore: [];
  openBacklinkSource: [id: string];
}>();

type BacklinkRow =
  | { kind: "header"; sourceId: string; sourceTitle: string }
  | { kind: "snippet"; entry: BacklinkEntry };

const backlinkRows = computed<BacklinkRow[]>(() => {
  const rows: BacklinkRow[] = [];
  let lastSourceId: string | null = null;
  for (const entry of props.backlinks) {
    if (entry.sourceId !== lastSourceId) {
      rows.push({ kind: "header", sourceId: entry.sourceId, sourceTitle: entry.sourceTitle });
      lastSourceId = entry.sourceId;
    }
    rows.push({ kind: "snippet", entry });
  }
  return rows;
});
</script>

<template>
  <main class="page-view">
    <p v-if="title === null" class="page-view-empty">Select a page from the list.</p>
    <article v-else class="page-article">
      <div class="page-title-row">
        <h1>{{ title }}</h1>
        <div class="page-title-actions">
          <button v-if="deletable" type="button" class="rename-page-button" @click="emit('rename')">
            Rename page
          </button>
          <button
            v-if="deletable"
            type="button"
            class="delete-page-button"
            aria-label="Delete page"
            title="Delete page"
            @click="emit('delete')"
          >
            <svg viewBox="0 0 24 24" width="1em" height="1em" aria-hidden="true" focusable="false">
              <path
                d="M9 3h6l1 2h4v2H4V5h4l1-2Zm-3 6h12l-1 12a2 2 0 0 1-2 2H9a2 2 0 0 1-2-2L6 9Zm3 2v8h1v-8H9Zm3 0v8h1v-8h-1Zm3 0v8h1v-8h-1Z"
                fill="currentColor"
              />
            </svg>
          </button>
        </div>
      </div>

      <p v-if="inTrash" class="page-trash-banner">
        This page is in trash.
        <button type="button" @click="emit('restore')">Restore</button>
      </p>

      <slot name="editor" />

      <section class="backlinks-section">
        <h2>Backlinks</h2>
        <p v-if="backlinkRows.length === 0" class="backlinks-empty">No backlinks yet.</p>
        <ul v-else class="backlinks-list">
          <li
            v-for="(row, i) in backlinkRows"
            :key="row.kind === 'header' ? `h-${row.sourceId}-${i}` : `s-${row.entry.sourceId}-${i}`"
            :class="row.kind === 'header' ? 'backlink-group-header' : 'backlink-snippet'"
          >
            <button v-if="row.kind === 'header'" type="button" @click="emit('openBacklinkSource', row.sourceId)">
              {{ row.sourceTitle }}
            </button>
            <template v-else>
              <button type="button" @click="emit('openBacklinkSource', row.entry.sourceId)">
                {{ row.entry.snippet }}
              </button>
              <span v-if="row.entry.targetHeadingSlug" class="backlink-heading-label">
                → {{ humanizeHeadingSlug(row.entry.targetHeadingSlug) }}
              </span>
            </template>
          </li>
        </ul>
      </section>
    </article>
  </main>
</template>
