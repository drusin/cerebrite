# 08: "Commit as" author identity step

**What to build:** Per [ticket 11](../../git-provider-integration/issues/11-git-author-identity-product.md), delete the `Cerebrite <cerebrite@local>` fallback and give every vault a confirmed author, stored repo-locally.

From the user's perspective: at vault setup (pick, create, or clone — before the first commit), a "Commit as" step shows a name and email, pre-filled from the best available source (existing repo-local git config, then global git config, then — for an OAuth sign-in only — the provider's noreply email/profile name), and always editable. The user confirms or edits it before continuing. Later, Settings has an editable "Commit as" field that affects future commits only. An unrealistic-looking domain (`.local`, no dot) gets a warning, never a block.

**Blocked by:** 06, 07

- [ ] The `Cerebrite <cerebrite@local>` fallback is deleted from both `vault.rs::commit_all` and the merge-commit path in `sync.rs`
- [ ] "Commit as" is a vault-setup step (pick/create/clone), shown before the first commit, not a branch inside the connect journey
- [ ] Prefill precedence implemented in order: repo-local `.git/config` → global `~/.gitconfig` → (OAuth sign-in only) provider identity — GitHub's `<id>+<login>@users.noreply.github.com` via `GET /user` (no extra scope requested, real email never fetched), GitLab's `commit_email` via the `api`/`read_user` scope already held → empty, for access-token/SSH connections with no git config to draw on
- [ ] Confirming writes `user.name`/`user.email` to the repository's `.git/config` (both author and committer set to the same value); a later change to global config never silently changes an already-confirmed vault's author
- [ ] Connecting a vault whose author differs from the provider's suggestion offers the switch once ("Commit as `…@users.noreply.github.com` from now on?"); default is to keep the current author, never switch silently
- [ ] Settings has an editable "Commit as" name/email field; a note states it affects future commits only, and history is never rewritten
- [ ] Validation: non-empty name; email requires `@` and a domain with no `<`, `>`, or line breaks; a domain that can't be real (`.local`, `localhost`, no dot) warns without blocking
- [ ] A second device cloning the same vault goes through its own "Commit as" confirmation (values suggested, not synced)
- [ ] Tests cover the prefill precedence chain and the fallback deletion (a vault with no git config anywhere no longer produces a `cerebrite@local` commit)
