# Automatic git sync, with conflict detection and a copy-safety net

Cerebrite commits and pulls/pushes automatically in the background, without the user manually triggering sync — "git as a first-class citizen" is read as "the user shouldn't have to think about git" (ADR-0001). Actual conflict *resolution* stays out of scope (no bespoke merge UI, per the MVP boundary): a conflict is an ordinary git conflict, resolved with normal git tooling.

The app does, however, detect when a file lands in a conflicted state and saves a copy of the user's locally-conflicting version before any automatic sync step could touch it, so background automation can never silently discard unsynced local edits. This is the one piece of conflict-handling UX the MVP takes on: notice and preserve, never resolve.
