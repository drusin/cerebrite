# 10: Deletion & trash

**What to build:** Deleting a persisted page moves its file into `.cerebrite/trash/`, git-committed as a move and synced immediately like any other edit (per [ADR-0010](../../../docs/adr/0010-page-deletion-via-git-tracked-trash-folder.md)). No auto-purge — only an explicit "empty trash" action removes files for good. An explicit in-app restore moves a file back to its original path with its frontmatter id intact. While a page sits in trash, inbound links keep resolving, rendering an "in trash" state with a restore prompt; backlinks sourced from a trashed page are excluded from the target's backlinks section (ticket 06).

**Blocked by:** 06

**Status:** ready-for-agent

- [ ] Deleting a persisted page moves the file to `.cerebrite/trash/` as a git-tracked move, committed automatically
- [ ] Trashed pages don't appear in "All pages" (ticket 02) or default search results
- [ ] A link to a trashed page renders an "in trash" state with a restore prompt, not a broken link
- [ ] Restore moves the file back to its original path, preserving the frontmatter id, and the page becomes ordinary again
- [ ] Links/backlinks sourced from a page currently in trash are excluded from the backlinks section of pages they target
- [ ] Explicit "empty trash" permanently removes files; there is no other purge path
