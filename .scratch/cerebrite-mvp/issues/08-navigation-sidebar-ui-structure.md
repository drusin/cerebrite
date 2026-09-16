Type: grilling
Status: open
Blocked by: 07

## Question

What's the top-level navigation / sidebar structure of the Cerebrite UI? What lists or trees does it show (all pages, recent pages, favorites/pinned, tags if any), how is it organized (flat list, folders, namespaces), and how does it need to adapt across the three platform targets (Windows, Linux, Android — e.g. collapsible on desktop vs. drawer on mobile)?

Note: [Page creation/deletion UX](07-page-creation-deletion-ux.md) settled that dynamic pages (link/tag to nonexistent text, no file yet — ADR-0009) are included in search/navigation the same as persisted pages once the index is built. Any "all pages" list here needs to account for that (e.g. does it distinguish dynamic from persisted pages visually, or are they truly indistinguishable?).
