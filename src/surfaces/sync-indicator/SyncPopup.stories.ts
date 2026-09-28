// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook).
// Ticket 02 checklist: the popup with/without provider and last-synced,
// plus one story per `SyncFailureCause` variant (all 11 from
// `vault-api.ts`), each showing that cause's text and its 0-2 CTAs via the
// real `syncIndicatorFor`/`ctasFor`. Every story passes a fixed anchor rect.
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import type { SyncFailureCause, SyncStatus } from "../../vault-api";
import { syncIndicatorFor } from "../../sync-status";
import SyncPopup from "./SyncPopup.vue";

const meta: Meta<typeof SyncPopup> = {
  component: SyncPopup,
  title: "Surfaces/Sync indicator/SyncPopup",
};
export default meta;

type Story = StoryObj<typeof SyncPopup>;

// A fixed anchor rect, roughly where the sidebar footer button sits, per
// the checklist's "stories pass a fixed anchor rect".
const FIXED_ANCHOR = new DOMRect(24, 620, 200, 36);

function forStatus(
  status: SyncStatus,
  extra: Partial<NonNullable<Story["args"]>> = {},
): Story["args"] {
  return {
    indicator: syncIndicatorFor(status),
    anchor: FIXED_ANCHOR,
    provider: null,
    lastSyncedAt: null,
    syncNowPending: false,
    ...extra,
  };
}

// --- With/without provider and last-synced ----------------------------------

export const WithProviderAndLastSynced: Story = {
  args: forStatus(
    { state: "synced" },
    { provider: "GitHub", lastSyncedAt: Math.floor(Date.parse("2026-09-27T10:15:00Z") / 1000) },
  ),
};

export const WithoutProviderOrLastSynced: Story = {
  args: forStatus({ state: "noRemote" }),
};

// --- One story per `SyncFailureCause` variant (11) --------------------------
//
// `networkUnreachable` only ever reaches a `transient` `SyncStatus` (see
// `sync-status.ts`'s `ctasFor` comment) -- every other cause reaches
// `needsAttention`.

export const CauseNetworkUnreachable: Story = {
  args: forStatus({
    state: "transient",
    cause: { cause: "networkUnreachable", detail: "the network is unreachable" },
  }),
};

function needsAttention(cause: SyncFailureCause): Story["args"] {
  return forStatus({ state: "needsAttention", cause });
}

export const CauseCredentialRejected: Story = {
  args: needsAttention({ cause: "credentialRejected", detail: "the token was rejected", credentialKind: "access_token" }),
};

export const CauseNonFastForwardPush: Story = {
  args: needsAttention({ cause: "nonFastForwardPush", detail: "the remote has newer commits" }),
};

export const CauseConflict: Story = {
  args: needsAttention({ cause: "conflict", detail: "local and remote changes conflict" }),
};

export const CauseOther: Story = {
  args: needsAttention({ cause: "other", detail: "an unexpected error occurred" }),
};

export const CauseHostKeyUnconfirmed: Story = {
  args: needsAttention({ cause: "hostKeyUnconfirmed", host: "github.com", fingerprint: "SHA256:abcd1234" }),
};

export const CauseHostKeyMismatch: Story = {
  args: needsAttention({ cause: "hostKeyMismatch", host: "github.com", fingerprint: "SHA256:efgh5678" }),
};

export const CauseOauthReconnectRequired: Story = {
  args: needsAttention({
    cause: "oauthReconnectRequired",
    detail: "the sign-in expired and can't be refreshed",
    credentialKind: "oauth_sign_in",
  }),
};

export const CauseKeychainLocked: Story = {
  args: needsAttention({ cause: "keychainLocked", detail: "unlock your keychain to continue" }),
};

export const CauseKeychainUnavailable: Story = {
  args: needsAttention({ cause: "keychainUnavailable", detail: "no keychain could be reached" }),
};

export const CauseRefreshedSignInNotSaved: Story = {
  args: needsAttention({
    cause: "refreshedSignInNotSaved",
    detail: "the renewed sign-in couldn't be saved",
    credentialKind: "oauth_sign_in",
  }),
};
