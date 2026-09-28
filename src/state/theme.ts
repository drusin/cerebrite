// Ticket 12: the color-scheme ("theme") state, moved out of main.ts into
// `src/state/` -- see spec.md#step-11-settings ("color scheme (theme moves
// into `src/state/`)"). Plain `ref()` plus actions, same shape as every
// other module here. Replaces main.ts's own module-scope `applyTheme`/
// `handleThemeRadioChange`/`setThemeRadioValue`, which wrote directly into
// the (now-removed) vanilla Settings theme radios.
import { readonly, ref, type Ref } from "vue";
import { getSettings, setTheme as setThemeCommand, type Theme } from "../vault-api";

const themeState: Ref<Theme> = ref("system");

/** Read-only outside this module. */
export const theme = readonly(themeState);

/** Applies `next` to the page: "system" defers to `prefers-color-scheme` (no
 * attribute), "light"/"dark" force it via `:root[data-theme]` overrides in
 * styles.css -- unchanged from main.ts's old module-scope `applyTheme`. */
function applyThemeToDocument(next: Theme): void {
  if (next === "system") {
    document.documentElement.removeAttribute("data-theme");
  } else {
    document.documentElement.setAttribute("data-theme", next);
  }
}

/** Loads the persisted theme once at startup and applies it -- replaces
 * main.ts's own `init()` sequence (`getSettings()` -> `applyTheme` ->
 * `setThemeRadioValue`, the last of which no longer applies now that the
 * Settings surface just reads {@link theme} reactively). */
export async function initTheme(): Promise<void> {
  const settings = await getSettings();
  themeState.value = settings.theme;
  applyThemeToDocument(settings.theme);
}

/** Settings' color-scheme action: applies `next` immediately (so switching
 * feels instant, same as before), then persists it -- a persist failure is
 * only logged, exactly like the old `handleThemeRadioChange`, since the
 * theme has already visibly applied either way. */
export async function setTheme(next: Theme): Promise<void> {
  themeState.value = next;
  applyThemeToDocument(next);
  try {
    await setThemeCommand(next);
  } catch (err) {
    console.error("Failed to persist theme", err);
  }
}
