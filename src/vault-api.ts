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

/// Result of an explicit "rename page" action: the renamed page's fresh
/// `PageSummary`, plus the ids of every *other* page whose body was rewritten
/// to keep its inbound `[[Old Title]]`-style links pointing at the new
/// title -- used to decide whether the currently-open page (if any) needs its
/// in-memory editor content reloaded.
export interface RenamePageResult {
  id: string;
  title: string;
  affectedPageIds: string[];
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

/// Forced color-scheme preference (Settings UI): "system" defers to
/// `prefers-color-scheme`, "light"/"dark" override it.
export type Theme = "system" | "light" | "dark";

/// The app's persisted settings (Settings UI): `vaultPath` is the remembered
/// vault folder (`null` on first run), `theme` the forced color scheme.
export interface Settings {
  vaultPath: string | null;
  theme: Theme;
}

export function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

/// Persists the forced color-scheme preference. Applying it to the page is
/// the caller's job (see `applyTheme` in main.ts).
export function setTheme(theme: Theme): Promise<void> {
  return invoke("set_theme", { theme });
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

/// Explicit "rename page" action: re-slugifies `newTitle` into a new
/// filename and rewrites every other page's (and this page's own)
/// `[[Old Title]]`-style links to the new title, all committed together.
/// Rejects with an in-app error message (not auto-suffixed) if a *different*
/// page already has that title.
export function renamePage(id: string, newTitle: string): Promise<RenamePageResult> {
  return invoke("rename_page", { id, newTitle });
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

/// One search result row (ticket 13), as returned by `searchPages`: one per
/// page, tagged with which strict tier matched it (1 = title, 2 = tag, 3 =
/// BM25 body). `matchedTag` is set only for a tier-2 hit (the frontend
/// renders it as a chip); `snippet` otherwise holds the title (tier 1) or a
/// best-matching-section body excerpt with ``/`` marking the
/// highlighted span (tier 3) -- see search.rs's `body_fts_matches`.
export interface SearchResult {
  id: string;
  title: string;
  tier: 1 | 2 | 3;
  snippet: string;
  inTrash: boolean;
  matchedTag?: string | null;
}

/// Full search (ticket 13): one query against title, tags, and body, ranked
/// in three strict tiers with one row per page. `includeTrash`, when true,
/// also searches trashed pages directly (they aren't indexed) and flags them
/// `inTrash` in the results.
export function searchPages(query: string, includeTrash: boolean): Promise<SearchResult[]> {
  return invoke("search_pages", { query, includeTrash });
}

/// Ticket 04's minimal/raw "connect with an access token" entry point -- the
/// generic HTTPS path for any git host that isn't GitHub/GitLab sign-in
/// (Bitbucket Cloud, Gitea, Forgejo, Codeberg, a bare HTTPS remote). The
/// backend runs a real test fetch with the given credentials *before* saving
/// anything; a rejected/unreachable test fetch rejects this promise and
/// leaves nothing persisted. On success, background sync picks up the new
/// connection on its own -- no further prompting.
export function connectAccessToken(remoteUrl: string, username: string, token: string): Promise<void> {
  return invoke("connect_access_token", { remoteUrl, username, token });
}

/// Ticket 05's SSH key connection path. `SshKeyInfo` is what
/// `generateSshKey`/`importSshKey` hand back: the public half to show (copy
/// button + link to the provider's "add SSH key" page, per the ticket) plus
/// everything `connectSshKey` needs. Nothing is persisted by generating or
/// importing alone -- only `connectSshKey` succeeding (a real test fetch,
/// same gate as `connectAccessToken`) stores anything.
export interface SshKeyInfo {
  privateKeyOpenssh: string;
  passphrase?: string;
  publicKeyOpenssh: string;
  fingerprintSha256: string;
  hadPassphrase: boolean;
}

/// Generates a fresh in-app ed25519 key (no passphrase, the default).
export function generateSshKey(): Promise<SshKeyInfo> {
  return invoke("generate_ssh_key");
}

/// Validates an imported private key -- and, if it's passphrase-protected,
/// that `passphrase` actually unlocks it -- without persisting anything yet.
export function importSshKey(privateKeyOpenssh: string, passphrase?: string): Promise<SshKeyInfo> {
  return invoke("import_ssh_key", { privateKeyOpenssh, passphrase });
}

/// Connects with an SSH key (generated or imported): runs a real test fetch
/// -- including this ticket's host-key check -- before persisting anything.
/// A rejected/unconfirmed/mismatched host key surfaces as a rejected
/// promise naming the host and fingerprint; confirm it via
/// `confirmSshHostKey` (only after showing it to the user and getting
/// explicit confirmation) and call this again.
export function connectSshKey(remoteUrl: string, privateKeyOpenssh: string, passphrase?: string): Promise<void> {
  return invoke("connect_ssh_key", { remoteUrl, privateKeyOpenssh, passphrase });
}

/// Persists explicit TOFU confirmation of `fingerprint` for `host` to
/// Cerebrite's own known_hosts-equivalent file. Never call this without
/// having actually shown the fingerprint to the user first.
export function confirmSshHostKey(host: string, fingerprint: string): Promise<void> {
  return invoke("confirm_ssh_host_key", { host, fingerprint });
}
