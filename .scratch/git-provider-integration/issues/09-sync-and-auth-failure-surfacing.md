# Surfacing sync state and auth failure

Type: grilling
Status: resolved
Blocked by: 06, 08

## Question

What does the user see when sync is working, and — far more importantly — when authentication stops working?

`get_sync_status` and the `sync-status-changed` event already exist on the Rust side, and **no UI reads either one** (see docs/known-gaps.md). So today a token that expires, a revoked key, or a rejected push produces exactly nothing: background sync silently stops and the user's notes quietly stop reaching their other devices. For an app whose pitch is "you shouldn't have to think about git", silent failure is the worst possible outcome — the user is not thinking about git precisely because they were told not to.

Settle:

- **The healthy state.** Is there a persistent indicator, or does success stay invisible? Invisible success is usually right, but it leaves no place for failure to appear.
- **The failure taxonomy.** Auth rejected, network unreachable, push rejected (non-fast-forward), merge conflict detected (ADR-0006 already preserves a copy, but nothing tells the user it happened), no remote configured. These need different messages and different recovery actions; collapsing them into "sync failed" is what makes the current situation unfixable from the user's side.
- **Recovery paths.** Does an expired credential prompt a re-auth in place, and can the user trigger it themselves rather than waiting for the timer?
- **Intrusiveness.** Background sync failing is not an emergency — the notes are safe on disk, which is the whole point of ADR-0001. The indicator should reflect that without being so quiet it is never noticed.
- **Credential loss mid-flight** (graduated from the map's fog once [Where credentials live at rest](07-credential-storage-decision.md) resolved). Sync can hit these failures:
  - an expired or revoked token;
  - a key the host no longer accepts;
  - a GitLab/Bitbucket access token past its one-year limit;
  - a GitHub refresh token unused for 6 months;
  - "keychain locked" (background calls never prompt, so this needs an "Unlock and retry" action that takes the interactive path);
  - "keychain unreachable" (the keychain has disappeared since the connection was saved; the store never changes on its own);
  - "couldn't save your refreshed sign-in" (the new token pair is held only in memory and is lost when the app quits).

  See [ADR-0013](../../../docs/adr/0013-credentials-live-in-the-os-keychain-with-consented-plaintext-as-the-only-fallback.md).
- **The pre-connection state.** A vault with no remote at all is not an error; it is the default and a legitimate way to use Cerebrite forever. It must not nag.

## Answer

A persistent, quiet icon in the sidebar footer, never a toast or banner. It has five visual states: **not connected** (no remote — its own calm icon, never nags), **synced** (healthy, at rest), **syncing** (an attempt is running now), **retrying** (transient trouble, no user action needed) and **needs attention** (something requires the user to act). Clicking the icon opens a small popup — provider, last-synced time, current status text, and a link into Settings › Sync — never an unprompted interruption; the user finds out on their own schedule, consistent with "sync failing is not an emergency, the notes are safe on disk" (ADR-0001/0006).

**The two-bucket taxonomy.** Every failure collapses into exactly two user-facing buckets, not the full internal list:

- **Transient / retrying** — network unreachable. Auto-retries on the existing timer; no CTA, no user action possible or needed. The icon shows "retrying," visually distinct from "needs attention."
- **Needs attention** — everything that requires the user to do something: auth rejected/expired/revoked token, SSH key the host no longer accepts, a GitLab/Bitbucket token past its one-year limit, a GitHub refresh token stale after 6 months, keychain locked, keychain unreachable, "couldn't save your refreshed sign-in," merge conflict, push rejected (non-fast-forward), and any other unclassified sync error. All share the same icon look and the same popup entry point, but each gets its own message text and its own specific call-to-action:
  - **Credential rejected/expired/revoked** (any credential kind): "Reconnect" button that jumps directly into the credential-kind-specific re-auth flow from ticket 08/ADR-0012 — OAuth re-triggers the device flow, access token shows a paste field, SSH key offers regenerate/re-import — pre-filled with the already-known provider and repository, never restarting the full connect wizard from the top.
  - **Keychain locked**: "Unlock and retry" (per ADR-0013), taking the interactive unlock path.
  - **Keychain unreachable**: "Set up a keychain" / "Store as plaintext instead," reusing ADR-0013's consent dialog.
  - **Refreshed sign-in not saved**: a lower-key warning ("your renewed sign-in couldn't be saved and will be lost when the app quits — reconnect to fix") — sync is still working right now, so this is a heads-up, not a stop-everything failure, but it still lives in the needs-attention bucket rather than being silently dropped.
  - **Merge conflict**: message names the affected file(s); CTA opens/reveals `.cerebrite/conflict-backups/` and explains resolving with ordinary git tooling. No in-app resolution UI (out of scope per the map).
  - **Push rejected / non-fast-forward**: message explains the remote had changes that couldn't be reconciled automatically; CTA is a plain "Retry sync."

**Manual trigger.** A "Sync now" action is always available (in the popup and/or Settings › Sync), independent of the background timer and independent of current status — useful after fixing a problem or just wanting to push immediately.

**Pre-connection state.** A vault with no remote gets its own distinct, calm icon state — not literally invisible, but not alarming either. Settings › Sync shows a plain "Not connected — connect a repository" with the connect action; nothing nags the user in the main UI.

**Implementation prerequisite flagged for planning, not decided here:** `src-tauri/src/sync.rs` currently collapses every non-conflict failure (network errors, auth errors, anything else from git2) into one opaque `anyhow` error with no classification — there is no way today to tell "network unreachable" apart from "credentials rejected" in code. Splitting the transient/needs-attention buckets requires inspecting `git2::Error`'s class/code to route network-class failures to "retrying" and everything else to "needs attention," rather than the current single `SyncStatus::Error{detail}` catch-all. This also means `SyncStatus` itself needs a richer shape than today's five variants to carry the specific failure-cause + recovery-action pairing above (e.g. a `NeedsAttention` sub-kind per cause, or a decoupled recovery-action id) — left for implementation planning to design, not specified further here.
