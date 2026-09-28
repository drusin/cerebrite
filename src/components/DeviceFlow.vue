<script setup lang="ts">
// Shared component (spec.md#shared-components): "a shared component is
// extracted when a second copy migrates" -- the clone wizard (ticket 10)
// wrote this inline as the first Vue copy (itself already a second *overall*
// copy of ticket 06/07's raw device-flow rendering, but the first inside
// Vue); the connect wizard (ticket 11) is the second Vue copy, so it's
// extracted here and both the clone wizard and clone manual form switch to
// it. Presentational: never imports `vault-api`/`dialogs.ts`/a `src/state/`
// module, and mounts from props alone -- purely a display of whatever the
// container's own device-flow polling loop (identical in every caller) has
// found so far; there is nothing here for the user to click; the poll loop
// itself stays in each container/composable, not duplicated here.
export interface DeviceCodeDisplay {
  verificationUri: string;
  userCode: string;
}

defineProps<{
  /** e.g. "Requesting a device code from GitHub…", "Waiting for you to
   * approve in the browser…", or an error string once the poll loop fails. */
  status: string;
  /** `null` before the device code has been fetched yet (or once sign-in
   * succeeds/fails and the caller clears it). */
  deviceCode: DeviceCodeDisplay | null;
}>();
</script>

<template>
  <p class="settings-connect-status">{{ status }}</p>
  <div v-if="deviceCode" class="settings-connect-status">
    <p>
      Go to
      <a :href="deviceCode.verificationUri" target="_blank" rel="noopener">{{ deviceCode.verificationUri }}</a>
      and enter code: <strong>{{ deviceCode.userCode }}</strong>
    </p>
  </div>
</template>
