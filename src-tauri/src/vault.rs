// Vault selection/persistence and git-repo bootstrapping (ADR-0001: markdown
// is always synced via git, so every vault must be a git repo).
//
// Per ADR-0011, a vault is the hardcoded `vault/` subdirectory of a git
// repository, not the repository root itself: the user picks the
// *repository* folder, and `ensure_git_repo` is what turns that pick into
// the actual page directory (`<picked>/vault`), handling all four cases from
// the ticket -- fresh folder, non-root pick (refused), adoption of an
// existing repo, and one-time migration of a pre-ADR-0011 root-level vault.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use tauri::{AppHandle, Manager};

/// Fixed subdirectory name a vault lives under, relative to its repository's
/// root (ADR-0011). Never configurable -- see CONTEXT.md's "Vault" entry.
const VAULT_SUBDIR: &str = "vault";

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

/// Returns (and mints on first use) a stable random device id, persisted
/// alongside the remembered-vault-path file in the app's config dir (ticket
/// 08): redirect-log entries carry this so a human reading a merge conflict
/// on `redirects.tsv` can tell which device made which entry. Plain text, no
/// JSON wrapper needed for a single opaque string.
pub fn load_or_create_device_id(app: &AppHandle) -> Result<String> {
    let dir = app
        .path()
        .app_config_dir()
        .context("resolving app config dir")?;
    fs::create_dir_all(&dir).context("creating app config dir")?;
    let path = dir.join("device_id.txt");

    if let Ok(existing) = fs::read_to_string(&path) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let id = uuid::Uuid::new_v4().to_string();
    fs::write(&path, &id).context("writing device id")?;
    Ok(id)
}

/// Ensures `picked_path` is the *root* of a git repository (running `git
/// init` via git2 if it isn't a repository at all yet), then ensures that
/// repository has a `vault/` subdirectory -- creating, adopting, or
/// migrating into it as appropriate -- and returns the vault directory's
/// absolute path.
///
/// Uses `Repository::discover` (which walks upward looking for a `.git`),
/// not `Repository::open` (which only checks the exact path): this is what
/// lets a folder picked *inside* an existing repository be refused with the
/// real root named in the error, rather than silently `git init`-ing a
/// second, nested repository inside the first.
///
/// Four cases, matching the ticket:
/// 1. `picked_path` is not inside any git repository at all -- it is
///    `git init`'d and becomes the repository root.
/// 2. `picked_path` is inside a repository, but is not that repository's
///    root -- refused, naming the discovered root.
/// 3. `picked_path` already *is* a repository's root, with no `vault/` yet,
///    but with other content at its root (or none at all) -- `vault/` is
///    created and adopted; existing root files are left untouched.
/// 4. `picked_path` already *is* a repository's root, with no `vault/` yet,
///    and its root content looks like a pre-ADR-0011 vault (see
///    `looks_like_a_legacy_root_vault`) -- that content is moved into a
///    freshly created `vault/`, as a single commit, so a normal `git log
///    --follow` can still track each file's history through the move.
pub fn ensure_git_repo(picked_path: &Path) -> Result<PathBuf> {
    match git2::Repository::discover(picked_path) {
        Ok(repo) => {
            let discovered_root = repo
                .workdir()
                .context("discovered repository has no working directory (bare repositories aren't supported as vaults)")?
                .to_path_buf();

            // Compare canonicalized paths so e.g. a trailing slash or a
            // symlinked tempdir (common in tests) doesn't produce a false
            // "not the root" refusal.
            let canonical_discovered = discovered_root.canonicalize().unwrap_or(discovered_root.clone());
            let canonical_picked = picked_path.canonicalize().unwrap_or_else(|_| picked_path.to_path_buf());

            if canonical_discovered != canonical_picked {
                bail!(
                    "'{}' is inside an existing git repository rooted at '{}'. Pick that folder instead \
                     of a folder nested inside it.",
                    picked_path.display(),
                    discovered_root.display()
                );
            }
        }
        Err(_) => {
            git2::Repository::init(picked_path).context("git init on repository folder")?;
        }
    }

    let vault_dir = picked_path.join(VAULT_SUBDIR);
    if !vault_dir.exists() {
        if looks_like_a_legacy_root_vault(picked_path) {
            migrate_root_vault_into_subdirectory(picked_path, &vault_dir)
                .context("migrating existing root-level vault into vault/")?;
        } else {
            fs::create_dir_all(&vault_dir).context("creating vault/ subdirectory")?;
        }
    }

    Ok(vault_dir)
}

/// Distinguishes "this repository's root content *is* a pre-ADR-0011 vault
/// that needs migrating in place" from "this repository has unrelated
/// content (a README, source code) that should simply be left alone while
/// `vault/` is adopted beside it".
///
/// The signal used is the presence of a root-level `.cerebrite/` directory:
/// per CONTEXT.md, `.cerebrite/` (the redirect log, the trash folder) is
/// state Cerebrite itself creates as a byproduct of managing pages, so it
/// can only exist at a repository's root if an earlier, pre-migration
/// version of Cerebrite put it there. A generic repository with a `README.md`
/// or source files has no reason to ever have a root-level `.cerebrite/`.
/// A pre-migration vault that happens to have neither been renamed nor had
/// anything trashed yet (and so has no `.cerebrite/` at all) has no
/// distinguishing signal at all and is treated as ordinary content to adopt
/// beside a fresh `vault/` -- there is nothing on disk that would make that
/// wrong, since "no `.cerebrite/`" also means no redirects/trash state would
/// be lost by not migrating markdown files individually.
fn looks_like_a_legacy_root_vault(repo_root: &Path) -> bool {
    repo_root.join(".cerebrite").is_dir()
}

/// Moves every markdown page and the `.cerebrite/` directory currently
/// sitting at `repo_root`'s root into a freshly created `vault_dir`, then
/// commits the move as a single commit.
///
/// Doing every move as plain filesystem renames staged together in one
/// commit (rather than as separate delete-then-add commits) is what keeps
/// `git log --follow` able to track each file's history across the move:
/// git detects renames by content similarity between the two trees of a
/// single diff, not from any explicit "this was a rename" marker, so an
/// old-path-deleted/new-path-added pair only registers as a rename when
/// both sides land in the same commit.
fn migrate_root_vault_into_subdirectory(repo_root: &Path, vault_dir: &Path) -> Result<()> {
    fs::create_dir_all(vault_dir).context("creating vault/ subdirectory")?;

    for entry in fs::read_dir(repo_root).context("reading repository root")? {
        let entry = entry.context("reading repository root entry")?;
        let file_name = entry.file_name();

        // Never touch git's own metadata or the vault/ directory itself
        // (already handled/created above).
        if file_name == ".git" || file_name == VAULT_SUBDIR {
            continue;
        }

        let path = entry.path();
        let is_markdown_file = path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("md");
        let is_cerebrite_state = file_name == ".cerebrite";

        if is_markdown_file || is_cerebrite_state {
            let dest = vault_dir.join(&file_name);
            fs::rename(&path, &dest)
                .with_context(|| format!("moving '{}' into vault/", path.display()))?;
        }
    }

    commit_all(repo_root, "Migrate vault into vault/ subdirectory (ADR-0011)")
        .context("committing vault migration")?;

    Ok(())
}

/// Stages every change in the repository rooted at `repo_root` and creates a
/// local commit with `message` (ADR-0006: automatic git sync -- this is the
/// local-commit half only, no remote push/pull, which is out of scope per
/// ticket 14).
///
/// Commit scope is deliberately the *whole repository*, not just `vault/`
/// (ADR-0011): a repository-root file (a README, a `.cerebrite/` migrated
/// out of the vault by an older device, etc.) travels with the same commits
/// as the pages it sits beside.
///
/// A no-op (returns `Ok(())` without creating an empty commit) when there is
/// nothing to commit.
pub fn commit_all(repo_root: &Path, message: &str) -> Result<()> {
    let repo = git2::Repository::open(repo_root).context("opening vault git repo")?;

    let mut index = repo.index().context("reading repo index")?;
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .context("staging changes")?;
    // `add_all` alone never removes an index entry for a file that has been
    // deleted from the working tree (e.g. trashing/emptying-trash, ticket
    // 10's file moves and permanent removals) -- `update_all` covers exactly
    // that case, matching plain `git add -A` semantics.
    index
        .update_all(["*"].iter(), None)
        .context("staging deletions")?;
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
    fn ensure_git_repo_inits_a_fresh_folder_and_creates_vault_subdir() {
        let dir = tempdir().unwrap();
        assert!(git2::Repository::open(dir.path()).is_err());

        let vault_path = ensure_git_repo(dir.path()).unwrap();

        assert_eq!(vault_path, dir.path().join("vault"));
        assert!(vault_path.is_dir());
        // The repository root is the *picked* folder, not vault/.
        assert!(git2::Repository::open(dir.path()).is_ok());
    }

    #[test]
    fn ensure_git_repo_is_a_noop_on_an_existing_repo_that_already_has_a_vault() {
        let dir = tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        fs::create_dir_all(dir.path().join("vault")).unwrap();
        fs::write(dir.path().join("vault/existing.md"), "---\nid: x\n---\n").unwrap();

        let vault_path = ensure_git_repo(dir.path()).unwrap();

        assert_eq!(vault_path, dir.path().join("vault"));
        // Should not touch the existing vault contents.
        assert!(vault_path.join("existing.md").exists());
    }

    #[test]
    fn ensure_git_repo_refuses_a_folder_picked_inside_an_existing_repo_naming_the_real_root() {
        let dir = tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        let nested = dir.path().join("some").join("nested").join("folder");
        fs::create_dir_all(&nested).unwrap();

        let err = ensure_git_repo(&nested).unwrap_err();

        let message = err.to_string();
        let expected_root = dir.path().canonicalize().unwrap_or_else(|_| dir.path().to_path_buf());
        assert!(
            message.contains(&expected_root.display().to_string()),
            "expected the error to name the real repo root '{}', got: {message}",
            expected_root.display()
        );
    }

    #[test]
    fn ensure_git_repo_adopts_a_content_bearing_repo_leaving_existing_files_untouched() {
        let dir = tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        fs::write(dir.path().join("README.md"), "# My project\n").unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();

        let vault_path = ensure_git_repo(dir.path()).unwrap();

        assert_eq!(vault_path, dir.path().join("vault"));
        assert!(vault_path.is_dir());
        assert!(
            fs::read_dir(&vault_path).unwrap().next().is_none(),
            "a freshly adopted repo's vault/ should start empty"
        );
        // Existing root content is left exactly where it was -- not treated
        // as pages, not moved.
        assert!(dir.path().join("README.md").exists());
        assert!(dir.path().join("src/main.rs").exists());
    }

    #[test]
    fn ensure_git_repo_migrates_a_root_level_vault_into_a_vault_subdirectory() {
        let dir = tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        fs::write(dir.path().join("page.md"), "---\nid: abc\ntitle: Page\n---\nBody.\n").unwrap();
        fs::create_dir_all(dir.path().join(".cerebrite")).unwrap();
        fs::write(dir.path().join(".cerebrite/redirects.tsv"), "").unwrap();
        commit_all(dir.path(), "Initial commit").unwrap();
        let pre_migration_commit = repo.head().unwrap().peel_to_commit().unwrap();

        let vault_path = ensure_git_repo(dir.path()).unwrap();

        assert_eq!(vault_path, dir.path().join("vault"));
        assert!(vault_path.join("page.md").exists());
        assert!(vault_path.join(".cerebrite/redirects.tsv").exists());
        // The old root-level locations are gone.
        assert!(!dir.path().join("page.md").exists());
        assert!(!dir.path().join(".cerebrite").exists());

        let post_migration_commit = repo.head().unwrap().peel_to_commit().unwrap();
        assert_ne!(pre_migration_commit.id(), post_migration_commit.id());

        // The move must be detectable as a rename (what `git log --follow`
        // relies on), not merely as an unrelated delete + add.
        let mut diff = repo
            .diff_tree_to_tree(
                Some(&pre_migration_commit.tree().unwrap()),
                Some(&post_migration_commit.tree().unwrap()),
                None,
            )
            .unwrap();
        let mut find_opts = git2::DiffFindOptions::new();
        find_opts.renames(true);
        diff.find_similar(Some(&mut find_opts)).unwrap();

        let renamed = diff.deltas().any(|d| {
            d.status() == git2::Delta::Renamed
                && d.old_file().path().map(|p| p.to_string_lossy().to_string()) == Some("page.md".to_string())
                && d.new_file().path().map(|p| p.to_string_lossy().to_string())
                    == Some("vault/page.md".to_string())
        });
        assert!(renamed, "expected git to detect page.md -> vault/page.md as a rename");
    }

    #[test]
    fn ensure_git_repo_migration_is_a_single_commit() {
        let dir = tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        fs::write(dir.path().join("page.md"), "---\nid: abc\n---\nBody.\n").unwrap();
        fs::create_dir_all(dir.path().join(".cerebrite")).unwrap();
        fs::write(dir.path().join(".cerebrite/redirects.tsv"), "").unwrap();
        commit_all(dir.path(), "Initial commit").unwrap();
        let commit_count_before = commit_count(&repo);

        ensure_git_repo(dir.path()).unwrap();

        assert_eq!(commit_count(&repo), commit_count_before + 1);
    }

    fn commit_count(repo: &git2::Repository) -> usize {
        let mut revwalk = repo.revwalk().unwrap();
        revwalk.push_head().unwrap();
        revwalk.count()
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
