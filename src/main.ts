import {
  getSettings,
  setTheme,
  pickVaultFolder,
  getPage,
  resolvePage,
  connectAccessToken,
  generateSshKey,
  importSshKey,
  connectSshKey,
  startGithubDeviceFlow,
  pollGithubDeviceFlow,
  checkGithubInstallation,
  connectGithubOauth,
  createGithubRepository,
  listGithubRepositories,
  startGitlabDeviceFlow,
  pollGitlabDeviceFlow,
  connectGitlabOauth,
  createGitlabRepository,
  listGitlabRepositories,
  commitAuthorPrefill,
  getCommitAuthor,
  confirmCommitAuthor,
  disconnectVault,
  scanOrphanedConnections,
  cleanupOrphanedConnections,
  removeAllStoredCredentials,
  PLAINTEXT_CONSENT_REQUIRED_ERROR,
  type DisconnectOutcome,
  type Provider,
  type CommitAuthor,
  type PageResolution,
  type Theme,
  type RepoInfo,
  type SshKeyInfo,
  type CredentialKind,
} from "./vault-api";
import { mountIsland } from "./mount-island";
import { refreshSyncStatus, syncStatus } from "./state/sync";
import {
  openSettings,
  openSearchModal,
  closeModal,
  vaultView,
  type OpenSettingsOptions,
} from "./state/ui";
import { vaultPath, openVault } from "./state/vault";
import * as pagesState from "./state/pages";
import { friendlyVaultOpenError } from "./vault-open-error";
import SyncIndicatorContainer from "./surfaces/sync-indicator/SyncIndicatorContainer.vue";
import SyncPopupContainer from "./surfaces/sync-indicator/SyncPopupContainer.vue";
import SearchModalContainer from "./surfaces/search/SearchModalContainer.vue";
import VaultPickerContainer from "./surfaces/vault-picker/VaultPickerContainer.vue";
import CloneWizardContainer from "./surfaces/clone-wizard/CloneWizardContainer.vue";
import CloneManualFormContainer from "./surfaces/clone-manual-form/CloneManualFormContainer.vue";
import ArticleContainer from "./surfaces/article/ArticleContainer.vue";
import PageListContainer from "./surfaces/page-list/PageListContainer.vue";
import RecentContainer from "./surfaces/recent/RecentContainer.vue";
import TrashContainer from "./surfaces/trash/TrashContainer.vue";
import { createApp, watch } from "vue";
import {
  reduceWizard,
  initialWizardState,
  isDone as isWizardDone,
  isSwitchedToManual as isWizardSwitchedToManual,
  isBusyStep,
  type WizardState,
  type WizardAction,
  type WizardProvider,
} from "./connect-wizard";
// Foundation ticket (01): every native dialog call in this file goes
// through `./dialogs`, which documents (in one place) why `confirm`/
// `message` come from the dialog plugin rather than `window.confirm`/
// `window.alert`, and why a few pre-existing call sites still use those
// broken globals unchanged.
import { confirmDialog, messageDialog, promptDialog, confirmBrowser, alertBrowser } from "./dialogs";

// Ticket 10: the guided clone wizard and the standalone "git clone" manual
// form are now the `surfaces/clone-wizard/`/`surfaces/clone-manual-form/`
// Vue islands, mounted at `#clone-wizard-root`/`#clone-manual-root` --
// each shows/hides itself from `state/ui.ts`'s `vaultView` and needs no
// root props (both read/call everything they need directly).
const cloneWizardRootEl = document.querySelector<HTMLElement>("#clone-wizard-root");
if (cloneWizardRootEl) mountIsland(cloneWizardRootEl, CloneWizardContainer);
const cloneManualRootEl = document.querySelector<HTMLElement>("#clone-manual-root");
if (cloneManualRootEl) mountIsland(cloneManualRootEl, CloneManualFormContainer);

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
// `#sync-popup-root`. Only Settings' still-vanilla "Sync" section elements
// remain here.
const syncSectionNotConnectedEl = document.querySelector<HTMLElement>("#sync-section-not-connected");
const syncSectionConnectButtonEl = document.querySelector<HTMLButtonElement>("#sync-section-connect-button");
// Ticket 14: "Disconnect…" (the inverse banner of the two above) plus the
// standalone "Credentials" section's orphan notice and "Remove all" action.
const syncSectionConnectedEl = document.querySelector<HTMLElement>("#sync-section-connected");
const syncSectionDisconnectButtonEl = document.querySelector<HTMLButtonElement>("#sync-section-disconnect-button");

// Ticket 02 (the pilot): the sync indicator/popup Vue islands, mounted at
// module scope like every other DOM lookup here (the script is `defer`red,
// so the DOM is already parsed by the time this file runs). Two indicator
// instances share the same `state/sync.ts`/`state/ui.ts` state as the one
// popup instance. `openSettingsInVanilla` is the popup's only temporary
// callback root prop -- see its own doc comment below.
const syncIndicatorFooterRootEl = document.querySelector<HTMLElement>("#sync-indicator-footer-root");
const syncIndicatorRailRootEl = document.querySelector<HTMLElement>("#sync-indicator-rail-root");
const syncPopupRootEl = document.querySelector<HTMLElement>("#sync-popup-root");
if (syncIndicatorFooterRootEl) mountIsland(syncIndicatorFooterRootEl, SyncIndicatorContainer, { variant: "footer" });
if (syncIndicatorRailRootEl) mountIsland(syncIndicatorRailRootEl, SyncIndicatorContainer, { variant: "rail" });
if (syncPopupRootEl) {
  mountIsland(syncPopupRootEl, SyncPopupContainer, { openSettingsInVanilla });
}
const settingsOrphanNoticeEl = document.querySelector<HTMLElement>("#settings-orphan-notice");
const settingsOrphanCleanupButtonEl = document.querySelector<HTMLButtonElement>("#settings-orphan-cleanup-button");
const settingsRemoveAllCredentialsButtonEl = document.querySelector<HTMLButtonElement>(
  "#settings-remove-all-credentials-button",
);
const settingsRemoveAllCredentialsStatusEl = document.querySelector<HTMLElement>(
  "#settings-remove-all-credentials-status",
);

// Ticket 03: the search modal is now the `surfaces/search/` Vue island,
// mounted at `#search-modal-root`. Its two temporary callback root props
// (`openPageByTitleInVanilla`/`createPageFromQueryInVanilla`) are defined
// further down, alongside `openPageByTitle`/`createPage`'s other callers --
// referencing them here relies on `function` hoisting, same as
// `openSettingsInVanilla` above.
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

const settingsModalOverlayEl = document.querySelector<HTMLElement>("#settings-modal-overlay");
const settingsVaultPathEl = document.querySelector<HTMLElement>("#settings-vault-path");
// Ticket 04: replaces every call site's own `settingsVaultPathEl.textContent
// = path` write (`openVaultAndLoad`, the clone wizard/manual form's own
// finishing tails -- now `useCloneWizard`'s `finishIntoWorkspace`/
// `CloneManualFormContainer`'s `handleSubmit` -- and
// `handleChangeVaultFolderClick`) with one spot that follows
// `state/vault.ts`'s `vaultPath` -- still-vanilla Settings' one remaining
// read of the vault path.
watch(
  vaultPath,
  (path) => {
    if (settingsVaultPathEl) settingsVaultPathEl.textContent = path ?? "";
  },
  { immediate: true },
);
const settingsChangeFolderButtonEl = document.querySelector<HTMLButtonElement>("#settings-change-folder-button");
const settingsThemeRadios = document.querySelectorAll<HTMLInputElement>('input[name="settings-theme"]');
const settingsConnectFormEl = document.querySelector<HTMLFormElement>("#settings-connect-form");
const settingsConnectUrlEl = document.querySelector<HTMLInputElement>("#settings-connect-url");
const settingsConnectUsernameEl = document.querySelector<HTMLInputElement>("#settings-connect-username");
const settingsConnectTokenEl = document.querySelector<HTMLInputElement>("#settings-connect-token");
const settingsConnectButtonEl = document.querySelector<HTMLButtonElement>("#settings-connect-button");
const settingsConnectStatusEl = document.querySelector<HTMLElement>("#settings-connect-status");

// Ticket 11: the always-visible "Sync" section's credential-kind selector
// (shows/hides the sub-form matching the selected `data-sync-kind`), plus
// the new raw SSH key sub-form (tickets 04/06/07 already had a raw Settings
// form; ticket 05 only had the guided wizard's until now).
const syncCredentialKindEl = document.querySelector<HTMLSelectElement>("#sync-credential-kind");
const settingsSshKeyUrlEl = document.querySelector<HTMLInputElement>("#settings-sshkey-url");
const settingsSshKeyGenerateButtonEl = document.querySelector<HTMLButtonElement>("#settings-sshkey-generate-button");
const settingsSshKeyImportButtonEl = document.querySelector<HTMLButtonElement>("#settings-sshkey-import-button");
const settingsSshKeyConnectButtonEl = document.querySelector<HTMLButtonElement>("#settings-sshkey-connect-button");
const settingsSshKeyStatusEl = document.querySelector<HTMLElement>("#settings-sshkey-status");

// Ticket 06: minimal "Sign in with GitHub" device-flow UI elements.
const settingsGithubFormEl = document.querySelector<HTMLFormElement>("#settings-github-form");
const settingsGithubUrlEl = document.querySelector<HTMLInputElement>("#settings-github-url");
const settingsGithubButtonEl = document.querySelector<HTMLButtonElement>("#settings-github-button");
const settingsGithubDeviceCodeEl = document.querySelector<HTMLElement>("#settings-github-device-code");
const settingsGithubVerificationLinkEl = document.querySelector<HTMLAnchorElement>(
  "#settings-github-verification-link",
);
const settingsGithubUserCodeEl = document.querySelector<HTMLElement>("#settings-github-user-code");
const settingsGithubStatusEl = document.querySelector<HTMLElement>("#settings-github-status");
const settingsGithubInstallEl = document.querySelector<HTMLElement>("#settings-github-install");
const settingsGithubInstallLinkEl = document.querySelector<HTMLAnchorElement>("#settings-github-install-link");
const settingsGithubInstallContinueButtonEl = document.querySelector<HTMLButtonElement>(
  "#settings-github-install-continue-button",
);

/// Holds the acquired token pair between "device flow finished" and "the
/// user confirmed the app install" -- `connectGithubOauth` needs it, but
/// it can't be persisted (and shouldn't be, per ADR-0012's persist-gate)
/// until the install check clears and the real `connectGithubOauth` test
/// fetch succeeds.
let pendingGithubTokenPair: { accessToken: string; refreshToken: string; accessTokenExpiresAt: string } | null = null;

// Ticket 07: minimal "Sign in with GitLab" device-flow UI elements -- same
// shape as ticket 06's GitHub elements above, minus the install-CTA
// elements (a GitLab OAuth application needs no per-repo install step).
const settingsGitlabFormEl = document.querySelector<HTMLFormElement>("#settings-gitlab-form");
const settingsGitlabUrlEl = document.querySelector<HTMLInputElement>("#settings-gitlab-url");
const settingsGitlabButtonEl = document.querySelector<HTMLButtonElement>("#settings-gitlab-button");
const settingsGitlabDeviceCodeEl = document.querySelector<HTMLElement>("#settings-gitlab-device-code");
const settingsGitlabVerificationLinkEl = document.querySelector<HTMLAnchorElement>(
  "#settings-gitlab-verification-link",
);
const settingsGitlabUserCodeEl = document.querySelector<HTMLElement>("#settings-gitlab-user-code");
const settingsGitlabStatusEl = document.querySelector<HTMLElement>("#settings-gitlab-status");

// Ticket 08: the editable "Commit as" field -- prefilled from
// `commitAuthorPrefill`'s precedence chain when nothing is confirmed yet,
// or the vault's currently confirmed author otherwise (`getCommitAuthor`).
const settingsCommitAuthorFormEl = document.querySelector<HTMLFormElement>("#settings-commit-author-form");
const settingsCommitAuthorNameEl = document.querySelector<HTMLInputElement>("#settings-commit-author-name");
const settingsCommitAuthorEmailEl = document.querySelector<HTMLInputElement>("#settings-commit-author-email");
const settingsCommitAuthorButtonEl = document.querySelector<HTMLButtonElement>("#settings-commit-author-button");
const settingsCommitAuthorStatusEl = document.querySelector<HTMLElement>("#settings-commit-author-status");

// Ticket 09: the guided connect wizard's DOM handles. `connectWizardBodyEl`
// is rebuilt per-step by `renderWizardStep` below; everything else is
// static chrome (header, back/manual-setup footer) shared by every step.
const connectWizardOpenButtonEl = document.querySelector<HTMLButtonElement>("#connect-wizard-open-button");
const connectWizardOverlayEl = document.querySelector<HTMLElement>("#connect-wizard-overlay");
const connectWizardBodyEl = document.querySelector<HTMLElement>("#connect-wizard-body");
const connectWizardCloseButtonEl = document.querySelector<HTMLButtonElement>("#connect-wizard-close-button");
const connectWizardBackButtonEl = document.querySelector<HTMLButtonElement>("#connect-wizard-back-button");
const connectWizardManualButtonEl = document.querySelector<HTMLButtonElement>("#connect-wizard-manual-button");

// Ticket 05: the open page, page list, trash list, and Recent all moved
// into `state/pages.ts` (`pagesState.openPage`/`pagesState.pages`/
// `pagesState.trash`/`pagesState.recent`, all read-only) -- this file used
// to keep the rendering that read them.
//
// Ticket 08: the page list and Recent renderers (`renderPageList`/
// `renderRecentList`) and the sidebar's active-item highlight
// (`highlightActivePage`, which reached into both lists' DOM) are gone --
// they're now the `surfaces/page-list/`/`surfaces/recent/` Vue islands
// (mounted above), each reading `pagesState.pages`/`pagesState.openPage`/
// `pagesState.recent` reactively and deriving their own active item from a
// prop instead.
//
// Ticket 09: the Trash renderer (`renderTrashList`) and its `watch()` are
// gone too -- now the `surfaces/trash/` Vue island (mounted above), reading
// `pagesState.trash` reactively.
//
// Ticket 06: the editor and its autosave (`saveTimer`/`pageEditor`/
// `scheduleAutosave`/`flushSave`/`flushPendingSaveForCurrentPage`, all
// removed from this file) are now `surfaces/page-editor/`'s Vue island,
// embedded inside the ticket 07 article island below (mounted at
// `#article-root`). Its container owns the debounce and the immediate
// save-on-`commit` that fixes the lost-edit bug -- see its own doc
// comments.

// Ticket 04: `friendlyVaultOpenError` moved to `./vault-open-error` (shared
// by the vault picker Vue island and "Change folder…" below).
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
// Opened via the sidebar's gear button (both expanded and collapsed-rail
// states) and Cmd/Ctrl+,. Everything in it applies live -- no Save/Cancel --
// since each setting is independent and low-risk.

/** Applies `theme` to the page: "system" defers to `prefers-color-scheme` (no attribute), "light"/"dark" force it via `:root[data-theme]` overrides in styles.css. */
function applyTheme(theme: Theme) {
  if (theme === "system") {
    document.documentElement.removeAttribute("data-theme");
  } else {
    document.documentElement.setAttribute("data-theme", theme);
  }
}

async function handleThemeRadioChange(event: Event) {
  const theme = (event.target as HTMLInputElement).value as Theme;
  applyTheme(theme);
  try {
    await setTheme(theme);
  } catch (err) {
    console.error("Failed to persist theme", err);
  }
}

function setThemeRadioValue(theme: Theme) {
  settingsThemeRadios.forEach((radio) => {
    radio.checked = radio.value === theme;
  });
}

/**
 * "Change vault folder" action: a heavy, one-shot operation (full
 * index/git-repo rebuild against the new path, same as first-run
 * `open_vault`), so the native folder picker is the only confirmation --
 * no extra in-app dialog. Clears the currently open page since it belongs
 * to the vault being left.
 *
 * Ticket 06 known gap: `pagesState.reset()` below sets `openPage` to
 * `null`, which unmounts the editor island and so still fires its
 * "commit unsaved edit" flush (same mechanism as switching pages) -- but
 * that flush's own save call is fire-and-forget from here, no longer
 * `await`ed before `openVault(path)` starts against the new vault the way
 * the old, now-removed `flushPendingSaveForCurrentPage` guaranteed. A save
 * for an edit made in the last ~200ms before changing vaults could
 * therefore race the new vault opening -- narrower than the bug this
 * ticket fixes (that race needs both "editing" and "changing vaults" in
 * the same instant), and reaching it exposes only a console-logged failed
 * save (the backend errors on an id it can't find in the new vault), not
 * silent data corruption. Exposing a way to await it isn't possible within
 * this ticket's constraints (`defineExpose` is scrollToHeading-only, per
 * spec.md#imperative-escape-hatches); left as a documented gap.
 */
async function handleChangeVaultFolderClick() {
  let path: string | null;
  try {
    path = await pickVaultFolder();
  } catch (err) {
    await messageDialog(String(err));
    return;
  }
  if (!path) return; // user cancelled

  // `pagesState.reset()` clears `openPage` -- the article island (ticket 07)
  // reacts to that on its own, so there's no DOM to clear here anymore.
  pagesState.reset();

  try {
    closeSettingsModal();
    await openVault(path);
  } catch (err) {
    await messageDialog(friendlyVaultOpenError(String(err)));
  }
}

/**
 * Ticket 04's minimal/raw "connect with an access token" submit handler:
 * disables the form while the backend runs its test fetch, then reports
 * success/failure inline. A failed connect (wrong/expired token, unreachable
 * remote) leaves the fields filled in so the user can just fix the token and
 * resubmit, rather than clearing the form on error.
 */
async function handleConnectFormSubmit(event: SubmitEvent) {
  event.preventDefault();
  if (!settingsConnectUrlEl || !settingsConnectUsernameEl || !settingsConnectTokenEl) return;

  const remoteUrl = settingsConnectUrlEl.value.trim();
  const username = settingsConnectUsernameEl.value.trim();
  const token = settingsConnectTokenEl.value;

  settingsConnectButtonEl?.setAttribute("disabled", "");
  if (settingsConnectStatusEl) {
    settingsConnectStatusEl.textContent = "Connecting…";
    settingsConnectStatusEl.removeAttribute("hidden");
  }

  try {
    await withPlaintextFallbackConsent((allow) => connectAccessToken(remoteUrl, username, token, allow));
    if (settingsConnectStatusEl) settingsConnectStatusEl.textContent = "Connected.";
    settingsConnectTokenEl.value = "";
  } catch (err) {
    if (settingsConnectStatusEl) settingsConnectStatusEl.textContent = String(err);
  } finally {
    settingsConnectButtonEl?.removeAttribute("disabled");
  }
}

/**
 * Ticket 06's minimal "Sign in with GitHub" submit handler: requests a
 * device code, shows it to the user, then polls on GitHub's own reported
 * interval until it succeeds, is denied, or expires. On success, checks
 * whether the app is installed on the given repo before finishing the
 * connect -- if not, shows the install CTA and waits for the user to click
 * "I've installed it, continue" (`handleGithubInstallContinueClick`) rather
 * than looping the check itself.
 */
async function handleGithubFormSubmit(event: SubmitEvent) {
  event.preventDefault();
  if (!settingsGithubUrlEl) return;
  const remoteUrl = settingsGithubUrlEl.value.trim();
  if (!remoteUrl) return;

  settingsGithubButtonEl?.setAttribute("disabled", "");
  settingsGithubInstallEl?.setAttribute("hidden", "");
  settingsGithubDeviceCodeEl?.setAttribute("hidden", "");
  pendingGithubTokenPair = null;

  const setStatus = (text: string) => {
    if (!settingsGithubStatusEl) return;
    settingsGithubStatusEl.textContent = text;
    settingsGithubStatusEl.removeAttribute("hidden");
  };

  try {
    setStatus("Requesting a device code from GitHub…");
    const device = await startGithubDeviceFlow();

    if (settingsGithubVerificationLinkEl) {
      settingsGithubVerificationLinkEl.href = device.verificationUri;
      settingsGithubVerificationLinkEl.textContent = device.verificationUri;
    }
    if (settingsGithubUserCodeEl) settingsGithubUserCodeEl.textContent = device.userCode;
    settingsGithubDeviceCodeEl?.removeAttribute("hidden");
    setStatus("Waiting for you to approve in the browser…");

    const deadline = Date.now() + device.expiresInSecs * 1000;
    let intervalMs = Math.max(device.intervalSecs, 1) * 1000;

    // Frontend-owned poll loop (same "poll on a timer" shape as sync
    // status): each iteration waits `intervalMs`, then polls once, and
    // reacts to GitHub's device-flow outcome -- see `DevicePollResult`.
    for (;;) {
      if (Date.now() >= deadline) throw new Error("The GitHub sign-in code expired before it was confirmed.");
      await new Promise((resolve) => setTimeout(resolve, intervalMs));

      const result = await pollGithubDeviceFlow(device.deviceCode);
      if (result.outcome === "pending") continue;
      if (result.outcome === "slowDown") {
        intervalMs += 5000;
        continue;
      }
      if (result.outcome === "denied") throw new Error("GitHub sign-in was denied.");
      if (result.outcome === "expired") throw new Error("The GitHub sign-in code expired before it was confirmed.");
      if (result.outcome === "error") throw new Error(result.message);

      pendingGithubTokenPair = {
        accessToken: result.accessToken,
        refreshToken: result.refreshToken,
        accessTokenExpiresAt: result.accessTokenExpiresAt,
      };
      break;
    }

    settingsGithubDeviceCodeEl?.setAttribute("hidden", "");
    await finishGithubConnect(remoteUrl);
  } catch (err) {
    settingsGithubDeviceCodeEl?.setAttribute("hidden", "");
    setStatus(String(err));
  } finally {
    settingsGithubButtonEl?.removeAttribute("disabled");
  }
}

/**
 * Shared tail of the GitHub sign-in flow, called once a token pair is in
 * hand (`pendingGithubTokenPair`): checks the app's installation on
 * `remoteUrl`'s repo and either shows the install CTA or finishes
 * `connectGithubOauth`'s real test-fetch-then-persist gate. Also the retry
 * path `handleGithubInstallContinueClick` calls after the user says
 * they've installed the app.
 */
async function finishGithubConnect(remoteUrl: string) {
  if (!pendingGithubTokenPair || !settingsGithubStatusEl) return;
  const { accessToken, refreshToken, accessTokenExpiresAt } = pendingGithubTokenPair;

  settingsGithubStatusEl.textContent = "Checking whether Cerebrite is installed on this repository…";
  settingsGithubStatusEl.removeAttribute("hidden");

  const installation = await checkGithubInstallation(remoteUrl, accessToken);
  if (installation.status === "notInstalled") {
    if (settingsGithubInstallLinkEl) settingsGithubInstallLinkEl.href = installation.installUrl;
    settingsGithubInstallEl?.removeAttribute("hidden");
    settingsGithubStatusEl.textContent = "";
    settingsGithubStatusEl.setAttribute("hidden", "");
    return;
  }

  settingsGithubInstallEl?.setAttribute("hidden", "");
  settingsGithubStatusEl.textContent = "Connecting…";
  const result = await withPlaintextFallbackConsent((allow) =>
    connectGithubOauth(remoteUrl, accessToken, refreshToken, accessTokenExpiresAt, allow),
  );
  settingsGithubStatusEl.textContent = "Connected.";
  pendingGithubTokenPair = null;
  await maybeOfferProviderCommitAuthorSwitch(result.providerSuggestedAuthor);
}

async function handleGithubInstallContinueClick() {
  if (!settingsGithubUrlEl) return;
  const remoteUrl = settingsGithubUrlEl.value.trim();
  settingsGithubInstallContinueButtonEl?.setAttribute("disabled", "");
  try {
    await finishGithubConnect(remoteUrl);
  } catch (err) {
    if (settingsGithubStatusEl) {
      settingsGithubStatusEl.textContent = String(err);
      settingsGithubStatusEl.removeAttribute("hidden");
    }
  } finally {
    settingsGithubInstallContinueButtonEl?.removeAttribute("disabled");
  }
}

/**
 * Ticket 07's minimal "Sign in with GitLab" submit handler -- the same
 * device-flow shape `handleGithubFormSubmit` uses, but there is no
 * install-CTA detour: once a token pair is acquired, it goes straight to
 * `connectGitlabOauth`'s real test-fetch-then-persist gate (a GitLab OAuth
 * application reaches every repo the authorizing user can, unlike a GitHub
 * App). `refreshToken` may come back `undefined` -- ticket 07's defensive
 * dual path for GitLab's still-unverified device-grant response shape --
 * and is passed through to `connectGitlabOauth` as-is rather than treated
 * as an error.
 */
async function handleGitlabFormSubmit(event: SubmitEvent) {
  event.preventDefault();
  if (!settingsGitlabUrlEl) return;
  const remoteUrl = settingsGitlabUrlEl.value.trim();
  if (!remoteUrl) return;

  settingsGitlabButtonEl?.setAttribute("disabled", "");
  settingsGitlabDeviceCodeEl?.setAttribute("hidden", "");

  const setStatus = (text: string) => {
    if (!settingsGitlabStatusEl) return;
    settingsGitlabStatusEl.textContent = text;
    settingsGitlabStatusEl.removeAttribute("hidden");
  };

  try {
    setStatus("Requesting a device code from GitLab…");
    const device = await startGitlabDeviceFlow();

    if (settingsGitlabVerificationLinkEl) {
      settingsGitlabVerificationLinkEl.href = device.verificationUri;
      settingsGitlabVerificationLinkEl.textContent = device.verificationUri;
    }
    if (settingsGitlabUserCodeEl) settingsGitlabUserCodeEl.textContent = device.userCode;
    settingsGitlabDeviceCodeEl?.removeAttribute("hidden");
    setStatus("Waiting for you to approve in the browser…");

    const deadline = Date.now() + device.expiresInSecs * 1000;
    let intervalMs = Math.max(device.intervalSecs, 1) * 1000;

    let accessToken: string;
    let refreshToken: string | undefined;
    let accessTokenExpiresAt: string;

    // Frontend-owned poll loop, identical shape to `handleGithubFormSubmit`'s.
    for (;;) {
      if (Date.now() >= deadline) throw new Error("The GitLab sign-in code expired before it was confirmed.");
      await new Promise((resolve) => setTimeout(resolve, intervalMs));

      const result = await pollGitlabDeviceFlow(device.deviceCode);
      if (result.outcome === "pending") continue;
      if (result.outcome === "slowDown") {
        intervalMs += 5000;
        continue;
      }
      if (result.outcome === "denied") throw new Error("GitLab sign-in was denied.");
      if (result.outcome === "expired") throw new Error("The GitLab sign-in code expired before it was confirmed.");
      if (result.outcome === "error") throw new Error(result.message);

      accessToken = result.accessToken;
      refreshToken = result.refreshToken;
      accessTokenExpiresAt = result.accessTokenExpiresAt;
      break;
    }

    settingsGitlabDeviceCodeEl?.setAttribute("hidden", "");
    setStatus("Connecting…");
    const connectResult = await withPlaintextFallbackConsent((allow) =>
      connectGitlabOauth(remoteUrl, accessToken, refreshToken, accessTokenExpiresAt, allow),
    );
    setStatus("Connected.");
    await maybeOfferProviderCommitAuthorSwitch(connectResult.providerSuggestedAuthor);
  } catch (err) {
    settingsGitlabDeviceCodeEl?.setAttribute("hidden", "");
    setStatus(String(err));
  } finally {
    settingsGitlabButtonEl?.removeAttribute("disabled");
  }
}

// --- Ticket 11: the always-visible "Sync" section's credential-kind toggle
// and its new raw SSH key sub-form ------------------------------------------
//
// The access-token/GitHub/GitLab sub-forms (tickets 04/06/07) are reused
// completely unchanged -- only the SSH key sub-form (mirroring ticket 05's
// generate/import/connect commands, the same way the guided wizard's own SSH
// step already does) is new here.

/** Shows the sub-form matching `#sync-credential-kind`'s current value, hides the other three -- called on `change` and once at startup. */
function updateSyncSubformVisibility() {
  const kind = syncCredentialKindEl?.value ?? "accessToken";
  document.querySelectorAll<HTMLElement>("#sync-section .sync-subform").forEach((subform) => {
    subform.hidden = subform.dataset.syncKind !== kind;
  });
}

// --- Settings' still-vanilla "Sync" section ---------------------------------
//
// The sync indicator/popup themselves are `surfaces/sync-indicator/` (a
// Vue island, see its own components for the `syncIndicatorFor` mapping and
// the click-to-open popup). This section is what's left in main.ts: the
// always-visible "Sync" section's not-connected/connected banners and its
// raw sub-forms, none of which have migrated yet.

/** Ticket 02: the Settings "Sync" section's not-connected/connected banners
 * now read `state/sync.ts` directly instead of being written by the old
 * `applySyncIndicator` -- driven by the `watch` below, so it can't go stale
 * while the modal happens to be open. */
function updateSyncSectionNotConnectedBanner() {
  if (!syncSectionNotConnectedEl) return;
  syncSectionNotConnectedEl.hidden = syncStatus.value.state !== "noRemote";
  // Ticket 14: the "Disconnect…" button's own banner is the exact inverse --
  // shown whenever this vault has *some* connection configured, regardless
  // of whether that connection is currently healthy (a needs-attention
  // connection is still one Disconnect should be able to tear down).
  if (syncSectionConnectedEl) syncSectionConnectedEl.hidden = syncStatus.value.state === "noRemote";
}
watch(syncStatus, updateSyncSectionNotConnectedBanner, { immediate: true });

/** Maps a connection's stored `credentialKind` (+ `provider`, for
 * `oauth_sign_in`, which doesn't say by itself whether it's GitHub's or
 * GitLab's device flow) to the always-visible "Sync" section's
 * `data-sync-kind` sub-form values (`index.html`'s `#sync-credential-kind`
 * options). Defaults to `"accessToken"` when nothing is known yet (no
 * connection configured) -- the same default `updateSyncSubformVisibility`
 * already falls back to. */
function syncSubformKindFor(credentialKind: CredentialKind | null, provider: string | null): string {
  switch (credentialKind) {
    case "ssh_key":
      return "sshKey";
    case "oauth_sign_in":
      return provider === "GitLab" ? "gitlabOauth" : "githubOauth";
    case "access_token":
    case null:
      return "accessToken";
  }
}

/** Code-review follow-up (ticket 02/04): wraps a `connect*` call so the
 * backend's `PLAINTEXT_CONSENT_REQUIRED_ERROR` rejection (no keychain
 * reachable, and the caller hadn't consented to plaintext storage yet) turns
 * into the same "store as plaintext instead?" consent dialog ticket 13's
 * reconnect path already uses (`handleStoreAsPlaintextClick`) -- reused here
 * for the *initial* connect path, which never had one before. `attempt` is
 * always first called with `allowPlaintextFallback: false`; it's retried
 * with `true` only if that specific rejection comes back and the user
 * confirms. Any other rejection (wrong token, unreachable remote, rejected
 * SSH host key, ...) passes straight through unchanged. */
async function withPlaintextFallbackConsent<T>(attempt: (allowPlaintextFallback: boolean) => Promise<T>): Promise<T> {
  try {
    return await attempt(false);
  } catch (err) {
    if (String(err) !== PLAINTEXT_CONSENT_REQUIRED_ERROR) throw err;
    const confirmed = confirmBrowser(
      "No keychain is available on this device. Store this connection's credential as a plaintext " +
        "file instead? This is less secure than the keychain, and should only be used when no keychain " +
        "is available.",
    );
    if (!confirmed) throw err;
    return attempt(true);
  }
}

/** Holds the generated/imported key between "Generate"/"Import" and "Connect" -- mirrors `wizardSshKey`. */
let settingsSshKey: SshKeyInfo | null = null;

async function handleSettingsSshKeyGenerateClick() {
  if (!settingsSshKeyStatusEl) return;
  settingsSshKeyStatusEl.textContent = "Generating…";
  settingsSshKeyStatusEl.removeAttribute("hidden");
  try {
    settingsSshKey = await generateSshKey();
    settingsSshKeyStatusEl.textContent =
      `Key ready (fingerprint ${settingsSshKey.fingerprintSha256}). Add the public key to your provider, then Connect.`;
  } catch (err) {
    settingsSshKeyStatusEl.textContent = String(err);
  }
}

/** Same `window.prompt`-based import as the guided wizard's `handleWizardImportSshKey`. */
async function handleSettingsSshKeyImportClick() {
  if (!settingsSshKeyStatusEl) return;
  const privateKeyOpenssh = promptDialog("Paste the private key (OpenSSH format):");
  if (privateKeyOpenssh === null || !privateKeyOpenssh.trim()) return;
  const passphrase = promptDialog("Passphrase (leave blank if none):") ?? undefined;

  settingsSshKeyStatusEl.textContent = "Importing…";
  settingsSshKeyStatusEl.removeAttribute("hidden");
  try {
    settingsSshKey = await importSshKey(privateKeyOpenssh, passphrase || undefined);
    settingsSshKeyStatusEl.textContent =
      `Key imported (fingerprint ${settingsSshKey.fingerprintSha256}). Add the public key to your provider, then Connect.`;
  } catch (err) {
    settingsSshKeyStatusEl.textContent = String(err);
  }
}

/** Calls `connectSshKey` directly -- the exact same command/test-fetch-then-persist gate ticket 05 and the guided wizard's SSH step already use. */
async function handleSettingsSshKeyConnectClick() {
  if (!settingsSshKeyUrlEl || !settingsSshKeyStatusEl) return;
  const remoteUrl = settingsSshKeyUrlEl.value.trim();
  if (!remoteUrl) {
    settingsSshKeyStatusEl.textContent = "Enter the repository's URL first.";
    settingsSshKeyStatusEl.removeAttribute("hidden");
    return;
  }
  if (!settingsSshKey) {
    settingsSshKeyStatusEl.textContent = "Generate (or import) a key first.";
    settingsSshKeyStatusEl.removeAttribute("hidden");
    return;
  }

  settingsSshKeyConnectButtonEl?.setAttribute("disabled", "");
  settingsSshKeyStatusEl.textContent = "Connecting…";
  settingsSshKeyStatusEl.removeAttribute("hidden");
  try {
    await withPlaintextFallbackConsent((allow) =>
      connectSshKey(remoteUrl, settingsSshKey!.privateKeyOpenssh, settingsSshKey!.passphrase, allow),
    );
    settingsSshKeyStatusEl.textContent = "Connected.";
  } catch (err) {
    settingsSshKeyStatusEl.textContent = String(err);
  } finally {
    settingsSshKeyConnectButtonEl?.removeAttribute("disabled");
  }
}

/**
 * Ticket 02: the vanilla side of `state/ui.ts`'s `openSettings` deep-link
 * action, passed to the sync popup island as its `openSettingsInVanilla`
 * temporary callback root prop (Settings itself hasn't migrated to Vue yet).
 * Replaces the old `openSyncManualForm`/`openQuickReconnect`, which wrote
 * into Settings' DOM directly instead of going through a shared action.
 * Prefills every sub-form's own repository URL field with `prefillUrl`
 * (whatever's already known, if anything) so the user doesn't have to
 * retype a URL they already entered, and preselects the credential-kind
 * sub-form when `credentialKind` is known. Per ticket 11's original
 * decision, re-asking for the credential itself is an accepted limitation.
 */
function openSettingsInVanilla(options: OpenSettingsOptions): void {
  openSettingsModal();
  if (options.section !== "sync") return;
  const remoteUrl = options.prefillUrl ?? "";
  if (remoteUrl) {
    if (settingsConnectUrlEl) settingsConnectUrlEl.value = remoteUrl;
    if (settingsSshKeyUrlEl) settingsSshKeyUrlEl.value = remoteUrl;
    if (settingsGithubUrlEl) settingsGithubUrlEl.value = remoteUrl;
    if (settingsGitlabUrlEl) settingsGitlabUrlEl.value = remoteUrl;
  }
  if (options.credentialKind !== undefined && syncCredentialKindEl) {
    syncCredentialKindEl.value = syncSubformKindFor(options.credentialKind, options.provider ?? null);
    updateSyncSubformVisibility();
  }
  document.getElementById("sync-section")?.scrollIntoView({ block: "start" });
}

function openSettingsModal() {
  if (!settingsModalOverlayEl) return;
  settingsModalOverlayEl.removeAttribute("hidden");
  void refreshCommitAuthorFields();
  void refreshOrphanNotice();
}

// --- Ticket 14: disconnect & credential revocation --------------------------
//
// "Disconnect" deletes the three on-disk artifacts a connection leaves
// behind (backend: `provider_disconnect::disconnect_connection`) and, for a
// GitLab sign-in, attempts to revoke the refresh token at GitLab itself. The
// dialog copy built here is the one place ticket 14's core trust constraint
// actually has to be honored: never say "revoked" unless
// `outcome.gitlabRevoked === true`, and for every other credential kind,
// only ever "removed locally, revoke it yourself at: <link>".

/** The host-specific "revoke it yourself" link for an access-token or SSH-key
 * connection -- `null` for a host this app doesn't recognize (the dialog
 * falls back to plain guidance text with no link, per the ticket). */
function revocationLink(provider: Provider, kind: "token" | "sshKey"): string | null {
  if (provider.kind === "git_hub") {
    return kind === "token" ? "https://github.com/settings/tokens" : "https://github.com/settings/keys";
  }
  if (provider.kind === "git_lab") {
    return kind === "token"
      ? "https://gitlab.com/-/user_settings/personal_access_tokens"
      : "https://gitlab.com/-/user_settings/ssh_keys";
  }
  return null;
}

/** GitHub's Authorized/Installed GitHub Apps management page -- where a user
 * reviews or revokes Cerebrite's GitHub App installation, since revoking it
 * directly needs a client secret Cerebrite deliberately doesn't hold (that's
 * exactly why this app signs in via GitHub's device-flow-for-Apps instead of
 * embedding one -- see `src-tauri/src/github_oauth.rs`'s module doc
 * comment). */
const GITHUB_INSTALLATIONS_URL = "https://github.com/settings/installations";

/** GitLab's own "Authorized applications" settings page -- offered as the
 * manual fallback whenever this app's own `/oauth/revoke` call couldn't be
 * confirmed to have succeeded. */
const GITLAB_AUTHORIZED_APPS_URL = "https://gitlab.com/-/profile/applications";

/** Builds the Disconnect result message honestly, per credential kind --
 * this is the one function in the whole feature this ticket's trust
 * constraint lives or dies by. */
function disconnectResultMessage(outcome: DisconnectOutcome): string {
  if (outcome.credentialKind === "oauth_sign_in" && outcome.provider.kind === "git_lab") {
    if (outcome.gitlabRevoked === true) {
      return "Disconnected. Cerebrite revoked your GitLab sign-in token.";
    }
    return (
      "Disconnected locally. Cerebrite could not confirm your GitLab sign-in token was " +
      `revoked -- it may still be valid at GitLab. Revoke it yourself at: ${GITLAB_AUTHORIZED_APPS_URL}`
    );
  }
  if (outcome.credentialKind === "oauth_sign_in" && outcome.provider.kind === "git_hub") {
    return (
      "Disconnected locally. GitHub App revocation needs a client secret Cerebrite doesn't " +
      `hold. Review or revoke Cerebrite's access yourself at: ${GITHUB_INSTALLATIONS_URL}`
    );
  }
  const linkKind = outcome.credentialKind === "ssh_key" ? "sshKey" : "token";
  const link = revocationLink(outcome.provider, linkKind);
  if (link) {
    return `Removed locally -- revoke it yourself at: ${link}`;
  }
  const what = outcome.credentialKind === "ssh_key" ? "SSH key" : "access token";
  return `Removed locally -- revoke this ${what} yourself at your git host's settings.`;
}

/** "Disconnect…" in Settings' Sync section (ticket 14 checklist item 1):
 * confirms first (this codebase's `window.confirm` precedent, tickets
 * 09-13), then deletes the stored secret, connection record, and origin
 * remote, then shows the honest, per-kind result via `disconnectResultMessage`. */
async function handleDisconnectClick() {
  const confirmed = confirmBrowser(
    "Disconnect this vault from its remote repository? Cerebrite deletes the stored " +
      "credential and connection record from this device. This can't be undone from within " +
      "Cerebrite.",
  );
  if (!confirmed) return;

  try {
    const outcome = await disconnectVault();
    alertBrowser(outcome ? disconnectResultMessage(outcome) : "Nothing was connected.");
  } catch (err) {
    alertBrowser(`Couldn't disconnect: ${err}`);
  } finally {
    await refreshSyncStatus();
    updateSyncSectionNotConnectedBanner();
  }
}

/** Ticket 14 checklist item 6: refreshes Settings' orphaned-credential
 * notice -- called whenever Settings opens (a startup dialog isn't required
 * per the ticket; this minimal in-Settings banner is). Silently does
 * nothing on failure (e.g. no app config dir resolvable) rather than
 * blocking the rest of Settings from opening. */
async function refreshOrphanNotice() {
  if (!settingsOrphanNoticeEl) return;
  try {
    const orphans = await scanOrphanedConnections();
    settingsOrphanNoticeEl.hidden = orphans.length === 0;
  } catch {
    settingsOrphanNoticeEl.hidden = true;
  }
}

async function handleOrphanCleanupClick() {
  settingsOrphanCleanupButtonEl?.setAttribute("disabled", "");
  try {
    await cleanupOrphanedConnections();
  } catch (err) {
    alertBrowser(`Couldn't clean up orphaned credentials: ${err}`);
  } finally {
    settingsOrphanCleanupButtonEl?.removeAttribute("disabled");
    await refreshOrphanNotice();
  }
}

/** Ticket 14 checklist item 7: "Remove all stored Cerebrite credentials" --
 * confirms, then disconnects every connection `settings.json` knows about
 * (not just this vault's), best-effort. Reports which ones (if any) failed
 * rather than silently swallowing a partial failure. */
async function handleRemoveAllCredentialsClick() {
  const confirmed = confirmBrowser(
    "Remove all stored Cerebrite credentials? This permanently deletes every credential " +
      "Cerebrite has stored on this device, for every vault it has ever connected -- not just " +
      "this one. This does not revoke anything at the provider, and can't be undone from " +
      "within Cerebrite.",
  );
  if (!confirmed) return;

  settingsRemoveAllCredentialsButtonEl?.setAttribute("disabled", "");
  if (settingsRemoveAllCredentialsStatusEl) settingsRemoveAllCredentialsStatusEl.hidden = true;
  try {
    const failed = await removeAllStoredCredentials();
    if (settingsRemoveAllCredentialsStatusEl) {
      settingsRemoveAllCredentialsStatusEl.textContent =
        failed.length === 0
          ? "All stored credentials were removed."
          : `Removed what it could -- ${failed.length} connection(s) could not be fully removed.`;
      settingsRemoveAllCredentialsStatusEl.hidden = false;
    }
  } catch (err) {
    if (settingsRemoveAllCredentialsStatusEl) {
      settingsRemoveAllCredentialsStatusEl.textContent = String(err);
      settingsRemoveAllCredentialsStatusEl.hidden = false;
    }
  } finally {
    settingsRemoveAllCredentialsButtonEl?.removeAttribute("disabled");
    await refreshOrphanNotice();
    await refreshSyncStatus();
    updateSyncSectionNotConnectedBanner();
  }
}

/**
 * Ticket 08 checklist item 6/8: fills the Settings "Commit as" fields with
 * the vault's currently confirmed author, or -- if nothing has been
 * confirmed yet -- the best available prefill (repo-local -> global ->
 * empty; the provider tier only ever applies during an OAuth connect, see
 * `maybeOfferProviderCommitAuthorSwitch`). Silently does nothing if no
 * vault is open (the commands themselves require one).
 */
async function refreshCommitAuthorFields() {
  if (!settingsCommitAuthorNameEl || !settingsCommitAuthorEmailEl) return;
  try {
    const confirmed = await getCommitAuthor();
    const author = confirmed ?? (await commitAuthorPrefill()).author;
    settingsCommitAuthorNameEl.value = author?.name ?? "";
    settingsCommitAuthorEmailEl.value = author?.email ?? "";
  } catch {
    // No vault open yet -- leave the fields blank rather than erroring the
    // whole Settings modal open.
  }
}

/**
 * Ticket 08 checklist item 4/7: validates and writes the "Commit as" name
 * and email to the vault's repo-local git config. A hard validation
 * failure (empty name, malformed email) is shown as an error; an
 * unrealistic-but-well-formed domain (`.local`, `localhost`, no dot) is
 * shown as a warning alongside the "Saved." confirmation -- never blocked.
 */
async function handleCommitAuthorFormSubmit(event: SubmitEvent) {
  event.preventDefault();
  if (!settingsCommitAuthorNameEl || !settingsCommitAuthorEmailEl || !settingsCommitAuthorStatusEl) return;
  const name = settingsCommitAuthorNameEl.value.trim();
  const email = settingsCommitAuthorEmailEl.value.trim();

  settingsCommitAuthorButtonEl?.setAttribute("disabled", "");
  try {
    const result = await confirmCommitAuthor(name, email);
    settingsCommitAuthorStatusEl.textContent = result.warning ? `Saved. ${result.warning}` : "Saved.";
    settingsCommitAuthorStatusEl.removeAttribute("hidden");
  } catch (err) {
    settingsCommitAuthorStatusEl.textContent = String(err);
    settingsCommitAuthorStatusEl.removeAttribute("hidden");
  } finally {
    settingsCommitAuthorButtonEl?.removeAttribute("disabled");
  }
}

/**
 * Ticket 08 checklist item 5: the one-time "switch to the provider's
 * address?" offer after an OAuth sign-in connect, shown only when the vault
 * already had a *different* confirmed author (`suggested` is `undefined`/
 * `null` otherwise -- see `connectGithubOauth`/`connectGitlabOauth`'s doc
 * comments). Default (Cancel, or dismissing the dialog) keeps the current
 * author -- this never switches silently.
 */
async function maybeOfferProviderCommitAuthorSwitch(suggested: CommitAuthor | null | undefined) {
  if (!suggested) return;
  const switchToProvider = await confirmDialog(
    `Commit as ${suggested.name} <${suggested.email}> from now on? This keeps your real email address private.`,
    { title: "Switch commit author?", kind: "info" },
  );
  if (!switchToProvider) return;
  await confirmCommitAuthor(suggested.name, suggested.email);
  await refreshCommitAuthorFields();
}

// --- Ticket 09: guided connect wizard -------------------------------------
//
// A compact modal (layered over the Settings modal, not full-screen -- see
// index.html's `#connect-wizard-overlay`) driven by `connect-wizard.ts`'s
// pure `reduceWizard` state machine. This section owns everything the
// reducer deliberately doesn't: rendering each step's DOM and performing the
// actual `invoke` calls (device-flow polling, repo list/create, the real
// `connect_*` test-fetch-then-persist calls, and finally ticket 08's
// "Commit as" step).
//
// Ephemeral data the reducer doesn't track (access tokens, fetched repo
// lists, generated/imported SSH key material) lives in these module-level
// variables, reset every time the wizard is (re)opened -- mirroring how
// `pendingGithubTokenPair` already holds the raw-form GitHub flow's token
// between steps.

let wizardState: WizardState = initialWizardState;
let wizardAccessToken: string | null = null;
/** Only meaningful for `credentialKind === "oauth"` -- carried alongside `wizardAccessToken` since `connectGithubOauth`/`connectGitlabOauth` need the full token pair, not just the access token, to persist a refreshable connection (tickets 06/07). */
let wizardRefreshToken: string | undefined;
let wizardAccessTokenExpiresAt = "";
let wizardRepos: RepoInfo[] = [];
let wizardSshKey: SshKeyInfo | null = null;
/** Bumped on every open/close so a stale device-flow poll loop from a previous attempt can tell it's been abandoned and stop touching the DOM/state. */
let wizardGeneration = 0;

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function dispatchWizard(action: WizardAction) {
  wizardState = reduceWizard(wizardState, action);
  if (isWizardSwitchedToManual(wizardState)) {
    // Ticket 11: close this wizard's own chrome and open the "Sync" section
    // instead of rendering a step -- reads `wizardState.remoteUrl` before
    // it's reset by the next `openConnectWizard`, same as `closeConnectWizard`
    // below leaves it alone until then.
    handleWizardManualSetupClick();
    return;
  }
  renderWizardStep();
  if (isWizardDone(wizardState)) {
    closeConnectWizard();
  }
}

function openConnectWizard() {
  if (!connectWizardOverlayEl) return;
  wizardGeneration += 1;
  wizardState = { ...initialWizardState };
  wizardAccessToken = null;
  wizardRefreshToken = undefined;
  wizardAccessTokenExpiresAt = "";
  wizardRepos = [];
  wizardSshKey = null;
  wizardPendingAccessTokenConnect = null;
  connectWizardOverlayEl.removeAttribute("hidden");
  renderWizardStep();
}

function closeConnectWizard() {
  wizardGeneration += 1; // invalidates any in-flight poll loop
  connectWizardOverlayEl?.setAttribute("hidden", "");
}

/**
 * Ticket 11's "Switch to manual setup" escape hatch: closes this wizard's
 * own chrome and opens Settings' always-visible "Sync" section instead,
 * carrying over `wizardState.remoteUrl` if the wizard had already committed
 * one (a pasted/created/picked repository URL) -- see `openSettingsInVanilla`'s
 * doc comment for what "carrying over" does and doesn't cover.
 */
function handleWizardManualSetupClick() {
  const remoteUrl = wizardState.remoteUrl ?? "";
  closeConnectWizard();
  openSettings({ section: "sync", prefillUrl: remoteUrl });
}

function providerLabel(provider: WizardProvider | null): string {
  if (provider === "github") return "GitHub";
  if (provider === "gitlab") return "GitLab";
  return "this provider";
}

/** Renders the current step's content into `#connect-wizard-body`, and updates the shared "Back" button's visibility/label. */
function renderWizardStep() {
  if (!connectWizardBodyEl) return;
  connectWizardBodyEl.innerHTML = "";

  const canGoBack = !isBusyStep(wizardState.step) && wizardState.step !== "hasRepo" && wizardState.step !== "done";
  if (connectWizardBackButtonEl) connectWizardBackButtonEl.hidden = !canGoBack;

  switch (wizardState.step) {
    case "hasRepo":
      renderHasRepoStep();
      break;
    case "providerChoice":
      renderProviderChoiceStep();
      break;
    case "credentialChoice":
      renderCredentialChoiceStep();
      break;
    case "otherCredentialKindChoice":
      renderOtherCredentialKindChoiceStep();
      break;
    case "oauthSignIn":
      renderOauthSignInStep();
      break;
    case "repoVisibility":
      renderRepoVisibilityStep();
      break;
    case "repoPicker":
      renderRepoPickerStep();
      break;
    case "pasteUrl":
      renderPasteUrlStep();
      break;
    case "connecting":
      renderConnectingStep();
      break;
    case "connectError":
      renderConnectErrorStep();
      break;
    case "commitAuthor":
      void renderCommitAuthorStep();
      break;
    case "done":
      break;
  }
}

function renderHasRepoStep() {
  if (!connectWizardBodyEl) return;
  connectWizardBodyEl.appendChild(el("p", "wizard-question", "Do you already have a repository?"));
  const row = el("div", "wizard-button-row");

  const yesButton = el("button", undefined, "Yes, I have one");
  yesButton.type = "button";
  yesButton.addEventListener("click", () => dispatchWizard({ type: "chooseHasRepo", hasRepo: true }));

  const noButton = el("button", undefined, "No, create one");
  noButton.type = "button";
  noButton.addEventListener("click", () => dispatchWizard({ type: "chooseHasRepo", hasRepo: false }));

  row.appendChild(yesButton);
  row.appendChild(noButton);
  connectWizardBodyEl.appendChild(row);
}

function renderProviderChoiceStep() {
  if (!connectWizardBodyEl) return;
  connectWizardBodyEl.appendChild(el("p", "wizard-question", "Which provider is the repository on?"));
  const row = el("div", "wizard-button-row");

  const githubButton = el("button", undefined, "GitHub");
  githubButton.type = "button";
  githubButton.addEventListener("click", () => dispatchWizard({ type: "chooseProvider", provider: "github" }));
  row.appendChild(githubButton);

  const gitlabButton = el("button", undefined, "GitLab");
  gitlabButton.type = "button";
  gitlabButton.addEventListener("click", () => dispatchWizard({ type: "chooseProvider", provider: "gitlab" }));
  row.appendChild(gitlabButton);

  // A generic-provider create-repo API doesn't exist (ticket 09's
  // create-new path is GitHub/GitLab only) -- only offered for pick-existing.
  if (wizardState.hasRepo) {
    const otherButton = el("button", undefined, "Another provider");
    otherButton.type = "button";
    otherButton.addEventListener("click", () => dispatchWizard({ type: "chooseProvider", provider: "other" }));
    row.appendChild(otherButton);
  }

  connectWizardBodyEl.appendChild(row);
}

function renderCredentialChoiceStep() {
  if (!connectWizardBodyEl) return;
  const label = providerLabel(wizardState.provider);
  connectWizardBodyEl.appendChild(el("p", "wizard-question", `How do you want to connect to ${label}?`));

  const signInButton = el("button", "wizard-primary-action", `Sign in with ${label}`);
  signInButton.type = "button";
  signInButton.addEventListener("click", () => dispatchWizard({ type: "chooseOauthSignIn" }));
  connectWizardBodyEl.appendChild(signInButton);

  // Ticket 09 checklist item 5: always available, even for GitHub/GitLab --
  // for orgs that block third-party OAuth apps.
  const otherWaysButton = el("button", "wizard-secondary-action", "Other ways to connect");
  otherWaysButton.type = "button";
  otherWaysButton.addEventListener("click", () => dispatchWizard({ type: "chooseOtherWaysToConnect" }));
  connectWizardBodyEl.appendChild(otherWaysButton);
}

function renderOtherCredentialKindChoiceStep() {
  if (!connectWizardBodyEl) return;
  connectWizardBodyEl.appendChild(el("p", "wizard-question", "How do you want to authenticate?"));
  const row = el("div", "wizard-button-row");

  const tokenButton = el("button", undefined, "Access token");
  tokenButton.type = "button";
  tokenButton.addEventListener("click", () =>
    dispatchWizard({ type: "chooseOtherCredentialKind", kind: "accessToken" }),
  );
  row.appendChild(tokenButton);

  const sshButton = el("button", undefined, "SSH key");
  sshButton.type = "button";
  sshButton.addEventListener("click", () => dispatchWizard({ type: "chooseOtherCredentialKind", kind: "sshKey" }));
  row.appendChild(sshButton);

  connectWizardBodyEl.appendChild(row);
}

/** Ticket 06/07's device-flow steps, driven the same way the raw forms already do, but landing on `dispatchWizard` transitions instead of their own local status text. */
function renderOauthSignInStep() {
  if (!connectWizardBodyEl) return;
  const provider = wizardState.provider;
  const statusEl = el("p", "settings-connect-status", `Requesting a device code from ${providerLabel(provider)}…`);
  connectWizardBodyEl.appendChild(statusEl);
  const codeEl = el("div", "settings-connect-status");
  codeEl.hidden = true;
  connectWizardBodyEl.appendChild(codeEl);

  const generation = wizardGeneration;
  void runOauthSignIn(provider, statusEl, codeEl, generation);
}

async function runOauthSignIn(
  provider: WizardProvider | null,
  statusEl: HTMLElement,
  codeEl: HTMLElement,
  generation: number,
) {
  const stale = () => generation !== wizardGeneration;
  try {
    const device = provider === "gitlab" ? await startGitlabDeviceFlow() : await startGithubDeviceFlow();
    if (stale()) return;

    const link = el("a", undefined, device.verificationUri);
    link.href = device.verificationUri;
    link.target = "_blank";
    link.rel = "noopener";
    const codeText = el("p");
    codeText.appendChild(document.createTextNode("Go to "));
    codeText.appendChild(link);
    codeText.appendChild(document.createTextNode(" and enter code: "));
    codeText.appendChild(el("strong", undefined, device.userCode));
    codeEl.innerHTML = "";
    codeEl.appendChild(codeText);
    codeEl.hidden = false;
    statusEl.textContent = "Waiting for you to approve in the browser…";

    const deadline = Date.now() + device.expiresInSecs * 1000;
    let intervalMs = Math.max(device.intervalSecs, 1) * 1000;
    let accessToken: string;
    let refreshToken: string | undefined;
    let accessTokenExpiresAt: string;

    for (;;) {
      if (stale()) return;
      if (Date.now() >= deadline) throw new Error(`The ${providerLabel(provider)} sign-in code expired before it was confirmed.`);
      await new Promise((resolve) => setTimeout(resolve, intervalMs));
      if (stale()) return;

      const result = provider === "gitlab" ? await pollGitlabDeviceFlow(device.deviceCode) : await pollGithubDeviceFlow(device.deviceCode);
      if (result.outcome === "pending") continue;
      if (result.outcome === "slowDown") {
        intervalMs += 5000;
        continue;
      }
      if (result.outcome === "denied") throw new Error(`${providerLabel(provider)} sign-in was denied.`);
      if (result.outcome === "expired") throw new Error(`The ${providerLabel(provider)} sign-in code expired before it was confirmed.`);
      if (result.outcome === "error") throw new Error(result.message);

      accessToken = result.accessToken;
      refreshToken = result.refreshToken;
      accessTokenExpiresAt = result.accessTokenExpiresAt;
      break;
    }

    if (stale()) return;

    // The GitHub App-installation check from ticket 06 is per-repository, so
    // it can't run yet here (no repo has been picked/created); the wizard's
    // later `connecting` step (`performWizardConnect`) runs it right before
    // the real `connect_github_oauth` test fetch instead, once `remoteUrl`
    // is known -- same "install first, connect second" order ticket 06's
    // raw form already uses.
    wizardAccessToken = accessToken;
    wizardRefreshToken = refreshToken;
    wizardAccessTokenExpiresAt = accessTokenExpiresAt;
    codeEl.hidden = true;
    dispatchWizard({ type: "oauthSignInSucceeded", accessToken });
  } catch (err) {
    if (stale()) return;
    statusEl.textContent = String(err);
    codeEl.hidden = true;
  }
}

function renderRepoVisibilityStep() {
  if (!connectWizardBodyEl) return;
  connectWizardBodyEl.appendChild(el("p", "wizard-question", "Name the new repository:"));

  const nameInput = el("input");
  nameInput.type = "text";
  nameInput.placeholder = "my-notes";
  nameInput.value = wizardState.repoName;
  nameInput.addEventListener("input", () => dispatchWizard({ type: "setRepoName", name: nameInput.value }));
  connectWizardBodyEl.appendChild(nameInput);

  const visibilityRow = el("div", "wizard-button-row");
  const privateLabel = el("label");
  const privateRadio = el("input");
  privateRadio.type = "radio";
  privateRadio.name = "wizard-visibility";
  privateRadio.checked = wizardState.visibility === "private";
  privateRadio.addEventListener("change", () => dispatchWizard({ type: "setVisibility", visibility: "private" }));
  privateLabel.appendChild(privateRadio);
  privateLabel.appendChild(document.createTextNode(" Private (recommended)"));
  visibilityRow.appendChild(privateLabel);

  const publicLabel = el("label");
  const publicRadio = el("input");
  publicRadio.type = "radio";
  publicRadio.name = "wizard-visibility";
  publicRadio.checked = wizardState.visibility === "public";
  publicRadio.addEventListener("change", () => dispatchWizard({ type: "setVisibility", visibility: "public" }));
  publicLabel.appendChild(publicRadio);
  publicLabel.appendChild(document.createTextNode(" Public"));
  visibilityRow.appendChild(publicLabel);
  connectWizardBodyEl.appendChild(visibilityRow);

  const createButton = el("button", "wizard-primary-action", "Create repository");
  createButton.type = "button";
  createButton.addEventListener("click", () => void handleCreateRepoClick(createButton));
  connectWizardBodyEl.appendChild(createButton);

  const errorEl = el("p", "error wizard-inline-error");
  errorEl.hidden = true;
  errorEl.id = "wizard-create-repo-error";
  connectWizardBodyEl.appendChild(errorEl);
}

async function handleCreateRepoClick(button: HTMLButtonElement) {
  const name = wizardState.repoName.trim();
  const errorEl = document.getElementById("wizard-create-repo-error");
  const showError = (message: string) => {
    if (errorEl) {
      errorEl.textContent = message;
      errorEl.hidden = false;
    }
  };
  if (!name) {
    showError("Give the repository a name.");
    return;
  }
  if (!wizardAccessToken) {
    showError("Sign-in is required before creating a repository.");
    return;
  }

  button.setAttribute("disabled", "");
  const generation = wizardGeneration;
  try {
    const isPrivate = wizardState.visibility === "private";
    const repo =
      wizardState.provider === "gitlab"
        ? await createGitlabRepository(name, isPrivate, wizardAccessToken)
        : await createGithubRepository(name, isPrivate, wizardAccessToken);
    if (generation !== wizardGeneration) return;
    dispatchWizard({ type: "repoReady", remoteUrl: repo.cloneUrl });
  } catch (err) {
    if (generation !== wizardGeneration) return;
    showError(`Couldn't create the repository: ${String(err)}`);
  } finally {
    if (generation === wizardGeneration) button.removeAttribute("disabled");
  }
}

function renderRepoPickerStep() {
  if (!connectWizardBodyEl) return;
  const statusEl = el("p", "settings-connect-status", "Loading your repositories…");
  connectWizardBodyEl.appendChild(statusEl);
  const listEl = el("ul", "wizard-repo-list");
  connectWizardBodyEl.appendChild(listEl);

  const generation = wizardGeneration;
  void loadRepoPicker(statusEl, listEl, generation);
}

async function loadRepoPicker(statusEl: HTMLElement, listEl: HTMLElement, generation: number) {
  if (!wizardAccessToken) {
    statusEl.textContent = "Sign-in is required before listing repositories.";
    return;
  }
  try {
    wizardRepos =
      wizardState.provider === "gitlab"
        ? await listGitlabRepositories(wizardAccessToken)
        : await listGithubRepositories(wizardAccessToken);
    if (generation !== wizardGeneration) return;

    if (wizardRepos.length === 0) {
      statusEl.textContent = "No repositories found for this account.";
      return;
    }
    statusEl.textContent = "Pick a repository:";
    for (const repo of wizardRepos) {
      const item = el("li");
      const button = el("button", "wizard-repo-item");
      button.type = "button";
      button.appendChild(el("span", "wizard-repo-name", repo.fullName));
      button.appendChild(el("span", "wizard-repo-visibility", repo.private ? "Private" : "Public"));
      button.addEventListener("click", () => dispatchWizard({ type: "repoReady", remoteUrl: repo.cloneUrl }));
      item.appendChild(button);
      listEl.appendChild(item);
    }
  } catch (err) {
    if (generation !== wizardGeneration) return;
    statusEl.textContent = `Couldn't load repositories: ${String(err)}`;
  }
}

function renderPasteUrlStep() {
  if (!connectWizardBodyEl) return;
  connectWizardBodyEl.appendChild(el("p", "wizard-question", "Enter the repository's URL:"));

  const urlInput = el("input");
  urlInput.type = "text";
  urlInput.placeholder = "https://example.com/user/repo.git";
  connectWizardBodyEl.appendChild(urlInput);

  if (wizardState.credentialKind === "accessToken") {
    const usernameInput = el("input");
    usernameInput.type = "text";
    usernameInput.placeholder = "Username";
    connectWizardBodyEl.appendChild(usernameInput);
    const tokenInput = el("input");
    tokenInput.type = "password";
    tokenInput.placeholder = "Access token";
    connectWizardBodyEl.appendChild(tokenInput);

    const errorEl = el("p", "error wizard-inline-error");
    errorEl.hidden = true;

    const connectButton = el("button", "wizard-primary-action", "Connect");
    connectButton.type = "button";
    connectButton.addEventListener("click", () => {
      const remoteUrl = urlInput.value.trim();
      const username = usernameInput.value.trim();
      const token = tokenInput.value;
      if (!remoteUrl || !username || !token) {
        errorEl.textContent = "Repository URL, username, and access token are all required.";
        errorEl.hidden = false;
        return;
      }
      errorEl.hidden = true;
      wizardPendingAccessTokenConnect = { remoteUrl, username, token };
      dispatchWizard({ type: "urlEntered", remoteUrl });
    });
    connectWizardBodyEl.appendChild(connectButton);
    connectWizardBodyEl.appendChild(errorEl);
    return;
  }

  // SSH key path (ticket 05): generate (default) or import, show the public
  // key + fingerprint, then connect.
  const keyStatusEl = el("p", "settings-connect-status");
  connectWizardBodyEl.appendChild(keyStatusEl);

  const generateButton = el("button", undefined, "Generate a new key");
  generateButton.type = "button";
  generateButton.addEventListener("click", () => void handleWizardGenerateSshKey(keyStatusEl));
  connectWizardBodyEl.appendChild(generateButton);

  const importButton = el("button", undefined, "Import an existing key");
  importButton.type = "button";
  importButton.addEventListener("click", () => void handleWizardImportSshKey(keyStatusEl));
  connectWizardBodyEl.appendChild(importButton);

  const connectButton = el("button", "wizard-primary-action", "Connect");
  connectButton.type = "button";
  connectButton.addEventListener("click", () => {
    const remoteUrl = urlInput.value.trim();
    if (!remoteUrl) {
      keyStatusEl.textContent = "Enter the repository's URL first.";
      return;
    }
    if (!wizardSshKey) {
      keyStatusEl.textContent = "Generate (or import) a key first.";
      return;
    }
    dispatchWizard({ type: "urlEntered", remoteUrl });
  });
  connectWizardBodyEl.appendChild(connectButton);
}

/** Holds the pasted-URL access-token form's values between `pasteUrl` and `connecting`, since the reducer only tracks `remoteUrl`. */
let wizardPendingAccessTokenConnect: { remoteUrl: string; username: string; token: string } | null = null;

async function handleWizardGenerateSshKey(statusEl: HTMLElement) {
  statusEl.textContent = "Generating…";
  try {
    wizardSshKey = await generateSshKey();
    statusEl.textContent = `Key ready (fingerprint ${wizardSshKey.fingerprintSha256}). Add the public key to your provider, then Connect.`;
  } catch (err) {
    statusEl.textContent = String(err);
  }
}

/**
 * Ticket 05's import path, offered here via `window.prompt` the same way
 * this app already collects a couple of other simple text values (e.g.
 * the "new page" title prompt, now `PageListContainer.vue`'s `newPage`)
 * rather than a bespoke multi-line
 * form -- validates the key (and passphrase, if given) without persisting
 * anything, same as `handleWizardGenerateSshKey`.
 */
async function handleWizardImportSshKey(statusEl: HTMLElement) {
  const privateKeyOpenssh = promptDialog("Paste the private key (OpenSSH format):");
  if (privateKeyOpenssh === null || !privateKeyOpenssh.trim()) return;
  const passphrase = promptDialog("Passphrase (leave blank if none):") ?? undefined;

  statusEl.textContent = "Importing…";
  try {
    wizardSshKey = await importSshKey(privateKeyOpenssh, passphrase || undefined);
    statusEl.textContent = `Key imported (fingerprint ${wizardSshKey.fingerprintSha256}). Add the public key to your provider, then Connect.`;
  } catch (err) {
    statusEl.textContent = String(err);
  }
}

function renderConnectingStep() {
  if (!connectWizardBodyEl) return;
  connectWizardBodyEl.appendChild(el("p", "settings-connect-status", "Connecting…"));
  const generation = wizardGeneration;
  void performWizardConnect(generation);
}

/**
 * Ticket 09 checklist item 7: the real test fetch every `connect_*` command
 * already runs before persisting anything (`connection::try_connect`) --
 * this just routes to whichever one matches `credentialKind`/`provider` and
 * turns a failure into `connectFailed` (a clear in-wizard error banner) or a
 * success into `connectSucceeded` (-> the "Commit as" step).
 */
async function performWizardConnect(generation: number) {
  const remoteUrl = wizardState.remoteUrl;
  if (!remoteUrl) {
    dispatchWizard({ type: "connectFailed", message: "No repository URL to connect to." });
    return;
  }
  try {
    if (wizardState.credentialKind === "oauth") {
      if (!wizardAccessToken) throw new Error("Sign-in is required before connecting.");
      if (wizardState.provider === "github") {
        const installation = await checkGithubInstallation(remoteUrl, wizardAccessToken);
        if (installation.status === "notInstalled") {
          throw new Error(
            `Cerebrite isn't installed on this repository yet. Install it at ${installation.installUrl}, then try again.`,
          );
        }
        await withPlaintextFallbackConsent((allow) =>
          connectGithubOauth(remoteUrl, wizardAccessToken!, wizardRefreshToken ?? "", wizardAccessTokenExpiresAt, allow),
        );
      } else {
        await withPlaintextFallbackConsent((allow) =>
          connectGitlabOauth(remoteUrl, wizardAccessToken!, wizardRefreshToken, wizardAccessTokenExpiresAt, allow),
        );
      }
    } else if (wizardState.credentialKind === "accessToken") {
      if (!wizardPendingAccessTokenConnect) throw new Error("Missing access token details.");
      await withPlaintextFallbackConsent((allow) =>
        connectAccessToken(
          wizardPendingAccessTokenConnect!.remoteUrl,
          wizardPendingAccessTokenConnect!.username,
          wizardPendingAccessTokenConnect!.token,
          allow,
        ),
      );
    } else if (wizardState.credentialKind === "sshKey") {
      if (!wizardSshKey) throw new Error("Generate or import an SSH key first.");
      await withPlaintextFallbackConsent((allow) =>
        connectSshKey(remoteUrl, wizardSshKey!.privateKeyOpenssh, wizardSshKey!.passphrase, allow),
      );
    } else {
      throw new Error("No connection method was chosen.");
    }
    if (generation !== wizardGeneration) return;
    dispatchWizard({ type: "connectSucceeded" });
  } catch (err) {
    if (generation !== wizardGeneration) return;
    dispatchWizard({ type: "connectFailed", message: `Couldn't connect: ${String(err)}` });
  }
}

function renderConnectErrorStep() {
  if (!connectWizardBodyEl) return;
  connectWizardBodyEl.appendChild(el("p", "error", wizardState.error ?? "Couldn't connect."));
  const retryButton = el("button", "wizard-primary-action", "Try again");
  retryButton.type = "button";
  retryButton.addEventListener("click", () => dispatchWizard({ type: "retry" }));
  connectWizardBodyEl.appendChild(retryButton);
}

/** Ticket 09's last step, reusing ticket 08's exact commands (never a parallel author-writing path). */
async function renderCommitAuthorStep() {
  if (!connectWizardBodyEl) return;
  connectWizardBodyEl.appendChild(el("p", "wizard-question", "Commit as:"));

  const nameInput = el("input");
  nameInput.type = "text";
  nameInput.placeholder = "Name";
  const emailInput = el("input");
  emailInput.type = "text";
  emailInput.placeholder = "Email";

  try {
    const confirmed = await getCommitAuthor();
    const author = confirmed ?? (await commitAuthorPrefill()).author;
    nameInput.value = author?.name ?? "";
    emailInput.value = author?.email ?? "";
  } catch {
    // No vault open -- leave blank; the commands below will surface a clear
    // error if that's somehow still the case by submit time.
  }

  connectWizardBodyEl.appendChild(nameInput);
  connectWizardBodyEl.appendChild(emailInput);

  const errorEl = el("p", "error wizard-inline-error");
  errorEl.hidden = true;

  const saveButton = el("button", "wizard-primary-action", "Finish");
  saveButton.type = "button";
  saveButton.addEventListener("click", () => {
    void (async () => {
      try {
        await confirmCommitAuthor(nameInput.value.trim(), emailInput.value.trim());
        await refreshCommitAuthorFields();
        dispatchWizard({ type: "commitAuthorConfirmed" });
      } catch (err) {
        errorEl.textContent = String(err);
        errorEl.hidden = false;
      }
    })();
  });
  connectWizardBodyEl.appendChild(saveButton);
  connectWizardBodyEl.appendChild(errorEl);
}

function closeSettingsModal() {
  settingsModalOverlayEl?.setAttribute("hidden", "");
}

function isSettingsModalOpen(): boolean {
  return settingsModalOverlayEl ? !settingsModalOverlayEl.hasAttribute("hidden") : false;
}

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
      if (isSettingsModalOpen()) {
        closeSettingsModal();
      } else {
        openSettingsModal();
      }
    } else if (event.key === "Escape" && isSettingsModalOpen()) {
      closeSettingsModal();
    }
  });

  settingsButtonEl?.addEventListener("click", openSettingsModal);
  sidebarRailSettingsButtonEl?.addEventListener("click", openSettingsModal);
  settingsChangeFolderButtonEl?.addEventListener("click", () => void handleChangeVaultFolderClick());
  settingsThemeRadios.forEach((radio) => radio.addEventListener("change", (e) => void handleThemeRadioChange(e)));
  // Ticket 02: the sidebar sync-status icon (footer + rail) and its popup
  // are now the `surfaces/sync-indicator/` Vue island (mounted near the top
  // of this file, at module scope) -- it owns its own click-to-open/Escape/
  // outside-click handling, replacing everything that used to be wired up
  // here.
  syncSectionConnectButtonEl?.addEventListener("click", openConnectWizard);
  syncSectionDisconnectButtonEl?.addEventListener("click", () => void handleDisconnectClick());
  settingsOrphanCleanupButtonEl?.addEventListener("click", () => void handleOrphanCleanupClick());
  settingsRemoveAllCredentialsButtonEl?.addEventListener("click", () => void handleRemoveAllCredentialsClick());
  void refreshSyncStatus();

  connectWizardOpenButtonEl?.addEventListener("click", openConnectWizard);
  connectWizardCloseButtonEl?.addEventListener("click", closeConnectWizard);
  connectWizardBackButtonEl?.addEventListener("click", () => dispatchWizard({ type: "back" }));
  connectWizardManualButtonEl?.addEventListener("click", () => dispatchWizard({ type: "switchToManual" }));
  connectWizardOverlayEl?.addEventListener("click", (event) => {
    if (event.target === connectWizardOverlayEl) closeConnectWizard();
  });
  connectWizardOverlayEl?.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      closeConnectWizard();
    }
  });

  // Ticket 10: the guided clone wizard's and the standalone "git clone"
  // manual dialog's wiring (close/back/switch-to-manual buttons, Escape,
  // backdrop click, and every sub-form control) is now owned by
  // `surfaces/clone-wizard/`/`surfaces/clone-manual-form/`'s own
  // containers/presentational SFCs -- there is nothing left to wire up here.

  settingsConnectFormEl?.addEventListener("submit", (e) => void handleConnectFormSubmit(e));
  settingsGithubFormEl?.addEventListener("submit", (e) => void handleGithubFormSubmit(e));
  settingsGithubInstallContinueButtonEl?.addEventListener("click", () => void handleGithubInstallContinueClick());
  settingsGitlabFormEl?.addEventListener("submit", (e) => void handleGitlabFormSubmit(e));
  settingsCommitAuthorFormEl?.addEventListener("submit", (e) => void handleCommitAuthorFormSubmit(e));

  // Ticket 11: the always-visible "Sync" section's credential-kind toggle
  // and its new raw SSH key sub-form.
  syncCredentialKindEl?.addEventListener("change", updateSyncSubformVisibility);
  updateSyncSubformVisibility();
  settingsSshKeyGenerateButtonEl?.addEventListener("click", () => void handleSettingsSshKeyGenerateClick());
  settingsSshKeyImportButtonEl?.addEventListener("click", () => void handleSettingsSshKeyImportClick());
  settingsSshKeyConnectButtonEl?.addEventListener("click", () => void handleSettingsSshKeyConnectClick());
  // Clicking the dimmed backdrop (not the modal card itself) closes it.
  settingsModalOverlayEl?.addEventListener("click", (event) => {
    if (event.target === settingsModalOverlayEl) closeSettingsModal();
  });

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

  const settings = await getSettings();
  applyTheme(settings.theme);
  setThemeRadioValue(settings.theme);

  // Ticket 14 checklist item 6: startup orphan-credential scan -- app-wide
  // (`settings.json`'s connections index isn't scoped to one vault), so
  // this runs regardless of whether a vault is remembered/opens below. The
  // notice itself only ever surfaces once Settings is opened (minimal UI,
  // per the ticket) -- this just makes sure it's not stale the first time.
  void refreshOrphanNotice();

  // Ticket 04: the remembered-vault auto-open this used to do here (via
  // `openVaultAndLoad`) moved into `VaultPickerContainer`'s own `onMounted`
  // -- it owns every vault-open backend call now, including this one.
}

window.addEventListener("DOMContentLoaded", () => {
  void init();
});
