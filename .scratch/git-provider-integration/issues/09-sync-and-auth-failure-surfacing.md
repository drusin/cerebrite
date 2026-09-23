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
