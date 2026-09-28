<script setup lang="ts">
// Presentational surface (spec.md#surface-contract): mounts from props
// alone, no `vault-api`/`dialogs.ts`/`src/state/` imports. A thin wrapper
// around the existing `PageEditor` class -- see spec.md#editor-wrapper-and-
// commit -- which stays completely unchanged (this file is the whole
// glue, same as the prototype's variant A on `prototype/editor-in-vue`).
import { onBeforeUnmount, onMounted, onUnmounted, ref, watch } from "vue";
import { PageEditor } from "../../page-editor";

const props = defineProps<{
  /** Stable identity of the page currently shown -- reloading only
   * happens when *this* changes (see the `watch` below). */
  pageKey: string;
  /** The page's markdown, loaded once on mount and again whenever
   * `pageKey` changes. A `markdown`-only change (e.g. the parent echoing
   * back this wrapper's own `change` event) must NOT reload -- doing so
   * would reset the cursor mid-typing. */
  markdown: string;
}>();

const emit = defineEmits<{
  /** Ordinary edit, debounced by the container. Carries `pageKey` because
   * the parent's current page may already have moved on by the time this
   * is handled. */
  change: [pageKey: string, markdown: string];
  linkClick: [title: string];
  /**
   * The lost-edit fix (spec.md#editor-wrapper-and-commit): Milkdown's
   * `listener` waits ~200ms before calling `markdownUpdated`, and
   * `destroy()` cancels that pending call -- so an edit made in the last
   * ~200ms before a page switch or unmount used to just vanish. This is
   * emitted instead, immediately, with the page being *left* (`oldPageKey`)
   * and its live content, whenever that differs from the last value this
   * wrapper itself emitted. The container saves it with no debounce.
   */
  commit: [oldPageKey: string, markdown: string];
}>();

const root = ref<HTMLDivElement | null>(null);
let editor: PageEditor | null = null;

// The dedupe baseline for `commit`: the last markdown value this wrapper
// itself emitted (via `change` or `commit`), so leaving a page nothing was
// typed into since its last autosave tick doesn't emit a redundant commit.
let lastEmitted = props.markdown;

/** Reads the live editor content and emits `commit` for `oldPageKey` if it moved past `lastEmitted`. */
function commitIfChanged(oldPageKey: string) {
  const current = editor?.getMarkdown();
  if (current !== null && current !== undefined && current !== lastEmitted) {
    lastEmitted = current;
    emit("commit", oldPageKey, current);
  }
}

onMounted(async () => {
  editor = new PageEditor(
    root.value!,
    (markdown) => {
      lastEmitted = markdown;
      emit("change", props.pageKey, markdown);
    },
    (title) => emit("linkClick", title),
  );
  await editor.load(props.markdown);
});

// Only a `pageKey` change reloads -- see the `markdown` prop's doc comment
// above for why a `markdown`-only change must not.
watch(
  () => props.pageKey,
  async (_newKey, oldKey) => {
    commitIfChanged(oldKey);
    lastEmitted = props.markdown;
    await editor?.load(props.markdown);
  },
);

// The other half of the lost-edit fix: the same flush, run once more right
// before the editor (and any pending Milkdown listener callback) is torn
// down for good.
onBeforeUnmount(() => commitIfChanged(props.pageKey));

onUnmounted(() => void editor?.destroy());

// Imperative escape hatch (spec.md#imperative-escape-hatches): stateless,
// one-shot, and the only thing exposed -- `getMarkdown` is read internally
// above, never by callers, since data leaves this wrapper only through
// emits.
defineExpose({
  scrollToHeading: (slug: string) => editor?.scrollToHeading(slug) ?? false,
});
</script>

<template>
  <div ref="root" class="milkdown-root"></div>
</template>
