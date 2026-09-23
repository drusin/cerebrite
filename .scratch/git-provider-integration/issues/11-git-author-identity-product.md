# Git author identity in the connect journey

Type: grilling
Status: resolved
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

## Answer

Recorded as the **Author** entry in [CONTEXT.md](../../../CONTEXT.md). No ADR: the choice is cheap to revisit before release.

### The rule

**Cerebrite never commits under an author the user has not seen and confirmed.**
- The `Cerebrite <cerebrite@local>` fallback is deleted from both of its call sites: `commit_all` in `vault.rs`, and the merge commit in `sync.rs`.
- The map fixes the rule. How it is enforced is left to implementation.

### The author belongs to the vault, not the connection

- A vault with no connection commits too (ADR-0006), so it needs an author as well.
- The "Commit as" step is therefore part of **vault setup** (pick, create or clone), before the first commit. It is not a branch inside the connect journey.
- Scenario: a vault used locally for a week and then connected on day 8. Its early commits carry the author confirmed on day 1.

### Storage

- The author lives in **repo-local git config**: `user.name` and `user.email` in the repository's `.git/config`. Committer and author are always set to the same value.
- This way Cerebrite and `git commit` run in the same folder can never disagree.
- It is not synced. A second device confirms its author again at clone time, with values suggested.
- This supersedes the "author identity" field that [Where credentials live at rest](07-credential-storage-decision.md) put in `.git/cerebrite/connection.json`. That ticket and ADR-0013 are amended to match.

### What is suggested

The step is always shown and always editable. Its fields are filled from the first source that has a value:

1. **Repo-local git config** already in the picked repository (someone put it there on purpose).
2. **Global git config** (`~/.gitconfig`).
3. **The provider**, OAuth sign-in only:
   - **GitHub:** the noreply address `<id>+<login>@users.noreply.github.com`, built from `GET /user`, which needs no permission.
   - **GitLab:** `commit_email`.
   - The name comes from the profile in both cases.
   - The GitHub App **never** requests the email permission, and the real address is never suggested. It costs an extra consent line and is the one variant that can trip `GH007`.
4. **Empty.** This is the base case for access-token and SSH-key connections that have no git config to draw on.

Confirming writes the value repo-locally. A later change to the global config therefore never silently changes a vault's author.

### Connecting a vault that already has an author

- If the provider's suggested address differs from the confirmed author, the connect step offers the switch **once**, for example "Commit as `…@users.noreply.github.com` from now on? Keeps your email private."
- The default is to keep the current author. It never switches silently.

### Changing it later

- The vault's Settings has an editable **"Commit as"** name and email.
- It affects future commits only, with a note saying so. History is never rewritten.
- A wrong-but-real address is recoverable on GitHub's side (add it to the account and the graph rebuilds), so a rewrite is not worth its risk.

### Validation

- Minimal checks: a non-empty name, and an email with an `@` and a domain that contains no `<`, `>` or line breaks.
- The mailbox is never verified.
- A domain that cannot be real (`.local`, `localhost`, no dot) gets a **warning, not a block**: "commits with this address can't be linked to any account". This is the permanent-loss case ticket 10 found.

### GitLab contingency

- The live test pending under ADR-0012 may show that `write_repository` is enough to push. In that case, Cerebrite also requests **`read_user`** rather than drop the suggestion. That is still far narrower than `api`.
- The same live test must confirm that `GET /user` returns `commit_email` under `read_user`.

### Handed on

- To [The connect and clone onboarding journey](08-connect-and-clone-journey.md):
  - the "Commit as" step, in **both** journeys (connect and clone), placed at vault setup and suggested as above;
  - the one-time "switch to the provider's address?" offer on the connect path;
  - the "Commit as" field in vault Settings.
