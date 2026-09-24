import {
  getSettings,
  setTheme,
  pickVaultFolder,
  openVault,
  listPages,
  getPage,
  savePage,
  createPage,
  renamePage,
  resolvePage,
  materializeAndSavePage,
  getBacklinks,
  trashPage,
  restorePage,
  emptyTrash,
  listTrashedPages,
  searchPages,
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
  cloneAndOpenVault,
  getSyncStatus,
  getSyncDetails,
  triggerSyncNow,
  onSyncStatusChanged,
  type CommitAuthor,
  type PageSummary,
  type PageResolution,
  type TrashedPageSummary,
  type SearchResult,
  type Theme,
  type RepoInfo,
  type SshKeyInfo,
  type CloneCredential,
  type CommitAuthorPrefillResult,
  type SyncStatus,
} from "./vault-api";
import { syncIndicatorFor } from "./sync-status";
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
import {
  reduceCloneWizard,
  initialCloneWizardState,
  isCloneWizardDone,
  isCloneWizardSwitchedToManual,
  isCloneWizardBusyStep,
  type CloneWizardState,
  type CloneWizardAction,
} from "./clone-wizard";
// `confirm`/`message` from the dialog plugin, not `window.confirm`/`window.alert`:
// tauri-plugin-dialog's auto-injected webview shim (init-iife.js in the 2.7.3
// crate) overrides those globals to call a `plugin:dialog|confirm` IPC
// command that plugin version never actually registers (only `message` is),
// so `window.confirm()` always rejects with "Command not found" and
// `window.alert()` fires-and-forgets without blocking. The plugin's own JS
// API calls the correct `plugin:dialog|message` command under the hood and
// actually works. `window.prompt()` is untouched by that shim and still
// works natively, so it's used as-is elsewhere in this file.
import { confirm as confirmDialog, message as messageDialog } from "@tauri-apps/plugin-dialog";
import { PageEditor } from "./page-editor";
import { humanizeHeadingSlug } from "./heading-slug";

// Autosave debounce: fires this long after the last edit with no further
// typing, rather than on every keystroke or requiring an explicit "save"
// action -- see issue 03's save-trigger note.
const AUTOSAVE_DEBOUNCE_MS = 1500;

const vaultPickerEl = document.querySelector<HTMLElement>("#vault-picker");
const vaultPickerErrorEl = document.querySelector<HTMLElement>("#vault-picker-error");
const selectVaultButtonEl = document.querySelector<HTMLButtonElement>("#select-vault-button");

// Ticket 10: the guided clone wizard's full-screen DOM handles -- entry
// point lives on the first-run folder-picker screen above.
const cloneWizardOpenButtonEl = document.querySelector<HTMLButtonElement>("#clone-wizard-open-button");
const cloneWizardOverlayEl = document.querySelector<HTMLElement>("#clone-wizard-overlay");
const cloneWizardBodyEl = document.querySelector<HTMLElement>("#clone-wizard-body");
const cloneWizardCloseButtonEl = document.querySelector<HTMLButtonElement>("#clone-wizard-close-button");
const cloneWizardBackButtonEl = document.querySelector<HTMLButtonElement>("#clone-wizard-back-button");
const cloneWizardManualButtonEl = document.querySelector<HTMLButtonElement>("#clone-wizard-manual-button");

// Ticket 11: the standalone "git clone" manual form's DOM handles --
// reachable from the first-run screen (`cloneManualOpenButtonEl`) and from
// the guided clone wizard's own "Switch to manual setup" link.
const cloneManualOpenButtonEl = document.querySelector<HTMLButtonElement>("#clone-manual-open-button");
const cloneManualOverlayEl = document.querySelector<HTMLElement>("#clone-manual-overlay");
const cloneManualCloseButtonEl = document.querySelector<HTMLButtonElement>("#clone-manual-close-button");
const cloneManualUrlEl = document.querySelector<HTMLInputElement>("#clone-manual-url");
const cloneManualDestinationEl = document.querySelector<HTMLInputElement>("#clone-manual-destination");
const cloneManualDestinationButtonEl = document.querySelector<HTMLButtonElement>("#clone-manual-destination-button");
const cloneManualCredentialKindEl = document.querySelector<HTMLSelectElement>("#clone-manual-credential-kind");
const cloneManualTokenUsernameEl = document.querySelector<HTMLInputElement>("#clone-manual-token-username");
const cloneManualTokenValueEl = document.querySelector<HTMLInputElement>("#clone-manual-token-value");
const cloneManualSshKeyGenerateButtonEl = document.querySelector<HTMLButtonElement>(
  "#clone-manual-sshkey-generate-button",
);
const cloneManualSshKeyImportButtonEl = document.querySelector<HTMLButtonElement>(
  "#clone-manual-sshkey-import-button",
);
const cloneManualSshKeyStatusEl = document.querySelector<HTMLElement>("#clone-manual-sshkey-status");
const cloneManualGithubSigninButtonEl = document.querySelector<HTMLButtonElement>(
  "#clone-manual-github-signin-button",
);
const cloneManualGithubDeviceCodeEl = document.querySelector<HTMLElement>("#clone-manual-github-device-code");
const cloneManualGithubStatusEl = document.querySelector<HTMLElement>("#clone-manual-github-status");
const cloneManualGitlabSigninButtonEl = document.querySelector<HTMLButtonElement>(
  "#clone-manual-gitlab-signin-button",
);
const cloneManualGitlabDeviceCodeEl = document.querySelector<HTMLElement>("#clone-manual-gitlab-device-code");
const cloneManualGitlabStatusEl = document.querySelector<HTMLElement>("#clone-manual-gitlab-status");
const cloneManualSubmitButtonEl = document.querySelector<HTMLButtonElement>("#clone-manual-submit-button");
const cloneManualStatusEl = document.querySelector<HTMLElement>("#clone-manual-status");

const workspaceEl = document.querySelector<HTMLElement>("#workspace");
const sidebarEl = document.querySelector<HTMLElement>("#sidebar");
const pageListEl = document.querySelector<HTMLUListElement>("#page-list");
const newPageButtonEl = document.querySelector<HTMLButtonElement>("#new-page-button");
const todayButtonEl = document.querySelector<HTMLButtonElement>("#today-button");
const searchButtonEl = document.querySelector<HTMLButtonElement>("#search-button");
const recentListEl = document.querySelector<HTMLUListElement>("#recent-list");
const recentEmptyEl = document.querySelector<HTMLElement>("#recent-empty");
const sidebarOpenButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-open-button");
const sidebarCloseButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-close-button");
const sidebarCollapseToggleEl = document.querySelector<HTMLButtonElement>("#sidebar-collapse-toggle");
const sidebarOverlayEl = document.querySelector<HTMLElement>("#sidebar-overlay");
const sidebarRailExpandButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-rail-expand");
const sidebarRailSearchButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-rail-search");
const sidebarRailNewPageButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-rail-new-page");
const settingsButtonEl = document.querySelector<HTMLButtonElement>("#settings-button");
const sidebarRailSettingsButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-rail-settings");

// Ticket 12: the sidebar sync-status indicator -- footer icon (expanded
// sidebar/compact drawer) plus the matching collapsed-rail icon, both kept
// in sync by `applySyncIndicator`, and the popup either one opens.
const syncStatusButtonEl = document.querySelector<HTMLButtonElement>("#sync-status-button");
const syncStatusIconEl = document.querySelector<HTMLElement>("#sync-status-icon");
const syncStatusLabelEl = document.querySelector<HTMLElement>("#sync-status-label");
const sidebarRailSyncStatusButtonEl = document.querySelector<HTMLButtonElement>("#sidebar-rail-sync-status");
const sidebarRailSyncStatusIconEl = document.querySelector<HTMLElement>("#sync-status-icon-rail");
const syncPopupEl = document.querySelector<HTMLElement>("#sync-popup");
const syncPopupIconEl = document.querySelector<HTMLElement>("#sync-popup-icon");
const syncPopupStatusTextEl = document.querySelector<HTMLElement>("#sync-popup-status-text");
const syncPopupProviderEl = document.querySelector<HTMLElement>("#sync-popup-provider");
const syncPopupLastSyncedEl = document.querySelector<HTMLElement>("#sync-popup-last-synced");
const syncPopupSyncNowButtonEl = document.querySelector<HTMLButtonElement>("#sync-popup-sync-now-button");
const syncPopupSettingsLinkEl = document.querySelector<HTMLButtonElement>("#sync-popup-settings-link");
const syncSectionNotConnectedEl = document.querySelector<HTMLElement>("#sync-section-not-connected");
const syncSectionConnectButtonEl = document.querySelector<HTMLButtonElement>("#sync-section-connect-button");

const pageViewEmptyEl = document.querySelector<HTMLElement>("#page-view-empty");
const pageArticleEl = document.querySelector<HTMLElement>("#page-article");
const pageTitleEl = document.querySelector<HTMLElement>("#page-title");
const pageBodyEl = document.querySelector<HTMLElement>("#page-body");
const backlinksListEl = document.querySelector<HTMLUListElement>("#backlinks-list");
const backlinksEmptyEl = document.querySelector<HTMLElement>("#backlinks-empty");
const renamePageButtonEl = document.querySelector<HTMLButtonElement>("#rename-page-button");
const deletePageButtonEl = document.querySelector<HTMLButtonElement>("#delete-page-button");
const pageTrashBannerEl = document.querySelector<HTMLElement>("#page-trash-banner");
const restorePageButtonEl = document.querySelector<HTMLButtonElement>("#restore-page-button");
const trashListEl = document.querySelector<HTMLUListElement>("#trash-list");
const emptyTrashButtonEl = document.querySelector<HTMLButtonElement>("#empty-trash-button");

const searchModalOverlayEl = document.querySelector<HTMLElement>("#search-modal-overlay");
const searchInputEl = document.querySelector<HTMLInputElement>("#search-input");
const searchIncludeTrashEl = document.querySelector<HTMLInputElement>("#search-include-trash");
const searchResultsListEl = document.querySelector<HTMLUListElement>("#search-results-list");
const searchEmptyHintEl = document.querySelector<HTMLElement>("#search-empty-hint");

const settingsModalOverlayEl = document.querySelector<HTMLElement>("#settings-modal-overlay");
const settingsVaultPathEl = document.querySelector<HTMLElement>("#settings-vault-path");
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

// The page currently loaded in the editor: either a persisted page (has an
// id/file) or a dynamic page (issue 05 / ADR-0009) -- title-only, no
// backing file until the first write materializes it. A persisted page may
// currently sit in trash (issue 10 / ADR-0010) -- still resolves/renders,
// but flagged so the UI can show an "in trash" indicator and restore prompt
// instead of the ordinary "Delete page" action.
type OpenPage =
  | { kind: "persisted"; id: string; inTrash: boolean; trashedFilename: string | null }
  | { kind: "dynamic"; title: string };

let currentPage: OpenPage | null = null;
let saveTimer: ReturnType<typeof setTimeout> | null = null;
let pageEditor: PageEditor | null = null;
/** The currently open vault's folder path, shown in the Settings modal. */
let currentVaultPath: string | null = null;
/** Ticket 12: the last `SyncStatus` applied to the sidebar icon -- kept so
 * the Settings "Sync" section's not-connected banner can reflect it without
 * a separate fetch every time the modal opens. */
let currentSyncStatus: SyncStatus = { state: "noRemote" };

// --- Recent (issue 12) ---------------------------------------------------
//
// Last-*opened* pages (not last-edited), most-recent-first, capped at
// RECENT_LIMIT, no pagination -- per issue 08's sidebar spec. Deliberately
// in-memory only (module-level array, not persisted to disk/config): there
// is no existing persistence mechanism for this kind of transient UI state
// (the config file only stores the vault path + device id), and the ticket
// doesn't require surviving a restart, so the simplest option -- resetting
// each app launch -- is the pragmatic default here.
const RECENT_LIMIT = 10;

interface RecentEntry {
  /** Dedupe/identity key: `p:<id>` for a persisted page, `d:<normalizedTitle>` for a dynamic one. */
  key: string;
  title: string;
  kind: "persisted" | "dynamic";
  /** Set only for `kind === "persisted"`; used to navigate via `selectPage`. */
  pageId?: string;
}

let recentPages: RecentEntry[] = [];

/** Records a page-open event: moves an existing entry to the top (no duplicate) or inserts a new one, capped at RECENT_LIMIT. */
function recordRecentOpen(entry: RecentEntry) {
  recentPages = recentPages.filter((existing) => existing.key !== entry.key);
  recentPages.unshift(entry);
  if (recentPages.length > RECENT_LIMIT) recentPages.length = RECENT_LIMIT;
  renderRecentList();
}

/** Drops any Recent entries pointing at pages that are no longer valid (trashed or purged), then re-renders. */
function pruneRecentEntries(removedPageIds: Iterable<string>) {
  const removed = new Set(removedPageIds);
  if (removed.size === 0) return;
  const before = recentPages.length;
  recentPages = recentPages.filter((entry) => !(entry.kind === "persisted" && entry.pageId && removed.has(entry.pageId)));
  if (recentPages.length !== before) renderRecentList();
}

function renderRecentList() {
  if (!recentListEl) return;
  recentListEl.innerHTML = "";

  if (recentPages.length === 0) {
    recentEmptyEl?.removeAttribute("hidden");
  } else {
    recentEmptyEl?.setAttribute("hidden", "");
  }

  for (const entry of recentPages) {
    const li = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = entry.title;
    if (entry.kind === "persisted" && entry.pageId) {
      const pageId = entry.pageId;
      button.dataset.pageId = pageId;
      button.addEventListener("click", () => void selectPage(pageId));
    } else {
      button.dataset.dynamicTitle = entry.title;
      button.addEventListener("click", () => void openPageByTitle(entry.title));
    }
    li.appendChild(button);
    recentListEl.appendChild(li);
  }

  highlightActivePage();
}

/**
 * Cancels any pending debounced autosave and immediately saves `markdown`
 * against whichever page is currently open, if not already saved.
 *
 * Branches per ADR-0009: a persisted page just saves normally; a dynamic
 * page materializes first (same mechanics as the explicit "new page"
 * action, issue 04) and then becomes the persisted page from now on.
 */
async function flushSave(markdown: string) {
  if (saveTimer !== null) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  if (!currentPage) return;

  try {
    if (currentPage.kind === "persisted") {
      await savePage(currentPage.id, markdown);
    } else {
      const dynamicKey = `d:${currentPage.title}`;
      const summary = await materializeAndSavePage(currentPage.title, markdown);
      currentPage = { kind: "persisted", id: summary.id, inTrash: false, trashedFilename: null };
      // Keep the Recent entry (if any) pointing at the same page now that it
      // has materialized, rather than leaving a stale dynamic-kind entry.
      recentPages = recentPages.map((entry) =>
        entry.key === dynamicKey
          ? { key: `p:${summary.id}`, title: summary.title, kind: "persisted", pageId: summary.id }
          : entry
      );
      renderRecentList();
      await loadPages();
    }
  } catch (err) {
    console.error("Failed to save page", err);
  }
}

function scheduleAutosave(markdown: string) {
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    void flushSave(markdown);
  }, AUTOSAVE_DEBOUNCE_MS);
}

/** Flushes any pending save for the page currently loaded in the editor before switching away from it. */
async function flushPendingSaveForCurrentPage() {
  if (saveTimer === null || !currentPage || !pageEditor) return;
  const markdown = pageEditor.getMarkdown();
  if (markdown === null) return;
  await flushSave(markdown);
}

function highlightActivePage() {
  const isActiveButton = (btn: HTMLButtonElement) => {
    if (currentPage?.kind === "persisted") return btn.dataset.pageId === currentPage.id;
    if (currentPage?.kind === "dynamic") return btn.dataset.dynamicTitle === currentPage.title;
    return false;
  };
  pageListEl?.querySelectorAll<HTMLButtonElement>("button").forEach((btn) => {
    btn.classList.toggle("active", isActiveButton(btn));
  });
  recentListEl?.querySelectorAll<HTMLButtonElement>("button").forEach((btn) => {
    btn.classList.toggle("active", isActiveButton(btn));
  });
}

/**
 * Ticket 09 checklist item 8: translates ticket 01's `ensure_git_repo`
 * refusal message (`vault.rs`: "'<picked>' is inside an existing git
 * repository rooted at '<root>'. Pick that folder instead of a folder
 * nested inside it.") into copy a non-technical user can act on, rather
 * than showing the raw Rust error string verbatim. This is the only
 * "adoption"/"refusal" case `ensure_git_repo` actually surfaces as an
 * `Err` -- adopting an existing content-bearing repo (README, source
 * files) is a silent success, not an error, so there is nothing to
 * translate for that case. Applied everywhere a vault-open error reaches
 * the user (initial picker, "Change folder…" in Settings, and the
 * remembered-vault auto-open at launch) since that's the only place this
 * particular error can occur -- not inside the connect wizard itself, which
 * only ever operates on an already-open vault.
 */
function friendlyVaultOpenError(raw: string): string {
  const match = raw.match(/is inside an existing git repository rooted at '([^']+)'/);
  if (!match) return raw;
  const root = match[1];
  return (
    `That folder is inside an existing repository. Pick the repository's own top-level folder instead: ` +
    `"${root}".`
  );
}

function showVaultPicker(errorMessage?: string) {
  vaultPickerEl?.removeAttribute("hidden");
  workspaceEl?.setAttribute("hidden", "");
  if (vaultPickerErrorEl) {
    if (errorMessage) {
      vaultPickerErrorEl.textContent = errorMessage;
      vaultPickerErrorEl.removeAttribute("hidden");
    } else {
      vaultPickerErrorEl.setAttribute("hidden", "");
    }
  }
}

function showWorkspace() {
  vaultPickerEl?.setAttribute("hidden", "");
  workspaceEl?.removeAttribute("hidden");
}

function renderPageList(pages: PageSummary[]) {
  if (!pageListEl) return;
  pageListEl.innerHTML = "";

  for (const page of pages) {
    const li = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = page.title;
    button.dataset.pageId = page.id;
    button.classList.toggle("active", currentPage?.kind === "persisted" && page.id === currentPage.id);
    button.addEventListener("click", () => void selectPage(page.id));
    li.appendChild(button);
    pageListEl.appendChild(li);
  }
}

/**
 * Renders the "Backlinks" section always appended at the bottom of a page's
 * rendered content (issue 06): every other page's `[[Link]]` occurrences
 * whose normalized target matches `title`, grouped by source page
 * (most-recently-modified source first, per `get_backlinks`'s ordering),
 * with a plain-text snippet per entry. Renders identically for persisted and
 * dynamic pages -- always present, "No backlinks yet" rather than hidden
 * when empty.
 */
async function renderBacklinks(title: string) {
  if (!backlinksListEl) return;

  const entries = await getBacklinks(title);
  backlinksListEl.innerHTML = "";

  if (entries.length === 0) {
    backlinksEmptyEl?.removeAttribute("hidden");
    return;
  }
  backlinksEmptyEl?.setAttribute("hidden", "");

  let lastSourceId: string | null = null;
  for (const entry of entries) {
    if (entry.sourceId !== lastSourceId) {
      const header = document.createElement("li");
      header.className = "backlink-group-header";
      const headerButton = document.createElement("button");
      headerButton.type = "button";
      headerButton.textContent = entry.sourceTitle;
      headerButton.addEventListener("click", () => void selectPage(entry.sourceId));
      header.appendChild(headerButton);
      backlinksListEl.appendChild(header);
      lastSourceId = entry.sourceId;
    }

    const item = document.createElement("li");
    item.className = "backlink-snippet";
    const snippetButton = document.createElement("button");
    snippetButton.type = "button";
    snippetButton.textContent = entry.snippet;
    snippetButton.addEventListener("click", () => void selectPage(entry.sourceId));
    item.appendChild(snippetButton);

    // Per-entry target-heading label (ticket 07): shown only when the link
    // that produced this entry targeted a heading rather than the page
    // itself. The label is a best-effort reversal of the stored slug (see
    // heading-slug.ts's `humanizeHeadingSlug`) since only the slug, not the
    // original heading text, is persisted.
    if (entry.targetHeadingSlug) {
      const headingLabel = document.createElement("span");
      headingLabel.className = "backlink-heading-label";
      headingLabel.textContent = `→ ${humanizeHeadingSlug(entry.targetHeadingSlug)}`;
      item.appendChild(headingLabel);
    }

    backlinksListEl.appendChild(item);
  }
}

/**
 * Renders the title/body into the article view and (re)loads them into the
 * editor. `headingSlug` (ticket 07), if given, is the target heading of the
 * `[[Page#Heading]]` link that navigated here -- once the fresh content is
 * mounted, the matching heading (if any) is scrolled into view.
 */
async function renderPageArticle(
  title: string,
  body: string,
  headingSlug?: string | null,
  options?: {
    /** Whether the "Delete page" button should be offered at all -- only a persisted, non-trashed page is deletable. */
    deletable?: boolean;
    pageId?: string;
    /** In trash (issue 10 / ADR-0010): renders a banner + inline "Restore" action instead of the "Delete page" button. */
    inTrash?: boolean;
    trashedFilename?: string | null;
  }
) {
  if (pageTitleEl) pageTitleEl.textContent = title;
  pageViewEmptyEl?.setAttribute("hidden", "");
  pageArticleEl?.removeAttribute("hidden");

  const inTrash = options?.inTrash ?? false;
  const trashedFilename = options?.trashedFilename ?? null;
  const deletable = (options?.deletable ?? false) && !inTrash;
  const pageId = options?.pageId ?? null;

  if (renamePageButtonEl) {
    renamePageButtonEl.hidden = !deletable;
    renamePageButtonEl.onclick = deletable && pageId ? () => void handleRenamePageClick(pageId, title) : null;
  }
  if (deletePageButtonEl) {
    deletePageButtonEl.hidden = !deletable;
    deletePageButtonEl.onclick = deletable && pageId ? () => void handleDeletePageClick(pageId) : null;
  }
  if (pageTrashBannerEl) {
    pageTrashBannerEl.hidden = !inTrash;
  }
  if (restorePageButtonEl) {
    restorePageButtonEl.onclick =
      inTrash && trashedFilename ? () => void handleRestoreClick(trashedFilename) : null;
  }

  if (pageBodyEl) {
    if (!pageEditor) {
      // `currentPage`/`scheduleAutosave` are read at callback time (not
      // captured here), so this single instance stays correct across page
      // switches -- including a dynamic page turning into a persisted one
      // mid-session.
      pageEditor = new PageEditor(
        pageBodyEl,
        (markdown) => scheduleAutosave(markdown),
        (linkTitle) => void openPageByTitle(linkTitle)
      );
    }
    await pageEditor.load(body);

    // Click-through-to-heading (ticket 07): a link to a heading that
    // doesn't (yet) exist on the target page is simply a no-op scroll here
    // -- the page itself still opens normally, per the ticket's "behaves
    // like a dynamic-page link at the page level" acceptance criterion.
    if (headingSlug) {
      pageEditor.scrollToHeading(headingSlug);
    }
  }

  // Always appended at the bottom of the page's rendered content (issue 06),
  // for both persisted and dynamic pages, since both render identically
  // (ADR-0009).
  await renderBacklinks(title);
}

/** Opens whatever `resolution` points to: an existing persisted page, or a dynamic (unmaterialized) one. */
async function openResolution(resolution: PageResolution) {
  // Recent (issue 12) tracks last-*opened*, so every navigation here counts
  // as an open -- including re-opening the already-active page (e.g. a
  // different heading target on the same page) -- and moves it to the top
  // rather than duplicating it. Trashed pages are excluded: they're reached
  // only from the Trash list itself, and recording them would leave a dead
  // link behind in Recent once the page is later purged.
  const isTrashedPage = resolution.kind === "persisted" && resolution.inTrash === true;
  if (!isTrashedPage) {
    recordRecentOpen(
      resolution.kind === "persisted"
        ? { key: `p:${resolution.id}`, title: resolution.title, kind: "persisted", pageId: resolution.id }
        : { key: `d:${resolution.normalizedTitle}`, title: resolution.normalizedTitle, kind: "dynamic" }
    );
  }

  const alreadyOpen =
    (resolution.kind === "persisted" && currentPage?.kind === "persisted" && currentPage.id === resolution.id) ||
    (resolution.kind === "dynamic" &&
      currentPage?.kind === "dynamic" &&
      currentPage.title === resolution.normalizedTitle);
  // Even when the page is already open, a heading-targeted link still needs
  // to scroll -- only skip the (re)load, not the scroll.
  if (alreadyOpen) {
    if (resolution.headingSlug) pageEditor?.scrollToHeading(resolution.headingSlug);
    return;
  }

  // Persist any unsaved edit on the page we're leaving before switching.
  await flushPendingSaveForCurrentPage();

  if (resolution.kind === "persisted") {
    const inTrash = resolution.inTrash ?? false;
    const trashedFilename = resolution.trashedFilename ?? null;
    currentPage = { kind: "persisted", id: resolution.id, inTrash, trashedFilename };
    highlightActivePage();
    await renderPageArticle(resolution.title, resolution.body, resolution.headingSlug, {
      deletable: true,
      pageId: resolution.id,
      inTrash,
      trashedFilename,
    });
  } else {
    // Dynamic page (ADR-0009): UI-identical to a persisted page, but merely
    // viewing it must not create a file -- no id, no file, just its
    // normalized title, until the first write materializes it. Per the
    // ticket, a heading-specific dynamic target isn't a thing, so
    // `resolution.headingSlug` is intentionally not passed through here.
    currentPage = { kind: "dynamic", title: resolution.normalizedTitle };
    highlightActivePage();
    await renderPageArticle(resolution.normalizedTitle, "");
  }
}

async function selectPage(id: string) {
  if (currentPage?.kind === "persisted" && currentPage.id === id && !currentPage.inTrash) return;
  const page = await getPage(id);
  await openResolution({ kind: "persisted", id: page.id, title: page.title, body: page.body, html: page.html });
}

/** Navigates to whatever a clicked `[[Link]]` chip targets (issue 05). */
async function openPageByTitle(rawTitle: string) {
  const resolution = await resolvePage(rawTitle);
  await openResolution(resolution);
}

/**
 * Today's date as an ISO-8601 `YYYY-MM-DD` string, per issue 11 / the "Daily
 * note" glossary entry -- built from the *local* calendar date
 * (getFullYear/getMonth/getDate), not `toISOString()`, which reports the UTC
 * date and would land on the wrong day whenever local time is far enough
 * from UTC (e.g. any time after ~4pm PST or before ~2am CEST).
 */
function todaysDateTitle(): string {
  const now = new Date();
  const year = String(now.getFullYear()).padStart(4, "0");
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/**
 * Sidebar "Today" shortcut (issue 11): navigates to today's date-titled page
 * through the exact same `resolve_page` flow a `[[YYYY-MM-DD]]` link chip
 * would use -- no distinct "daily note" code path. If a page with that exact
 * title already exists, it opens normally; otherwise it opens as an ordinary
 * dynamic page (ADR-0009), materializing only on first write.
 */
async function handleTodayClick() {
  await openPageByTitle(todaysDateTitle());
}

async function loadPages() {
  const pages = await listPages();
  renderPageList(pages);
}

/** Renders the sidebar's "Trash" list (issue 10): clicking an entry opens it the same way a `[[Link]]` to it would -- rendered with the "in trash" banner and inline restore action. */
function renderTrashList(pages: TrashedPageSummary[]) {
  if (!trashListEl) return;
  trashListEl.innerHTML = "";

  for (const page of pages) {
    const li = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = page.title;
    button.addEventListener("click", () => void openPageByTitle(page.title));
    li.appendChild(button);
    trashListEl.appendChild(li);
  }
}

async function loadTrash() {
  const pages = await listTrashedPages();
  renderTrashList(pages);
}

/**
 * Explicit "delete page" action (issue 10 / ADR-0010): moves the page's file
 * into `.cerebrite/trash/` (git-tracked move, auto-committed) and closes the
 * page view, since the page is no longer an ordinary persisted page.
 */
async function handleDeletePageClick(id: string) {
  if (!(await confirmDialog("Move this page to trash?"))) return;

  try {
    await trashPage(id);
    currentPage = null;
    pageArticleEl?.setAttribute("hidden", "");
    pageViewEmptyEl?.removeAttribute("hidden");
    pruneRecentEntries([id]);
    await loadPages();
    await loadTrash();
  } catch (err) {
    await messageDialog(String(err));
  }
}

/**
 * Explicit "rename page" action (the checklist item ticket 04 left undone,
 * see `docs/known-gaps.md`): prompts for a new title pre-filled with the
 * current one, warns first if other pages have inbound links that will be
 * rewritten (a rename's blast radius isn't limited to this one file, unlike
 * every other mutation in this app), then renames.
 *
 * If the currently-open page's content was touched by the rewrite (either
 * because it's the page being renamed, or because it was one of the
 * `affectedPageIds`), its title/body are reloaded from the backend and
 * pushed back into the editor -- otherwise a pending autosave on that page
 * would overwrite the just-rewritten file with its stale in-memory content.
 */
async function handleRenamePageClick(id: string, currentTitle: string) {
  const newTitle = window.prompt("New title for this page:", currentTitle);
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
    const result = await renamePage(id, trimmed);
    await loadPages();

    recentPages = recentPages.map((entry) =>
      entry.kind === "persisted" && entry.pageId === id ? { ...entry, title: result.title } : entry
    );
    renderRecentList();

    const currentPageNeedsReload =
      currentPage?.kind === "persisted" &&
      (currentPage.id === id || result.affectedPageIds.includes(currentPage.id));
    if (currentPageNeedsReload && currentPage?.kind === "persisted") {
      const page = await getPage(currentPage.id);
      if (pageTitleEl) pageTitleEl.textContent = page.title;
      await pageEditor?.load(page.body);
      await renderBacklinks(page.title);
    }
  } catch (err) {
    await messageDialog(String(err));
  }
}

/** Explicit "restore" action (issue 10): moves a trashed page's file back to its original path and reopens it as an ordinary persisted page. */
async function handleRestoreClick(trashedFilename: string) {
  try {
    const summary = await restorePage(trashedFilename);
    await loadPages();
    await loadTrash();
    currentPage = null; // force a fresh render so the trash banner/button clear
    await selectPage(summary.id);
  } catch (err) {
    await messageDialog(String(err));
  }
}

/** Explicit "empty trash" action (issue 10): permanently deletes every trashed page. There is no other purge path. */
async function handleEmptyTrashClick() {
  if (!(await confirmDialog("Permanently delete all trashed pages? This cannot be undone."))) return;

  try {
    const purgedIds = (await listTrashedPages()).map((page) => page.id);
    await emptyTrash();
    pruneRecentEntries(purgedIds);
    if (currentPage?.kind === "persisted" && currentPage.inTrash) {
      currentPage = null;
      pageArticleEl?.setAttribute("hidden", "");
      pageViewEmptyEl?.removeAttribute("hidden");
    }
    await loadTrash();
  } catch (err) {
    await messageDialog(String(err));
  }
}

/**
 * Explicit "new page" action (issue 04): prompts for a title, persists a
 * real file immediately via the `create_page` command, refreshes the "All
 * pages" list (alphabetical re-sort happens server-side), and opens the new
 * page straight into the editor.
 */
async function handleNewPageClick() {
  const title = window.prompt("Title for the new page:");
  if (title === null) return; // user cancelled

  const trimmed = title.trim();
  if (!trimmed) {
    await messageDialog("Title cannot be empty.");
    return;
  }

  try {
    const summary = await createPage(trimmed);
    await loadPages();
    await selectPage(summary.id);
  } catch (err) {
    await messageDialog(String(err));
  }
}

async function openVaultAndLoad(path: string) {
  await openVault(path);
  currentVaultPath = path;
  if (settingsVaultPathEl) settingsVaultPathEl.textContent = path;
  showWorkspace();
  await loadPages();
  await loadTrash();
  await refreshSyncStatus();
}

async function handleSelectVaultClick() {
  // `pickVaultFolder` itself can reject -- not just `openVaultAndLoad` below
  // -- on platforms with no folder-picker at all (Android currently has
  // none; see `pick_vault_folder`'s `#[cfg(mobile)]` arm in lib.rs), so both
  // calls need to land on the same error path rather than leaving that
  // rejection unhandled.
  try {
    const path = await pickVaultFolder();
    if (!path) return; // user cancelled
    await openVaultAndLoad(path);
  } catch (err) {
    showVaultPicker(friendlyVaultOpenError(String(err)));
  }
}

// --- Search modal (ticket 13) --------------------------------------------
//
// A single quick-switcher-style overlay (per the referenced prototype spec,
// issue 11): one query against title/tags/body, results in three strict
// tiers, one row per page. Opened via the sidebar's Search button and
// Ctrl/Cmd+K; closed by Escape or clicking outside the modal card.

/** One rendered row in the search results list: either a real search hit, or the trailing "Create page" action. */
type SearchEntry = { kind: "result"; result: SearchResult } | { kind: "create"; query: string };

let searchEntries: SearchEntry[] = [];
let searchSelectedIndex = -1;
let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;
/** Guards against a slow, now-stale search response overwriting a newer one. */
let searchRequestId = 0;

const SEARCH_DEBOUNCE_MS = 120;

/** Splits an FTS5 snippet built with ``/`` markers (search.rs's `body_fts_matches`) into DOM nodes, wrapping the marked span(s) in `<mark>` without ever using `innerHTML` on vault-derived text. */
function renderHighlightedSnippet(container: HTMLElement, snippet: string) {
  const parts = snippet.split("");
  container.appendChild(document.createTextNode(parts[0] ?? ""));
  for (const part of parts.slice(1)) {
    const [marked, ...restParts] = part.split("");
    const mark = document.createElement("mark");
    mark.textContent = marked ?? "";
    container.appendChild(mark);
    container.appendChild(document.createTextNode(restParts.join("")));
  }
}

/** Opens the resolved search result the same way clicking any `[[Link]]` chip or a Trash-list entry would -- `resolve_page` already handles the "in trash" state, so this works identically for a persisted or a trashed hit. */
async function openSearchResult(result: SearchResult) {
  closeSearchModal();
  await openPageByTitle(result.title);
}

/** Empty-state action (ticket 13 point 6): creates the typed query as a brand-new page and opens it straight into the editor, reusing the exact same action as the "New page" button. */
async function handleCreatePageFromSearch(query: string) {
  try {
    const summary = await createPage(query);
    closeSearchModal();
    await loadPages();
    await selectPage(summary.id);
  } catch (err) {
    await messageDialog(String(err));
  }
}

function searchResultRowLabel(result: SearchResult): string {
  return result.inTrash ? `${result.title} (in trash)` : result.title;
}

function renderSearchEntries() {
  if (!searchResultsListEl) return;
  searchResultsListEl.innerHTML = "";

  const query = searchInputEl?.value.trim() ?? "";
  if (searchEmptyHintEl) {
    if (query === "") {
      searchEmptyHintEl.textContent = "Type to search.";
      searchEmptyHintEl.removeAttribute("hidden");
    } else if (searchEntries.length === 0) {
      searchEmptyHintEl.textContent = "No results.";
      searchEmptyHintEl.removeAttribute("hidden");
    } else {
      searchEmptyHintEl.setAttribute("hidden", "");
    }
  }

  searchEntries.forEach((entry, index) => {
    const li = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.className = "search-result";
    button.classList.toggle("active", index === searchSelectedIndex);

    if (entry.kind === "result") {
      const { result } = entry;
      const titleRow = document.createElement("span");
      titleRow.className = "search-result-title";
      const titleText = document.createElement("span");
      titleText.textContent = searchResultRowLabel(result);
      titleRow.appendChild(titleText);
      if (result.inTrash) {
        const badge = document.createElement("span");
        badge.className = "search-result-in-trash-badge";
        badge.textContent = "In trash";
        titleRow.appendChild(badge);
      }
      button.appendChild(titleRow);

      if (result.tier === 2 && result.matchedTag) {
        const chip = document.createElement("span");
        chip.className = "search-result-tag-chip";
        chip.textContent = `#${result.matchedTag}`;
        button.appendChild(chip);
      } else if (result.tier === 3 && result.snippet) {
        const snippetEl = document.createElement("span");
        snippetEl.className = "search-result-snippet";
        renderHighlightedSnippet(snippetEl, result.snippet);
        button.appendChild(snippetEl);
      }

      button.addEventListener("click", () => void openSearchResult(result));
    } else {
      button.classList.add("create-page-action");
      button.textContent = `Create page: '${entry.query}'`;
      button.addEventListener("click", () => void handleCreatePageFromSearch(entry.query));
    }

    li.appendChild(button);
    searchResultsListEl.appendChild(li);
  });
}

/**
 * Runs one search for whatever's currently in the input, builds the
 * strict-tier results into `searchEntries`, and appends the "Create page"
 * action (ticket 13 point 6) whenever there's no exact title match among the
 * results -- not only when there are zero results, per the prototype spec.
 */
async function runSearch() {
  const query = searchInputEl?.value ?? "";
  const trimmed = query.trim();
  const includeTrash = searchIncludeTrashEl?.checked ?? false;

  if (trimmed === "") {
    searchEntries = [];
    searchSelectedIndex = -1;
    renderSearchEntries();
    return;
  }

  const requestId = ++searchRequestId;
  let results: SearchResult[];
  try {
    results = await searchPages(trimmed, includeTrash);
  } catch (err) {
    console.error("Search failed", err);
    results = [];
  }
  if (requestId !== searchRequestId) return; // a newer search has since started

  const trimmedLower = trimmed.toLowerCase();
  const hasExactTitleMatch = results.some((r) => r.title.toLowerCase() === trimmedLower);

  searchEntries = results.map((result): SearchEntry => ({ kind: "result", result }));
  if (!hasExactTitleMatch) {
    searchEntries.push({ kind: "create", query: trimmed });
  }
  searchSelectedIndex = searchEntries.length > 0 ? 0 : -1;
  renderSearchEntries();
}

function scheduleSearch() {
  if (searchDebounceTimer !== null) clearTimeout(searchDebounceTimer);
  searchDebounceTimer = setTimeout(() => {
    searchDebounceTimer = null;
    void runSearch();
  }, SEARCH_DEBOUNCE_MS);
}

function isSearchModalOpen(): boolean {
  return searchModalOverlayEl ? !searchModalOverlayEl.hasAttribute("hidden") : false;
}

function openSearchModal() {
  if (!searchModalOverlayEl) return;
  searchModalOverlayEl.removeAttribute("hidden");
  if (searchInputEl) {
    searchInputEl.value = "";
    searchInputEl.focus();
  }
  if (searchIncludeTrashEl) searchIncludeTrashEl.checked = false;
  searchEntries = [];
  searchSelectedIndex = -1;
  renderSearchEntries();
}

function closeSearchModal() {
  if (searchDebounceTimer !== null) {
    clearTimeout(searchDebounceTimer);
    searchDebounceTimer = null;
  }
  searchModalOverlayEl?.setAttribute("hidden", "");
  searchInputEl?.blur();
}

function moveSearchSelection(delta: number) {
  if (searchEntries.length === 0) return;
  const next = searchSelectedIndex < 0 ? 0 : searchSelectedIndex + delta;
  searchSelectedIndex = Math.max(0, Math.min(searchEntries.length - 1, next));
  renderSearchEntries();
  const rows = searchResultsListEl?.querySelectorAll<HTMLButtonElement>(".search-result");
  rows?.[searchSelectedIndex]?.scrollIntoView({ block: "nearest" });
}

function activateSelectedSearchEntry() {
  const index = searchSelectedIndex >= 0 ? searchSelectedIndex : 0;
  const entry = searchEntries[index];
  if (!entry) return;
  if (entry.kind === "result") void openSearchResult(entry.result);
  else void handleCreatePageFromSearch(entry.query);
}

function handleSearchModalKeydown(event: KeyboardEvent) {
  switch (event.key) {
    case "Escape":
      event.preventDefault();
      event.stopPropagation();
      closeSearchModal();
      break;
    case "ArrowDown":
      event.preventDefault();
      moveSearchSelection(1);
      break;
    case "ArrowUp":
      event.preventDefault();
      moveSearchSelection(-1);
      break;
    case "Enter":
      event.preventDefault();
      activateSelectedSearchEntry();
      break;
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
 * no extra in-app dialog. Flushes any pending autosave first, and clears the
 * currently open page since it belongs to the vault being left.
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

  await flushPendingSaveForCurrentPage();
  currentPage = null;
  pageArticleEl?.setAttribute("hidden", "");
  pageViewEmptyEl?.removeAttribute("hidden");
  recentPages = [];
  renderRecentList();

  try {
    closeSettingsModal();
    await openVaultAndLoad(path);
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
    await connectAccessToken(remoteUrl, username, token);
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
  const result = await connectGithubOauth(remoteUrl, accessToken, refreshToken, accessTokenExpiresAt);
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
    const connectResult = await connectGitlabOauth(remoteUrl, accessToken, refreshToken, accessTokenExpiresAt);
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

// --- Ticket 12: sidebar sync-status indicator ------------------------------
//
// A persistent, quiet icon -- never a toast/banner -- with five states
// (`syncIndicatorFor` in sync-status.ts does the actual `SyncStatus` ->
// icon-state mapping, kept pure/testable there). This section owns the DOM
// side: applying that mapping to both icon locations (expanded sidebar
// footer + collapsed rail), and the click-to-open popup.

/** Applies `status` to both sync-status icon locations (footer + rail) and,
 * if the popup is currently open, its own icon/status line too -- called on
 * load (`get_sync_status`) and on every `sync-status-changed` event, so the
 * icon "updates live" per the ticket without the frontend polling on a timer
 * of its own. Also refreshes the Settings "Sync" section's not-connected
 * banner, since that must never show stale state while Settings is open. */
function applySyncIndicator(status: SyncStatus) {
  currentSyncStatus = status;
  const indicator = syncIndicatorFor(status);

  for (const iconEl of [syncStatusIconEl, sidebarRailSyncStatusIconEl, syncPopupIconEl]) {
    if (!iconEl) continue;
    iconEl.textContent = indicator.glyph;
    iconEl.dataset.state = indicator.iconState;
  }
  if (syncStatusLabelEl) syncStatusLabelEl.textContent = indicator.statusText;
  const ariaLabel = `Sync status: ${indicator.statusText}`;
  syncStatusButtonEl?.setAttribute("aria-label", ariaLabel);
  sidebarRailSyncStatusButtonEl?.setAttribute("aria-label", ariaLabel);
  sidebarRailSyncStatusButtonEl?.setAttribute("title", ariaLabel);
  if (syncPopupStatusTextEl) syncPopupStatusTextEl.textContent = indicator.statusText;

  updateSyncSectionNotConnectedBanner();
}

/** Ticket 12 checklist item 5: the plain "Not connected -- connect a
 * repository" message, shown only while `currentSyncStatus` is `NoRemote` --
 * never nags otherwise. Called whenever the status changes and whenever
 * Settings opens, so it can't go stale while the modal is up. */
function updateSyncSectionNotConnectedBanner() {
  if (!syncSectionNotConnectedEl) return;
  syncSectionNotConnectedEl.hidden = currentSyncStatus.state !== "noRemote";
}

/** Loads the current sync status once (on app start, and again once a vault
 * finishes opening) -- live updates after that come from
 * `sync-status-changed` alone, not further polling. */
async function refreshSyncStatus() {
  try {
    applySyncIndicator(await getSyncStatus());
  } catch {
    // No vault open yet, or the command otherwise unavailable -- leave the
    // icon at its default "not connected" state rather than erroring.
  }
}

function isSyncPopupOpen(): boolean {
  return !!syncPopupEl && !syncPopupEl.hasAttribute("hidden");
}

/** Positions `#sync-popup` just above/beside whichever icon (footer or
 * rail, whichever is actually visible) was clicked, then shows it and loads
 * the popup-only detail (`get_sync_details`) -- never auto-opened, only in
 * response to a click, per the ticket. */
function openSyncPopup(anchor: HTMLElement) {
  if (!syncPopupEl) return;
  const anchorRect = anchor.getBoundingClientRect();
  syncPopupEl.style.left = `${Math.round(anchorRect.left)}px`;
  syncPopupEl.style.bottom = `${Math.round(window.innerHeight - anchorRect.top + 8)}px`;
  syncPopupEl.style.top = "auto";
  syncPopupEl.removeAttribute("hidden");
  syncStatusButtonEl?.setAttribute("aria-expanded", "true");
  sidebarRailSyncStatusButtonEl?.setAttribute("aria-expanded", "true");
  void refreshSyncPopupDetails();
}

function closeSyncPopup() {
  if (!syncPopupEl) return;
  syncPopupEl.setAttribute("hidden", "");
  syncStatusButtonEl?.setAttribute("aria-expanded", "false");
  sidebarRailSyncStatusButtonEl?.setAttribute("aria-expanded", "false");
}

function toggleSyncPopup(anchor: HTMLElement) {
  if (isSyncPopupOpen()) {
    closeSyncPopup();
  } else {
    openSyncPopup(anchor);
  }
}

/** Fills the popup's provider/last-synced lines from `get_sync_details` --
 * fetched only when the popup actually opens (ticket 12 checklist item 3),
 * not on every status change. */
async function refreshSyncPopupDetails() {
  if (!syncPopupProviderEl || !syncPopupLastSyncedEl) return;
  try {
    const details = await getSyncDetails();
    if (details.provider) {
      syncPopupProviderEl.textContent = details.provider;
      syncPopupProviderEl.removeAttribute("hidden");
    } else {
      syncPopupProviderEl.setAttribute("hidden", "");
    }
    if (details.lastSyncedAt != null) {
      const date = new Date(details.lastSyncedAt * 1000);
      syncPopupLastSyncedEl.textContent = `Last synced: ${date.toLocaleString()}`;
      syncPopupLastSyncedEl.removeAttribute("hidden");
    } else {
      syncPopupLastSyncedEl.setAttribute("hidden", "");
    }
  } catch {
    syncPopupProviderEl.setAttribute("hidden", "");
    syncPopupLastSyncedEl.setAttribute("hidden", "");
  }
}

/** "Sync now" (ticket 12 checklist item 4): triggers an immediate attempt
 * independent of the background timer. The resulting status change arrives
 * via `sync-status-changed` like any other transition -- this doesn't wait
 * for or reflect the outcome itself, just fires the attempt. */
async function handleSyncNowClick() {
  if (!syncPopupSyncNowButtonEl) return;
  syncPopupSyncNowButtonEl.setAttribute("disabled", "");
  try {
    await triggerSyncNow();
  } catch {
    // No vault open -- nothing to sync; the button simply has no effect.
  } finally {
    syncPopupSyncNowButtonEl.removeAttribute("disabled");
  }
}

/** The popup's "Sync settings…" link and the not-connected banner's
 * "Connect…" button both close the popup and open Settings' "Sync" section
 * -- reusing `openSyncManualForm`'s scroll-into-view, ticket 11's existing
 * entry point, rather than duplicating it. */
function openSyncSettingsFromPopup() {
  closeSyncPopup();
  openSyncManualForm("");
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
  const privateKeyOpenssh = window.prompt("Paste the private key (OpenSSH format):");
  if (privateKeyOpenssh === null || !privateKeyOpenssh.trim()) return;
  const passphrase = window.prompt("Passphrase (leave blank if none):") ?? undefined;

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
    await connectSshKey(remoteUrl, settingsSshKey.privateKeyOpenssh, settingsSshKey.passphrase);
    settingsSshKeyStatusEl.textContent = "Connected.";
  } catch (err) {
    settingsSshKeyStatusEl.textContent = String(err);
  } finally {
    settingsSshKeyConnectButtonEl?.removeAttribute("disabled");
  }
}

/**
 * Ticket 11's carry-over into the manual "Sync" section: prefills every
 * sub-form's own repository URL field with `remoteUrl` (whatever the wizard
 * had already committed, if anything) so the user doesn't have to retype a
 * URL they already entered, even though which sub-form ends up visible
 * depends on the credential kind they pick next. Per the ticket, re-asking
 * for a credential is an accepted limitation -- this just avoids re-asking
 * for the URL too, where it's cheap to.
 */
function openSyncManualForm(remoteUrl: string) {
  openSettingsModal();
  if (remoteUrl) {
    if (settingsConnectUrlEl) settingsConnectUrlEl.value = remoteUrl;
    if (settingsSshKeyUrlEl) settingsSshKeyUrlEl.value = remoteUrl;
    if (settingsGithubUrlEl) settingsGithubUrlEl.value = remoteUrl;
    if (settingsGitlabUrlEl) settingsGitlabUrlEl.value = remoteUrl;
  }
  document.getElementById("sync-section")?.scrollIntoView({ block: "start" });
}

function openSettingsModal() {
  if (!settingsModalOverlayEl) return;
  if (settingsVaultPathEl) settingsVaultPathEl.textContent = currentVaultPath ?? "";
  settingsModalOverlayEl.removeAttribute("hidden");
  void refreshCommitAuthorFields();
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
 * one (a pasted/created/picked repository URL) -- see `openSyncManualForm`'s
 * doc comment for what "carrying over" does and doesn't cover.
 */
function handleWizardManualSetupClick() {
  const remoteUrl = wizardState.remoteUrl ?? "";
  closeConnectWizard();
  openSyncManualForm(remoteUrl);
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
 * `handleNewPageClick`'s title prompt) rather than a bespoke multi-line
 * form -- validates the key (and passphrase, if given) without persisting
 * anything, same as `handleWizardGenerateSshKey`.
 */
async function handleWizardImportSshKey(statusEl: HTMLElement) {
  const privateKeyOpenssh = window.prompt("Paste the private key (OpenSSH format):");
  if (privateKeyOpenssh === null || !privateKeyOpenssh.trim()) return;
  const passphrase = window.prompt("Passphrase (leave blank if none):") ?? undefined;

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
        await connectGithubOauth(remoteUrl, wizardAccessToken, wizardRefreshToken ?? "", wizardAccessTokenExpiresAt);
      } else {
        await connectGitlabOauth(remoteUrl, wizardAccessToken, wizardRefreshToken, wizardAccessTokenExpiresAt);
      }
    } else if (wizardState.credentialKind === "accessToken") {
      if (!wizardPendingAccessTokenConnect) throw new Error("Missing access token details.");
      await connectAccessToken(
        wizardPendingAccessTokenConnect.remoteUrl,
        wizardPendingAccessTokenConnect.username,
        wizardPendingAccessTokenConnect.token,
      );
    } else if (wizardState.credentialKind === "sshKey") {
      if (!wizardSshKey) throw new Error("Generate or import an SSH key first.");
      await connectSshKey(remoteUrl, wizardSshKey.privateKeyOpenssh, wizardSshKey.passphrase);
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

// --- Ticket 10: guided clone wizard (device #2, full-screen) -------------
//
// Same architecture as ticket 09's connect wizard section above (a pure
// `reduceCloneWizard` state machine plus this section owning rendering and
// the actual `invoke` calls), but full-screen instead of a modal, and with
// clone's own ordering: authenticate first, then pick/paste a repository,
// then pick a destination folder, then the real clone (`clone_and_open_vault`
// -- the clone itself is the test that gates persistence), then "Commit as".
//
// Ephemeral data the reducer doesn't track lives in these module-level
// variables, reset every time the wizard is (re)opened -- same pattern as
// `wizardAccessToken`/`wizardRepos`/etc above.

let cloneWizardState: CloneWizardState = initialCloneWizardState;
let cloneWizardAccessToken: string | null = null;
let cloneWizardRefreshToken: string | undefined;
let cloneWizardAccessTokenExpiresAt = "";
let cloneWizardRepos: RepoInfo[] = [];
let cloneWizardSshKey: SshKeyInfo | null = null;
/** Holds the pasted-URL access-token form's values between `pasteUrl` and `destinationPicker`, since the reducer only tracks `remoteUrl` -- mirrors `wizardPendingAccessTokenConnect`. */
let cloneWizardPendingAccessTokenConnect: { remoteUrl: string; username: string; token: string } | null = null;
/** What the backend's `cloneAndOpenVault` returned once the clone itself succeeds -- the "Commit as" step reads its prefill straight from here instead of a second round trip (see `cloneAndOpenVault`'s doc comment for why the prefill has to be computed at clone time, not via the ordinary `commitAuthorPrefill` command). */
let cloneWizardAuthorPrefill: CommitAuthorPrefillResult | null = null;
/** Bumped on every open/close so a stale device-flow poll loop or in-flight clone from a previous attempt can tell it's been abandoned. */
let cloneWizardGeneration = 0;

function dispatchCloneWizard(action: CloneWizardAction) {
  cloneWizardState = reduceCloneWizard(cloneWizardState, action);
  if (isCloneWizardSwitchedToManual(cloneWizardState)) {
    // Ticket 11: tear down the full-screen wizard and open the standalone
    // "git clone" dialog instead of rendering a step.
    handleCloneWizardManualSetupClick();
    return;
  }
  renderCloneWizardStep();
  if (isCloneWizardDone(cloneWizardState)) {
    // The clone already opened the vault (server side) -- swap the
    // full-screen wizard for the ordinary workspace, same tail
    // `openVaultAndLoad` runs after a plain first-run pick.
    closeCloneWizard();
    void finishCloneWizardIntoWorkspace();
  }
}

/**
 * Ticket 11's "Switch to manual setup" escape hatch for the clone wizard:
 * tears down the full-screen surface (back to the first-run folder-picker,
 * same as `closeCloneWizard` normally does) and opens the standalone
 * "git clone" dialog, carrying over `remoteUrl`/`destination` if either was
 * already committed -- same accepted "may re-ask for values" limitation as
 * the connect wizard's equivalent, per ticket 08's answer.
 */
function handleCloneWizardManualSetupClick() {
  const remoteUrl = cloneWizardState.remoteUrl ?? "";
  const destination = cloneWizardState.destination ?? "";
  closeCloneWizard();
  openCloneManualDialog(remoteUrl, destination);
}

async function finishCloneWizardIntoWorkspace() {
  const path = cloneWizardClonedPath;
  if (!path) return;
  currentVaultPath = path;
  if (settingsVaultPathEl) settingsVaultPathEl.textContent = path;
  showWorkspace();
  await loadPages();
  await loadTrash();
  await refreshSyncStatus();
}

/** Set once `clone_and_open_vault` succeeds -- the vault path the finishing tail above opens the workspace onto. */
let cloneWizardClonedPath: string | null = null;

function openCloneWizard() {
  if (!cloneWizardOverlayEl) return;
  cloneWizardGeneration += 1;
  cloneWizardState = { ...initialCloneWizardState };
  cloneWizardAccessToken = null;
  cloneWizardRefreshToken = undefined;
  cloneWizardAccessTokenExpiresAt = "";
  cloneWizardRepos = [];
  cloneWizardSshKey = null;
  cloneWizardPendingAccessTokenConnect = null;
  cloneWizardAuthorPrefill = null;
  cloneWizardClonedPath = null;
  vaultPickerEl?.setAttribute("hidden", "");
  cloneWizardOverlayEl.removeAttribute("hidden");
  renderCloneWizardStep();
}

/** Cancels the wizard and returns to the first-run folder-picker screen -- there is no vault to fall back into (unlike closing ticket 09's connect wizard, which just returns to Settings). */
function closeCloneWizard() {
  cloneWizardGeneration += 1; // invalidates any in-flight poll loop/clone
  cloneWizardOverlayEl?.setAttribute("hidden", "");
  if (!isCloneWizardDone(cloneWizardState)) {
    vaultPickerEl?.removeAttribute("hidden");
  }
}

/** Renders the current step's content into `#clone-wizard-body`, and updates the shared "Back" button's visibility. */
function renderCloneWizardStep() {
  if (!cloneWizardBodyEl) return;
  cloneWizardBodyEl.innerHTML = "";

  const canGoBack = !isCloneWizardBusyStep(cloneWizardState.step) && cloneWizardState.step !== "providerChoice";
  if (cloneWizardBackButtonEl) cloneWizardBackButtonEl.hidden = !canGoBack;

  switch (cloneWizardState.step) {
    case "providerChoice":
      renderCloneProviderChoiceStep();
      break;
    case "credentialChoice":
      renderCloneCredentialChoiceStep();
      break;
    case "otherCredentialKindChoice":
      renderCloneOtherCredentialKindChoiceStep();
      break;
    case "oauthSignIn":
      renderCloneOauthSignInStep();
      break;
    case "repoPicker":
      renderCloneRepoPickerStep();
      break;
    case "pasteUrl":
      renderClonePasteUrlStep();
      break;
    case "destinationPicker":
      renderCloneDestinationPickerStep();
      break;
    case "cloning":
      renderCloningStep();
      break;
    case "cloneError":
      renderCloneErrorStep();
      break;
    case "commitAuthor":
      void renderCloneCommitAuthorStep();
      break;
    case "done":
      break;
  }
}

function renderCloneProviderChoiceStep() {
  if (!cloneWizardBodyEl) return;
  cloneWizardBodyEl.appendChild(el("p", "wizard-question", "Which provider is the repository on?"));
  const row = el("div", "wizard-button-row");

  const githubButton = el("button", undefined, "GitHub");
  githubButton.type = "button";
  githubButton.addEventListener("click", () => dispatchCloneWizard({ type: "chooseProvider", provider: "github" }));
  row.appendChild(githubButton);

  const gitlabButton = el("button", undefined, "GitLab");
  gitlabButton.type = "button";
  gitlabButton.addEventListener("click", () => dispatchCloneWizard({ type: "chooseProvider", provider: "gitlab" }));
  row.appendChild(gitlabButton);

  const otherButton = el("button", undefined, "Another provider");
  otherButton.type = "button";
  otherButton.addEventListener("click", () => dispatchCloneWizard({ type: "chooseProvider", provider: "other" }));
  row.appendChild(otherButton);

  cloneWizardBodyEl.appendChild(row);
}

function renderCloneCredentialChoiceStep() {
  if (!cloneWizardBodyEl) return;
  const label = providerLabel(cloneWizardState.provider);
  cloneWizardBodyEl.appendChild(el("p", "wizard-question", `How do you want to connect to ${label}?`));

  const signInButton = el("button", "wizard-primary-action", `Sign in with ${label}`);
  signInButton.type = "button";
  signInButton.addEventListener("click", () => dispatchCloneWizard({ type: "chooseOauthSignIn" }));
  cloneWizardBodyEl.appendChild(signInButton);

  const otherWaysButton = el("button", "wizard-secondary-action", "Other ways to connect");
  otherWaysButton.type = "button";
  otherWaysButton.addEventListener("click", () => dispatchCloneWizard({ type: "chooseOtherWaysToConnect" }));
  cloneWizardBodyEl.appendChild(otherWaysButton);
}

function renderCloneOtherCredentialKindChoiceStep() {
  if (!cloneWizardBodyEl) return;
  cloneWizardBodyEl.appendChild(el("p", "wizard-question", "How do you want to authenticate?"));
  const row = el("div", "wizard-button-row");

  const tokenButton = el("button", undefined, "Access token");
  tokenButton.type = "button";
  tokenButton.addEventListener("click", () =>
    dispatchCloneWizard({ type: "chooseOtherCredentialKind", kind: "accessToken" }),
  );
  row.appendChild(tokenButton);

  const sshButton = el("button", undefined, "SSH key");
  sshButton.type = "button";
  sshButton.addEventListener("click", () =>
    dispatchCloneWizard({ type: "chooseOtherCredentialKind", kind: "sshKey" }),
  );
  row.appendChild(sshButton);

  cloneWizardBodyEl.appendChild(row);
}

/** Ticket 06/07's device-flow steps -- same mechanics as `runOauthSignIn` above, landing on `dispatchCloneWizard` transitions instead. */
function renderCloneOauthSignInStep() {
  if (!cloneWizardBodyEl) return;
  const provider = cloneWizardState.provider;
  const statusEl = el("p", "settings-connect-status", `Requesting a device code from ${providerLabel(provider)}…`);
  cloneWizardBodyEl.appendChild(statusEl);
  const codeEl = el("div", "settings-connect-status");
  codeEl.hidden = true;
  cloneWizardBodyEl.appendChild(codeEl);

  const generation = cloneWizardGeneration;
  void runCloneOauthSignIn(provider, statusEl, codeEl, generation);
}

async function runCloneOauthSignIn(
  provider: WizardProvider | null,
  statusEl: HTMLElement,
  codeEl: HTMLElement,
  generation: number,
) {
  const stale = () => generation !== cloneWizardGeneration;
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
      if (Date.now() >= deadline)
        throw new Error(`The ${providerLabel(provider)} sign-in code expired before it was confirmed.`);
      await new Promise((resolve) => setTimeout(resolve, intervalMs));
      if (stale()) return;

      const result =
        provider === "gitlab" ? await pollGitlabDeviceFlow(device.deviceCode) : await pollGithubDeviceFlow(device.deviceCode);
      if (result.outcome === "pending") continue;
      if (result.outcome === "slowDown") {
        intervalMs += 5000;
        continue;
      }
      if (result.outcome === "denied") throw new Error(`${providerLabel(provider)} sign-in was denied.`);
      if (result.outcome === "expired")
        throw new Error(`The ${providerLabel(provider)} sign-in code expired before it was confirmed.`);
      if (result.outcome === "error") throw new Error(result.message);

      accessToken = result.accessToken;
      refreshToken = result.refreshToken;
      accessTokenExpiresAt = result.accessTokenExpiresAt;
      break;
    }

    if (stale()) return;

    cloneWizardAccessToken = accessToken;
    cloneWizardRefreshToken = refreshToken;
    cloneWizardAccessTokenExpiresAt = accessTokenExpiresAt;
    codeEl.hidden = true;
    dispatchCloneWizard({ type: "oauthSignInSucceeded" });
  } catch (err) {
    if (stale()) return;
    statusEl.textContent = String(err);
    codeEl.hidden = true;
  }
}

function renderCloneRepoPickerStep() {
  if (!cloneWizardBodyEl) return;
  const statusEl = el("p", "settings-connect-status", "Loading your repositories…");
  cloneWizardBodyEl.appendChild(statusEl);
  const listEl = el("ul", "wizard-repo-list");
  cloneWizardBodyEl.appendChild(listEl);

  const generation = cloneWizardGeneration;
  void loadCloneRepoPicker(statusEl, listEl, generation);
}

async function loadCloneRepoPicker(statusEl: HTMLElement, listEl: HTMLElement, generation: number) {
  if (!cloneWizardAccessToken) {
    statusEl.textContent = "Sign-in is required before listing repositories.";
    return;
  }
  try {
    cloneWizardRepos =
      cloneWizardState.provider === "gitlab"
        ? await listGitlabRepositories(cloneWizardAccessToken)
        : await listGithubRepositories(cloneWizardAccessToken);
    if (generation !== cloneWizardGeneration) return;

    if (cloneWizardRepos.length === 0) {
      statusEl.textContent = "No repositories found for this account.";
      return;
    }
    statusEl.textContent = "Pick a repository to clone:";
    for (const repo of cloneWizardRepos) {
      const item = el("li");
      const button = el("button", "wizard-repo-item");
      button.type = "button";
      button.appendChild(el("span", "wizard-repo-name", repo.fullName));
      button.appendChild(el("span", "wizard-repo-visibility", repo.private ? "Private" : "Public"));
      button.addEventListener("click", () => dispatchCloneWizard({ type: "repoSelected", remoteUrl: repo.cloneUrl }));
      item.appendChild(button);
      listEl.appendChild(item);
    }
  } catch (err) {
    if (generation !== cloneWizardGeneration) return;
    statusEl.textContent = `Couldn't load repositories: ${String(err)}`;
  }
}

function renderClonePasteUrlStep() {
  if (!cloneWizardBodyEl) return;
  cloneWizardBodyEl.appendChild(el("p", "wizard-question", "Enter the repository's URL:"));

  const urlInput = el("input");
  urlInput.type = "text";
  urlInput.placeholder = "https://example.com/user/repo.git";
  cloneWizardBodyEl.appendChild(urlInput);

  if (cloneWizardState.credentialKind === "accessToken") {
    const usernameInput = el("input");
    usernameInput.type = "text";
    usernameInput.placeholder = "Username";
    cloneWizardBodyEl.appendChild(usernameInput);
    const tokenInput = el("input");
    tokenInput.type = "password";
    tokenInput.placeholder = "Access token";
    cloneWizardBodyEl.appendChild(tokenInput);

    const errorEl = el("p", "error wizard-inline-error");
    errorEl.hidden = true;

    const nextButton = el("button", "wizard-primary-action", "Continue");
    nextButton.type = "button";
    nextButton.addEventListener("click", () => {
      const remoteUrl = urlInput.value.trim();
      const username = usernameInput.value.trim();
      const token = tokenInput.value;
      if (!remoteUrl || !username || !token) {
        errorEl.textContent = "Repository URL, username, and access token are all required.";
        errorEl.hidden = false;
        return;
      }
      errorEl.hidden = true;
      cloneWizardPendingAccessTokenConnect = { remoteUrl, username, token };
      dispatchCloneWizard({ type: "urlEntered", remoteUrl });
    });
    cloneWizardBodyEl.appendChild(nextButton);
    cloneWizardBodyEl.appendChild(errorEl);
    return;
  }

  // SSH key path (ticket 05): generate (default) or import, then continue.
  const keyStatusEl = el("p", "settings-connect-status");
  cloneWizardBodyEl.appendChild(keyStatusEl);

  const generateButton = el("button", undefined, "Generate a new key");
  generateButton.type = "button";
  generateButton.addEventListener("click", () => void handleCloneWizardGenerateSshKey(keyStatusEl));
  cloneWizardBodyEl.appendChild(generateButton);

  const importButton = el("button", undefined, "Import an existing key");
  importButton.type = "button";
  importButton.addEventListener("click", () => void handleCloneWizardImportSshKey(keyStatusEl));
  cloneWizardBodyEl.appendChild(importButton);

  const nextButton = el("button", "wizard-primary-action", "Continue");
  nextButton.type = "button";
  nextButton.addEventListener("click", () => {
    const remoteUrl = urlInput.value.trim();
    if (!remoteUrl) {
      keyStatusEl.textContent = "Enter the repository's URL first.";
      return;
    }
    if (!cloneWizardSshKey) {
      keyStatusEl.textContent = "Generate (or import) a key first.";
      return;
    }
    dispatchCloneWizard({ type: "urlEntered", remoteUrl });
  });
  cloneWizardBodyEl.appendChild(nextButton);
}

async function handleCloneWizardGenerateSshKey(statusEl: HTMLElement) {
  statusEl.textContent = "Generating…";
  try {
    cloneWizardSshKey = await generateSshKey();
    statusEl.textContent = `Key ready (fingerprint ${cloneWizardSshKey.fingerprintSha256}). Add the public key to your provider, then Continue.`;
  } catch (err) {
    statusEl.textContent = String(err);
  }
}

async function handleCloneWizardImportSshKey(statusEl: HTMLElement) {
  const privateKeyOpenssh = window.prompt("Paste the private key (OpenSSH format):");
  if (privateKeyOpenssh === null || !privateKeyOpenssh.trim()) return;
  const passphrase = window.prompt("Passphrase (leave blank if none):") ?? undefined;

  statusEl.textContent = "Importing…";
  try {
    cloneWizardSshKey = await importSshKey(privateKeyOpenssh, passphrase || undefined);
    statusEl.textContent = `Key imported (fingerprint ${cloneWizardSshKey.fingerprintSha256}). Add the public key to your provider, then Continue.`;
  } catch (err) {
    statusEl.textContent = String(err);
  }
}

/** Ticket 10 checklist item 3: pick a destination folder, reusing the exact same native folder-picker mechanism (`pickVaultFolder`) the first-run screen already uses -- there is nothing vault-specific about it, it just opens a directory picker. */
function renderCloneDestinationPickerStep() {
  if (!cloneWizardBodyEl) return;
  cloneWizardBodyEl.appendChild(
    el("p", "wizard-question", "Pick an empty folder to clone into:"),
  );
  const statusEl = el("p", "settings-connect-status");
  cloneWizardBodyEl.appendChild(statusEl);

  const pickButton = el("button", "wizard-primary-action", "Choose folder…");
  pickButton.type = "button";
  pickButton.addEventListener("click", () => void handleCloneDestinationPickClick(statusEl));
  cloneWizardBodyEl.appendChild(pickButton);
}

async function handleCloneDestinationPickClick(statusEl: HTMLElement) {
  try {
    const path = await pickVaultFolder();
    if (!path) return; // user cancelled
    dispatchCloneWizard({ type: "destinationChosen", destination: path });
  } catch (err) {
    statusEl.textContent = String(err);
  }
}

function renderCloningStep() {
  if (!cloneWizardBodyEl) return;
  cloneWizardBodyEl.appendChild(el("p", "settings-connect-status", "Cloning…"));
  const generation = cloneWizardGeneration;
  void performClone(generation);
}

/**
 * Ticket 10 checklist item 4/5/7: the real clone -- `clone_and_open_vault`
 * authenticates with whichever credential this wizard obtained, clones,
 * classifies the four post-clone states, and only *then* persists the
 * Connection + remembered vault path. A failure here (network, credential,
 * or the "refused" classification) turns into `cloneFailed` (a clear
 * in-wizard error, nothing saved); success turns into `cloneSucceeded` (->
 * "Commit as").
 */
async function performClone(generation: number) {
  const remoteUrl = cloneWizardState.remoteUrl;
  const destination = cloneWizardState.destination;
  if (!remoteUrl || !destination) {
    dispatchCloneWizard({ type: "cloneFailed", message: "No repository or destination folder to clone into." });
    return;
  }
  try {
    let credential: CloneCredential;
    if (cloneWizardState.credentialKind === "oauth") {
      if (!cloneWizardAccessToken) throw new Error("Sign-in is required before cloning.");
      if (cloneWizardState.provider === "github") {
        const installation = await checkGithubInstallation(remoteUrl, cloneWizardAccessToken);
        if (installation.status === "notInstalled") {
          throw new Error(
            `Cerebrite isn't installed on this repository yet. Install it at ${installation.installUrl}, then try again.`,
          );
        }
        credential = {
          kind: "githubOauth",
          accessToken: cloneWizardAccessToken,
          refreshToken: cloneWizardRefreshToken ?? "",
          accessTokenExpiresAt: cloneWizardAccessTokenExpiresAt,
        };
      } else {
        credential = {
          kind: "gitlabOauth",
          accessToken: cloneWizardAccessToken,
          refreshToken: cloneWizardRefreshToken,
          accessTokenExpiresAt: cloneWizardAccessTokenExpiresAt,
        };
      }
    } else if (cloneWizardState.credentialKind === "accessToken") {
      if (!cloneWizardPendingAccessTokenConnect) throw new Error("Missing access token details.");
      credential = {
        kind: "accessToken",
        username: cloneWizardPendingAccessTokenConnect.username,
        token: cloneWizardPendingAccessTokenConnect.token,
      };
    } else if (cloneWizardState.credentialKind === "sshKey") {
      if (!cloneWizardSshKey) throw new Error("Generate or import an SSH key first.");
      credential = {
        kind: "sshKey",
        privateKeyOpenssh: cloneWizardSshKey.privateKeyOpenssh,
        passphrase: cloneWizardSshKey.passphrase,
      };
    } else {
      throw new Error("No authentication method was chosen.");
    }

    const result = await cloneAndOpenVault(remoteUrl, destination, credential);
    if (generation !== cloneWizardGeneration) return;
    cloneWizardClonedPath = result.vault.path;
    cloneWizardAuthorPrefill = result.authorPrefill;
    dispatchCloneWizard({ type: "cloneSucceeded" });
  } catch (err) {
    if (generation !== cloneWizardGeneration) return;
    dispatchCloneWizard({ type: "cloneFailed", message: `Couldn't clone: ${String(err)}` });
  }
}

function renderCloneErrorStep() {
  if (!cloneWizardBodyEl) return;
  cloneWizardBodyEl.appendChild(el("p", "error", cloneWizardState.error ?? "Couldn't clone."));
  const retryButton = el("button", "wizard-primary-action", "Pick a different folder and try again");
  retryButton.type = "button";
  retryButton.addEventListener("click", () => dispatchCloneWizard({ type: "retry" }));
  cloneWizardBodyEl.appendChild(retryButton);
}

/** Ticket 10's last step, reusing ticket 08's exact commands -- prefilled from `cloneWizardAuthorPrefill` (computed by the backend at clone time, see its declaration above) rather than a second `commitAuthorPrefill` round trip. */
async function renderCloneCommitAuthorStep() {
  if (!cloneWizardBodyEl) return;
  cloneWizardBodyEl.appendChild(el("p", "wizard-question", "Commit as:"));

  const nameInput = el("input");
  nameInput.type = "text";
  nameInput.placeholder = "Name";
  const emailInput = el("input");
  emailInput.type = "text";
  emailInput.placeholder = "Email";

  const author = cloneWizardAuthorPrefill?.author ?? null;
  nameInput.value = author?.name ?? "";
  emailInput.value = author?.email ?? "";

  cloneWizardBodyEl.appendChild(nameInput);
  cloneWizardBodyEl.appendChild(emailInput);

  const errorEl = el("p", "error wizard-inline-error");
  errorEl.hidden = true;

  const saveButton = el("button", "wizard-primary-action", "Finish");
  saveButton.type = "button";
  saveButton.addEventListener("click", () => {
    void (async () => {
      try {
        // `confirmCommitAuthor` operates on `AppState`'s currently open
        // vault -- `clone_and_open_vault` already opened it server-side by
        // this point, so this is the same call ticket 08's own commands use.
        await confirmCommitAuthor(nameInput.value.trim(), emailInput.value.trim());
        await refreshCommitAuthorFields();
        dispatchCloneWizard({ type: "commitAuthorConfirmed" });
      } catch (err) {
        errorEl.textContent = String(err);
        errorEl.hidden = false;
      }
    })();
  });
  cloneWizardBodyEl.appendChild(saveButton);
  cloneWizardBodyEl.appendChild(errorEl);
}

// --- Ticket 11: the standalone "git clone" manual dialog -------------------
//
// Variant B's raw-git-vocabulary door into `clone_and_open_vault` (ticket
// 10) -- reachable with no vault/Settings surface to anchor it to (the
// first-run screen's own link), and from the guided clone wizard's
// persistent "Switch to manual setup" link. Same shape as the Sync section
// above: a credential-kind selector reveals one of four sub-forms, each
// calling the same device-flow/key/token mechanisms tickets 04-07 already
// expose, and the actual `cloneAndOpenVault` call is the same real
// test-fetch-then-persist gate the guided wizard uses -- nothing here
// bypasses it.

let cloneManualGithubToken: { accessToken: string; refreshToken: string; accessTokenExpiresAt: string } | null =
  null;
let cloneManualGitlabToken: { accessToken: string; refreshToken?: string; accessTokenExpiresAt: string } | null =
  null;
let cloneManualSshKey: SshKeyInfo | null = null;
/** Bumped on every open/close so a stale device-flow poll loop from a previous attempt can tell it's been abandoned -- same pattern as `wizardGeneration`/`cloneWizardGeneration`. */
let cloneManualGeneration = 0;

function updateCloneManualSubformVisibility() {
  const kind = cloneManualCredentialKindEl?.value ?? "accessToken";
  document.querySelectorAll<HTMLElement>("#clone-manual-dialog .sync-subform").forEach((subform) => {
    subform.hidden = subform.dataset.cloneKind !== kind;
  });
}

function resetCloneManualStatuses() {
  for (const statusEl of [cloneManualStatusEl, cloneManualSshKeyStatusEl, cloneManualGithubStatusEl, cloneManualGitlabStatusEl]) {
    if (!statusEl) continue;
    statusEl.textContent = "";
    statusEl.setAttribute("hidden", "");
  }
  cloneManualGithubDeviceCodeEl?.setAttribute("hidden", "");
  cloneManualGitlabDeviceCodeEl?.setAttribute("hidden", "");
}

function showCloneManualError(message: string) {
  if (!cloneManualStatusEl) return;
  cloneManualStatusEl.textContent = message;
  cloneManualStatusEl.removeAttribute("hidden");
}

/** Opens the dialog fresh, optionally prefilled (ticket 11's wizard-switch carry-over) with a remote URL and/or destination already committed elsewhere. */
function openCloneManualDialog(prefillRemoteUrl = "", prefillDestination = "") {
  if (!cloneManualOverlayEl) return;
  cloneManualGeneration += 1;
  cloneManualGithubToken = null;
  cloneManualGitlabToken = null;
  cloneManualSshKey = null;
  if (cloneManualUrlEl) cloneManualUrlEl.value = prefillRemoteUrl;
  if (cloneManualDestinationEl) cloneManualDestinationEl.value = prefillDestination;
  if (cloneManualCredentialKindEl) cloneManualCredentialKindEl.value = "accessToken";
  if (cloneManualTokenUsernameEl) cloneManualTokenUsernameEl.value = "";
  if (cloneManualTokenValueEl) cloneManualTokenValueEl.value = "";
  updateCloneManualSubformVisibility();
  resetCloneManualStatuses();
  cloneManualOverlayEl.removeAttribute("hidden");
}

function closeCloneManualDialog() {
  cloneManualGeneration += 1; // invalidates any in-flight poll loop/clone
  cloneManualOverlayEl?.setAttribute("hidden", "");
}

async function handleCloneManualDestinationClick() {
  try {
    const path = await pickVaultFolder();
    if (!path) return; // user cancelled
    if (cloneManualDestinationEl) cloneManualDestinationEl.value = path;
  } catch (err) {
    showCloneManualError(String(err));
  }
}

async function handleCloneManualSshKeyGenerateClick() {
  if (!cloneManualSshKeyStatusEl) return;
  cloneManualSshKeyStatusEl.textContent = "Generating…";
  cloneManualSshKeyStatusEl.removeAttribute("hidden");
  try {
    cloneManualSshKey = await generateSshKey();
    cloneManualSshKeyStatusEl.textContent =
      `Key ready (fingerprint ${cloneManualSshKey.fingerprintSha256}). Add the public key to your provider, then Clone.`;
  } catch (err) {
    cloneManualSshKeyStatusEl.textContent = String(err);
  }
}

async function handleCloneManualSshKeyImportClick() {
  if (!cloneManualSshKeyStatusEl) return;
  const privateKeyOpenssh = window.prompt("Paste the private key (OpenSSH format):");
  if (privateKeyOpenssh === null || !privateKeyOpenssh.trim()) return;
  const passphrase = window.prompt("Passphrase (leave blank if none):") ?? undefined;

  cloneManualSshKeyStatusEl.textContent = "Importing…";
  cloneManualSshKeyStatusEl.removeAttribute("hidden");
  try {
    cloneManualSshKey = await importSshKey(privateKeyOpenssh, passphrase || undefined);
    cloneManualSshKeyStatusEl.textContent =
      `Key imported (fingerprint ${cloneManualSshKey.fingerprintSha256}). Add the public key to your provider, then Clone.`;
  } catch (err) {
    cloneManualSshKeyStatusEl.textContent = String(err);
  }
}

/** The device-flow sign-in shared shape (tickets 06/07), landing in `cloneManualGithubToken`/`cloneManualGitlabToken` instead of dispatching a wizard action -- this dialog has no reducer of its own to drive. */
async function runCloneManualOauthSignIn(provider: "github" | "gitlab") {
  const signinButtonEl = provider === "github" ? cloneManualGithubSigninButtonEl : cloneManualGitlabSigninButtonEl;
  const deviceCodeEl = provider === "github" ? cloneManualGithubDeviceCodeEl : cloneManualGitlabDeviceCodeEl;
  const statusEl = provider === "github" ? cloneManualGithubStatusEl : cloneManualGitlabStatusEl;
  if (!statusEl) return;
  const label = provider === "github" ? "GitHub" : "GitLab";
  const generation = cloneManualGeneration;

  const setStatus = (text: string) => {
    statusEl.textContent = text;
    statusEl.removeAttribute("hidden");
  };

  signinButtonEl?.setAttribute("disabled", "");
  deviceCodeEl?.setAttribute("hidden", "");
  try {
    setStatus(`Requesting a device code from ${label}…`);
    const device = provider === "github" ? await startGithubDeviceFlow() : await startGitlabDeviceFlow();
    if (generation !== cloneManualGeneration) return;

    if (deviceCodeEl) {
      deviceCodeEl.innerHTML = "";
      const link = el("a", undefined, device.verificationUri);
      link.href = device.verificationUri;
      link.target = "_blank";
      link.rel = "noopener";
      const codeText = el("p");
      codeText.appendChild(document.createTextNode("Go to "));
      codeText.appendChild(link);
      codeText.appendChild(document.createTextNode(" and enter code: "));
      codeText.appendChild(el("strong", undefined, device.userCode));
      deviceCodeEl.appendChild(codeText);
      deviceCodeEl.removeAttribute("hidden");
    }
    setStatus("Waiting for you to approve in the browser…");

    const deadline = Date.now() + device.expiresInSecs * 1000;
    let intervalMs = Math.max(device.intervalSecs, 1) * 1000;

    // Declared as one shared union-typed triple (same shape `runCloneOauthSignIn`
    // above uses) rather than branching on `provider` inside the loop, so
    // `refreshToken`'s type (required for GitHub, optional for GitLab) stays
    // whatever the poll result actually reported instead of being narrowed
    // away by an `if (provider === ...)` TypeScript can't tie back to it.
    let accessToken: string;
    let refreshToken: string | undefined;
    let accessTokenExpiresAt: string;

    for (;;) {
      if (generation !== cloneManualGeneration) return;
      if (Date.now() >= deadline) throw new Error(`The ${label} sign-in code expired before it was confirmed.`);
      await new Promise((resolve) => setTimeout(resolve, intervalMs));
      if (generation !== cloneManualGeneration) return;

      const result =
        provider === "github" ? await pollGithubDeviceFlow(device.deviceCode) : await pollGitlabDeviceFlow(device.deviceCode);
      if (result.outcome === "pending") continue;
      if (result.outcome === "slowDown") {
        intervalMs += 5000;
        continue;
      }
      if (result.outcome === "denied") throw new Error(`${label} sign-in was denied.`);
      if (result.outcome === "expired") throw new Error(`The ${label} sign-in code expired before it was confirmed.`);
      if (result.outcome === "error") throw new Error(result.message);

      accessToken = result.accessToken;
      refreshToken = result.refreshToken;
      accessTokenExpiresAt = result.accessTokenExpiresAt;
      break;
    }

    if (provider === "github") {
      cloneManualGithubToken = { accessToken, refreshToken: refreshToken ?? "", accessTokenExpiresAt };
    } else {
      cloneManualGitlabToken = { accessToken, refreshToken, accessTokenExpiresAt };
    }

    deviceCodeEl?.setAttribute("hidden", "");
    setStatus("Signed in. Click Clone to continue.");
  } catch (err) {
    deviceCodeEl?.setAttribute("hidden", "");
    setStatus(String(err));
  } finally {
    signinButtonEl?.removeAttribute("disabled");
  }
}

/**
 * Ticket 11 checklist item 4: the same test-fetch-before-save gate as every
 * other door into these mechanisms -- this just builds whichever
 * `CloneCredential` the selected kind needs (identical union `performClone`
 * above builds from the guided wizard's own state) and calls
 * `cloneAndOpenVault` directly. A failure leaves the dialog open with
 * nothing persisted; success tears the dialog down, opens the workspace, and
 * opens Settings so the user can confirm ticket 08's "Commit as" step (the
 * Settings modal already shows it, prefilled -- no separate step needed
 * here).
 */
async function handleCloneManualSubmitClick() {
  if (!cloneManualUrlEl || !cloneManualDestinationEl) return;
  const remoteUrl = cloneManualUrlEl.value.trim();
  const destination = cloneManualDestinationEl.value.trim();
  const kind = cloneManualCredentialKindEl?.value ?? "accessToken";

  if (!remoteUrl) {
    showCloneManualError("Repository URL is required.");
    return;
  }
  if (!destination) {
    showCloneManualError("Choose a destination folder first.");
    return;
  }

  let credential: CloneCredential;
  try {
    if (kind === "accessToken") {
      const username = cloneManualTokenUsernameEl?.value.trim() ?? "";
      const token = cloneManualTokenValueEl?.value ?? "";
      if (!username || !token) throw new Error("Username and access token are both required.");
      credential = { kind: "accessToken", username, token };
    } else if (kind === "sshKey") {
      if (!cloneManualSshKey) throw new Error("Generate or import an SSH key first.");
      credential = {
        kind: "sshKey",
        privateKeyOpenssh: cloneManualSshKey.privateKeyOpenssh,
        passphrase: cloneManualSshKey.passphrase,
      };
    } else if (kind === "githubOauth") {
      if (!cloneManualGithubToken) throw new Error("Sign in with GitHub first.");
      const installation = await checkGithubInstallation(remoteUrl, cloneManualGithubToken.accessToken);
      if (installation.status === "notInstalled") {
        throw new Error(
          `Cerebrite isn't installed on this repository yet. Install it at ${installation.installUrl}, then try again.`,
        );
      }
      credential = { kind: "githubOauth", ...cloneManualGithubToken };
    } else {
      if (!cloneManualGitlabToken) throw new Error("Sign in with GitLab first.");
      credential = { kind: "gitlabOauth", ...cloneManualGitlabToken };
    }
  } catch (err) {
    showCloneManualError(String(err));
    return;
  }

  cloneManualSubmitButtonEl?.setAttribute("disabled", "");
  if (cloneManualStatusEl) {
    cloneManualStatusEl.textContent = "Cloning…";
    cloneManualStatusEl.removeAttribute("hidden");
  }
  const generation = cloneManualGeneration;
  try {
    const result = await cloneAndOpenVault(remoteUrl, destination, credential);
    if (generation !== cloneManualGeneration) return;
    closeCloneManualDialog();
    currentVaultPath = result.vault.path;
    if (settingsVaultPathEl) settingsVaultPathEl.textContent = result.vault.path;
    showWorkspace();
    await loadPages();
    await loadTrash();
    openSettingsModal();
  } catch (err) {
    if (generation !== cloneManualGeneration) return;
    showCloneManualError(`Couldn't clone: ${String(err)}`);
  } finally {
    if (generation === cloneManualGeneration) cloneManualSubmitButtonEl?.removeAttribute("disabled");
  }
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
  selectVaultButtonEl?.addEventListener("click", handleSelectVaultClick);
  newPageButtonEl?.addEventListener("click", () => void handleNewPageClick());
  todayButtonEl?.addEventListener("click", () => void handleTodayClick());
  emptyTrashButtonEl?.addEventListener("click", () => void handleEmptyTrashClick());

  searchButtonEl?.addEventListener("click", openSearchModal);
  window.addEventListener("keydown", (event) => {
    const isSearchShortcut = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k";
    const isSettingsShortcut = (event.ctrlKey || event.metaKey) && event.key === ",";
    if (isSearchShortcut) {
      event.preventDefault();
      if (isSearchModalOpen()) {
        searchInputEl?.focus();
      } else {
        openSearchModal();
      }
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
  // Ticket 12: sidebar sync-status icon (footer + collapsed rail) and its
  // click-to-open popup -- never auto-opened.
  syncStatusButtonEl?.addEventListener("click", () => {
    if (syncStatusButtonEl) toggleSyncPopup(syncStatusButtonEl);
  });
  sidebarRailSyncStatusButtonEl?.addEventListener("click", () => {
    if (sidebarRailSyncStatusButtonEl) toggleSyncPopup(sidebarRailSyncStatusButtonEl);
  });
  document.addEventListener("click", (event) => {
    if (!isSyncPopupOpen()) return;
    const target = event.target as Node;
    if (
      syncPopupEl?.contains(target) ||
      syncStatusButtonEl?.contains(target) ||
      sidebarRailSyncStatusButtonEl?.contains(target)
    ) {
      return;
    }
    closeSyncPopup();
  });
  window.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && isSyncPopupOpen()) {
      closeSyncPopup();
    }
  });
  syncPopupSyncNowButtonEl?.addEventListener("click", () => void handleSyncNowClick());
  syncPopupSettingsLinkEl?.addEventListener("click", openSyncSettingsFromPopup);
  syncSectionConnectButtonEl?.addEventListener("click", openConnectWizard);
  void onSyncStatusChanged(applySyncIndicator);
  void refreshSyncStatus();

  connectWizardOpenButtonEl?.addEventListener("click", openConnectWizard);
  connectWizardCloseButtonEl?.addEventListener("click", closeConnectWizard);
  connectWizardBackButtonEl?.addEventListener("click", () => dispatchWizard({ type: "back" }));
  connectWizardManualButtonEl?.addEventListener("click", () => dispatchWizard({ type: "switchToManual" }));
  connectWizardOverlayEl?.addEventListener("click", (event) => {
    if (event.target === connectWizardOverlayEl) closeConnectWizard();
  });
  cloneWizardOpenButtonEl?.addEventListener("click", openCloneWizard);
  cloneWizardCloseButtonEl?.addEventListener("click", closeCloneWizard);
  cloneWizardBackButtonEl?.addEventListener("click", () => dispatchCloneWizard({ type: "back" }));
  cloneWizardManualButtonEl?.addEventListener("click", () => dispatchCloneWizard({ type: "switchToManual" }));
  cloneWizardOverlayEl?.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !isCloneWizardBusyStep(cloneWizardState.step)) {
      event.preventDefault();
      closeCloneWizard();
    }
  });

  connectWizardOverlayEl?.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      closeConnectWizard();
    }
  });

  // Ticket 11: the standalone "git clone" manual dialog's wiring.
  cloneManualOpenButtonEl?.addEventListener("click", () => openCloneManualDialog());
  cloneManualCloseButtonEl?.addEventListener("click", closeCloneManualDialog);
  cloneManualOverlayEl?.addEventListener("click", (event) => {
    if (event.target === cloneManualOverlayEl) closeCloneManualDialog();
  });
  cloneManualOverlayEl?.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      closeCloneManualDialog();
    }
  });
  cloneManualCredentialKindEl?.addEventListener("change", updateCloneManualSubformVisibility);
  cloneManualDestinationButtonEl?.addEventListener("click", () => void handleCloneManualDestinationClick());
  cloneManualSshKeyGenerateButtonEl?.addEventListener("click", () => void handleCloneManualSshKeyGenerateClick());
  cloneManualSshKeyImportButtonEl?.addEventListener("click", () => void handleCloneManualSshKeyImportClick());
  cloneManualGithubSigninButtonEl?.addEventListener("click", () => void runCloneManualOauthSignIn("github"));
  cloneManualGitlabSigninButtonEl?.addEventListener("click", () => void runCloneManualOauthSignIn("gitlab"));
  cloneManualSubmitButtonEl?.addEventListener("click", () => void handleCloneManualSubmitClick());

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

  searchInputEl?.addEventListener("input", scheduleSearch);
  searchIncludeTrashEl?.addEventListener("change", () => void runSearch());
  searchModalOverlayEl?.addEventListener("keydown", handleSearchModalKeydown);
  // Clicking the dimmed backdrop (not the modal card itself) closes it.
  searchModalOverlayEl?.addEventListener("click", (event) => {
    if (event.target === searchModalOverlayEl) closeSearchModal();
  });

  sidebarCollapseToggleEl?.addEventListener("click", () => {
    workspaceEl?.classList.add("sidebar-collapsed");
  });
  sidebarRailExpandButtonEl?.addEventListener("click", () => {
    workspaceEl?.classList.remove("sidebar-collapsed");
  });
  sidebarRailSearchButtonEl?.addEventListener("click", openSearchModal);
  sidebarRailNewPageButtonEl?.addEventListener("click", () => void handleNewPageClick());
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

  if (settings.vaultPath) {
    try {
      await openVaultAndLoad(settings.vaultPath);
      return;
    } catch (err) {
      showVaultPicker(friendlyVaultOpenError(String(err)));
      return;
    }
  }

  showVaultPicker();
}

window.addEventListener("DOMContentLoaded", () => {
  void init();
});
