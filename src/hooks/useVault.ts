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

/** Stored favicons for the environment, keyed by entry id (list overlay). */
export function useEntryIcons(envId: string | undefined) {
  return useQuery({
    queryKey: ["entryIcons", envId],
    queryFn: () => api.entryIcons(envId as string),
    enabled: !!envId,
  });
}

/**
 * Fetch + store an entry's favicon in the background, then refresh the icon
 * caches. Best-effort: a failure is silent (the icon is purely cosmetic).
 */
function refreshIcon(
  qc: ReturnType<typeof useQueryClient>,
  envId: string,
  entryId: string,
  url: string | null,
) {
  if (!url) return;
  void api
    .refreshEntryIcon(envId, entryId, url)
    .then(() => {
      void qc.invalidateQueries({ queryKey: ["entryIcons", envId] });
      void qc.invalidateQueries({ queryKey: ["entry", envId, entryId] });
    })
    .catch(() => {});
}

function useEntryInvalidation() {
  const qc = useQueryClient();
  return () => qc.invalidateQueries({ queryKey: ["entries"] });
}

export function useCreateEntry(envId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: EntryInput) => api.createEntry(envId as string, input),
    onSuccess: (newId, input) => {
      void qc.invalidateQueries({ queryKey: ["entries"] });
      // Grab the site favicon in the background (direct-to-site, encrypted).
      refreshIcon(qc, envId as string, newId, input.url);
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
      // Refresh the favicon in case the URL changed.
      refreshIcon(qc, envId as string, args.entryId, args.input.url);
      toast.success("Identifiant mis à jour.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}

/**
 * Duplicate an entry into another environment of the same project. We read the
 * decrypted source entry, then create a fresh copy under the target env's own
 * envKey (no cross-env re-encryption: the new entry is sealed independently).
 */
export function useDuplicateEntryToEnvironment(sourceEnvId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (args: { entryId: string; targetEnvId: string }) => {
      const detail = await api.getEntry(sourceEnvId as string, args.entryId);
      const input: EntryInput = {
        title: detail.title,
        url: detail.url,
        username: detail.username,
        password: detail.password,
        notes: detail.notes,
      };
      const newId = await api.createEntry(args.targetEnvId, input);
      return { newId, targetEnvId: args.targetEnvId, url: detail.url };
    },
    onSuccess: ({ newId, targetEnvId, url }) => {
      void qc.invalidateQueries({ queryKey: ["entries"] });
      refreshIcon(qc, targetEnvId, newId, url);
      toast.success("Identifiant dupliqué dans l'environnement.");
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

/** Archived ("trash") entries for an environment. */
export function useArchivedEntries(envId: string | undefined) {
  return useQuery({
    queryKey: ["archived", envId],
    queryFn: () => api.listArchivedEntries(envId as string),
    enabled: !!envId,
  });
}

function invalidateLists(qc: ReturnType<typeof useQueryClient>) {
  void qc.invalidateQueries({ queryKey: ["entries"] });
  void qc.invalidateQueries({ queryKey: ["archived"] });
}

export function useArchiveEntry(envId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (entryId: string) => api.archiveEntry(envId as string, entryId),
    onSuccess: () => {
      invalidateLists(qc);
      toast.success("Identifiant archivé.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}

export function useRestoreEntry(envId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (entryId: string) => api.restoreEntry(envId as string, entryId),
    onSuccess: () => {
      invalidateLists(qc);
      toast.success("Identifiant restauré.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}

export function useDeleteEntry(envId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (entryId: string) => api.deleteEntry(envId as string, entryId),
    onSuccess: () => {
      invalidateLists(qc);
      toast.success("Identifiant supprimé définitivement.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}
