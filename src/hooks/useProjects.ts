import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { toast } from "sonner";
import { api, errorMessage, type EnvironmentInfo } from "../lib/api";

/** All non-archived projects (sorted by name in Rust). */
export function useProjects() {
  return useQuery({
    queryKey: ["projects"],
    queryFn: api.listProjects,
  });
}

/**
 * Flat lookup of every live environment across every live project, keyed by
 * environment id. Lets the unified entry list resolve an entry's owning project
 * (for edit / duplicate) without the user navigating a hierarchy.
 */
export function useAllEnvironments() {
  return useQuery({
    queryKey: ["all-environments"],
    queryFn: async (): Promise<Record<string, EnvironmentInfo>> => {
      const projects = await api.listProjects();
      const lists = await Promise.all(
        projects.map((p) => api.listEnvironments(p.id)),
      );
      const map: Record<string, EnvironmentInfo> = {};
      for (const list of lists) {
        for (const env of list) map[env.id] = env;
      }
      return map;
    },
  });
}

/** Non-archived environments of a project. */
export function useEnvironments(projectId: string | undefined) {
  return useQuery({
    queryKey: ["environments", projectId],
    queryFn: () => api.listEnvironments(projectId as string),
    enabled: !!projectId,
  });
}

/**
 * Invalidate the project/environment caches AND the unified entry list, since
 * creating/renaming/archiving a project or environment changes what the grouped
 * home view shows (badges, available targets, the env lookup).
 */
function invalidateProjectTree(
  qc: ReturnType<typeof useQueryClient>,
  projectId?: string,
) {
  void qc.invalidateQueries({ queryKey: ["projects"] });
  void qc.invalidateQueries({ queryKey: ["all-environments"] });
  void qc.invalidateQueries({ queryKey: ["all-entries"] });
  // The default environment ("Add"/"Import" target) can change when a project or
  // environment is archived — refresh it so those actions never target an
  // archived env (B9, complements the B1 backend fix).
  void qc.invalidateQueries({ queryKey: ["environment"] });
  if (projectId) {
    void qc.invalidateQueries({ queryKey: ["environments", projectId] });
  } else {
    void qc.invalidateQueries({ queryKey: ["environments"] });
  }
}

export function useCreateProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (name: string) => api.createProject(name),
    onSuccess: () => {
      invalidateProjectTree(qc);
      toast.success("Projet créé.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}

export function useRenameProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { projectId: string; name: string }) =>
      api.renameProject(args.projectId, args.name),
    onSuccess: () => {
      invalidateProjectTree(qc);
      toast.success("Projet renommé.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}

export function useArchiveProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (projectId: string) => api.archiveProject(projectId),
    onSuccess: () => {
      invalidateProjectTree(qc);
      toast.success("Projet archivé.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}

export function useCreateEnvironment(projectId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (name: string) =>
      api.createEnvironment(projectId as string, name),
    onSuccess: () => {
      invalidateProjectTree(qc, projectId);
      toast.success("Environnement créé.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}

export function useRenameEnvironment(projectId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (args: { envId: string; name: string }) =>
      api.renameEnvironment(args.envId, args.name),
    onSuccess: () => {
      invalidateProjectTree(qc, projectId);
      toast.success("Environnement renommé.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}

export function useArchiveEnvironment(projectId: string | undefined) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (envId: string) => api.archiveEnvironment(envId),
    onSuccess: () => {
      invalidateProjectTree(qc, projectId);
      toast.success("Environnement archivé.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}
