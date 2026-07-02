import { invoke } from "@tauri-apps/api/core";

/** What the UI needs to choose a screen (mirrors the Rust `VaultStatus`). */
export interface VaultStatus {
  initialized: boolean;
  unlocked: boolean;
}

export interface ProjectInfo {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
}

export interface EnvironmentInfo {
  id: string;
  name: string;
  project_id: string | null;
}

export interface ChannelInfo {
  port: number;
  token: string;
}

export interface EntrySummary {
  id: string;
  env_id: string;
  type: string;
  title: string;
  url: string | null;
  updated_at: string;
  /**
   * Name of the owning environment (clear metadata, never a secret). Present on
   * the unified `list_all_entries` view; may be null for legacy/per-env lists.
   */
  env_name: string | null;
}

export interface EntryDetail {
  id: string;
  env_id: string;
  type: string;
  title: string;
  url: string | null;
  username: string | null;
  password: string | null;
  notes: string | null;
  icon: string | null;
  created_at: string;
  updated_at: string;
}

export interface EntryInput {
  title: string;
  url: string | null;
  username: string | null;
  password: string | null;
  notes: string | null;
}

/**
 * Thin wrappers around Tauri IPC. All vault crypto lives in Rust; the frontend
 * only passes values through and keeps nothing in persistent storage.
 */
export const api = {
  vaultStatus: (): Promise<VaultStatus> => invoke("vault_status"),
  createVault: (masterPassword: string): Promise<void> =>
    invoke("create_vault", { masterPassword }),
  unlock: (masterPassword: string): Promise<void> =>
    invoke("unlock", { masterPassword }),
  lock: (): Promise<void> => invoke("lock"),
  changeMasterPassword: (
    currentPassword: string,
    newPassword: string,
  ): Promise<void> =>
    invoke("change_master_password", { currentPassword, newPassword }),

  localChannelInfo: (): Promise<ChannelInfo | null> =>
    invoke("local_channel_info"),

  defaultEnvironment: (): Promise<EnvironmentInfo> =>
    invoke("default_environment"),

  // Projects (Phase 10) — clear metadata grouping environments.
  createProject: (name: string): Promise<ProjectInfo> =>
    invoke("create_project", { name }),
  listProjects: (): Promise<ProjectInfo[]> => invoke("list_projects"),
  renameProject: (projectId: string, name: string): Promise<void> =>
    invoke("rename_project", { projectId, name }),
  archiveProject: (projectId: string): Promise<void> =>
    invoke("archive_project", { projectId }),

  // Environments (Phase 10) — each owns its own entries under a fresh envKey.
  createEnvironment: (
    projectId: string,
    name: string,
  ): Promise<EnvironmentInfo> =>
    invoke("create_environment", { projectId, name }),
  listEnvironments: (projectId: string): Promise<EnvironmentInfo[]> =>
    invoke("list_environments", { projectId }),
  renameEnvironment: (envId: string, name: string): Promise<void> =>
    invoke("rename_environment", { envId, name }),
  archiveEnvironment: (envId: string): Promise<void> =>
    invoke("archive_environment", { envId }),
  /**
   * Unified flat list of entries across ALL live environments (non-archived
   * env + non-archived project), each carrying its `env_name`. Optional local
   * search filters on title/url server-side.
   */
  listAllEntries: (search?: string): Promise<EntrySummary[]> =>
    invoke("list_all_entries", { search: search ?? null }),
  getEntry: (envId: string, entryId: string): Promise<EntryDetail> =>
    invoke("get_entry", { envId, entryId }),
  createEntry: (envId: string, input: EntryInput): Promise<string> =>
    invoke("create_entry", { envId, input }),
  updateEntry: (
    envId: string,
    entryId: string,
    input: EntryInput,
  ): Promise<void> => invoke("update_entry", { envId, entryId, input }),
  archiveEntry: (envId: string, entryId: string): Promise<void> =>
    invoke("archive_entry", { envId, entryId }),
  listArchivedEntries: (envId: string): Promise<EntrySummary[]> =>
    invoke("list_archived_entries", { envId }),
  restoreEntry: (envId: string, entryId: string): Promise<void> =>
    invoke("restore_entry", { envId, entryId }),
  deleteEntry: (envId: string, entryId: string): Promise<void> =>
    invoke("delete_entry", { envId, entryId }),

  importEntries: (envId: string, entries: EntryInput[]): Promise<number> =>
    invoke("import_entries", { envId, entries }),

  /** Fetch + store the site favicon for an entry (best-effort, direct-to-site). */
  refreshEntryIcon: (
    envId: string,
    entryId: string,
    url: string,
  ): Promise<string | null> =>
    invoke("refresh_entry_icon", { envId, entryId, url }),
  /** All stored favicons for the environment, keyed by entry id. */
  entryIcons: (envId: string): Promise<Record<string, string>> =>
    invoke("entry_icons", { envId }),

  generatePassword: (opts: GeneratorOptions): Promise<string> =>
    invoke("generate_password", { ...opts }),
};

export interface GeneratorOptions {
  length: number;
  lowercase: boolean;
  uppercase: boolean;
  digits: boolean;
  symbols: boolean;
}

/** Tauri rejects with the serialized `AppError` (a plain string). */
export function errorMessage(e: unknown): string {
  return typeof e === "string" ? e : "Une erreur est survenue.";
}
