<script setup lang="ts">
// Shared component (spec.md#shared-components): "a shared component is
// extracted when a second copy migrates" -- the search modal (ticket 03)
// wrote its overlay/backdrop inline as the first modal (there was no
// `<Modal>` yet); the connect wizard (ticket 11) is the second, so the
// shared overlay+backdrop+Escape shell is extracted here, and search
// switches to it too. Presentational: never imports `vault-api`/
// `dialogs.ts`/a `src/state/` module, and mounts from props alone.
//
// Deliberately thin: it owns only the overlay/backdrop/Escape/dialog-role
// mechanics common to every modal so far, plus the optional corner close
// button (`closable`) -- not layout, not a title bar, not footer buttons,
// since those already differ per caller (the search
// modal's "header" is its search input row, not a title; the connect
// wizard has its own header + footer). Existing class names are passed in
// as props rather than hard-coded, since today's callers use two different
// pairs (`search-modal-overlay`/`search-modal` and `settings-modal-overlay`/
// `settings-modal`) and this step doesn't rename either (spec.md#no-style-
// block: renders the existing `styles.css` class names verbatim).
import { ref } from "vue";

withDefaults(defineProps<{
  /** Rendered as the dialog's `aria-label`. Named `label`, not `ariaLabel`,
   * because Vue's template compiler deliberately never camelizes `aria-*`
   * attributes to match a prop (they're passed through as real DOM
   * attributes instead) -- so a prop actually named `ariaLabel` could only
   * ever be set via `:aria-label`, not the plain `aria-label="…"` every
   * caller here already writes. */
  label: string;
  overlayClass: string;
  cardClass: string;
  /** Renders a ✕ pinned to the card's top-right corner (it stays put while
   * a tall card scrolls). Meant for multi-step flows and forms; transient
   * popups like search close on Escape/backdrop alone. */
  closable?: boolean;
}>(), { closable: false });

const emit = defineEmits<{ close: [] }>();

const rootEl = ref<HTMLElement | null>(null);

function handleBackdropClick(event: MouseEvent) {
  if (event.target === rootEl.value) emit("close");
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    emit("close");
  }
}
</script>

<template>
  <div ref="rootEl" :class="overlayClass" @click="handleBackdropClick" @keydown="handleKeydown">
    <div :class="cardClass" role="dialog" aria-modal="true" :aria-label="label">
      <button v-if="closable" type="button" class="modal-close-button" aria-label="Close" @click="emit('close')">
        ✕
      </button>
      <slot />
    </div>
  </div>
</template>
