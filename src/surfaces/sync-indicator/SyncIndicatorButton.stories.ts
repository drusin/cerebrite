// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per icon state (ticket 02 checklist), built from real `SyncStatus`
// values run through the real `syncIndicatorFor`, not hand-written
// `SyncIndicator` objects.
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import type { SyncStatus } from "../../vault-api";
import { syncIndicatorFor } from "../../sync-status";
import SyncIndicatorButton from "./SyncIndicatorButton.vue";

const meta: Meta<typeof SyncIndicatorButton> = {
  component: SyncIndicatorButton,
  title: "Surfaces/Sync indicator/SyncIndicatorButton",
};
export default meta;

type Story = StoryObj<typeof SyncIndicatorButton>;

function forStatus(status: SyncStatus, variant: "footer" | "rail" = "footer"): Story["args"] {
  return { indicator: syncIndicatorFor(status), variant, open: false };
}

export const NotConnected: Story = {
  args: forStatus({ state: "noRemote" }),
};

export const Syncing: Story = {
  args: forStatus({ state: "syncing" }),
};

export const Synced: Story = {
  args: forStatus({ state: "synced" }),
};

export const Retrying: Story = {
  args: forStatus({
    state: "transient",
    cause: { cause: "networkUnreachable", detail: "the network is unreachable" },
  }),
};

export const NeedsAttention: Story = {
  args: forStatus({
    state: "needsAttention",
    cause: { cause: "other", detail: "Something went wrong." },
  }),
};

export const Warning: Story = {
  name: "Warning (refreshedSignInNotSaved)",
  args: forStatus({
    state: "needsAttention",
    cause: { cause: "refreshedSignInNotSaved", detail: "no keychain reachable", credentialKind: "oauth_sign_in" },
  }),
};

export const RailVariant: Story = {
  args: forStatus({ state: "synced" }, "rail"),
};
