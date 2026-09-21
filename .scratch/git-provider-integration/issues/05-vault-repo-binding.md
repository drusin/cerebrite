# What a vault is, in git terms

Type: grilling
Status: resolved

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

## Answer

The starting hypothesis (one vault = one repo, at the repo root) broke on the first probe: a vault at the repo root leaves nowhere to put a `README.md` or an `AGENTS.md` that belongs to the repository but is not a page. The resolution inverts it — the vault is a **subdirectory**, and the repository is the thing bound to a provider.

Written up as **[ADR-0011](../../../docs/adr/0011-vault-is-a-hardcoded-subdirectory-of-its-git-repository.md)**, with the glossary entry the ticket asked for added to **[CONTEXT.md](../../../CONTEXT.md)** (`Vault`, plus an explicit `_Avoid_` against using "vault" for the whole repository).

### Repo root, or subdirectory?

**Subdirectory, always** — the vault is `vault/` at the root of a git repository, a **hardcoded** path, never configurable and never the repository root itself. The user picks the *repository* folder:

- a plain folder in no repository is `init`ed and given a `vault/`, exactly as today's flow expects;
- a folder that sits inside an existing repository without being its root is **refused with an explanation naming the actual root** (`Repository::discover` finds it), replacing today's silent nested-`.git` bug in `ensure_git_repo` — which uses `Repository::open`, not `discover`, and so currently nests a second repository inside the first without saying so.

The path is hardcoded rather than chosen because it must be derivable with **no local configuration**: on device #2 the repository is cloned before any vault exists, so a constant is what lets the clone locate its pages at all. It also turns "is this repository already a vault?" into a one-line file-existence check, which is what decides the three remote states below.

### One vault, one repo?

**One vault, one repository, one remote (`origin`).** Two vaults cannot share a repository (the path is a constant, so they would collide). A vault does not span two remotes: multi-remote doubles the failure taxonomy [ticket 09](09-sync-and-auth-failure-surfacing.md) has to define — *which* remote failed, is the vault "synced" if one of two succeeded — for no MVP payoff. Remotes other than `origin` are ignored, not adopted, so a user who wants a mirror can add and push one with git themselves.

### Which branch?

**Left deliberately implicit, and not part of the binding.** A clone uses the remote's default branch; an existing repository uses whatever is checked out, dynamically, as `sync.rs:177` already does. The branch is not recorded and is not surfaced in the UI — a branch picker is a git concept leaking for near-zero benefit, and a user who wants a different branch checks it out with git.

Implementation hazard to carry forward, not a decision: `head.shorthand()` returns `"HEAD"` on a detached HEAD, so sync would push `refs/heads/HEAD:refs/heads/HEAD`. Needs guarding wherever the branch name is derived.

### Foreign files

**Allowed, and committed.** Two distinct halves, both accepted and both to be stated plainly rather than discovered:

- Foreign **`.md` under `vault/`** *are pages*. `index.rs:76` already walks recursively for `.md`, skipping dot-directories, so this is today's behaviour made explicit: a vault is not "the pages Cerebrite created", it is **every markdown file under `vault/`**. The connect/clone journey must not surprise anyone with that.
- Files **beside** `vault/` at the repository root are in the repository but are **not** pages. That separation is the entire reason the vault became a subdirectory.

### Commit and push scope

**Repo-wide, not vault-scoped.** `commit_all`'s `add_all(["*"]) + update_all` continues to stage from the **repository root**, so root-level files travel without the user reaching for `git commit` — which is the only thing that makes "a sensible place to put a README" actually useful. One staging rule beats two.

The cost is real and is handled by **disclosure, not mechanism**: Cerebrite auto-commits every change in the repository it adopted, so the pick UI names that repository explicitly ("Notes in `vault/` — Cerebrite will commit every change in the repository at `~/…`"). The catastrophic version (Cerebrite swallowing a codebase) is held off by the refusal rule above: the tool never wanders up into a repository the user did not deliberately point at.

### Where Cerebrite's own state lives

**Inside the vault**: `vault/.cerebrite/redirects.tsv` and `vault/.cerebrite/trash/` (ADR-0010). They are page state, not repository housekeeping, and keeping them there is what leaves the repository root clean for the user's files. `index.rs` already skips dot-directories, so they stay invisible to the page walk for free.

### Empty remote vs. populated remote

The hardcoded path makes all three cases a single check for `vault/` on the remote side:

| Remote state | Behaviour |
| --- | --- |
| **Empty** | Push local. Nothing to reconcile. |
| **Has `vault/`**, local has none | This is the clone journey — [ticket 08](08-connect-and-clone-journey.md). |
| **Has `vault/`**, local has its own with unrelated history | **Refuse.** Merging two independently-rooted page sets is a data-loss shape, not a merge. Tell the user to clone it into a new folder instead. |
| **Has content, no `vault/`** | **Adopt**: create `vault/` in the existing repository and push. A good outcome, not an edge case — this is how you add notes to a project you already have. |

### Consequences for the rest of the map

- Every downstream ticket can now say "the git repository" for the sync unit and "the vault" for the page set without ambiguity; no new coined term was needed (`vault repository` was considered and dropped — Cerebrite is git-first, so plainly calling it the git repository is the honest choice).
- [Ticket 08](08-connect-and-clone-journey.md) inherits a concrete clone target (repository → `vault/`) and the three remote states above as its branch points.
- This is a **migration** for any existing repository, including the developer's own: today's vaults have pages at the repository root. Handling that is implementation planning's problem, not this map's, but it must not be discovered late.
