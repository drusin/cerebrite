// Full search (ticket 13 / issue 11-search-ui-ux): one query against title,
// tags, and body, ranked in three STRICT tiers -- title match, then tag
// match, then BM25-ranked body match -- with no cross-tier blending, and one
// row per page (a page's best tier wins over any weaker match it also has).
//
// Tag-search ambiguity, resolved: per CONTEXT.md/issue 09, a tag is pure
// syntax sugar for a link to a page titled the tag text -- there is no
// separate "tag identity." That still leaves two readings of "search finds
// pages via their tags":
//   (a) searching "foo" finds the page *titled* "foo" (already covered for
//       free by the title tier, since a tag and a page title share the same
//       normalized-text space), or
//   (b) searching "foo" finds *other* pages that carry `#foo`/`tags: [foo]`
//       on themselves, i.e. a page's own tags contribute to what makes THAT
//       PAGE findable, on top of its title/body.
// This module implements (b): a page's frontmatter tags are folded into its
// own searchable content as tier 2, between the title and body tiers. This
// is the "more useful" reading called for by the ticket when read against
// the prototype spec: the tier list is explicitly "title -> tag -> body," a
// tier that only ever fired identically to the title tier (reading (a) alone)
// would be a dead tier that could never rank differently from tier 1 -- it
// only earns its place in the ordering under reading (b), where a page
// *tagged* `#project` surfaces above a page that merely *mentions* "project"
// in its body, without also being titled or exactly-tagged something that
// duplicates the title tier. Reading (a) still holds too, unaffected: a
// search for "foo" also finds the page titled "foo" via the title tier,
// same as ever, and a tag's own backlink behavior (ticket 09) is completely
// unchanged by any of this -- this module only adds a new *read* path over
// `pages.tags`, it writes nothing to `backlinks`.

use std::collections::HashSet;
use std::path::Path;

use anyhow::Result;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::links::{ceil_char_boundary, floor_char_boundary};
use crate::{frontmatter, trash};

/// One search result row (ticket 13): one per page, tagged with which tier
/// matched (1 = title, 2 = tag, 3 = BM25 body), a display snippet, and
/// whether the page currently sits in trash (only possible when the caller
/// asked to include trash).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub tier: u8,
    pub snippet: String,
    pub in_trash: bool,
    /// The matched tag text (tier 2 only) -- the frontend renders this as a
    /// chip instead of a body snippet, per the prototype spec.
    pub matched_tag: Option<String>,
}

/// Internal scoring wrapper: carries the within-tier sort key (`rank`, lower
/// is better in every tier -- 0/1/2 for title/tag rank, and BM25's own
/// lower-is-better scale for the body tier) alongside the public result
/// fields, so sorting doesn't need a second pass.
#[derive(Debug, Clone)]
struct Scored {
    id: String,
    title: String,
    tier: u8,
    rank: f64,
    snippet: String,
    in_trash: bool,
    matched_tag: Option<String>,
}

impl Scored {
    fn into_result(self) -> SearchResult {
        SearchResult {
            id: self.id,
            title: self.title,
            tier: self.tier,
            snippet: self.snippet,
            in_trash: self.in_trash,
            matched_tag: self.matched_tag,
        }
    }
}

/// How many plain-text characters of context to keep on each side of a match
/// in the naive trashed-page snippet fallback (mirrors `links.rs`'s
/// `SNIPPET_RADIUS` for the same reason: a short, readable preview).
const NAIVE_SNIPPET_RADIUS: usize = 40;

/// Runs the tiered search described at the top of this file.
///
/// `vault_path` is only needed when `include_trash` is true (trashed pages
/// aren't indexed, so their content has to be read straight off disk); pass
/// `None` when trash is excluded, or when no vault is open at all in a
/// context that can't reach trashed pages anyway.
pub fn search_pages(
    conn: &Connection,
    vault_path: Option<&Path>,
    query: &str,
    include_trash: bool,
) -> Result<Vec<SearchResult>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let query_lower = trimmed.to_lowercase();

    let mut results: Vec<Scored> = Vec::new();

    // Tiers 1/2: title and tag match, done in Rust over every persisted
    // (never-trashed -- ticket 10 already keeps trashed pages out of
    // `pages`) page's own title/tags. The corpus is small (~500 pages per
    // the index-build perf test), so a single full scan per search is cheap
    // and far simpler than round-tripping through FTS query syntax for
    // matches that don't need tokenization or BM25 at all.
    {
        let mut stmt = conn.prepare("SELECT id, title, tags FROM pages")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let tags_json: String = row.get(2)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            if let Some(scored) = title_or_tag_match(&id, &title, &tags, &query_lower, false) {
                results.push(scored);
            }
        }
    }

    // Tier 3: BM25-ranked body match via FTS5, for whatever pages didn't
    // already earn a stronger (tier 1/2) match above -- "no cross-tier
    // blending" means a page that matched on title never also shows up
    // again lower down as a body hit.
    let already_matched: HashSet<String> = results.iter().map(|r| r.id.clone()).collect();
    if let Ok(body_hits) = body_fts_matches(conn, trimmed) {
        for (id, title, bm25_rank, snippet) in body_hits {
            if already_matched.contains(&id) {
                continue;
            }
            results.push(Scored {
                id,
                title,
                tier: 3,
                rank: bm25_rank,
                snippet,
                in_trash: false,
                matched_tag: None,
            });
        }
    }

    // Trash: excluded by default (ticket 10's trashed pages already have no
    // `pages`/`pages_fts` rows, so the scan above never sees them at all).
    // When the caller opts in, fall back to a direct, un-indexed read of
    // each trashed page's title/tags/body straight off disk -- trashed pages
    // are rare and few, so no BM25-quality ranking is needed here, just the
    // same tiering logic plus a naive case-insensitive substring match for
    // the body tier.
    if include_trash {
        if let Some(vault_path) = vault_path {
            if let Ok(trashed_pages) = trash::list_trashed_pages(vault_path) {
                for trashed in trashed_pages {
                    let file_path = trash::trash_dir(vault_path).join(&trashed.trashed_filename);
                    let Ok(parsed) = frontmatter::parse_and_ensure_id(&file_path) else {
                        continue;
                    };

                    if let Some(scored) =
                        title_or_tag_match(&parsed.id, &parsed.title, &parsed.tags, &query_lower, true)
                    {
                        results.push(scored);
                        continue;
                    }

                    let body_lower = parsed.body.to_lowercase();
                    if let Some(match_start) = body_lower.find(&query_lower) {
                        let snippet = naive_snippet(&parsed.body, match_start, query_lower.len());
                        results.push(Scored {
                            id: parsed.id,
                            title: parsed.title,
                            tier: 3,
                            rank: 0.0,
                            snippet,
                            in_trash: true,
                            matched_tag: None,
                        });
                    }
                }
            }
        }
    }

    // Final ordering: tier ascending (strict, no blending), then each tier's
    // own rank (title exact/prefix/substring; tag exact/substring; BM25
    // score), with non-trashed results preferred over trashed ones within an
    // equal rank, and page title as the last, deterministic tiebreaker.
    results.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then_with(|| a.rank.partial_cmp(&b.rank).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.in_trash.cmp(&b.in_trash))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });

    Ok(results.into_iter().map(Scored::into_result).collect())
}

/// Checks one page's title and tags against `query_lower` (already
/// lowercased by the caller), returning its best tier-1/tier-2 match, if
/// any. Title wins over tags whenever both would otherwise match, since tier
/// 1 always outranks tier 2 -- this function simply never looks at tags once
/// the title already matched.
fn title_or_tag_match(
    id: &str,
    title: &str,
    tags: &[String],
    query_lower: &str,
    in_trash: bool,
) -> Option<Scored> {
    let title_lower = title.to_lowercase();

    // Tier 1: exact match ranks above prefix match ranks above "contains"
    // (per the ticket's "exact/prefix title match before partial").
    let title_rank = if title_lower == *query_lower {
        Some(0.0)
    } else if title_lower.starts_with(query_lower) {
        Some(1.0)
    } else if title_lower.contains(query_lower) {
        Some(2.0)
    } else {
        None
    };
    if let Some(rank) = title_rank {
        return Some(Scored {
            id: id.to_string(),
            title: title.to_string(),
            tier: 1,
            rank,
            snippet: title.to_string(),
            in_trash,
            matched_tag: None,
        });
    }

    // Tier 2: this page's own frontmatter tags (see this module's doc
    // comment for why tags feed the *tagged page's* own searchability, on
    // top of being a link target). Exact tag match ranks above a tag that
    // merely contains the query; the best-ranked tag among this page's own
    // tags wins if it has more than one.
    let mut best_tag: Option<(f64, String)> = None;
    for tag in tags {
        let tag_lower = tag.to_lowercase();
        let tag_rank = if tag_lower == *query_lower {
            0.0
        } else if tag_lower.contains(query_lower) {
            1.0
        } else {
            continue;
        };
        if best_tag.as_ref().map(|(best, _)| tag_rank < *best).unwrap_or(true) {
            best_tag = Some((tag_rank, tag.clone()));
        }
    }
    best_tag.map(|(rank, tag_text)| Scored {
        id: id.to_string(),
        title: title.to_string(),
        tier: 2,
        rank,
        snippet: format!("#{tag_text}"),
        in_trash,
        matched_tag: Some(tag_text),
    })
}

/// Runs the BM25-ranked body-only FTS5 query behind tier 3, returning
/// `(id, title, bm25_rank, snippet)` tuples ordered best-first (BM25's scale
/// is lower-is-better, so callers should keep that ordering rather than
/// re-sort ascending/descending by mistake).
///
/// The query is column-filtered to `body:` (never matching on title, which
/// would blur tier 3 back into tier 1) and every whitespace-separated token
/// is individually double-quoted and ANDed together -- this both sidesteps
/// FTS5's query-syntax special characters (a raw `-`, `"`, `AND`, `NOT`, ...
/// in the user's typed query would otherwise be parsed as FTS5 operators
/// instead of literal search text) and matches tokens anywhere in the body
/// rather than requiring them adjacent, which plain phrase-quoting the whole
/// query would otherwise demand.
fn body_fts_matches(conn: &Connection, raw_query: &str) -> rusqlite::Result<Vec<(String, String, f64, String)>> {
    let fts_query = build_body_fts_query(raw_query);
    let mut stmt = conn.prepare(
        "SELECT pages_fts.id, pages_fts.title, bm25(pages_fts) AS rnk,
                snippet(pages_fts, 2, char(1), char(2), '…', 12)
         FROM pages_fts
         WHERE pages_fts MATCH ?1
         ORDER BY rnk ASC",
    )?;
    let rows = stmt.query_map(params![fts_query], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    rows.collect()
}

/// Builds the `body:(...)` FTS5 MATCH expression for `raw_query` -- see
/// `body_fts_matches`'s doc comment for why every token is quoted and ANDed
/// rather than passed through as one phrase.
fn build_body_fts_query(raw_query: &str) -> String {
    let tokens: Vec<String> = raw_query
        .split_whitespace()
        .map(|tok| format!("\"{}\"", tok.replace('"', "\"\"")))
        .collect();
    if tokens.is_empty() {
        // Caller (`search_pages`) already guards against an empty/whitespace
        // -only query, but stay defensive: a phrase with no tokens is valid
        // FTS5 syntax that simply never matches anything.
        return "body:\"\"".to_string();
    }
    format!("body:({})", tokens.join(" AND "))
}

/// Builds a plain-text snippet of `body` around the byte range
/// `[match_start, match_start + match_len)`, expanded by
/// `NAIVE_SNIPPET_RADIUS` characters on each side -- the trashed-page
/// fallback's un-indexed counterpart to `links.rs`'s `build_snippet` (same
/// char-boundary-safety helpers, reused via `pub(crate)`).
fn naive_snippet(body: &str, match_start: usize, match_len: usize) -> String {
    let start = floor_char_boundary(body, match_start.saturating_sub(NAIVE_SNIPPET_RADIUS));
    let end = ceil_char_boundary(body, (match_start + match_len + NAIVE_SNIPPET_RADIUS).min(body.len()));

    let raw = &body[start..end];
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");

    let prefix = if start > 0 { "…" } else { "" };
    let suffix = if end < body.len() { "…" } else { "" };
    format!("{prefix}{collapsed}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index;
    use crate::vault;
    use std::fs;
    use tempfile::TempDir;

    fn write_page(dir: &TempDir, name: &str, content: &str) {
        fs::write(dir.path().join(name), content).unwrap();
    }

    fn build_conn(dir: &TempDir) -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        index::build_index(&mut conn, dir.path()).unwrap();
        conn
    }

    #[test]
    fn title_match_outranks_tag_match_which_outranks_body_match() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "widget.md", "---\nid: t\ntitle: Widget\n---\nNothing special here.\n");
        write_page(
            &dir,
            "gadget.md",
            "---\nid: g\ntitle: Gadget\ntags: [widget]\n---\nA gadget page, tagged widget.\n",
        );
        write_page(
            &dir,
            "essay.md",
            "---\nid: e\ntitle: Essay\n---\nThis essay just happens to mention widget in passing.\n",
        );
        let conn = build_conn(&dir);

        let results = search_pages(&conn, None, "widget", false).unwrap();

        assert_eq!(results.len(), 3, "expected all three pages to match: {results:?}");
        assert_eq!(results[0].id, "t");
        assert_eq!(results[0].tier, 1);
        assert_eq!(results[1].id, "g");
        assert_eq!(results[1].tier, 2);
        assert_eq!(results[1].matched_tag.as_deref(), Some("widget"));
        assert_eq!(results[2].id, "e");
        assert_eq!(results[2].tier, 3);
    }

    #[test]
    fn one_row_per_page_takes_the_best_tier_even_when_it_also_matches_lower_tiers() {
        let dir = TempDir::new().unwrap();
        // Titled "Widget" *and* tagged "widget" *and* mentions "widget" in
        // the body -- must appear exactly once, at tier 1.
        write_page(
            &dir,
            "widget.md",
            "---\nid: w\ntitle: Widget\ntags: [widget]\n---\nThis widget page really is about widgets.\n",
        );
        let conn = build_conn(&dir);

        let results = search_pages(&conn, None, "widget", false).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "w");
        assert_eq!(results[0].tier, 1);
    }

    #[test]
    fn exact_title_match_ranks_above_prefix_which_ranks_above_substring() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "note.md", "---\nid: sub\ntitle: A Note About Cats\n---\n");
        write_page(&dir, "catnip.md", "---\nid: pre\ntitle: Cats Everywhere\n---\n");
        write_page(&dir, "cats.md", "---\nid: exact\ntitle: Cats\n---\n");
        let conn = build_conn(&dir);

        let results = search_pages(&conn, None, "cats", false).unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].id, "exact");
        assert_eq!(results[1].id, "pre");
        assert_eq!(results[2].id, "sub");
    }

    #[test]
    fn body_tier_uses_bm25_ranking_and_a_snippet() {
        let dir = TempDir::new().unwrap();
        write_page(
            &dir,
            "sparse.md",
            "---\nid: sparse\ntitle: Sparse\n---\nOnly one mention of dolphins here.\n",
        );
        write_page(
            &dir,
            "dense.md",
            "---\nid: dense\ntitle: Dense\n---\nDolphins dolphins dolphins, this page is all about dolphins.\n",
        );
        let conn = build_conn(&dir);

        let results = search_pages(&conn, None, "dolphins", false).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.tier == 3));
        // The denser mention should BM25-rank first.
        assert_eq!(results[0].id, "dense");
        assert!(results[0].snippet.to_lowercase().contains("dolphins"));
    }

    #[test]
    fn trash_excluded_by_default_and_included_when_asked() {
        let dir = TempDir::new().unwrap();
        let vault_path = vault::ensure_git_repo(dir.path()).unwrap();
        crate::author::confirm_test_author(dir.path());
        fs::write(
            vault_path.join("ghost.md"),
            "---\nid: ghost\ntitle: Ghost Page\n---\nA spooky mention of pumpkins.\n",
        )
        .unwrap();
        let mut conn = Connection::open_in_memory().unwrap();
        index::build_index(&mut conn, &vault_path).unwrap();

        let ghost_path = vault_path.join("ghost.md");
        crate::trash::trash_page(&vault_path, dir.path(), &ghost_path, "ghost", "Trash Ghost").unwrap();
        index::remove_page(&conn, "ghost").unwrap();

        let excluded = search_pages(&conn, Some(&vault_path), "pumpkins", false).unwrap();
        assert!(excluded.is_empty(), "trashed page must not appear by default: {excluded:?}");

        let included = search_pages(&conn, Some(&vault_path), "pumpkins", true).unwrap();
        assert_eq!(included.len(), 1);
        assert_eq!(included[0].id, "ghost");
        assert!(included[0].in_trash);
        assert!(included[0].snippet.to_lowercase().contains("pumpkins"));

        // Also reachable by title while trashed.
        let by_title = search_pages(&conn, Some(&vault_path), "Ghost Page", true).unwrap();
        assert_eq!(by_title.len(), 1);
        assert_eq!(by_title[0].tier, 1);
        assert!(by_title[0].in_trash);
    }

    #[test]
    fn empty_query_returns_no_results() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "a.md", "---\nid: a\ntitle: A\n---\nBody.\n");
        let conn = build_conn(&dir);

        assert!(search_pages(&conn, None, "", false).unwrap().is_empty());
        assert!(search_pages(&conn, None, "   ", false).unwrap().is_empty());
    }

    #[test]
    fn query_with_fts_special_characters_does_not_error() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "a.md", "---\nid: a\ntitle: A\n---\nSome body text.\n");
        let conn = build_conn(&dir);

        // Characters meaningful to FTS5 query syntax must not blow up the
        // search -- they're just literal (and non-matching) text here.
        let results = search_pages(&conn, None, "\"quoted\" AND NOT -weird:query", false).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn multi_word_body_query_matches_tokens_regardless_of_order() {
        let dir = TempDir::new().unwrap();
        write_page(&dir, "a.md", "---\nid: a\ntitle: A\n---\nRed apples and green pears.\n");
        let conn = build_conn(&dir);

        let results = search_pages(&conn, None, "pears apples", false).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "a");
    }
}
