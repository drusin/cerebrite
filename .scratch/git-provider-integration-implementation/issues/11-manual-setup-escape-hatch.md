# 11: Manual setup escape hatch

**What to build:** Per [ticket 08](../../git-provider-integration/issues/08-connect-and-clone-journey.md), a persistent "Switch to manual setup" link on every wizard step (tickets 09 and 10), dropping into a transparent, git-literal single-screen form using full git vocabulary — for users who already know what they want.

From the user's perspective: at any point in either wizard, clicking "Switch to manual setup" replaces the guided flow with a plain form. For connect, that's an always-visible "Sync" section inline in Settings showing raw remote URL, branch, and credential-kind fields. For clone, that's an explicit "git clone" dialog: raw remote URL, branch, destination folder. Both forms use the same underlying mechanisms (tickets 04–07) as the guided wizard — this is a different door into the same rooms, not a separate implementation.

**Blocked by:** 04, 05, 06, 07

- [ ] "Switch to manual setup" link present on every step of both wizards (09, 10), always reachable, not just on the first screen
- [ ] Connect manual form: an always-visible "Sync" accordion/section in Settings with raw fields for remote URL, branch, and credential kind (each kind's sub-form reuses tickets 04–07's mechanisms directly)
- [ ] Clone manual form: a standalone "git clone" dialog with raw remote URL, branch, and destination folder fields, reachable even with no existing vault/Settings surface to anchor it to
- [ ] Both manual forms enforce the same test-fetch-before-save gate as the guided wizards
- [ ] Manual smoke test: switching mid-wizard to manual setup does not lose the ability to complete the connection (may re-ask for values already entered — that's an acceptable, documented limitation per ticket 08's answer, not a bug)
