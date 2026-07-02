import { useCallback, useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  Archive,
  ChevronRight,
  Copy,
  ExternalLink,
  Eye,
  EyeOff,
  FolderCog,
  KeyRound,
  LockKeyhole,
  Plus,
  Puzzle,
  Search,
  Upload,
  User,
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
import { copyPlain, copySecret } from "../lib/clipboard";
import { openExternal } from "../lib/openExternal";
import { Button, inputClass } from "../components/ui";
import { EntryForm } from "../components/EntryForm";
import { EntryDetailView } from "../components/EntryDetail";
import { CommandPalette } from "../components/CommandPalette";
import { isModalOpen } from "../components/Modal";
import { ImportCsv } from "../components/ImportCsv";
import { ExtensionPairing } from "../components/ExtensionPairing";
import { ArchivedEntries } from "../components/ArchivedEntries";
import { ProjectsManager } from "../components/ProjectsManager";

/** Platform-correct label for the quick-search shortcut (⌘ on macOS, Ctrl
 *  elsewhere) so Windows/Linux users don't see the Mac Command glyph (#11). */
const IS_MAC =
  typeof navigator !== "undefined" &&
  /mac/i.test(navigator.platform || navigator.userAgent || "");
const SHORTCUT_LABEL = IS_MAC ? "⌘K" : "Ctrl K";

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
  // The full, unfiltered list — backs the command palette (⌘K searches ALL
  // entries, not the header-filtered subset, B7) and the env-badge decision (B14).
  // Same query key as `entries` when the header search is empty, so no extra
  // fetch in the common case.
  const { data: allEntries = [] } = useAllEntries("");
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
  // Computed on the FULL list so badges don't flicker when a search narrows the
  // visible entries down to a single environment (B14).
  const distinctEnvCount = useMemo(
    () => new Set(allEntries.map((e) => e.env_id)).size,
    [allEntries],
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
        // Don't toggle the palette behind an already-open dialog (B3); Escape
        // closes the topmost modal instead.
        if (isModalOpen()) return;
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
        <div className="mx-auto flex max-w-3xl items-center gap-2 px-4 py-3 min-[720px]:gap-3 min-[720px]:px-6">
          <div className="flex min-w-0 items-center gap-2">
            <span className="avatar-gradient flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-xs font-bold text-white">
              F
            </span>
            <span className="hidden font-serif text-base font-semibold text-ink-800 min-[720px]:block">
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
              {SHORTCUT_LABEL}
            </span>
          </div>

          <Button
            onClick={() => setCreating(true)}
            className="h-9 shrink-0"
            disabled={!defaultEnvId}
            title="Ajouter un identifiant"
          >
            <Plus size={16} className="min-[720px]:mr-1" />
            <span className="hidden min-[720px]:inline">Ajouter</span>
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
            title="Verrouiller"
            className="inline-flex h-9 shrink-0 items-center gap-1.5 rounded-lg border border-cream-400 px-2.5 text-sm font-medium text-ink-600 transition-colors hover:bg-cream-300 min-[720px]:px-3"
          >
            <LockKeyhole size={15} />
            <span className="hidden min-[720px]:inline">Verrouiller</span>
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
        ) : (
          <div className="overflow-hidden rounded-2xl border border-cream-400 bg-card shadow-soft">
            {searching
              ? entries.map((e, i) => (
                  <DenseRow
                    key={e.id}
                    entry={e}
                    icon={icons[e.id]}
                    showEnvBadge={showEnvBadge}
                    first={i === 0}
                    onOpen={() => setSelected(e)}
                  />
                ))
              : groups.map((g, i) =>
                  g.entries.length >= 2 ? (
                    <DomainGroupRow
                      key={g.domain || "__no-site__"}
                      group={g}
                      icons={icons}
                      showEnvBadge={showEnvBadge}
                      first={i === 0}
                      onOpen={setSelected}
                    />
                  ) : (
                    <DenseRow
                      key={g.entries[0].id}
                      entry={g.entries[0]}
                      icon={icons[g.entries[0].id]}
                      showEnvBadge={showEnvBadge}
                      first={i === 0}
                      onOpen={() => setSelected(g.entries[0])}
                    />
                  ),
                )}
          </div>
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
          entries={allEntries}
          onClose={() => setPaletteOpen(false)}
          onSelect={(id) => {
            setPaletteOpen(false);
            const entry = allEntries.find((e) => e.id === id) ?? null;
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

const onActivate =
  (fn: () => void) => (e: React.KeyboardEvent) => {
    // Only act on keys targeting the row itself — a keydown bubbling up from a
    // focused quick-action button must fire that button, not open the row (B4).
    if (e.target !== e.currentTarget) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      fn();
    }
  };

/** Dense credential row with inline quick actions (reveal / copy / open). */
function DenseRow({
  entry,
  icon,
  showEnvBadge,
  first = false,
  indent = false,
  onOpen,
}: {
  entry: EntrySummary;
  icon?: string;
  showEnvBadge: boolean;
  first?: boolean;
  indent?: boolean;
  onOpen: () => void;
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={onActivate(onOpen)}
      className={
        "row-hover flex cursor-pointer items-center gap-2.5 px-3 py-2 outline-none focus-visible:bg-cream-200 " +
        (first ? "" : "border-t border-cream-300 ") +
        (indent ? "bg-cream-200/40 pl-9" : "")
      }
    >
      <EntryIcon icon={icon} />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[13px] font-semibold text-ink-800">
          {entry.title}
        </span>
        {entry.url && (
          <span className="block truncate text-[11px] text-ink-400">
            {entry.url}
          </span>
        )}
      </span>
      {showEnvBadge && entry.env_name && <EnvBadge name={entry.env_name} />}
      <QuickActions entry={entry} />
    </div>
  );
}

/** A collapsible group of entries sharing one registrable domain. */
function DomainGroupRow({
  group,
  icons,
  showEnvBadge,
  first,
  onOpen,
}: {
  group: DomainGroup;
  icons: Record<string, string>;
  showEnvBadge: boolean;
  first: boolean;
  onOpen: (entry: EntrySummary) => void;
}) {
  const [open, setOpen] = useState(false);
  // Reuse the first member's favicon as the group glyph.
  const groupIcon = icons[group.entries[0].id];
  const label = group.domain || "Sans site";

  return (
    <div className={first ? "" : "border-t border-cream-300"}>
      <div
        role="button"
        tabIndex={0}
        onClick={() => setOpen((v) => !v)}
        onKeyDown={onActivate(() => setOpen((v) => !v))}
        aria-expanded={open}
        className="row-hover flex cursor-pointer items-center gap-2.5 px-3 py-2 outline-none focus-visible:bg-cream-200"
      >
        <ChevronRight
          size={15}
          className={
            "shrink-0 text-ink-300 transition-transform " +
            (open ? "rotate-90" : "")
          }
        />
        <EntryIcon icon={groupIcon} fallbackLabel={label} />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-[13px] font-semibold text-ink-800">
            {label}
          </span>
          <span className="block text-[11px] text-ink-400">
            {group.entries.length} identifiants
          </span>
        </span>
      </div>
      {open &&
        group.entries.map((e) => (
          <DenseRow
            key={e.id}
            entry={e}
            icon={icons[e.id]}
            showEnvBadge={showEnvBadge}
            indent
            onOpen={() => onOpen(e)}
          />
        ))}
    </div>
  );
}

/**
 * Inline quick actions: reveal / copy password / copy username / open site.
 * Secrets are fetched ON DEMAND for this one entry (never bulk-loaded), then
 * cached by react-query and purged on lock (F3/F5).
 */
function QuickActions({ entry }: { entry: EntrySummary }) {
  const qc = useQueryClient();
  const [revealed, setRevealed] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = () =>
    qc.fetchQuery({
      queryKey: ["entry", entry.env_id, entry.id],
      queryFn: () => api.getEntry(entry.env_id, entry.id),
      staleTime: 30_000,
    });

  const run =
    (fn: () => Promise<void>) => async (e: React.MouseEvent) => {
      e.stopPropagation();
      if (busy) return;
      setBusy(true);
      try {
        await fn();
      } catch (err) {
        toast.error(errorMessage(err));
      } finally {
        setBusy(false);
      }
    };

  const toggleReveal = run(async () => {
    if (revealed !== null) {
      setRevealed(null);
      return;
    }
    const d = await load();
    setRevealed(d.password ?? "—");
  });
  const copyPw = run(async () => {
    const d = await load();
    if (d.password) copySecret(d.password, "Mot de passe");
    else toast.info("Aucun mot de passe sur cette entrée.");
  });
  const copyUser = run(async () => {
    const d = await load();
    if (d.username) copyPlain(d.username, "Identifiant");
    else toast.info("Aucun identifiant sur cette entrée.");
  });

  return (
    <div
      className="flex items-center gap-1"
      onClick={(e) => e.stopPropagation()}
    >
      <span
        className="mr-0.5 hidden w-16 truncate text-right font-mono text-[11px] tracking-widest text-ink-400 min-[520px]:inline"
        title={revealed ?? undefined}
      >
        {revealed !== null ? revealed : "••••••••"}
      </span>
      <ActBtn
        title={revealed !== null ? "Masquer" : "Révéler"}
        onClick={toggleReveal}
      >
        {revealed !== null ? <EyeOff size={15} /> : <Eye size={15} />}
      </ActBtn>
      <ActBtn title="Copier le mot de passe" onClick={copyPw}>
        <Copy size={15} />
      </ActBtn>
      <ActBtn title="Copier l'identifiant" onClick={copyUser}>
        <User size={15} />
      </ActBtn>
      {entry.url && (
        <ActBtn
          title="Ouvrir le site"
          onClick={(e) => {
            e.stopPropagation();
            openExternal(entry.url!);
          }}
        >
          <ExternalLink size={15} />
        </ActBtn>
      )}
    </div>
  );
}

function ActBtn({
  title,
  onClick,
  children,
}: {
  title: string;
  onClick: (e: React.MouseEvent) => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      onClick={onClick}
      className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-ink-400 transition-colors hover:bg-cream-300 hover:text-brand-600"
    >
      {children}
    </button>
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
      <span className="flex h-7 w-7 shrink-0 items-center justify-center overflow-hidden rounded-md border border-cream-400 bg-card">
        <img
          src={icon}
          alt=""
          className="h-4 w-4 object-contain"
          onError={() => setBroken(true)}
        />
      </span>
    );
  }
  const letter = fallbackLabel?.trim()?.[0]?.toUpperCase();
  return (
    <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-brand-100 text-brand-700">
      {letter ? (
        <span className="text-xs font-semibold">{letter}</span>
      ) : (
        <KeyRound size={15} />
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
