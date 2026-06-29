import { useState } from "react";
import { Copy, Eye, EyeOff, Pencil, Trash2, ExternalLink } from "lucide-react";
import type { EntryDetail as Entry } from "../lib/api";
import { useArchiveEntry, useEntry } from "../hooks/useVault";
import { copyPlain, copySecret } from "../lib/clipboard";
import { openExternal } from "../lib/openExternal";
import { ConfirmDialog } from "./ConfirmDialog";
import { Modal } from "./Modal";

/** Read view for one entry: reveal/copy fields, edit, or archive. */
export function EntryDetailView({
  envId,
  entryId,
  onClose,
  onEdit,
}: {
  envId: string;
  entryId: string;
  onClose: () => void;
  onEdit: (entry: Entry) => void;
}) {
  const { data: entry, isLoading } = useEntry(envId, entryId);
  const archive = useArchiveEntry(envId);
  const [reveal, setReveal] = useState(false);
  const [confirmArchive, setConfirmArchive] = useState(false);

  return (
    <Modal title={entry?.title ?? "Identifiant"} onClose={onClose}>
      {isLoading || !entry ? (
        <p className="text-sm text-ink-500">Déchiffrement…</p>
      ) : (
        <div className="space-y-4">
          {entry.icon && (
            <div className="flex justify-center">
              <span className="flex h-12 w-12 items-center justify-center overflow-hidden rounded-xl border border-cream-400 bg-card">
                <img src={entry.icon} alt="" className="h-7 w-7 object-contain" />
              </span>
            </div>
          )}
          {entry.url && (
            <Row label="Site web">
              <button
                onClick={() => openExternal(entry.url!)}
                title="Ouvrir dans le navigateur"
                className="inline-flex max-w-full items-center gap-1 text-left text-brand-700 hover:underline"
              >
                <span className="truncate">{entry.url}</span>
                <ExternalLink size={13} className="shrink-0" />
              </button>
            </Row>
          )}
          {entry.username && (
            <Row label="Identifiant" onCopy={() => copyPlain(entry.username!, "Identifiant")}>
              <span className="text-ink-800">{entry.username}</span>
            </Row>
          )}
          {entry.password && (
            <Row
              label="Mot de passe"
              onCopy={() => copySecret(entry.password!, "Mot de passe")}
              extra={
                <button
                  onClick={() => setReveal((v) => !v)}
                  className="rounded p-1 text-ink-400 hover:bg-cream-300 hover:text-ink-700"
                  aria-label={reveal ? "Masquer" : "Afficher"}
                >
                  {reveal ? <EyeOff size={15} /> : <Eye size={15} />}
                </button>
              }
            >
              <span className="font-mono text-ink-800">
                {reveal ? entry.password : "••••••••••••"}
              </span>
            </Row>
          )}
          {entry.notes && (
            <Row label="Notes">
              <p className="whitespace-pre-wrap text-ink-700">{entry.notes}</p>
            </Row>
          )}

          <div className="flex justify-end gap-2 border-t border-cream-400 pt-4">
            <button
              onClick={() => setConfirmArchive(true)}
              className="inline-flex h-9 items-center gap-1.5 rounded-lg border border-cream-400 px-3 text-sm font-medium text-danger-600 transition-colors hover:bg-danger-50"
            >
              <Trash2 size={15} /> Archiver
            </button>
            <button
              onClick={() => onEdit(entry)}
              className="inline-flex h-9 items-center gap-1.5 rounded-lg bg-brand-500 px-3 text-sm font-medium text-white transition-colors hover:bg-brand-600"
            >
              <Pencil size={15} /> Modifier
            </button>
          </div>

          {confirmArchive && (
            <ConfirmDialog
              title="Archiver cet identifiant ?"
              message={`« ${entry.title} » sera déplacé dans les archivés. Vous pourrez le restaurer ou le supprimer définitivement.`}
              confirmLabel="Archiver"
              busy={archive.isPending}
              onConfirm={() => {
                archive.mutate(entry.id);
                setConfirmArchive(false);
                onClose();
              }}
              onClose={() => setConfirmArchive(false)}
            />
          )}
        </div>
      )}
    </Modal>
  );
}

function Row({
  label,
  children,
  onCopy,
  extra,
}: {
  label: string;
  children: React.ReactNode;
  onCopy?: () => void;
  extra?: React.ReactNode;
}) {
  return (
    <div>
      <div className="mb-0.5 flex items-center justify-between">
        <span className="text-xs font-medium uppercase tracking-wide text-ink-400">
          {label}
        </span>
        <div className="flex items-center gap-0.5">
          {extra}
          {onCopy && (
            <button
              onClick={onCopy}
              className="rounded p-1 text-ink-400 hover:bg-cream-300 hover:text-ink-700"
              aria-label="Copier"
            >
              <Copy size={15} />
            </button>
          )}
        </div>
      </div>
      <div className="text-sm">{children}</div>
    </div>
  );
}
