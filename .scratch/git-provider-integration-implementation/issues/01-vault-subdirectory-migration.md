# 01: Vault becomes a `vault/` subdirectory of its git repository

**What to build:** Today `vault.rs`'s `ensure_git_repo` `git init`s whatever folder the user picks, and pages live at the repository root. Per [ADR-0011](../../../docs/adr/0011-vault-is-a-hardcoded-subdirectory-of-its-git-repository.md), the vault becomes a hardcoded `vault/` subdirectory, and the user picks the *repository*, not the vault folder directly.

From the user's perspective:
- Picking a plain folder with no repository still works exactly as today, except pages now live under `<folder>/vault/` instead of at `<folder>/`.
- Picking a folder that sits inside an existing repository, but is not that repository's root, is refused with a message naming the actual repository root (use `Repository::discover`, not today's `Repository::open`, which currently nests a second repo silently).
- Picking a repository that already has non-Cerebrite content (a README, source code) at its root, but no `vault/`, adopts it: Cerebrite creates `vault/` inside it and treats the whole repository as the sync unit going forward.
- The developer's own existing root-level vault (and any other pre-migration vault) is migrated in place: its markdown pages move under a new `vault/` subdirectory without losing git history in a way a normal `git log --follow` can't track.
- `.cerebrite/redirects.tsv` and `.cerebrite/trash/` (ADR-0010) move to live under `vault/.cerebrite/...` instead of at the repository root.

**Blocked by:** None (can start immediately)

- [ ] `vault.rs::ensure_git_repo` uses `Repository::discover` and refuses (with the discovered root path in the error) when the picked folder is inside a repo but not its root
- [ ] A freshly picked plain folder gets `git init` + a `vault/` subdirectory; all page I/O (`index.rs`'s walk, page CRUD, `.cerebrite/` paths) operates under `vault/`, not the repo root
- [ ] A repository with existing non-Cerebrite root content and no `vault/` is adopted: `vault/` is created inside it, existing root files are left untouched and are not treated as pages
- [ ] A one-time migration moves an existing root-level vault's markdown files and `.cerebrite/` state into `vault/`, preserving git history
- [ ] `sync.rs`'s branch-name derivation guards against `head.shorthand()` returning `"HEAD"` on a detached HEAD (does not attempt to push `refs/heads/HEAD:refs/heads/HEAD`)
- [ ] Tests cover: plain-folder init, refusal-with-named-root for a non-root pick, adoption of a content-bearing repo, and the root-to-`vault/` migration
