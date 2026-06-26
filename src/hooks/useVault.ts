import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { toast } from "sonner";
import { api, errorMessage, type EntryInput } from "../lib/api";

/** The (single, in v1) default environment. */
export function useEnvironment() {
  return useQuery({
    queryKey: ["environment"],
    queryFn: api.defaultEnvironment,
  });
}

/** Entry summaries for an environment, filtered by an optional local search. */
export function useEntries(envId: string | undefined, search: string) {
  return useQuery({
    queryKey: ["entries", envId, search],
    queryFn: () => api.listEntries(envId as string, search || undefined),
    enabled: !!envId,
  });
}

/** Full decrypted entry, fetched only when an entry is opened. */
export function useEntry(envId: string | undefined, entryId: string | null) {
  return useQuery({
    queryKey: ["entry", envId, entryId],
    queryFn: () => api.getEntry(envId as string, entryId as string),
    enabled: !!envId && !!entryId,
  });
}

function useEntryInvalidation() {
  const qc = useQueryClient();
  return () => qc.invalidateQueries({ queryKey: ["entries"] });
}

export function useCreateEntry(envId: string | undefined) {
  const invalidate = useEntryInvalidation();
  return useMutation({
    mutationFn: (input: EntryInput) => api.createEntry(envId as string, input),
    onSuccess: () => {
      void invalidate();
      toast.success("Identifiant ajouté.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}

export function useUpdateEntry(envId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { entryId: string; input: EntryInput }) =>
      api.updateEntry(envId as string, args.entryId, args.input),
    onSuccess: (_data, args) => {
      void qc.invalidateQueries({ queryKey: ["entries"] });
      void qc.invalidateQueries({ queryKey: ["entry", envId, args.entryId] });
      toast.success("Identifiant mis à jour.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}

export function useImportEntries(envId: string | undefined) {
  const invalidate = useEntryInvalidation();
  return useMutation({
    mutationFn: (entries: EntryInput[]) =>
      api.importEntries(envId as string, entries),
    onSuccess: (count) => {
      void invalidate();
      toast.success(`${count} identifiant${count > 1 ? "s" : ""} importé${count > 1 ? "s" : ""}.`);
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}

export function useArchiveEntry(envId: string | undefined) {
  const invalidate = useEntryInvalidation();
  return useMutation({
    mutationFn: (entryId: string) => api.archiveEntry(envId as string, entryId),
    onSuccess: () => {
      void invalidate();
      toast.success("Identifiant archivé.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}
