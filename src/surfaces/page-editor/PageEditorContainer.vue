<script setup lang="ts">
// Container (spec.md#surface-contract): owns the autosave debounce, the
// immediate save-on-commit, and every backend call the editor needs; no
// story (containers aren't storied). Mounted once, for the app's whole
// lifetime -- `openPage`/`openPageKey`/`openPageMarkdown` drive whether
// (and what) the wrapper below renders, so navigating between pages never
// needs to remount or re-root this container itself.
import { ref } from "vue";
import { openPage, openPageKey, openPageMarkdown, save, materialize } from "../../state/pages";
import PageEditorSurface from "./PageEditorSurface.vue";

// Temporary callback root prop (spec.md#islands-and-how-they-merge):
// page-open logic (resolving a `[[Link]]` title, deciding already-open vs.
// navigate, Recent, etc.) hasn't moved out of `main.ts` yet -- that's
// ticket 07's "Article + backlinks" and beyond. Deleted once it has.
const props = defineProps<{
  /** Opens a `[[Link]]` chip's target the same way any other page-open path does. */
  openPageByTitleInVanilla: (title: string) => Promise<void>;
}>();

const AUTOSAVE_DEBOUNCE_MS = 1500;
let saveTimer: ReturnType<typeof setTimeout> | null = null;

// Once a dynamic page has been materialized under a given `pageKey`, later
// saves for that *same* key go through `save` with the id this returned,
// not `materialize` again. `pageKey` deliberately doesn't change just
// because the page turned persisted mid-edit (see `state/pages.ts`'s
// `open`) -- without this map, a second autosave tick for the same page
// would call `materialize` a second time on an already-persisted page.
const materializedIds = new Map<string, string>();

/**
 * `state/pages.ts`'s `reloadOpenPage` (the rename-forced-refresh case) can
 * suffix `openPageKey` with `#<n>` without changing the page's actual
 * identity -- strip that back off before decoding, so a forced reload
 * right before an autosave tick doesn't turn a persisted page's own id
 * into `"<id>#<n>"`.
 */
function routingKeyOf(pageKey: string): string {
  const hashIndex = pageKey.indexOf("#");
  return hashIndex === -1 ? pageKey : pageKey.slice(0, hashIndex);
}

async function saveFor(pageKey: string, markdown: string): Promise<void> {
  const routingKey = routingKeyOf(pageKey);
  const resolvedId = materializedIds.get(routingKey);
  if (resolvedId) {
    await save(resolvedId, markdown);
    return;
  }
  if (routingKey.startsWith("p:")) {
    await save(routingKey.slice(2), markdown);
  } else if (routingKey.startsWith("d:")) {
    const summary = await materialize(routingKey.slice(2), markdown);
    materializedIds.set(routingKey, summary.id);
  }
}

function cancelPendingSave() {
  if (saveTimer !== null) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
}

/** Ordinary edit: debounced 1500ms, same as before ticket 06. */
function handleChange(pageKey: string, markdown: string) {
  cancelPendingSave();
  saveTimer = setTimeout(() => {
    saveTimer = null;
    void saveFor(pageKey, markdown).catch((err) => console.error("Failed to save page", err));
  }, AUTOSAVE_DEBOUNCE_MS);
}

/**
 * The lost-edit fix (spec.md#editor-wrapper-and-commit): saved immediately,
 * with no debounce. Also cancels any pending debounced save so a moment
 * later it can't redundantly (or, worse, racily) save the same edit again.
 */
function handleCommit(oldPageKey: string, markdown: string) {
  cancelPendingSave();
  void saveFor(oldPageKey, markdown).catch((err) => console.error("Failed to save page", err));
}

function handleLinkClick(title: string) {
  void props.openPageByTitleInVanilla(title);
}

const surfaceRef = ref<InstanceType<typeof PageEditorSurface> | null>(null);

/** Forwarded to the mounted wrapper -- `null` while no page is open (see the template's `v-if`), in which case there's nothing to scroll. */
function scrollToHeading(slug: string): boolean {
  return surfaceRef.value?.scrollToHeading(slug) ?? false;
}
defineExpose({ scrollToHeading });
</script>

<template>
  <PageEditorSurface
    v-if="openPage"
    ref="surfaceRef"
    :page-key="openPageKey"
    :markdown="openPageMarkdown"
    @change="handleChange"
    @link-click="handleLinkClick"
    @commit="handleCommit"
  />
</template>
