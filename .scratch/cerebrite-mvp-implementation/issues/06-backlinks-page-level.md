# 06: Backlinks (page-level)

**What to build:** A "Backlinks" section always appended at the bottom of a page's rendered content (per [12-backlinks-display](../../cerebrite-mvp/issues/12-backlinks-display.md)), rolling up page-level inbound links, grouped by source page (most-recently-modified source first), with a snippet of surrounding text per entry. This ticket covers page-level links only — heading-level targets and their per-entry labels are ticket 07.

**Blocked by:** 05

**Status:** done

- [x] Every page renders a "Backlinks" section at the bottom, even with zero backlinks ("No backlinks yet" empty state)
- [x] The derived index tracks, for every link, its source page and target page
- [x] Backlink entries are grouped by source page, groups ordered by source page's most-recent-modification
- [x] Each entry shows a snippet of text around the link
- [x] Clicking an entry navigates to the source page

## Comments

Implemented: `src-tauri/src/links.rs` (regex-based `[[...]]` extraction, ported
from the frontend's `wiki-link-plugin.ts` pattern, with trailing `#heading`
fragments stripped per ticket 07 scoping); `index.rs`'s `backlinks` table now
stores `source_id` + `target_normalized_title` + `snippet` (replacing issue
02's placeholder `source_id`/`target_id` schema, since a link's target may be
a dynamic page with no row in `pages`); a `modified_at` column on `pages`
(file mtime on full rebuild, wall-clock "now" on incremental
insert/update) drives "most-recently-modified source first" ordering;
`replace_page_links` re-extracts and replaces a page's outbound links on
every body change (full rebuild, single-page save, and page
creation/materialization) so other pages' backlinks stay correct without a
full rebuild; new `get_backlinks` Tauri command, keyed by normalized title so
it works for dynamic (unmatched) targets too; frontend renders the section
under the editor for both persisted and dynamic pages, grouped by source
with a clickable snippet per entry that navigates to the source page.

Not covered (out of scope for this repo's current state): trash exclusion
(no trash feature exists yet) and heading-level per-entry labels (ticket 07).
No code-fence/inline-code exclusion in the Rust extractor (the frontend's
plugin has it via mdast ancestor-ignoring; the Rust regex scan doesn't have
an AST to consult) -- documented as an acceptable limitation per the ticket's
own guidance that a regex is the pragmatic approach here.
