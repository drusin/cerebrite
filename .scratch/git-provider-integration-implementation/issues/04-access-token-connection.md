# 04: Access-token HTTPS connection

**What to build:** The generic path for any HTTPS git host (Bitbucket Cloud, Gitea, Forgejo, Codeberg, a bare HTTPS host) — per [ticket 01's research](../../git-provider-integration/issues/01-https-credential-landscape.md), "paste a token" via `Cred::userpass_plaintext`.

From the user's perspective: given a repository URL, a username and a personal access token, the user connects a vault, a test fetch confirms the credential works before anything is saved, and background sync then pushes/pulls using that token over HTTP Basic auth. When the token expires (GitLab/Bitbucket enforce this within a year), the next sync attempt fails identifiably as this credential kind, not a generic error (full UI for that is ticket 13; this ticket just needs the failure to be classifiable).

This ticket's UI can be minimal/raw (a bare form) — the polished wizard experience is ticket 09/10; this establishes the mechanism end-to-end.

**Blocked by:** 03

- [ ] A form (however minimal) collects repository URL, username, and token, and calls a connect operation
- [ ] The connect operation runs a test fetch with `Cred::userpass_plaintext(username, token)` before saving the Connection; a failed test fetch shows an error and saves nothing
- [ ] On success, a Connection with credential kind "access token" is saved via ticket 02/03's infrastructure, and background sync (fetch/push) subsequently uses it with no further prompting
- [ ] An expired/rejected token surfaces through ticket 03's classification as a needs-attention failure naming this credential kind
- [ ] Works against a real non-GitHub/GitLab host in a manual smoke test (e.g. a Forgejo/Gitea instance or bare HTTPS test repo)
- [ ] Integration test: a fixture HTTPS remote requiring Basic auth, connect succeeds with correct token, fails with wrong token, and does not fall back to any other mechanism
