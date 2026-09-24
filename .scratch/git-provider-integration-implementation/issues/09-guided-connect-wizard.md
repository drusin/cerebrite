# 09: Guided connect wizard (device #1)

**What to build:** Per [ticket 08](../../git-provider-integration/issues/08-connect-and-clone-journey.md)'s Variant-C decision — the guided branching wizard that attaches a remote to an existing local vault, as a compact modal inside Settings.

From the user's perspective: from Settings, the user starts "Connect", is asked (in moderate git vocabulary — "repository", "provider", never "origin"/"branch") whether they already have a repository or want one created; for GitHub/GitLab, "Sign in" walks the device-flow steps from tickets 06/07 and, when creating a new repository, offers private-by-default (public requires an explicit choice, never a hard default); for any other provider, the user pastes a URL and picks access-token or SSH-key auth from tickets 04/05. The wizard ends on the "Commit as" step from ticket 08. Every step carries a "Switch to manual setup" link (wired up fully in ticket 11).

**Blocked by:** 01, 04, 05, 06, 07, 08

- [ ] Wizard entry point in Settings, rendered as a compact modal (not full-screen — that's clone's treatment, ticket 10)
- [ ] Branching question: "do you already have a repository?" — pick-existing vs. create-new
- [ ] Create-new path: repository is created via the provider API (GitHub/GitLab only, using the OAuth token from ticket 06/07); private is the default, public requires an explicit extra choice
- [ ] Pick-existing path: for GitHub/GitLab, a list of the signed-in user's repositories to choose from; for any other provider, a pasted URL
- [ ] Credential-kind branching surfaces the right sub-flow: "Sign in with GitHub/GitLab" (tickets 06/07) as the primary option, "Other ways to connect" (access token or SSH key, tickets 04/05) as a secondary option, always available even for GitHub/GitLab (for orgs blocking third-party apps)
- [ ] Wizard ends on the "Commit as" step (ticket 08) before completing
- [ ] A test fetch (from whichever mechanism ticket ran) gates saving the Connection, with a clear in-wizard error on failure rather than a saved-but-broken Connection
- [ ] Refusal/adoption behavior from ticket 01 (picking a non-root folder, adopting existing content) is surfaced in wizard copy, not just as a raw error
- [ ] Manual QA pass through every branch (create-private, create-public, pick-existing, each of the four credential kinds) confirms no dead ends
