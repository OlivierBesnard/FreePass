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

/**
 * Unified flat list of entries across ALL live environments, optionally
 * filtered by a local search. Backs the grouped-by-site home view.
 */
export function useAllEntries(search: string) {
  return useQuery({
    queryKey: ["all-entries", search],
    queryFn: () => api.listAllEntries(search || undefined),
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
 * Merged favicon map across several environments, keyed by entry id. Backs the
 * unified list, whose entries span multiple environments. Entry ids are unique
 * across environments, so a flat merge is unambiguous.
 */
export function useAllEntryIcons(envIds: string[]) {
  // Stable key independent of order so the cache hits regardless of list order.
  const sorted = [...new Set(envIds)].sort();
  return useQuery({
    queryKey: ["allEntryIcons", sorted],
    queryFn: async (): Promise<Record<string, string>> => {
      const maps = await Promise.all(sorted.map((id) => api.entryIcons(id)));
      return Object.assign({}, ...maps) as Record<string, string>;
    },
    enabled: sorted.length > 0,
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
      void qc.invalidateQueries({ queryKey: ["allEntryIcons"] });
      void qc.invalidateQueries({ queryKey: ["entry", envId, entryId] });
    })
    .catch(() => {});
}

/**
 * Invalidate both the per-environment lists and the unified cross-env list so
 * the grouped home view and any open environment view stay in sync.
 */
function invalidateEntryLists(qc: ReturnType<typeof useQueryClient>) {
  void qc.invalidateQueries({ queryKey: ["entries"] });
  void qc.invalidateQueries({ queryKey: ["all-entries"] });
}

function useEntryInvalidation() {
  const qc = useQueryClient();
  return () => invalidateEntryLists(qc);
}

export function useCreateEntry(envId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: EntryInput) => api.createEntry(envId as string, input),
    onSuccess: (newId, input) => {
      invalidateEntryLists(qc);
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
      invalidateEntryLists(qc);
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
      invalidateEntryLists(qc);
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

/**
 * Archived entries aggregated across several environments (the unified trash).
 * Each summary keeps its own `env_id`, so restore/delete can target the right
 * environment without the user navigating a hierarchy.
 */
export function useAllArchivedEntries(envIds: string[]) {
  const sorted = [...new Set(envIds)].sort();
  return useQuery({
    queryKey: ["allArchived", sorted],
    queryFn: async () => {
      const lists = await Promise.all(
        sorted.map((id) => api.listArchivedEntries(id)),
      );
      return lists.flat();
    },
    enabled: sorted.length > 0,
  });
}

function invalidateLists(qc: ReturnType<typeof useQueryClient>) {
  invalidateEntryLists(qc);
  void qc.invalidateQueries({ queryKey: ["archived"] });
  void qc.invalidateQueries({ queryKey: ["allArchived"] });
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

/**
 * Restore an archived entry, taking its environment per call (the unified trash
 * spans several environments).
 */
export function useRestoreEntryAt() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { envId: string; entryId: string }) =>
      api.restoreEntry(args.envId, args.entryId),
    onSuccess: () => {
      invalidateLists(qc);
      toast.success("Identifiant restauré.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}

/** Permanently delete an archived entry, taking its environment per call. */
export function useDeleteEntryAt() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { envId: string; entryId: string }) =>
      api.deleteEntry(args.envId, args.entryId),
    onSuccess: () => {
      invalidateLists(qc);
      toast.success("Identifiant supprimé définitivement.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}
