// Vault selection/persistence and git-repo bootstrapping (ADR-0001: markdown
// is always synced via git, so every vault must be a git repo).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize)]
struct VaultConfig {
    vault_path: String,
}

fn config_file_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .context("resolving app config dir")?;
    fs::create_dir_all(&dir).context("creating app config dir")?;
    Ok(dir.join("vault.json"))
}

/// Returns the app's data directory, where the derived SQLite index lives.
/// Never inside the vault -- the index is derived, not git-synced.
pub fn app_data_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .context("resolving app data dir")?;
    fs::create_dir_all(&dir).context("creating app data dir")?;
    Ok(dir)
}

/// Loads the previously remembered vault path, if any, and only if that
/// folder still exists.
pub fn load_remembered_vault(app: &AppHandle) -> Option<PathBuf> {
    let path = config_file_path(app).ok()?;
    let content = fs::read_to_string(path).ok()?;
    let cfg: VaultConfig = serde_json::from_str(&content).ok()?;
    let candidate = PathBuf::from(cfg.vault_path);
    candidate.is_dir().then_some(candidate)
}

/// Persists `vault_path` so it can be reopened automatically next launch.
pub fn persist_vault_path(app: &AppHandle, vault_path: &Path) -> Result<()> {
    let path = config_file_path(app)?;
    let cfg = VaultConfig {
        vault_path: vault_path.to_string_lossy().to_string(),
    };
    fs::write(path, serde_json::to_string_pretty(&cfg)?).context("writing vault config")?;
    Ok(())
}

/// Ensures `vault_path` is a git repository, running `git init` (via git2)
/// if it isn't one already.
pub fn ensure_git_repo(vault_path: &Path) -> Result<()> {
    if git2::Repository::open(vault_path).is_err() {
        git2::Repository::init(vault_path).context("git init on vault folder")?;
    }
    Ok(())
}

/// Stages every change in `vault_path` and creates a local commit with
/// `message` (ADR-0006: automatic git sync -- this is the local-commit half
/// only, no remote push/pull, which is out of scope per ticket 14).
///
/// A no-op (returns `Ok(())` without creating an empty commit) when there is
/// nothing to commit.
pub fn commit_all(vault_path: &Path, message: &str) -> Result<()> {
    let repo = git2::Repository::open(vault_path).context("opening vault git repo")?;

    let mut index = repo.index().context("reading repo index")?;
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .context("staging changes")?;
    index.write().context("writing repo index")?;
    let tree_id = index.write_tree().context("writing tree from index")?;

    let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());

    // Nothing to commit if the new tree is identical to the parent's tree.
    if let Some(parent) = &parent_commit {
        if parent.tree_id() == tree_id {
            return Ok(());
        }
    }

    let tree = repo.find_tree(tree_id).context("looking up written tree")?;
    let signature = repo
        .signature()
        .or_else(|_| git2::Signature::now("Cerebrite", "cerebrite@local"))
        .context("building commit signature")?;

    let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
    repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &parents)
        .context("creating commit")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ensure_git_repo_inits_a_fresh_folder() {
        let dir = tempdir().unwrap();
        assert!(git2::Repository::open(dir.path()).is_err());

        ensure_git_repo(dir.path()).unwrap();

        assert!(git2::Repository::open(dir.path()).is_ok());
    }

    #[test]
    fn ensure_git_repo_is_a_noop_on_an_existing_repo() {
        let dir = tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();

        // Should not error or blow away the existing repo.
        ensure_git_repo(dir.path()).unwrap();
        assert!(git2::Repository::open(dir.path()).is_ok());
    }

    #[test]
    fn commit_all_creates_a_commit_with_the_expected_message_and_diff() {
        let dir = tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        let file_path = dir.path().join("page.md");
        fs::write(&file_path, "---\nid: abc\n---\nOriginal body.\n").unwrap();
        commit_all(dir.path(), "Update Page").unwrap();

        let head_before = repo.head().unwrap().peel_to_commit().unwrap();

        fs::write(&file_path, "---\nid: abc\n---\nEdited body.\n").unwrap();
        commit_all(dir.path(), "Update Page").unwrap();

        let head_after = repo.head().unwrap().peel_to_commit().unwrap();
        assert_ne!(head_before.id(), head_after.id());
        assert_eq!(head_after.message().unwrap(), "Update Page");

        let diff = repo
            .diff_tree_to_tree(
                Some(&head_before.tree().unwrap()),
                Some(&head_after.tree().unwrap()),
                None,
            )
            .unwrap();
        assert_eq!(diff.deltas().len(), 1);

        let mut patch_text = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            patch_text.push_str(std::str::from_utf8(line.content()).unwrap());
            true
        })
        .unwrap();
        assert!(patch_text.contains("Edited body."));
    }

    #[test]
    fn commit_all_is_a_noop_when_nothing_changed() {
        let dir = tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        fs::write(dir.path().join("page.md"), "---\nid: abc\n---\nBody.\n").unwrap();
        commit_all(dir.path(), "Update Page").unwrap();
        let head_before = repo.head().unwrap().peel_to_commit().unwrap().id();

        // No file changes since the last commit -- must not create an empty commit.
        commit_all(dir.path(), "Update Page").unwrap();
        let head_after = repo.head().unwrap().peel_to_commit().unwrap().id();

        assert_eq!(head_before, head_after);
    }
}
