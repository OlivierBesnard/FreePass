import { useCallback, useState } from "react";
import {
  ChevronRight,
  FolderOpen,
  LockKeyhole,
  Pencil,
  Plus,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import { api, errorMessage, type ProjectInfo } from "../lib/api";
import {
  useArchiveProject,
  useCreateProject,
  useProjects,
  useRenameProject,
} from "../hooks/useProjects";
import { useAutoLock } from "../hooks/useAutoLock";
import { Button, inputClass } from "../components/ui";
import { Modal } from "../components/Modal";
import { ConfirmDialog } from "../components/ConfirmDialog";

/**
 * Landing screen: list of projects (Studio cards). Picking one opens its
 * environments + entries (VaultHome). Create / rename / archive projects here.
 */
export function ProjectsHome({
  onLock,
  onOpen,
}: {
  onLock: () => void;
  onOpen: (project: ProjectInfo) => void;
}) {
  const { data: projects = [], isLoading } = useProjects();

  const [creating, setCreating] = useState(false);
  const [renaming, setRenaming] = useState<ProjectInfo | null>(null);
  const [archiving, setArchiving] = useState<ProjectInfo | null>(null);

  const lockNow = useCallback(async () => {
    try {
      await api.lock();
      onLock();
    } catch (err) {
      toast.error(errorMessage(err));
    }
  }, [onLock]);

  useAutoLock(lockNow);

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
          <div className="flex-1" />
          <Button onClick={() => setCreating(true)} className="h-9 shrink-0">
            <Plus size={16} className="mr-1" /> Nouveau projet
          </Button>
          <button
            onClick={lockNow}
            className="inline-flex h-9 items-center gap-1.5 rounded-lg border border-cream-400 px-3 text-sm font-medium text-ink-600 transition-colors hover:bg-cream-300"
          >
            <LockKeyhole size={15} /> Verrouiller
          </button>
        </div>
      </header>

      <div className="mx-auto max-w-3xl px-6 py-6">
        <h1 className="mb-4 font-serif text-xl font-semibold text-ink-800">
          Projets
        </h1>
        {isLoading ? (
          <p className="text-sm text-ink-500">Chargement…</p>
        ) : projects.length === 0 ? (
          <EmptyProjects onAdd={() => setCreating(true)} />
        ) : (
          <ul className="grid gap-3 sm:grid-cols-2">
            {projects.map((p) => (
              <li key={p.id}>
                <div className="row-hover group flex items-center gap-3 rounded-2xl border border-cream-400 bg-card px-4 py-4 shadow-soft transition-colors">
                  <button
                    onClick={() => onOpen(p)}
                    className="flex min-w-0 flex-1 items-center gap-3 text-left"
                  >
                    <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-brand-100 text-brand-700">
                      <FolderOpen size={19} />
                    </span>
                    <span className="block truncate font-medium text-ink-800">
                      {p.name}
                    </span>
                  </button>
                  <button
                    onClick={() => setRenaming(p)}
                    className="rounded-lg p-1.5 text-ink-400 opacity-0 transition-opacity hover:bg-cream-300 hover:text-ink-700 group-hover:opacity-100"
                    title="Renommer le projet"
                    aria-label="Renommer le projet"
                  >
                    <Pencil size={15} />
                  </button>
                  <button
                    onClick={() => setArchiving(p)}
                    className="rounded-lg p-1.5 text-ink-400 opacity-0 transition-opacity hover:bg-danger-50 hover:text-danger-600 group-hover:opacity-100"
                    title="Archiver le projet"
                    aria-label="Archiver le projet"
                  >
                    <Trash2 size={15} />
                  </button>
                  <ChevronRight
                    size={16}
                    className="shrink-0 text-ink-300"
                    onClick={() => onOpen(p)}
                  />
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>

      {creating && <ProjectNameDialog onClose={() => setCreating(false)} />}
      {renaming && (
        <ProjectNameDialog
          project={renaming}
          onClose={() => setRenaming(null)}
        />
      )}
      {archiving && (
        <ArchiveProjectDialog
          project={archiving}
          onClose={() => setArchiving(null)}
        />
      )}
    </main>
  );
}

/** Create or rename a project (single text field). */
function ProjectNameDialog({
  project,
  onClose,
}: {
  project?: ProjectInfo;
  onClose: () => void;
}) {
  const isEdit = !!project;
  const [name, setName] = useState(project?.name ?? "");
  const create = useCreateProject();
  const rename = useRenameProject();
  const busy = create.isPending || rename.isPending;

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;
    if (isEdit && project) {
      await rename.mutateAsync({ projectId: project.id, name: trimmed });
    } else {
      await create.mutateAsync(trimmed);
    }
    onClose();
  }

  return (
    <Modal
      title={isEdit ? "Renommer le projet" : "Nouveau projet"}
      onClose={onClose}
      width="max-w-sm"
    >
      <form onSubmit={submit} className="space-y-4">
        <input
          className={inputClass}
          autoFocus
          placeholder="Ex. Mon SaaS"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            className="inline-flex h-10 items-center rounded-lg border border-cream-400 px-4 text-sm font-medium text-ink-600 transition-colors hover:bg-cream-300"
          >
            Annuler
          </button>
          <Button type="submit" disabled={busy || !name.trim()}>
            {busy ? "Enregistrement…" : isEdit ? "Renommer" : "Créer"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}

function ArchiveProjectDialog({
  project,
  onClose,
}: {
  project: ProjectInfo;
  onClose: () => void;
}) {
  const archive = useArchiveProject();
  return (
    <ConfirmDialog
      title="Archiver ce projet ?"
      message={`« ${project.name} » et ses environnements seront masqués. Les données sont conservées (archivage réversible).`}
      confirmLabel="Archiver"
      busy={archive.isPending}
      onConfirm={() => {
        archive.mutate(project.id);
        onClose();
      }}
      onClose={onClose}
    />
  );
}

function EmptyProjects({ onAdd }: { onAdd: () => void }) {
  return (
    <div className="anim-fade-in rounded-2xl border border-dashed border-cream-500 bg-card/50 p-12 text-center">
      <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-2xl bg-brand-100 text-brand-700">
        <FolderOpen size={22} />
      </div>
      <p className="mt-3 text-sm text-ink-600">Aucun projet pour l'instant.</p>
      <div className="mt-4">
        <Button onClick={onAdd}>
          <Plus size={16} className="mr-1" /> Créer un projet
        </Button>
      </div>
    </div>
  );
}
