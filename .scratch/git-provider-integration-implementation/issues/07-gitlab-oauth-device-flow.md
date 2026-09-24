# 07: GitLab OAuth device-flow sign-in

**What to build:** "Sign in with GitLab" — a non-confidential GitLab application using the device authorization grant (GitLab 17.1+), per [ADR-0012](../../../docs/adr/0012-explicit-per-vault-connection-with-three-credential-kinds.md). Three facts ticket 06 (the auth-mechanisms decision) flagged as unverified must be established live, first, before the rest of this ticket is built: whether refresh works with no client secret, whether the device grant actually returns a refresh token, and whether `write_repository` scope is sufficient to push (vs. the broader `api` scope ticket 02's research assumed).

From the user's perspective: clicking "Sign in with GitLab" shows a device code and URL; after approving, a test fetch confirms the credential and the Connection is saved. Background sync refreshes the token unattended if GitLab's device grant supports it; if it doesn't, the failure mode (re-prompting the user) is explicit rather than a silent break after 2 hours.

**Blocked by:** 03

- [ ] Spike/live test against gitlab.com, recorded in this ticket's resolution: does `write_repository` alone permit `git push` over HTTPS for an OAuth token; does the device-grant token response include a refresh token; does refresh succeed with no client secret. These findings decide the scope requested and whether unattended refresh is possible at all
- [ ] Cerebrite registered as a non-confidential ("public") GitLab application with the device grant enabled, requesting the scope the spike found sufficient
- [ ] Device Authorization Grant flow implemented: request device/user code, poll for token, surface the code and verification URL
- [ ] The resulting OAuth token is used directly as the HTTPS Basic-auth credential with username `oauth2`
- [ ] If refresh is possible: runs unattended in the background sync path, persisting the rotated pair via ticket 02, with the same in-memory-only fallback on a store-write failure as ticket 06
- [ ] If refresh is not possible (spike finding): the 2-hour token expiry surfaces through ticket 03's classification as a needs-attention "reconnect" failure, not a silent break
- [ ] A test fetch runs before the Connection is saved
- [ ] Manual smoke test against a real GitLab.com repository
- [ ] Integration test mocking the device-flow token exchange (and refresh, if the spike confirms it exists)
