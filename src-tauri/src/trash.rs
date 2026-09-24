// Page deletion via a git-tracked trash folder (issue 10 / ADR-0010):
// deleting a persisted page moves its file into `.cerebrite/trash/` rather
// than deleting it outright, git-committed as an ordinary move so it syncs
// immediately like any other edit. Only an explicit "empty trash" action
// removes files for good; an explicit "restore" moves a file back to its
// original path.
//
// Original-path tracking: a small JSON manifest file
// (`.cerebrite/trash/manifest.json`) maps each trashed file's *filename*
// (relative to the trash dir) to the page's original path, relative to the
// vault root. This is simpler than encoding the path into the trashed
// filename (which would need escaping for nested directories) and doesn't
// touch the file's frontmatter at all -- the id inside stays byte-for-byte
// untouched by trashing/restoring, since both are plain filesystem moves.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::frontmatter;

/// Relative path (from the vault root) of the trash directory.
pub const TRASH_DIR_REL: &str = ".cerebrite/trash";

fn manifest_file_path(vault_path: &Path) -> PathBuf {
    trash_dir(vault_path).join("manifest.json")
}

/// Absolute path of `.cerebrite/trash/` under `vault_path`.
pub fn trash_dir(vault_path: &Path) -> PathBuf {
    vault_path.join(TRASH_DIR_REL)
}

/// Manifest mapping each trashed file's filename (not full path -- just the
/// name under `trash_dir`) to the page's original path, relative to the
/// vault root (POSIX-style forward slashes, so the manifest is portable and
/// diffs cleanly across platforms).
type Manifest = HashMap<String, String>;

fn load_manifest(vault_path: &Path) -> Manifest {
    let path = manifest_file_path(vault_path);
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn save_manifest(vault_path: &Path, manifest: &Manifest) -> Result<()> {
    let path = manifest_file_path(vault_path);
    let content = serde_json::to_string_pretty(manifest).context("serializing trash manifest")?;
    fs::write(path, content).context("writing trash manifest")?;
    Ok(())
}

/// Converts an absolute path (must be inside `vault_path`) into a
/// vault-root-relative, forward-slash string for storage in the manifest.
fn relative_to_vault(vault_path: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(vault_path)
        .context("page path is not inside the vault")?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

/// A trashed page, as surfaced to the UI (issue 10 point 5): enough to list
/// what's in the trash and to drive an in-app "Restore" action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashedPage {
    pub id: String,
    pub title: String,
    /// Filename of the file under `.cerebrite/trash/` (the manifest key) --
    /// what `restore_page` needs to find it again.
    pub trashed_filename: String,
    /// The page's original path, relative to the vault root, as recorded in
    /// the manifest at trash time.
    pub original_relative_path: String,
}

/// Moves `page_path` (an absolute path to a persisted page file, somewhere
/// under `vault_path`) into `.cerebrite/trash/`, creating the trash
/// directory if needed and disambiguating a filename collision (e.g. a
/// previously trashed page of the same name) by suffixing the page's id.
/// Records the original path in the trash manifest, then git-commits the
/// move with `message` via `vault::commit_all`.
///
/// `repo_root` is the git repository's root (ADR-0011: distinct from
/// `vault_path`, since the vault is now a `vault/` subdirectory of the
/// repository rather than the repository itself) -- `commit_all` opens the
/// repo there.
///
/// The frontmatter (and therefore the id) is never rewritten -- this is a
/// pure filesystem move.
pub fn trash_page(
    vault_path: &Path,
    repo_root: &Path,
    page_path: &Path,
    page_id: &str,
    message: &str,
) -> Result<PathBuf> {
    let dir = trash_dir(vault_path);
    fs::create_dir_all(&dir).context("creating trash directory")?;

    let file_name = page_path
        .file_name()
        .context("page path has no file name")?
        .to_string_lossy()
        .to_string();

    let mut candidate = dir.join(&file_name);
    if candidate.exists() {
        // Disambiguate by appending the page's id before the extension.
        let stem = page_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        let ext = page_path.extension().and_then(|e| e.to_str());
        let disambiguated = match ext {
            Some(ext) => format!("{stem}-{page_id}.{ext}"),
            None => format!("{stem}-{page_id}"),
        };
        candidate = dir.join(disambiguated);
    }

    let original_relative = relative_to_vault(vault_path, page_path)?;
    fs::rename(page_path, &candidate).context("moving page file into trash")?;

    let trashed_filename = candidate
        .file_name()
        .context("trashed path has no file name")?
        .to_string_lossy()
        .to_string();

    let mut manifest = load_manifest(vault_path);
    manifest.insert(trashed_filename, original_relative);
    save_manifest(vault_path, &manifest)?;

    crate::vault::commit_all(repo_root, message).context("committing trash move")?;

    Ok(candidate)
}

/// Moves a trashed page (identified by its filename under `.cerebrite/trash/`
/// -- the manifest key) back to its original path, recreating any parent
/// directories that no longer exist, then git-commits the restore. The
/// file's contents (including its frontmatter id) are untouched by the move.
pub fn restore_page(
    vault_path: &Path,
    repo_root: &Path,
    trashed_filename: &str,
    message: &str,
) -> Result<PathBuf> {
    let mut manifest = load_manifest(vault_path);
    let original_relative = manifest
        .remove(trashed_filename)
        .context("trashed file is not recorded in the trash manifest")?;

    let trashed_path = trash_dir(vault_path).join(trashed_filename);
    let original_path = vault_path.join(&original_relative);

    if let Some(parent) = original_path.parent() {
        fs::create_dir_all(parent).context("recreating original page's parent directory")?;
    }

    fs::rename(&trashed_path, &original_path).context("moving page file back to its original path")?;

    save_manifest(vault_path, &manifest)?;

    crate::vault::commit_all(repo_root, message).context("committing restore")?;

    Ok(original_path)
}

/// Permanently deletes every file under `.cerebrite/trash/` (real filesystem
/// removal -- this is the only purge path per the ticket; trashing itself
/// never auto-purges), clears the manifest, and git-commits the removal.
pub fn empty_trash(vault_path: &Path, repo_root: &Path) -> Result<()> {
    let dir = trash_dir(vault_path);
    if dir.is_dir() {
        for entry in fs::read_dir(&dir).context("reading trash directory")? {
            let entry = entry.context("reading trash directory entry")?;
            let path = entry.path();
            if path.is_file() {
                fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
            }
        }
    }

    crate::vault::commit_all(repo_root, "Empty trash").context("committing empty trash")?;
    Ok(())
}

/// Enumerates every page currently sitting in `.cerebrite/trash/`, parsing
/// each file's frontmatter for its id/title and cross-referencing the
/// manifest for its original path. A trashed file with no manifest entry
/// (shouldn't normally happen, but tolerated defensively) is skipped rather
/// than erroring the whole listing.
pub fn list_trashed_pages(vault_path: &Path) -> Result<Vec<TrashedPage>> {
    let dir = trash_dir(vault_path);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let manifest = load_manifest(vault_path);
    let mut pages = Vec::new();

    for entry in fs::read_dir(&dir).context("reading trash directory")? {
        let entry = entry.context("reading trash directory entry")?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        let Some(original_relative_path) = manifest.get(&file_name) else {
            continue;
        };

        let parsed = frontmatter::parse_and_ensure_id(&path)?;
        pages.push(TrashedPage {
            id: parsed.id,
            title: parsed.title,
            trashed_filename: file_name,
            original_relative_path: original_relative_path.clone(),
        });
    }

    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_page(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn trash_page_moves_the_file_and_commits() {
        let dir = tempdir().unwrap();
        crate::vault::ensure_git_repo(dir.path()).unwrap();
        crate::author::confirm_test_author(dir.path());
        let repo = git2::Repository::open(dir.path()).unwrap();
        let page_path = write_page(dir.path(), "hello.md", "---\nid: abc\ntitle: Hello\n---\nBody.\n");
        crate::vault::commit_all(dir.path(), "Create Hello").unwrap();
        let head_before = repo.head().unwrap().peel_to_commit().unwrap().id();

        let trashed_path = trash_page(dir.path(), dir.path(), &page_path, "abc", "Trash Hello").unwrap();

        assert!(!page_path.exists());
        assert!(trashed_path.exists());
        assert_eq!(trashed_path.parent().unwrap(), trash_dir(dir.path()));
        assert_eq!(fs::read_to_string(&trashed_path).unwrap(), "---\nid: abc\ntitle: Hello\n---\nBody.\n");

        let head_after = repo.head().unwrap().peel_to_commit().unwrap().id();
        assert_ne!(head_before, head_after);
        assert_eq!(repo.head().unwrap().peel_to_commit().unwrap().message().unwrap(), "Trash Hello");
    }

    #[test]
    fn trash_page_disambiguates_a_filename_collision() {
        let dir = tempdir().unwrap();
        crate::vault::ensure_git_repo(dir.path()).unwrap();
        crate::author::confirm_test_author(dir.path());

        // First page named hello.md gets trashed.
        let first_path = write_page(dir.path(), "hello.md", "---\nid: first\ntitle: Hello\n---\nOne.\n");
        crate::vault::commit_all(dir.path(), "Create").unwrap();
        trash_page(dir.path(), dir.path(), &first_path, "first", "Trash 1").unwrap();

        // A second, unrelated page that also happens to be named hello.md
        // (e.g. re-created after the first was trashed) gets trashed too.
        let second_path = write_page(dir.path(), "hello.md", "---\nid: second\ntitle: Hello Again\n---\nTwo.\n");
        crate::vault::commit_all(dir.path(), "Create 2").unwrap();
        let trashed_second = trash_page(dir.path(), dir.path(), &second_path, "second", "Trash 2").unwrap();

        // Both files must coexist under trash, distinctly.
        let trash = trash_dir(dir.path());
        assert!(trash.join("hello.md").exists());
        assert_ne!(trashed_second, trash.join("hello.md"));
        assert!(trashed_second.exists());
        assert_eq!(fs::read_to_string(&trashed_second).unwrap(), "---\nid: second\ntitle: Hello Again\n---\nTwo.\n");
    }

    #[test]
    fn restore_page_moves_the_file_back_and_preserves_the_id() {
        let dir = tempdir().unwrap();
        crate::vault::ensure_git_repo(dir.path()).unwrap();
        crate::author::confirm_test_author(dir.path());
        let repo = git2::Repository::open(dir.path()).unwrap();
        let page_path = write_page(dir.path(), "hello.md", "---\nid: abc\ntitle: Hello\n---\nBody.\n");
        crate::vault::commit_all(dir.path(), "Create").unwrap();

        trash_page(dir.path(), dir.path(), &page_path, "abc", "Trash Hello").unwrap();
        assert!(!page_path.exists());

        let head_before = repo.head().unwrap().peel_to_commit().unwrap().id();
        let restored = restore_page(dir.path(), dir.path(), "hello.md", "Restore Hello").unwrap();
        let head_after = repo.head().unwrap().peel_to_commit().unwrap().id();

        assert_eq!(restored, page_path);
        assert!(restored.exists());
        let content = fs::read_to_string(&restored).unwrap();
        assert!(content.contains("id: abc"));
        assert_ne!(head_before, head_after);
        assert_eq!(repo.head().unwrap().peel_to_commit().unwrap().message().unwrap(), "Restore Hello");

        // No longer listed as trashed.
        assert!(list_trashed_pages(dir.path()).unwrap().is_empty());
    }

    #[test]
    fn restore_page_recreates_a_missing_parent_directory() {
        let dir = tempdir().unwrap();
        crate::vault::ensure_git_repo(dir.path()).unwrap();
        crate::author::confirm_test_author(dir.path());
        fs::create_dir_all(dir.path().join("notes")).unwrap();
        let page_path = write_page(dir.path().join("notes").as_path(), "nested.md", "---\nid: n1\ntitle: Nested\n---\n");
        crate::vault::commit_all(dir.path(), "Create").unwrap();

        trash_page(dir.path(), dir.path(), &page_path, "n1", "Trash Nested").unwrap();
        // Remove the now-empty original directory, simulating it having
        // never existed by the time restore runs.
        fs::remove_dir(dir.path().join("notes")).unwrap();

        let restored = restore_page(dir.path(), dir.path(), "nested.md", "Restore Nested").unwrap();
        assert_eq!(restored, page_path);
        assert!(restored.exists());
    }

    #[test]
    fn empty_trash_permanently_removes_files_and_commits() {
        let dir = tempdir().unwrap();
        crate::vault::ensure_git_repo(dir.path()).unwrap();
        crate::author::confirm_test_author(dir.path());
        let repo = git2::Repository::open(dir.path()).unwrap();
        let page_path = write_page(dir.path(), "hello.md", "---\nid: abc\ntitle: Hello\n---\n");
        crate::vault::commit_all(dir.path(), "Create").unwrap();
        trash_page(dir.path(), dir.path(), &page_path, "abc", "Trash Hello").unwrap();

        let head_before = repo.head().unwrap().peel_to_commit().unwrap().id();
        empty_trash(dir.path(), dir.path()).unwrap();
        let head_after = repo.head().unwrap().peel_to_commit().unwrap().id();

        assert!(trash_dir(dir.path()).join("hello.md").is_file() == false);
        assert!(list_trashed_pages(dir.path()).unwrap().is_empty());
        assert_ne!(head_before, head_after);
        assert_eq!(repo.head().unwrap().peel_to_commit().unwrap().message().unwrap(), "Empty trash");
    }

    #[test]
    fn list_trashed_pages_reports_id_title_and_original_path() {
        let dir = tempdir().unwrap();
        crate::vault::ensure_git_repo(dir.path()).unwrap();
        crate::author::confirm_test_author(dir.path());
        let page_path = write_page(dir.path(), "hello.md", "---\nid: abc\ntitle: Hello World\n---\n");
        crate::vault::commit_all(dir.path(), "Create").unwrap();
        trash_page(dir.path(), dir.path(), &page_path, "abc", "Trash Hello").unwrap();

        let trashed = list_trashed_pages(dir.path()).unwrap();
        assert_eq!(trashed.len(), 1);
        assert_eq!(trashed[0].id, "abc");
        assert_eq!(trashed[0].title, "Hello World");
        assert_eq!(trashed[0].trashed_filename, "hello.md");
        assert_eq!(trashed[0].original_relative_path, "hello.md");
    }

    #[test]
    fn list_trashed_pages_is_empty_when_there_is_no_trash_dir() {
        let dir = tempdir().unwrap();
        crate::vault::ensure_git_repo(dir.path()).unwrap();
        crate::author::confirm_test_author(dir.path());
        assert!(list_trashed_pages(dir.path()).unwrap().is_empty());
    }
}
