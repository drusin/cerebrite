# Secret-free OAuth flows for GitHub and GitLab

Type: research
Status: resolved

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

## Answer

**Both GitHub and GitLab have genuine secret-free (public-client) OAuth paths — neither is disqualified.**

- **GitHub**: Device Authorization Grant (must be manually enabled per-app) and PKCE (Authorization Code + PKCE, added 2025-07-14, de-facto rather than de-jure secret-free — GitHub has no formal "public client" flag). Register as a **GitHub App** (not OAuth App): scopable to "Contents: Read and write" instead of the OAuth App's blanket `repo`, and its mandatory short-lived-token + secret-free refresh model is a feature for unattended background sync. GitHub's fixed-port, pre-registered-redirect-URI requirement (no RFC 8252 any-port support) makes PKCE-loopback more fragile than device flow.
- **GitLab**: explicit "Confidential" vs. non-confidential ("public") application flag makes secret-free PKCE and device-code grant (since GitLab 17.1) first-class and well-documented. **Gotcha**: OAuth tokens need the broad `api` scope to `git push` — `write_repository` alone doesn't work for OAuth tokens specifically (a known, still-open GitLab limitation), unlike PATs where `write_repository` suffices.
- **Device Authorization Grant is the constraint-safest choice for both providers**: no redirect-URI/port/scheme plumbing at all, survives the Android-possible constraint trivially, familiar UX precedent (`gh`/`glab auth login`). Costs one manual code-entry step. PKCE (loopback or custom-scheme via Tauri's deep-link plugin) is more "effortless" on desktop but carries more moving parts and Android friction.
- **Key architectural finding for ticket 06**: the resulting OAuth token is directly usable as the git HTTPS Basic-auth credential for both providers (once correctly scoped) — OAuth and HTTPS-token auth are the same underlying mechanism with two ways of acquiring the credential, not a third parallel mechanism.

Full findings, redirect-mechanics table, and sources (note: `web_fetch` to public docs was blocked in the research sandbox this session — claims sourced via AI web-search with cited URLs, recommend a live-doc eyeball pass before implementation): [research report](02-secretless-oauth-flows-research.md).
