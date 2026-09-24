# 13: Needs-attention failure surfacing & recovery CTAs

**What to build:** Per [ticket 09](../../git-provider-integration/issues/09-sync-and-auth-failure-surfacing.md), give each needs-attention cause its own message and call-to-action inside ticket 12's popup, instead of a single generic "needs attention" state.

From the user's perspective: when sync needs attention, the popup names the specific cause and offers the right fix in one click — a rejected/expired/revoked credential gets "Reconnect" that jumps straight into the matching credential kind's re-auth flow (OAuth re-triggers device flow, access token shows a paste field, SSH offers regenerate/re-import), pre-filled with the known provider and repository rather than restarting the wizard from scratch. A locked keychain gets "Unlock and retry"; an unreachable keychain gets "Set up a keychain" / "Store as plaintext instead" (ticket 02's consent dialog); a merge conflict names the affected file(s) and opens `.cerebrite/conflict-backups/`; a rejected push gets a plain "Retry sync".

**Blocked by:** 12, 04, 05, 06, 07, 02

- [ ] Popup message text and CTA vary by the specific needs-attention cause from ticket 03's classification, not a single generic string
- [ ] Credential rejected/expired/revoked (any kind): "Reconnect" jumps directly into that credential kind's re-auth flow, pre-filled with provider + repository, without restarting the connect wizard from the top
- [ ] Keychain locked: "Unlock and retry", taking the interactive unlock path from ticket 02
- [ ] Keychain unreachable: "Set up a keychain" / "Store as plaintext instead", reusing ticket 02's consent dialog
- [ ] "Refreshed sign-in not saved" (ticket 06/07's in-memory-only fallback firing): a lower-key warning distinct from a hard failure, still in the needs-attention bucket
- [ ] Merge conflict: message names the affected file(s); CTA opens/reveals `.cerebrite/conflict-backups/` with a note about resolving via ordinary git tooling (no in-app resolution UI)
- [ ] Push rejected / non-fast-forward: message explains the remote diverged; CTA is a plain "Retry sync"
- [ ] Manual QA: force each named cause (expired token, locked keychain, simulated conflict, force-pushed remote) and confirm the correct message + working CTA
