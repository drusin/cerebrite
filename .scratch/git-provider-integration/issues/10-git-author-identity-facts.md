# What providers expose about the signed-in user, and what a wrong author email costs

Type: research
Status: resolved

## Question

Purely factual groundwork for the git-author-identity question sitting in the map's fog. This ticket decides nothing about what Cerebrite *does*; it establishes what is even possible, so the product half can be settled quickly once the mechanism decision (ticket 06) lands.

`vault.rs`'s `commit_all` falls back to `Cerebrite <cerebrite@local>` whenever `repo.signature()` fails — i.e. on any machine with no global git config, which is precisely the non-git-native user this effort targets.

Establish, against primary sources:

- **What identity a device-flow token can read, per provider.** For a GitHub App user-to-server token and a GitLab OAuth token acquired via the device authorization grant: which endpoint returns the user's name and email, and which *additional scope* that costs on top of what git push already needs (`Contents: Read and write` for GitHub, `api` for GitLab). If reading an email requires widening the consent screen, that is a real cost and the map needs the number.
- **The private/noreply email problem.** GitHub users can hide their real address behind `<id>+<username>@users.noreply.github.com`, and an account may have "Block command line pushes that expose my email" enabled — which *rejects the push outright* if the author email is the real one. Establish what the API returns for such a user, what the correct noreply address is, and what the push actually fails with. Same question for GitLab's commit-email setting.
- **What a mismatched author email actually costs.** On GitHub and GitLab: does the commit still push? Does it show as authored by the user, or as an unlinked ghost? Is it retroactively fixed if the address is added to the account later, or baked into history? Distinguish cosmetic consequences from ones that break contribution attribution permanently.
- **What the SSH and pasted-token paths can know.** Under those mechanisms there is no identity API call at all. Establish whether anything can be inferred (e.g. from the SSH key's provider-side metadata) or whether the honest answer is "ask the user", which would mean identity capture cannot be a property of the OAuth path alone.
- **What comparable tools do.** How GitHub Desktop, VS Code's git integration, and Obsidian Git handle a machine with no `user.email` — prompt, derive from the API, or commit with a fallback and let it break. Precedent is cheap evidence here.

Explicitly out of scope for this ticket: whether this map fixes the identity gap at all, and whether an OAuth login should populate it automatically. Those wait on ticket 06 and stay in the fog.

## Answer

- **Reading identity costs almost nothing.** GitLab: zero extra scope — `api` is already required for push, and `GET /user` returns `commit_email`, literally the address git should use. GitHub: `GET /user` requires **no permission at all** on a GitHub App user token, returning `id` + `login`. Only the user's *real* address costs an extra consent-screen line (`Email addresses: Read-only`).
- **The noreply address is derivable for free and is the safer choice.** `<id>+<login>@users.noreply.github.com` for any account created after 2017-07-18 (the username-only form is silently wrong for modern accounts). It counts as a contribution, survives username changes, and is the only variant that cannot trip GitHub's push block. GitHub Desktop's own `getStealthEmailForUser()` does exactly this.
- **One documented hard push failure exists, and it punishes using the *real* email.** With "Block command line pushes that expose my email" on, GitHub checks the most recent commit and rejects with `GH007: Your push would publish a private email address`. GitLab has no equivalent — [gitlab-org/gitlab#24579](https://gitlab.com/gitlab-org/gitlab/-/issues/24579) is still an open feature request — only opt-in Premium push rules.
- **A mismatched email never blocks the push; it costs attribution.** Unlinked plain-text author, gray Octocat, and **no contribution-graph credit** on GitHub (email-gated, default branch only, forks excluded). GitLab is far gentler: authorship is a verified-email lookup resolved at *display* time, and its calendar is push-event-based, not email-based.
- **`cerebrite@local` is the worst case specifically because it is unfixable.** GitHub's retroactive escape hatch — *"Your contributions graph will be rebuilt automatically when you add the new address"* — requires a verifiable mailbox. A wrong-but-real address is recoverable; an invented one is permanent short of a history rewrite GitHub warns does not reliably retract anything.
- **libgit2 does not guess.** `repo.signature()` → `git_signature_default` returns `NotFound` if `user.name`/`user.email` are unset, unlike the `git` CLI which falls back to `$USER@$HOSTNAME`. So the fallback fires deterministically on the non-git-native machine, not occasionally.
- **Identity capture is a property of provider-API access, not of OAuth.** A pasted token gets the same data. The genuinely dark tier is generic SSH / arbitrary hosts: the SSH greeting yields a username on exactly two hosts, is never emitted during a real `git2` fetch/push, and can never yield an email. For that tier "ask the user" is the only honest answer — so **ticket 08's connect journey needs a user-supplied name/email base case**, not an identity step living only on the OAuth branch.
- **Precedent is unanimous against a silent placeholder.** GitHub Desktop derives from the account and shows an editable "Configure Git" step; Obsidian Git prompts (and on mobile must, since isomorphic-git has no global config and throws `MissingNameError`); VS Code deliberately hard-fails via `-c user.useConfigOnly=true` and closed the "guess for me" request as-designed. `gh`/`glab` set credentials only. Nobody ships an invented address.

Full detail, citations and gaps: [research report](10-git-author-identity-facts-research.md).
