---
status: accepted
---

# Dynamic pages: linking to nonexistent text materializes a file only on first write

Logseq-style linking means a `[[Link]]` or tag can point at a title nobody has written yet. The obvious approach — create a markdown file the moment such a link is typed — litters the vault with empty files for every passing mention, most of which are never elaborated on, and each would need a frontmatter id minted (ADR-0005) before there's any content to justify one.

Instead, a link to nonexistent text resolves to a [dynamic page](../../CONTEXT.md): no file, no frontmatter id, identified purely by the link's normalized text, but rendered with UI identical to a real page (title, empty body, backlinks panel) so the user can't tell the difference by looking. The moment the user performs any write on it — not merely opening it to view backlinks — it materializes: a file is created immediately with a freshly minted frontmatter id and a frontmatter-only body. From that point on it's an ordinary [persisted page](../../CONTEXT.md), addressed by id like any other.

The consequence: nothing needs to garbage-collect a dynamic page explicitly. It stops existing on its own once nothing links to it any more, discovered naturally on the next derived-index rebuild. Explicit page creation (a "new page" UI action with a chosen title) is the one path that skips the dynamic phase and persists immediately — title-only intent from an explicit create action is treated as sufficient, unlike incidentally typing a link.
