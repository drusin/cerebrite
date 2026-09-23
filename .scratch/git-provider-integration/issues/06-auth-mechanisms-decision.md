# Which auth mechanisms ship, and how they compose

Type: grilling
Status: resolved
Blocked by: 01, 02, 03

## Question

Given what tickets 01–03 found, which authentication mechanisms does Cerebrite actually ship, and how do they fit together into one coherent model rather than three parallel ones?

This is the central decision of the map. Everything downstream — storage, onboarding UX, failure handling — is shaped by it, and it is almost certainly worth an ADR.

Settle:

- **The shipping set.** SSH, HTTPS tokens, OAuth: which are in, and is any of them cut entirely? A mechanism that is merely *possible* is not automatically worth its maintenance cost and its share of the UI.
- **The relationship between OAuth and the transport.** If an OAuth token can serve directly as an HTTPS credential, OAuth and token auth are the same mechanism with two ways of acquiring the credential. If instead OAuth is used to upload an SSH key via the provider API, OAuth becomes a *setup* step for SSH rather than an auth mechanism at all. These are very different architectures and ticket 02's findings should decide between them.
- **Fallback vs. explicit choice.** `sync.rs` today silently tries agent → credential helper → default. Should the shipped model keep that implicit chain, or should a connection record exactly which mechanism it uses? Silent fallback is what makes auth failures unexplainable, which is most of why the current state is unusable.
- **What the two provider tiers concretely get.** "Effortless for GitHub/GitLab, works for everyone else" needs to become specific: the exact steps for each tier.
- **Existing manually-configured vaults.** Does the decided model adopt, override, or refuse a repo whose `origin` and credentials were set up by hand outside the app?

Standing constraints apply in full: no server component, nothing architecturally impossible on Android, no EOL dependencies.

## Answer

Written up as **[ADR-0012](../../../docs/adr/0012-explicit-per-vault-connection-with-three-credential-kinds.md)**. Two new glossary entries were added to **[CONTEXT.md](../../../CONTEXT.md)**: `Connection` and `Credential kind`.

### The shipping set

There are **three credential kinds over two transports**. None is cut, and no agent or credential-helper auth is kept:

- **OAuth sign-in** (GitHub/GitLab) produces a token sent over HTTPS Basic auth.
- **Access token** (pasted, any HTTPS host) is the same transport, obtained differently.
- **SSH key**, managed by Cerebrite. It is generated in the app (ed25519, no passphrase, the default) or imported. An imported key's passphrase is stored beside it so background sync keeps running. Host keys are pinned for GitHub/GitLab and confirmed on first contact (TOFU) for everything else.

### OAuth ↔ transport

**The OAuth token is the HTTPS credential**, as ticket 02 found. OAuth is not a setup step for SSH, and generated SSH keys are never uploaded through a provider API: on GitHub/GitLab the SSH kind is deliberately the manual escape hatch.

- **Flow:** the **Device Authorization Grant only**, on both providers. PKCE is deferred as a later polish, because it yields the same token.
- **GitHub: a GitHub App**, not an OAuth App. A follow-up check established the relevant facts:
  - A GitHub App user token reaches **no repository unless the app is installed there**, and device flow cannot trigger the install. So "install Cerebrite on your repository" is a required step in the journey.
  - Tokens last 8 hours. The refresh token lasts 6 months and rotates on use.
  - Refresh needs **no client secret for device-flow tokens** ([docs](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens)).
  - Token expiry stays on.
- **GitLab: a non-confidential app** with the device grant. Tokens last 2 hours and refresh rotates them. Three things are unverified and must be tested live before implementation:
  - Whether refresh works without a secret.
  - Whether the device grant returns a refresh token at all. The docs' sample response shows none.
  - Whether `write_repository` is enough to push. GitLab's docs now say it works for git-over-HTTPS with username `oauth2`, which contradicts ticket 02's `api` finding.

### Fallback vs. explicit choice

**Explicit.** A vault's **Connection** records exactly one credential kind. Sync uses only that credential, and a failure is reported as a failure of that kind. The agent → credential helper → `Cred::default()` chain in `sync.rs` is deleted, not reordered. No Connection is saved until a test fetch with its credential succeeds.

### What the two tiers get

**The URL's transport decides which kinds are valid:** `https` allows OAuth (on GitHub/GitLab) or an access token, and an SSH URL allows only an SSH key. On GitHub/GitLab, choosing a kind may rewrite `origin` to match. On any other host, the URL the user gave is final.

- **GitHub/GitLab:** "Sign in with GitHub/GitLab" → device code and URL → approve → (GitHub only) install the app on the repository → test fetch → saved. A secondary "Other ways to connect" option offers an access token or SSH key, for organisations that block third-party apps.
- **Everyone else:** paste the repository URL.
  - HTTPS: enter a username and token, with a link to the host's token page where known.
  - SSH: generate a key (the default) or import one. A generated key's public half is shown with a copy button and a link to the host's key page. The host key is confirmed on first contact.
  - Either way, a test fetch runs before the Connection is saved.

### Credential ownership

**Per vault.** Each Connection owns its credential, and there is no shared provider-account list. This holds while the app opens one vault at a time. Moving to shared accounts later means migrating stored secrets.

### Existing manually-configured vaults

**Not a use case.** Cerebrite has no users yet. A repository without a Connection is simply a new vault going through the connect journey. The root-level vault migration fog item is moot for the same reason.

### Handed on

- To [Where credentials live at rest](07-credential-storage-decision.md): the secrets to store are the OAuth access token plus its rotating refresh token, the pasted access token (plus username), the SSH private key, an imported key's passphrase, and Cerebrite's own pinned/TOFU host-key list (not secret, but it must persist).
- To [The connect and clone onboarding journey](08-connect-and-clone-journey.md): the tier steps above, including the required GitHub App install step and the test fetch before saving.
- To [Surfacing sync state and auth failure](09-sync-and-auth-failure-surfacing.md): every failure has exactly one named credential kind to blame. GitLab/Bitbucket access tokens expire within a year, and a GitHub refresh token dies after 6 months without use.
- A new ticket for the product half of git author identity: [Git author identity in the connect journey](11-git-author-identity-product.md).
