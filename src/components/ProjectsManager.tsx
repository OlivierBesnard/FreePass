import { useState } from "react";
import {
  ChevronRight,
  FolderOpen,
  Pencil,
  Plus,
  Trash2,
} from "lucide-react";
import type { EnvironmentInfo, ProjectInfo } from "../lib/api";
import {
  useArchiveProject,
  useCreateProject,
  useEnvironments,
  useProjects,
  useRenameProject,
} from "../hooks/useProjects";
import { Modal } from "./Modal";
import { ConfirmDialog } from "./ConfirmDialog";
import { EnvironmentSelector } from "./EnvironmentSelector";
import { Button, inputClass } from "./ui";

/**
 * Secondary surface: manage projects and their environments. This is NOT the
 * landing screen — the unified list is. Most personal users never open this; it
 * only earns its keep once you keep several projects or environments. Reuses the
 * existing project hooks and the EnvironmentSelector's create/rename/archive.
 */
export function ProjectsManager({ onClose }: { onClose: () => void }) {
  const { data: projects = [], isLoading } = useProjects();

  const [creating, setCreating] = useState(false);
  const [renaming, setRenaming] = useState<ProjectInfo | null>(null);
  const [archiving, setArchiving] = useState<ProjectInfo | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);

  return (
    <Modal title="Projets & environnements" onClose={onClose} width="max-w-xl">
      <div className="mb-4 flex items-center justify-between">
        <p className="text-sm text-ink-500">
          Organisez vos identifiants par projet et par environnement. Optionnel —
          pour la plupart des usages, un seul projet suffit.
        </p>
      </div>

      {isLoading ? (
        <p className="text-sm text-ink-500">Chargement…</p>
      ) : (
        <ul className="space-y-2">
          {projects.map((p) => (
            <li
              key={p.id}
              className="rounded-xl border border-cream-400 bg-card shadow-soft"
            >
              <div className="group flex items-center gap-3 px-4 py-3">
                <button
                  onClick={() =>
                    setExpanded((cur) => (cur === p.id ? null : p.id))
                  }
                  className="flex min-w-0 flex-1 items-center gap-3 text-left"
                  aria-expanded={expanded === p.id}
                >
                  <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-brand-100 text-brand-700">
                    <FolderOpen size={17} />
                  </span>
                  <span className="block truncate font-medium text-ink-800">
                    {p.name}
                  </span>
                </button>
                <button
                  onClick={() => setRenaming(p)}
                  className="rounded-lg p-1.5 text-ink-400 transition-colors hover:bg-cream-300 hover:text-ink-700"
                  title="Renommer le projet"
                  aria-label="Renommer le projet"
                >
                  <Pencil size={15} />
                </button>
                <button
                  onClick={() => setArchiving(p)}
                  className="rounded-lg p-1.5 text-ink-400 transition-colors hover:bg-danger-50 hover:text-danger-600"
                  title="Archiver le projet"
                  aria-label="Archiver le projet"
                >
                  <Trash2 size={15} />
                </button>
                <ChevronRight
                  size={16}
                  className={
                    "shrink-0 cursor-pointer text-ink-300 transition-transform " +
                    (expanded === p.id ? "rotate-90" : "")
                  }
                  onClick={() =>
                    setExpanded((cur) => (cur === p.id ? null : p.id))
                  }
                />
              </div>
              {expanded === p.id && (
                <div className="border-t border-cream-400 px-4 py-3">
                  <ProjectEnvironments projectId={p.id} />
                </div>
              )}
            </li>
          ))}
        </ul>
      )}

      <div className="mt-4 flex justify-end">
        <Button onClick={() => setCreating(true)} className="h-9">
          <Plus size={16} className="mr-1" /> Nouveau projet
        </Button>
      </div>

      {creating && <ProjectNameDialog onClose={() => setCreating(false)} />}
      {renaming && (
        <ProjectNameDialog project={renaming} onClose={() => setRenaming(null)} />
      )}
      {archiving && (
        <ArchiveProjectDialog
          project={archiving}
          onClose={() => setArchiving(null)}
        />
      )}
    </Modal>
  );
}

/** Environment chips for a project with create/rename/archive (read-only select). */
function ProjectEnvironments({ projectId }: { projectId: string }) {
  const { data: environments = [], isLoading } = useEnvironments(projectId);
  // The selector needs a "selected" chip to expose rename/archive; default to
  // the first. Selection here is purely for management, not for filtering.
  const [selectedId, setSelectedId] = useState<string | undefined>();
  const effectiveSelected =
    selectedId && environments.some((e: EnvironmentInfo) => e.id === selectedId)
      ? selectedId
      : environments[0]?.id;

  if (isLoading) {
    return <p className="text-sm text-ink-500">Chargement…</p>;
  }

  return (
    <EnvironmentSelector
      projectId={projectId}
      environments={environments}
      selectedId={effectiveSelected}
      onSelect={setSelectedId}
    />
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
