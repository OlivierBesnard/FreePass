import { useState } from "react";
import { ArchiveRestore, KeyRound, Trash2 } from "lucide-react";
import type { EntrySummary } from "../lib/api";
import {
  useArchivedEntries,
  useDeleteEntry,
  useRestoreEntry,
} from "../hooks/useVault";
import { ConfirmDialog } from "./ConfirmDialog";
import { Modal } from "./Modal";

/** "Trash": list archived entries, restore them, or delete permanently. */
export function ArchivedEntries({
  envId,
  onClose,
}: {
  envId: string;
  onClose: () => void;
}) {
  const { data: archived = [], isLoading } = useArchivedEntries(envId);
  const restore = useRestoreEntry(envId);
  const del = useDeleteEntry(envId);
  const [toDelete, setToDelete] = useState<EntrySummary | null>(null);

  return (
    <Modal title="Identifiants archivés" onClose={onClose}>
      {isLoading ? (
        <p className="text-sm text-ink-500">Chargement…</p>
      ) : archived.length === 0 ? (
        <p className="py-6 text-center text-sm text-ink-400">
          Aucun identifiant archivé.
        </p>
      ) : (
        <ul className="max-h-96 space-y-2 overflow-y-auto scrollbar-thin">
          {archived.map((e) => (
            <li
              key={e.id}
              className="flex items-center gap-3 rounded-xl border border-cream-400 bg-card px-3 py-2.5"
            >
              <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-cream-300 text-ink-500">
                <KeyRound size={15} />
              </span>
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm font-medium text-ink-700">
                  {e.title}
                </span>
                {e.url && (
                  <span className="block truncate text-xs text-ink-400">
                    {e.url}
                  </span>
                )}
              </span>
              <button
                onClick={() => restore.mutate(e.id)}
                title="Restaurer"
                aria-label="Restaurer"
                className="inline-flex h-8 items-center gap-1 rounded-lg border border-cream-400 px-2.5 text-xs font-medium text-ink-600 transition-colors hover:bg-cream-300"
              >
                <ArchiveRestore size={14} /> Restaurer
              </button>
              <button
                onClick={() => setToDelete(e)}
                title="Supprimer définitivement"
                aria-label="Supprimer définitivement"
                className="inline-flex h-8 w-8 items-center justify-center rounded-lg border border-cream-400 text-danger-600 transition-colors hover:bg-danger-50"
              >
                <Trash2 size={14} />
              </button>
            </li>
          ))}
        </ul>
      )}

      {toDelete && (
        <ConfirmDialog
          danger
          title="Supprimer définitivement ?"
          message={`« ${toDelete.title} » sera supprimé pour toujours. Cette action est irréversible.`}
          confirmLabel="Supprimer"
          busy={del.isPending}
          onConfirm={() => {
            del.mutate(toDelete.id);
            setToDelete(null);
          }}
          onClose={() => setToDelete(null)}
        />
      )}
    </Modal>
  );
}
