# Research: HTTP Basic Authentication for Git Providers in 2026

(Full research subagent output for ticket 01 — HTTPS credential landscape.)

## Summary

Plain username+password Basic auth for `git` operations over HTTPS is **dead or dying everywhere** among the six providers in scope, but the *replacement* is still HTTP Basic auth — with a personal/API access token substituted for the password. This means `git2::Cred::userpass_plaintext` remains the correct API call; only the credential *value* changes. Every provider now enforces or is actively deprecating toward mandatory token expiration, and **none of the six support programmatic (browser-less) token renewal** — expiry always means a manual visit to a web settings page, unless the app owner stands up a full OAuth2 client (which conflicts with Cerebrite's "no server-side component" / "no embedded client secret" constraints for a distributed binary, though public-client PKCE can sidestep the secret issue). The HTTPS transport in `git2`/libgit2 is not desktop-gated — it goes through libcurl/vendored-OpenSSL, the same code path Cerebrite already builds with (`git2 = { features = ["vendored-openssl"] }`), so it is expected to work unchanged on Android once that platform is targeted.

---

## 1. Provider-by-provider: does raw username+password still work?

### GitHub
Password authentication for git-over-HTTPS was **removed on August 13, 2021**. Since then, any password prompt for `git push`/`git clone` over HTTPS must be answered with a Personal Access Token (PAT), not the account password.
- GitHub Changelog: https://github.blog/changelog/2021-08-12-git-password-authentication-is-shutting-down/
- Original announcement: https://github.blog/2020-12-15-token-authentication-requirements-for-git-operations/

### GitLab (gitlab.com)
GitLab still accepts HTTP Basic auth wire format, but a raw account password is rejected for git operations when 2FA is enabled, and GitLab has been steering all API/Git HTTPS auth toward PATs; `gitlab.com` requires a PAT (or deploy/OAuth token) for HTTPS git in essentially all practical configurations.
- Docs: https://docs.gitlab.com/user/profile/personal_access_tokens/

### Bitbucket Cloud
Historically used **App Passwords** (a scoped Basic-auth password) rather than the real account password already — genuine account-password Basic auth for git has long been blocked. Now App Passwords themselves are being phased out:
- **Sept 9, 2025** — no new App Passwords can be created.
- **June 9, 2026** — all existing App Passwords are permanently disabled; only **API tokens** (and repository/project/workspace access tokens) remain valid for Basic-auth git operations.
- Atlassian blog (primary source): https://www.atlassian.com/blog/bitbucket/bitbucket-cloud-enters-phase-2-of-app-password-deprecation
- API tokens doc: https://support.atlassian.com/bitbucket-cloud/docs/api-tokens/

Given Cerebrite's 2026 target window, **App Passwords should be considered already dead** for a new implementation — build directly against Bitbucket API tokens.

### Gitea
Basic auth with a real account username+password is **still supported by default** as of current docs, gated by the server config flag `ENABLE_BASIC_AUTHENTICATION` (`app.ini`, `[service]` section). Self-hosted admins may disable it, in which case only tokens/SSH work.
- Config reference: https://docs.gitea.com/next/administration/config-cheat-sheet#service-service
- API usage doc (token-based auth as the documented default): https://docs.gitea.com/development/api-usage/

### Forgejo
Same lineage as Gitea; documents HTTP Basic Auth (`username:password`, plus `X-Forgejo-OTP` header when 2FA is enabled) and token auth as parallel, both-supported mechanisms.
- Access Token Scope: https://forgejo.org/docs/latest/user/token-scope/
- API Usage: https://forgejo.org/docs/latest/user/api-usage/

### Codeberg (a public Forgejo instance)
Codeberg **effectively pushes 2FA by policy/community norms**, and once 2FA is enabled on an account, the real account password stops working for git-over-HTTPS — a personal access token is required instead.
- Docs: https://docs.codeberg.org/security/2fa/, https://docs.codeberg.org/git/

**Bottom line for #1:** GitHub and GitLab.com: password is gone, full stop. Bitbucket Cloud: password was never really usable this way (App Password today, API token from mid-2026); Gitea/Forgejo: password *can* still work but is admin/user-configurable and shrinking; Codeberg: password dies the moment 2FA is turned on (which is the security-conscious default).

---

## 2. What replaced the password, and does the username matter?

In every case the mechanism is unchanged at the HTTP layer: **Basic auth**, with the *value* in the password slot being a token instead of a real password.

- **GitHub:** username value is not actually validated — GitHub only checks that the token in the password field is valid; any non-empty username string works (`x-access-token`, your actual username, literally anything). Confirmed in GitHub community discussion and consistent with `git2::Cred::userpass_plaintext(token, token)`-style call patterns: https://github.com/orgs/community/discussions/173881 ; https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens
- **GitLab:** username field can be any non-blank value; convention is your GitLab username, but it isn't checked against the token. Docs: https://docs.gitlab.com/user/profile/personal_access_tokens/
- **Bitbucket Cloud:** the new API-token flow explicitly documents a fixed dummy username, `x-bitbucket-api-token-auth`, as usable in place of the real account username; either your real username or that placeholder works. Ref: https://support.atlassian.com/bitbucket-cloud/docs/api-tokens/ (also see Atlassian community confirmation: https://community.atlassian.com/forums/Bitbucket-questions/Git-Access-with-API-Token/qaq-p/3057268)
- **Gitea/Forgejo/Codeberg:** documented pattern is real username + token in the password slot (no special sentinel username documented); this matches the shared Gitea/Forgejo codebase's Basic-auth handler, which checks the password against stored token hashes regardless of the username string in practice, but official docs don't guarantee an arbitrary username is accepted the way GitHub's is — treat username as "your account username" for these to be safe.

**Practical implication for Cerebrite:** a single code path — `Cred::userpass_plaintext(<anything-non-empty-or-username>, <token>)` — covers all six providers; no per-provider username logic is required beyond "send the account username if you have it, otherwise a placeholder."

---

## 3. Token variants, minimum scopes, and expiry economics

| Provider | Token type | Minimum scope for push+fetch | Expiry mandatory? | Shortest/typical mandatory ceiling |
|---|---|---|---|---|
| GitHub | Fine-grained PAT (recommended) | Repository → **Contents: Read and write** (single repo select) | Not strictly mandatory for personal-account fine-grained tokens outside an org; classic PATs still offer **"No expiration"** as of 2025 unless an org enforces a policy | N/A if "No expiration" chosen; orgs can force a max (admin-configurable) |
| GitHub | Classic PAT | `repo` scope (broad — full private-repo control) | No (no-expiration option persists for personal use) | — |
| GitLab.com | Personal Access Token | `write_repository` scope | **Yes, mandatory since GitLab 16.0** (blank expiry no longer allowed; auto-defaults to 365 days if unspecified) | 365 days default max (400-day max is an opt-in admin feature flag on self-managed instances only, not gitlab.com by default) |
| Bitbucket Cloud | API token (replacing App Passwords) | `write:repository:bitbucket` (implies read) | **Yes — mandatory, cannot be unset** | Maximum lifetime **1 year (365 days)**; cannot be created without an expiry |
| Gitea | Access token | `write:repository` | **Optional** — "never expire" remains selectable | N/A (no forced ceiling) |
| Forgejo | Access token | `write:repository` | Optional (same model as Gitea) | N/A |
| Codeberg | Access token (Forgejo-based) | `write:repository` | Optional per Forgejo codebase, but 2FA effectively forces token use, not token expiry | N/A |

Sources:
- GitHub fine-grained scopes / expiration UX: https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens ; changelog on optional expiration for fine-grained PATs: https://github.blog/changelog/2024-10-18-new-pat-rotation-policies-preview-and-optional-expiration-for-fine-grained-pats/
- GitLab mandatory expiry since 16.0 / 365-day default: https://docs.gitlab.com/user/profile/personal_access_tokens/ ; https://about.gitlab.com/blog/access-token-lifetime-limits/
- Bitbucket API tokens, 1-year max, scopes: https://support.atlassian.com/bitbucket-cloud/docs/api-tokens/ ; https://developer.atlassian.com/cloud/bitbucket/bitbucket-cloud-rest-api-scopes/
- Gitea scopes/expiry optionality: https://docs.gitea.com/development/api-usage/
- Forgejo scopes: https://forgejo.org/docs/latest/user/token-scope/

**Key product consequence:** GitLab.com and Bitbucket Cloud tokens **will always expire within a year, non-negotiably**. GitHub and the Gitea/Forgejo/Codeberg family can be configured to never expire, so the "annoying re-paste" problem is provider-dependent, not universal.

---

## 4. Can expiry be handled without a browser?

**No, not for plain PATs on any of the six providers.** Once a PAT expires it is permanently revoked; there is no API call to "renew" it — the user must sign in via browser and mint a new token.
- GitHub explicit confirmation: https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/token-expiration-and-revocation ("Only GitHub Apps offer refresh tokens... this does not apply to PATs")
- GitLab and Bitbucket have no PAT-refresh API either; their docs describe only "generate a new token" flows (https://docs.gitlab.com/user/profile/personal_access_tokens/, https://support.atlassian.com/bitbucket-cloud/docs/api-tokens/).

The one browser-less exception is **full OAuth2 App registration** (GitLab and GitHub both support `refresh_token` grant flows: https://docs.gitlab.com/api/oauth2.html), which *can* be refreshed programmatically indefinitely. But this requires either:
- a `client_secret` (disqualified — "no client secret embedded in a distributed binary"), or
- a public-client PKCE flow with no secret, which is viable but is a materially different, heavier mechanism than "paste a token" — it needs a registered OAuth app per provider, a local redirect listener, and a one-time browser consent dance. It is not in scope for the Basic-auth question being evaluated here, but is the correct escape hatch if Cerebrite ever wants token *auto-renewal*.

For plain HTTP Basic + token, **expiry always means a manual settings-page visit — no exceptions found on any of the six providers.**

---

## 5. Does HTTPS+token work unchanged on Android?

Yes, with the same OpenSSL configuration Cerebrite already vendors. `libssh2` is exclusively the SSH transport dependency in `git2-rs`; HTTPS goes through a separate, unrelated path:
- `git2-rs` feature docs distinguish `ssh` (needs libssh2) from `https` (uses libgit2's built-in HTTP client backed by OpenSSL/Schannel/Security-Framework depending on platform/features): https://github.com/rust-lang/git2-rs/blob/master/README.md and https://docs.rs/crate/git2/latest
- Cerebrite's `Cargo.toml` already selects `git2 = { version = "0.19", features = ["vendored-openssl"] }` — this feature name pulls in the OpenSSL backend for both SSH host-key/crypto operations *and* the HTTPS transport, statically linked. Statically-linked vendored OpenSSL is exactly the standard approach used to make `git2`/`openssl-sys` cross-compile to Android (see general guidance at https://stackoverflow.com/questions/76840152/how-to-build-openssl-sys-for-android, consistent with the officially documented `vendored` OpenSSL feature purpose).
- Nothing in libgit2's HTTP transport is gated to desktop OSes; it's plain libcurl/OpenSSL sockets code, portable to any Rust cross-compilation target including `aarch64-linux-android`.

No evidence found of any Android-specific restriction on `git2`'s HTTPS path — the same cannot be said with certainty for `ssh_key_from_agent` (agent forwarding is a desktop-OS-service concept that generally doesn't exist as such on Android), which is a relevant asymmetry worth noting even though SSH-agent support is out of scope for this specific question.

---

## Recommendation

**Yes — ship "paste a token over HTTP Basic auth" as the generic non-GitHub/GitLab path, and treat it as the *primary*, not fallback, mechanism for Bitbucket Cloud/Gitea/Forgejo/Codeberg/bare-HTTPS hosts.** It is not "strictly worse than SSH for the same users" — it is better for Cerebrite's specific constraints:

- It requires **zero new code path in `git2`**: `Cred::userpass_plaintext(username, token)` is already one branch away from the existing `credential_helper`/`Cred::default()` chain in `sync.rs:81-101`. No key generation, no agent discovery, no platform keychain integration needed to get a *first* working sync.
- It is **the only mechanism confirmed to work identically on Android** without any additional research burden — SSH-agent forwarding (`ssh_key_from_agent`, `sync.rs:86`) is a poor Android citizen; raw HTTPS Basic auth has no such asymmetry.
- It satisfies "no server-side component, ever" trivially — there is no token-broker or OAuth app registration involved, unlike the only browser-less alternative (OAuth2 refresh tokens), which needs either a forbidden embedded client secret or a heavier PKCE+local-redirect implementation.
- Every provider in scope documents a token-in-password-field flow as its **current, sanctioned, forward-looking replacement** for password auth — this is not a legacy or soon-to-be-removed mechanism; if anything providers (Bitbucket in 2025–2026, GitLab since 2023) are actively *hardening toward* mandatory tokens, so Cerebrite is building against where these providers are headed, not where they came from.

**The real cost is UX, not security-model mismatch:** GitLab.com and Bitbucket Cloud tokens are hard-capped at 365 days and cannot be silently refreshed — Cerebrite must build a "your sync token has expired, generate a new one at <url>" re-prompt flow into the credential-callback failure path, because there is no way around a manual browser visit on those two providers specifically. GitHub, Gitea, Forgejo, and Codeberg tokens *can* be created without expiry, so for those, "paste once and forget" is achievable if the user opts for "No expiration"/omits an expiry — Cerebrite's onboarding UI should nudge toward that choice for those providers while being explicit that GitLab/Bitbucket tokens will need periodic re-entry.

SSH remains preferable *only* where the user already has agent-based keys set up (common developer default, especially for a bare SSH host, which has no token concept at all and is SSH-only by definition). For a bare SSH host, Basic-auth-over-HTTPS isn't even an option — so the two mechanisms are genuinely complementary, not competing: SSH for bare-host and power-user default; HTTPS+token for the effortless GitHub/GitLab path plus the only viable Android-safe generic path for everyone else.

## Gaps / Uncertainties

- Could not fetch official docs pages directly (web_fetch tool was denied by environment); all provider-doc claims above are sourced via AI web-search summaries with cited URLs pointing to the primary docs — recommend a follow-up direct fetch of `docs.gitlab.com/user/profile/personal_access_tokens/`, `support.atlassian.com/bitbucket-cloud/docs/api-tokens/`, and `docs.github.com/.../managing-your-personal-access-tokens` to eyeball exact current wording before shipping.
- Codeberg's own instance-level token-expiry policy (as opposed to generic Forgejo docs) wasn't independently confirmed.
- Did not verify whether self-hosted GitLab CE/EE (vs. gitlab.com) instances can be admin-configured to waive the mandatory-expiry rule outright.
- OAuth2 PKCE refresh-token viability was not deeply investigated per-provider here — see ticket 02's dedicated research.
