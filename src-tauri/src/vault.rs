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

/// Moves every markdown page (at any depth) and the `.cerebrite/` directory
/// currently sitting at `repo_root`'s root into a freshly created
/// `vault_dir`, then commits the move as a single commit.
///
/// Doing every move as plain filesystem renames staged together in one
/// commit (rather than as separate delete-then-add commits) is what keeps
/// `git log --follow` able to track each file's history across the move:
/// git detects renames by content similarity between the two trees of a
/// single diff, not from any explicit "this was a rename" marker, so an
/// old-path-deleted/new-path-added pair only registers as a rename when
/// both sides land in the same commit.
///
/// Code-review follow-up (ticket 01): this used to `fs::read_dir` only
/// `repo_root`'s *top level*, so a page organized under a subdirectory (e.g.
/// `repo_root/journal/2024-01-01.md`) was silently left behind -- the
/// top-level `journal` entry is a directory, not a `.md` file, and nothing
/// ever descended into it to find what was inside. `WalkDir` (already a
/// dependency, used the same way by `index.rs`) walks the whole tree
/// instead, so a markdown page is found and moved regardless of how deeply
/// it's nested, preserving its relative path (and therefore its
/// subdirectory structure) under `vault_dir`.
fn migrate_root_vault_into_subdirectory(repo_root: &Path, vault_dir: &Path) -> Result<()> {
    fs::create_dir_all(vault_dir).context("creating vault/ subdirectory")?;

    let walker = walkdir::WalkDir::new(repo_root).min_depth(1).into_iter().filter_entry(|entry| {
        let name = entry.file_name();
        // Never descend into git's own metadata, the vault/ directory
        // itself (already handled/created above), or `.cerebrite` (moved
        // wholesale, as a single directory rename, below -- walking into it
        // here would just re-discover its own files one at a time for no
        // benefit, and ADR-0011 never creates it anywhere but the
        // repository root).
        name != ".git" && name != VAULT_SUBDIR && name != ".cerebrite"
    });
    for entry in walker {
        let entry = entry.context("walking repository root")?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let relative = path
            .strip_prefix(repo_root)
            .context("computing a page's path relative to the repository root")?;
        let dest = vault_dir.join(relative);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating '{}'", parent.display()))?;
        }
        fs::rename(path, &dest).with_context(|| format!("moving '{}' into vault/", path.display()))?;
    }

    // `.cerebrite/`, if present, moves as a single directory rename (its
    // contents are never walked individually above) -- ADR-0011 only ever
    // creates it at the repository root, never nested.
    let cerebrite_dir = repo_root.join(".cerebrite");
    if cerebrite_dir.is_dir() {
        fs::rename(&cerebrite_dir, vault_dir.join(".cerebrite"))
            .with_context(|| format!("moving '{}' into vault/", cerebrite_dir.display()))?;
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
    // Ticket 08 / ticket 11: the `Cerebrite <cerebrite@local>` fallback that
    // used to live here is gone outright. `Repository::signature()` already
    // resolves `user.name`/`user.email` through git's normal layered lookup
    // (repo-local overriding global), so once a "Commit as" author has been
    // confirmed (`author::confirm`, which writes repo-locally) this always
    // succeeds; on a vault with no author configured anywhere -- repo-local
    // or global -- it fails with `NotFound`, and that failure is left to
    // propagate as an ordinary error rather than being papered over. The
    // caller (a Tauri command, or `sync`'s background loop) is responsible
    // for having already walked the user through confirming an author
    // before the first commit -- see `author.rs`'s module doc comment.
    let signature = repo
        .signature()
        .context("no commit author is confirmed for this vault yet -- confirm a \"Commit as\" name and email before committing")?;

    let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
    repo.commit(Some("HEAD"), &signature, &signature, message, &tree, &parents)
        .context("creating commit")?;

    Ok(())
}

/// Whether `repo_root` currently has a commit author available anywhere in
/// git's normal layered lookup (repo-local overriding global) -- exactly
/// what `Repository::signature()` (and therefore `commit_all`) needs to
/// actually succeed.
///
/// Code-review follow-up (ticket 08): lets a caller with optional,
/// best-effort committing to do (`redirects::cleanup_and_prune`'s byproduct
/// link-rewrite/redirect-prune commits, run unconditionally as part of
/// every vault open) check first and skip attempting it, instead of hard
/// failing the whole operation it's a byproduct of. `open_vault` must
/// succeed regardless of whether an author has been confirmed yet on this
/// device -- confirmation happens lazily, at first real-edit commit time,
/// via `commit_author_prefill`/`confirm_commit_author`, which both need a
/// vault already open to be reachable at all.
pub fn has_commit_author(repo_root: &Path) -> bool {
    git2::Repository::open(repo_root)
        .map(|repo| repo.signature().is_ok())
        .unwrap_or(false)
}

/// Ticket 10: which of the post-*clone* states a freshly cloned repository
/// ended up in, returned by `classify_cloned_repo` alongside the vault path
/// it settled on. Distinct from (but deliberately mirrors) the four cases
/// `ensure_git_repo`'s doc comment lists for a *locally picked* repo --
/// clone's starting point is a remote's history rather than a folder
/// already on disk, so the case split is slightly different: there is no
/// "picked a folder nested inside an existing repo" case (the destination
/// is always freshly cloned into, never discovered), but there is a new
/// "empty remote" case ensure_git_repo never sees (a locally picked folder
/// that isn't a repo yet always becomes one via `git init`, which is never
/// literally empty of *possibility* the way a freshly cloned empty remote
/// is).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClonedVaultState {
    /// The remote had no commits at all (`Repository::is_empty()`) -- there
    /// is nothing to adopt, so `vault/` is simply created fresh, exactly
    /// like `ensure_git_repo`'s fresh-folder case.
    EmptyRemote,
    /// The remote already has a `vault/` directory at its root -- the
    /// ordinary case, nothing to create or adopt.
    Ordinary,
    /// The remote has commits/content but no `vault/` at its root -- adopted
    /// exactly like `ensure_git_repo`'s case 3: `vault/` is created fresh
    /// and existing root content is left untouched beside it.
    Adopted,
}

/// Ticket 10's post-clone counterpart to `ensure_git_repo`: `repo_root` must
/// already be the root of a just-cloned repository (not merely discovered --
/// clone always makes the destination the root, so there is no "nested
/// folder" case to refuse the way `ensure_git_repo` does for a locally
/// picked path). Classifies which of the three adoptable states the clone
/// landed in and creates `vault/` where needed, or refuses with an
/// explanation for the one state judged *not* safely adoptable.
///
/// # The refusal case
///
/// The ticket asks for a fourth state -- "the remote has an unrelated
/// `vault/` history that looks pre-existing-but-foreign" -- refused rather
/// than silently adopted, and leaves the exact signal to implementation
/// judgement. Unlike `ensure_git_repo`'s `looks_like_a_legacy_root_vault`
/// (which has a real, previously-established signal to key off: a
/// root-level `.cerebrite/` directory only an earlier Cerebrite version
/// could have created), nothing about a remote's `vault/` directory's
/// *contents* reliably distinguishes "a foreign, unrelated vault/" from "an
/// ordinary Cerebrite vault/ that simply hasn't been touched yet" -- a
/// vault/ with zero pages looks identical either way, and one with pages
/// looks like an ordinary vault regardless of who created it. Rather than
/// invent a heuristic over content that has no real distinguishing signal
/// (and risk refusing a perfectly good ordinary clone, which is a worse
/// failure mode than under-refusing), this reserves "refused" for a
/// concrete bad state actually discovered while wiring this up: a root-level
/// path named `vault` that exists but *isn't a directory* (a plain file, a
/// symlink to one, etc). That state can never be produced by
/// `ensure_git_repo`/`commit_all` (which only ever create/write inside a
/// `vault/` directory), so it is unambiguous evidence of something else
/// entirely occupying that name -- genuinely incompatible, not just
/// unfamiliar -- and adopting "beside" it the way case 3 does isn't
/// possible (the directory `vault/` this app needs can't be created where a
/// file of the same name already sits). Every other "has vault/" case is
/// treated as the ordinary case, per the ticket's own permitted fallback.
pub fn classify_cloned_repo(repo_root: &Path) -> Result<(ClonedVaultState, PathBuf)> {
    let repo = git2::Repository::open(repo_root).context("opening freshly cloned repository")?;
    let vault_dir = repo_root.join(VAULT_SUBDIR);

    if repo
        .is_empty()
        .context("checking whether the cloned repository is empty")?
    {
        fs::create_dir_all(&vault_dir).context("creating vault/ subdirectory in an empty cloned repository")?;
        return Ok((ClonedVaultState::EmptyRemote, vault_dir));
    }

    if vault_dir.exists() {
        if vault_dir.is_dir() {
            return Ok((ClonedVaultState::Ordinary, vault_dir));
        }
        bail!(
            "The cloned repository has a 'vault' entry at its root that isn't a directory, so it \
             can't be used as a Cerebrite vault. Clone into a new, empty folder, or resolve that \
             conflict on the remote first."
        );
    }

    fs::create_dir_all(&vault_dir).context("creating vault/ subdirectory to adopt existing repository content")?;
    Ok((ClonedVaultState::Adopted, vault_dir))
}

/// Ticket 10: clones `remote_url` into `destination` (which must already
/// exist and be empty -- callers check this before calling, since a
/// friendlier message than libgit2's own is worth giving up front) using
/// `connection`'s one credential kind and nothing else, exactly the way
/// `sync::test_fetch` authenticates a plain test fetch for `connect_*`
/// (ADR-0012: no fallback chain). Per the ticket: "the clone itself IS the
/// test" -- there is no separate pre-flight fetch before this, a failed
/// clone (bad credential, unreachable remote, rejected host key) is the
/// same signal `try_connect`'s test fetch gives, just produced by an actual
/// clone instead of a bare `connect_auth` handshake.
pub fn clone_repo(
    remote_url: &str,
    destination: &Path,
    connection: &crate::connection::Connection,
    known_hosts_path: Option<&Path>,
) -> std::result::Result<git2::Repository, git2::Error> {
    let callbacks = connection.make_callbacks(known_hosts_path);
    let mut fetch_options = git2::FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fetch_options);
    builder.clone(remote_url, destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Redirects git2's system/global/XDG config search paths to an empty,
    /// nonexistent directory, once per process -- isolates
    /// `repo.signature()` (and any other git2 config lookup) in this test
    /// binary from whatever real `~/.gitconfig` happens to exist on the
    /// machine running the tests. The sandbox this ticket was implemented
    /// in has one (`user.name`/`user.email` set globally), which would
    /// otherwise make `commit_all_errors_when_no_author_is_confirmed_anywhere`
    /// below pass by accident -- picking up the *real* host identity rather
    /// than proving the "nowhere" case ticket 08 checklist item 10 asks for.
    /// Safe across tests/threads: every caller redirects to the same
    /// (nonexistent) path, so repeated/concurrent calls are idempotent, and
    /// no test in this crate relies on the real host global git config
    /// being reachable (every other fixture that needs an author confirms
    /// one repo-locally via `author::confirm_test_author`, which always
    /// wins over global regardless).
    pub(crate) fn isolate_from_host_git_config() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let empty = std::env::temp_dir().join("cerebrite-test-empty-gitconfig");
            fs::create_dir_all(&empty).expect("creating empty test gitconfig dir");
            for level in [git2::ConfigLevel::System, git2::ConfigLevel::Global, git2::ConfigLevel::XDG] {
                unsafe {
                    git2::opts::set_search_path(level, &empty).expect("redirecting git2 config search path");
                }
            }
        });
    }

    #[test]
    fn commit_all_errors_when_no_author_is_confirmed_anywhere() {
        isolate_from_host_git_config();
        let dir = tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        fs::write(dir.path().join("page.md"), "---\nid: abc\n---\nBody.\n").unwrap();

        let result = commit_all(dir.path(), "Create page");

        assert!(
            result.is_err(),
            "expected commit_all to error without a confirmed author, got {result:?}"
        );
        // No commit was created -- the repo still has no HEAD at all.
        let repo = git2::Repository::open(dir.path()).unwrap();
        assert!(repo.head().is_err());
    }

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
        crate::author::confirm_test_author(dir.path());
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
    fn ensure_git_repo_migration_moves_pages_nested_in_subdirectories_too() {
        // Code-review follow-up (ticket 01): the migration used to only
        // `read_dir` the repository root, so a page organized under a
        // subdirectory was silently left behind entirely -- never moved,
        // never even looked at.
        let dir = tempdir().unwrap();
        git2::Repository::init(dir.path()).unwrap();
        crate::author::confirm_test_author(dir.path());
        fs::write(dir.path().join("page.md"), "---\nid: abc\ntitle: Page\n---\nBody.\n").unwrap();
        fs::create_dir_all(dir.path().join("journal/2024")).unwrap();
        fs::write(
            dir.path().join("journal/entry.md"),
            "---\nid: j1\ntitle: Entry\n---\nBody.\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("journal/2024/deep.md"),
            "---\nid: j2\ntitle: Deep Entry\n---\nBody.\n",
        )
        .unwrap();
        fs::create_dir_all(dir.path().join(".cerebrite")).unwrap();
        fs::write(dir.path().join(".cerebrite/redirects.tsv"), "").unwrap();
        commit_all(dir.path(), "Initial commit").unwrap();

        let vault_path = ensure_git_repo(dir.path()).unwrap();

        assert!(vault_path.join("page.md").exists());
        assert!(vault_path.join("journal/entry.md").exists());
        assert!(vault_path.join("journal/2024/deep.md").exists());
        assert!(vault_path.join(".cerebrite/redirects.tsv").exists());

        // Nothing left behind at the old root-level locations.
        assert!(!dir.path().join("page.md").exists());
        assert!(!dir.path().join("journal/entry.md").exists());
        assert!(!dir.path().join("journal/2024/deep.md").exists());
        assert!(!dir.path().join(".cerebrite").exists());
    }

    #[test]
    fn ensure_git_repo_migration_is_a_single_commit() {
        let dir = tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        crate::author::confirm_test_author(dir.path());
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
        crate::author::confirm_test_author(dir.path());

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
        crate::author::confirm_test_author(dir.path());

        fs::write(dir.path().join("page.md"), "---\nid: abc\n---\nBody.\n").unwrap();
        commit_all(dir.path(), "Update Page").unwrap();
        let head_before = repo.head().unwrap().peel_to_commit().unwrap().id();

        // No file changes since the last commit -- must not create an empty commit.
        commit_all(dir.path(), "Update Page").unwrap();
        let head_after = repo.head().unwrap().peel_to_commit().unwrap().id();

        assert_eq!(head_before, head_after);
    }

    // -- ticket 10: clone_repo / classify_cloned_repo ----------------------
    //
    // A local bare repo (same fixture pattern as `connection.rs`'s
    // `try_connect` tests) needs no credentials at all, so these exercise
    // `clone_repo`'s real libgit2 clone end to end without any network
    // dependency, and `classify_cloned_repo`'s four post-clone states.

    fn test_connection() -> crate::connection::Connection {
        let record = crate::connection_record::ConnectionRecord {
            connection_id: "clone-test".to_string(),
            credential_kind: crate::connection_record::CredentialKind::AccessToken,
            provider: crate::connection_record::Provider::Other("test-host".to_string()),
            https_username: Some("dawid".to_string()),
            token_expiry: None,
            credential_store: crate::connection_record::StoreKind::Plaintext,
        };
        crate::connection::Connection::from_parts(record, b"unused-against-a-local-remote".to_vec())
    }

    #[test]
    fn classify_cloned_repo_creates_vault_fresh_from_an_empty_remote() {
        let bare_dir = tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();

        let destination = tempdir().unwrap();
        // `tempdir()` already creates an empty directory -- clone into it
        // directly, matching what the real command does once it has
        // verified the picked destination is empty.
        fs::remove_dir(destination.path()).unwrap();
        let repo = clone_repo(
            bare_dir.path().to_str().unwrap(),
            destination.path(),
            &test_connection(),
            None,
        )
        .unwrap();
        drop(repo);

        let (state, vault_path) = classify_cloned_repo(destination.path()).unwrap();
        assert_eq!(state, ClonedVaultState::EmptyRemote);
        assert_eq!(vault_path, destination.path().join("vault"));
        assert!(vault_path.is_dir());
        assert!(fs::read_dir(&vault_path).unwrap().next().is_none());
    }

    #[test]
    fn classify_cloned_repo_is_the_ordinary_case_when_the_remote_already_has_a_vault_directory() {
        let source_dir = tempdir().unwrap();
        git2::Repository::init(source_dir.path()).unwrap();
        crate::author::confirm_test_author(source_dir.path());
        fs::create_dir_all(source_dir.path().join("vault")).unwrap();
        fs::write(source_dir.path().join("vault/existing.md"), "---\nid: x\n---\nHi.\n").unwrap();
        commit_all(source_dir.path(), "Initial commit").unwrap();

        let bare_dir = tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();
        push_to_bare(source_dir.path(), bare_dir.path());

        let destination = tempdir().unwrap();
        fs::remove_dir(destination.path()).unwrap();
        let repo = clone_repo(
            bare_dir.path().to_str().unwrap(),
            destination.path(),
            &test_connection(),
            None,
        )
        .unwrap();
        drop(repo);

        let (state, vault_path) = classify_cloned_repo(destination.path()).unwrap();
        assert_eq!(state, ClonedVaultState::Ordinary);
        assert_eq!(vault_path, destination.path().join("vault"));
        assert!(vault_path.join("existing.md").exists());
    }

    #[test]
    fn classify_cloned_repo_adopts_a_content_bearing_remote_with_no_vault_directory() {
        let source_dir = tempdir().unwrap();
        git2::Repository::init(source_dir.path()).unwrap();
        crate::author::confirm_test_author(source_dir.path());
        fs::write(source_dir.path().join("README.md"), "# A project\n").unwrap();
        commit_all(source_dir.path(), "Initial commit").unwrap();

        let bare_dir = tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();
        push_to_bare(source_dir.path(), bare_dir.path());

        let destination = tempdir().unwrap();
        fs::remove_dir(destination.path()).unwrap();
        let repo = clone_repo(
            bare_dir.path().to_str().unwrap(),
            destination.path(),
            &test_connection(),
            None,
        )
        .unwrap();
        drop(repo);

        let (state, vault_path) = classify_cloned_repo(destination.path()).unwrap();
        assert_eq!(state, ClonedVaultState::Adopted);
        assert_eq!(vault_path, destination.path().join("vault"));
        assert!(vault_path.is_dir());
        assert!(fs::read_dir(&vault_path).unwrap().next().is_none());
        // Existing root content is left exactly where it was.
        assert!(destination.path().join("README.md").exists());
    }

    #[test]
    fn classify_cloned_repo_refuses_a_remote_where_vault_is_not_a_directory() {
        let source_dir = tempdir().unwrap();
        git2::Repository::init(source_dir.path()).unwrap();
        crate::author::confirm_test_author(source_dir.path());
        // A root-level *file* named `vault` -- a concrete, unambiguous
        // conflict no real `ensure_git_repo`/`commit_all`-managed vault
        // could ever have produced (see `classify_cloned_repo`'s doc
        // comment for why this is the refusal signal chosen).
        fs::write(source_dir.path().join("vault"), "not a directory\n").unwrap();
        commit_all(source_dir.path(), "Initial commit").unwrap();

        let bare_dir = tempdir().unwrap();
        git2::Repository::init_bare(bare_dir.path()).unwrap();
        push_to_bare(source_dir.path(), bare_dir.path());

        let destination = tempdir().unwrap();
        fs::remove_dir(destination.path()).unwrap();
        let repo = clone_repo(
            bare_dir.path().to_str().unwrap(),
            destination.path(),
            &test_connection(),
            None,
        )
        .unwrap();
        drop(repo);

        let err = classify_cloned_repo(destination.path()).unwrap_err();
        assert!(
            err.to_string().contains("isn't a directory"),
            "expected a refusal naming the non-directory vault entry, got: {err}"
        );
    }

    #[test]
    fn clone_repo_fails_cleanly_against_an_unreachable_remote() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let remote_url = format!("http://127.0.0.1:{port}/repo.git");

        let destination = tempdir().unwrap();
        fs::remove_dir(destination.path()).unwrap();

        let result = clone_repo(&remote_url, destination.path(), &test_connection(), None);

        assert!(result.is_err());
        // Nothing usable was left behind for classify to look at.
        assert!(!destination.path().exists() || fs::read_dir(destination.path()).unwrap().next().is_none());
    }

    /// Pushes `source_dir`'s current `HEAD` branch to `bare_dir` -- the same
    /// "push a real local repo to a bare remote" pattern `connection.rs`'s
    /// `try_connect_persists_the_record_and_secret_once_the_test_fetch_succeeds`
    /// test already uses, factored out here since several clone tests need
    /// it.
    fn push_to_bare(source_dir: &Path, bare_dir: &Path) {
        let repo = git2::Repository::open(source_dir).unwrap();
        repo.remote("origin", bare_dir.to_str().unwrap()).unwrap();
        let mut remote = repo.find_remote("origin").unwrap();
        let branch = repo.head().unwrap().shorthand().unwrap().to_string();
        remote
            .push(&[format!("refs/heads/{branch}:refs/heads/{branch}").as_str()], None)
            .unwrap();
    }
}
