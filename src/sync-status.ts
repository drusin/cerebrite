// Ticket 12: a plain, framework-free mapping from the Rust core's
// `sync::SyncStatus` (via `get_sync_status`/`sync-status-changed`, see
// vault-api.ts) to what the sidebar footer icon and its popup show --
// deliberately factored out of main.ts, same precedent as
// connect-wizard.ts/clone-wizard.ts, so the five-state mapping is testable
// in isolation without a DOM.
//
// The two-bucket taxonomy this maps from is ticket 03's `SyncFailureCause`/
// `SyncStatus` split, read exactly as `.scratch/git-provider-integration/
// issues/09-sync-and-auth-failure-surfacing.md` resolved it: "retrying"
// (`Transient`, always `NetworkUnreachable`, no CTA) is visually distinct
// from "needs attention" (`NeedsAttention`, every other cause). Ticket 12
// deliberately showed no cause-specific CTA in the needs-attention popup
// beyond "Sync now" -- ticket 13 (this file's `ctasFor`/`SyncCta`, plus the
// `severity` field) is what fills that in: every needs-attention cause now
// gets its own message (`describeSyncFailureCause`) and CTA button(s)
// (`ctasFor`), and `refreshedSignInNotSaved` gets a `"warning"` severity
// instead of `"error"` so `main.ts` can style it as a lower-key heads-up
// without it being a separate `iconState`.
//
// # Testing
//
// Same as connect-wizard.ts: no frontend test runner is configured in this
// project. This module is kept dependency-free and side-effect-free so a
// plain script can exercise it directly, and `tsc --noEmit` type-checks it
// as part of `npm run build`. A one-off verification script
// (`scratchpad/verify-sync-status.ts`, not committed -- see ticket 12's and
// ticket 13's commit messages) was run once during ticket 12's development,
// and again during ticket 13's to also cover every new cause's message,
// `ctasFor` mapping, and severity.

import type { SyncStatus, SyncFailureCause, CredentialKind } from "./vault-api";

/** The sidebar icon's five distinct visual states, per the ticket. */
export type SyncIconState = "notConnected" | "synced" | "syncing" | "retrying" | "needsAttention";

/** What the sidebar icon and its popup render for a given `SyncStatus`. */
export interface SyncIndicator {
  iconState: SyncIconState;
  /** A single calm glyph for the icon itself -- distinct per state (and, for
   * "syncing" vs. "retrying", also distinguished by the caller's CSS class,
   * since both use a rotating-arrows glyph but mean different things). */
  glyph: string;
  /** A short label for the icon's `aria-label`/`title` and the popup's
   * status line. Never includes a call-to-action text -- see `ctas` below. */
  statusText: string;
  /** Ticket 13 checklist item 4: `needsAttention` is a hard failure by
   * default ("error"). `refreshedSignInNotSaved` is the one cause that's a
   * lower-key heads-up instead -- sync is still working right now -- so
   * `main.ts` can style it distinctly (e.g. a calmer color) without it being
   * a separate top-level `iconState`, matching the ticket's "not a separate
   * top-level state" requirement. Always `"error"` for every other state,
   * including `retrying`, so callers don't have to special-case its absence. */
  severity: "error" | "warning";
  /** The needs-attention popup's call-to-action button(s) for this status's
   * cause -- empty for every state that isn't `needsAttention` (`retrying`
   * never gets a CTA beyond the popup's always-present "Sync now", per
   * ticket 12's original decision). */
  ctas: SyncCta[];
}

function credentialLabel(kind: CredentialKind): string {
  switch (kind) {
    case "access_token":
      return "access token";
    case "ssh_key":
      return "SSH key";
    case "oauth_sign_in":
      return "sign-in";
  }
}

/** A short human description of a `SyncFailureCause`, mirroring (not
 * duplicating the classification logic of) `sync.rs`'s own `Display` impl. */
export function describeSyncFailureCause(cause: SyncFailureCause): string {
  switch (cause.cause) {
    case "networkUnreachable":
      return `network problem: ${cause.detail}`;
    case "credentialRejected":
      return cause.credentialKind
        ? `${credentialLabel(cause.credentialKind)} rejected: ${cause.detail}`
        : `credential rejected: ${cause.detail}`;
    case "nonFastForwardPush":
      return `push rejected: ${cause.detail}`;
    case "conflict":
      return `conflict: ${cause.detail}`;
    case "other":
      return cause.detail;
    case "hostKeyUnconfirmed":
      return `unknown SSH host key for ${cause.host} (${cause.fingerprint}) -- confirm before connecting`;
    case "hostKeyMismatch":
      return `SSH host key for ${cause.host} no longer matches what was trusted (now ${cause.fingerprint})`;
    case "oauthReconnectRequired":
      return `${credentialLabel(cause.credentialKind)} expired and can't be refreshed -- reconnect required: ${cause.detail}`;
    case "keychainLocked":
      return `Your keychain is locked: ${cause.detail}`;
    case "keychainUnavailable":
      return `No keychain could be reached: ${cause.detail}`;
    case "refreshedSignInNotSaved":
      return `Your renewed ${credentialLabel(cause.credentialKind)} couldn't be saved and will be lost when the app quits.`;
  }
}

/** Ticket 13: every needs-attention cause gets its own call-to-action(s), not
 * just ticket 12's single generic "Sync now". `id` is what `main.ts` wires
 * up to an actual handler; `label` is the button text. A cause can have more
 * than one CTA (`keychainUnavailable`'s two options) or none at all (a
 * `Transient` cause never reaches this function -- see `syncIndicatorFor`). */
export type SyncCtaId =
  | "reconnect"
  | "unlockAndRetry"
  | "setUpKeychain"
  | "storeAsPlaintext"
  | "retrySync"
  | "openConflictBackups";

export interface SyncCta {
  id: SyncCtaId;
  label: string;
}

/** Maps a `SyncFailureCause` to the CTA button(s) its needs-attention popup
 * entry shows (ticket 13 checklist). Exhaustive over every cause, same
 * discipline as `describeSyncFailureCause` -- a new backend variant fails
 * `tsc` here rather than silently showing no CTA at all. */
export function ctasFor(cause: SyncFailureCause): SyncCta[] {
  switch (cause.cause) {
    case "networkUnreachable":
      // Never actually reached: `networkUnreachable` only ever produces a
      // `Transient` status, which `main.ts` never renders CTAs for. Kept
      // here (rather than narrowing the parameter type) so this function
      // stays a total match over `SyncFailureCause` as a whole.
      return [];
    case "credentialRejected":
    case "oauthReconnectRequired":
    case "hostKeyMismatch":
    case "hostKeyUnconfirmed":
      // Ticket 13 checklist item 1 (and, for the two host-key causes, this
      // implementation's own extension of it -- see sync-status.ts's module
      // doc comment / the ticket 13 commit message): every one of these is
      // fixed by re-establishing the connection, pre-filled with what's
      // already known, rather than by retrying or restarting the wizard.
      return [{ id: "reconnect", label: "Reconnect" }];
    case "refreshedSignInNotSaved":
      // A lower-key warning, but per ticket 09's resolved answer ("reconnect
      // to fix") it still offers the same fix as a hard credential failure.
      return [{ id: "reconnect", label: "Reconnect" }];
    case "keychainLocked":
      return [{ id: "unlockAndRetry", label: "Unlock and retry" }];
    case "keychainUnavailable":
      return [
        { id: "setUpKeychain", label: "Set up a keychain" },
        { id: "storeAsPlaintext", label: "Store as plaintext instead" },
      ];
    case "conflict":
      return [{ id: "openConflictBackups", label: "Open backup folder" }];
    case "nonFastForwardPush":
      return [{ id: "retrySync", label: "Retry sync" }];
    case "other":
      // Unclassified -- per issue 09's resolved answer ("any other
      // unclassified sync error" shares the generic needs-attention
      // treatment), the only generically-safe action is trying again.
      return [{ id: "retrySync", label: "Retry sync" }];
  }
}

/** Maps one `SyncStatus` value to the sidebar icon's state/popup content
 * (ticket 12 checklist item 7's frontend logic test target). Exhaustive over
 * every `SyncStatus`/`SyncFailureCause` variant -- a new backend variant
 * fails `tsc` here rather than silently falling through to a generic state. */
export function syncIndicatorFor(status: SyncStatus): SyncIndicator {
  switch (status.state) {
    case "noRemote":
      return { iconState: "notConnected", glyph: "○", statusText: "Not connected", severity: "error", ctas: [] };
    case "syncing":
      return { iconState: "syncing", glyph: "↻", statusText: "Syncing…", severity: "error", ctas: [] };
    case "synced":
      return { iconState: "synced", glyph: "●", statusText: "Synced", severity: "error", ctas: [] };
    case "transient":
      return {
        iconState: "retrying",
        glyph: "↻",
        statusText: `Retrying — ${describeSyncFailureCause(status.cause)}`,
        severity: "error",
        ctas: [],
      };
    case "needsAttention":
      return {
        iconState: "needsAttention",
        glyph: "⚠",
        statusText: describeSyncFailureCause(status.cause),
        severity: status.cause.cause === "refreshedSignInNotSaved" ? "warning" : "error",
        ctas: ctasFor(status.cause),
      };
  }
}
