import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Download, X } from "lucide-react";

/**
 * Checks GitHub Releases for a signed update on startup. If one exists, shows a
 * banner; the user installs in-app (download → install → relaunch) without ever
 * visiting GitHub. Signature is verified by the Tauri updater (THREAT F15).
 */
export function UpdateBanner() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [busy, setBusy] = useState(false);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const found = await check();
        if (!cancelled && found) setUpdate(found);
      } catch {
        // No updater in dev, offline, or no release yet — stay silent.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (!update || dismissed) return null;

  async function install() {
    if (!update) return;
    setBusy(true);
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch {
      setBusy(false);
    }
  }

  return (
    <div className="flex items-center gap-3 border-b border-brand-300 bg-brand-100 px-4 py-2 text-sm text-brand-700">
      <Download size={16} className="shrink-0" />
      <span className="flex-1">
        Nouvelle version <strong>{update.version}</strong> disponible.
      </span>
      <button
        onClick={install}
        disabled={busy}
        className="inline-flex h-8 items-center rounded-lg bg-brand-500 px-3 text-xs font-medium text-white transition-colors hover:bg-brand-600 disabled:opacity-50"
      >
        {busy ? "Installation…" : "Mettre à jour"}
      </button>
      <button
        onClick={() => setDismissed(true)}
        className="rounded p-1 text-brand-700/70 hover:bg-brand-200"
        aria-label="Plus tard"
      >
        <X size={15} />
      </button>
    </div>
  );
}
