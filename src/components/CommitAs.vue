<script setup lang="ts">
// Shared component (spec.md#shared-components): the second Vue copy of
// ticket 08's "Commit as" step -- see `DeviceFlow.vue`'s doc comment for the
// same "second copy" reasoning (clone wizard first, connect wizard second;
// Settings' own always-visible "Commit as" section is a third copy, still
// vanilla until ticket 12, and out of scope here). Presentational: never
// imports `vault-api`/`dialogs.ts`/a `src/state/` module -- owns only its
// own name/email input state (spec.md#native-dialogs: "surfaces with their
// own input UI keep doing so"), primed from `prefill` once on mount (callers
// only ever show this component via `v-if`, so a fresh instance is mounted
// each time the step is (re)entered -- no need to re-watch a stable prop).
import { ref } from "vue";

export interface CommitAuthorValue {
  name: string;
  email: string;
}

const props = defineProps<{
  prefill: CommitAuthorValue | null;
  error?: string | null;
  /** Defaults to "Finish" (the wizards' label); Settings' own copy (not
   * migrated by this component) uses "Save" instead. */
  submitLabel?: string;
}>();

const emit = defineEmits<{ submit: [name: string, email: string] }>();

const name = ref(props.prefill?.name ?? "");
const email = ref(props.prefill?.email ?? "");

function submit() {
  emit("submit", name.value.trim(), email.value.trim());
}
</script>

<template>
  <p class="wizard-question">Commit as:</p>
  <input v-model="name" type="text" placeholder="Name" />
  <input v-model="email" type="text" placeholder="Email" />
  <button type="button" class="wizard-primary-action" @click="submit()">{{ submitLabel ?? "Finish" }}</button>
  <p v-if="error" class="error wizard-inline-error">{{ error }}</p>
</template>
