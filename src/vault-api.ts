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
///
/// `headingSlug` (ticket 07) is set when the resolved link included a
/// `#Heading` fragment -- the caller uses it to scroll to that heading after
/// navigating. Per the ticket, a heading-specific dynamic target isn't a
/// thing (only whole pages materialize, ADR-0009), so `headingSlug` on a
/// dynamic resolution is carried through for consistency only and never
/// acted on.
export type PageResolution =
  | {
      kind: "persisted";
      id: string;
      title: string;
      body: string;
      html: string;
      headingSlug?: string | null;
      /// True when this page currently sits in `.cerebrite/trash/` (issue
      /// 10) -- the page still resolves/renders, but the UI should show an
      /// "in trash" indicator with an inline restore action.
      inTrash?: boolean;
      /// The trashed file's filename under `.cerebrite/trash/`, needed by
      /// the "Restore" action. Present only when `inTrash` is true.
      trashedFilename?: string | null;
    }
  | { kind: "dynamic"; normalizedTitle: string; headingSlug?: string | null };

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

/// One grouped-by-source-page backlink entry (issue 06), as returned by
/// `get_backlinks`. `targetHeadingSlug` (ticket 07) is set when the link that
/// produced this entry targeted a heading (`[[Page#Heading]]`) rather than
/// the page itself.
export interface BacklinkEntry {
  sourceId: string;
  sourceTitle: string;
  snippet: string;
  modifiedAt: number;
  targetHeadingSlug?: string | null;
}

/// Returns every backlink pointing at the page titled `title`, grouped by
/// source page (most-recently-modified source first). Works identically for
/// a persisted page's own title and a dynamic page's normalized title (issue
/// 06) -- the lookup happens purely by normalized title text on the Rust
/// side, so a dynamic page (no id, no file) can still have backlinks.
export function getBacklinks(title: string): Promise<BacklinkEntry[]> {
  return invoke("get_backlinks", { title });
}

/// Materializes a dynamic page into a real persisted file the instant it
/// receives its first write (ADR-0009): mints an id, derives a slug
/// filename, writes frontmatter, then writes `markdownBody` as the body --
/// using the exact same creation mechanics as `create_page`, not a
/// from-scratch file write, so the frontmatter/id survive identically.
export function materializeAndSavePage(title: string, markdownBody: string): Promise<PageSummary> {
  return invoke("materialize_and_save_page", { title, markdownBody });
}

/// One trashed page (issue 10), as returned by `listTrashedPages` for the
/// "Trash" view. `trashedFilename` is the manifest key needed by `restorePage`.
export interface TrashedPageSummary {
  id: string;
  title: string;
  trashedFilename: string;
  originalRelativePath: string;
}

/// Explicit "delete page" action (issue 10 / ADR-0010): moves a persisted
/// page's file into `.cerebrite/trash/`, git-committed as a move like any
/// other edit, and removes it from "All pages"/search.
export function trashPage(id: string): Promise<void> {
  return invoke("trash_page", { id });
}

/// Explicit "restore" action (issue 10): moves a trashed page's file back to
/// its original path (preserving its frontmatter id) and re-adds it to "All
/// pages"/search.
export function restorePage(trashedFilename: string): Promise<PageSummary> {
  return invoke("restore_page", { trashedFilename });
}

/// Explicit "empty trash" action (issue 10): permanently deletes every file
/// under `.cerebrite/trash/`. There is no other purge path.
export function emptyTrash(): Promise<void> {
  return invoke("empty_trash");
}

/// Lists every page currently sitting in `.cerebrite/trash/` (issue 10).
export function listTrashedPages(): Promise<TrashedPageSummary[]> {
  return invoke("list_trashed_pages");
}
