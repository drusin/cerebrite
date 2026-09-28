<script setup lang="ts">
// Shared component (spec.md#shared-components): the second Vue copy of the
// generate/import SSH key sub-form -- see `DeviceFlow.vue`'s doc comment for
// the same "second copy" reasoning (clone wizard first, connect wizard
// second). Presentational: never imports `vault-api`/`dialogs.ts`/a
// `src/state/` module. The container still owns the actual
// `generateSshKey`/`importSshKey` calls (and, for import, the
// `window.prompt` collection of the private key/passphrase via
// `dialogs.ts`) -- this only renders the two buttons and whatever status
// text the container's own generate/import handlers produced. The caller's
// own primary action button (e.g. "Continue"/"Connect"/"Clone") is not part
// of this component, since its label and enabled-ness vary per caller.
defineProps<{
  /** `null` before Generate/Import has been used yet. */
  status: string | null;
}>();

const emit = defineEmits<{
  generate: [];
  import: [];
}>();
</script>

<template>
  <div class="wizard-button-row">
    <button type="button" @click="emit('generate')">Generate a new key</button>
    <button type="button" @click="emit('import')">Import an existing key</button>
  </div>
  <p v-if="status" class="settings-connect-status">{{ status }}</p>
</template>
