import { useEffect, useRef } from "react";
import { X } from "lucide-react";

/**
 * Shared stack of open modals so Escape only closes the TOPMOST one (B3). Each
 * Modal registers a unique id on mount and removes it on unmount.
 */
const modalStack: symbol[] = [];

/** True while at least one Modal is mounted — lets callers suppress global
 *  shortcuts (e.g. ⌘K) that would otherwise fire behind an open dialog (B3). */
export function isModalOpen(): boolean {
  return modalStack.length > 0;
}

/** Minimal modal: dimmed overlay, Escape (topmost only) + overlay-click to close. */
export function Modal({
  title,
  onClose,
  children,
  width = "max-w-lg",
}: {
  title: string;
  onClose: () => void;
  children: React.ReactNode;
  width?: string;
}) {
  const idRef = useRef<symbol | null>(null);
  if (idRef.current === null) idRef.current = Symbol("modal");
  // Track where a mousedown began so a drag that ends on the overlay (text
  // selection overflowing the dialog) doesn't close the modal and lose input (B13).
  const downOnOverlay = useRef(false);

  useEffect(() => {
    const id = idRef.current!;
    modalStack.push(id);
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // Only the topmost modal reacts to Escape; parents stay open.
      if (modalStack[modalStack.length - 1] !== id) return;
      e.stopPropagation();
      onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      const i = modalStack.lastIndexOf(id);
      if (i !== -1) modalStack.splice(i, 1);
    };
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center overflow-y-auto bg-ink-900/30 p-4 backdrop-blur-sm"
      onMouseDown={(e) => {
        downOnOverlay.current = e.target === e.currentTarget;
      }}
      onMouseUp={(e) => {
        if (downOnOverlay.current && e.target === e.currentTarget) onClose();
        downOnOverlay.current = false;
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className={`anim-fade-in flex max-h-[calc(100vh-2rem)] w-full ${width} flex-col rounded-2xl border bg-card shadow-pop`}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex shrink-0 items-center justify-between border-b border-cream-400 px-5 py-3.5">
          <h2 className="font-serif text-lg font-semibold text-ink-800">
            {title}
          </h2>
          <button
            onClick={onClose}
            className="rounded-lg p-1 text-ink-400 transition-colors hover:bg-cream-300 hover:text-ink-700"
            aria-label="Fermer"
          >
            <X size={18} />
          </button>
        </div>
        <div className="overflow-y-auto p-5">{children}</div>
      </div>
    </div>
  );
}
