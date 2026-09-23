# Git author identity in the connect journey

Type: grilling
Status: open
Blocked by: 06, 10

## Question

What name and email does Cerebrite commit as, and where does the user set them?

[Ticket 10](10-git-author-identity-facts.md) settled the facts. Provider identity is nearly free to read with provider-API access: GitHub's `GET /user` gives a deterministic `<id>+<login>@users.noreply.github.com`, and GitLab's `api` scope returns `commit_email`. A wrong-but-real email can be recovered later, but `cerebrite@local` is permanent. [Ticket 06](06-auth-mechanisms-decision.md) settled that provider-API access exists **only** for the *OAuth sign-in* credential kind. *Access token* and *SSH key* connections can never auto-fill identity.

Note: if GitLab's `write_repository` turns out to be enough to push (a live test is pending under ticket 06), Cerebrite would no longer hold the `api` scope. `commit_email` would then need either `read_user` as well, or the user to supply it.

Settle:

- **Does this map fix the `Cerebrite <cerebrite@local>` fallback** in `vault.rs`'s `commit_all` at all, or is the fix left to implementation?
- **Where the name/email step sits** in the connect journey. It must always be present, since two of the three credential kinds cannot auto-fill it. For OAuth sign-in, it is prefilled with the provider's value.
- **Which GitHub email to prefill**: the noreply address (no extra consent) or the real address (an extra consent line, and the one variant that can hard-fail a push with `GH007`).
- **Precedence against an existing global git config** (`user.name`/`user.email`) on the machine: prefill from it, override it, or ignore it.
- **Where identity is stored**: per Connection, per vault, or in repo-local git config.
