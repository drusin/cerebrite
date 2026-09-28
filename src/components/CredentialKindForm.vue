<script setup lang="ts">
// Shared component (spec.md#shared-components): "a shared component is
// extracted when a second copy migrates" -- the clone manual form (ticket
// 10) wrote the credential-kind selector + its four sub-forms (access
// token, SSH key, GitHub sign-in, GitLab sign-in) inline as the first Vue
// copy; Settings' own always-visible "Sync" section (ticket 12) is the
// second, so it's extracted here and the clone manual form switches to it.
// Presentational: never imports `vault-api`/`dialogs.ts`/a `src/state/`
// module, and mounts from props alone -- reuses `SshKey.vue`/`DeviceFlow.vue`
// (already-extracted shared components) for the SSH key and GitHub/GitLab
// sub-forms.
//
// The two callers differ in one respect: Settings shows one shared
// repository-URL field here (mirroring the vanilla Settings markup this
// replaces) and a per-kind "Connect" action for the access-token/SSH-key
// sub-forms (GitHub/GitLab connect as part of their own sign-in flow, so
// they need no separate action here); the clone manual form already has its
// own single remote-URL field above this component (one URL for whichever
// credential kind the clone ends up using) and defers the actual connect to
// its own outer "Clone" button, so it passes `showUrlField="false"` and
// omits `connectLabel` entirely.
import SshKey from "./SshKey.vue";
import DeviceFlow, { type DeviceCodeDisplay } from "./DeviceFlow.vue";

export type CredentialKindOption = "accessToken" | "sshKey" | "githubOauth" | "gitlabOauth";

const props = withDefaults(
  defineProps<{
    credentialKind: CredentialKindOption;
    /** Whether to render the shared repository-URL field at all -- see this
     * component's doc comment. */
    showUrlField?: boolean;
    remoteUrl?: string;
    tokenUsername: string;
    tokenValue: string;
    /** The access-token sub-form's own status text ("Connecting…"/
     * "Connected."/an error) -- `null` when Settings' own outer
     * `statusMessage` already covers it (e.g. the clone manual form, whose
     * single "Clone" button reports its own status instead). */
    accessTokenStatus?: string | null;
    sshKeyStatus: string | null;
    githubDeviceCode: DeviceCodeDisplay | null;
    githubStatus: string | null;
    gitlabDeviceCode: DeviceCodeDisplay | null;
    gitlabStatus: string | null;
    /** Renders a "Connect" button under the access-token/SSH-key sub-forms
     * when set (Settings' own action label); omitted entirely (no button)
     * when `undefined`, since the clone manual form's outer "Clone" button
     * already covers that action. */
    connectLabel?: string;
    connectDisabled?: boolean;
  }>(),
  { showUrlField: true, remoteUrl: "" },
);

defineEmits<{
  "update:credentialKind": [value: CredentialKindOption];
  "update:remoteUrl": [value: string];
  "update:tokenUsername": [value: string];
  "update:tokenValue": [value: string];
  generateSshKey: [];
  importSshKey: [];
  githubSignIn: [];
  gitlabSignIn: [];
  connect: [];
}>();
</script>

<template>
  <label>
    Credential kind
    <select
      :value="credentialKind"
      @change="$emit('update:credentialKind', ($event.target as HTMLSelectElement).value as CredentialKindOption)"
    >
      <option value="accessToken">Access token</option>
      <option value="sshKey">SSH key</option>
      <option value="githubOauth">GitHub sign-in</option>
      <option value="gitlabOauth">GitLab sign-in</option>
    </select>
  </label>

  <label v-if="showUrlField">
    Repository URL
    <input
      type="text"
      :placeholder="credentialKind === 'sshKey' ? 'git@example.com:user/repo.git' : 'https://example.com/user/repo.git'"
      :value="remoteUrl"
      @input="$emit('update:remoteUrl', ($event.target as HTMLInputElement).value)"
    />
  </label>

  <div v-if="credentialKind === 'accessToken'" class="settings-connect-form">
    <label>
      Username
      <input
        type="text"
        :value="tokenUsername"
        @input="$emit('update:tokenUsername', ($event.target as HTMLInputElement).value)"
      />
    </label>
    <label>
      Access token
      <input
        type="password"
        :value="tokenValue"
        @input="$emit('update:tokenValue', ($event.target as HTMLInputElement).value)"
      />
    </label>
    <button v-if="connectLabel" type="button" :disabled="connectDisabled" @click="$emit('connect')">
      {{ connectLabel }}
    </button>
    <p v-if="accessTokenStatus" class="settings-connect-status">{{ accessTokenStatus }}</p>
  </div>

  <div v-else-if="credentialKind === 'sshKey'" class="settings-connect-form">
    <SshKey :status="sshKeyStatus" @generate="$emit('generateSshKey')" @import="$emit('importSshKey')" />
    <button v-if="connectLabel" type="button" :disabled="connectDisabled" @click="$emit('connect')">
      {{ connectLabel }}
    </button>
  </div>

  <div v-else-if="credentialKind === 'githubOauth'" class="settings-connect-form">
    <button type="button" @click="$emit('githubSignIn')">Sign in with GitHub</button>
    <DeviceFlow v-if="githubStatus" :status="githubStatus" :device-code="githubDeviceCode" />
  </div>

  <div v-else class="settings-connect-form">
    <button type="button" @click="$emit('gitlabSignIn')">Sign in with GitLab</button>
    <DeviceFlow v-if="gitlabStatus" :status="gitlabStatus" :device-code="gitlabDeviceCode" />
  </div>
</template>
