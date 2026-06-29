import { useEffect, useState } from "react";
import { Copy, Download, Puzzle } from "lucide-react";
import { toast } from "sonner";
import { api, type ChannelInfo } from "../lib/api";
import { openExternal } from "../lib/openExternal";
import { Modal } from "./Modal";

const EXTENSION_URL = "https://github.com/OlivierBesnard/FreePass/tree/main/extension";

async function copy(value: string, label: string) {
  try {
    await navigator.clipboard.writeText(value);
    toast.success(`${label} copié.`);
  } catch {
    toast.error("Impossible de copier.");
  }
}

/** Explains how to install the extension and connect it to FreePass (DESIGN §7). */
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
        <div className="flex items-start gap-2 text-sm text-ink-600">
          <Puzzle size={18} className="mt-0.5 shrink-0 text-brand-600" />
          <p>
            L'extension pré-remplit vos identifiants en parlant à FreePass via un
            canal <strong>local uniquement</strong> (127.0.0.1). Trois étapes :
          </p>
        </div>

        <ol className="space-y-3">
          <Step n={1} title="Installer l'extension dans votre navigateur">
            <button
              onClick={() => openExternal(EXTENSION_URL)}
              className="inline-flex h-8 items-center gap-1.5 rounded-lg bg-brand-500 px-3 text-xs font-medium text-white transition-colors hover:bg-brand-600"
            >
              <Download size={14} /> Obtenir l'extension
            </button>
            <p className="mt-1.5 text-xs text-ink-400">
              Chrome/Edge : <span className="font-mono">chrome://extensions</span> →
              mode développeur → « Charger l'extension non empaquetée » → dossier{" "}
              <span className="font-mono">extension/</span>. Firefox :{" "}
              <span className="font-mono">about:debugging</span> → « Charger un
              module temporaire ». (Le lien ouvre le guide complet.)
            </p>
          </Step>

          <Step n={2} title="Coller ces valeurs dans le popup de l'extension">
            {info === undefined && (
              <p className="text-sm text-ink-500">Chargement…</p>
            )}
            {info === null && (
              <p className="text-sm text-danger-600">
                Le canal n'est pas démarré — déverrouillez le coffre.
              </p>
            )}
            {info && (
              <div className="space-y-2">
                <PairRow label="Port" value={String(info.port)} />
                <PairRow label="Token d'appairage" value={info.token} mono />
              </div>
            )}
          </Step>

          <Step n={3} title="Remplir sur un site">
            <p className="text-xs text-ink-400">
              Sur une page de connexion, ouvrez l'extension : elle liste les
              identifiants du site → cliquez <strong>Remplir</strong>.
            </p>
          </Step>
        </ol>

        <p className="text-xs text-ink-400">
          Le token est une <strong>clé d'accès au canal</strong>, pas votre mot de
          passe maître : il ne déverrouille pas le coffre, ne sort jamais de la
          machine, et change à chaque déverrouillage (ré-appairez si besoin).
        </p>
      </div>
    </Modal>
  );
}

function Step({
  n,
  title,
  children,
}: {
  n: number;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <li className="flex gap-3">
      <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-brand-100 text-xs font-semibold text-brand-700">
        {n}
      </span>
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-ink-700">{title}</p>
        <div className="mt-1">{children}</div>
      </div>
    </li>
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
      <span className="w-28 shrink-0 text-xs font-medium uppercase tracking-wide text-ink-400">
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
