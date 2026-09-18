# Secret-free OAuth flows for GitHub and GitLab

Type: research
Status: open

## Question

Under the standing "no server-side component, ever" constraint, is OAuth against GitHub and GitLab actually viable from a distributed desktop binary — and which flow?

A classic OAuth authorization-code web flow requires a client secret, which cannot be embedded in a binary users can unzip. This ticket establishes whether a secret-free path exists and what it costs the user.

Establish, against primary sources:

- **GitHub**: whether the device authorization flow (user visits a URL, types a short code) is available and what it requires; whether authorization-code + PKCE with a loopback redirect is supported for OAuth Apps without a secret. Also, the **GitHub App vs. OAuth App** distinction — which one a desktop client should register as, and how that changes token lifetime and scoping.
- **GitLab**: gitlab.com's support for PKCE with a loopback or custom-scheme redirect, and whether a device flow exists.
- For each viable flow: **which scopes grant git push/fetch over HTTPS**, whether the resulting token is directly usable as a git HTTPS credential (this is not a given — some providers issue API tokens that git will not accept), token lifetime, and whether a refresh token is issued and usable without a secret.
- **Redirect mechanics on each platform**: a loopback redirect means binding a localhost port, which is fine on desktop and awkward on Android; a custom URI scheme means registering a handler, which Tauri supports to varying degrees per platform. Device flow avoids both entirely. Note which flows survive the Android-possible constraint.
- What the user actually *sees* in each flow, step by step. Device flow costs the user a manual code entry; PKCE loopback is a single browser redirect. That difference is most of what "effortless" means here.

Flag explicitly if a provider turns out to have **no** secret-free flow — under the standing constraint that means that provider gets no OAuth, and the map needs to know.
