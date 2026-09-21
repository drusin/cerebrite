Label: wayfinder:map

# Git provider integration

## Destination

A written spec plus supporting ADRs for connecting a Cerebrite vault to a git provider: how a vault is bound to a remote repository, which authentication mechanisms ship, where credentials live at rest, and what the connect/clone onboarding journey looks like end to end. Ready to hand to implementation planning, as the MVP map was.

Desktop is the near-term target; Android implementation is explicitly *not* part of this map, but no decision here may pick a mechanism that is architecturally impossible on Android.

## Notes

- Domain vocabulary and settled decisions live in [CONTEXT.md](../../CONTEXT.md) and [docs/adr/](../../docs/adr/) — read both before resolving any ticket. **vault** now has a glossary entry, added by [ticket 05](issues/05-vault-repo-binding.md) along with [ADR-0011](../../docs/adr/0011-vault-is-a-hardcoded-subdirectory-of-its-git-repository.md).
- Where things stand today: `src-tauri/src/sync.rs` already runs a full fetch → merge/fast-forward → push cycle against `origin` on a timer, with a best-effort credential callback (`ssh_key_from_agent` → `Cred::credential_helper` → `Cred::default()`) that has never been exercised against a real provider. `src-tauri/src/vault.rs` only ever `Repository::init`s a picked folder — **there is no clone path in production code at all**. There is no remote-configuration UI and no sync-status UI; see the first three entries in [docs/known-gaps.md](../../docs/known-gaps.md).

### Standing constraints

Every ticket inherits these; they are settled, not open for relitigation.

- **No server-side component, ever.** No hosted token broker, no client secret embedded in a distributed binary. If a provider offers no secret-free flow, that provider gets no OAuth. This follows from [ADR-0001](../../docs/adr/0001-git-native-markdown-as-permanent-source-of-truth.md): an offline-first, self-hostable tool must not depend on infrastructure the project has to keep alive forever.
- **Android-possible, not Android-now.** A mechanism that cannot be made to work on Android is disqualified. Actually building the Android path is out of scope — Android sync is independently blocked on the Storage Access Framework vault-picker gap.
- **Two tiers of provider support.** GitHub and GitLab must be effortless, or as close to it as is sensible to implement. Every other provider — self-hosted Forgejo/Gitea, Bitbucket, a bare SSH host — must *work*, via SSH (or HTTPS tokens, if [ticket 01](issues/01-https-credential-landscape.md) finds that sensible).
- **No dependency may be end-of-life, deprecated, or abandoned** (carried over from the MVP map).
- Call the `grilling` and `domain-modeling` skills for any ticket that hinges on a product or domain decision rather than a pure research question.

## Decisions so far

<!-- one line per resolved ticket; zoom the link for the detail -->

- [HTTPS credential landscape across git providers](issues/01-https-credential-landscape.md): Password auth is dead/dying everywhere; ship "paste a token over HTTP Basic auth" as the primary generic path for non-GitHub/GitLab providers — `Cred::userpass_plaintext` needs no new code, works unchanged on Android, but GitLab/Bitbucket tokens expire non-negotiably within a year with no browser-less renewal.
- [Secret-free OAuth flows for GitHub and GitLab](issues/02-secretless-oauth-flows.md): Both providers have genuine secret-free OAuth paths. Device Authorization Grant is the constraint-safest choice for both (no redirect plumbing, Android-safe); register as a GitHub App (not OAuth App) for GitHub. The resulting OAuth token is directly usable as the git HTTPS credential — OAuth and HTTPS-token auth are the same mechanism, two ways of acquiring the credential. GitLab OAuth tokens need the broad `api` scope, not `write_repository`, to push.
- [SSH key management on desktop and Android](issues/03-ssh-key-management.md): SSH is viable but never as the sole mechanism — agent auth is opportunistic-only (inconsistent across platforms, absent on Android). Requires enabling the `ssh_key_from_memory` git2 feature, in-app ed25519 keygen via the `ssh-key` crate, no passphrase on generated keys (store raw key in OS secure storage instead), and explicit host-key verification (pin known providers, TOFU+dialog for others — libssh2 does not verify safely by default).
- [What providers expose about the signed-in user, and what a wrong author email costs](issues/10-git-author-identity-facts.md): Reading identity is nearly free — GitLab's already-required `api` scope returns `commit_email` directly, and GitHub's `GET /user` needs no permission at all, yielding `<id>+<login>@users.noreply.github.com` deterministically (only the *real* address costs an extra consent line, and it is the one variant that can hard-fail a push with `GH007`). A wrong-but-real email is recoverable — GitHub rebuilds the contribution graph when the address is added — but `cerebrite@local` is unverifiable and therefore permanent; libgit2, unlike the git CLI, never guesses, so the fallback fires deterministically. Identity capture is a property of provider-API access, not of OAuth, and is impossible on the generic SSH tier — so a user-supplied name/email must be the base case.
- [Secret storage options at rest](issues/04-secret-storage-options.md): No option is free of trade-offs. Recommend OS-native keychain (via `keyring-core` + explicit store crates) as default, with an explicit, user-visible fallback to permission-protected plaintext when the keychain is unavailable (a real, non-hypothetical gap on headless/minimal-WM Linux, this project's own target user base) — never a silent degrade. App-managed encryption without a keychain- or passphrase-derived key is security theater.
- [What a vault is, in git terms](issues/05-vault-repo-binding.md): A vault is `vault/` — a **hardcoded subdirectory** at the root of a git repository, never the root itself, so the root stays free for the user's `README.md`/`AGENTS.md`; every `.md` under it is a page, including ones Cerebrite never created. The user picks the *repository*; a folder inside a repository but not its root is refused, not silently nested. One vault, one repository, one `origin`. Commit and push stay **repo-wide** (disclosed at pick time, not designed away). Branch left implicit: default branch on clone, whatever is checked out otherwise. Recorded as [ADR-0011](../../docs/adr/0011-vault-is-a-hardcoded-subdirectory-of-its-git-repository.md) plus a `Vault` entry in [CONTEXT.md](../../CONTEXT.md).

## Not yet specified

- **Registering OAuth applications** with GitHub and GitLab, and wherever the resulting client IDs live in the repo. A `task` ticket if and only if OAuth survives [ticket 01](issues/01-https-credential-landscape.md)/[02](issues/02-secretless-oauth-flows.md) and the [mechanism decision](issues/06-auth-mechanisms-decision.md).
- **Git author identity — the product half.** The facts are now settled in [ticket 10](issues/10-git-author-identity-facts.md); what remains is whether *this* map fixes the `cerebrite@local` fallback at all, and where the always-present name/email step lands in the connect journey given that auto-fill can only ever be a convenience layer over it. Waits on the [mechanism decision](issues/06-auth-mechanisms-decision.md) and feeds [ticket 08](issues/08-connect-and-clone-journey.md).
- **Credential loss and revocation mid-flight.** What background sync does when a token expires, is revoked, or a key stops working — partly a UX question for [ticket 09](issues/09-sync-and-auth-failure-surfacing.md), partly a storage question, not sharp enough to separate until both are resolved.
- **Migrating today’s root-level vaults.** [ADR-0011](../../docs/adr/0011-vault-is-a-hardcoded-subdirectory-of-its-git-repository.md) hardcodes `vault/`, so every repository that exists today (including the developer’s own) has its pages in the wrong place. Supporting both layouts is already ruled out; what remains is whether this map’s spec merely *flags* the migration for implementation planning or has to decide its shape. Likely a one-line note rather than a ticket — revisit once [ticket 08](issues/08-connect-and-clone-journey.md) shows whether the connect journey has to handle it in the UI.

## Out of scope

- **Any hosted server component** — token broker, proxy, sync service. Ruled out as a standing constraint above, not merely unscheduled.
- **OAuth against self-hosted or enterprise instances** (self-hosted GitLab, GitHub Enterprise Server) via a user-supplied base URL and client ID. Those users reach the same place through the generic SSH path, and pasting a client ID is no more effortless than pasting an SSH URL.
- **Bitbucket as a first-class OAuth provider.** Covered by the generic path like any other provider.
- **Building the Android implementation** of whatever is decided here. Decisions must permit it; this map does not deliver it.
- **Conflict-resolution UX** beyond what [ADR-0006](../../docs/adr/0006-automatic-git-sync-with-conflict-detection-and-copy-safety-net.md) already settles (detect and preserve, never resolve).
