import { useEffect, useState } from "react";
import { KeyRound, LockKeyhole, Plus, Search, Upload } from "lucide-react";
import { toast } from "sonner";
import { api, errorMessage, type EntryDetail } from "../lib/api";
import { useEnvironment, useEntries } from "../hooks/useVault";
import { Button, inputClass } from "../components/ui";
import { EntryForm } from "../components/EntryForm";
import { EntryDetailView } from "../components/EntryDetail";
import { CommandPalette } from "../components/CommandPalette";
import { ImportCsv } from "../components/ImportCsv";

/** The unlocked vault: list, search, Cmd+K, CRUD, and lock. */
export function VaultHome({ onLock }: { onLock: () => void }) {
  const { data: env } = useEnvironment();
  const envId = env?.id;

  const [search, setSearch] = useState("");
  const { data: entries = [], isLoading } = useEntries(envId, search);

  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<EntryDetail | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((v) => !v);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  async function handleLock() {
    try {
      await api.lock();
      onLock();
    } catch (err) {
      toast.error(errorMessage(err));
    }
  }

  return (
    <main className="bg-mesh min-h-full">
      <header className="border-b border-cream-400/60 bg-card/60 backdrop-blur-sm">
        <div className="mx-auto flex max-w-3xl items-center gap-3 px-6 py-3">
          <div className="flex items-center gap-2">
            <span className="avatar-gradient flex h-8 w-8 items-center justify-center rounded-lg text-xs font-bold text-white">
              F
            </span>
            <span className="font-serif text-base font-semibold text-ink-800">
              FreePass
            </span>
          </div>

          <div className="relative ml-2 flex-1">
            <Search
              size={15}
              className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-ink-400"
            />
            <input
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Rechercher…"
              className={`${inputClass} h-9 pl-9 pr-12`}
            />
            <span className="kbd pointer-events-none absolute right-2 top-1/2 -translate-y-1/2">
              ⌘K
            </span>
          </div>

          <Button onClick={() => setCreating(true)} className="h-9 shrink-0">
            <Plus size={16} className="mr-1" /> Ajouter
          </Button>
          <button
            onClick={() => setImportOpen(true)}
            className="inline-flex h-9 items-center gap-1.5 rounded-lg border border-cream-400 px-3 text-sm font-medium text-ink-600 transition-colors hover:bg-cream-300"
            title="Importer un CSV"
          >
            <Upload size={15} /> Importer
          </button>
          <button
            onClick={handleLock}
            className="inline-flex h-9 items-center gap-1.5 rounded-lg border border-cream-400 px-3 text-sm font-medium text-ink-600 transition-colors hover:bg-cream-300"
          >
            <LockKeyhole size={15} /> Verrouiller
          </button>
        </div>
      </header>

      <div className="mx-auto max-w-3xl px-6 py-6">
        {isLoading ? (
          <p className="text-sm text-ink-500">Chargement…</p>
        ) : entries.length === 0 ? (
          <EmptyState onAdd={() => setCreating(true)} hasSearch={search.length > 0} />
        ) : (
          <ul className="space-y-2">
            {entries.map((e) => (
              <li key={e.id}>
                <button
                  onClick={() => setSelectedId(e.id)}
                  className="row-hover flex w-full items-center gap-3 rounded-xl border border-cream-400 bg-card px-4 py-3 text-left shadow-soft transition-colors"
                >
                  <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-brand-100 text-brand-700">
                    <KeyRound size={17} />
                  </span>
                  <span className="min-w-0">
                    <span className="block truncate font-medium text-ink-800">
                      {e.title}
                    </span>
                    {e.url && (
                      <span className="block truncate text-xs text-ink-400">
                        {e.url}
                      </span>
                    )}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {creating && envId && (
        <EntryForm envId={envId} entry={null} onClose={() => setCreating(false)} />
      )}
      {editing && envId && (
        <EntryForm envId={envId} entry={editing} onClose={() => setEditing(null)} />
      )}
      {selectedId && envId && (
        <EntryDetailView
          envId={envId}
          entryId={selectedId}
          onClose={() => setSelectedId(null)}
          onEdit={(entry) => {
            setSelectedId(null);
            setEditing(entry);
          }}
        />
      )}
      {paletteOpen && (
        <CommandPalette
          entries={entries}
          onClose={() => setPaletteOpen(false)}
          onSelect={(id) => {
            setPaletteOpen(false);
            setSelectedId(id);
          }}
        />
      )}
      {importOpen && envId && (
        <ImportCsv envId={envId} onClose={() => setImportOpen(false)} />
      )}
    </main>
  );
}

function EmptyState({
  onAdd,
  hasSearch,
}: {
  onAdd: () => void;
  hasSearch: boolean;
}) {
  return (
    <div className="anim-fade-in rounded-2xl border border-dashed border-cream-500 bg-card/50 p-12 text-center">
      <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-2xl bg-brand-100 text-brand-700">
        <KeyRound size={22} />
      </div>
      <p className="mt-3 text-sm text-ink-600">
        {hasSearch ? "Aucun identifiant ne correspond." : "Votre coffre est vide."}
      </p>
      {!hasSearch && (
        <div className="mt-4">
          <Button onClick={onAdd}>
            <Plus size={16} className="mr-1" /> Ajouter un identifiant
          </Button>
        </div>
      )}
    </div>
  );
}
