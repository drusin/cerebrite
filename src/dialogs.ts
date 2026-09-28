// Every native dialog call in this app goes through this module -- see
// spec.md#native-dialogs. It exists so containers (once surfaces migrate to
// Vue) call one thing instead of importing `@tauri-apps/plugin-dialog` and
// reaching for the DOM globals directly, and so this file is the one place
// that documents which primitive is used for which dialog and why.
//
// `confirm`/`message` come from the dialog plugin, not `window.confirm`/
// `window.alert`: tauri-plugin-dialog's auto-injected webview shim
// (init-iife.js in the 2.7.3 crate) overrides those globals to call a
// `plugin:dialog|confirm` IPC command that plugin version never actually
// registers (only `message` is), so `window.confirm()` always rejects with
// "Command not found" and `window.alert()` fires-and-forgets without
// blocking. The plugin's own JS API calls the correct `plugin:dialog|message`
// command under the hood and actually works.
//
// `window.prompt()` is untouched by that shim and still works natively.
// `window.confirm()` and `window.alert()` are still used in a handful of
// call sites elsewhere in the app (pre-existing; not this step's job to
// fix) -- they're wrapped here unchanged so every native dialog call, buggy
// or not, goes through one module. See each wrapper's doc comment.
import {
  confirm as pluginConfirm,
  message as pluginMessage,
  type ConfirmDialogOptions,
  type MessageDialogOptions,
  type MessageDialogResult,
} from "@tauri-apps/plugin-dialog";

/** The dialog plugin's blocking, native confirm/cancel dialog. Use this, not
 * {@link confirmBrowser}, unless a call site already relies on
 * `window.confirm`'s (broken) behavior. */
export function confirmDialog(message: string, options?: string | ConfirmDialogOptions): Promise<boolean> {
  return pluginConfirm(message, options);
}

/** The dialog plugin's blocking, native message/alert dialog. Use this, not
 * {@link alertBrowser}, unless a call site already relies on
 * `window.alert`'s (broken, non-blocking) behavior. */
export function messageDialog(message: string, options?: string | MessageDialogOptions): Promise<MessageDialogResult> {
  return pluginMessage(message, options);
}

/** `window.prompt`, untouched by the plugin's shim (see module doc comment)
 * and still the only way to collect free text via a native dialog. */
export function promptDialog(message: string, defaultValue?: string): string | null {
  return window.prompt(message, defaultValue);
}

/** `window.confirm`, kept only for existing call sites that already used it
 * -- see the module doc comment for why it's broken under the dialog
 * plugin's shim. Do not use for new call sites; use {@link confirmDialog}. */
export function confirmBrowser(message: string): boolean {
  return window.confirm(message);
}

/** `window.alert`, kept only for existing call sites that already used it
 * -- see the module doc comment for why it's broken (non-blocking) under
 * the dialog plugin's shim. Do not use for new call sites; use
 * {@link messageDialog}. */
export function alertBrowser(message: string): void {
  window.alert(message);
}
