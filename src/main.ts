import {
  getRememberedVault,
  pickVaultFolder,
  openVault,
  listPages,
  getPage,
  type PageSummary,
} from "./vault-api";

const vaultPickerEl = document.querySelector<HTMLElement>("#vault-picker");
const vaultPickerErrorEl = document.querySelector<HTMLElement>("#vault-picker-error");
const selectVaultButtonEl = document.querySelector<HTMLButtonElement>("#select-vault-button");

const workspaceEl = document.querySelector<HTMLElement>("#workspace");
const pageListEl = document.querySelector<HTMLUListElement>("#page-list");

const pageViewEmptyEl = document.querySelector<HTMLElement>("#page-view-empty");
const pageArticleEl = document.querySelector<HTMLElement>("#page-article");
const pageTitleEl = document.querySelector<HTMLElement>("#page-title");
const pageBodyEl = document.querySelector<HTMLElement>("#page-body");

let selectedPageId: string | null = null;

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
  selectedPageId = id;
  pageListEl
    ?.querySelectorAll<HTMLButtonElement>("button")
    .forEach((btn) => btn.classList.toggle("active", btn.dataset.pageId === id));

  const page = await getPage(id);
  if (pageTitleEl) pageTitleEl.textContent = page.title;
  if (pageBodyEl) pageBodyEl.innerHTML = page.html;
  pageViewEmptyEl?.setAttribute("hidden", "");
  pageArticleEl?.removeAttribute("hidden");
}

async function loadPages() {
  const pages = await listPages();
  renderPageList(pages);
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
