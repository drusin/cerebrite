# The connect and clone onboarding journey

Type: prototype
Status: open
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
