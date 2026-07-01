import { useEffect } from "react";
import { X } from "lucide-react";

/** Minimal modal: dimmed overlay, Escape + overlay-click to close. */
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
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center overflow-y-auto bg-ink-900/30 p-4 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className={`anim-fade-in flex max-h-[calc(100vh-2rem)] w-full ${width} flex-col rounded-2xl border bg-card shadow-pop`}
        onClick={(e) => e.stopPropagation()}
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
