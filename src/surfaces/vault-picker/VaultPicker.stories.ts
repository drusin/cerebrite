// CSF3, colocated -- no MDX/docs pages/autodocs (spec.md#storybook). One
// story per interesting state from the ticket 04 checklist.
import type { Meta, StoryObj } from "@storybook/vue3-vite";
import VaultPicker from "./VaultPicker.vue";

const meta: Meta<typeof VaultPicker> = {
  component: VaultPicker,
  title: "Surfaces/Vault picker/VaultPicker",
};
export default meta;

type Story = StoryObj<typeof VaultPicker>;

export const Default: Story = {
  args: {
    error: null,
  },
};

export const TranslatedNestedRepoError: Story = {
  args: {
    error:
      'That folder is inside an existing repository. Pick the repository\'s own top-level folder instead: "/home/user/notes".',
  },
};

/** A raw, untranslated error -- includes the Android "no folder picker"
 * rejection (`pick_vault_folder` in lib.rs), shown verbatim since it's
 * already user-facing copy, not a Rust-internal message needing
 * translation. */
export const RawError: Story = {
  args: {
    error: "Selecting a vault folder isn't supported on Android yet (no Storage Access Framework integration -- see pick_vault_folder in lib.rs).",
  },
};
