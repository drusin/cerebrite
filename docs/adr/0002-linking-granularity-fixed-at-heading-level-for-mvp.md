---
status: accepted
---

# Linking granularity fixed at page/heading level for MVP; paragraph-level deferred

Paragraph-level linking was the original ambition, but a paragraph has no natural stable identity: unlike a file path or a heading's text, it can be split, merged, or reordered with no anchor to hold onto. The obvious fix — an inline marker like `^block-id` — is exactly the kind of body-level metadata that [clean markdown](../../CONTEXT.md) rules out.

For the MVP, linkable entities are fixed at page, heading, and sub-heading granularity, all of which can be addressed by path and heading text without touching the body. Paragraph-level linking is deferred pending a prototype of a candidate identity mechanism (e.g., a temporary in-app mapping plus a background cleanup job for renamed headings) — this ADR should be revisited once that prototype has an answer.

That prototype ran and resolved the heading-rename half of the problem: see [ADR-0007](0007-persisted-redirect-log-for-heading-rename-identity.md). It doesn't touch paragraph-level linking, though — that's blocked on a different, harder problem than the one the prototype answered. A heading already has a natural addressable key (its text) that a redirect can map *from*; a paragraph has no such key to begin with, so there's nothing yet to redirect from, even before asking whether the redirect survives a rename. Paragraph-level linking stays deferred until that prior question — what would even serve as a paragraph's initial identity — has its own answer.
