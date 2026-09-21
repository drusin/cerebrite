# Research: git author identity — what providers expose, and what a wrong author email costs

Fact-finding for [ticket 10](10-git-author-identity-facts.md). This report decides nothing about what Cerebrite should do; it establishes what is possible and what the failure modes cost.

**Note on methodology/limitations:** Unlike the [ticket 02 session](02-secretless-oauth-flows-research.md), direct `WebFetch` against `docs.github.com`, `docs.gitlab.com` and `gitlab.com` issues **worked** in this environment, and the great majority of claims below are quoted from a direct fetch of the cited page. Three exceptions, flagged inline where they occur: `git-scm.com` and `libssh2.org` returned HTTP 403, `raw.githubusercontent.com/github/docs` returned 404 (so exact source-markdown diffing was not possible), and a handful of claims — the exact `GH007` push-error text, the `libssh2_session_banner_get` semantics, and GitLab's rendering of an unmatched author — are sourced via web search to issue trackers rather than to a vendor doc. Every such claim is marked. Claims marked **[inferred]** are reasoned from primary evidence rather than stated by a vendor.

Starting point in our own code — `src-tauri/src/vault.rs:88-90`:

```rust
let signature = repo
    .signature()
    .or_else(|_| git2::Signature::now("Cerebrite", "cerebrite@local"))
```

`Repository::signature` wraps libgit2's `git_signature_default`, whose contract is documented in the crate source: *"This looks up the user.name and user.email from the configuration and uses the current time as the timestamp… It will return `NotFound` if either the user.name or user.email are not set."* (`git2-0.19.0/src/repo.rs:1719-1724`). Note the divergence from the `git` CLI, which *guesses* an identity from `$USER@$HOSTNAME` and commits with a warning unless `user.useConfigOnly=true` ([git 2.8 `user.useConfigOnly`](https://github.com/git/git/commit/c37f9a1bc38cad56c9eca40014802e7cd822c21c) — *sourced via search; git-scm.com returned 403*). **libgit2 does not guess.** On a machine with no `user.name`/`user.email`, Cerebrite's fallback fires deterministically, not occasionally.

---

## 1. What identity a device-flow token can read, per provider

### GitHub (GitHub App user-to-server token)

GitHub Apps do not use OAuth scopes at all — *"If you're building a GitHub App, you don't provide scopes in your authorization request"* ([Scopes for OAuth apps](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps)) — they use fine-grained permissions, and **account permissions** are the user-granted class: *"When a user authorizes an app to act on their behalf, they will see and grant the account permissions that the app requested."* ([Choosing permissions for a GitHub App](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/choosing-permissions-for-a-github-app)). So an extra account permission **is** literally an extra line on the consent screen.

| What you want | Endpoint | Cost on top of `Contents: Read and write` |
|---|---|---|
| Login, numeric ID, display name, *public* profile email | `GET /user` | **Nothing.** `GET /user` does not appear under any permission in [Permissions required for GitHub Apps](https://docs.github.com/en/rest/authentication/permissions-required-for-github-apps); for fine-grained tokens the docs state the token *"does not require any permissions"* ([REST endpoints for users](https://docs.github.com/en/rest/users/users)). |
| The account's real/primary email addresses | `GET /user/emails`, `GET /user/public_emails` | **`Email addresses: Read-only` account permission**, an extra consent-screen line. Confirmed in the account-permissions section of [Permissions required for GitHub Apps](https://docs.github.com/en/rest/authentication/permissions-required-for-github-apps) (classic-PAT equivalent: `user:email` scope, per [REST endpoints for emails](https://docs.github.com/en/rest/users/emails)). |

The `email` field on `GET /user` is **not** the private primary address: *"the `email` key in the following response is the publicly visible email address"* and it is `null` if the user has not published one ([REST endpoints for users](https://docs.github.com/en/rest/users/users)). For a privacy-conscious user — exactly the one who has "Keep my email addresses private" on — it will be `null`.

**The key asymmetry:** `GET /user` returns `id` and `login` for free, and the correct noreply address is a pure function of those two (§2). So on GitHub, a *usable, attributable* author email costs **zero additional permissions**; only the user's *real* address costs a widened consent screen.

### GitLab (OAuth token via device grant)

| Scope | What it grants |
|---|---|
| `read_user` | *"Grants read-only access to the authenticated user's profile through the `/user` API endpoint."* ([Access token scopes](https://docs.gitlab.com/security/tokens/access_token_scopes/)) |
| `api` | *"Grants complete read and write access to the API for the token's scope."* (same page) — a superset of `read_user`. |

[Ticket 02](02-secretless-oauth-flows-research.md) established that a GitLab **OAuth** token needs the broad `api` scope to `git push` at all ([gitlab-org/gitlab#321359](https://gitlab.com/gitlab-org/gitlab/-/issues/321359)). Since `api` subsumes `read_user`, **reading identity on GitLab is free — zero additional scope, zero consent-screen widening.**

And GitLab returns exactly the right field. `GET /user` ("List current user") documents `name`, `username`, `id`, `email`, `public_email` **and `commit_email` — "User's commit email address"** ([Users API](https://docs.gitlab.com/api/users/)). No derivation needed: `commit_email` is literally the address git should use.

**Consequence for [ticket 06](06-auth-mechanisms-decision.md):** the "reading identity widens the consent screen" worry is largely a non-issue. On GitLab it costs nothing. On GitHub it costs nothing *if* you accept the noreply address, and one extra account permission only if you insist on the real one.

---

## 2. The private / noreply email problem

### GitHub: noreply address format

Two forms, split by a hard date ([Email addresses reference](https://docs.github.com/en/account-and-profile/reference/email-addresses-reference)):

- Account created **after July 18, 2017**: *"your `noreply` email address is an ID number and your username in the form of `ID+USERNAME@users.noreply.github.com`"*
- Account created **before July 18, 2017**, with privacy enabled before that date: *"your `noreply` email address is `USERNAME@users.noreply.github.com`"*

The same page warns why the ID-prefixed form is the one to use: *"If you use your `noreply` email address for GitHub to make commits and then change your username, those commits will not be associated with your account. This does not apply if you're using the ID-based `noreply` address from GitHub."*

**Do not derive the username-only form.** It is wrong for every account created after mid-2017, and wrong *silently* — the commit lands with an author GitHub will not link. Since `GET /user` hands you both `id` and `login` at zero permission cost, `<id>+<login>@users.noreply.github.com` is always constructible and always the right form for a modern account.

### GitHub: what the API returns for such a user

`GET /user.email` is `null` (no public email set). `GET /user/emails` — if the app has the `Email addresses` permission — returns entries of `{email, primary, verified, visibility}` ([REST endpoints for emails](https://docs.github.com/en/rest/users/emails)); the docs give no worked example and **do not state whether the noreply address appears as an entry** (*gap: not established by primary docs; a search for a definitive answer returned only secondary sources that disagree*). Treat the noreply address as **derived, not fetched**.

### GitHub: "Block command line pushes that expose my email" — this rejects the push

*"Each time you push to GitHub, we'll check the most recent commit. If the author email on that commit is a private email on your GitHub account, we will block the push and warn you about exposing your private email."* ([Blocking command line pushes that expose your personal email address](https://docs.github.com/en/account-and-profile/how-tos/email-preferences/blocking-command-line-pushes-that-expose-your-personal-email-address))

The doc does not quote the error string. In practice it is (*sourced via search to [atom/github#1910](https://github.com/atom/github/issues/1910) and [this gist](https://gist.github.com/gengwg/ef9a073a6fd845786d5993914439c946), not to a GitHub doc*):

```
remote: error: GH007: Your push would publish a private email address.
remote: You can make your email public or disable this protection by visiting:
remote: http://github.com/settings/emails
```

Two things matter here:

1. This is the **one documented case where an author email hard-fails a push on GitHub.** It fires precisely when the app helpfully fetched the user's real primary address and used it — i.e. the "better" behaviour (paying for `Email addresses`, using the real email) is the one that can break. Using the derived noreply address cannot trigger GH007, because a noreply address is not a private email on the account.
2. It checks only **the most recent commit** in the push — so a background-sync app that pushes several commits at once gets a non-deterministic-feeling failure, and [ticket 09](09-sync-and-auth-failure-surfacing.md) will need to surface `GH007` intelligibly rather than as a generic push failure. Atom shipped exactly that bug: the linked issue is about it reporting *"The tip of your current branch is behind its remote counterpart."*

### GitLab: private commit email

GitLab's equivalent is a profile setting: *"In the **Commit email** dropdown list, select **Use a private email**."* after which *"Every Git-related action uses the private commit email"*, and the docs explicitly tell the user they *"can also copy the private email and configure it on your local machine by using… `git config --global user.email <your email address>`"* ([Profile settings](https://docs.gitlab.com/user/profile/)).

Format: `<ID>-<USERNAME>@users.noreply.gitlab.com` — note the **hyphen**, not GitHub's `+` (*sourced via search; the format is documented in GitLab's admin email-settings doc as `users.noreply.<hostname>`, but I could not fetch a page stating the full user-facing form directly — [GitLab admin email settings](https://docs.gitlab.com/administration/settings/email/), [gitlab-org/gitlab-foss#43521](https://gitlab.com/gitlab-org/gitlab-foss/-/issues/43521)*). This is a good reason to **read `commit_email` from `GET /user` rather than derive it** on GitLab — the API gives you the authoritative value, including when the user has *not* enabled privacy.

Crucially, GitLab makes the field do the work regardless of the privacy setting: *"Any of your own verified email addresses can be used as the commit email. Your primary email is used by default."* ([Profile settings](https://docs.gitlab.com/user/profile/)) — so `commit_email` is always populated with something correct.

### GitLab does *not* block pushes that expose a private email

There is no GitLab analogue of GH007. [gitlab-org/gitlab#24579](https://gitlab.com/gitlab-org/gitlab/-/issues/24579), *"Block command line pushes if they have the user's private email address"*, is an **open feature request** proposing to add such a setting — which confirms the behaviour does not exist today.

What GitLab *does* have is opt-in, paid, per-project **push rules** that can reject on author email, all off by default ([Push rules](https://docs.gitlab.com/user/project/repository/push_rules/)): *"Check whether the commit author is a GitLab user"* (*"Both the commit author and committer email addresses must match a GitLab user's verified email addresses"*), *"Reject unverified users"*, and a *"Commit author's email"* regex where *"To allow any email address, leave empty."* These are Premium/Ultimate and project-scoped, so irrelevant to a personal vault but a real hazard if a user connects a work repo.

---

## 3. What a mismatched author email actually costs

### Does the push succeed?

**Yes on both platforms**, absent the two specific exceptions above (GitHub's GH007, which only fires on the user's *own private* address; GitLab's opt-in push rules). **[inferred]** — neither vendor states "the push succeeds"; it follows from GitHub documenting unlinked commits purely as a post-push *display* state, and from GitLab's author-email enforcement being documented as opt-in rules that are off by default.

So `Cerebrite <cerebrite@local>` **pushes fine**. The damage is entirely downstream of the push.

### How it displays on GitHub

*"GitHub links a commit to a user by matching the email address in the commit header to an email address on a GitHub account."* and *"If your commits are not linked to any user, the commit author's name will not be rendered as a link to a user profile."* ([Why are my commits linked to the wrong user?](https://docs.github.com/en/pull-requests/committing-changes-to-your-project/troubleshooting-commits/why-are-my-commits-linked-to-the-wrong-user))

Avatar: *"If the email address has a Gravatar associated with it, the Gravatar will be displayed next to the commit, rather than the default gray Octocat."* (same page, [Enterprise Server 3.9 copy](https://docs.github.com/en/enterprise-server@3.9/pull-requests/committing-changes-to-your-project/troubleshooting-commits/why-are-my-commits-linked-to-the-wrong-user) — the dotcom fetch did not return the avatar sentence). So: plain-text name, gray Octocat, no profile link.

GitHub surfaces three diagnostic states on hover: *Unrecognized author (with email address)*, *Unrecognized author (no email address)*, and *Invalid email*. `cerebrite@local` lands in the first.

Worth knowing for the wrong-email-belongs-to-someone-else case: *"If your commits are linked to another user, that does not give them access to your repository."* (same page). Cosmetic misattribution, not a security hole.

### Contribution graph on GitHub — this is the permanent-feeling one

Documented conditions ([Troubleshooting missing contributions](https://docs.github.com/en/account-and-profile/how-tos/contribution-settings/troubleshooting-missing-contributions)):

1. *"Commits must be made with an email address that is connected to your account on GitHub, or the GitHub-provided `noreply` email address"*
2. *"Commits are only counted if they are made in the default branch or the `gh-pages` branch"*
3. *"Commits made in a fork will not count toward your contributions."*
4. Up to 24 h latency.

`cerebrite@local` fails condition 1 outright. Every commit is invisible on the profile.

### Is it retroactively fixable?

**GitHub contribution graph: yes, explicitly.** *"Your contributions graph will be rebuilt automatically when you add the new address."* — and symmetrically, *"If you remove an email address that was used to author older commits… those historical contributions will no longer appear on your contributions graph."* (same page). The graph is recomputed from *current* email ownership, not frozen at push time.

**But `cerebrite@local` is not a fixable address.** It is not a real mailbox, so GitHub's verification email cannot be delivered and the address can never be added to the account. The retroactive escape hatch exists but is closed for precisely our fallback value. A wrong-but-real address (`user@oldjob.com`) is recoverable; `cerebrite@local` is not. **That is the sharpest single fact in this report.**

**GitHub commit-display linking: ambiguous.** The docs say only *"Future commits that use the email address will be linked to your account"* and *"Old commits might not be linked after you update your email settings."* — with no condition given. **Not established by primary docs.**

Changing your commit email never touches history: *"Any commits you made prior to changing your commit email address are still associated with your previous email address."* ([Setting your commit email address](https://docs.github.com/en/account-and-profile/setting-up-and-managing-your-personal-account-on-github/managing-email-preferences/setting-your-commit-email-address))

### GitLab: association is a live lookup, so it *is* retroactive

- Matching is by email, and privacy does not exempt it: *"Making your email non-public does not prevent it from being used for commit matching."* ([Profile settings](https://docs.gitlab.com/user/profile/))
- Old addresses keep working by design: *"If your primary email changes, your original primary email is added as a secondary email. This feature allows commits made with your original primary email to remain associated with your account."* (same page)
- Attribution is re-evaluated on read. For signed commits: *"When a verified signed commit's committer email is no longer verified to the signing user, GitLab displays an orange verified badge with a warning sign"* and *"To restore the green **Verified** badge, add the committer email address to your GitLab profile and verify it."* ([Signed commits](https://docs.gitlab.com/user/project/repository/signed_commits/)) — degradation and restoration both happen at display time.
- Implementation confirmation (*GitLab's own repo, authoritative code but not documentation*): [gitlab-org/gitlab!21214](https://gitlab.com/gitlab-org/gitlab/-/merge_requests/21214) — *"we only assign a commit author or committer is a specific user if the e-mail has been confirmed"* — implemented in view helpers (`commit_author_link`), i.e. at render time.
- Rendering for an unmatched author (*sourced to issue trackers, not docs*): plain-text name, no profile link, Gravatar derived from the commit email — [gitlab-org/gitlab#353490](https://gitlab.com/gitlab-org/gitlab/-/issues/353490), [gitlab-org/gitlab-foss#54046](https://gitlab.com/gitlab-org/gitlab-foss/-/issues/54046).

**GitLab's contributions calendar is event-based, not email-based.** It *"displays a user's events from the past 12 months. This includes contributions made in forked and private repositories"*, and counts *"pushed"* events; the page **never mentions commit author email** ([Contributions calendar](https://docs.gitlab.com/user/profile/contributions_calendar/)). **[inferred]**: a mismatched author email most likely still credits the GitLab calendar, because the calendar records the *push* by the authenticated account. GitLab never states the negative.

**Net:** the mismatched-email penalty is materially harsher on GitHub (email-gated calendar, default-branch-only, forks excluded) than on GitLab (event-gated calendar, display-time author lookup).

### The only true fix for existing commits: rewrite history

The author email is part of the commit header and therefore of the commit hash. Neither platform can edit it in place. GitHub's own statement of the cost ([Changing a commit message](https://docs.github.com/en/pull-requests/committing-changes-to-your-project/creating-and-editing-commits/changing-a-commit-message)):

> *"Changing a commit message creates a new commit ID. If the commit has already been pushed, you must force push the rewritten history."*
> *"Force pushing can disrupt collaborators who have based work on the old commits."*
> *"If a commit message included sensitive information, force pushing an amended commit might not remove the original commit from GitHub."*

That last line is the sharp one: **a rewrite does not reliably retract an already-published email address** — the orphaned original stays reachable by SHA until GC, and GitHub's guidance is to contact Support.

For a personal vault with one collaborator (the user), a `git filter-repo --mailmap` + force-push is *tolerable* but is a destructive, out-of-app operation that Cerebrite's own auto-sync would fight. **[inferred]** — that last point is reasoning about our own `sync.rs` fetch→merge→push loop, not a sourced claim.

**Distinguishing cosmetic from permanent:**

| Consequence | Cosmetic / recoverable | Permanent without rewrite |
|---|---|---|
| Commit shows unlinked name + gray Octocat (GitHub) | partially — "might not be linked" is undocumented | likely |
| Missing from GitHub contribution graph | **recoverable** if the address is real and addable | **permanent for `cerebrite@local`** (unverifiable address) |
| Unlinked author on GitLab | **fully recoverable** — display-time lookup | no |
| GitLab contributions calendar | unaffected **[inferred]** | no |
| The bytes in the commit object | — | **always permanent** |

---

## 4. What the SSH and pasted-token paths can know

### SSH: a username, at best, on exactly two hosts

`ssh -T git@github.com` returns *"Hi USERNAME! You've successfully authenticated, but GitHub does not provide shell access."* ([Testing your SSH connection](https://docs.github.com/en/authentication/connecting-to-github-with-ssh/testing-your-ssh-connection)); `ssh -T git@gitlab.com` returns *"Welcome to GitLab, @username!"* ([GitLab SSH docs](https://docs.gitlab.com/user/ssh/)). So the **username is recoverable over pure SSH** on github.com and gitlab.com.

Three reasons this is not usable as a mechanism:

1. **It yields no email.** And the username alone is not enough to build a GitHub noreply address — the correct modern form needs the numeric `id` (§2), which requires a REST call (`GET /users/{login}`, unauthenticated, 60 req/h). Deriving `<username>@users.noreply.github.com` from the SSH greeting would be **silently wrong** for every post-2017 account.
2. **libgit2 never sees the greeting.** The text is stderr output of the server's forced command *when no git command is requested*; during a real fetch/push the client execs `git-upload-pack`/`git-receive-pack` and the forced command runs that service instead, so the string is never emitted on that code path. libssh2's `libssh2_session_banner_get()` returns the SSH protocol identification string (`SSH-2.0-…`), not this text (*sourced via search to the [Ubuntu man page](https://manpages.ubuntu.com/manpages/resolute/man3/libssh2_session_banner_get.3.html); libssh2.org returned 403*), and the `git2` crate's `RemoteCallbacks` surface exposes credentials / certificate_check / transfer_progress / sideband_progress / update_tips — no SSH stderr. *This section's "libgit2 does not surface it" is reasoned, not source-verified; docs.rs and libgit2's `ssh.c` were both unreachable.* Harvesting it would mean shelling out to a system `ssh` binary and scraping an undocumented, unstable message.
3. **It does not generalise.** Forgejo/Gitea emit their own unstandardised greeting; a self-hosted instance is only queryable if you already know its API base URL; and a **bare SSH host** (`git@server:/srv/repo.git`) has no identity concept at all — the SSH user is literally `git`, shared by everyone on the box.

### Pasted tokens: actually the *better*-informed path

| Token | `GET /user` | Emails |
|---|---|---|
| GitHub classic PAT | `read:user` (or `user`) for the *private* user response; a **zero-scope** token still authenticates and returns the public response with `login`, `id`, `name` ([REST users](https://docs.github.com/en/rest/users/users)) | `user:email` ([REST emails](https://docs.github.com/en/rest/users/emails)) |
| GitHub fine-grained PAT | *"does not require any permissions"* — returns `login`, `id`, `name`; `email` null unless public | `Email addresses` account permission ([Permissions for fine-grained PATs](https://docs.github.com/en/rest/authentication/permissions-required-for-fine-grained-personal-access-tokens)) |
| GitLab PAT | `read_user` minimum, `api` sufficient ([Access token scopes](https://docs.gitlab.com/security/tokens/access_token_scopes/)) | `commit_email` is on the `GET /user` response ([Users API](https://docs.gitlab.com/api/users/)) |

**A pasted token gives you everything OAuth gives you** — identity discovery is a property of *having a token for a known provider API*, not of the OAuth acquisition path. The asymmetry [ticket 02](02-secretless-oauth-flows-research.md) found (OAuth and HTTPS-token auth are the same mechanism, two acquisition routes) holds here too.

### Bottom line for [ticket 06](06-auth-mechanisms-decision.md)

Ranked by what is derivable:

| Path | Name | Email |
|---|---|---|
| GitLab OAuth (`api`) or PAT (`read_user`) | `name` | **`commit_email` — exact, no derivation** |
| GitHub App UAT / PAT + `Email addresses` | `name` | real primary email (but can trip GH007) |
| GitHub App UAT / PAT, no extra permission | `name`, `login` | derive `<id>+<login>@users.noreply.github.com` |
| SSH to github.com / gitlab.com | username only, via an unstable stderr scrape of a *separate* `ssh -T` invocation | none |
| SSH / token to Forgejo, Gitea, Bitbucket, bare host | none | none |

**Identity capture cannot be a property of the OAuth path alone** — but not for the reason the ticket anticipated. It is a property of *provider-API access*, which the pasted-token path also has. The genuinely dark path is the **generic SSH / arbitrary-host tier**, which is exactly the tier the map's second provider tier covers. For that tier, "ask the user" is the only honest answer. **Whatever [ticket 08](08-connect-and-clone-journey.md) designs, the connect journey needs a user-supplied name/email as the always-present base case, with auto-fill as a convenience layer on the two first-class providers — not an identity step that only exists on one branch of the flow.**

---

## 5. What comparable tools do

Three genuinely different postures — and none of them is "commit with a fallback and let it break".

| | Behaviour with no `user.email` | Where identity comes from | Writes git config? |
|---|---|---|---|
| **GitHub Desktop** | Dedicated onboarding "Configure Git" step | Signed-in account: `name \|\| login`, and `lookupPreferredEmail` (public primary → account stealth/noreply → generated stealth) | **Yes** — global, plus per-repo override |
| **VS Code Git** | **Hard fail**, deliberately: commits run as `git -c user.useConfigOnly=true commit …` | Nothing. User runs `git config` themselves | **No** — never, not even from the GitHub sign-in |
| **Obsidian Git** | Desktop: git's own fatal. Mobile: pre-flight check → *"Git author information is not set. Please set it in the settings"* | Plugin settings fields | **Yes** — into `.git/config` (repo-local; the only scope isomorphic-git supports) |

### GitHub Desktop — the closest precedent, and it does exactly the derivation §2 recommends

- Onboarding has a **"Configure Git"** step (`app/src/ui/welcome/configure-git.tsx` → [`app/src/ui/lib/configure-git-user.tsx`](https://github.com/desktop/desktop/blob/development/app/src/ui/lib/configure-git-user.tsx)), copy: *"Configure Git — This is used to identify the commits you create. Anyone will be able to see this information if you publish commits."* It offers **"Use my GitHub account name and email address"** vs **"Configure manually"**, shows a live example-commit preview, and on save writes `setGlobalConfigValue('user.name' / 'user.email')`.
- Defaults come from the account: `props.globalUserName || account?.name || account?.login` and `lookupPreferredEmail(account)`.
- [`app/src/lib/email.ts`](https://github.com/desktop/desktop/blob/development/app/src/lib/email.ts) contains `getStealthEmailForUser()`, which builds **`id+login@users.noreply.github.com`** (modern form, with the legacy `login@…` form still *recognised* but not generated; GHES uses `users.noreply.<endpoint host>`). `lookupPreferredEmail()` prefers a publicly visible primary, then the stealth address, then a generated one. `isAttributableEmailFor()` validates a candidate against verified emails *plus both stealth forms*.
  **This is independent confirmation of §2's conclusion**: the reference implementation, written by GitHub, derives the ID-prefixed noreply address rather than demanding the real email.
- Docs ([Configuring Git for GitHub Desktop](https://docs.github.com/en/desktop/configuring-and-customizing-github-desktop/configuring-git-for-github-desktop)): *"If your name and email address have already been set in the global Git configuration for your computer, GitHub Desktop will detect and use those values."* and *"If the email address that has been set in your Git configuration does not match an email address associated with the GitHub account you are currently logged in to, GitHub Desktop will show a warning prior to committing."*
- The warning's wording, quoted from a bug report ([desktop/desktop#15795](https://github.com/desktop/desktop/issues/15795) — *sourced to an issue, not a doc*): **"This commit will be misattributed."** *"The email in your global Git config (…) doesn't match your GitHub account. You can also choose an email local to this repository from the repository settings."*
- Notably, Desktop **does not pass `--author` or `GIT_AUTHOR_*`** — [`app/src/lib/git/commit.ts`](https://github.com/desktop/desktop/blob/development/app/src/lib/git/commit.ts) builds `['-F','-']` plus flags only. Identity comes purely from git config, which is why it has to *write* the config.
- Its known gap is instructive: because it doesn't set `user.useConfigOnly`, a user who clears their config after onboarding gets git's guessed `user@machine.local` silently ([desktop/desktop#1364](https://github.com/desktop/desktop/issues/1364), still open).

### VS Code — refuses to guess, refuses to fix

- Commits are invoked as `git -c user.useConfigOnly=true commit …` (in `extensions/git/src/git.ts`, gated on `opts.requireUserConfig`), which suppresses git's `user@hostname` guess and produces `Author identity unknown / *** Please tell me who you are.` (*attested by the verbatim git-output logs pasted in the issues below plus web search; the 4k-line source file could not be fetched whole*).
- [microsoft/vscode#112294](https://github.com/microsoft/vscode/issues/112294) — *"Git fails to auto-detect user.name and user.email after update to 1.52.0"* — was closed **as-designed**. [microsoft/vscode#207728](https://github.com/microsoft/vscode/issues/207728)'s title is the user-facing string: *"Make sure you configure your 'user.name' and 'user.email' in git."*
- It is opt-out: `git.requireGitUserConfig`, described in [`package.nls.json`](https://github.com/microsoft/vscode/blob/main/extensions/git/package.nls.json) as *"Controls whether to require explicit Git user configuration or allow Git to guess if missing."* (*its literal default `true` is inferred from behaviour, not read from source*).
- **The GitHub Authentication extension does not populate git config.** VS Code's own [Working with GitHub](https://code.visualstudio.com/docs/sourcecontrol/github) page keeps `git config --global user.name/user.email` in *Prerequisites*, separate from sign-in; signing in yields credentials only.

### Obsidian Git — the mobile-shaped precedent

- Ships **"Author name for commit"** / **"Author email for commit"** settings that write straight through: `await plugin.gitManager.setConfig("user.name", …)` ([`src/setting/settings.ts`](https://github.com/Vinzent03/obsidian-git/blob/master/src/setting/settings.ts)).
- On mobile it runs `await this.checkAuthorInfo()` **before** `git.commit(...)` and surfaces *"Git author information is not set. Please set it in the settings"* ([obsidian-git#663](https://github.com/Vinzent03/obsidian-git/issues/663) — *string from the issue, not source*). On desktop with unset config you get git's own `fatal: empty ident name (for <>) not allowed` ([#213](https://github.com/Vinzent03/obsidian-git/issues/213)).
- The underlying reason is structural and **directly relevant to the Android-possible constraint**: isomorphic-git's `commit()` defaults `author.name`/`author.email` from config, but [`normalizeAuthorObject`](https://github.com/isomorphic-git/isomorphic-git/blob/main/src/utils/normalizeAuthorObject.js) returns `undefined` when the name is still unset, throwing `MissingNameError` — *"Author name and email must be specified as an argument or in the .git/config file."* And isomorphic-git reads **only** `$GIT_DIR/config`, never `~/.gitconfig` (*isomorphic-git config docs, via search; site 403'd*). On mobile there is no global git config to fall back on **at all**.

### `gh` and `glab`

Neither sets author identity. `gh auth setup-git` *"configures `git` to use GitHub CLI as a credential helper"* ([manual](https://cli.github.com/manual/gh_auth_setup-git)); `glab auth git-credential` is likewise a credential helper only ([GitLab CLI auth](https://docs.gitlab.com/cli/authentication/)). Third-party tools like `glab-setup-git-identity` exist precisely because `glab` doesn't.

**Precedent summary:** every tool that *writes* identity (GitHub Desktop, Obsidian Git) exposes it as an explicit, user-visible, editable step — even when it can derive the value. Every tool that *can't* derive it (VS Code, both CLIs) refuses and tells the user, rather than inventing a placeholder. **Nobody ships a silent fake-address fallback. Cerebrite currently does.**

---

## Recommendation

Fact-finding conclusions only; the product decision stays with [ticket 06](06-auth-mechanisms-decision.md).

1. **The `cerebrite@local` fallback is worse than it looks, and specifically worse than any wrong-but-real address.** A wrong real address is recoverable — GitHub explicitly rebuilds the contribution graph when the address is added to the account, and GitLab resolves authorship at display time. `cerebrite@local` is an undeliverable address that can never be verified onto any account, so its commits are permanently unattributable on GitHub short of a history rewrite that GitHub itself warns does not reliably retract published data. And because libgit2 (unlike the `git` CLI) does not guess an identity, this fires **deterministically** on the target user's machine, not occasionally.
2. **Reading identity is nearly free on both first-class providers.** GitLab: zero extra scope (`api` is already required for push) and `GET /user` hands back `commit_email` — the exact right value. GitHub: `GET /user` costs no permission at all and yields `id` + `login`, from which `<id>+<login>@users.noreply.github.com` is deterministic. The consent-screen cost the ticket asked to price is **zero on GitLab and zero on GitHub unless you want the real address** (which costs one `Email addresses: Read-only` account permission line).
3. **Prefer the derived noreply address over the real one on GitHub**, if identity is ever auto-populated. It is free, it is what GitHub Desktop itself does, it survives username changes (the ID-prefixed form specifically), it counts as a contribution, and it is the only variant that **cannot** trip `GH007` — which is the one documented way an author email hard-rejects a push, and which fires precisely when you use the user's real private address.
4. **Identity capture is a property of provider-API access, not of OAuth.** The pasted-token path gets the same data. The genuinely dark tier is generic SSH / arbitrary hosts, where "ask the user" is the only honest answer — the SSH greeting yields a username on exactly two hosts, is never emitted during an actual `git2` fetch/push, and cannot produce an email. **For [ticket 08](08-connect-and-clone-journey.md): the connect journey needs a user-supplied name/email as the always-present base case, with auto-fill as a convenience layer — not an identity step that exists only on the OAuth branch.**
5. **For [ticket 09](09-sync-and-auth-failure-surfacing.md):** `GH007` is a distinct, actionable push failure with its own remote error code, and it can appear mid-background-sync on an account whose settings changed after connect. It must not be surfaced as a generic push error — Atom shipped exactly that bug, reporting it as *"The tip of your current branch is behind its remote counterpart."*
6. **Precedent is unanimous against a silent placeholder.** GitHub Desktop derives-and-shows; Obsidian Git prompts; VS Code deliberately hard-fails with `user.useConfigOnly=true` and closed the "please guess for me" request as-designed. No comparable tool commits under an invented address.
7. **Android note:** on the isomorphic-git path there is no global git config at all, and `commit()` throws `MissingNameError` without an explicit author. Whatever identity model is chosen must carry the name/email as app-owned state (or write it to the repo-local `.git/config`), not assume a machine-level git identity exists.

### Gaps not closed by primary sources

- Whether GitHub re-links the **display** of old commits after the address is added (docs say only *"Old commits might not be linked"*). The contribution-graph rebuild **is** documented; display re-linking is not.
- Whether `GET /user/emails` includes the noreply address as an entry. Undocumented; treat the noreply address as derived.
- GitLab's exact rendering of an unmatched author (plain name + Gravatar, no link) — issue-tracker sourced.
- The `GH007` error string — issue/gist sourced, not quoted by GitHub docs.
- GitLab's full user-facing private-commit-email format (`<id>-<username>@users.noreply.gitlab.com`) — search-sourced; the authoritative value should be read from `commit_email` rather than derived anyway.
