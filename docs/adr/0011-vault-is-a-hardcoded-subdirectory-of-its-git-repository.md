---
status: accepted
---

# A vault is a hardcoded `vault/` subdirectory of its git repository, and Cerebrite commits the whole repository

Git is the source of truth (ADR-0001) and every change is committed and pushed automatically (ADR-0006), so the boundary between "files Cerebrite manages" and "files that merely live nearby" has to be drawn somewhere on disk. Until now it was drawn by accident: `ensure_git_repo` ran `Repository::init` on whatever folder the user picked, `commit_all` staged everything under it, and the page walk treated every `.md` beneath it as a page. That makes the picked folder simultaneously the repository, the page set, and the commit scope, with no room for a `README.md` or an `AGENTS.md` that is part of the repository but is not a page.

A [vault](../../CONTEXT.md) is therefore the directory `vault/` at the root of a git repository — a fixed path, not a configurable one. The user picks the *repository* folder; a folder that is not yet a repository is initialised and given a `vault/`, and a folder that sits inside an existing repository without being its root is refused with an error naming the actual root, rather than silently nesting a second repository inside the first. Everything under `vault/` is page state, including `.cerebrite/` with its redirect log and trash; everything beside it at the repository root belongs to the user.

The alternative of keeping the vault at the repository root was rejected precisely because it leaves nowhere to put repository-level files, and the alternative of letting the user choose the subdirectory was rejected because the path must be derivable with no local configuration: on a second device the repository is cloned before any vault exists, so a hardcoded path is what lets the clone locate its pages at all. It also makes "is this repository already a vault?" a single file-existence check, which is what decides whether connecting to a populated remote is a clone, an adoption, or a refusal.

Commit and push scope stays the **whole repository**, not just `vault/`. Repository-root files are only useful if they travel without the user reaching for `git commit`, and one staging rule is easier to reason about than two. The cost is real and must be disclosed rather than designed away: Cerebrite auto-commits every change in the repository it adopted, so the connect UI names that repository explicitly at pick time. Restricting the vault to a subdirectory of a repository the user deliberately chose is what keeps this tolerable — the tool never wanders up into a repository the user did not point at.

Which branch is synced is deliberately left implicit: a clone uses the remote's default branch, and an existing repository uses whatever is checked out. The branch is not recorded in a binding and is not surfaced in the UI; a user who wants a different branch checks it out with git.
