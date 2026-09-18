# HTTPS credential landscape across git providers

Type: research
Status: resolved

## Question

Does HTTP Basic authentication still make sense as a Cerebrite auth mechanism in 2026, and if so, in what form?

`git2`'s `Cred::userpass_plaintext` is the cheapest possible mechanism to implement — no key management, no browser round-trip, no provider API. The question is whether real providers still accept it, and with what.

Establish, against primary sources (provider docs, not blog posts):

- Which of **GitHub, GitLab (gitlab.com), Bitbucket Cloud, Gitea, Forgejo, Codeberg** still accept username + password over HTTPS for git operations, and which have removed it outright.
- Where password auth is gone, what replaced it: personal access tokens presented in the Basic-auth password field, and whether the username field matters (`x-access-token`, `oauth2`, the account name, anything).
- **Token variants and their cost to the user**: classic vs. fine-grained tokens, what scopes are the minimum for `git push`/`git fetch` on a private repo, whether expiry is mandatory or optional, and what the shortest mandatory expiry is. A token the user must manually regenerate every 90 days is a very different product than one that lasts forever.
- Whether any of these tokens can be **refreshed programmatically** without a browser, or whether expiry always means a manual visit to a settings page.
- Whether HTTPS-with-token works unchanged on **Android** (it should — it is plain HTTP — but confirm nothing in `git2`/libssh2's HTTPS transport is desktop-gated, given the project has already been bitten twice by desktop-only APIs).

The output that matters is a recommendation: is "paste a token" a mechanism worth shipping as the generic path for non-GitHub/GitLab providers, or is it strictly worse than SSH for the same users?

Note the standing constraint: whatever this finds must be implementable with no server-side component.

## Answer

**Yes — ship "paste a token over HTTP Basic auth" as the generic path for non-GitHub/GitLab providers (Bitbucket Cloud, Gitea, Forgejo, Codeberg, bare HTTPS hosts), and treat it as primary, not fallback, for that tier.**

- Real username+password is dead (GitHub, removed 2021-08-13; GitLab, blocked once 2FA is on; Bitbucket, App Passwords dying by 2026-06-09) or admin/user-configurable-and-shrinking (Gitea/Forgejo/Codeberg). The universal replacement is Basic auth with a **personal access token in the password field** — `git2::Cred::userpass_plaintext(username, token)` needs no new code path, and the username value is largely unchecked (any non-empty string works on GitHub/GitLab/Bitbucket; use the real account username on Gitea/Forgejo/Codeberg to be safe).
- Token expiry is **mandatory and non-negotiable** on GitLab.com (365-day cap since GitLab 16.0) and Bitbucket Cloud (1-year cap); **optional** ("no expiration" selectable) on GitHub, Gitea, Forgejo, Codeberg. No provider supports browser-less PAT renewal — expiry always means a manual settings-page visit. Cerebrite must build a "your sync token expired, regenerate at &lt;url&gt;" re-prompt into the credential-failure path, unavoidable for GitLab/Bitbucket specifically.
- HTTPS+token works unchanged on Android — it rides libcurl/vendored-OpenSSL, not libssh2, so it has none of SSH-agent's desktop-only baggage. This makes it the one mechanism confirmed safe across the Android-possible constraint with no extra work.
- SSH and HTTPS+token are complementary, not competing: bare SSH hosts have no token concept at all (SSH-only by definition), while HTTPS+token is the only Android-safe generic path for hosted providers.

Full findings, per-provider tables, and sources: [research report](01-https-credential-landscape-research.md).
