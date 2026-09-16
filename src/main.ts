import {
  getRememberedVault,
  pickVaultFolder,
  openVault,
  listPages,
  getPage,
  savePage,
  createPage,
  type PageSummary,
} from "./vault-api";
import { PageEditor } from "./page-editor";

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

const pageViewEmptyEl = document.querySelector<HTMLElement>("#page-view-empty");
const pageArticleEl = document.querySelector<HTMLElement>("#page-article");
const pageTitleEl = document.querySelector<HTMLElement>("#page-title");
const pageBodyEl = document.querySelector<HTMLElement>("#page-body");

let selectedPageId: string | null = null;
let saveTimer: ReturnType<typeof setTimeout> | null = null;
let pageEditor: PageEditor | null = null;

/** Cancels any pending debounced autosave and immediately saves `id` with `markdown`, if not already saved. */
async function flushSave(id: string, markdown: string) {
  if (saveTimer !== null) {
    clearTimeout(saveTimer);
    saveTimer = null;
  }
  try {
    await savePage(id, markdown);
  } catch (err) {
    console.error("Failed to save page", id, err);
  }
}

function scheduleAutosave(id: string, markdown: string) {
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    void flushSave(id, markdown);
  }, AUTOSAVE_DEBOUNCE_MS);
}

/** Flushes any pending save for the page currently loaded in the editor before switching away from it. */
async function flushPendingSaveForCurrentPage() {
  if (saveTimer === null || selectedPageId === null || !pageEditor) return;
  const markdown = pageEditor.getMarkdown();
  if (markdown === null) return;
  await flushSave(selectedPageId, markdown);
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
    button.classList.toggle("active", page.id === selectedPageId);
    button.addEventListener("click", () => selectPage(page.id));
    li.appendChild(button);
    pageListEl.appendChild(li);
  }
}

async function selectPage(id: string) {
  if (id === selectedPageId) return;

  // Persist any unsaved edit on the page we're leaving before switching.
  await flushPendingSaveForCurrentPage();

  selectedPageId = id;
  pageListEl
    ?.querySelectorAll<HTMLButtonElement>("button")
    .forEach((btn) => btn.classList.toggle("active", btn.dataset.pageId === id));

  const page = await getPage(id);
  if (pageTitleEl) pageTitleEl.textContent = page.title;
  pageViewEmptyEl?.setAttribute("hidden", "");
  pageArticleEl?.removeAttribute("hidden");

  if (pageBodyEl) {
    if (!pageEditor) {
      // `selectedPageId` is read at callback time (not captured here), so
      // this single instance stays correct across page switches.
      pageEditor = new PageEditor(pageBodyEl, (markdown) => {
        if (selectedPageId) scheduleAutosave(selectedPageId, markdown);
      });
    }
    await pageEditor.load(page.body);
  }
}

async function loadPages() {
  const pages = await listPages();
  renderPageList(pages);
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
