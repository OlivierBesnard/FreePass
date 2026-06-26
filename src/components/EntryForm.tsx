import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import type { EntryDetail, EntryInput } from "../lib/api";
import { useCreateEntry, useUpdateEntry } from "../hooks/useVault";
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

/** Create or edit a login entry (RHF + Zod). */
export function EntryForm({
  envId,
  entry,
  onClose,
}: {
  envId: string;
  entry: EntryDetail | null;
  onClose: () => void;
}) {
  const create = useCreateEntry(envId);
  const update = useUpdateEntry(envId);
  const isEdit = entry !== null;

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
