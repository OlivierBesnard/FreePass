import { useEffect, useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import type { EntryDetail, EntryInput } from "../lib/api";
import { useCreateEntry, useUpdateEntry } from "../hooks/useVault";
import {
  useAllEnvironments,
  useEnvironments,
  useProjects,
} from "../hooks/useProjects";
import { Modal } from "./Modal";
import { PasswordGenerator } from "./PasswordGenerator";
import { Button, inputClass } from "./ui";

const schema = z.object({
  title: z.string().trim().min(1, "Le titre est requis"),
  url: z.string().trim().optional(),
  username: z.string().optional(),
  password: z.string().optional(),
  notes: z.string().optional(),
});

type FormValues = z.infer<typeof schema>;

const orNull = (s?: string): string | null => (s && s.length > 0 ? s : null);

/**
 * Create or edit a login entry (RHF + Zod). A new entry lands in the default
 * environment with zero ceremony; an optional, collapsed selector lets advanced
 * users pick another environment of the same project. Editing keeps the entry
 * in its own environment (no cross-env move — each env has its own envKey).
 */
export function EntryForm({
  defaultEnvId,
  entry,
  onClose,
}: {
  /** Target environment for a NEW entry, or the entry's own env when editing. */
  defaultEnvId: string;
  entry: EntryDetail | null;
  onClose: () => void;
}) {
  const isEdit = entry !== null;
  // The environment the entry will be written to. For an edit it's fixed.
  const [targetEnvId, setTargetEnvId] = useState(defaultEnvId);
  const effectiveEnvId = isEdit ? defaultEnvId : targetEnvId;

  const create = useCreateEntry(effectiveEnvId);
  const update = useUpdateEntry(effectiveEnvId);

  const {
    register,
    handleSubmit,
    setValue,
    formState: { errors, isSubmitting },
  } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      title: entry?.title ?? "",
      url: entry?.url ?? "",
      username: entry?.username ?? "",
      password: entry?.password ?? "",
      notes: entry?.notes ?? "",
    },
  });

  async function onSubmit(values: FormValues) {
    const input: EntryInput = {
      title: values.title.trim(),
      url: orNull(values.url?.trim()),
      username: orNull(values.username),
      password: orNull(values.password),
      notes: orNull(values.notes),
    };
    if (isEdit && entry) {
      await update.mutateAsync({ entryId: entry.id, input });
    } else {
      await create.mutateAsync(input);
    }
    onClose();
  }

  return (
    <Modal title={isEdit ? "Modifier l'identifiant" : "Nouvel identifiant"} onClose={onClose}>
      <form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
        <Field label="Titre" error={errors.title?.message}>
          <input className={inputClass} autoFocus placeholder="Ex. GitHub" {...register("title")} />
        </Field>
        <Field label="Site web">
          <input className={inputClass} placeholder="github.com" {...register("url")} />
        </Field>
        <Field label="Identifiant">
          <input className={inputClass} autoComplete="off" placeholder="email ou nom d'utilisateur" {...register("username")} />
        </Field>
        <Field label="Mot de passe">
          <input className={`${inputClass} font-mono`} type="password" autoComplete="new-password" {...register("password")} />
          <PasswordGenerator
            onGenerated={(pw) =>
              setValue("password", pw, { shouldDirty: true, shouldValidate: true })
            }
          />
        </Field>
        <Field label="Notes">
          <textarea className={`${inputClass} h-20 resize-none py-2`} {...register("notes")} />
        </Field>

        {!isEdit && (
          <LocationPicker
            defaultEnvId={defaultEnvId}
            value={targetEnvId}
            onChange={setTargetEnvId}
          />
        )}

        <div className="flex justify-end gap-2 pt-1">
          <button
            type="button"
            onClick={onClose}
            className="inline-flex h-10 items-center rounded-lg border border-cream-400 px-4 text-sm font-medium text-ink-600 transition-colors hover:bg-cream-300"
          >
            Annuler
          </button>
          <Button type="submit" disabled={isSubmitting}>
            {isSubmitting ? "Enregistrement…" : "Enregistrer"}
          </Button>
        </div>
      </form>
    </Modal>
  );
}

/**
 * Optional location selector for a NEW entry: pick its project and environment.
 * Hidden entirely in the zero-ceremony case (a single project with a single
 * environment). The project list lets you file an entry under any project you
 * created — not just the default one. The chosen environment drives where the
 * entry is encrypted (its own envKey).
 */
function LocationPicker({
  defaultEnvId,
  value,
  onChange,
}: {
  defaultEnvId: string;
  value: string;
  onChange: (envId: string) => void;
}) {
  const { data: projects = [] } = useProjects();
  const { data: envMap = {} } = useAllEnvironments();
  const defaultProjectId = envMap[defaultEnvId]?.project_id ?? undefined;

  const [projectId, setProjectId] = useState<string | undefined>(
    defaultProjectId,
  );
  // Adopt the default project once it resolves (first render may be empty).
  useEffect(() => {
    if (!projectId && defaultProjectId) setProjectId(defaultProjectId);
  }, [defaultProjectId, projectId]);

  const { data: environments = [] } = useEnvironments(projectId);

  // Keep the selected environment valid for the chosen project: when the
  // project changes (or loads), snap the target to its first environment.
  useEffect(() => {
    if (environments.length === 0) return;
    if (!environments.some((e) => e.id === value)) {
      onChange(environments[0].id);
    }
  }, [environments, value, onChange]);

  const multiProject = projects.length > 1;
  const multiEnv = environments.length > 1;
  // Nothing to choose: one project, one environment → stay out of the way.
  if (!multiProject && !multiEnv) return null;

  return (
    <div className="space-y-3 rounded-xl border border-cream-400 bg-cream-200/40 p-3">
      <p className="text-sm font-medium text-ink-600">Emplacement</p>
      {multiProject && (
        <div>
          <label className="mb-1 block text-xs font-medium text-ink-500">
            Projet
          </label>
          <select
            className={inputClass}
            value={projectId ?? ""}
            onChange={(e) => setProjectId(e.target.value)}
          >
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </div>
      )}
      {multiEnv && (
        <div>
          <label className="mb-1 block text-xs font-medium text-ink-500">
            Environnement
          </label>
          <select
            className={inputClass}
            value={value}
            onChange={(e) => onChange(e.target.value)}
          >
            {environments.map((env) => (
              <option key={env.id} value={env.id}>
                {env.name}
              </option>
            ))}
          </select>
        </div>
      )}
    </div>
  );
}

function Field({
  label,
  error,
  children,
}: {
  label: string;
  error?: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <label className="mb-1.5 block text-sm font-medium text-ink-700">
        {label}
      </label>
      {children}
      {error && <p className="mt-1 text-xs text-danger-600">{error}</p>}
    </div>
  );
}
