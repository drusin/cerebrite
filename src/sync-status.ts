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
// from "needs attention" (`NeedsAttention`, every other cause). This ticket
// deliberately shows no cause-specific CTA in the needs-attention popup
// beyond "Sync now" -- ticket 13 owns per-cause CTA wording/wiring.
//
// # Testing
//
// Same as connect-wizard.ts: no frontend test runner is configured in this
// project. This module is kept dependency-free and side-effect-free so a
// plain script can exercise it directly, and `tsc --noEmit` type-checks it
// as part of `npm run build`. A one-off verification script
// (`scratchpad/verify-sync-status.ts`, not committed -- see this ticket's
// commit message) was run once during development to exercise every
// state/cause branch below.

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
   * status line. Never includes a call-to-action -- ticket 13's job. */
  statusText: string;
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
  }
}

/** Maps one `SyncStatus` value to the sidebar icon's state/popup content
 * (ticket 12 checklist item 7's frontend logic test target). Exhaustive over
 * every `SyncStatus`/`SyncFailureCause` variant -- a new backend variant
 * fails `tsc` here rather than silently falling through to a generic state. */
export function syncIndicatorFor(status: SyncStatus): SyncIndicator {
  switch (status.state) {
    case "noRemote":
      return { iconState: "notConnected", glyph: "○", statusText: "Not connected" };
    case "syncing":
      return { iconState: "syncing", glyph: "↻", statusText: "Syncing…" };
    case "synced":
      return { iconState: "synced", glyph: "●", statusText: "Synced" };
    case "transient":
      return { iconState: "retrying", glyph: "↻", statusText: `Retrying — ${describeSyncFailureCause(status.cause)}` };
    case "needsAttention":
      return { iconState: "needsAttention", glyph: "⚠", statusText: describeSyncFailureCause(status.cause) };
  }
}
