<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports -- even the
// Escape/outside-click handling below is pure DOM + props, replacing
// `init`'s old document-level listeners (spec.md#overlays: "Escape handling
// moves out of `init`'s document listeners and into each surface as that
// surface migrates"). Renders `#sync-popup`'s old markup/classes verbatim.
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { SyncIndicator, SyncCtaId } from "../../sync-status";

const props = defineProps<{
  indicator: SyncIndicator;
  /** `null` when the popup isn't anchored to anything yet -- the container
   * only renders this component while the popup is open, so in practice
   * this is only ever `null` in a story exercising that edge. */
  anchor: DOMRect | null;
  provider: string | null;
  /** Unix seconds, same unit `get_sync_details` reports. */
  lastSyncedAt: number | null;
  syncNowPending: boolean;
}>();

const emit = defineEmits<{
  close: [];
  syncNow: [];
  cta: [id: SyncCtaId];
  settingsLink: [];
}>();

const rootEl = ref<HTMLElement | null>(null);

// Same left/bottom math as the old `openSyncPopup`, which anchored the
// popup just above/beside whichever icon (footer or rail) was clicked.
const positionStyle = computed(() => {
  if (!props.anchor) return {};
  return {
    left: `${Math.round(props.anchor.left)}px`,
    bottom: `${Math.round(window.innerHeight - props.anchor.top + 8)}px`,
    top: "auto",
  };
});

const lastSyncedText = computed(() =>
  props.lastSyncedAt != null ? `Last synced: ${new Date(props.lastSyncedAt * 1000).toLocaleString()}` : null,
);

const severity = computed(() => (props.indicator.severity === "warning" ? "warning" : null));

function handleDocumentClick(event: MouseEvent) {
  if (rootEl.value?.contains(event.target as Node)) return;
  emit("close");
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

onMounted(() => {
  document.addEventListener("click", handleDocumentClick);
  window.addEventListener("keydown", handleKeydown);
});

onUnmounted(() => {
  document.removeEventListener("click", handleDocumentClick);
  window.removeEventListener("keydown", handleKeydown);
});
</script>

<template>
  <div
    ref="rootEl"
    class="sync-popup"
    role="dialog"
    aria-label="Sync status"
    :data-severity="severity"
    :style="positionStyle"
  >
    <p class="sync-popup-status-line">
      <span class="sync-status-icon" :data-state="indicator.iconState" aria-hidden="true">{{ indicator.glyph }}</span>
      <span class="sync-popup-status-text">{{ indicator.statusText }}</span>
    </p>
    <p v-if="provider" class="sync-popup-detail">{{ provider }}</p>
    <p v-if="lastSyncedText" class="sync-popup-detail">{{ lastSyncedText }}</p>
    <div v-if="indicator.ctas.length" class="sync-popup-cause-actions">
      <button v-for="cta in indicator.ctas" :key="cta.id" type="button" @click="emit('cta', cta.id)">
        {{ cta.label }}
      </button>
    </div>
    <div class="sync-popup-actions">
      <button type="button" :disabled="syncNowPending" @click="emit('syncNow')">Sync now</button>
      <button type="button" class="sync-popup-settings-link" @click="emit('settingsLink')">Sync settings…</button>
    </div>
  </div>
</template>
