// Ticket 13 (app shell): the navigation-only logic that used to live in
// `main.ts` as free functions (`openResolution`/`selectPage`/
// `openPageByTitle`) plus the `scrollToHeading` imperative escape hatch
// (spec.md#imperative-escape-hatches) now live here, the same
// plain-ref-plus-actions shape as `state/sync.ts`/`state/vault.ts`/
// `state/pages.ts`. This is what lets `SearchModalContainer.vue` -- the one
// remaining caller with a heading target to reach -- import `openPageByTitle`
// directly instead of receiving it as a temporary callback root prop
// (`openPageByTitleInVanilla`/`createPageFromQueryInVanilla`, both retired by
// this ticket): now that the whole app is one `createApp`, there's no more
// "vanilla code" for a callback to reach back into.
import { shallowRef } from "vue";
import { getPage, resolvePage, type PageResolution } from "../vault-api";
import * as pagesState from "./pages";

interface ArticleHandle {
  scrollToHeading(slug: string): boolean;
}

const articleHandleRef = shallowRef<ArticleHandle | null>(null);

/**
 * Registered by `App.vue` with a template ref onto the mounted
 * `ArticleContainer` (and cleared with `null` if it's ever unmounted) --
 * the one remaining imperative escape hatch, used only by navigations that
 * originate outside the article surface itself (the sidebar's page/Recent/
 * Trash lists, search, "Today"). A `[[Link]]` click inside the editor
 * scrolls on its own, entirely within `ArticleContainer.vue`.
 */
export function registerArticleHandle(handle: ArticleHandle | null): void {
  articleHandleRef.value = handle;
}

/**
 * Calls the registered article handle's `scrollToHeading`, retrying
 * briefly: a fresh navigation reloads the editor's content asynchronously
 * (the wrapper's own `pageKey` watcher awaits `PageEditor.load()`), so the
 * heading's DOM node may not exist yet the instant this is called. Bounded
 * and self-cancelling -- once `scrollToHeading` returns `true`, or the
 * budget runs out, it stops. A no-op if nothing is registered yet. Moved
 * from `main.ts` verbatim.
 */
function scrollToHeadingWhenReady(slug: string): void {
  const attempts = 20;
  const intervalMs = 25;
  let tries = 0;
  const tick = () => {
    if (articleHandleRef.value?.scrollToHeading(slug)) return;
    tries += 1;
    if (tries < attempts) setTimeout(tick, intervalMs);
  };
  tick();
}

/**
 * Opens whatever `resolution` points to: an existing persisted page, or a
 * dynamic (unmaterialized) one. The Recent-recording and open-page state
 * update themselves live in `state/pages.ts`'s `open` action.
 */
export async function openResolution(resolution: PageResolution): Promise<void> {
  pagesState.open(resolution);
  if (resolution.headingSlug) scrollToHeadingWhenReady(resolution.headingSlug);
}

/** Opens a page by id, unless it's already the open page. */
export async function selectPage(id: string): Promise<void> {
  const current = pagesState.openPage.value;
  if (current?.kind === "persisted" && current.id === id && !current.inTrash) return;
  const page = await getPage(id);
  await openResolution({ kind: "persisted", id: page.id, title: page.title, body: page.body, html: page.html });
}

/** Navigates to whatever a clicked `[[Link]]` chip or a search result's title targets -- `resolve_page` already handles the "in trash" state. */
export async function openPageByTitle(rawTitle: string): Promise<void> {
  const resolution = await resolvePage(rawTitle);
  await openResolution(resolution);
}
