// App-level settings (vault path + forced color scheme), persisted as
// `settings.json` in the app's config dir. Replaces the old single-purpose
// `vault.json` now that there's more than one app-level preference to
// persist -- no migration path needed, since there's no released version
// with an existing `vault.json` to preserve.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::connection_record::StoreKind;

/// One entry in `Settings::connections` -- which credential store holds a
/// connection's secret (ADR-0013), plus the repository path it belongs to.
/// The repo path (added by ticket 14) is what lets a startup scan tell
/// whether that repository still exists on disk without opening every vault
/// to read its own `.git/cerebrite/connection.json` -- see
/// `Settings::orphaned_connections`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionEntry {
    pub store: StoreKind,
    pub repo_path: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub vault_path: Option<String>,
    #[serde(default)]
    pub theme: Theme,
    /// Ticket 02 / ADR-0013: an app-wide index of every connection ID
    /// Cerebrite has ever stored a secret for, and which credential store
    /// (keychain or plaintext) currently holds it. This is *not* where the
    /// secret lives -- see `credential::KeychainBackend` /
    /// `credential::PlaintextStore` for that -- it exists so a startup scan
    /// can offer to clean up orphaned entries (their repository deleted
    /// outside the app) and so "Remove all stored Cerebrite credentials"
    /// knows what to remove without walking every vault on disk.
    #[serde(default)]
    pub connections: BTreeMap<String, ConnectionEntry>,
}

impl Settings {
    /// Ticket 14 checklist item 6: entries in `connections` whose repository
    /// no longer exists on disk (deleted or moved outside the app) -- what a
    /// startup scan surfaces for the "clean up orphaned credentials?"
    /// notice. Pure and AppHandle-free on purpose, so it's directly
    /// unit-testable; `scan_orphaned_connections` in `lib.rs` is the thin
    /// Tauri-command wrapper that loads `Settings` first.
    pub fn orphaned_connections(&self) -> Vec<(String, ConnectionEntry)> {
        self.connections
            .iter()
            .filter(|(_, entry)| !Path::new(&entry.repo_path).exists())
            .map(|(id, entry)| (id.clone(), entry.clone()))
            .collect()
    }
}

fn settings_file_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app
        .path()
        .app_config_dir()
        .context("resolving app config dir")?;
    fs::create_dir_all(&dir).context("creating app config dir")?;
    Ok(dir.join("settings.json"))
}

/// Loads persisted settings, falling back to defaults (no remembered vault,
/// `Theme::System`) if the file doesn't exist yet or fails to parse.
pub fn load(app: &AppHandle) -> Settings {
    settings_file_path(app)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn save(app: &AppHandle, settings: &Settings) -> Result<()> {
    let path = settings_file_path(app)?;
    fs::write(path, serde_json::to_string_pretty(settings)?).context("writing settings")?;
    Ok(())
}

/// Persists `vault_path` as the remembered vault, leaving every other
/// setting (e.g. `theme`) untouched -- switching vaults is a heavy, one-shot
/// operation (full index/git-repo rebuild), but it must not reset the rest
/// of the user's preferences.
pub fn set_vault_path(app: &AppHandle, vault_path: &Path) -> Result<()> {
    let mut settings = load(app);
    settings.vault_path = Some(vault_path.to_string_lossy().to_string());
    save(app, &settings)
}

/// Persists the forced color-scheme preference, leaving `vault_path`
/// untouched.
pub fn set_theme(app: &AppHandle, theme: Theme) -> Result<()> {
    let mut settings = load(app);
    settings.theme = theme;
    save(app, &settings)
}

/// Records (or updates) which credential store a connection ID's secret is
/// in, and which repository it belongs to (ticket 14's addition -- see
/// `ConnectionEntry`'s doc comment). Called whenever a secret is first
/// stored and whenever "Move to keychain" (ticket 02) succeeds.
pub fn set_connection_store(app: &AppHandle, connection_id: &str, store: StoreKind, repo_root: &Path) -> Result<()> {
    let mut settings = load(app);
    settings.connections.insert(
        connection_id.to_string(),
        ConnectionEntry {
            store,
            repo_path: repo_root.to_string_lossy().to_string(),
        },
    );
    save(app, &settings)
}

/// Removes a connection ID from the index -- called on Disconnect (ticket
/// 14) and by "Remove all stored Cerebrite credentials" (ticket 14) once
/// the underlying secret has actually been deleted.
pub fn remove_connection(app: &AppHandle, connection_id: &str) -> Result<()> {
    let mut settings = load(app);
    settings.connections.remove(connection_id);
    save(app, &settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn entry(repo_path: &str) -> ConnectionEntry {
        ConnectionEntry {
            store: StoreKind::Plaintext,
            repo_path: repo_path.to_string(),
        }
    }

    #[test]
    fn orphaned_connections_finds_entries_whose_repo_path_is_gone() {
        let mut settings = Settings::default();
        settings.connections.insert("conn-gone".to_string(), entry("/does/not/exist/anywhere"));

        let orphans = settings.orphaned_connections();

        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].0, "conn-gone");
    }

    #[test]
    fn orphaned_connections_excludes_entries_whose_repo_path_still_exists() {
        let dir = tempdir().unwrap();
        let mut settings = Settings::default();
        settings
            .connections
            .insert("conn-present".to_string(), entry(&dir.path().to_string_lossy()));

        assert!(settings.orphaned_connections().is_empty());
    }

    #[test]
    fn orphaned_connections_is_empty_when_there_are_no_connections() {
        assert!(Settings::default().orphaned_connections().is_empty());
    }
}
