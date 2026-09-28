// Ticket 10's guided clone wizard has a "Switch to manual setup" escape
// hatch (`clone-wizard.ts`'s `switchToManual` action) that hands off to the
// standalone "git clone" manual form, carrying over whatever `remoteUrl`/
// `destination` the wizard had already committed. Both surfaces are now
// separate Vue islands, each driven by `state/ui.ts`'s `vaultView` alone
// (which has no payload slot -- it's just `'cloneWizard' | 'cloneManual' |
// ...`), so this tiny module is the one-shot carry-over channel between
// them: `useCloneWizard` writes it right before switching `vaultView` to
// `'cloneManual'`, and `CloneManualFormContainer` reads-and-clears it the
// next time it sees that view become active.
//
// Deliberately not `src/state/`: this is single-use handoff data for one
// specific transition between two sibling surfaces, not state anything else
// in the app reads (spec.md#shared-state-statets's "local state stays
// local").
export interface CloneManualPrefill {
  remoteUrl: string;
  destination: string;
}

let pending: CloneManualPrefill | null = null;

export function setCloneManualPrefill(prefill: CloneManualPrefill): void {
  pending = prefill;
}

/** Reads and clears the pending prefill, if any -- a one-shot read, so a
 * later plain open of the manual form (not via the wizard's handoff) never
 * sees stale data from a previous switch. */
export function takeCloneManualPrefill(): CloneManualPrefill | null {
  const value = pending;
  pending = null;
  return value;
}
