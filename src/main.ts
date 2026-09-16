import {
  getRememberedVault,
  pickVaultFolder,
  openVault,
  listPages,
  getPage,
  savePage,
  createPage,
  resolvePage,
  materializeAndSavePage,
  getBacklinks,
  trashPage,
  restorePage,
  emptyTrash,
  listTrashedPages,
  type PageSummary,
  type PageResolution,
  type TrashedPageSummary,
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
const pageListEl = document.querySelector<HTMLUListElement>("#page-list");
const newPageButtonEl = document.querySelector<HTMLButtonElement>("#new-page-button");
const todayButtonEl = document.querySelector<HTMLButtonElement>("#today-button");

const pageViewEmptyEl = document.querySelector<HTMLElement>("#page-view-empty");
const pageArticleEl = document.querySelector<HTMLElement>("#page-article");
const pageTitleEl = document.querySelector<HTMLElement>("#page-title");
const pageBodyEl = document.querySelector<HTMLElement>("#page-body");
const backlinksListEl = document.querySelector<HTMLUListElement>("#backlinks-list");
const backlinksEmptyEl = document.querySelector<HTMLElement>("#backlinks-empty");
const deletePageButtonEl = document.querySelector<HTMLButtonElement>("#delete-page-button");
const pageTrashBannerEl = document.querySelector<HTMLElement>("#page-trash-banner");
const restorePageButtonEl = document.querySelector<HTMLButtonElement>("#restore-page-button");
const trashListEl = document.querySelector<HTMLUListElement>("#trash-list");
const emptyTrashButtonEl = document.querySelector<HTMLButtonElement>("#empty-trash-button");

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
      const summary = await materializeAndSavePage(currentPage.title, markdown);
      currentPage = { kind: "persisted", id: summary.id, inTrash: false, trashedFilename: null };
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
  pageListEl?.querySelectorAll<HTMLButtonElement>("button").forEach((btn) => {
    const isActive = currentPage?.kind === "persisted" && btn.dataset.pageId === currentPage.id;
    btn.classList.toggle("active", isActive);
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
  showWorkspace();
  await loadPages();
  await loadTrash();
}

async function handleSelectVaultClick() {
  const path = await pickVaultFolder();
  if (!path) return; // user cancelled

  try {
    await openVaultAndLoad(path);
  } catch (err) {
    showVaultPicker(String(err));
  }
}

async function init() {
  selectVaultButtonEl?.addEventListener("click", handleSelectVaultClick);
  newPageButtonEl?.addEventListener("click", () => void handleNewPageClick());
  todayButtonEl?.addEventListener("click", () => void handleTodayClick());
  emptyTrashButtonEl?.addEventListener("click", () => void handleEmptyTrashClick());

  const remembered = await getRememberedVault();
  if (remembered) {
    try {
      await openVaultAndLoad(remembered);
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
