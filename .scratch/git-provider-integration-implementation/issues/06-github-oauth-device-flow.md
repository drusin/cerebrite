# 06: GitHub OAuth device-flow sign-in

**What to build:** "Sign in with GitHub" per [ADR-0012](../../../docs/adr/0012-explicit-per-vault-connection-with-three-credential-kinds.md) and [ticket 02's research](../../git-provider-integration/issues/02-secretless-oauth-flows.md) — registering Cerebrite as a GitHub App and using the Device Authorization Grant, whose resulting token is used directly as the git HTTPS credential.

From the user's perspective: clicking "Sign in with GitHub" shows a device code and a URL to visit; after approving in the browser, if the app isn't yet installed on the target repository the user is walked through installing it (a required step — a GitHub App token reaches no repository it isn't installed on); a test fetch then confirms the credential and the Connection is saved. Background sync refreshes the token unattended using the no-secret-required refresh flow; the refresh token itself is rotated and re-stored on every use.

**Blocked by:** 03

- [ ] Cerebrite is registered as a GitHub App (not an OAuth App) scoped to "Contents: Read and write", with Device Flow enabled
- [ ] Device Authorization Grant flow implemented: request device/user code, poll for token, surface the code and verification URL to the user
- [ ] After token acquisition, the app checks whether it's installed on the target repository and, if not, walks the user to the GitHub installation URL before proceeding
- [ ] The resulting user access token is used directly as the HTTPS Basic-auth credential (no separate SSH-key-upload path)
- [ ] Token refresh (8-hour token, 6-month rotating refresh token, no client secret needed) runs unattended in the background sync path; each refresh's new pair is persisted via ticket 02's store, with ticket 02's "couldn't save your refresh" fallback-to-memory behavior on a write failure
- [ ] A test fetch runs before the Connection is saved
- [ ] A stale refresh token (unused 6+ months) or a revoked install surfaces through ticket 03's classification as a needs-attention failure naming this credential kind
- [ ] Manual smoke test against a real GitHub repository, covering both a repo the app is already installed on and one requiring the install step
- [ ] Integration test mocking the device-flow token exchange and refresh endpoints
