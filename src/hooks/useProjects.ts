import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { toast } from "sonner";
import { api, errorMessage } from "../lib/api";

/** All non-archived projects (sorted by name in Rust). */
export function useProjects() {
  return useQuery({
    queryKey: ["projects"],
    queryFn: api.listProjects,
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

export function useCreateProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (name: string) => api.createProject(name),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["projects"] });
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
      void qc.invalidateQueries({ queryKey: ["projects"] });
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
      void qc.invalidateQueries({ queryKey: ["projects"] });
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
      void qc.invalidateQueries({ queryKey: ["environments", projectId] });
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
      void qc.invalidateQueries({ queryKey: ["environments", projectId] });
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
      void qc.invalidateQueries({ queryKey: ["environments", projectId] });
      toast.success("Environnement archivé.");
    },
    onError: (e) => toast.error(errorMessage(e)),
  });
}
