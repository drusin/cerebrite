Type: grilling
Status: open
Blocked by: 01, 07

## Question

How is search presented to the user? Quick-switcher-style modal vs. dedicated search page/pane, how are SQLite FTS5 results ranked and grouped (by page, by matching block/heading, recency), what result metadata is shown (snippet/highlight, backlinks count), and how are results filtered (by tag if [tag system](09-tag-system.md) exists, by date, by page vs. block match)?

Note: [Page creation/deletion UX](07-page-creation-deletion-ux.md) settled that dynamic pages (no file yet — ADR-0009) surface in search the same as persisted pages once the index is built, and that a page sitting in trash still resolves (in an "in trash" state) rather than disappearing outright — this ticket should account for both when defining what a search result actually represents.
