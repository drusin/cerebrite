import { openUrl } from "@tauri-apps/plugin-opener";

/** Opens `url` in the system browser. Best effort: callers always also show
 * the URL as text, so a failure to launch a browser is not an error. */
export async function openInBrowser(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch {
    // The user can open the URL by hand.
  }
}
