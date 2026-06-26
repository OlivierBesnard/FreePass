import { useEffect, useState } from "react";
import { Copy, Puzzle } from "lucide-react";
import { toast } from "sonner";
import { api, type ChannelInfo } from "../lib/api";
import { Modal } from "./Modal";

async function copy(value: string, label: string) {
  try {
    await navigator.clipboard.writeText(value);
    toast.success(`${label} copié.`);
  } catch {
    toast.error("Impossible de copier.");
  }
}

/** Shows the loopback pairing details the browser extension needs (DESIGN §7). */
export function ExtensionPairing({ onClose }: { onClose: () => void }) {
  const [info, setInfo] = useState<ChannelInfo | null | undefined>(undefined);

  useEffect(() => {
    api
      .localChannelInfo()
      .then(setInfo)
      .catch(() => setInfo(null));
  }, []);

  return (
    <Modal title="Connecter l'extension" onClose={onClose}>
      <div className="space-y-4">
        <div className="flex items-center gap-2 text-sm text-ink-600">
          <Puzzle size={18} className="text-brand-600" />
          <p>
            L'extension FreePass se connecte à ce coffre via un canal{" "}
            <strong>local uniquement</strong> (127.0.0.1). Renseignez ces valeurs
            dans l'extension pour l'appairer.
          </p>
        </div>

        {info === undefined && <p className="text-sm text-ink-500">Chargement…</p>}
        {info === null && (
          <p className="text-sm text-danger-600">
            Le canal n'est pas démarré. Déverrouillez le coffre.
          </p>
        )}
        {info && (
          <div className="space-y-2">
            <PairRow label="Port" value={String(info.port)} />
            <PairRow label="Token d'appairage" value={info.token} mono />
          </div>
        )}

        <p className="text-xs text-ink-400">
          Le token est une <strong>clé d'accès au canal</strong>, pas votre mot de
          passe maître : il ne déverrouille pas le coffre et ne sort jamais de la
          machine. Il change à chaque déverrouillage.
        </p>
      </div>
    </Modal>
  );
}

function PairRow({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-cream-400 bg-cream-200/60 px-3 py-2">
      <span className="w-32 shrink-0 text-xs font-medium uppercase tracking-wide text-ink-400">
        {label}
      </span>
      <span className={`min-w-0 flex-1 truncate text-sm text-ink-800 ${mono ? "font-mono" : ""}`}>
        {value}
      </span>
      <button
        onClick={() => copy(value, label)}
        className="rounded p-1 text-ink-400 hover:bg-cream-300 hover:text-ink-700"
        aria-label="Copier"
      >
        <Copy size={15} />
      </button>
    </div>
  );
}
