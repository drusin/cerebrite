// Ticket 02 (the pilot): first cut of the view/modal state module -- see
// spec.md#view-and-modal-state-stateuits. This cut only has what the pilot
// surface needs: the sync popup's open/closed state (tracked separately
// from `modal`, since the popup is anchored rather than modal -- there is
// no `modal` value here yet, later steps add it) and the `openSettings`
// deep-link action. Later steps extend this module; they don't replace this
// shape.
import { readonly, ref, shallowRef, type Ref, type ShallowRef } from "vue";
import type { CredentialKind } from "../vault-api";

// --- Sync popup open/closed state -------------------------------------------
//
// Replaces main.ts's own `isSyncPopupOpen`/`openSyncPopup`/`closeSyncPopup`/
// `toggleSyncPopup`, which tracked openness via `#sync-popup`'s `hidden`
// attribute and positioned it from whichever button element was clicked.
// The anchor is now data (a `DOMRect` snapshot), not an element reference,
// per spec.md#overlays's anchored-popup rule.

const syncPopupOpenState: Ref<boolean> = ref(false);
const syncPopupAnchorState: ShallowRef<DOMRect | null> = shallowRef(null);

export const syncPopupOpen = readonly(syncPopupOpenState);
export const syncPopupAnchor = readonly(syncPopupAnchorState);

export function openSyncPopup(anchor: DOMRect): void {
  syncPopupAnchorState.value = anchor;
  syncPopupOpenState.value = true;
}

export function closeSyncPopup(): void {
  syncPopupOpenState.value = false;
}

export function toggleSyncPopup(anchor: DOMRect): void {
  if (syncPopupOpenState.value) {
    closeSyncPopup();
  } else {
    openSyncPopup(anchor);
  }
}

// --- `openSettings` deep-link action ----------------------------------------
//
// Replaces `openSyncManualForm`/`openQuickReconnect` writing directly into
// Settings' DOM (per spec.md#view-and-modal-state-stateuits). Every caller
// that used to reach into Settings now calls this instead.

export interface OpenSettingsOptions {
  section?: "sync";
  /** A repository URL already known (e.g. from a wizard, or from
   * `get_sync_details`) to prefill the sync section's URL fields with. */
  prefillUrl?: string;
  /** Which credential-kind sub-form to preselect, when known. */
  credentialKind?: CredentialKind;
  /** The connection's provider label (e.g. `"GitHub"`/`"GitLab"`, as
   * `SyncDetails.provider` reports it) -- needed alongside `credentialKind`
   * to disambiguate the GitHub/GitLab OAuth sub-forms, since a bare
   * `"oauth_sign_in"` doesn't say which. Not in the ticket's illustrative
   * `openSettings({ section, prefillUrl, credentialKind })` signature, but
   * required to preserve `openQuickReconnect`'s existing behavior. */
  provider?: string | null;
}

export type OpenSettingsHandler = (options: OpenSettingsOptions) => void;

// Temporary callback root prop (spec.md#islands-and-how-they-merge):
// Settings hasn't migrated to Vue yet, so the actual modal-opening/
// prefilling/scrolling logic still lives in main.ts. It's registered here
// by the sync popup's container, which receives it as a root prop
// (`openSettingsInVanilla`) from `mountIsland`. `openSettings` itself stays
// framework/DOM-agnostic; only the registered handler touches the vanilla
// Settings DOM. Delete this indirection -- and call `openSettingsHandler`
// directly -- once Settings itself migrates to Vue and can just read/act on
// state the way every other migrated surface does.
let openSettingsHandler: OpenSettingsHandler | null = null;

export function registerOpenSettingsHandler(handler: OpenSettingsHandler): void {
  openSettingsHandler = handler;
}

/** Deep-link action: opens Settings, optionally jumping straight to a
 * section and prefilling it. Also closes the sync popup, since every
 * current caller reaches this from there or wants it closed regardless. */
export function openSettings(options: OpenSettingsOptions = {}): void {
  closeSyncPopup();
  openSettingsHandler?.(options);
}

// --- Modal state -------------------------------------------------------
//
// Replaces main.ts's own `#search-modal-overlay` `hidden`-attribute
// toggling (`isSearchModalOpen`/`openSearchModal`/`closeSearchModal`). Per
// spec.md#view-and-modal-state-stateuits, `modal` is eventually
// `null | 'search' | 'settings' | 'connectWizard'`; this step (03) is the
// first real use of the field, so only `'search'` exists so far -- later
// steps extend the union as Settings and the connect wizard migrate. The
// sync popup stays on its own `syncPopupOpen`/`syncPopupAnchor` state above,
// since it's anchored rather than modal.

export type ModalId = "search";

const modalState: Ref<ModalId | null> = ref(null);

export const modal = readonly(modalState);

export function openSearchModal(): void {
  modalState.value = "search";
}

export function closeModal(): void {
  modalState.value = null;
}

// --- Vault view state ----------------------------------------------------
//
// Replaces main.ts's own `showVaultPicker()`/`showWorkspace()` -- which
// toggled `#vault-picker`'s and `#workspace`'s `hidden` attributes directly
// -- with one piece of state every vault-related surface can read.
// `"cloneWizard"`/`"cloneManual"` are introduced now (per spec.md#step-3-
// vault-picker) but aren't fully wired up yet: the guided clone wizard (a
// full-screen takeover that replaces the picker) does call `setVaultView`
// so the Vue picker hides itself while it's open, but the standalone manual
// form (a modal overlay *on top of* the picker, same as before) doesn't --
// both stay driven by their own still-vanilla overlay show/hide otherwise,
// until they migrate onto this state in step 9.
export type VaultView = "picker" | "cloneWizard" | "cloneManual" | "workspace";

const vaultViewState: Ref<VaultView> = ref("picker");

export const vaultView = readonly(vaultViewState);

export function setVaultView(view: VaultView): void {
  vaultViewState.value = view;
}
