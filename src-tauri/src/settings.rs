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
    pub connections: BTreeMap<String, StoreKind>,
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
/// in. Called whenever a secret is first stored and whenever "Move to
/// keychain" (ticket 02) succeeds.
///
/// No caller yet -- wiring a real connection into a vault starts at ticket
/// 04 -- so `#[allow(dead_code)]` is deliberate here, not an oversight.
#[allow(dead_code)]
pub fn set_connection_store(app: &AppHandle, connection_id: &str, store: StoreKind) -> Result<()> {
    let mut settings = load(app);
    settings.connections.insert(connection_id.to_string(), store);
    save(app, &settings)
}

/// Removes a connection ID from the index -- called on Disconnect (ticket
/// 14) and by "Remove all stored Cerebrite credentials" (ticket 02) once
/// the underlying secret has actually been deleted.
#[allow(dead_code)]
pub fn remove_connection(app: &AppHandle, connection_id: &str) -> Result<()> {
    let mut settings = load(app);
    settings.connections.remove(connection_id);
    save(app, &settings)
}
