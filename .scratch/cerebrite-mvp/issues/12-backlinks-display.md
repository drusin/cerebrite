Type: grilling
Status: resolved
Blocked by: 04

## Question

How are backlinks displayed to the reader on a page? Dedicated panel/section vs. inline, grouped by source page or flat list, how much context is shown per backlink (full block, heading, snippet), and does this differ for heading-level backlinks now that [stable heading identity](04-stable-heading-identity-prototype.md) makes those trackable vs. page-level links?

## Answer

A single **Backlinks** section, always appended at the bottom of a page's rendered content — not a side panel, not collapsed by default. This holds for persisted pages, dynamic pages, and daily notes alike, since all three render identically.

**Rollup**: the section shows every backlink whose target is the current page itself *or* any heading/sub-heading within it, as one unified list — the reader thinks "who links to this page" as a single question, not one per heading. This is a display-time aggregation of the existing per-linkable-entity backlink data (see [ADR-0002](../../../docs/adr/0002-linking-granularity-fixed-at-heading-level-for-mvp.md)); it doesn't change what a backlink *is*, per [CONTEXT.md](../../../CONTEXT.md)'s "Backlink" definition.

**Grouping**: entries are grouped by source page, one group per distinct source page (avoids a flat list repeating the same source page's name when it links here multiple times, e.g. from a daily note).

**Per-entry target label**: within a group, an entry shows which heading it targeted (e.g. "→ Setup") when the link targets a heading; omitted when the link targets the page itself. Cheap to compute — the target heading is already tracked for the redirect log ([ADR-0007](../../../docs/adr/0007-persisted-redirect-log-for-heading-rename-identity.md)).

**Ordering**: groups ordered by source page's most-recent-modification (most recent first, mirroring the sidebar's "Recent"). Entries within a group in document order (top-to-bottom as they appear in the source file).

**Empty state**: a page with zero backlinks still renders the section, showing a single muted "No backlinks yet" line, rather than omitting the section — keeps page layout predictable and mirrors search's existing empty-state pattern.

**Snippet depth**: each entry shows a snippet of surrounding text around the link, reusing the same best-section-snippet mechanism already built for [search](11-search-ui-ux.md) — not the full source block/paragraph.

**Tags**: a tag-sourced backlink (`#tagname`) is fully indistinguishable from an ordinary `[[link]]`-sourced backlink — no icon, no separate label, per the [Tag](../../../CONTEXT.md) decision's "no separate identity or mechanism."

**Click-through**: clicking a backlink entry navigates to the source page and scrolls/jumps directly to the linking heading/paragraph, not just to the top of the page.

**Trash exclusion**: links living inside a page currently in `.cerebrite/trash/` are excluded from backlink sourcing entirely, consistent with search's default trash exclusion — a link from a page the user has thrown away isn't a reference they'd expect surfaced, and it avoids a confusing link-count change on restore.

Domain-modeling pass found no conflicts with CONTEXT.md and no new glossary entry or ADR warranted — this is a straightforward UI/behavior spec, not a hard-to-reverse architectural trade-off.
