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

/// Result of resolving a `[[Link]]` title (issue 05): either an existing
/// persisted page (with its content, ready to display), or a dynamic page
/// that has no backing file yet -- identified purely by its normalized
/// title (see src-tauri/src/frontmatter.rs's `normalize_title` for the
/// exact normalization rule).
export type PageResolution =
  | { kind: "persisted"; id: string; title: string; body: string; html: string }
  | { kind: "dynamic"; normalizedTitle: string };

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

/// Explicit "new page" action (issue 04): mints a frontmatter id, derives a
/// slugified filename from `title`, and writes a frontmatter-only file
/// (empty body). Rejects with an in-app error message (not auto-suffixed) if
/// a page with that title already exists.
export function createPage(title: string): Promise<PageSummary> {
  return invoke("create_page", { title });
}

/// Resolves a clicked `[[Link]]`'s raw title to either an existing
/// persisted page or a not-yet-materialized dynamic page (issue 05). Used
/// both for link-click navigation and (implicitly, by the caller) to decide
/// whether a subsequent save must materialize the page first.
export function resolvePage(title: string): Promise<PageResolution> {
  return invoke("resolve_page", { title });
}

/// Materializes a dynamic page into a real persisted file the instant it
/// receives its first write (ADR-0009): mints an id, derives a slug
/// filename, writes frontmatter, then writes `markdownBody` as the body --
/// using the exact same creation mechanics as `create_page`, not a
/// from-scratch file write, so the frontmatter/id survive identically.
export function materializeAndSavePage(title: string, markdownBody: string): Promise<PageSummary> {
  return invoke("materialize_and_save_page", { title, markdownBody });
}
