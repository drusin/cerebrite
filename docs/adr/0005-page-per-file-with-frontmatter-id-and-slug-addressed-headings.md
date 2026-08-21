# Page-per-file layout, addressed by a frontmatter id; headings addressed by slug within the page

Each page is a single markdown file, identified by a stable id stored in its YAML frontmatter (so renaming or moving the file doesn't break links to the page). Headings and sub-headings within a page — the other linkable entities (ADR-0002) — are addressed by their slugified heading text rather than a second id, since adding an id per heading would violate [clean markdown](../../CONTEXT.md) (ADR-0003) by putting tool-owned metadata into the body.

This means a heading's link identity is tied to its text: renaming a heading changes what a link resolves to. [ADR-0007](0007-persisted-redirect-log-for-heading-rename-identity.md) resolves this with a redirect log the app maintains alongside the files, so a rename doesn't leave existing links dangling.
