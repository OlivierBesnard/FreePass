import { toast } from "sonner";

/** Delay before a copied secret is wiped from the clipboard (THREAT F9). */
export const CLIPBOARD_CLEAR_MS = 20_000;

/** Copy a secret, then clear the clipboard after a delay (if still ours). */
export async function copySecret(
  value: string,
  label: string,
  clearMs: number = CLIPBOARD_CLEAR_MS,
) {
  try {
    await navigator.clipboard.writeText(value);
    toast.success(`${label} copié — effacé dans ${Math.round(clearMs / 1000)} s.`);
    window.setTimeout(async () => {
      try {
        let current = value;
        try {
          current = await navigator.clipboard.readText();
        } catch {
          current = value; // read may be denied; clear anyway
        }
        if (current === value) await navigator.clipboard.writeText("");
      } catch {
        // best effort — nothing more we can do
      }
    }, clearMs);
  } catch {
    toast.error("Impossible de copier.");
  }
}

/** Copy non-secret metadata (no auto-clear). */
export async function copyPlain(value: string, label: string) {
  try {
    await navigator.clipboard.writeText(value);
    toast.success(`${label} copié.`);
  } catch {
    toast.error("Impossible de copier.");
  }
}
