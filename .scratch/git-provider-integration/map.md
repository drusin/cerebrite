Label: wayfinder:map

# Git provider integration

## Destination

A written spec plus supporting ADRs for connecting a Cerebrite vault to a git provider: how a vault is bound to a remote repository, which authentication mechanisms ship, where credentials live at rest, and what the connect/clone onboarding journey looks like end to end. Ready to hand to implementation planning, as the MVP map was.

Desktop is the near-term target; Android implementation is explicitly *not* part of this map, but no decision here may pick a mechanism that is architecturally impossible on Android.

## Notes

- Domain vocabulary and settled decisions live in [CONTEXT.md](../../CONTEXT.md) and [docs/adr/](../../docs/adr/) — read both before resolving any ticket. Note that **vault** is used throughout the code but has no glossary entry yet; [ticket 05](issues/05-vault-repo-binding.md) is expected to add one.
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

_(none yet — charting session only)_

## Not yet specified

- **Registering OAuth applications** with GitHub and GitLab, and wherever the resulting client IDs live in the repo. A `task` ticket if and only if OAuth survives [ticket 01](issues/01-https-credential-landscape.md)/[02](issues/02-secretless-oauth-flows.md) and the [mechanism decision](issues/06-auth-mechanisms-decision.md).
- **Git author identity.** `vault.rs`'s `commit_all` falls back to `Cerebrite <cerebrite@local>` whenever `repo.signature()` fails — i.e. on any machine with no global git config, which is precisely the non-git-native user this effort targets. Their entire push history would carry a fake address, and on GitHub it would not associate with their account. Whether this map fixes it, and whether an OAuth login should populate it automatically, waits on the mechanism decision.
- **Credential loss and revocation mid-flight.** What background sync does when a token expires, is revoked, or a key stops working — partly a UX question for [ticket 09](issues/09-sync-and-auth-failure-surfacing.md), partly a storage question, not sharp enough to separate until both are resolved.
- **Existing vaults already wired up externally.** Users (including the developer) who configured `origin` by hand with working credentials: does connecting in-app adopt, override, or refuse that? Waits on the vault↔repo binding model.

## Out of scope

- **Any hosted server component** — token broker, proxy, sync service. Ruled out as a standing constraint above, not merely unscheduled.
- **OAuth against self-hosted or enterprise instances** (self-hosted GitLab, GitHub Enterprise Server) via a user-supplied base URL and client ID. Those users reach the same place through the generic SSH path, and pasting a client ID is no more effortless than pasting an SSH URL.
- **Bitbucket as a first-class OAuth provider.** Covered by the generic path like any other provider.
- **Building the Android implementation** of whatever is decided here. Decisions must permit it; this map does not deliver it.
- **Conflict-resolution UX** beyond what [ADR-0006](../../docs/adr/0006-automatic-git-sync-with-conflict-detection-and-copy-safety-net.md) already settles (detect and preserve, never resolve).
