// Builds the on-disk SQLite derived index (ADR-0008): a `pages` table plus an
// FTS5 virtual table for full-text search over title/body, rebuilt from
// scratch on every launch. Never stored inside the vault -- it's a derived
// index (CONTEXT.md), not part of the git-synced source of truth (ADR-0001).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use rusqlite::{params, Connection};
use walkdir::WalkDir;

use crate::frontmatter;
use crate::links;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRecord {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub body: String,
    pub modified_at: i64,
    /// Frontmatter `tags:` list (issue 09), verbatim from `ParsedPage`; folded
    /// into this page's outbound links alongside body-derived links/tags by
    /// `replace_page_links`.
    pub tags: Vec<String>,
}

/// One grouped-by-source-page backlink entry (issue 06): a page whose body
/// contains a `[[Link]]`, a `#tag`/`#[[tag]]` (issue 09, both pure sugar for
/// a link per CONTEXT.md's "Tag" definition), or a frontmatter `tags:` entry
/// targeting the page whose backlinks were requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacklinkEntry {
    pub source_id: String,
    pub source_title: String,
    pub snippet: String,
    pub modified_at: i64,
    /// The target heading's slug (ticket 07), if the link that produced this
    /// entry targeted a heading (`[[Page#Heading]]`) rather than the page
    /// itself.
    pub target_heading_slug: Option<String>,
}

/// Current wall-clock time as a unix-epoch second count, used to timestamp
/// `modified_at` on every index write that isn't the initial full rebuild
/// (which instead uses each file's on-disk mtime -- see `collect_pages`).
/// Falls back to `0` in the (practically impossible) case the system clock
/// reads before the epoch, rather than panicking.
fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A file's last-modified time as a unix-epoch second count, used as the
/// initial `modified_at` baseline when the index is rebuilt from files that
/// were never touched through this session (so a full rebuild still reflects
/// real edit recency for "most-recently-modified source first" ordering).
/// Falls back to "now" if the filesystem doesn't report a mtime.
fn file_modified_at(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or_else(now_epoch)
}

/// Walks `vault_path` for `.md` files (skipping dot-directories such as
/// `.git`), parsing frontmatter and ensuring each page has a stable id.
pub fn collect_pages(vault_path: &Path) -> Result<Vec<PageRecord>> {
    let mut pages = Vec::new();

    for entry in WalkDir::new(vault_path)
        .into_iter()
        .filter_entry(|e| {
            // Skip directories whose name starts with `.` (e.g. `.git`),
            // but never filter out the root itself.
            e.depth() == 0
                || !e
                    .file_name()
                    .to_str()
                    .map(|name| name.starts_with('.'))
                    .unwrap_or(false)
        })
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let parsed = frontmatter::parse_and_ensure_id(path)?;
        pages.push(PageRecord {
            id: parsed.id,
            title: parsed.title,
            path: path.to_path_buf(),
            body: parsed.body,
            modified_at: file_modified_at(path),
            tags: parsed.tags,
        });
    }

    Ok(pages)
}

/// Drops and recreates the derived-index schema on `conn`.
///
/// `backlinks` tracks, per link occurrence, its source page and the
/// *normalized title* of its target (issue 06) -- not a target id, since a
/// link's target may be a dynamic page with no row in `pages` at all. This
/// is a deliberate change from issue 02's original placeholder schema
/// (`source_id`/`target_id`, both ids), which couldn't represent that case.
///
/// `pages.tags` (ticket 13) carries this page's own frontmatter `tags:` list,
/// JSON-encoded, so search's tag tier can check "does this page's own tags
/// mention X" without re-parsing every file's frontmatter from disk on every
/// keystroke. This is a separate concern from `backlinks`, which records tags
/// as *outbound links* (issue 09) -- a page's tags are simultaneously "this
/// page is taggable content for tier-2 search" and "this page links to the
/// page titled by that tag," and both readings are backed by their own
/// column/table now.
pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS pages_fts;
         DROP TABLE IF EXISTS pages;
         DROP TABLE IF EXISTS backlinks;

         CREATE TABLE pages (
             id TEXT PRIMARY KEY,
             title TEXT NOT NULL,
             path TEXT NOT NULL,
             body TEXT NOT NULL,
             modified_at INTEGER NOT NULL DEFAULT 0,
             tags TEXT NOT NULL DEFAULT '[]'
         );

         CREATE VIRTUAL TABLE pages_fts USING fts5(id UNINDEXED, title, body);

         CREATE TABLE backlinks (
             source_id TEXT NOT NULL,
             target_normalized_title TEXT NOT NULL,
             snippet TEXT NOT NULL,
             target_heading_slug TEXT
         );

         CREATE INDEX idx_backlinks_target ON backlinks(target_normalized_title);
         CREATE INDEX idx_backlinks_source ON backlinks(source_id);",
    )
}

/// Rebuilds the whole derived index from the markdown files under
/// `vault_path`, in a single transaction. Returns the number of pages
/// indexed.
pub fn build_index(conn: &mut Connection, vault_path: &Path) -> Result<usize> {
    init_schema(conn)?;

    let pages = collect_pages(vault_path)?;

    let tx = conn.transaction()?;
    {
        let mut insert_page = tx.prepare(
            "INSERT INTO pages (id, title, path, body, modified_at, tags) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        let mut insert_fts =
            tx.prepare("INSERT INTO pages_fts (id, title, body) VALUES (?1, ?2, ?3)")?;

        for page in &pages {
            let tags_json = serde_json::to_string(&page.tags).unwrap_or_else(|_| "[]".to_string());
            insert_page.execute(params![
                page.id,
                page.title,
                page.path.to_string_lossy(),
                page.body,
                page.modified_at,
                tags_json,
            ])?;
            insert_fts.execute(params![page.id, page.title, page.body])?;
        }
    }
    // Link extraction (issue 06) runs as its own pass, after every page row
    // exists, so `replace_page_links` (which only needs the `backlinks`
    // table) doesn't have to worry about insert ordering relative to targets
    // -- a link's target is stored/matched purely by normalized title text,
    // never a foreign key into `pages`.
    for page in &pages {
        replace_page_links(&tx, &page.id, &page.body, &page.tags)?;
    }
    tx.commit()?;

    Ok(pages.len())
}

/// Re-extracts every `[[Link]]`/`#tag`/`#[[tag]]` occurrence out of `body`,
/// folds in `tags` (this page's frontmatter `tags:` list, issue 09), and
/// replaces `source_id`'s rows in the `backlinks` table with the fresh
/// combined set -- run every time a page's body or frontmatter changes (full
/// rebuild, single-page save, or page creation) so other pages' Backlinks
/// sections stay correct without a full index rebuild (issue 06). Tags feed
/// this table identically to bracket links -- no separate flag or column
/// distinguishes their origin.
pub fn replace_page_links(
    conn: &Connection,
    source_id: &str,
    body: &str,
    tags: &[String],
) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM backlinks WHERE source_id = ?1", params![source_id])?;

    let mut insert = conn.prepare(
        "INSERT INTO backlinks (source_id, target_normalized_title, snippet, target_heading_slug) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for link in links::extract_all_links(body, tags) {
        insert.execute(params![source_id, link.normalized_target, link.snippet, link.heading_slug])?;
    }
    Ok(())
}

/// Looks up every backlink whose target normalized title matches
/// `normalized_target`, grouped by source page (most-recently-modified
/// source first, entries within a group in document order) -- works
/// identically whether `normalized_target` belongs to a persisted page or a
/// dynamic (unmatched) one, since the lookup never requires a `pages` row
/// for the target itself.
pub fn get_backlinks(conn: &Connection, normalized_target: &str) -> rusqlite::Result<Vec<BacklinkEntry>> {
    let mut stmt = conn.prepare(
        "SELECT b.source_id, p.title, b.snippet, p.modified_at, b.target_heading_slug
         FROM backlinks b
         JOIN pages p ON p.id = b.source_id
         WHERE b.target_normalized_title = ?1
         ORDER BY p.modified_at DESC, p.id ASC, b.rowid ASC",
    )?;
    let rows = stmt.query_map(params![normalized_target], |row| {
        Ok(BacklinkEntry {
            source_id: row.get(0)?,
            source_title: row.get(1)?,
            snippet: row.get(2)?,
            modified_at: row.get(3)?,
            target_heading_slug: row.get(4)?,
        })
    })?;
    rows.collect()
}

/// Updates the derived index's `pages` and `pages_fts` rows for a single
/// page id, in place -- used after a save so the index stays in sync without
/// a full vault rebuild on every edit (per issue 03: debounce/only update on
/// explicit save, not on every keystroke).
pub fn update_page_content(
    conn: &Connection,
    id: &str,
    title: &str,
    body: &str,
    tags: &[String],
) -> rusqlite::Result<()> {
    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "UPDATE pages SET title = ?1, body = ?2, modified_at = ?3, tags = ?4 WHERE id = ?5",
        params![title, body, now_epoch(), tags_json, id],
    )?;
    conn.execute(
        "UPDATE pages_fts SET title = ?1, body = ?2 WHERE id = ?3",
        params![title, body, id],
    )?;
    // Keep the derived link graph in sync with this page's new body/tags
    // (issue 06/09) -- other pages' Backlinks sections must reflect this
    // save without a full index rebuild.
    replace_page_links(conn, id, body, tags)?;
    Ok(())
}

/// Inserts a brand-new page's row into the derived index's `pages` and
/// `pages_fts` tables -- used by the explicit "new page" action (issue 04) so
/// a freshly created file shows up without a full vault rebuild.
pub fn insert_page(
    conn: &Connection,
    id: &str,
    title: &str,
    path: &Path,
    body: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO pages (id, title, path, body, modified_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, title, path.to_string_lossy(), body, now_epoch()],
    )?;
    conn.execute(
        "INSERT INTO pages_fts (id, title, body) VALUES (?1, ?2, ?3)",
        params![id, title, body],
    )?;
    // A brand-new page has no frontmatter tags yet (`new_page_content` never
    // writes a `tags` key), so this only needs the empty slice -- but the
    // parameter exists so a future frontmatter-tags-on-create path doesn't
    // need a signature change.
    replace_page_links(conn, id, body, &[])?;
    Ok(())
}

/// Inserts a restored page's row back into the derived index, identically to
/// `insert_page` but carrying whatever tags its frontmatter has (a restored
/// page's file is untouched by trash/restore, so its tags may be non-empty,
/// unlike a brand-new page).
pub fn insert_restored_page(
    conn: &Connection,
    id: &str,
    title: &str,
    path: &Path,
    body: &str,
    tags: &[String],
) -> rusqlite::Result<()> {
    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO pages (id, title, path, body, modified_at, tags) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, title, path.to_string_lossy(), body, now_epoch(), tags_json],
    )?;
    conn.execute(
        "INSERT INTO pages_fts (id, title, body) VALUES (?1, ?2, ?3)",
        params![id, title, body],
    )?;
    replace_page_links(conn, id, body, tags)?;
    Ok(())
}

/// Removes a page's rows from the derived index entirely: `pages`,
/// `pages_fts`, and every `backlinks` row *sourced* by this page (its
/// outbound links) -- used when a page is trashed (issue 10).
///
/// This is the chosen mechanism for excluding a trashed page from "All
/// pages"/search (it's no longer in `pages` at all) *and* for excluding its
/// outbound links from other pages' backlinks sections (point 7 of the
/// ticket): removing its `backlinks` rows at trash time is simpler than
/// adding an "is this source currently trashed" check to every
/// `get_backlinks` query, and produces the same visible result. Backlink rows
/// that merely *target* this page (sourced by other, non-trashed pages) are
/// untouched, since a trashed page's own backlinks section should still work
/// once it's restored (or even while still trashed, per the ticket -- a
/// trashed page keeps resolving).
pub fn remove_page(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM backlinks WHERE source_id = ?1", params![id])?;
    conn.execute("DELETE FROM pages_fts WHERE id = ?1", params![id])?;
    conn.execute("DELETE FROM pages WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_page(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn collects_only_markdown_files_and_skips_dot_dirs() {
        let dir = tempdir().unwrap();
        write_page(dir.path(), "a.md", "---\nid: a\ntitle: A\n---\nBody A\n");
        write_page(dir.path(), "notes.txt", "not markdown");
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        write_page(dir.path().join(".git").as_path(), "config.md", "should be ignored");

        let pages = collect_pages(dir.path()).unwrap();

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id, "a");
        assert_eq!(pages[0].title, "A");
    }

    #[test]
    fn build_index_populates_pages_and_fts_tables() {
        let dir = tempdir().unwrap();
        write_page(
            dir.path(),
            "zebra.md",
            "---\nid: z1\ntitle: Zebra\n---\nStripes and hooves.\n",
        );
        write_page(
            dir.path(),
            "apple.md",
            "---\ntitle: Apple\n---\nA tasty fruit.\n",
        );

        let mut conn = Connection::open_in_memory().unwrap();
        let count = build_index(&mut conn, dir.path()).unwrap();
        assert_eq!(count, 2);

        let mut stmt = conn
            .prepare("SELECT title FROM pages ORDER BY title COLLATE NOCASE ASC")
            .unwrap();
        let titles: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(titles, vec!["Apple".to_string(), "Zebra".to_string()]);

        let hits: i64 = conn
            .query_row(
                "SELECT count(*) FROM pages_fts WHERE pages_fts MATCH 'hooves'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1);
    }

    #[test]
    fn build_index_over_500_files_is_sub_second() {
        let dir = tempdir().unwrap();
        for i in 0..500 {
            write_page(
                dir.path(),
                &format!("page-{i:04}.md"),
                &format!("---\ntitle: Page {i}\n---\nBody content for page {i}.\n"),
            );
        }

        let mut conn = Connection::open_in_memory().unwrap();
        let start = std::time::Instant::now();
        let count = build_index(&mut conn, dir.path()).unwrap();
        let elapsed = start.elapsed();

        assert_eq!(count, 500);
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "index rebuild over 500 files took {elapsed:?}, expected sub-second"
        );
    }

    #[test]
    fn update_page_content_updates_pages_and_fts_without_full_rebuild() {
        let dir = tempdir().unwrap();
        write_page(dir.path(), "one.md", "---\nid: one\ntitle: One\n---\nOriginal body.\n");

        let mut conn = Connection::open_in_memory().unwrap();
        build_index(&mut conn, dir.path()).unwrap();

        update_page_content(&conn, "one", "One", "Edited body with newword.\n", &[]).unwrap();

        let body: String = conn
            .query_row("SELECT body FROM pages WHERE id = 'one'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(body, "Edited body with newword.\n");

        let hits: i64 = conn
            .query_row(
                "SELECT count(*) FROM pages_fts WHERE pages_fts MATCH 'newword'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1);
    }

    #[test]
    fn insert_page_adds_a_new_row_to_pages_and_fts_without_full_rebuild() {
        let dir = tempdir().unwrap();
        write_page(dir.path(), "one.md", "---\nid: one\ntitle: One\n---\nOriginal body.\n");

        let mut conn = Connection::open_in_memory().unwrap();
        build_index(&mut conn, dir.path()).unwrap();

        let new_path = dir.path().join("brand-new.md");
        insert_page(&conn, "two", "Brand New", &new_path, "").unwrap();

        let count: i64 = conn.query_row("SELECT count(*) FROM pages", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 2);

        let title: String = conn
            .query_row("SELECT title FROM pages WHERE id = 'two'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(title, "Brand New");

        let hits: i64 = conn
            .query_row(
                "SELECT count(*) FROM pages_fts WHERE pages_fts MATCH 'Brand'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1);
    }

    #[test]
    fn build_index_populates_backlinks_from_wiki_links_in_bodies() {
        let dir = tempdir().unwrap();
        write_page(
            dir.path(),
            "a.md",
            "---\nid: a\ntitle: A\n---\nA links to [[B]] right here.\n",
        );
        write_page(dir.path(), "b.md", "---\nid: b\ntitle: B\n---\nNo outgoing links.\n");

        let mut conn = Connection::open_in_memory().unwrap();
        build_index(&mut conn, dir.path()).unwrap();

        let entries = get_backlinks(&conn, "b").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_id, "a");
        assert!(entries[0].snippet.contains("[[B]]"));

        assert!(get_backlinks(&conn, "a").unwrap().is_empty());
    }

    #[test]
    fn backlinks_carry_the_target_heading_slug_when_present() {
        let dir = tempdir().unwrap();
        write_page(
            dir.path(),
            "a.md",
            "---\nid: a\ntitle: A\n---\nSee [[B#Some Heading]] and also [[B]] plain.\n",
        );
        write_page(dir.path(), "b.md", "---\nid: b\ntitle: B\n---\n");

        let mut conn = Connection::open_in_memory().unwrap();
        build_index(&mut conn, dir.path()).unwrap();

        let mut entries = get_backlinks(&conn, "b").unwrap();
        entries.sort_by(|a, b| a.target_heading_slug.cmp(&b.target_heading_slug));

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].target_heading_slug, None);
        assert_eq!(entries[1].target_heading_slug.as_deref(), Some("some-heading"));
    }

    #[test]
    fn replace_page_links_updates_backlinks_in_place() {
        let dir = tempdir().unwrap();
        write_page(dir.path(), "a.md", "---\nid: a\ntitle: A\n---\nLinks to [[B]].\n");
        write_page(dir.path(), "b.md", "---\nid: b\ntitle: B\n---\n");
        write_page(dir.path(), "c.md", "---\nid: c\ntitle: C\n---\n");

        let mut conn = Connection::open_in_memory().unwrap();
        build_index(&mut conn, dir.path()).unwrap();
        assert_eq!(get_backlinks(&conn, "b").unwrap().len(), 1);

        replace_page_links(&conn, "a", "Now links to [[C]] instead.\n", &[]).unwrap();

        assert!(get_backlinks(&conn, "b").unwrap().is_empty());
        let entries = get_backlinks(&conn, "c").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_id, "a");
    }

    #[test]
    fn build_index_populates_backlinks_from_frontmatter_tags() {
        let dir = tempdir().unwrap();
        write_page(
            dir.path(),
            "a.md",
            "---\nid: a\ntitle: A\ntags: [foo, bar]\n---\nNo inline tags here.\n",
        );
        write_page(dir.path(), "foo.md", "---\nid: f\ntitle: Foo\n---\n");

        let mut conn = Connection::open_in_memory().unwrap();
        build_index(&mut conn, dir.path()).unwrap();

        let entries = get_backlinks(&conn, "foo").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_id, "a");
    }

    #[test]
    fn build_index_populates_backlinks_from_inline_hash_tags() {
        let dir = tempdir().unwrap();
        write_page(dir.path(), "a.md", "---\nid: a\ntitle: A\n---\nThis is #foo tagged.\n");
        write_page(dir.path(), "foo.md", "---\nid: f\ntitle: Foo\n---\n");

        let mut conn = Connection::open_in_memory().unwrap();
        build_index(&mut conn, dir.path()).unwrap();

        let entries = get_backlinks(&conn, "foo").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_id, "a");
    }

    #[test]
    fn inline_tag_and_frontmatter_tag_both_feed_the_same_target_backlinks() {
        let dir = tempdir().unwrap();
        write_page(
            dir.path(),
            "a.md",
            "---\nid: a\ntitle: A\n---\nInline #shared tag here.\n",
        );
        write_page(
            dir.path(),
            "b.md",
            "---\nid: b\ntitle: B\ntags: [shared]\n---\nNo inline tag.\n",
        );

        let mut conn = Connection::open_in_memory().unwrap();
        build_index(&mut conn, dir.path()).unwrap();

        let entries = get_backlinks(&conn, "shared").unwrap();
        let mut source_ids: Vec<&str> = entries.iter().map(|e| e.source_id.as_str()).collect();
        source_ids.sort();
        assert_eq!(source_ids, vec!["a", "b"]);
    }

    #[test]
    fn remove_page_deletes_pages_fts_and_its_outbound_backlinks_only() {
        let dir = tempdir().unwrap();
        write_page(dir.path(), "a.md", "---\nid: a\ntitle: A\n---\nLinks to [[B]].\n");
        write_page(dir.path(), "b.md", "---\nid: b\ntitle: B\n---\nLinks to [[A]].\n");

        let mut conn = Connection::open_in_memory().unwrap();
        build_index(&mut conn, dir.path()).unwrap();
        assert_eq!(get_backlinks(&conn, "b").unwrap().len(), 1);
        assert_eq!(get_backlinks(&conn, "a").unwrap().len(), 1);

        remove_page(&conn, "a").unwrap();

        let count: i64 = conn.query_row("SELECT count(*) FROM pages WHERE id = 'a'", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0);
        let fts_count: i64 = conn
            .query_row("SELECT count(*) FROM pages_fts WHERE id = 'a'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fts_count, 0);

        // A's outbound link (to B) is gone -- B's backlinks section no
        // longer lists A.
        assert!(get_backlinks(&conn, "b").unwrap().is_empty());
        // But A's own backlinks (sourced by B, which is not trashed) remain.
        assert_eq!(get_backlinks(&conn, "a").unwrap().len(), 1);
    }

    #[test]
    fn insert_restored_page_reinstates_a_removed_page_with_its_tags() {
        let dir = tempdir().unwrap();
        write_page(dir.path(), "a.md", "---\nid: a\ntitle: A\ntags: [foo]\n---\nBody.\n");
        write_page(dir.path(), "foo.md", "---\nid: f\ntitle: Foo\n---\n");

        let mut conn = Connection::open_in_memory().unwrap();
        build_index(&mut conn, dir.path()).unwrap();
        remove_page(&conn, "a").unwrap();
        assert!(get_backlinks(&conn, "foo").unwrap().is_empty());

        let path = dir.path().join("a.md");
        insert_restored_page(&conn, "a", "A", &path, "Body.\n", &["foo".to_string()]).unwrap();

        let count: i64 = conn.query_row("SELECT count(*) FROM pages WHERE id = 'a'", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
        let entries = get_backlinks(&conn, "foo").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source_id, "a");
    }

    #[test]
    fn build_index_is_idempotent_across_rebuilds() {
        let dir = tempdir().unwrap();
        write_page(dir.path(), "one.md", "---\nid: one\n---\nHello\n");

        let mut conn = Connection::open_in_memory().unwrap();
        build_index(&mut conn, dir.path()).unwrap();
        let count = build_index(&mut conn, dir.path()).unwrap();

        assert_eq!(count, 1);
    }
}
