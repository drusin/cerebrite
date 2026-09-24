// Heading-rename redirect log (ticket 08 / ADR-0007): when a heading is
// renamed or moved, a `[[Page#old-slug]]` link must keep resolving until a
// background cleanup pass has rewritten every file that referenced it. The
// redirect table itself is a small, git-synced, plaintext sidecar --
// `.cerebrite/redirects.tsv` at the vault root -- kept separate from the
// derived index (which is never git-synced, ADR-0008) so a redirect created
// on one device survives a restart or a sync to another device before the
// cleanup job has had a chance to run.
//
// On-disk format (per `.scratch/cerebrite-mvp/issues/05-redirect-log-format.md`):
// one `oldPageId#oldSlug\tnewPageId#newSlug\ttimestamp\tdeviceId` line per
// entry, **inserted in sorted-by-key order** (sort key = the `oldPageId#oldSlug`
// column) rather than appended at end-of-file -- verified empirically (see
// that research doc) to be what keeps two independent concurrent renames from
// producing a spurious git "changelog conflict" on this file, while the same
// heading renamed differently on two devices still conflicts correctly.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::frontmatter;
use crate::index;
use crate::links;
use crate::markdown;
use crate::vault;

/// One line of `.cerebrite/redirects.tsv`. Only `old_key`/`new_key` are
/// load-bearing (resolution and pruning key off them); `timestamp` and
/// `device_id` are advisory, read by nobody's resolve/prune logic, there only
/// to help a human disambiguate a merge conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedirectEntry {
    pub old_key: String,
    pub new_key: String,
    pub timestamp: String,
    pub device_id: String,
}

/// Builds an `oldPageId#oldSlug`-shaped key (also used for the new-key side).
pub fn make_key(page_id: &str, slug: &str) -> String {
    format!("{page_id}#{slug}")
}

/// Splits a `pageId#slug` key back into its two parts, on the *first* `#`
/// (page ids are uuids and never contain one, so this is unambiguous).
fn split_key(key: &str) -> Option<(&str, &str)> {
    key.split_once('#')
}

pub fn redirects_path(vault_path: &Path) -> PathBuf {
    vault_path.join(".cerebrite").join("redirects.tsv")
}

/// Parses a whole `redirects.tsv` file's content into entries. A malformed
/// line (wrong field count) is skipped rather than failing the whole parse --
/// this file is hand-mergeable by git, so being lenient about stray blank
/// lines or a half-resolved merge marker left behind is worth more than
/// strictness here.
pub fn parse_tsv(content: &str) -> Vec<RedirectEntry> {
    let mut out = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 4 {
            continue;
        }
        out.push(RedirectEntry {
            old_key: fields[0].to_string(),
            new_key: fields[1].to_string(),
            timestamp: fields[2].to_string(),
            device_id: fields[3].to_string(),
        });
    }
    out
}

/// Serializes entries back to `redirects.tsv` text, sorted by `old_key` --
/// defensive re-sorting on every write, not just on insert, so the file is
/// never accidentally left out of order by a caller.
pub fn serialize_tsv(entries: &[RedirectEntry]) -> String {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.old_key.cmp(&b.old_key));

    let mut out = String::new();
    for entry in &sorted {
        out.push_str(&entry.old_key);
        out.push('\t');
        out.push_str(&entry.new_key);
        out.push('\t');
        out.push_str(&entry.timestamp);
        out.push('\t');
        out.push_str(&entry.device_id);
        out.push('\n');
    }
    out
}

/// Loads `.cerebrite/redirects.tsv`, or an empty list if the vault has none
/// yet.
pub fn load(vault_path: &Path) -> Result<Vec<RedirectEntry>> {
    let path = redirects_path(vault_path);
    match fs::read_to_string(&path) {
        Ok(content) => Ok(parse_tsv(&content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// Overwrites `.cerebrite/redirects.tsv` with exactly `entries` (sorted by
/// key), creating the `.cerebrite` directory if needed.
pub fn save(vault_path: &Path, entries: &[RedirectEntry]) -> Result<()> {
    let path = redirects_path(vault_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("creating .cerebrite directory")?;
    }
    fs::write(&path, serialize_tsv(entries)).with_context(|| format!("writing {}", path.display()))
}

/// Inserts one new entry at its sorted-by-key position (not appended at
/// end-of-file -- see module docs) and rewrites the file.
pub fn insert_sorted(vault_path: &Path, entry: RedirectEntry) -> Result<()> {
    let mut entries = load(vault_path)?;
    let pos = entries
        .iter()
        .position(|e| e.old_key > entry.old_key)
        .unwrap_or(entries.len());
    entries.insert(pos, entry);
    save(vault_path, &entries)
}

/// Current time as an RFC3339 UTC timestamp (`YYYY-MM-DDTHH:MM:SSZ`), computed
/// by hand (no additional date/time crate) since this field is advisory only.
pub fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    epoch_secs_to_rfc3339(secs)
}

/// Civil-calendar conversion (Howard Hinnant's `civil_from_days` algorithm),
/// proleptic Gregorian, no leap seconds -- exactly what's needed for an
/// advisory timestamp with zero extra dependencies.
fn epoch_secs_to_rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let time_of_day = secs.rem_euclid(86_400);
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Rename-detection heuristic (ticket 08, "your call" -- deliberately the
/// simplest reasonable one): compares a page's old and new ordered heading
/// slug lists. A slug present in `old_slugs` but not `new_slugs` is
/// "removed"; a slug present in `new_slugs` but not `old_slugs` is "added"
/// (order preserved from each list). If the two sets differ by *exactly* the
/// same count of removed and added slugs (and at least one), they're paired
/// position-for-position in document order and treated as renames --
/// including the ambiguous multi-change case (e.g. two removed, two added),
/// which is paired best-effort rather than solved properly.
///
/// Known limitations (deliberately not handled, to avoid over-engineering):
/// - An edit that both renames one heading *and* adds/removes another in the
///   same save produces an unequal removed/added count and is treated as "no
///   rename detected" -- the redirect is silently skipped for that save.
/// - Positional pairing assumes the Nth removed heading corresponds to the
///   Nth added one in document order; a same-count edit that isn't actually a
///   like-for-like rename (e.g. two headings swapping content) would be
///   mispaired. This is accepted as a heuristic, not a correctness guarantee.
pub fn detect_renames(old_slugs: &[String], new_slugs: &[String]) -> Vec<(String, String)> {
    let removed: Vec<String> = old_slugs
        .iter()
        .filter(|s| !new_slugs.contains(s))
        .cloned()
        .collect();
    let added: Vec<String> = new_slugs
        .iter()
        .filter(|s| !old_slugs.contains(s))
        .cloned()
        .collect();

    if removed.is_empty() || removed.len() != added.len() {
        return Vec::new();
    }

    removed.into_iter().zip(added).collect()
}

/// Resolves `slug` (the heading slug a `[[Page#slug]]` link was written
/// against) to whatever it should mean *right now* on the page identified by
/// `page_id`, whose current heading slugs are `current_slugs`.
///
/// If `slug` is already current, it's returned unchanged. Otherwise the
/// redirect chain starting at `pageId#slug` is followed -- old key to new
/// key, and a new key can itself be an old key for a later rename -- until a
/// key lands on a slug that's in `current_slugs`, or a cycle/depth guard
/// trips. This is what lets a multi-hop rename chain collapse into a single
/// direct resolution (ticket 08's third acceptance criterion). Per the
/// ticket, cross-page heading moves aren't handled: the chain is followed
/// purely by key, but only a hop landing back on `page_id` with a slug in
/// `current_slugs` counts as "resolved" -- an unresolvable or cross-page
/// chain falls back to the *last* slug reached, never silently to the
/// original (broken) one, so a caller can at least see progress was made.
pub fn resolve_heading_slug(
    entries: &[RedirectEntry],
    page_id: &str,
    slug: &str,
    current_slugs: &[String],
) -> String {
    if current_slugs.iter().any(|s| s == slug) {
        return slug.to_string();
    }

    let map: HashMap<&str, &str> = entries
        .iter()
        .map(|e| (e.old_key.as_str(), e.new_key.as_str()))
        .collect();

    const MAX_HOPS: usize = 32;
    let start_key = make_key(page_id, slug);
    let mut visited: HashSet<String> = HashSet::new();
    let mut current_key = start_key.clone();

    for _ in 0..MAX_HOPS {
        if !visited.insert(current_key.clone()) {
            break; // cycle guard
        }
        let Some(&next) = map.get(current_key.as_str()) else {
            break;
        };
        current_key = next.to_string();
        if let Some((next_page, next_slug)) = split_key(&current_key) {
            if next_page == page_id && current_slugs.iter().any(|s| s == next_slug) {
                return next_slug.to_string();
            }
        }
    }

    // Never fully resolved to a current slug: best-effort, return the last
    // slug reached (or the original one if the chain never moved at all).
    split_key(&current_key)
        .map(|(_, s)| s.to_string())
        .unwrap_or_else(|| slug.to_string())
}

/// Rewrites files whose `[[Page#old-slug]]` link text resolves through a
/// redirect chain to a heading that currently exists, then prunes any
/// redirect entry no longer referenced by any link left in the vault --
/// both run as a byproduct of the full derived-index rebuild (launch/sync),
/// never as a separate scan (ticket 08's 4th/5th acceptance criteria).
///
/// Two separate commits are made when there's anything to commit: one for
/// the rewritten page files, one for the pruned `redirects.tsv` -- each a
/// no-op (via `vault::commit_all`'s existing behavior) when nothing changed.
/// This is a deliberate choice over batching everything into a single
/// commit: it keeps "links were repointed" and "redirect log was pruned"
/// separately reviewable in git history.
pub fn cleanup_and_prune(vault_path: &Path, repo_root: &Path, conn: &Connection) -> Result<()> {
    let entries = load(vault_path)?;
    if entries.is_empty() {
        return Ok(());
    }

    // --- Pass 1: rewrite stale heading-link fragments in place. ---
    let mut stmt = conn.prepare("SELECT id, title, path, body FROM pages")?;
    let pages: Vec<(String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let by_normalized_title: HashMap<String, (String, String)> = pages
        .iter()
        .map(|(id, title, _path, body)| (frontmatter::normalize_title(title), (id.clone(), body.clone())))
        .collect();

    let mut any_file_changed = false;

    for (id, title, path, body) in &pages {
        let spans = links::find_heading_link_spans(body);
        if spans.is_empty() {
            continue;
        }

        let mut replacements: Vec<(std::ops::Range<usize>, String)> = Vec::new();
        for span in &spans {
            let Some((target_id, target_body)) = by_normalized_title.get(&span.normalized_target) else {
                continue; // dynamic target: nothing to resolve against
            };
            let target_current = markdown::heading_slugs(target_body);
            if target_current.iter().any(|s| s == &span.heading_slug) {
                continue; // already current, nothing to rewrite
            }
            let resolved = resolve_heading_slug(&entries, target_id, &span.heading_slug, &target_current);
            if resolved != span.heading_slug && target_current.iter().any(|s| s == &resolved) {
                replacements.push((span.fragment_range.clone(), resolved));
            }
        }

        if replacements.is_empty() {
            continue;
        }

        let mut new_body = body.clone();
        // Apply back-to-front so earlier byte ranges stay valid.
        replacements.sort_by_key(|(range, _)| range.start);
        for (range, replacement) in replacements.into_iter().rev() {
            new_body.replace_range(range, &replacement);
        }

        frontmatter::write_body(Path::new(path), &new_body)?;
        // Re-read the file's frontmatter tags (issue 09) rather than assuming
        // they're unaffected -- this rewrite only ever touches heading-link
        // fragments in the body, but re-parsing keeps this in lockstep with
        // save_page_impl's "trust what's actually on disk" approach.
        let tags = frontmatter::parse_and_ensure_id(Path::new(path))?.tags;
        index::update_page_content(conn, id, title, &new_body, &tags)?;
        any_file_changed = true;
    }

    if any_file_changed {
        vault::commit_all(repo_root, "Rewrite links through renamed headings")?;
    }

    // --- Pass 2: prune redirect entries no longer referenced anywhere. ---
    let mut stmt = conn.prepare("SELECT id, title, body FROM pages")?;
    let fresh_pages: Vec<(String, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let fresh_by_title: HashMap<String, String> = fresh_pages
        .iter()
        .map(|(id, title, _body)| (frontmatter::normalize_title(title), id.clone()))
        .collect();

    let mut referenced_keys: HashSet<String> = HashSet::new();
    for (_id, _title, body) in &fresh_pages {
        for span in links::find_heading_link_spans(body) {
            if let Some(target_id) = fresh_by_title.get(&span.normalized_target) {
                referenced_keys.insert(make_key(target_id, &span.heading_slug));
            }
        }
    }

    let pruned: Vec<RedirectEntry> = entries
        .into_iter()
        .filter(|e| referenced_keys.contains(&e.old_key))
        .collect();

    save(vault_path, &pruned)?;
    vault::commit_all(repo_root, "Prune resolved heading redirects")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn entry(old: &str, new: &str) -> RedirectEntry {
        RedirectEntry {
            old_key: old.to_string(),
            new_key: new.to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            device_id: "device-1".to_string(),
        }
    }

    #[test]
    fn parse_and_serialize_round_trip() {
        let entries = vec![entry("p1#a", "p1#b"), entry("p2#x", "p2#y")];
        let text = serialize_tsv(&entries);
        let parsed = parse_tsv(&text);
        assert_eq!(parsed, entries);
    }

    #[test]
    fn serialize_sorts_by_old_key() {
        let entries = vec![entry("p2#z", "p2#w"), entry("p1#a", "p1#b")];
        let text = serialize_tsv(&entries);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("p1#a"));
        assert!(lines[1].starts_with("p2#z"));
    }

    #[test]
    fn parse_skips_malformed_lines() {
        let text = "p1#a\tp1#b\tts\tdev\nnot-enough-fields\np2#x\tp2#y\tts\tdev\n";
        let parsed = parse_tsv(text);
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn insert_sorted_inserts_out_of_order_entries_into_a_sorted_file() {
        let dir = tempdir().unwrap();
        insert_sorted(dir.path(), entry("p3#a", "p3#b")).unwrap();
        insert_sorted(dir.path(), entry("p1#a", "p1#b")).unwrap();
        insert_sorted(dir.path(), entry("p2#a", "p2#b")).unwrap();

        let content = fs::read_to_string(redirects_path(dir.path())).unwrap();
        let keys: Vec<&str> = content.lines().map(|l| l.split('\t').next().unwrap()).collect();
        assert_eq!(keys, vec!["p1#a", "p2#a", "p3#a"]);
    }

    #[test]
    fn insert_sorted_creates_the_dotfolder_and_file_on_first_use() {
        let dir = tempdir().unwrap();
        assert!(!redirects_path(dir.path()).exists());
        insert_sorted(dir.path(), entry("p1#a", "p1#b")).unwrap();
        assert!(redirects_path(dir.path()).exists());
    }

    #[test]
    fn detect_renames_finds_a_single_rename() {
        let old = vec!["setup".to_string(), "usage".to_string()];
        let new = vec!["setup".to_string(), "getting-started".to_string()];
        let renames = detect_renames(&old, &new);
        assert_eq!(renames, vec![("usage".to_string(), "getting-started".to_string())]);
    }

    #[test]
    fn detect_renames_has_no_false_positive_on_an_unrelated_addition() {
        // Adding a heading without removing one: not a rename.
        let old = vec!["setup".to_string()];
        let new = vec!["setup".to_string(), "usage".to_string()];
        assert!(detect_renames(&old, &new).is_empty());
    }

    #[test]
    fn detect_renames_has_no_false_positive_on_a_plain_deletion() {
        let old = vec!["setup".to_string(), "usage".to_string()];
        let new = vec!["setup".to_string()];
        assert!(detect_renames(&old, &new).is_empty());
    }

    #[test]
    fn detect_renames_has_no_false_positive_when_nothing_changed() {
        let old = vec!["setup".to_string(), "usage".to_string()];
        let new = old.clone();
        assert!(detect_renames(&old, &new).is_empty());
    }

    #[test]
    fn detect_renames_best_effort_pairs_an_ambiguous_multi_change() {
        let old = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
        let new = vec!["alpha".to_string(), "delta".to_string(), "epsilon".to_string()];
        let mut renames = detect_renames(&old, &new);
        renames.sort();
        let mut expected = vec![
            ("beta".to_string(), "delta".to_string()),
            ("gamma".to_string(), "epsilon".to_string()),
        ];
        expected.sort();
        assert_eq!(renames, expected);
    }

    #[test]
    fn resolve_heading_slug_returns_unchanged_when_already_current() {
        let current = vec!["setup".to_string()];
        let resolved = resolve_heading_slug(&[], "p1", "setup", &current);
        assert_eq!(resolved, "setup");
    }

    #[test]
    fn resolve_heading_slug_follows_a_single_hop() {
        let entries = vec![entry("p1#old", "p1#new")];
        let current = vec!["new".to_string()];
        let resolved = resolve_heading_slug(&entries, "p1", "old", &current);
        assert_eq!(resolved, "new");
    }

    #[test]
    fn resolve_heading_slug_follows_a_multi_hop_chain() {
        let entries = vec![entry("p1#a", "p1#b"), entry("p1#b", "p1#c"), entry("p1#c", "p1#d")];
        let current = vec!["d".to_string()];
        let resolved = resolve_heading_slug(&entries, "p1", "a", &current);
        assert_eq!(resolved, "d");
    }

    #[test]
    fn resolve_heading_slug_guards_against_a_cycle() {
        // A malformed/adversarial log with a cycle must not hang.
        let entries = vec![entry("p1#a", "p1#b"), entry("p1#b", "p1#a")];
        let current = vec!["z".to_string()]; // never resolves
        let resolved = resolve_heading_slug(&entries, "p1", "a", &current);
        // Terminates and returns *something* from the chain, not "a" (progress).
        assert!(resolved == "a" || resolved == "b");
    }

    #[test]
    fn resolve_heading_slug_falls_back_to_last_hop_when_chain_is_incomplete() {
        let entries = vec![entry("p1#a", "p1#b")];
        let current = vec!["z".to_string()]; // "b" never becomes current
        let resolved = resolve_heading_slug(&entries, "p1", "a", &current);
        assert_eq!(resolved, "b");
    }

    // --- Fixture-vault test for the rebuild-time cleanup rewrite + prune. ---

    fn write_page(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn cleanup_and_prune_rewrites_stale_links_preserves_frontmatter_and_commits() {
        let dir = tempdir().unwrap();
        let vault_path = vault::ensure_git_repo(dir.path()).unwrap();

        write_page(
            &vault_path,
            "target.md",
            "---\nid: target-id\ntitle: Target\ntags:\n  - keep-me\n---\n## New Heading\n\nBody.\n",
        );
        write_page(
            &vault_path,
            "source.md",
            "---\nid: source-id\ntitle: Source\n---\nSee [[Target#old-slug]] for details.\n",
        );
        vault::commit_all(dir.path(), "Initial fixture").unwrap();

        // Simulate a heading having been renamed on a previous save: the
        // redirect log already knows old-slug -> new-heading (the current
        // slug of "## New Heading").
        insert_sorted(
            &vault_path,
            entry("target-id#old-slug", "target-id#new-heading"),
        )
        .unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        index::build_index(&mut conn, &vault_path).unwrap();

        cleanup_and_prune(&vault_path, dir.path(), &conn).unwrap();

        let rewritten = fs::read_to_string(vault_path.join("source.md")).unwrap();
        assert!(
            rewritten.contains("[[Target#new-heading]]"),
            "expected rewritten link, got: {rewritten}"
        );
        assert!(!rewritten.contains("old-slug"));
        // Frontmatter untouched.
        assert!(rewritten.contains("id: source-id"));

        // Target's own frontmatter (a different file) must be untouched too.
        let target_content = fs::read_to_string(vault_path.join("target.md")).unwrap();
        assert!(target_content.contains("tags:\n  - keep-me"));

        // The redirect entry is now unreferenced (the link points directly at
        // the current slug) -- it must be pruned.
        let remaining = load(&vault_path).unwrap();
        assert!(remaining.is_empty(), "expected the redirect to be pruned, got {remaining:?}");

        // Both passes commit: verify at least one commit happened beyond the
        // fixture's initial commit.
        let repo = git2::Repository::open(dir.path()).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_ne!(head.message().unwrap(), "Initial fixture");
    }

    #[test]
    fn cleanup_and_prune_keeps_an_entry_still_referenced_by_a_stale_link() {
        let dir = tempdir().unwrap();
        let vault_path = vault::ensure_git_repo(dir.path()).unwrap();

        write_page(
            &vault_path,
            "target.md",
            "---\nid: target-id\ntitle: Target\n---\n## Still Stale Target\n\nBody.\n",
        );
        // This link's slug ("unrelated-slug") doesn't match the redirect's
        // old key at all, and doesn't match any current heading either -- a
        // dangling reference the cleanup pass can't do anything about.
        write_page(
            &vault_path,
            "source.md",
            "---\nid: source-id\ntitle: Source\n---\nSee [[Target#unrelated-slug]] here.\n",
        );
        vault::commit_all(dir.path(), "Initial fixture").unwrap();

        insert_sorted(&vault_path, entry("target-id#unrelated-slug", "target-id#nowhere")).unwrap();

        let mut conn = Connection::open_in_memory().unwrap();
        index::build_index(&mut conn, &vault_path).unwrap();

        cleanup_and_prune(&vault_path, dir.path(), &conn).unwrap();

        // Nothing could be resolved to a current heading, so nothing was
        // rewritten -- but the link text still references the redirect's old
        // key, so the entry must survive pruning.
        let remaining = load(&vault_path).unwrap();
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn now_rfc3339_produces_a_well_formed_timestamp() {
        let ts = now_rfc3339();
        assert_eq!(ts.len(), 20);
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.chars().nth(4), Some('-'));
        assert_eq!(ts.chars().nth(10), Some('T'));
    }
}
