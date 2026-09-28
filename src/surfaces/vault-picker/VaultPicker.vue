<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. Renders
// `#vault-picker`'s old markup/classes verbatim (spec.md#step-3-vault-
// picker) -- `error` is already display-ready text (the container does any
// translation, e.g. the nested-repo message) or `null` for none.
defineProps<{
  error: string | null;
}>();

const emit = defineEmits<{
  select: [];
  /** Opens the full-screen guided clone wizard (`surfaces/clone-wizard/`). */
  openCloneWizard: [];
  /** Opens the standalone "git clone" manual form
   * (`surfaces/clone-manual-form/`). */
  openCloneManual: [];
}>();
</script>

<template>
  <section class="vault-picker">
    <h1>Cerebrite</h1>
    <p>Select a folder to use as your vault. It doesn't need to be a git repo yet.</p>
    <button type="button" @click="emit('select')">Select vault folder…</button>
    <p class="vault-picker-alt">
      Setting up a new device?
      <button class="link-button" type="button" @click="emit('openCloneWizard')">I already have a repository…</button>
    </p>
    <p class="vault-picker-alt">
      <button class="link-button" type="button" @click="emit('openCloneManual')">Or clone with raw git fields…</button>
    </p>
    <p v-if="error" class="error">{{ error }}</p>
  </section>
</template>
