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
}
