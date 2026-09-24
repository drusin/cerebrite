# The connect and clone onboarding journey

Type: prototype
Status: resolved
Blocked by: 05, 06

## Question

What does the user actually see and do, from "I have a vault on this machine" to "it is syncing with my provider" — and from "I have nothing on this machine" to "my vault is here"?

This is where "make git provider integration easy" is won or lost, and it is the reason the map's scope is the whole journey rather than auth alone. Build a rough, throwaway artifact to react to rather than describing screens in prose.

Cover both directions, because they are not symmetric:

- **Connect (device #1)**: an existing local vault gains a remote. Does the user paste a URL, or pick from a list of their repositories after logging in? Can the app **create** the repository on the provider for them — and if so, public or private, and is that a choice or a hard default? (Private must be at minimum the default; a second brain accidentally made public is an unrecoverable mistake.)
- **Clone (device #2)**: no local vault exists yet. Today there is **no clone path in production code at all** — `Repository::clone` appears only inside a sync test — so this is entirely new ground. Note the ordering problem: the user must authenticate *before* a repo exists locally, so nothing can be read from that repo's config to authenticate with. Any design that assumes an existing `origin` or a repo-local credential helper fails here.
- **Where this lives in the UI.** The Settings modal (`src/main.ts`) is the obvious home for reconnecting, but first-run clone happens before any vault exists, next to the existing folder picker. These may need to be two different surfaces.
- **How much git leaks through.** Does the user ever see the words "remote", "branch", "origin"? ADR-0006 reads "git as a first-class citizen" as "the user shouldn't have to think about git" — this journey is the sharpest test of that reading so far.

Link the prototype artifact from this ticket rather than pasting it inline.

## Asset

Prototype committed on throwaway branch `prototype/git-connect-clone-journey`
(commit `36d363d`): `prototype-git-connect.html`. Three structurally
different variants, switchable via `?variant=A|B|C` and the floating
bottom-bar switcher (also `←`/`→` keys):

- **A — Vault-first, git hidden**: first-run overlay wizard living in
  Settings › Sync. No git vocabulary at all ("link an account", "keep it
  private", "bring your notes here"); warm, one-decision-per-screen.
- **B — Transparent, power-user**: an always-visible "Sync" accordion
  section inline in the real Settings modal (remote/branch/credential-kind
  shown as plain fields), *not* a wizard. Clone is a separate, explicit
  "git clone" dialog (raw URL, branch, destination folder) rather than
  folded into Settings, since nothing local exists yet to attach a Settings
  entry point to.
- **C — Guided branching wizard**: question-driven stepper ("Do you already
  have a repository?") with moderate git vocabulary ("repository",
  "provider", never "origin"/"branch"). Deliberately uses **two different
  chrome treatments for the same step content** to make the map's
  "where does this live" question concrete: full-screen takeover for
  first-run clone (there's no window/vault yet to put a modal on top of)
  vs. a compact modal inside Settings for reconnecting an existing vault.

All three cover both directions (Connect: pick-from-list or create-new,
private-by-default; Clone: auth-before-repo-exists ordering; both end on a
"Commit as" author-confirmation step per ticket 11) and all credential
kinds from ADR-0012 (OAuth device flow, access token, SSH key).

Run: `npm run dev` on the `prototype/git-connect-clone-journey` branch,
then open `http://localhost:1420/prototype-git-connect.html`.

Awaiting reaction/discussion (HITL) before this ticket resolves.

## Answer

Variant C (guided branching wizard) is the default surface for both
directions, keeping its two chrome treatments: a full-screen takeover for
first-run clone (nothing exists yet to anchor a modal to) and a compact
modal inside Settings for reconnecting an existing vault. Variant C's
moderate git vocabulary ("repository", "provider", never "origin"/"branch")
is the default register — variant A's fully-hidden vocabulary is dropped.

Every step of the wizard, in both directions, carries a persistent
**"Switch to manual setup"** escape hatch that drops into variant B's
transparent, single-screen form instead of continuing the step-by-step
flow — for connect, that's the always-visible Sync section inline in
Settings (raw remote/branch/credential-kind fields); for clone, that's the
explicit "git clone" form (raw remote URL, branch, destination folder).
Manual setup uses B's full git vocabulary throughout, on the premise that a
user who reaches for it already wants that detail. The escape hatch applies
to **both** connect and clone, not just connect — clone gets its own
manual/raw form rather than being wizard-only, even though there's no
existing Settings surface to anchor it to on a fresh device.

Left for implementation planning, not re-opened as a ticket here: the exact
placement/wording of the "Switch to manual setup" link on each step, and
whether progress already made in the wizard (credential kind chosen, repo
picked) carries over when switching to manual mid-flow.

Primary source: `prototype-git-connect.html` on branch
`prototype/git-connect-clone-journey` (commit `36d363d`, plus a follow-up
fix for the clone flow's credential-kind branching). The prototype itself
is not updated to reflect this merged C+B answer — it remains the
comparison artifact that produced the decision, not the design.
