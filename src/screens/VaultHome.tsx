import { useCallback, useEffect, useState } from "react";
import {
  Archive,
  ArrowLeft,
  KeyRound,
  LockKeyhole,
  Plus,
  Puzzle,
  Search,
  Upload,
} from "lucide-react";
import { toast } from "sonner";
import {
  api,
  errorMessage,
  type EntryDetail,
  type ProjectInfo,
} from "../lib/api";
import { useEntries, useEntryIcons } from "../hooks/useVault";
import { useEnvironments } from "../hooks/useProjects";
import { useAutoLock } from "../hooks/useAutoLock";
import { Button, inputClass } from "../components/ui";
import { EntryForm } from "../components/EntryForm";
import { EntryDetailView } from "../components/EntryDetail";
import { CommandPalette } from "../components/CommandPalette";
import { ImportCsv } from "../components/ImportCsv";
import { ExtensionPairing } from "../components/ExtensionPairing";
import { ArchivedEntries } from "../components/ArchivedEntries";
import { EnvironmentSelector } from "../components/EnvironmentSelector";

/**
 * The unlocked vault scoped to a (project, environment) pair: environment
 * selector, list, search, Cmd+K, CRUD, archives, icons, extension, and lock.
 */
export function VaultHome({
  project,
  onLock,
  onBack,
}: {
  project: ProjectInfo;
  onLock: () => void;
  onBack: () => void;
}) {
  const projectId = project.id;
  const { data: environments = [] } = useEnvironments(projectId);

  // Selected environment: default to the first one of the project. Re-sync if
  // the selection points to an environment that no longer exists (archived).
  const [selectedEnvId, setSelectedEnvId] = useState<string | undefined>();
  useEffect(() => {
    if (environments.length === 0) {
      setSelectedEnvId(undefined);
      return;
    }
    setSelectedEnvId((cur) =>
      cur && environments.some((e) => e.id === cur) ? cur : environments[0].id,
    );
  }, [environments]);

  const envId = selectedEnvId;

  const [search, setSearch] = useState("");
  const { data: entries = [], isLoading } = useEntries(envId, search);
  const { data: icons = {} } = useEntryIcons(envId);

  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<EntryDetail | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [pairingOpen, setPairingOpen] = useState(false);
  const [archivedOpen, setArchivedOpen] = useState(false);

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

  const lockNow = useCallback(async () => {
    try {
      await api.lock();
      onLock();
    } catch (err) {
      toast.error(errorMessage(err));
    }
  }, [onLock]);

  // Auto-lock after inactivity (THREAT F11): zeroizes keys. The channel stays
  // up but serves no credentials while locked.
  useAutoLock(lockNow);

  // A single "Personnel" environment stays out of the way: only show the
  // selector once there is something to choose or manage beyond the default.
  const showSelector = environments.length > 0;

  return (
    <main className="bg-mesh min-h-full">
      <header className="border-b border-cream-400/60 bg-card/60 backdrop-blur-sm">
        <div className="mx-auto flex max-w-3xl items-center gap-3 px-6 py-3">
          <button
            onClick={onBack}
            className="inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-cream-400 text-ink-600 transition-colors hover:bg-cream-300"
            title="Retour aux projets"
            aria-label="Retour aux projets"
          >
            <ArrowLeft size={16} />
          </button>
          <div className="flex min-w-0 items-center gap-2">
            <span className="avatar-gradient flex h-8 w-8 items-center justify-center rounded-lg text-xs font-bold text-white">
              F
            </span>
            <span className="truncate font-serif text-base font-semibold text-ink-800">
              {project.name}
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

          <Button
            onClick={() => setCreating(true)}
            className="h-9 shrink-0"
            disabled={!envId}
          >
            <Plus size={16} className="mr-1" /> Ajouter
          </Button>
          <button
            onClick={() => setImportOpen(true)}
            disabled={!envId}
            className="inline-flex h-9 w-9 items-center justify-center rounded-lg border border-cream-400 text-ink-600 transition-colors hover:bg-cream-300 disabled:opacity-50"
            title="Importer un CSV"
            aria-label="Importer un CSV"
          >
            <Upload size={15} />
          </button>
          <button
            onClick={() => setArchivedOpen(true)}
            disabled={!envId}
            className="inline-flex h-9 w-9 items-center justify-center rounded-lg border border-cream-400 text-ink-600 transition-colors hover:bg-cream-300 disabled:opacity-50"
            title="Identifiants archivés"
            aria-label="Identifiants archivés"
          >
            <Archive size={15} />
          </button>
          <button
            onClick={() => setPairingOpen(true)}
            className="inline-flex h-9 w-9 items-center justify-center rounded-lg border border-cream-400 text-ink-600 transition-colors hover:bg-cream-300"
            title="Connecter l'extension"
            aria-label="Connecter l'extension"
          >
            <Puzzle size={15} />
          </button>
          <button
            onClick={lockNow}
            className="inline-flex h-9 items-center gap-1.5 rounded-lg border border-cream-400 px-3 text-sm font-medium text-ink-600 transition-colors hover:bg-cream-300"
          >
            <LockKeyhole size={15} /> Verrouiller
          </button>
        </div>
      </header>

      <div className="mx-auto max-w-3xl px-6 py-6">
        {showSelector && (
          <div className="mb-5">
            <EnvironmentSelector
              projectId={projectId}
              environments={environments}
              selectedId={selectedEnvId}
              onSelect={setSelectedEnvId}
            />
          </div>
        )}

        {isLoading ? (
          <p className="text-sm text-ink-500">Chargement…</p>
        ) : entries.length === 0 ? (
          <EmptyState
            onAdd={() => setCreating(true)}
            hasSearch={search.length > 0}
            canAdd={!!envId}
          />
        ) : (
          <ul className="space-y-2">
            {entries.map((e) => (
              <li key={e.id}>
                <button
                  onClick={() => setSelectedId(e.id)}
                  className="row-hover flex w-full items-center gap-3 rounded-xl border border-cream-400 bg-card px-4 py-3 text-left shadow-soft transition-colors"
                >
                  <EntryIcon icon={icons[e.id]} />
                  <span className="min-w-0 flex-1">
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
          projectId={projectId}
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
      {pairingOpen && <ExtensionPairing onClose={() => setPairingOpen(false)} />}
      {archivedOpen && envId && (
        <ArchivedEntries envId={envId} onClose={() => setArchivedOpen(false)} />
      )}
    </main>
  );
}

/** A site favicon if we have one, else the default key glyph. */
function EntryIcon({ icon }: { icon?: string }) {
  const [broken, setBroken] = useState(false);
  if (icon && !broken) {
    return (
      <span className="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-cream-400 bg-card">
        <img
          src={icon}
          alt=""
          className="h-5 w-5 object-contain"
          onError={() => setBroken(true)}
        />
      </span>
    );
  }
  return (
    <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-brand-100 text-brand-700">
      <KeyRound size={17} />
    </span>
  );
}

function EmptyState({
  onAdd,
  hasSearch,
  canAdd,
}: {
  onAdd: () => void;
  hasSearch: boolean;
  canAdd: boolean;
}) {
  return (
    <div className="anim-fade-in rounded-2xl border border-dashed border-cream-500 bg-card/50 p-12 text-center">
      <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-2xl bg-brand-100 text-brand-700">
        <KeyRound size={22} />
      </div>
      <p className="mt-3 text-sm text-ink-600">
        {hasSearch
          ? "Aucun identifiant ne correspond."
          : "Cet environnement est vide."}
      </p>
      {!hasSearch && canAdd && (
        <div className="mt-4">
          <Button onClick={onAdd}>
            <Plus size={16} className="mr-1" /> Ajouter un identifiant
          </Button>
        </div>
      )}
    </div>
  );
}
