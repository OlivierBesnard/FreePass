import { useState } from "react";
import type { EnvironmentInfo } from "../lib/api";
import { useEnvironments } from "../hooks/useProjects";
import { useDuplicateEntryToEnvironment } from "../hooks/useVault";
import { Modal } from "./Modal";
import { Button } from "./ui";

/**
 * Copy an entry into another environment of the same project. The duplicate is
 * sealed independently under the target env's own envKey (no cross-env move).
 */
export function DuplicateToEnvironment({
  projectId,
  sourceEnvId,
  entryId,
  entryTitle,
  onClose,
  onDone,
}: {
  projectId: string;
  sourceEnvId: string;
  entryId: string;
  entryTitle: string;
  onClose: () => void;
  onDone: () => void;
}) {
  const { data: environments = [], isLoading } = useEnvironments(projectId);
  const duplicate = useDuplicateEntryToEnvironment(sourceEnvId);
  const [targetId, setTargetId] = useState<string | null>(null);

  const targets: EnvironmentInfo[] = environments.filter(
    (e) => e.id !== sourceEnvId,
  );

  async function submit() {
    if (!targetId) return;
    await duplicate.mutateAsync({ entryId, targetEnvId: targetId });
    onDone();
  }

  return (
    <Modal title="Dupliquer vers un environnement" onClose={onClose} width="max-w-sm">
      <p className="mb-4 text-sm text-ink-600">
        Copier « {entryTitle} » dans un autre environnement de ce projet.
      </p>
      {isLoading ? (
        <p className="text-sm text-ink-500">Chargement…</p>
      ) : targets.length === 0 ? (
        <p className="py-2 text-sm text-ink-400">
          Aucun autre environnement disponible. Créez-en un d'abord.
        </p>
      ) : (
        <ul className="space-y-2">
          {targets.map((env) => {
            const active = env.id === targetId;
            return (
              <li key={env.id}>
                <button
                  onClick={() => setTargetId(env.id)}
                  className={
                    "flex w-full items-center rounded-xl border px-4 py-2.5 text-left text-sm font-medium transition-colors " +
                    (active
                      ? "border-brand-400 bg-brand-100 text-brand-700"
                      : "border-cream-400 bg-card text-ink-700 hover:bg-cream-300")
                  }
                >
                  {env.name}
                </button>
              </li>
            );
          })}
        </ul>
      )}

      <div className="mt-5 flex justify-end gap-2">
        <button
          onClick={onClose}
          className="inline-flex h-10 items-center rounded-lg border border-cream-400 px-4 text-sm font-medium text-ink-600 transition-colors hover:bg-cream-300"
        >
          Annuler
        </button>
        <Button
          onClick={submit}
          disabled={!targetId || duplicate.isPending}
        >
          {duplicate.isPending ? "Duplication…" : "Dupliquer"}
        </Button>
      </div>
    </Modal>
  );
}
