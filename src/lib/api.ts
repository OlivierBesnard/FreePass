import { invoke } from "@tauri-apps/api/core";

/** What the UI needs to choose a screen (mirrors the Rust `VaultStatus`). */
export interface VaultStatus {
  initialized: boolean;
  unlocked: boolean;
}

export interface EnvironmentInfo {
  id: string;
  name: string;
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
  listEntries: (envId: string, search?: string): Promise<EntrySummary[]> =>
    invoke("list_entries", { envId, search: search ?? null }),
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
