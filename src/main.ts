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
  type PageSummary,
  type PageResolution,
  type TrashedPageSummary,
  type SearchResult,
  type Theme,
} from "./vault-api";
import { PageEditor } from "./page-editor";
import { humanizeHeadingSlug } from "./heading-slug";

// Autosave debounce: fires this long after the last edit with no further
// typing, rather than on every keystroke or requiring an explicit "save"
// action -- see issue 03's save-trigger note.
const AUTOSAVE_DEBOUNCE_MS = 1500;

const vaultPickerEl = document.querySelector<HTMLElement>("#vault-picker");
const vaultPickerErrorEl = document.querySelector<HTMLElement>("#vault-picker-error");
const selectVaultButtonEl = document.querySelector<HTMLButtonElement>("#select-vault-button");

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
  // rather than duplicating it.
  recordRecentOpen(
    resolution.kind === "persisted"
      ? { key: `p:${resolution.id}`, title: resolution.title, kind: "persisted", pageId: resolution.id }
      : { key: `d:${resolution.normalizedTitle}`, title: resolution.normalizedTitle, kind: "dynamic" }
  );

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
  if (!window.confirm("Move this page to trash?")) return;

  try {
    await trashPage(id);
    currentPage = null;
    pageArticleEl?.setAttribute("hidden", "");
    pageViewEmptyEl?.removeAttribute("hidden");
    await loadPages();
    await loadTrash();
  } catch (err) {
    window.alert(String(err));
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
    window.alert("Title cannot be empty.");
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
    if (!window.confirm(`This will also update ${affectedCount} ${linkWord} in other pages. Continue?`)) {
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
    window.alert(String(err));
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
    window.alert(String(err));
  }
}

/** Explicit "empty trash" action (issue 10): permanently deletes every trashed page. There is no other purge path. */
async function handleEmptyTrashClick() {
  if (!window.confirm("Permanently delete all trashed pages? This cannot be undone.")) return;

  try {
    await emptyTrash();
    if (currentPage?.kind === "persisted" && currentPage.inTrash) {
      currentPage = null;
      pageArticleEl?.setAttribute("hidden", "");
      pageViewEmptyEl?.removeAttribute("hidden");
    }
    await loadTrash();
  } catch (err) {
    window.alert(String(err));
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
    window.alert("Title cannot be empty.");
    return;
  }

  try {
    const summary = await createPage(trimmed);
    await loadPages();
    await selectPage(summary.id);
  } catch (err) {
    window.alert(String(err));
  }
}

async function openVaultAndLoad(path: string) {
  await openVault(path);
  currentVaultPath = path;
  if (settingsVaultPathEl) settingsVaultPathEl.textContent = path;
  showWorkspace();
  await loadPages();
  await loadTrash();
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
    showVaultPicker(String(err));
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
    window.alert(String(err));
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
    window.alert(String(err));
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
    window.alert(String(err));
  }
}

function openSettingsModal() {
  if (!settingsModalOverlayEl) return;
  if (settingsVaultPathEl) settingsVaultPathEl.textContent = currentVaultPath ?? "";
  settingsModalOverlayEl.removeAttribute("hidden");
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
      showVaultPicker(String(err));
      return;
    }
  }

  showVaultPicker();
}

window.addEventListener("DOMContentLoaded", () => {
  void init();
});
