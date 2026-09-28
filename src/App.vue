<script setup lang="ts">
// The whole frontend is one Vue app (spec.md#step-12-app-shell). This is
// the last piece of vanilla DOM/event wiring left: the `vaultView` switch,
// the workspace layout and sidebar chrome (delegated to
// `components/WorkspaceChrome.vue`, a presentational component), and the
// modal host. Every `mountIsland` call in the old `main.ts` is now a
// template usage below; `mountIsland`/`main.ts` are both gone (see
// `index.html`'s doc comment for the new bootstrap).
//
// This container is not storied (spec.md#surface-contract: containers
// aren't storied) -- if `WorkspaceChrome.vue`'s own layout needs checking
// in isolation, its stories cover that.
import { onMounted, onUnmounted, ref, watch } from "vue";
import { vaultView, modal, openSearchModal, openSettings, closeModal } from "./state/ui";
import { initTheme } from "./state/theme";
import { refreshSyncStatus } from "./state/sync";
import { registerArticleHandle } from "./state/navigation";

import WorkspaceChrome from "./components/WorkspaceChrome.vue";
import VaultPickerContainer from "./surfaces/vault-picker/VaultPickerContainer.vue";
import CloneWizardContainer from "./surfaces/clone-wizard/CloneWizardContainer.vue";
import CloneManualFormContainer from "./surfaces/clone-manual-form/CloneManualFormContainer.vue";
import ConnectWizardContainer from "./surfaces/connect-wizard/ConnectWizardContainer.vue";
import SettingsContainer from "./surfaces/settings/SettingsContainer.vue";
import SearchModalContainer from "./surfaces/search/SearchModalContainer.vue";
import SyncIndicatorContainer from "./surfaces/sync-indicator/SyncIndicatorContainer.vue";
import SyncPopupContainer from "./surfaces/sync-indicator/SyncPopupContainer.vue";
import ArticleContainer from "./surfaces/article/ArticleContainer.vue";
import RecentContainer from "./surfaces/recent/RecentContainer.vue";
import PageListContainer from "./surfaces/page-list/PageListContainer.vue";
import TrashContainer from "./surfaces/trash/TrashContainer.vue";

// --- Imperative escape hatches (spec.md#imperative-escape-hatches) --------
//
// The article container's `scrollToHeading` is registered with
// `state/navigation.ts` so the search modal (the one remaining caller with
// a heading target) can reach it without a callback prop -- replaces the
// old `main.ts`'s own `articleHandle` variable.
const articleRef = ref<InstanceType<typeof ArticleContainer> | null>(null);
watch(articleRef, (handle) => registerArticleHandle(handle));

// The still-vanilla sidebar rail's own "+" icon used to reach
// `pageListHandle.newPage()` this same way; now `WorkspaceChrome`'s own
// `@new-page` emits straight into this.
const pageListRef = ref<InstanceType<typeof PageListContainer> | null>(null);

// --- Sidebar chrome state ---------------------------------------------------
//
// Replaces `main.ts`'s own DOM `classList` toggling
// (`sidebar-collapsed`/`compact`/`drawer-open`) with plain local refs, fed
// into `WorkspaceChrome`'s props -- this container's state is local, same
// as the search modal's own query/debounce state, since nothing else in
// the app needs to read it.
const collapsed = ref(false);
const drawerOpen = ref(false);

/**
 * Compact-mode layout (issue 12): Desktop (Windows/Linux) gets a persistent,
 * user-collapsible sidebar; Android gets a hamburger-triggered drawer,
 * hidden by default. This project has no runtime OS-detection yet, so
 * rather than build one for an MVP milestone with no Android device to test
 * on, both behaviors are driven by one boolean -- "compact mode" -- and a
 * viewport-width media query stands in as a *proxy* for "phone-sized
 * screen." This is explicitly a proxy, not real platform detection: it will
 * also fire on a narrow desktop window. Ticket 15 (actual Android bring-up)
 * is expected to replace this with a real platform check (e.g.
 * `@tauri-apps/plugin-os`) if the proxy proves wrong in practice.
 */
const COMPACT_MEDIA_QUERY = window.matchMedia("(max-width: 700px)");
const compact = ref(COMPACT_MEDIA_QUERY.matches);

function applyLayoutMode() {
  compact.value = COMPACT_MEDIA_QUERY.matches;
  // Neither mode's "open" state should leak into the other when the
  // viewport crosses the threshold (e.g. resizing a window).
  drawerOpen.value = false;
}

// --- Global keyboard shortcuts ----------------------------------------------
//
// Ctrl/Cmd+K (search) and Ctrl/Cmd+, (settings) call `state/ui.ts` actions
// directly -- no DOM lookups, replacing `main.ts`'s old `init`'s
// `window.addEventListener("keydown", ...)`. Escape is handled per-surface
// already (`components/Modal.vue`, `SyncPopup.vue`, the wizards' own
// handlers), so there is nothing left to move for it.
function handleKeydown(event: KeyboardEvent) {
  const isSearchShortcut = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k";
  const isSettingsShortcut = (event.ctrlKey || event.metaKey) && event.key === ",";
  if (isSearchShortcut) {
    event.preventDefault();
    openSearchModal();
  } else if (isSettingsShortcut) {
    event.preventDefault();
    if (modal.value === "settings") closeModal();
    else openSettings();
  }
}

onMounted(async () => {
  window.addEventListener("keydown", handleKeydown);
  COMPACT_MEDIA_QUERY.addEventListener("change", applyLayoutMode);
  void refreshSyncStatus();
  await initTheme();
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeydown);
  COMPACT_MEDIA_QUERY.removeEventListener("change", applyLayoutMode);
});
</script>

<template>
  <VaultPickerContainer />
  <CloneWizardContainer />
  <CloneManualFormContainer />

  <WorkspaceChrome
    v-if="vaultView === 'workspace'"
    :collapsed="collapsed"
    :compact="compact"
    :drawer-open="drawerOpen"
    @search="openSearchModal()"
    @settings="openSettings()"
    @new-page="pageListRef?.newPage()"
    @collapse="collapsed = true"
    @expand="collapsed = false"
    @open-drawer="drawerOpen = true"
    @close-drawer="drawerOpen = false"
  >
    <template #recent><RecentContainer /></template>
    <template #pageList><PageListContainer ref="pageListRef" /></template>
    <template #trash><TrashContainer /></template>
    <template #syncFooter><SyncIndicatorContainer variant="footer" /></template>
    <template #syncRail><SyncIndicatorContainer variant="rail" /></template>
    <template #main><ArticleContainer ref="articleRef" /></template>
    <template #overlays>
      <SyncPopupContainer />
      <SearchModalContainer />
      <SettingsContainer />
      <ConnectWizardContainer />
    </template>
  </WorkspaceChrome>
</template>
