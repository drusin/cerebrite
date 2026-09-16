---
status: accepted
---

# Persisted-page deletion moves the file into a git-tracked trash folder, not `git rm` or the OS trash

Deleting a [persisted page](../../CONTEXT.md) needs a realistic undo path. Relying on git history alone (`git rm`, recover via `git log`/`git checkout`) is technically safe but not a real workflow for most users. Routing the delete through the OS trash/recycle bin instead would put the one step that actually removes a page outside git, conflicting with git being the sole source of truth (ADR-0001).

Instead, deleting a page moves its file into `.cerebrite/trash/`, committed as an ordinary git move and synced immediately in the background like any other edit (ADR-0006) — no delete-specific sync delay. The trash has no auto-purge: it only empties when the user explicitly empties it, and an explicit in-app restore action moves a file back to its original path with its frontmatter id intact. While a page sits in trash, links to it keep resolving — rendered with an "in trash" state and a restore prompt — rather than appearing broken, since the id and content are still present on disk; only emptying the trash makes them dangling.
