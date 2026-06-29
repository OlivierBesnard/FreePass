import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * Open a URL in the user's default browser. In a Tauri webview a plain anchor
 * does not reach the real browser — the opener plugin does. Bare hosts get an
 * https:// prefix.
 */
export async function openExternal(url: string) {
  const full = /^https?:\/\//i.test(url) ? url : `https://${url}`;
  try {
    await openUrl(full);
  } catch {
    // ignore — nothing actionable for the user
  }
}
