import { useState } from "react";
import { Pencil, Plus, Trash2 } from "lucide-react";
import type { EnvironmentInfo } from "../lib/api";
import {
  useArchiveEnvironment,
  useCreateEnvironment,
  useRenameEnvironment,
} from "../hooks/useProjects";
import { Modal } from "./Modal";
import { ConfirmDialog } from "./ConfirmDialog";
import { Button, inputClass } from "./ui";

/**
 * Horizontal chips selecting the active environment of a project, plus
 * add/rename/archive. Stays out of the way when there is a single environment.
 */
export function EnvironmentSelector({
  projectId,
  environments,
  selectedId,
  onSelect,
}: {
  projectId: string;
  environments: EnvironmentInfo[];
  selectedId: string | undefined;
  onSelect: (envId: string) => void;
}) {
  const [creating, setCreating] = useState(false);
  const [renaming, setRenaming] = useState<EnvironmentInfo | null>(null);
  const [archiving, setArchiving] = useState<EnvironmentInfo | null>(null);

  const selected = environments.find((e) => e.id === selectedId);
  const canArchive = environments.length > 1;

  return (
    <div className="flex flex-wrap items-center gap-2">
      {environments.map((env) => {
        const active = env.id === selectedId;
        return (
          <button
            key={env.id}
            onClick={() => onSelect(env.id)}
            className={
              "inline-flex h-8 items-center rounded-full border px-3 text-sm font-medium transition-colors " +
              (active
                ? "border-brand-400 bg-brand-100 text-brand-700"
                : "border-cream-400 bg-card text-ink-600 hover:bg-cream-300")
            }
          >
            {env.name}
          </button>
        );
      })}

      <button
        onClick={() => setCreating(true)}
        className="inline-flex h-8 items-center gap-1 rounded-full border border-dashed border-cream-500 px-3 text-sm font-medium text-ink-500 transition-colors hover:bg-cream-300"
        title="Ajouter un environnement"
      >
        <Plus size={14} /> Ajouter un environnement
      </button>

      {selected && (
        <span className="ml-1 inline-flex items-center gap-0.5">
          <button
            onClick={() => setRenaming(selected)}
            className="rounded-lg p-1.5 text-ink-400 transition-colors hover:bg-cream-300 hover:text-ink-700"
            title="Renommer l'environnement"
            aria-label="Renommer l'environnement"
          >
            <Pencil size={14} />
          </button>
          {canArchive && (
            <button
              onClick={() => setArchiving(selected)}
              className="rounded-lg p-1.5 text-ink-400 transition-colors hover:bg-danger-50 hover:text-danger-600"
              title="Archiver l'environnement"
              aria-label="Archiver l'environnement"
            >
              <Trash2 size={14} />
            </button>
          )}
        </span>
      )}

      {creating && (
        <EnvironmentNameDialog
          projectId={projectId}
          onClose={() => setCreating(false)}
        />
      )}
      {renaming && (
        <EnvironmentNameDialog
          projectId={projectId}
          environment={renaming}
          onClose={() => setRenaming(null)}
        />
      )}
      {archiving && (
        <ArchiveEnvironmentDialog
          projectId={projectId}
          environment={archiving}
          onClose={() => setArchiving(null)}
        />
      )}
    </div>
  );
}

/** Create or rename an environment (single text field). */
function EnvironmentNameDialog({
  projectId,
  environment,
  onClose,
}: {
  projectId: string;
  environment?: EnvironmentInfo;
  onClose: () => void;
}) {
  const isEdit = !!environment;
  const [name, setName] = useState(environment?.name ?? "");
  const create = useCreateEnvironment(projectId);
  const rename = useRenameEnvironment(projectId);
  const busy = create.isPending || rename.isPending;

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;
    if (isEdit && environment) {
      await rename.mutateAsync({ envId: environment.id, name: trimmed });
    } else {
      await create.mutateAsync(trimmed);
    }
    onClose();
  }

  return (
    <Modal
      title={isEdit ? "Renommer l'environnement" : "Nouvel environnement"}
      onClose={onClose}
      width="max-w-sm"
    >
      <form onSubmit={submit} className="space-y-4">
        <input
          className={inputClass}
          autoFocus
          placeholder="Ex. staging"
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

function ArchiveEnvironmentDialog({
  projectId,
  environment,
  onClose,
}: {
  projectId: string;
  environment: EnvironmentInfo;
  onClose: () => void;
}) {
  const archive = useArchiveEnvironment(projectId);
  return (
    <ConfirmDialog
      title="Archiver cet environnement ?"
      message={`« ${environment.name} » et ses identifiants seront masqués. Les données sont conservées (archivage réversible).`}
      confirmLabel="Archiver"
      busy={archive.isPending}
      onConfirm={() => {
        archive.mutate(environment.id);
        onClose();
      }}
      onClose={onClose}
    />
  );
}
