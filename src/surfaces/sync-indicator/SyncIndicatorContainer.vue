<script setup lang="ts">
// Container (spec.md#surface-contract): owns the state read and the popup
// toggle action; no story (containers aren't storied). One of these is
// mounted per indicator location (footer + rail), both driven by the same
// shared `state/sync.ts` status and `state/ui.ts` popup state.
import { computed } from "vue";
import { syncStatus } from "../../state/sync";
import { syncPopupOpen, toggleSyncPopup } from "../../state/ui";
import { syncIndicatorFor } from "../../sync-status";
import SyncIndicatorButton from "./SyncIndicatorButton.vue";

const props = defineProps<{
  variant: "footer" | "rail";
}>();

const indicator = computed(() => syncIndicatorFor(syncStatus.value));
</script>

<template>
  <SyncIndicatorButton
    :indicator="indicator"
    :variant="props.variant"
    :open="syncPopupOpen"
    @toggle="toggleSyncPopup($event)"
  />
</template>
