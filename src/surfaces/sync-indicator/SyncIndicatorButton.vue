<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. Renders the two
// places the sync indicator shows up -- the expanded sidebar's footer
// button and the collapsed rail's icon-only button -- reusing the exact
// class names `styles.css` already has for them (no `<style>` block here).
import type { SyncIndicator } from "../../sync-status";

const props = defineProps<{
  indicator: SyncIndicator;
  variant: "footer" | "rail";
  /** Whether the popup this button opens is currently open, for
   * `aria-expanded`. */
  open: boolean;
}>();

const emit = defineEmits<{
  /** The anchor rect the popup should position itself against -- computed
   * here, from the clicked button's own geometry, per
   * spec.md#overlays's "anchor is a `DOMRect`, not an element" rule. */
  toggle: [anchor: DOMRect];
}>();

function handleClick(event: MouseEvent) {
  emit("toggle", (event.currentTarget as HTMLElement).getBoundingClientRect());
}

const statusLabel = () => `Sync status: ${props.indicator.statusText}`;
</script>

<template>
  <button
    v-if="variant === 'footer'"
    class="sync-status-button"
    type="button"
    aria-haspopup="dialog"
    :aria-expanded="open"
    :aria-label="statusLabel()"
    @click="handleClick"
  >
    <span class="sync-status-icon" :data-state="indicator.iconState" aria-hidden="true">{{ indicator.glyph }}</span>
    <span class="sync-status-label">{{ indicator.statusText }}</span>
  </button>
  <button
    v-else
    class="sidebar-rail-button sync-status-button"
    type="button"
    aria-haspopup="dialog"
    :aria-expanded="open"
    :aria-label="statusLabel()"
    :title="statusLabel()"
    @click="handleClick"
  >
    <span class="sync-status-icon" :data-state="indicator.iconState" aria-hidden="true">{{ indicator.glyph }}</span>
  </button>
</template>
