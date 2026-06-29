import { Modal } from "./Modal";

/** Reusable confirmation dialog (archive, permanent delete, …). */
export function ConfirmDialog({
  title,
  message,
  confirmLabel = "Confirmer",
  danger,
  busy,
  onConfirm,
  onClose,
}: {
  title: string;
  message: string;
  confirmLabel?: string;
  danger?: boolean;
  busy?: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <Modal title={title} onClose={onClose} width="max-w-sm">
      <p className="text-sm text-ink-600">{message}</p>
      <div className="mt-5 flex justify-end gap-2">
        <button
          onClick={onClose}
          className="inline-flex h-9 items-center rounded-lg border border-cream-400 px-3 text-sm font-medium text-ink-600 transition-colors hover:bg-cream-300"
        >
          Annuler
        </button>
        <button
          onClick={onConfirm}
          disabled={busy}
          className={`inline-flex h-9 items-center rounded-lg px-3 text-sm font-medium text-white transition-colors disabled:opacity-50 ${
            danger ? "bg-danger-600 hover:bg-danger-700" : "bg-brand-500 hover:bg-brand-600"
          }`}
        >
          {busy ? "…" : confirmLabel}
        </button>
      </div>
    </Modal>
  );
}
