# 10: Guided clone wizard (device #2)

**What to build:** Per [ticket 08](../../git-provider-integration/issues/08-connect-and-clone-journey.md)'s Variant-C decision — the first-run wizard for a device with no local vault yet, as a full-screen takeover (there's no window/vault to put a modal on top of), handling the auth-before-repo-exists ordering problem.

From the user's perspective: on a fresh install with no vault, alongside the existing folder picker, the user can instead say they already have a repository elsewhere. They authenticate first (device flow or paste credentials — nothing local exists yet to read config from), then pick or paste the repository, then Cerebrite clones it and figures out which of ticket 01's three remote states applies (empty repo, has `vault/` already, has content but no `vault/`, or a `vault/` with unrelated history — refused with an explanation). The wizard ends on the "Commit as" step from ticket 08, suggested from the cloned repo's config if any, or the provider.

**Blocked by:** 01, 04, 05, 06, 07, 08

- [ ] Full-screen wizard surface, reachable from the existing first-run folder-picker screen as an alternative path ("I already have a repository")
- [ ] Authentication happens before any local repository exists: OAuth device flow (06/07) or manual credential entry (04/05), with no dependency on repo-local git config or a credential helper
- [ ] After authenticating, the user picks (GitHub/GitLab: from a list) or pastes (others) the repository to clone, and a destination folder
- [ ] Clone target is `<destination>/vault/` per ticket 01/05: an empty remote pushes nothing extra, a remote with `vault/` already is the ordinary clone case, a remote with content but no `vault/` is adopted, a remote with an unrelated `vault/` history is refused with an explanation (told to clone into a new folder)
- [ ] Wizard ends on the "Commit as" step (ticket 08), prefilled from the cloned repo's config or the provider
- [ ] A test fetch/clone success gates completing the wizard; a failed clone shows a clear in-wizard error
- [ ] Manual QA pass covering all four remote states from ticket 01 and each of the four credential kinds
