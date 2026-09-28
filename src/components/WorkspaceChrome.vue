<script setup lang="ts">
// Shared/presentational (spec.md#surface-contract): the workspace layout
// and sidebar chrome -- the collapse rail, the compact-mode drawer, and the
// hamburger/overlay that opens it. Ticket 13 (the app shell) extracted this
// out of `App.vue` since it's pure layout, driven entirely by the three
// booleans below (`App.vue` itself owns `COMPACT_MEDIA_QUERY` and the
// `collapsed`/`drawerOpen` refs, the same "container owns state, SFC mounts
// from props alone" split every other surface uses). Never imports
// `vault-api`/`dialogs.ts`/a `src/state/` module: every actual surface
// (Recent, "All pages", Trash, the sync indicator, the article view) is
// slotted in by `App.vue`, so this component never needs to know they
// exist. No `<style>` block -- renders `styles.css`'s existing
// `.workspace`/`.sidebar`/`.sidebar-rail`/etc. class names verbatim.
defineProps<{
  /** Desktop-only: the sidebar is replaced by the narrow icon rail. */
  collapsed: boolean;
  /** Android-proxy layout (see `App.vue`'s `COMPACT_MEDIA_QUERY` doc
   * comment): the sidebar becomes a hamburger-triggered drawer instead of
   * a persistent, collapsible column. */
  compact: boolean;
  /** Compact mode only: whether the drawer is currently open. */
  drawerOpen: boolean;
}>();

const emit = defineEmits<{
  search: [];
  settings: [];
  newPage: [];
  collapse: [];
  expand: [];
  openDrawer: [];
  closeDrawer: [];
}>();
</script>

<template>
  <div class="workspace" :class="{ 'sidebar-collapsed': collapsed, compact }">
    <!-- Android-proxy layout only: hamburger trigger for the hidden-by-
         default drawer. Hidden outright on desktop via CSS. -->
    <button class="sidebar-open-button" type="button" aria-label="Open sidebar" @click="emit('openDrawer')">☰</button>
    <div class="sidebar-overlay" :hidden="!drawerOpen" @click="emit('closeDrawer')"></div>

    <aside class="sidebar" :class="{ 'drawer-open': drawerOpen }">
      <div class="sidebar-top">
        <button class="search-button" type="button" @click="emit('search')">🔍 Search</button>
        <button
          class="sidebar-icon-button"
          type="button"
          aria-label="Settings"
          title="Settings (Ctrl/Cmd+,)"
          @click="emit('settings')"
        >
          ⚙
        </button>
        <button
          class="sidebar-collapse-toggle"
          type="button"
          aria-label="Collapse sidebar"
          title="Collapse sidebar"
          @click="emit('collapse')"
        >
          ←
        </button>
        <button class="sidebar-close-button" type="button" aria-label="Close sidebar" @click="emit('closeDrawer')">
          ✕
        </button>
      </div>

      <slot name="recent" />
      <slot name="pageList" />
      <slot name="trash" />

      <div class="sidebar-footer">
        <slot name="syncFooter" />
      </div>
    </aside>

    <!-- Collapsed-state rail (desktop only): shown in place of the full
         sidebar once it's collapsed, so there's always a visible way back
         in plus quick access to the two most common actions. -->
    <div class="sidebar-rail">
      <button class="sidebar-rail-button" type="button" aria-label="Expand sidebar" title="Expand sidebar" @click="emit('expand')">
        →
      </button>
      <button class="sidebar-rail-button" type="button" aria-label="Search" title="Search" @click="emit('search')">🔍</button>
      <button class="sidebar-rail-button" type="button" aria-label="New page" title="New page" @click="emit('newPage')">+</button>
      <button
        class="sidebar-rail-button"
        type="button"
        aria-label="Settings"
        title="Settings (Ctrl/Cmd+,)"
        @click="emit('settings')"
      >
        ⚙
      </button>
      <slot name="syncRail" />
    </div>

    <slot name="main" />

    <!-- The sync popup and every modal (search/settings/connect wizard) are
         `position: fixed` overlays, so their position in this flex layout
         doesn't matter -- they're slotted in here (rather than as `App.vue`
         siblings of this component) only to preserve the original
         `index.html` nesting: all were inside `#workspace`, so they were
         only ever reachable while the workspace itself was showing (the
         vault picker/clone wizard/clone manual views hide the whole
         workspace subtree, keyboard shortcuts included). -->
    <slot name="overlays" />
  </div>
</template>
