---
status: accepted
---

# Each vault has one explicit connection with exactly one credential kind, and sync never falls back

Sync (ADR-0006) runs unattended, so when authentication fails the user has to be told *what* failed. The original `make_callbacks` in `sync.rs` made that impossible: on every fetch and push it tried the SSH agent, then git's credential helper, then `Cred::default()`, and used whichever worked. A failure meant "none of three guesses worked", which cannot be explained to someone who was promised they would not have to think about git.

Each [vault](../../CONTEXT.md) therefore has one [connection](../../CONTEXT.md): its repository's `origin` plus exactly one **credential kind** and the credential it holds. Sync uses that credential and nothing else. A failure is always a failure of a named thing ("your GitLab sign-in was rejected"). The fallback chain is deleted, not reordered.

Three credential kinds ship, over two transports:

- **OAuth sign-in** (GitHub and GitLab only). Uses the OAuth 2.0 Device Authorization Grant, the only flow that needs no client secret and no redirect plumbing and works unchanged on Android. The resulting token is sent as an HTTPS Basic-auth password. GitHub uses a **GitHub App**, not an OAuth App, so access is limited to the repositories the user installs it on. Its short-lived user token (8 hours) is refreshed with a 6-month rotating refresh token, which needs no client secret for device-flow tokens. The GitHub App install is a separate, required step: a device-flow token alone reaches no repository.
- **Access token** (any HTTPS host). The user pastes a token, and it is sent as an HTTPS Basic-auth password. This is the same transport as OAuth sign-in, obtained differently. It is the primary path for Gitea, Forgejo, Codeberg, Bitbucket and other HTTPS hosts. No provider accepts account passwords for git any more.
- **SSH key**, managed by Cerebrite. Either generated in the app (ed25519, no passphrase, the default) or imported. An imported key's passphrase, if it has one, is stored beside it so unattended sync keeps working. Host keys are pinned for GitHub and GitLab. For every other host they are confirmed by the user on first contact (TOFU) and persisted by Cerebrite. libssh2 does not verify host keys safely by default.

The remote URL's transport decides which kinds are valid: `https` allows OAuth sign-in (on GitHub/GitLab) or an access token, and an SSH URL allows an SSH key. On GitHub and GitLab, where both URL forms are known, choosing a kind may rewrite `origin` to match. For any other host, the URL the user gave is final. No connection is saved until a test fetch with its credential succeeds.

The credential belongs to the vault, not to a provider account shared between vaults. The app opens one vault at a time, and ADR-0011 fixes one vault to one repository and one `origin`.

## Considered options

- **SSH agent / system git credentials as a kind.** Rejected. Agent availability varies across platforms (the Windows agent is off by default, Linux depends on the desktop environment, Android has no agent), and it reintroduces a credential Cerebrite cannot see or explain. Users with existing keys import them instead.
- **HTTPS only (no SSH).** Rejected. Bare SSH hosts have no token concept, and they must work.
- **OAuth as a setup step for SSH** (sign in, upload a generated public key via the provider API, sync over SSH). Rejected. It puts all of SSH's burden (host keys, `ssh_key_from_memory`) on the tier that is meant to be effortless, and gains nothing over using the OAuth token directly. For the same reason, SSH keys are never uploaded through a provider API: on GitHub/GitLab, the SSH kind is the manual escape hatch.
- **PKCE with a loopback or deep-link redirect.** Deferred rather than rejected. It yields the same token, so it can replace the device-code step later as a polish without changing this model.
- **GitHub OAuth App.** Rejected in favour of a GitHub App. An OAuth App skips the install step but needs the blanket `repo` scope. New organisations block OAuth Apps by default anyway, so org repos need an owner's approval under both.

## Consequences

- The refresh token (GitHub, and GitLab if its device grant returns one) and any imported key's passphrase are secrets that must be stored with the credential. See the credential-storage decision.
- Registering the apps is implementation work. The requirements: a GitHub App with device flow enabled, `Contents: read and write`, user-token expiration left on, and a public install page. A GitLab non-confidential application with the device grant.
- Before implementation, verify against gitlab.com:
  - whether `write_repository` suffices for git-over-HTTPS with an OAuth token (GitLab's docs say yes; earlier research said `api` is needed);
  - whether the device grant returns a refresh token;
  - whether refresh works without a client secret.
- Provider identity (for git author name/email) is only reachable with OAuth sign-in. The other two kinds always need the user to supply it.
