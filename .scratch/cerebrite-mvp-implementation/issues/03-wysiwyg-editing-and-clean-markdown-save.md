# 03: WYSIWYG editing & clean-markdown save

**What to build:** Replace the read-only page view with an editable Milkdown instance (per [02-wysiwyg-editor-component](../../cerebrite-mvp/issues/02-wysiwyg-editor-component.md)). Saving writes clean markdown back to the file — no metadata outside YAML frontmatter, no injected markup into the body (per [ADR-0003](../../../docs/adr/0003-clean-markdown-excludes-round-trip-fidelity.md)) — and the save is auto-committed to the local git repo (per [ADR-0006](../../../docs/adr/0006-automatic-git-sync-with-conflict-detection-and-copy-safety-net.md); remote push/pull and conflict handling are ticket 14, not here).

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] Opening a persisted page shows it in an editable Milkdown surface
- [ ] Typing plain-text edge cases from the prototype (mid-sentence `>`, `[[...]]`-shaped text) round-trip to clean markdown on save — no HTML-entity escaping, no backslash-escaping
- [ ] Save writes the file to disk and creates a local git commit automatically, with no explicit "commit" step exposed to the user
- [ ] Derived index reflects the edit (at minimum: re-synced on next rebuild trigger)
