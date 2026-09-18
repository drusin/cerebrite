# Which auth mechanisms ship, and how they compose

Type: grilling
Status: open
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
