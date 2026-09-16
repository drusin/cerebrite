// Builds the on-disk SQLite derived index (ADR-0008): a `pages` table plus an
// FTS5 virtual table for full-text search over title/body, rebuilt from
// scratch on every launch. Never stored inside the vault -- it's a derived
// index (CONTEXT.md), not part of the git-synced source of truth (ADR-0001).

use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::{params, Connection};
use walkdir::WalkDir;

use crate::frontmatter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRecord {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub body: String,
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
        });
    }

    Ok(pages)
}

/// Drops and recreates the derived-index schema on `conn`.
pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS pages_fts;
         DROP TABLE IF EXISTS pages;
         DROP TABLE IF EXISTS backlinks;

         CREATE TABLE pages (
             id TEXT PRIMARY KEY,
             title TEXT NOT NULL,
             path TEXT NOT NULL,
             body TEXT NOT NULL
         );

         CREATE VIRTUAL TABLE pages_fts USING fts5(id UNINDEXED, title, body);

         CREATE TABLE backlinks (
             source_id TEXT NOT NULL,
             target_id TEXT NOT NULL
         );",
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
        let mut insert_page =
            tx.prepare("INSERT INTO pages (id, title, path, body) VALUES (?1, ?2, ?3, ?4)")?;
        let mut insert_fts =
            tx.prepare("INSERT INTO pages_fts (id, title, body) VALUES (?1, ?2, ?3)")?;

        for page in &pages {
            insert_page.execute(params![
                page.id,
                page.title,
                page.path.to_string_lossy(),
                page.body
            ])?;
            insert_fts.execute(params![page.id, page.title, page.body])?;
        }
    }
    tx.commit()?;

    Ok(pages.len())
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
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE pages SET title = ?1, body = ?2 WHERE id = ?3",
        params![title, body, id],
    )?;
    conn.execute(
        "UPDATE pages_fts SET title = ?1, body = ?2 WHERE id = ?3",
        params![title, body, id],
    )?;
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

        update_page_content(&conn, "one", "One", "Edited body with newword.\n").unwrap();

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
    fn build_index_is_idempotent_across_rebuilds() {
        let dir = tempdir().unwrap();
        write_page(dir.path(), "one.md", "---\nid: one\n---\nHello\n");

        let mut conn = Connection::open_in_memory().unwrap();
        build_index(&mut conn, dir.path()).unwrap();
        let count = build_index(&mut conn, dir.path()).unwrap();

        assert_eq!(count, 1);
    }
}
