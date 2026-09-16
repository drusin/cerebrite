Type: grilling
Status: resolved

## Question

How does a user create and delete a page in Cerebrite? Where does "new page" live in the UI (command, sidebar action, link-to-nonexistent-page-creates-it, etc.), what happens to a page's on-disk markdown file and any backlinks pointing at it on deletion, and how does this interact with the git-sync model (is delete a git-tracked operation, is there an undo/trash, etc.)?

## Answer

Pages have two lifecycle states — see [Dynamic page / Persisted page](../../../CONTEXT.md), [ADR-0009](../../../docs/adr/0009-dynamic-pages-materialize-on-first-write.md), [ADR-0010](../../../docs/adr/0010-page-deletion-via-git-tracked-trash-folder.md).

**Creation**: A link/tag to nonexistent text creates a UI-identical *dynamic page* (no file, no frontmatter id — identified by normalized link text), which persists to a real file + minted id on the user's first *write* (viewing/opening does not persist it). Separately, an explicit "new page" UI action persists immediately once a title is given, skipping the dynamic phase — frontmatter-only body, no auto-inserted title heading.

**Dynamic pages** need no explicit delete: they cease to exist once nothing references them, discovered on the next derived-index rebuild. They're searchable/browsable the same as persisted pages once the index is built (may lag briefly during/after a rebuild).

**Persisted pages**: filename is a slug of the title, re-slugified on every rename (safe — links resolve via frontmatter id, ADR-0005, not filename). Title collisions between two persisted pages are blocked with an in-app error at creation/rename time, not auto-suffixed.

**Deletion** (persisted pages only): moves the file to `.cerebrite/trash/`, git-committed as a move and synced immediately like any other edit (no delete-specific sync delay). No auto-purge — trash empties only when the user explicitly empties it. Explicit in-app restore action puts the file back at its original path with the same frontmatter id. While a page sits in trash, inbound links still resolve, rendering an "in trash" state with a restore prompt rather than appearing broken; only emptying the trash makes them dangling.
