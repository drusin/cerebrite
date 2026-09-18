# HTTPS credential landscape across git providers

Type: research
Status: open

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
