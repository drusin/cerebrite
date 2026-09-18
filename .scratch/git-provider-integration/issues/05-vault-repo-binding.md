# What a vault is, in git terms

Type: grilling
Status: open

## Question

What exactly is the unit being connected to a provider, and what shape of repository is a valid vault?

This has never been pinned down. `vault.rs`'s `ensure_git_repo` simply `Repository::init`s whatever folder the user picked, and `sync.rs` pushes whatever branch happens to be checked out. Every onboarding screen downstream depends on the answer, and **CONTEXT.md has no entry for "vault" at all** despite the term being used throughout the code — so this ticket should produce a glossary entry, not just a decision.

Settle:

- **Repo root, or subdirectory?** Is a vault always the root of its own repository, or may it be a folder inside a larger repo (notes living alongside a codebase, a docs/ directory in an existing project)? The latter is a real use case for a developer-facing tool, and it changes what "connect this vault to a remote" even means.
- **One vault, one repo?** Can two vaults share a repository? Can one vault span two remotes?
- **Which branch?** Does the user choose, or is it always the remote's default branch? `push_current_branch` currently implies "whatever is checked out", which is a decision made by accident rather than on purpose.
- **Foreign files.** Is a repository containing non-Cerebrite content (a README, source code, CI config) a valid vault? Cerebrite's automatic `add_all` + `update_all` commits *everything* in the folder, so a vault sharing a repo with a codebase would have Cerebrite auto-committing that codebase. Whatever is decided, this consequence needs facing directly.
- **Empty remote vs. populated remote.** Connecting to a brand-new empty repository, connecting to a repository that already holds someone else's vault, and connecting to a repository with unrelated content are three different situations with three different correct behaviours.

The starting hypothesis, agreed while charting, is the most restrictive one: **one vault = one repo, at the repo root, on the default branch**. Treat that as the position to argue against, not as settled — the ticket's job is to find where it breaks.
