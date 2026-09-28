import { getPage, resolvePage, type PageResolution } from "./vault-api";
import { mountIsland } from "./mount-island";
import { initTheme } from "./state/theme";
import { refreshSyncStatus } from "./state/sync";
import { openSearchModal, openSettings, closeModal, modal, vaultView } from "./state/ui";
import * as pagesState from "./state/pages";
import SyncIndicatorContainer from "./surfaces/sync-indicator/SyncIndicatorContainer.vue";
import SyncPopupContainer from "./surfaces/sync-indicator/SyncPopupContainer.vue";
import SearchModalContainer from "./surfaces/search/SearchModalContainer.vue";
import VaultPickerContainer from "./surfaces/vault-picker/VaultPickerContainer.vue";
import CloneWizardContainer from "./surfaces/clone-wizard/CloneWizardContainer.vue";
import CloneManualFormContainer from "./surfaces/clone-manual-form/CloneManualFormContainer.vue";
import ConnectWizardContainer from "./surfaces/connect-wizard/ConnectWizardContainer.vue";
import SettingsContainer from "./surfaces/settings/SettingsContainer.vue";
import ArticleContainer from "./surfaces/article/ArticleContainer.vue";
import PageListContainer from "./surfaces/page-list/PageListContainer.vue";
import RecentContainer from "./surfaces/recent/RecentContainer.vue";
import TrashContainer from "./surfaces/trash/TrashContainer.vue";
import { createApp, watch } from "vue";
// Foundation ticket (01): every native dialog call in this file goes
// through `./dialogs`, which documents (in one place) why `confirm`/
// `message` come from the dialog plugin rather than `window.confirm`/
// `window.alert`, and why a few pre-existing call sites still use those
// broken globals unchanged.
import { messageDialog } from "./dialogs";

// Ticket 10: the guided clone wizard and the standalone "git clone" manual
// form are now the `surfaces/clone-wizard/`/`surfaces/clone-manual-form/`
// Vue islands, mounted at `#clone-wizard-root`/`#clone-manual-root` --
// each shows/hides itself from `state/ui.ts`'s `vaultView` and needs no
// root props (both read/call everything they need directly).
const cloneWizardRootEl = document.querySelector<HTMLElement>("#clone-wizard-root");
if (cloneWizardRootEl) mountIsland(cloneWizardRootEl, CloneWizardContainer);
const cloneManualRootEl = document.querySelector<HTMLElement>("#clone-manual-root");
if (cloneManualRootEl) mountIsland(cloneManualRootEl, CloneManualFormContainer);

// Ticket 11: the guided connect wizard is now the `surfaces/connect-wizard/`
// Vue island, mounted at `#connect-wizard-root` -- shown from `state/ui.ts`'s
// `modal` state (`'connectWizard'`), layered over the Settings modal
// (ticket 12: also Vue now) exactly as before (see `styles.css`'s
// `.connect-wizard` doc comment). Needs no root props anymore: ticket 12
// retired its one temporary callback (`refreshCommitAuthorFieldsInVanilla`)
// -- see `useConnectWizard.ts`'s doc comment.
const connectWizardRootEl = document.querySelector<HTMLElement>("#connect-wizard-root");
if (connectWizardRootEl) mountIsland(connectWizardRootEl, ConnectWizardContainer);

// Ticket 12: Settings is now the `surfaces/settings/` Vue island, mounted at
// `#settings-modal-root` -- shown from `state/ui.ts`'s `modal` state
// (`'settings'`), replacing `#settings-modal-overlay`'s old direct
// `hidden`-attribute toggling and every handler that used to live below in
// this file. Needs no root props: it reads `state/`, calls `vault-api`/
// `dialogs.ts` directly, and consumes `openSettings`'s deep link itself via
// `state/ui.ts`'s `settingsDeepLink` (retiring the `openSettingsInVanilla`
// callback the sync popup island used to receive -- see
// `SyncPopupContainer.vue`'s doc comment).
const settingsModalRootEl = document.querySelector<HTMLElement>("#settings-modal-root");
if (settingsModalRootEl) mountIsland(settingsModalRootEl, SettingsContainer);

const workspaceEl = document.querySelector<HTMLElement>("#workspace");
const sidebarEl = document.querySelector<HTMLElement>("#sidebar");
const searchButtonEl = document.querySelector<HTMLButtonElement>("#search-button");
const sidebarOpenButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-open-button");
const sidebarCloseButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-close-button");
const sidebarCollapseToggleEl = document.querySelector<HTMLButtonElement>("#sidebar-collapse-toggle");
const sidebarOverlayEl = document.querySelector<HTMLElement>("#sidebar-overlay");
const sidebarRailExpandButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-rail-expand");
const sidebarRailSearchButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-rail-search");
const sidebarRailNewPageButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-rail-new-page");
const settingsButtonEl = document.querySelector<HTMLButtonElement>("#settings-button");
const sidebarRailSettingsButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-rail-settings");

// Ticket 02: the sidebar sync-status indicator (footer + rail icon) and its
// popup are now the `surfaces/sync-indicator/` Vue island, mounted below at
// `#sync-indicator-footer-root`/`#sync-indicator-rail-root`/
// `#sync-popup-root`.
//
// Ticket 12: Settings' own "Sync" section markup (not-connected/Disconnect
// banners, the four raw sub-forms) and the "Credentials" section are now
// part of the `surfaces/settings/` Vue island mounted above -- nothing left
// to look up here.

// Ticket 02 (the pilot): the sync indicator/popup Vue islands, mounted at
// module scope like every other DOM lookup here (the script is `defer`red,
// so the DOM is already parsed by the time this file runs). Two indicator
// instances share the same `state/sync.ts`/`state/ui.ts` state as the one
// popup instance. Needs no root props anymore: ticket 12 retired its one
// temporary callback (`openSettingsInVanilla`) -- see
// `SyncPopupContainer.vue`'s doc comment.
const syncIndicatorFooterRootEl = document.querySelector<HTMLElement>("#sync-indicator-footer-root");
const syncIndicatorRailRootEl = document.querySelector<HTMLElement>("#sync-indicator-rail-root");
const syncPopupRootEl = document.querySelector<HTMLElement>("#sync-popup-root");
if (syncIndicatorFooterRootEl) mountIsland(syncIndicatorFooterRootEl, SyncIndicatorContainer, { variant: "footer" });
if (syncIndicatorRailRootEl) mountIsland(syncIndicatorRailRootEl, SyncIndicatorContainer, { variant: "rail" });
if (syncPopupRootEl) mountIsland(syncPopupRootEl, SyncPopupContainer);

// Ticket 03: the search modal is now the `surfaces/search/` Vue island,
// mounted at `#search-modal-root`. Its two temporary callback root props
// (`openPageByTitleInVanilla`/`createPageFromQueryInVanilla`) are defined
// further down, alongside `openPageByTitle`/`createPage`'s other callers --
// referencing them here relies on `function` hoisting, same as before.
const searchModalRootEl = document.querySelector<HTMLElement>("#search-modal-root");
if (searchModalRootEl) {
  mountIsland(searchModalRootEl, SearchModalContainer, {
    openPageByTitleInVanilla,
    createPageFromQueryInVanilla,
  });
}

// Ticket 04: the vault picker is now the `surfaces/vault-picker/` Vue
// island, mounted at `#vault-picker-root`. `#workspace`'s own visibility
// now follows `state/ui.ts`'s `vaultView` (replacing
// `showVaultPicker`/`showWorkspace`'s toggling of it) -- the picker's own
// visibility is the island's own `v-if` on that same state.
//
// Ticket 05: `loadPagesAndTrashInVanilla` is gone -- page/trash state is
// now `state/pages.ts`, so the container calls its `refreshPages`/
// `refreshTrash` directly (via `registerVaultOpenedHandler`) instead of
// routing through a callback into this file.
//
// Ticket 10: the picker's last two temporary callback root props
// (`openCloneWizardInVanilla`/`openCloneManualInVanilla`) are gone -- the
// guided clone wizard and the standalone manual clone form are now their
// own Vue islands (mounted above), each driven by `vaultView` alone, so the
// picker's container just sets that view directly.
const vaultPickerRootEl = document.querySelector<HTMLElement>("#vault-picker-root");
if (vaultPickerRootEl) mountIsland(vaultPickerRootEl, VaultPickerContainer);

// Ticket 07: the whole article view -- title row (rename/delete), trash
// banner (Restore), the editor (embedded via a slot; see
// `surfaces/article/ArticleContainer.vue`'s template), and backlinks -- is
// now the `surfaces/article/` Vue island, mounted at `#article-root`
// (replacing `#page-view`). It needs no root props at all: unlike ticket
// 06's editor island, it reads `state/pages.ts` and calls `vault-api`/
// `dialogs.ts` directly for everything it owns (rename/delete/restore,
// backlinks, and opening a `[[Link]]` chip's target), so there's no more
// vanilla logic for a callback to reach back into.
//
// Mounted directly (not via `mountIsland`) so `articleHandle` below can
// keep a typed reference to the container's own `defineExpose` --
// `mountIsland` intentionally returns only the `App` (see its own doc
// comment), which doesn't expose that. This is the one place `main.ts`
// still needs an imperative handle into an island, for `scrollToHeading`
// (spec.md#imperative-escape-hatches), used by navigations that originate
// outside the article surface itself (the sidebar's page/Recent/Trash
// lists, search, "Today") -- a `[[Link]]` click inside the editor scrolls
// on its own, entirely within the container.
const articleRootEl = document.querySelector<HTMLElement>("#article-root");
let articleHandle: { scrollToHeading(slug: string): boolean } | null = null;
if (articleRootEl) {
  const articleApp = createApp(ArticleContainer);
  articleHandle = articleApp.mount(articleRootEl) as unknown as { scrollToHeading(slug: string): boolean };
}

// Ticket 08: the sidebar's Recent list is now the `surfaces/recent/` Vue
// island, mounted at `#recent-root` -- it reads `state/pages.ts` and calls
// `vault-api` directly, so it needs no root props. `highlightActivePage`
// (removed below) used to reach into this list's DOM; the active item now
// comes from the container's own `activeKey` prop instead.
const recentRootEl = document.querySelector<HTMLElement>("#recent-root");
if (recentRootEl) mountIsland(recentRootEl, RecentContainer);

// Ticket 08: the sidebar's "All pages" list (New page, Today) is now the
// `surfaces/page-list/` Vue island, mounted at `#page-list-root` -- same
// no-root-props shape as Recent above.
//
// Mounted directly (not via `mountIsland`), same reason as the article
// island above: `pageListHandle` keeps a typed reference to the
// container's own `defineExpose({ newPage })`, so the still-vanilla
// sidebar rail's own "+" icon (a separate button on a surface that hasn't
// migrated yet) can trigger the exact same "new page" flow the sidebar's
// own button (now inside the island) does.
const pageListRootEl = document.querySelector<HTMLElement>("#page-list-root");
let pageListHandle: { newPage(): Promise<void> } | null = null;
if (pageListRootEl) {
  const pageListApp = createApp(PageListContainer);
  pageListHandle = pageListApp.mount(pageListRootEl) as unknown as { newPage(): Promise<void> };
}

// Ticket 09: the sidebar's Trash list (with Empty trash) is now the
// `surfaces/trash/` Vue island, mounted at `#trash-root` -- same
// no-root-props shape as Recent/"All pages" above. `renderTrashList`/
// `handleEmptyTrashClick` (removed below) used to live here;
// `TrashContainer.vue` now confirms "Empty trash" through `dialogs.ts`
// itself and calls `state/pages.ts`'s `emptyTrash` action directly.
const trashRootEl = document.querySelector<HTMLElement>("#trash-root");
if (trashRootEl) mountIsland(trashRootEl, TrashContainer);

/**
 * Calls the mounted article island's exposed `scrollToHeading`, retrying
 * briefly: a fresh navigation reloads the editor's content asynchronously
 * (the wrapper's own `pageKey` watcher awaits `PageEditor.load()`), so the
 * heading's DOM node may not exist yet the instant this is called. Bounded
 * and self-cancelling -- once `scrollToHeading` returns `true`, or the
 * budget runs out, it stops. A no-op if nothing is mounted yet.
 */
function scrollToHeadingWhenReady(slug: string) {
  const attempts = 20;
  const intervalMs = 25;
  let tries = 0;
  const tick = () => {
    if (articleHandle?.scrollToHeading(slug)) return;
    tries += 1;
    if (tries < attempts) setTimeout(tick, intervalMs);
  };
  tick();
}

watch(
  vaultView,
  (view) => {
    if (view === "workspace") workspaceEl?.removeAttribute("hidden");
    else workspaceEl?.setAttribute("hidden", "");
  },
  { immediate: true },
);

// Ticket 04/12: Settings' own read of the vault path is now the
// `surfaces/settings/` island's own reactive prop (`state/vault.ts`'s
// `vaultPath`, read directly by `SettingsContainer.vue`) -- nothing left to
// write into vanilla DOM here.

// Ticket 04: `friendlyVaultOpenError` moved to `./vault-open-error` (shared
// by the vault picker Vue island and Settings' "Change folder…", now inside
// `surfaces/settings/SettingsContainer.vue`).
//
// `showVaultPicker`/`showWorkspace` are gone -- `#workspace`'s visibility
// now follows `state/ui.ts`'s `vaultView` (see the `watch(vaultView, ...)`
// near the other module-scope island setup, above), and `#vault-picker`'s
// own visibility is the Vue island's own `v-if` on that same state.

/**
 * Opens whatever `resolution` points to: an existing persisted page, or a
 * dynamic (unmaterialized) one. The Recent-recording and open-page state
 * update themselves live in `state/pages.ts`'s `open` action (ticket 05).
 *
 * Ticket 07: the article view (title, rename/delete, the trash banner,
 * backlinks) is now the `surfaces/article/` island, and it reads
 * `state/pages.ts` reactively -- so this function no longer renders
 * anything itself, and no longer needs the "already open" skip it used to
 * (a `pagesState.open()` call whose resolution doesn't actually change
 * `openPage`'s value is a no-op for anyone `watch`ing it, article island
 * included).
 *
 * Ticket 08: `highlightActivePage()` is gone (the page list/Recent islands
 * derive their own active item from state now), so this only remains for
 * its other job: the heading-targeted scroll for a navigation that
 * originates outside the article surface itself. The only remaining caller
 * is the search modal's `openPageByTitleInVanilla` callback -- the page
 * list/Recent/Trash islands and "Today" call `vault-api`/`state/pages.ts`
 * directly instead (see `surfaces/page-list/PageListContainer.vue`/
 * `surfaces/recent/RecentContainer.vue`/`surfaces/trash/TrashContainer.vue`),
 * since none of them ever has a heading target to scroll to.
 */
async function openResolution(resolution: PageResolution) {
  pagesState.open(resolution);
  if (resolution.headingSlug) scrollToHeadingWhenReady(resolution.headingSlug);
}

async function selectPage(id: string) {
  const current = pagesState.openPage.value;
  if (current?.kind === "persisted" && current.id === id && !current.inTrash) return;
  const page = await getPage(id);
  await openResolution({ kind: "persisted", id: page.id, title: page.title, body: page.body, html: page.html });
}

/** Navigates to whatever a clicked `[[Link]]` chip targets (issue 05). */
async function openPageByTitle(rawTitle: string) {
  const resolution = await resolvePage(rawTitle);
  await openResolution(resolution);
}

// Ticket 07: "delete page"/"rename page"/"restore" (issue 10 / ADR-0010,
// and the rename action the ticket 04 checklist left undone) are now
// `surfaces/article/ArticleContainer.vue`'s job -- it confirms through
// `dialogs.ts` and calls `state/pages.ts`'s actions itself, and its own
// reactive read of `openPage`/`pages`/`trash` (for the article's title)
// means it doesn't need this file to push a re-render at it afterwards.

// Ticket 09: the Trash list's rendering (`renderTrashList`) and "Empty
// trash" (`handleEmptyTrashClick`) moved into `surfaces/trash/
// TrashContainer.vue`, which confirms through `dialogs.ts` and calls
// `state/pages.ts`'s `emptyTrash` action itself, the same shape as the
// article container above.

// Ticket 08: "new page" (prompt, `pagesState.create`, open it) moved into
// `surfaces/page-list/PageListContainer.vue`. The still-vanilla sidebar
// rail's own "+" icon triggers it via `pageListHandle.newPage()` (mounted
// above) instead of calling a local function here.

// Ticket 04: `openVaultAndLoad`/`handleSelectVaultClick` moved into
// `surfaces/vault-picker/VaultPickerContainer.vue` (its "Select vault
// folder…" handler and its `onMounted` remembered-vault auto-open), which
// call `state/vault.ts`'s `openVault` action instead.

// --- Search modal's temporary callbacks (ticket 03) ------------------------
//
// The search modal itself is now `surfaces/search/` (a Vue island mounted
// above); these two intents are all it still needs from vanilla code -- the
// modal's rendering never migrated, so it still can't call `openResolution`/
// `selectPage`'s rendering itself. Ticket 05 moved the state each of these
// touches into `state/pages.ts`, but both callbacks stay (unlike the vault
// picker's third callback, which was pure state and is gone) since the rest
// of what they do -- closing the modal, rendering the opened/created page --
// is still vanilla. Both close the modal via `state/ui.ts`'s `closeModal`
// themselves, matching the old `openSearchResult`/`handleCreatePageFromSearch`'s
// exact control flow (in particular: on `createPage` failure, the modal
// stays open).

/** Opens the resolved search result the same way clicking any `[[Link]]` chip or a Trash-list entry would -- `resolve_page` already handles the "in trash" state, so this works identically for a persisted or a trashed hit. */
async function openPageByTitleInVanilla(title: string) {
  closeModal();
  await openPageByTitle(title);
}

/** Empty-state action: creates the typed query as a brand-new page and opens it straight into the editor, reusing the exact same action as the "New page" button. */
async function createPageFromQueryInVanilla(query: string) {
  try {
    const summary = await pagesState.create(query);
    closeModal();
    await selectPage(summary.id);
  } catch (err) {
    await messageDialog(String(err));
  }
}

// --- Settings modal --------------------------------------------------------
//
// Ticket 12: Settings is now the `surfaces/settings/` Vue island (mounted
// near the top of this file). It's opened via `state/ui.ts`'s `openSettings`
// action (the sidebar's gear button, both expanded and collapsed-rail
// states, and Cmd/Ctrl+, below) and owns its own Escape/backdrop-click
// closing (via `components/Modal.vue`) -- nothing left to wire up here.
// Every setting still applies live -- no Save/Cancel.

/**
 * Compact-mode layout (issue 12): Desktop (Windows/Linux) gets a persistent,
 * user-collapsible sidebar; Android gets a hamburger-triggered drawer,
 * hidden by default. This project has no runtime OS-detection yet, so
 * rather than build one for an MVP milestone with no Android device to test
 * on, both behaviors are driven by one boolean -- "compact mode" -- and a
 * viewport-width media query stands in as a *proxy* for "phone-sized
 * screen." This is explicitly a proxy, not real platform detection: it will
 * also fire on a narrow desktop window. Ticket 15 (actual Android bring-up)
 * is expected to replace this with a real platform check (e.g.
 * `@tauri-apps/plugin-os`) if the proxy proves wrong in practice.
 */
const COMPACT_MEDIA_QUERY = window.matchMedia("(max-width: 700px)");

function closeSidebarDrawer() {
  sidebarEl?.classList.remove("drawer-open");
  sidebarOverlayEl?.setAttribute("hidden", "");
}

function applyLayoutMode() {
  const compact = COMPACT_MEDIA_QUERY.matches;
  workspaceEl?.classList.toggle("compact", compact);
  // Neither mode's "hidden" state should leak into the other when the
  // viewport crosses the threshold (e.g. resizing a window).
  closeSidebarDrawer();
}

async function init() {
  searchButtonEl?.addEventListener("click", openSearchModal);
  window.addEventListener("keydown", (event) => {
    const isSearchShortcut = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k";
    const isSettingsShortcut = (event.ctrlKey || event.metaKey) && event.key === ",";
    if (isSearchShortcut) {
      event.preventDefault();
      // Ticket 03: `state/ui.ts`'s `openSearchModal` action -- idempotent
      // when the modal's already open (the surface itself keeps input
      // focus in that case, so there's nothing more to do here).
      openSearchModal();
    } else if (isSettingsShortcut) {
      event.preventDefault();
      // Ticket 12: Settings owns its own Escape handling now (via
      // `components/Modal.vue`) -- this only toggles it open/closed, same
      // as the search shortcut above.
      if (modal.value === "settings") closeModal();
      else openSettings();
    }
  });

  // Ticket 12: both Settings entry points now call `state/ui.ts`'s
  // `openSettings` directly, the same way `searchButtonEl` above calls
  // `openSearchModal` directly -- Settings itself is the
  // `surfaces/settings/` Vue island (mounted near the top of this file),
  // which owns everything past this click.
  settingsButtonEl?.addEventListener("click", () => openSettings());
  sidebarRailSettingsButtonEl?.addEventListener("click", () => openSettings());
  void refreshSyncStatus();

  // Ticket 10: the guided clone wizard's and the standalone "git clone"
  // manual dialog's wiring (close/back/switch-to-manual buttons, Escape,
  // backdrop click, and every sub-form control) is now owned by
  // `surfaces/clone-wizard/`/`surfaces/clone-manual-form/`'s own
  // containers/presentational SFCs -- there is nothing left to wire up here.

  // Ticket 12: Settings' "Sync"/"Credentials" sections (raw connect forms,
  // credential-kind toggle, Disconnect, orphan notice, remove all) are now
  // owned by `surfaces/settings/SettingsContainer.vue` -- nothing left to
  // wire up here.

  // Ticket 03: the search modal is now the `surfaces/search/` Vue island
  // (mounted near the top of this file, at module scope) -- it owns its
  // own input/checkbox/keyboard-nav/Escape/backdrop-click handling,
  // replacing everything that used to be wired up here.

  sidebarCollapseToggleEl?.addEventListener("click", () => {
    workspaceEl?.classList.add("sidebar-collapsed");
  });
  sidebarRailExpandButtonEl?.addEventListener("click", () => {
    workspaceEl?.classList.remove("sidebar-collapsed");
  });
  sidebarRailSearchButtonEl?.addEventListener("click", openSearchModal);
  sidebarRailNewPageButtonEl?.addEventListener("click", () => void pageListHandle?.newPage());
  sidebarOpenButtonEl?.addEventListener("click", () => {
    sidebarEl?.classList.add("drawer-open");
    sidebarOverlayEl?.removeAttribute("hidden");
  });
  sidebarCloseButtonEl?.addEventListener("click", closeSidebarDrawer);
  sidebarOverlayEl?.addEventListener("click", closeSidebarDrawer);
  COMPACT_MEDIA_QUERY.addEventListener("change", applyLayoutMode);
  applyLayoutMode();

  // Ticket 12: theme state moved into `state/theme.ts` -- replaces this
  // file's own `getSettings()` -> `applyTheme` -> `setThemeRadioValue`
  // sequence (the last of which no longer applies: the Settings surface
  // reads `state/theme.ts`'s `theme` reactively instead).
  await initTheme();

  // Ticket 04: the remembered-vault auto-open this used to do here (via
  // `openVaultAndLoad`) moved into `VaultPickerContainer`'s own `onMounted`
  // -- it owns every vault-open backend call now, including this one.
}

window.addEventListener("DOMContentLoaded", () => {
  void init();
});
