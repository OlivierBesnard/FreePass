import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Archive,
  ChevronRight,
  FolderCog,
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
  type EntrySummary,
} from "../lib/api";
import {
  useAllEntries,
  useAllEntryIcons,
  useEnvironment,
} from "../hooks/useVault";
import { useAllEnvironments } from "../hooks/useProjects";
import { useAutoLock } from "../hooks/useAutoLock";
import { registrableDomain } from "../lib/domain";
import { Button, inputClass } from "../components/ui";
import { EntryForm } from "../components/EntryForm";
import { EntryDetailView } from "../components/EntryDetail";
import { CommandPalette } from "../components/CommandPalette";
import { ImportCsv } from "../components/ImportCsv";
import { ExtensionPairing } from "../components/ExtensionPairing";
import { ArchivedEntries } from "../components/ArchivedEntries";
import { ProjectsManager } from "../components/ProjectsManager";

/** A registrable-domain bucket of entries for the grouped (no-search) view. */
interface DomainGroup {
  /** Registrable domain, or "" for the "no site" bucket. */
  domain: string;
  entries: EntrySummary[];
}

/** Build domain buckets, preserving the backend order within each bucket. */
function groupByDomain(entries: EntrySummary[]): DomainGroup[] {
  const order: string[] = [];
  const map = new Map<string, EntrySummary[]>();
  for (const e of entries) {
    const domain = registrableDomain(e.url);
    let bucket = map.get(domain);
    if (!bucket) {
      bucket = [];
      map.set(domain, bucket);
      order.push(domain);
    }
    bucket.push(e);
  }
  return order.map((domain) => ({ domain, entries: map.get(domain)! }));
}

/**
 * The unlocked vault, unified: one flat list of every credential across all
 * live environments, auto-folded into per-site groups. The environment is a
 * discreet badge, not a folder. Search yields a flat list (no grouping).
 * Project/environment management lives behind a secondary surface.
 */
export function VaultHome({ onLock }: { onLock: () => void }) {
  const [search, setSearch] = useState("");
  const { data: entries = [], isLoading } = useAllEntries(search);
  const envIds = useMemo(
    () => [...new Set(entries.map((e) => e.env_id))],
    [entries],
  );
  const { data: icons = {} } = useAllEntryIcons(envIds);
  // Default environment for zero-ceremony create / import targeting.
  const { data: defaultEnv } = useEnvironment();
  // Lookup so an entry can resolve its owning project (edit / duplicate).
  const { data: envMap = {} } = useAllEnvironments();
  // Every live environment id (not just those with live entries), so the
  // unified archives view also surfaces archived entries from empty envs.
  const allLiveEnvIds = useMemo(
    () => [...new Set([...Object.keys(envMap), ...envIds])],
    [envMap, envIds],
  );

  // Show env badges only when entries actually span more than one environment;
  // a single "Personnel" environment stays badge-free for a clean personal use.
  const distinctEnvCount = useMemo(
    () => new Set(entries.map((e) => e.env_id)).size,
    [entries],
  );
  const showEnvBadge = distinctEnvCount > 1;

  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<{
    entry: EntryDetail;
    envId: string;
  } | null>(null);
  const [selected, setSelected] = useState<EntrySummary | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [pairingOpen, setPairingOpen] = useState(false);
  const [archivedOpen, setArchivedOpen] = useState(false);
  const [projectsOpen, setProjectsOpen] = useState(false);

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

  const searching = search.trim().length > 0;
  const groups = useMemo(
    () => (searching ? [] : groupByDomain(entries)),
    [entries, searching],
  );

  const defaultEnvId = defaultEnv?.id;
  const selectedProjectId = selected
    ? envMap[selected.env_id]?.project_id ?? null
    : null;

  return (
    <main className="bg-mesh min-h-full">
      <header className="border-b border-cream-400/60 bg-card/60 backdrop-blur-sm">
        <div className="mx-auto flex max-w-3xl items-center gap-3 px-6 py-3">
          <div className="flex min-w-0 items-center gap-2">
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

          <Button
            onClick={() => setCreating(true)}
            className="h-9 shrink-0"
            disabled={!defaultEnvId}
          >
            <Plus size={16} className="mr-1" /> Ajouter
          </Button>
          <button
            onClick={() => setImportOpen(true)}
            disabled={!defaultEnvId}
            className="inline-flex h-9 w-9 items-center justify-center rounded-lg border border-cream-400 text-ink-600 transition-colors hover:bg-cream-300 disabled:opacity-50"
            title="Importer un CSV"
            aria-label="Importer un CSV"
          >
            <Upload size={15} />
          </button>
          <button
            onClick={() => setArchivedOpen(true)}
            className="inline-flex h-9 w-9 items-center justify-center rounded-lg border border-cream-400 text-ink-600 transition-colors hover:bg-cream-300"
            title="Identifiants archivés"
            aria-label="Identifiants archivés"
          >
            <Archive size={15} />
          </button>
          <button
            onClick={() => setProjectsOpen(true)}
            className="inline-flex h-9 w-9 items-center justify-center rounded-lg border border-cream-400 text-ink-600 transition-colors hover:bg-cream-300"
            title="Projets & environnements"
            aria-label="Projets & environnements"
          >
            <FolderCog size={15} />
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
        {isLoading ? (
          <p className="text-sm text-ink-500">Chargement…</p>
        ) : entries.length === 0 ? (
          <EmptyState
            onAdd={() => setCreating(true)}
            hasSearch={searching}
            canAdd={!!defaultEnvId}
          />
        ) : searching ? (
          <ul className="space-y-2">
            {entries.map((e) => (
              <EntryRow
                key={e.id}
                entry={e}
                icon={icons[e.id]}
                showEnvBadge={showEnvBadge}
                onOpen={() => setSelected(e)}
              />
            ))}
          </ul>
        ) : (
          <ul className="space-y-2">
            {groups.map((g) =>
              g.entries.length >= 2 ? (
                <DomainGroupRow
                  key={g.domain || "__no-site__"}
                  group={g}
                  icons={icons}
                  showEnvBadge={showEnvBadge}
                  onOpen={setSelected}
                />
              ) : (
                <EntryRow
                  key={g.entries[0].id}
                  entry={g.entries[0]}
                  icon={icons[g.entries[0].id]}
                  showEnvBadge={showEnvBadge}
                  onOpen={() => setSelected(g.entries[0])}
                />
              ),
            )}
          </ul>
        )}
      </div>

      {creating && defaultEnvId && (
        <EntryForm
          defaultEnvId={defaultEnvId}
          entry={null}
          onClose={() => setCreating(false)}
        />
      )}
      {editing && (
        <EntryForm
          defaultEnvId={editing.envId}
          entry={editing.entry}
          onClose={() => setEditing(null)}
        />
      )}
      {selected && (
        <EntryDetailView
          envId={selected.env_id}
          projectId={selectedProjectId}
          entryId={selected.id}
          onClose={() => setSelected(null)}
          onEdit={(entry) => {
            const envId = selected.env_id;
            setSelected(null);
            setEditing({ entry, envId });
          }}
        />
      )}
      {paletteOpen && (
        <CommandPalette
          entries={entries}
          onClose={() => setPaletteOpen(false)}
          onSelect={(id) => {
            setPaletteOpen(false);
            const entry = entries.find((e) => e.id === id) ?? null;
            setSelected(entry);
          }}
        />
      )}
      {importOpen && defaultEnvId && (
        <ImportCsv envId={defaultEnvId} onClose={() => setImportOpen(false)} />
      )}
      {pairingOpen && <ExtensionPairing onClose={() => setPairingOpen(false)} />}
      {archivedOpen && (
        <ArchivedEntries
          envIds={allLiveEnvIds}
          onClose={() => setArchivedOpen(false)}
        />
      )}
      {projectsOpen && (
        <ProjectsManager onClose={() => setProjectsOpen(false)} />
      )}
    </main>
  );
}

/** A single flat credential row, with an optional environment badge. */
function EntryRow({
  entry,
  icon,
  showEnvBadge,
  onOpen,
}: {
  entry: EntrySummary;
  icon?: string;
  showEnvBadge: boolean;
  onOpen: () => void;
}) {
  return (
    <li>
      <button
        onClick={onOpen}
        className="row-hover flex w-full items-center gap-3 rounded-xl border border-cream-400 bg-card px-4 py-3 text-left shadow-soft transition-colors"
      >
        <EntryIcon icon={icon} />
        <span className="min-w-0 flex-1">
          <span className="block truncate font-medium text-ink-800">
            {entry.title}
          </span>
          {entry.url && (
            <span className="block truncate text-xs text-ink-400">
              {entry.url}
            </span>
          )}
        </span>
        {showEnvBadge && entry.env_name && <EnvBadge name={entry.env_name} />}
      </button>
    </li>
  );
}

/** A collapsible group of entries sharing one registrable domain. */
function DomainGroupRow({
  group,
  icons,
  showEnvBadge,
  onOpen,
}: {
  group: DomainGroup;
  icons: Record<string, string>;
  showEnvBadge: boolean;
  onOpen: (entry: EntrySummary) => void;
}) {
  const [open, setOpen] = useState(false);
  // Reuse the first member's favicon as the group glyph.
  const groupIcon = icons[group.entries[0].id];
  const label = group.domain || "Sans site";

  return (
    <li>
      <button
        onClick={() => setOpen((v) => !v)}
        className="row-hover flex w-full items-center gap-3 rounded-xl border border-cream-400 bg-card px-4 py-3 text-left shadow-soft transition-colors"
        aria-expanded={open}
      >
        <EntryIcon icon={groupIcon} fallbackLabel={label} />
        <span className="min-w-0 flex-1">
          <span className="block truncate font-medium text-ink-800">
            {label}
          </span>
          <span className="block text-xs text-ink-400">
            {group.entries.length} identifiants
          </span>
        </span>
        <ChevronRight
          size={16}
          className={
            "shrink-0 text-ink-300 transition-transform " +
            (open ? "rotate-90" : "")
          }
        />
      </button>

      {open && (
        <ul className="mt-2 space-y-2 border-l-2 border-cream-400 pl-3">
          {group.entries.map((e) => (
            <li key={e.id}>
              <button
                onClick={() => onOpen(e)}
                className="row-hover flex w-full items-center gap-3 rounded-xl border border-cream-400 bg-card px-4 py-2.5 text-left shadow-soft transition-colors"
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
                {showEnvBadge && e.env_name && <EnvBadge name={e.env_name} />}
              </button>
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}

/** Discreet environment label shown only when entries span several envs. */
function EnvBadge({ name }: { name: string }) {
  return (
    <span className="shrink-0 rounded-full border border-cream-400 bg-cream-200 px-2 py-0.5 text-[11px] font-medium text-ink-500">
      {name}
    </span>
  );
}

/** A site favicon if we have one, else a letter/key glyph. */
function EntryIcon({
  icon,
  fallbackLabel,
}: {
  icon?: string;
  fallbackLabel?: string;
}) {
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
  const letter = fallbackLabel?.trim()?.[0]?.toUpperCase();
  return (
    <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-brand-100 text-brand-700">
      {letter ? (
        <span className="text-sm font-semibold">{letter}</span>
      ) : (
        <KeyRound size={17} />
      )}
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
          : "Aucun identifiant pour l'instant."}
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
