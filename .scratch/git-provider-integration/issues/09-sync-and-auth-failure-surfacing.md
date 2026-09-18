# Surfacing sync state and auth failure

Type: grilling
Status: open
Blocked by: 06, 08

## Question

What does the user see when sync is working, and — far more importantly — when authentication stops working?

`get_sync_status` and the `sync-status-changed` event already exist on the Rust side, and **no UI reads either one** (see docs/known-gaps.md). So today a token that expires, a revoked key, or a rejected push produces exactly nothing: background sync silently stops and the user's notes quietly stop reaching their other devices. For an app whose pitch is "you shouldn't have to think about git", silent failure is the worst possible outcome — the user is not thinking about git precisely because they were told not to.

Settle:

- **The healthy state.** Is there a persistent indicator, or does success stay invisible? Invisible success is usually right, but it leaves no place for failure to appear.
- **The failure taxonomy.** Auth rejected, network unreachable, push rejected (non-fast-forward), merge conflict detected (ADR-0006 already preserves a copy, but nothing tells the user it happened), no remote configured. These need different messages and different recovery actions; collapsing them into "sync failed" is what makes the current situation unfixable from the user's side.
- **Recovery paths.** Does an expired credential prompt a re-auth in place, and can the user trigger it themselves rather than waiting for the timer?
- **Intrusiveness.** Background sync failing is not an emergency — the notes are safe on disk, which is the whole point of ADR-0001. The indicator should reflect that without being so quiet it is never noticed.
- **The pre-connection state.** A vault with no remote at all is not an error; it is the default and a legitimate way to use Cerebrite forever. It must not nag.
