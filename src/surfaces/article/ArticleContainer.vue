<script setup lang="ts">
// Container (spec.md#surface-contract): owns every state read, backend
// call, and dialog confirmation the article view needs; no story
// (containers aren't storied). Replaces `main.ts`'s
// `renderPageArticle`/`renderBacklinks`/`handleRenamePageClick`/
// `handleDeletePageClick`/`handleRestoreClick` -- the navigation-only parts
// of the old flow (`openResolution`/`selectPage`/`openPageByTitle`, still
// needed by the search modal's still-vanilla callback) stay in `main.ts`;
// only the "render the open page into the article view" and
// "confirm + mutate" halves move here.
import { computed, ref, watch } from "vue";
import * as pagesState from "../../state/pages";
import { resolvePage, getPage, getBacklinks } from "../../vault-api";
import { confirmDialog, messageDialog, promptDialog } from "../../dialogs";
import ArticleSurface from "./ArticleSurface.vue";
import PageEditorContainer from "../page-editor/PageEditorContainer.vue";
import type { BacklinkEntry } from "./backlink-entry";

const open = pagesState.openPage;

/**
 * The open page's title. `state/pages.ts`'s `OpenPage` only carries an id
 * for a persisted page (see its own doc comment for why), so this looks it
 * up by id in whichever list it currently belongs to -- reactive, so a
 * rename (which re-fetches the page list) updates this on its own, without
 * the dedicated re-render call the old vanilla `handleRenamePageClick`
 * needed.
 */
const title = computed<string | null>(() => {
  const current = open.value;
  if (current === null) return null;
  if (current.kind === "dynamic") return current.title;
  const list = current.inTrash ? pagesState.trash.value : pagesState.pages.value;
  return list.find((page) => page.id === current.id)?.title ?? null;
});

const currentPageId = computed<string | null>(() => {
  const current = open.value;
  return current !== null && current.kind === "persisted" ? current.id : null;
});

const inTrash = computed<boolean>(() => {
  const current = open.value;
  return current !== null && current.kind === "persisted" && current.inTrash;
});

const trashedFilename = computed<string | null>(() => {
  const current = open.value;
  return current !== null && current.kind === "persisted" ? current.trashedFilename : null;
});

/** Rename/delete are offered only for a persisted, non-trashed page -- same rule the old `renderPageArticle` used. */
const deletable = computed<boolean>(() => {
  const current = open.value;
  return current !== null && current.kind === "persisted" && !current.inTrash;
});

// --- Backlinks ---------------------------------------------------------
//
// Always present for both persisted and dynamic pages (issue 06) --
// re-fetched whenever `title` changes, which covers a genuine navigation
// and a rename of the open page alike (the rename handler below doesn't
// need its own explicit `renderBacklinks` call the way the vanilla version
// did).
const backlinks = ref<BacklinkEntry[]>([]);
watch(
  title,
  async (newTitle) => {
    if (newTitle === null) {
      backlinks.value = [];
      return;
    }
    try {
      backlinks.value = await getBacklinks(newTitle);
    } catch (err) {
      console.error("Failed to load backlinks", err);
      backlinks.value = [];
    }
  },
  { immediate: true },
);

// --- Heading-targeted scroll / the embedded editor ----------------------

const editorRef = ref<InstanceType<typeof PageEditorContainer> | null>(null);

/** Forwarded to the embedded editor -- `null` while no page is open (see the template's `v-if`), in which case there's nothing to scroll. */
function scrollToHeading(slug: string): boolean {
  return editorRef.value?.scrollToHeading(slug) ?? false;
}
/** Imperative escape hatch (spec.md#imperative-escape-hatches): the first
 * real consumer of this expose (ticket 06 only wired the plumbing) --
 * `main.ts` calls it for navigations that originate outside this surface
 * (the sidebar's page/Recent/Trash lists, search, "Today"). */
defineExpose({ scrollToHeading });

/**
 * Bounded, self-cancelling retry, same shape as `main.ts`'s own
 * `scrollToHeadingWhenReady` (which targets *this* container's exposed
 * `scrollToHeading` for navigations from outside this surface) -- a fresh
 * navigation reloads the editor's content asynchronously, so the heading's
 * DOM node may not exist the instant this runs.
 */
function scrollToHeadingWhenReady(slug: string) {
  const attempts = 20;
  const intervalMs = 25;
  let tries = 0;
  const tick = () => {
    if (scrollToHeading(slug)) return;
    tries += 1;
    if (tries < attempts) setTimeout(tick, intervalMs);
  };
  tick();
}

/**
 * Opens a `[[Link]]` chip's target clicked inside the embedded editor.
 * Replaces `openPageByTitleInVanilla`, the temporary callback root prop
 * `PageEditorContainer` used to need for this (see its own updated doc
 * comment) -- now that this container reads `state/pages.ts` and calls
 * `vault-api` directly, there's no more vanilla logic to reach back into
 * for it. Click-through-to-heading (ticket 07): a link to a heading that
 * doesn't (yet) exist on the target page is simply a no-op scroll -- the
 * page itself still opens normally.
 */
async function handleLinkClick(rawTitle: string) {
  const resolution = await resolvePage(rawTitle);
  pagesState.open(resolution);
  if (resolution.headingSlug) scrollToHeadingWhenReady(resolution.headingSlug);
}

/** Opens a backlink entry's source page -- the same navigation a page-list/Recent click uses, just with no heading target (a backlink doesn't carry one for its own source page). */
async function handleOpenBacklinkSource(id: string) {
  const page = await getPage(id);
  pagesState.open({ kind: "persisted", id: page.id, title: page.title, body: page.body, html: page.html });
}

// --- Rename / delete / restore ------------------------------------------

/**
 * Explicit "rename page" action: prompts for a new title pre-filled with
 * the current one, warns first if other pages have inbound links that
 * will be rewritten (a rename's blast radius isn't limited to this one
 * file, unlike every other mutation in this app), then renames.
 *
 * If the currently-open page's content was touched by the rewrite (either
 * because it's the page being renamed, or because it was one of the
 * `affectedPageIds`), its body is reloaded from the backend and pushed
 * back into the editor -- otherwise a pending autosave on that page would
 * overwrite the just-rewritten file with its stale in-memory content. The
 * title itself needs no equivalent push: `title` above already re-derives
 * from `state/pages.ts`'s `pages` list, which `rename()` re-fetches.
 */
async function handleRename() {
  const id = currentPageId.value;
  const currentTitle = title.value;
  if (id === null || currentTitle === null) return;

  const newTitle = promptDialog("New title for this page:", currentTitle);
  if (newTitle === null) return; // user cancelled

  const trimmed = newTitle.trim();
  if (!trimmed) {
    await messageDialog("Title cannot be empty.");
    return;
  }
  if (trimmed === currentTitle) return;

  let affectedCount = 0;
  try {
    affectedCount = (await getBacklinks(currentTitle)).length;
  } catch (err) {
    console.error("Failed to look up backlinks before renaming", err);
  }
  if (affectedCount > 0) {
    const linkWord = affectedCount === 1 ? "link" : "links";
    if (!(await confirmDialog(`This will also update ${affectedCount} ${linkWord} in other pages. Continue?`))) {
      return;
    }
  }

  try {
    const result = await pagesState.rename(id, trimmed);

    const current = pagesState.openPage.value;
    const currentPageNeedsReload =
      current !== null &&
      current.kind === "persisted" &&
      (current.id === id || result.affectedPageIds.includes(current.id));
    if (currentPageNeedsReload) {
      const page = await getPage(id);
      // The renamed page's own id (and so `openPageKey`) doesn't change --
      // the editor wrapper only reloads on a `pageKey` change, so a
      // same-id content refresh needs this forced-reload action instead
      // (ticket 06; see its own doc comment in state/pages.ts).
      pagesState.reloadOpenPage(page.body);
    }
  } catch (err) {
    await messageDialog(String(err));
  }
}

/** Explicit "delete page" action (issue 10 / ADR-0010): moves the page's file into `.cerebrite/trash/`. `pagesState.deletePage` itself clears the open page, so the surface's own `v-if` falls back to "nothing selected" on its own. */
async function handleDelete() {
  const id = currentPageId.value;
  if (id === null) return;
  if (!(await confirmDialog("Move this page to trash?"))) return;

  try {
    await pagesState.deletePage(id);
  } catch (err) {
    await messageDialog(String(err));
  }
}

/** Explicit "restore" action (issue 10): moves a trashed page's file back to its original path and reopens it as an ordinary persisted page. */
async function handleRestore() {
  const filename = trashedFilename.value;
  if (filename === null) return;

  try {
    const summary = await pagesState.restore(filename);
    const page = await getPage(summary.id);
    pagesState.open({ kind: "persisted", id: page.id, title: page.title, body: page.body, html: page.html });
  } catch (err) {
    await messageDialog(String(err));
  }
}
</script>

<template>
  <ArticleSurface
    :title="title"
    :deletable="deletable"
    :in-trash="inTrash"
    :backlinks="backlinks"
    @rename="handleRename"
    @delete="handleDelete"
    @restore="handleRestore"
    @open-backlink-source="handleOpenBacklinkSource"
  >
    <template #editor>
      <PageEditorContainer ref="editorRef" @link-click="handleLinkClick" />
    </template>
  </ArticleSurface>
</template>
