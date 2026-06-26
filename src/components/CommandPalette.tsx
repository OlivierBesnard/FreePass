import { useMemo, useState } from "react";
import { Search } from "lucide-react";
import type { EntrySummary } from "../lib/api";
import { Modal } from "./Modal";
import { inputClass } from "./ui";

/** Cmd+K quick switcher: filters entries locally on title/url. */
export function CommandPalette({
  entries,
  onSelect,
  onClose,
}: {
  entries: EntrySummary[];
  onSelect: (id: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");

  const results = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return entries.slice(0, 50);
    return entries
      .filter(
        (e) =>
          e.title.toLowerCase().includes(q) ||
          (e.url ?? "").toLowerCase().includes(q),
      )
      .slice(0, 50);
  }, [entries, query]);

  function handleKey(e: React.KeyboardEvent) {
    if (e.key === "Enter" && results.length > 0) {
      onSelect(results[0].id);
    }
  }

  return (
    <Modal title="Rechercher" onClose={onClose} width="max-w-xl">
      <div className="relative mb-3">
        <Search
          size={16}
          className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-ink-400"
        />
        <input
          autoFocus
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKey}
          placeholder="Filtrer par titre ou site…"
          className={`${inputClass} pl-9`}
        />
      </div>
      <ul className="max-h-72 space-y-1 overflow-y-auto scrollbar-thin">
        {results.length === 0 && (
          <li className="px-2 py-6 text-center text-sm text-ink-400">
            Aucun résultat.
          </li>
        )}
        {results.map((e) => (
          <li key={e.id}>
            <button
              onClick={() => onSelect(e.id)}
              className="row-hover flex w-full items-center justify-between rounded-lg px-3 py-2 text-left"
            >
              <span className="text-sm font-medium text-ink-800">{e.title}</span>
              {e.url && <span className="text-xs text-ink-400">{e.url}</span>}
            </button>
          </li>
        ))}
      </ul>
    </Modal>
  );
}
