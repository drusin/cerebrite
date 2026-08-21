Type: prototype
Status: resolved

## Question

Is there a clean-markdown-compatible way to give headings (and, ideally, paragraphs) a stable link identity that survives edits and moves — without violating [clean markdown](../../../CONTEXT.md) (no metadata outside frontmatter, per ADR-0003)?

Prototype this concrete scenario, using the temporary-mapping approach as the leading candidate:

1. A link is created targeting a sub-heading.
2. The sub-heading is then edited and/or moved (renamed, relocated within or across files).
3. The link continues to resolve correctly *immediately* after the change, via a temporary in-app mapping (not written into the file).
4. A background job subsequently rewrites all files that reference the moved/renamed sub-heading so their links point at the new heading text directly — at which point the temporary mapping for that heading is no longer needed and can be dropped.

Answer first whether *any* clean-markdown-compatible stable-identity approach is viable at all; if the mapping-plus-cleanup-job approach fails, explore why and whether a variant could work. This resolves the open question left by [ADR-0002](../../../docs/adr/0002-linking-granularity-fixed-at-heading-level-for-mvp.md) and [ADR-0005](../../../docs/adr/0005-page-per-file-with-frontmatter-id-and-slug-addressed-headings.md), and determines whether paragraph-level linking (deferred by ADR-0002) becomes viable for a future release.

## Answer

Yes, viable for headings/sub-headings — with one change to the leading candidate. The prototype (`04-stable-heading-identity.prototype.html`, in this directory) confirmed the redirect-plus-cleanup-job mechanism itself is sound: it correctly disambiguates slug collisions on move, and a single cleanup step collapses a multi-hop rename chain into one direct rewrite.

But a purely ephemeral, in-memory-only redirect table (the original leading candidate) is unsound: the derived index it would live in is rebuilt from the markdown files alone and never git-synced, so restarting the app or syncing to another device before the cleanup job drains a redirect silently breaks the link with no trace of why. Traced through all four scenarios in Node to confirm this isn't just a UI artifact of the demo.

Resolution: persist the redirect table itself in a small, git-synced, plaintext sidecar file — not one of the linkable pages, so it doesn't touch clean markdown (ADR-0003) — with one entry per line so concurrent renames on different devices merge as ordinary non-conflicting git diffs. "Temporary" now means "until the cleanup job has rewritten every referencing file," not "never persisted." Captured as [ADR-0007](../../../docs/adr/0007-persisted-redirect-log-for-heading-rename-identity.md); ADR-0002 and ADR-0005 updated to cross-reference it.

Paragraph-level linking (ADR-0002) does **not** graduate to viable. This mechanism only solves keeping an *existing* addressable key (heading text) resolvable across a rename — it presupposes a key to redirect from. Paragraphs have no natural key at all, which is the harder, prior problem this prototype didn't touch. Stays deferred.

Follow-on, now-specifiable question opened by this answer: the concrete on-disk format/location/pruning trigger for the redirect log — see [05-redirect-log-format](05-redirect-log-format.md).
