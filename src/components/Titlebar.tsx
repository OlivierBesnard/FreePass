import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";

const appWindow = getCurrentWindow();

/**
 * Custom, cream-colored window title bar. The native (dark) title bar is removed
 * (`decorations: false`), so this draggable strip replaces it and blends with the
 * app. It carries the window controls (minimize / maximize / close).
 */
export function Titlebar() {
  return (
    <div
      data-tauri-drag-region
      className="flex h-8 shrink-0 select-none items-center justify-between border-b border-cream-400/70 bg-cream-100 pl-3"
    >
      <span
        data-tauri-drag-region
        className="text-xs font-semibold tracking-wide text-ink-500"
      >
        FreePass
      </span>
      <div className="flex items-stretch">
        <button
          onClick={() => appWindow.minimize()}
          className="flex h-8 w-11 items-center justify-center text-ink-500 transition-colors hover:bg-cream-300"
          aria-label="Réduire"
          title="Réduire"
        >
          <Minus size={15} />
        </button>
        <button
          onClick={() => appWindow.toggleMaximize()}
          className="flex h-8 w-11 items-center justify-center text-ink-500 transition-colors hover:bg-cream-300"
          aria-label="Agrandir"
          title="Agrandir"
        >
          <Square size={12} />
        </button>
        <button
          onClick={() => appWindow.close()}
          className="flex h-8 w-11 items-center justify-center text-ink-500 transition-colors hover:bg-danger-600 hover:text-white"
          aria-label="Fermer"
          title="Fermer"
        >
          <X size={16} />
        </button>
      </div>
    </div>
  );
}
