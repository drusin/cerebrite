// Ticket 05: page list, trash list, Recent, and the open page move into one
// state module -- see spec.md#step-4-page-state-only. Same shape as
// `state/sync.ts`/`state/vault.ts` (plain `ref()` plus actions, no Pinia),
// but with no DOM access of its own: `main.ts`'s rendering (page list,
// trash list, Recent, the article/editor) stays exactly where it is and
// simply reads this module's exports and calls its actions instead of
// owning the state itself. That split is why every action below only ever
// re-fetches the one list it actually changed, rather than reloading
// everything the way `main.ts`'s old `loadPages()`/`loadTrash()` call sites
// did after nearly every mutation.
import { readonly, ref, type Ref } from "vue";
import {
  listPages,
  savePage,
  createPage,
  renamePage,
  materializeAndSavePage,
  trashPage as trashPageCommand,
  restorePage,
  emptyTrash as emptyTrashCommand,
  listTrashedPages,
  type PageSummary,
  type PageResolution,
  type RenamePageResult,
  type TrashedPageSummary,
} from "../vault-api";

// --- Open page -------------------------------------------------------------
//
// Either a persisted page (has an id/file) or a dynamic page (issue 05 /
// ADR-0009) -- title-only, no backing file until the first write
// materializes it. A persisted page may currently sit in trash (issue 10 /
// ADR-0010) -- still resolves/renders, but flagged so the UI can show an
// "in trash" indicator and restore prompt instead of the ordinary "Delete
// page" action. Moved from `main.ts`'s own `OpenPage` type/`currentPage`
// variable verbatim -- same shape, same meaning.
export type OpenPage =
  | { kind: "persisted"; id: string; inTrash: boolean; trashedFilename: string | null }
  | { kind: "dynamic"; title: string };

const openPageState: Ref<OpenPage | null> = ref(null);

/** Read-only outside this module. `null` means no page is open. */
export const openPage = readonly(openPageState);

// --- Open page's editor content (ticket 06) ---------------------------------
//
// The markdown body the editor wrapper (`surfaces/page-editor/`) should be
// showing, plus a stable identity key for it -- fed straight into the
// wrapper's `pageKey`/`markdown` props by its container, reactively, so
// `main.ts` doesn't need to push them down as one-shot root props (which
// wouldn't update after the initial mount -- see spec.md#islands-and-how-
// they-merge: root props are for static config/callbacks, not changing
// data).
//
// `openPageKey` is deliberately its own piece of state, not derived from
// `openPage.value` on the fly: `openPage.value.kind` flips from "dynamic"
// to "persisted" the moment a dynamic page's first autosave materializes
// it (see `materialize` below), but that must NOT look like a page change
// to the editor wrapper -- reloading at that moment would reset the
// cursor mid-typing, the exact bug this ticket exists to avoid
// reintroducing. `open()` below only bumps this key when the page's
// identity actually changes; `materialize()` deliberately never touches
// it.
const openPageKeyState: Ref<string> = ref("");

/** Read-only outside this module. `""` when no page is open. Stable across a dynamic page materializing mid-edit; see the comment above. */
export const openPageKey = readonly(openPageKeyState);

const openPageMarkdownState: Ref<string> = ref("");

/** Read-only outside this module. The editor's current page body -- updated on a real navigation (`open`) or an explicit forced refresh (`reloadOpenPage`), never on every keystroke (the editor wrapper owns its own live content; this is only the value it (re)loads from). */
export const openPageMarkdown = readonly(openPageMarkdownState);

/** `p:<id>` / `d:<title>` -- same scheme as {@link RecentEntry.key}, reused here as the editor's stable per-page identity. */
function keyForResolution(resolution: PageResolution): string {
  return resolution.kind === "persisted" ? `p:${resolution.id}` : `d:${resolution.normalizedTitle}`;
}

let reloadCounter = 0;

/**
 * Forces the editor wrapper to reload the currently open page's content
 * even though its identity (`openPageKey`) hasn't changed -- e.g. a rename
 * rewrote the *currently open* page's own `[[Link]]` text without
 * navigating away from it (`main.ts`'s `handleRenamePageClick`, still
 * living there until ticket 07). The wrapper only reloads on a `pageKey`
 * change (by design -- see the comment above), so a genuine content
 * refresh needs its own key bump. Harmless side effect: the *next* time
 * this same page is reopened via `open()` below, its key will have this
 * suffix and so will no longer match, causing one extra (but correct,
 * self-healing) reload -- acceptable since that's a rare, cold path.
 */
export function reloadOpenPage(markdown: string): void {
  if (!openPageState.value) return;
  openPageMarkdownState.value = markdown;
  openPageKeyState.value = `${openPageKeyState.value}#${++reloadCounter}`;
}

// --- Page list / trash list -------------------------------------------------

const pagesState: Ref<PageSummary[]> = ref([]);
const trashState: Ref<TrashedPageSummary[]> = ref([]);

/** Read-only outside this module. "All pages", alphabetical (server-sorted). */
export const pages = readonly(pagesState);

/** Read-only outside this module. Every page currently in `.cerebrite/trash/`. */
export const trash = readonly(trashState);

/**
 * Re-fetches the page list from `list_pages`. Exported (not just used
 * internally by the actions below) because opening a vault needs an initial
 * load too -- `surfaces/vault-picker/VaultPickerContainer.vue` calls this
 * directly on every vault-open path, replacing the old
 * `loadPagesAndTrashInVanilla` temporary callback (ticket 04).
 */
export async function refreshPages(): Promise<void> {
  pagesState.value = await listPages();
}

/** Re-fetches the trash list from `list_trashed_pages`. See {@link refreshPages}. */
export async function refreshTrash(): Promise<void> {
  trashState.value = await listTrashedPages();
}

// --- Recent (issue 12) ------------------------------------------------------
//
// Last-*opened* pages (not last-edited), most-recent-first, capped at
// RECENT_LIMIT, no pagination -- per issue 08's sidebar spec. Deliberately
// in-memory only (module-level ref, not persisted to disk/config): there is
// no existing persistence mechanism for this kind of transient UI state
// (the config file only stores the vault path + device id), and the ticket
// doesn't require surviving a restart, so the simplest option -- resetting
// each app launch -- is the pragmatic default here.
const RECENT_LIMIT = 10;

export interface RecentEntry {
  /** Dedupe/identity key: `p:<id>` for a persisted page, `d:<normalizedTitle>` for a dynamic one. */
  key: string;
  title: string;
  kind: "persisted" | "dynamic";
  /** Set only for `kind === "persisted"`; used to navigate via `selectPage`. */
  pageId?: string;
}

const recentState: Ref<RecentEntry[]> = ref([]);

/** Read-only outside this module. */
export const recent = readonly(recentState);

/** Records a page-open event: moves an existing entry to the top (no duplicate) or inserts a new one, capped at RECENT_LIMIT. */
function recordRecentOpen(entry: RecentEntry): void {
  recentState.value = [entry, ...recentState.value.filter((existing) => existing.key !== entry.key)].slice(
    0,
    RECENT_LIMIT,
  );
}

/** Drops any Recent entries pointing at pages that are no longer valid (trashed or purged). */
function pruneRecentEntries(removedPageIds: Iterable<string>): void {
  const removed = new Set(removedPageIds);
  if (removed.size === 0) return;
  recentState.value = recentState.value.filter(
    (entry) => !(entry.kind === "persisted" && entry.pageId && removed.has(entry.pageId)),
  );
}

// --- Actions -----------------------------------------------------------------

/**
 * "open" action: records `resolution` in Recent (unless it's a trashed
 * page -- see below) and makes it the open page. Doesn't touch the page/
 * trash lists (a `resolve_page`/`get_page` result never changes either),
 * and doesn't render anything -- callers (`main.ts`'s `openResolution`)
 * still own deciding whether the page is already open (skip-vs-reload) and
 * pushing the resolved title into the article view (ticket 07). The
 * editor's own content (`openPageKey`/`openPageMarkdown`, ticket 06) *is*
 * updated here, but only when `resolution`'s identity actually differs
 * from what's already open -- re-opening the same page (e.g. a different
 * heading target) must not look like a page change to the editor wrapper.
 */
export function open(resolution: PageResolution): void {
  // Recent tracks last-*opened*, so every navigation here counts as an open
  // -- including re-opening the already-active page (e.g. a different
  // heading target on the same page) -- and moves it to the top rather than
  // duplicating it. Trashed pages are excluded: they're reached only from
  // the Trash list itself, and recording them would leave a dead link
  // behind in Recent once the page is later purged.
  const isTrashedPage = resolution.kind === "persisted" && resolution.inTrash === true;
  if (!isTrashedPage) {
    recordRecentOpen(
      resolution.kind === "persisted"
        ? { key: `p:${resolution.id}`, title: resolution.title, kind: "persisted", pageId: resolution.id }
        : { key: `d:${resolution.normalizedTitle}`, title: resolution.normalizedTitle, kind: "dynamic" },
    );
  }

  if (resolution.kind === "persisted") {
    openPageState.value = {
      kind: "persisted",
      id: resolution.id,
      inTrash: resolution.inTrash ?? false,
      trashedFilename: resolution.trashedFilename ?? null,
    };
  } else {
    openPageState.value = { kind: "dynamic", title: resolution.normalizedTitle };
  }

  const newKey = keyForResolution(resolution);
  if (newKey !== openPageKeyState.value) {
    openPageKeyState.value = newKey;
    openPageMarkdownState.value = resolution.kind === "persisted" ? resolution.body : "";
  }
}

/**
 * Explicit "new page" action (issue 04): persists a real file immediately,
 * then re-fetches the page list (the only thing a new page changes).
 * Opening it into the editor is the caller's job, same as before.
 */
export async function create(title: string): Promise<PageSummary> {
  const summary = await createPage(title);
  await refreshPages();
  return summary;
}

/**
 * Explicit "save" action for an already-persisted page: saves the edited
 * markdown body. Nothing else changes (the page list's titles/ids are
 * untouched by a body save), so nothing is re-fetched.
 */
export async function save(id: string, markdown: string): Promise<void> {
  await savePage(id, markdown);
}

/**
 * "materialize" action (dynamic -> persisted, ADR-0009): the first write to
 * a dynamic page creates its backing file, using the same mechanics as
 * `create` above. Updates the open page to the now-persisted page, rewrites
 * its Recent entry in place (so it stays pointing at the same page instead
 * of leaving a stale dynamic-kind entry), and re-fetches the page list
 * (the new page needs to show up in "All pages").
 */
export async function materialize(title: string, markdown: string): Promise<PageSummary> {
  const dynamicKey = `d:${title}`;
  const summary = await materializeAndSavePage(title, markdown);
  // Ticket 06's "commit on page switch" flush can call this for a page the
  // user has since navigated away from (a slow materialize racing a fast
  // switch) -- only update the *currently open* page if it's still this
  // same dynamic page, so a late-resolving materialize doesn't clobber
  // whatever's since been navigated to. `openPageKey` is deliberately left
  // untouched either way -- see its own comment above.
  if (openPageState.value?.kind === "dynamic" && openPageState.value.title === title) {
    openPageState.value = { kind: "persisted", id: summary.id, inTrash: false, trashedFilename: null };
  }
  recentState.value = recentState.value.map((entry) =>
    entry.key === dynamicKey
      ? { key: `p:${summary.id}`, title: summary.title, kind: "persisted", pageId: summary.id }
      : entry,
  );
  await refreshPages();
  return summary;
}

/**
 * Explicit "rename page" action: renames on the backend (which also
 * rewrites every other page's inbound `[[Old Title]]` links), re-fetches
 * the page list, and updates the renamed page's own Recent entry in place.
 * Reloading the currently-open page's editor content, if it was affected,
 * stays the caller's job (`main.ts`'s `handleRenamePageClick`) -- that's a
 * rendering concern, not page-list state.
 */
export async function rename(id: string, newTitle: string): Promise<RenamePageResult> {
  const result = await renamePage(id, newTitle);
  await refreshPages();
  recentState.value = recentState.value.map((entry) =>
    entry.kind === "persisted" && entry.pageId === id ? { ...entry, title: result.title } : entry,
  );
  return result;
}

/**
 * Explicit "delete page" action (issue 10 / ADR-0010): moves the page's
 * file into trash, clears it as the open page if it was the one open,
 * drops its Recent entry, and re-fetches both the page list and the trash
 * list (the only two lists this touches).
 */
export async function deletePage(id: string): Promise<void> {
  await trashPageCommand(id);
  if (openPageState.value?.kind === "persisted" && openPageState.value.id === id) {
    openPageState.value = null;
    openPageKeyState.value = "";
    openPageMarkdownState.value = "";
  }
  pruneRecentEntries([id]);
  await refreshPages();
  await refreshTrash();
}

/**
 * Explicit "restore" action (issue 10): moves a trashed page's file back to
 * its original path. Unconditionally clears the open page first -- the
 * restored page's `inTrash` flag flips, but its id doesn't change, and the
 * caller's "already open, skip reload" check keys off id alone, so without
 * this the trash banner/restore button would never clear on the page just
 * restored. Re-fetches both lists; opening the restored page back into the
 * editor stays the caller's job.
 */
export async function restore(trashedFilename: string): Promise<PageSummary> {
  const summary = await restorePage(trashedFilename);
  openPageState.value = null;
  openPageKeyState.value = "";
  openPageMarkdownState.value = "";
  await refreshPages();
  await refreshTrash();
  return summary;
}

/**
 * Explicit "empty trash" action (issue 10): permanently deletes every
 * trashed page. Drops every purged page's Recent entry, clears the open
 * page if it was a trashed page (there is no longer anything to restore
 * back to), and re-fetches the trash list only -- "All pages" is untouched,
 * since trashed pages were never in it.
 */
export async function emptyTrash(): Promise<void> {
  const purgedIds = (await listTrashedPages()).map((page) => page.id);
  await emptyTrashCommand();
  pruneRecentEntries(purgedIds);
  if (openPageState.value?.kind === "persisted" && openPageState.value.inTrash) {
    openPageState.value = null;
    openPageKeyState.value = "";
    openPageMarkdownState.value = "";
  }
  await refreshTrash();
}

/**
 * Clears every list plus the open page and Recent -- used when leaving the
 * current vault (`main.ts`'s "Change folder…"), since none of it belongs to
 * the vault about to open. The vault picker's own vault-open path
 * (`refreshPages`/`refreshTrash` above) populates everything fresh
 * afterwards.
 */
export function reset(): void {
  pagesState.value = [];
  trashState.value = [];
  recentState.value = [];
  openPageState.value = null;
  openPageKeyState.value = "";
  openPageMarkdownState.value = "";
}
