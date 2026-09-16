// Thin typed wrapper around the Rust core's vault/page Tauri commands.
import { invoke } from "@tauri-apps/api/core";

export interface VaultInfo {
  path: string;
  pageCount: number;
}

export interface PageSummary {
  id: string;
  title: string;
}

export interface PageContent {
  id: string;
  title: string;
  body: string;
  html: string;
}

export function getRememberedVault(): Promise<string | null> {
  return invoke("get_remembered_vault");
}

export function pickVaultFolder(): Promise<string | null> {
  return invoke("pick_vault_folder");
}

export function openVault(path: string): Promise<VaultInfo> {
  return invoke("open_vault", { path });
}

export function listPages(): Promise<PageSummary[]> {
  return invoke("list_pages");
}

export function getPage(id: string): Promise<PageContent> {
  return invoke("get_page", { id });
}

/// Saves a page's edited markdown body. The frontmatter block is preserved
/// exactly by the Rust side (src-tauri/src/frontmatter.rs) -- the editor
/// only ever sends the body.
export function savePage(id: string, markdownBody: string): Promise<void> {
  return invoke("save_page", { id, markdownBody });
}
