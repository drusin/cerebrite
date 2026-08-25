# Redirect Log Format — Research (2026-08-25)

## Recommendation (summary)

**File**: `.cerebrite/redirects.tsv` at the vault root — a dotfolder, non-`.md` extension, so it's structurally excluded from any `*.md` page walk and reads as tool-owned housekeeping, the same way `.git/`, `.github/`, or an Obsidian vault's `.obsidian/` do.

**Line format**: one entry per line, tab-separated, keyed by *frontmatter page id* (ADR-0005) not file path:

```
oldPageId#oldSlug<TAB>newPageId#newSlug<TAB>rfc3339-timestamp<TAB>deviceId
```

Only the first two fields are load-bearing (mirrors the prototype's `redirects[oldKey] = {toPageId, toSlug}`); the trailing timestamp/device-id are advisory, for human debugging during conflict resolution, and are never read by `resolve()` or the pruning logic.

**Write discipline — the one correction this research makes to ADR-0007's stated assumption**: entries must be appended in **sorted order by `oldPageId#oldSlug`**, not blindly at end-of-file. A direct empirical test (below, "Finding 1") shows blind end-of-file append makes git's default merge produce **false-positive conflicts between two completely unrelated, independent heading renames** — not the "ordinary non-conflicting line additions" ADR-0007 and the ticket assume. Sorted insertion fixes this in the common case by giving each new line distinct anchor context, while still leaving the one case that *must* conflict — the same heading renamed differently on two devices — conflicting exactly as required, because both devices then insert at the identical sorted position. Existing lines are never edited in place; the only two mutations are "insert a new line" and "delete a line whole" (pruning).

**Location**: `.cerebrite/redirects.tsv`, git-tracked (ADR-0007's exception to "derived index is never git-synced," per ADR-0008), never in app-data.

**Pruning trigger**: an entry is safe to delete once no link anywhere in the corpus resolves through it *and* it's not the `fromKey` of any queued pending rewrite — computed as a byproduct of the derived-index rebuild that already walks all ~500 files on every launch and every completed sync (ADR-0008), not a separate scan. Pruning is itself a file mutation and needs its own commit, batched with the same rebuild cycle's cleanup-job rewrites. A genuine cross-device race exists (device A can prune a redirect that device B still silently depends on) and is not fully closable without a sync consensus mechanism out of MVP's scope — see Q3 below for why the failure mode this produces (a link going *broken*, not silently wrong) is an acceptable, bounded risk.

**Conflict case**: confirmed, empirically, to reduce to an ordinary git merge conflict on `.cerebrite/redirects.tsv`, handled by ADR-0006's existing detect + copy-safety-net mechanism — no bespoke conflict logic needed. But the boundary between "merges cleanly" and "conflicts" is not exactly where the ticket assumed; see Finding 1.

Confidence: **high** on the git-merge-mechanics claims (all verified directly against installed git 2.53.0, not taken on faith). **Medium** on the cross-device pruning race in Q3 — reasoned from the model, not independently testable without a real multi-device fixture, and explicitly flagged as a residual, accepted risk rather than a solved problem.

---

## Finding 1 (load-bearing): blind end-of-file append does *not* reliably avoid spurious conflicts

The ticket and ADR-0007 both assume concurrent renames on different devices produce "ordinary non-conflicting line additions." This was tested directly rather than assumed, per the ticket's own instruction to verify via `git merge-file`/`git merge` experiments in a scratch directory (git 2.53.0, default `ort` merge strategy, default `merge.conflictstyle`).

**Setup**: a 2-line base file (redirect log already has two entries). Two branches each append one new, unrelated line at the literal end of the file.

```
# base
p2#weekly-staples	p2#weekly-basics
p4#this-week	p4#current-week

# deviceA appends: p1#ingredients -> p1#ingredients-substitutions
# deviceB appends: p3#method      -> p3#cooking-method   (a totally different heading)
```

Real `git merge deviceA` from `deviceB`'s branch:

```
Auto-merging redirects.log
CONFLICT (content): Merge conflict in redirects.log
Automatic merge failed; fix conflicts and then commit the result.
```

Resulting file:
```
p2#weekly-staples	p2#weekly-basics
p4#this-week	p4#current-week
<<<<<<< HEAD
p3#method	p3#cooking-method
=======
p1#ingredients	p1#ingredients-substitutions
>>>>>>> deviceA
```

This is **not** a bug in the test — it's git's well-known "changelog conflict" behavior: git's line-based 3-way merge (libxdiff, used identically by `git merge-file` and by `git merge`'s content merge regardless of `ort`/`recursive` strategy — the strategy only decides which base(s) to diff against, not how a single file's hunks are combined) treats two insertions anchored at the *same* position (immediately after the same last common context line, with nothing between them) as two edits to the same "region," and has no way to know the two devices' intended order — so it conflicts rather than guessing. This is a widely-reported git gotcha for exactly this shape of file (two people appending different entries to the end of a shared log/changelog).

**Fix, verified directly**: keep the file sorted by `oldKey` and insert each new line at its correct sorted position instead of at EOF.

- Two *different* headings renamed on two devices, inserted at their correct (different) sorted positions → real `git merge` reports `Merge made by the 'ort' strategy` with **no conflict**, both lines present in sorted order. (Verified: base `p2#…`, `p4#…`; deviceA inserts `p1#…` before `p2#…`; deviceB inserts `p3#…` between `p2#…` and `p4#…`; clean merge.)
- The *same* heading renamed differently on two devices (same `fromKey`, sorting to the identical position on both sides) → real `git merge` still reports `CONFLICT (content)`, with both candidate destination lines shown as the two conflict sides — exactly the desired outcome (verified with `p1#ingredients -> p1#ingredients-substitutions` vs. `p1#ingredients -> p1#prep-notes`).

**Residual limitation, also verified**: sorted insertion narrows but does not eliminate false-positive conflicts. If the file is empty, or the two new keys happen to be sort-adjacent with no existing line between them, both insertions still land in the same single "gap" and still conflict even though the keys are unrelated (verified: an empty base file, deviceA adds `p1#aaa…`, deviceB adds `p9#zzz…` — still `CONFLICT (content)`, despite the keys being nowhere near each other alphabetically). This is intrinsic to line-based diffing, not fixable by choice of delimiter or column order — only a custom git merge driver with domain-aware (key-set-union) semantics could close it fully, and that would mean writing bespoke merge logic, which ADR-0006 explicitly keeps out of MVP scope ("no bespoke merge UI... resolved with normal git tooling"). The practical mitigation is accepting this: a spurious conflict here is always *safe* — a two-second "keep both incoming and current lines" resolution in whatever git tool the user already uses — never silent loss, and it gets rarer as the log accumulates more entries (more existing lines to fall between). Recommend leaving this as-is for MVP and revisiting only if it proves a real annoyance in practice.

---

## Q1: On-disk format

### Line shape

```
oldPageId#oldSlug<TAB>newPageId#newSlug<TAB>timestamp<TAB>deviceId
```

- **Keys are `pageId#slug`**, exactly matching the prototype's `keyOf(pageId, slug)` and the shape links themselves use — so a redirect target is drop-in resolvable the same way a link target is (`HeadingIdentityModel.resolve`).
- **`newPageId` may differ from `oldPageId`** — the prototype's `moveHeading` redirects across pages (a heading moved from `Recipes.md` to `Kitchen Notes.md`), so the format must support cross-page targets, ruling out any per-page-scoped file layout (see Q2).
- **Delimiter: tab.** Slugs are `[a-z0-9-]+` (per the prototype's `slugify`: lowercase, trim, collapse non-alphanumerics to `-`, strip leading/trailing `-`) and page ids, whatever their exact generation scheme (unspecified in the ADRs — no UUID/ULID/nanoid format is committed to anywhere in this repo), are near-universally hyphen/alphanumeric identifiers in every common scheme (UUID, ULID, nanoid all avoid whitespace and control characters by construction, since they must be filesystem- and URL-safe). A tab is a hard, non-printing character guaranteed absent from both fields regardless of exactly which id scheme is eventually chosen — no escaping logic is ever needed, unlike comma or `|`, which are visually plausible inside free-form text and would need escape-sequence handling if either field's format ever loosened. `#` is reserved as the *intra-key* separator (page id vs. slug), so it can't double as the *inter-key* separator without ambiguity — tab avoids colliding with either.
- **Timestamp/device-id as trailing columns**: safe to add because they're informational-only, read by nobody's resolve/prune logic, and adding columns to a *line* doesn't change git's line-based merge behavior at all — the merge algorithm still operates on whole lines, so extra columns can't introduce a new false-conflict shape beyond what already exists at the 2-column level. They earn their keep at conflict-resolution time: when a human is staring at a `<<<<<<<` block deciding which destination wins for the same `oldKey`, "which device, when" is exactly the disambiguating context a bare `oldKey\tnewKey` pair doesn't give them. Format: RFC 3339 UTC timestamp (`2026-08-25T14:03:11Z`, no embedded tab/newline) and a short device label (already sluggified, so it can't introduce a stray tab either).

### Append-only vs. in-place edits

Every mutation to the file must be classifiable, in git's own diff model, as either "one line added" or "one line removed" — never "one line's content changed." Concretely:

- **Adding a redirect** is always a brand-new line for a brand-new key. Confirmed against the prototype's `renameHeading`/`moveHeading`: the `oldKey` used is always derived from the heading's *current* live slug at the moment of that specific rename, so even a second rename of the same heading (before the first redirect is cleaned up) produces a distinct new `oldKey` chained onto the first — it never rewrites a previously-written line. A chain of N renames is N lines, never 1 line mutated N times.
- **Never "coalesce" a multi-hop chain into a single rewritten line.** `resolve()` already walks a chain of redirects to the live heading and the cleanup job already collapses that chain into one direct rewrite *in the referencing markdown file*. There must be no analogous "collapse the log itself" optimization — that would edit an existing line's content, breaking the append-only invariant that makes the conflict story in Finding 1 hold.
- **Removing a redirect (pruning)** deletes the entire line, never edits it down. See Q3.
- **Insertion position**: sorted by `oldKey`, not literal EOF-append — required for Finding 1's mitigation to apply. This is still "append-only" in the sense that matters (no existing byte is ever touched, no line is ever reordered relative to lines that already existed at write time) — it's just not naive tail-append.

### Pages addressed by frontmatter id, not path

Confirmed as the only correct choice, not merely convention-following. A page's file path can change independently of any heading rename inside it — that's precisely why ADR-0005 introduced the frontmatter id in the first place (so links survive a file move without redirecting). If the log instead keyed entries by path:

- A plain file move/rename (headings untouched) would force rewriting every redirect line that mentions that page's path, even though **zero** heading-identity redirection is actually needed for a pure file move — massively overprovisioning writes for an operation ADR-0005 already handles for free via the id.
- A heading rename and a later, independent file move on the *same* page would introduce a **second, uncoordinated staleness axis**: a path-keyed redirect entry would go stale the moment the file moves, on a completely different schedule than the cleanup job's own heading-tracking logic, which was only ever designed to detect "has every referencing file been rewritten," not "has the target page's path also drifted." The cleanup/pruning model in the prototype has no mechanism for this second failure mode at all.

Keying by frontmatter id keeps the redirect log in exactly the same address space links themselves already use (`pageId#slug`), so "does this key still resolve" is a single, uniform question — never two independent ones.

---

## Q2: Location

**`.cerebrite/redirects.tsv`**, at the vault root.

- **Dotfolder, not a bare dotfile at root**: `.cerebrite/` gives Cerebrite one obvious, growable namespace for all its own git-tracked-but-not-a-page state (this log now; anything else that turns up later that also needs to be synced but isn't content) — the same pattern as `.git/`, `.github/`, `.vscode/`, and (closest precedent for this exact kind of app) Obsidian's `.obsidian/` vault-config folder. Any tool, including a naive one, that's used to skipping dot-prefixed directories when walking a repo will skip this by default; Cerebrite's own page-walk should filter by `*.md` explicitly anyway (see next point), so this isn't relying on convention alone.
- **Non-`.md` extension is what actually matters for correctness**, not just tidiness: anything that iterates "linkable entities" via a `**/*.md` glob structurally cannot match a `.tsv` file regardless of which directory it lives in. Recommend `.tsv` over `.log` — the content is genuinely tab-separated data meant to be parsed, not prose meant to be read top-to-bottom; `.tsv` communicates the on-disk contract precisely (and pairs naturally with the tab delimiter chosen in Q1).
- **A single file at vault root, not one per page**: because redirects can cross pages (Q1), there's no natural single page to "own" a per-page sidecar for a redirect whose `oldKey` and `newKey` name two different pages. A single root file also means pruning/cleanup only ever has one file to reason about and commit, keeping the git history for this mechanism in one place rather than scattered across the tree (which would also multiply the false-conflict surface from Finding 1 across many small files instead of concentrating it in one).
- **Optional pairing**: a `.gitattributes` entry (e.g. `.cerebrite/redirects.tsv -diff`) so any tool that tries to apply markdown-aware diff/render heuristics to it doesn't. Minor, but cheap, and reinforces "this is not content" at the tooling layer too.

---

## Q3: Pruning trigger

**Exactly when**: a redirect entry for `oldKey` is safe to delete once (a) no link currently found anywhere in the corpus resolves through `oldKey` (directly or as part of a longer chain), and (b) `oldKey` is not the `fromKey` of any entry still sitting in the cleanup job's pending-rewrite queue. This is a direct restatement of the prototype's `pruneRedirects`, which computes exactly this `stillNeeded` set from `state.links` targets plus `state.pendingRewrites[].fromKey` and drops anything not in it.

**Cheap detection, no added scan**: fold this computation into the derived-index rebuild that already exists and already walks every one of the ~500 files, on launch and after every completed sync (ADR-0008) — the same pass that builds the backlinks table already visits every link in every file to record `(source, target)` pairs. Extending that same pass to also test each link's target against the (small — realistically low tens of entries at any moment, since they're pruned promptly) redirect-log key set is a hash-set membership check per link, not a second file walk. This is exactly the sub-second-rebuild constraint's own framing: "the log now part of what's read on rebuild" was already budgeted as part of that walk, and a few dozen short lines is negligible next to parsing ~500 markdown files. **Do not** implement a separate incremental reference-counter that's updated per-save outside the rebuild — that requires keeping a running count correct across edits, open buffers, and partial cleanup runs, which is strictly more moving parts than just recomputing it for free on a rebuild that already happens liberally (this mirrors research doc 01's own reasoning for why the derived index overall is "rebuild cheaply and often" rather than "maintain incrementally" at this corpus size).

**Scoping to only the files the cleanup job just touched** (the alternative the ticket raises) doesn't actually answer the question on its own: knowing file F no longer references `oldKey` after its own rewrite tells you nothing about whether some *other*, untouched file G still does. Answering that fully requires exactly the same corpus-wide reference check described above — so "scope to just-touched files" isn't a cheaper alternative to the full-rebuild-piggyback, it's a strictly weaker one that would need the full check anyway before it could actually prune.

**Is pruning a mutation needing its own commit?** Yes — deleting a line from `.cerebrite/redirects.tsv` is exactly as much a git-tracked write as the cleanup job's rewrite of a referencing page (the prototype's own cleanup-job log line calls this out: "this is a real file edit — it needs its own commit"). Recommend batching: run pruning once per rebuild cycle that found anything to prune, as one file write, committed together with that same cycle's cleanup-job rewrites (or immediately in the next scheduled auto-commit) — not a separate commit per individual redirect dropped, to keep commit-log noise proportional to actual cleanup activity.

**Can pruning race a concurrent device's in-flight rename?** Yes, and this is a real, only-partially-closable gap:

Walk-through: device A's rebuild computes `stillNeeded` purely from what A can currently see in its own pulled git history and its own local files. Suppose device B, not yet synced with A, still holds an unpushed link whose target is `oldKey` (B simply hasn't gotten to committing/pushing that link yet, or has been offline). From A's vantage point, `oldKey` looks completely unreferenced — A's `stillNeeded` computation has no way to see B's not-yet-shared state — so A prunes it and pushes. When B eventually syncs, B pulls a history where `oldKey`'s redirect line is already gone, while B's own link (now finally pushed, or already resolved locally against the stale key) still points at it. Crucially, this is **not** a textual git conflict the way Q4's same-key-rename race is: A deleted a line in `redirects.tsv`; B's conflicting dependency lives in an entirely different file that never touches `redirects.tsv` at all, so git has no overlapping hunk to flag. Nothing in the merge itself signals a problem.

This is a genuine race inherent to any eventually-consistent, server-less git-sync model with no central "who has pulled what" authority — closing it completely would require some form of cross-device consensus on "safe to prune" that Cerebrite's architecture (ADR-0006: no bespoke conflict UI, no server) deliberately doesn't have. Two things bound the actual damage, though:

1. **The failure mode is "link becomes broken," not "link silently resolves wrong."** Per the prototype's `resolve()`, a key with no live heading and no redirect returns `status: 'broken'` — a detectable, surfaceable state (the same state a link to a heading that was simply deleted outright would already produce), not a silently-wrong target. This is meaningfully different from the silent, no-trace failure ADR-0007 was written to prevent in the ephemeral-redirect-table design (that failure mode looked identical to "this link always worked fine," with no way to tell anything had gone wrong).
2. **A cheap mitigation, not a fix**: don't prune on the very first rebuild where an entry looks unreferenced — require it to still look unreferenced across at least one full sync round-trip (a push *and* a subsequent pull observed) since the entry was created, shrinking the window without pretending to close it for a device that's been offline far longer than that.

Given ADR-0006's explicit scope boundary (conflict *resolution* — and by extension, any bespoke cross-device coordination protocol — is out of scope; only detection + copy-safety-net is in), the recommended posture is: accept this as a known, low-probability, non-corrupting residual risk, document it, and don't build consensus machinery for MVP.

---

## Q4: Conflict case

**Scenario**: the same heading is renamed differently on two devices before either syncs.

**Confirmed, empirically** (see Finding 1's second experiment): under the recommended format (sorted-by-`oldKey` insertion), this reduces to an ordinary git merge conflict on `.cerebrite/redirects.tsv` — `git merge` reports `CONFLICT (content)` with both destination lines shown as the two sides of the conflict, verified directly against git 2.53.0's default `ort` strategy and default conflict style. ADR-0006's existing machinery — detect the conflicted file, save a copy of the user's pre-sync local version before automated sync touches it further — applies to this file exactly as it would to any other conflicted file; nothing bespoke is needed. The only implementation care point: whatever file-list the sync automation scans for conflict state must not special-case-exclude dotfiles/dotfolders on the assumption that "conflicts only matter for pages" — `.cerebrite/redirects.tsv` must be included in that sweep.

**Which append operations merge cleanly vs. which must not, precisely:**

| Scenario | `oldKey` | Result under sorted-insert format | Verified |
|---|---|---|---|
| Two *different* headings renamed on two devices | different on each side | Clean merge, both lines present, sorted | Yes — real `git merge`, `Merge made by the 'ort' strategy`, no conflict |
| Two different headings, but their sorted positions coincide (e.g., empty log, or sort-adjacent keys with nothing between them) | different on each side | **False-positive** conflict — safe (no data lost, trivial "keep both" resolution) but not clean | Yes — real `git merge`, `CONFLICT (content)`, even though keys are unrelated |
| The *same* heading renamed differently on two devices | identical on both sides | Conflict, correctly, every time (sorted-insert always lands both sides at the same position when the key is identical) | Yes — real `git merge`, `CONFLICT (content)`, both destination lines shown |

**Discipline required for the reduction to hold**: (1) a line's content, once written, is never edited in place — a line's `oldKey` is a fixed identity from creation until it is deleted whole; (2) new lines are inserted at a deterministic position (sorted by `oldKey`) rather than blind end-of-file append, so unrelated concurrent inserts get distinct anchor context whenever possible; (3) pruning removes whole lines only, never partial edits. Together these keep every possible mutation to the file expressible in git's diff model as "line added" or "line removed" — which is exactly the vocabulary in which "two adds at the same key must conflict, two adds at different keys should usually not" makes sense as a design target in the first place.

**Correcting the ticket's premise, explicitly**: the ticket (echoing ADR-0007's phrasing) frames "two independent renames → two independent line additions, no conflict" as the assumed default and only asks to double check the same-key collision case. Direct experimentation shows the reverse needed more scrutiny: naive end-of-file append does **not** reliably give independent renames a clean merge — it's a well-known git behavior (sometimes called the "changelog conflict" pattern) that two insertions anchored at the same position, with no context line between them, conflict regardless of whether their content is actually related. The same-key case, reassuringly, was never in doubt — it conflicts under *any* insertion discipline, sorted or not, because both sides insert at the identical position by construction. The format recommendation in Q1 (sorted-by-key insertion) exists specifically to make the "should merge cleanly" case behave as advertised as often as structurally possible, while the "must conflict" case was already safe without it.

---

## Confidence and caveats

- **High confidence** on all git-merge-mechanics claims in Finding 1 and Q4's table — each cell was independently reproduced with real `git merge` (not just `git merge-file` in isolation) against the actually-installed git 2.53.0, using the default `ort` strategy and default conflict style, with full command transcripts. This directly answers the ticket's explicit ask to verify rather than assume the "two different renames merge cleanly" claim, and the answer is more nuanced than the ticket's own framing assumed — that nuance (sorted-insertion as a required mitigation, and the residual false-conflict edge case) is the main actionable finding of this research.
- **Medium confidence** on Q3's cross-device pruning race: reasoned carefully from the prototype's own `pruneRedirects` model and the constraints already settled in the ADRs, but not independently verified against a real multi-device fixture (git has no built-in way to simulate "a device that hasn't pulled yet" the way it has a built-in way to simulate a merge). The conclusion — that this race is real, bounded to "broken link" rather than "silently wrong link," and acceptable to leave open per ADR-0006's scope — is a judgment call grounded in the settled architecture, not an empirically closed question.
- **Not verified**: the exact page-id generation scheme (UUID/ULID/nanoid/etc.) is not committed to anywhere in this repo as of this research — the tab-delimiter recommendation in Q1 is robust to any of the common choices, but if a future id scheme ever permitted embedded whitespace, that assumption would need revisiting.
- **Out of scope, correctly**: this research does not reopen paragraph-level linking (ADR-0002) or propose any custom git merge driver — both were explicitly ruled out of scope by prior decisions (the former by the prototype ticket, the latter by ADR-0006's "no bespoke merge logic" boundary), and the false-positive-conflict residual in Finding 1 is deliberately left as an accepted cost rather than an invitation to build one.

## Sources

- `docs/adr/0002-linking-granularity-fixed-at-heading-level-for-mvp.md`, `docs/adr/0003-clean-markdown-excludes-round-trip-fidelity.md`, `docs/adr/0005-page-per-file-with-frontmatter-id-and-slug-addressed-headings.md`, `docs/adr/0006-automatic-git-sync-with-conflict-detection-and-copy-safety-net.md`, `docs/adr/0007-persisted-redirect-log-for-heading-rename-identity.md`, `docs/adr/0008-tauri-rust-core-with-sqlite-fts5.md` — settled architecture this research builds on directly.
- `.scratch/cerebrite-mvp/issues/04-stable-heading-identity.prototype.html` — the `HeadingIdentityModel` module (`keyOf`, `renameHeading`, `moveHeading`, `resolve`, `pruneRedirects`), used throughout as the reference behavior the on-disk format must support.
- `.scratch/cerebrite-mvp/issues/04-stable-heading-identity-prototype.md`, `.scratch/cerebrite-mvp/issues/05-redirect-log-format.md`, `.scratch/cerebrite-mvp/map.md` — ticket framing and MVP constraints.
- `.scratch/cerebrite-mvp/research/01-tech-stack-and-core-architecture.md` — sub-second derived-index rebuild reasoning that Q3's pruning approach piggybacks on.
- Direct empirical verification against installed `git version 2.53.0` (`git merge-file`, `git merge` with real branches, default `ort` strategy, default conflict style) — five scratch-repo experiments run in this research session; see Finding 1 and Q4's table for transcripts and results. This is git's own behavior observed directly, per the ticket's own instruction to verify rather than assume it.
