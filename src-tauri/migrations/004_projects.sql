-- Phase 10 — projects: a clear-metadata grouping layer above environments.
--
-- Additive only (DESIGN §4/§10, CRYPTO_SPEC §3): NO crypto change. A project is
-- pure clear metadata; the key hierarchy (masterKey -> vaultKey -> envKey) and
-- the per-entry AAD (env_id + entry_id + field_name) are untouched. Environments
-- keep their own entries and their own envKey; we only attach an environment to a
-- project via a nullable foreign key.
--
-- Mono-user invariant (DESIGN §6): `project_id` is an OBJECT identifier (like
-- env_id / entry_id), never a principal. No user / owner / tenant column.

CREATE TABLE projects (
    id          TEXT PRIMARY KEY NOT NULL,   -- UUID v4
    name        TEXT NOT NULL,               -- clear metadata (free FR text)
    created_at  TEXT NOT NULL,               -- RFC3339
    updated_at  TEXT NOT NULL,
    archived_at TEXT                         -- soft-delete (nullable)
);

-- Attach an environment to a project. Nullable while the startup backfill runs;
-- afterwards every environment points at the default "Personnel" project.
ALTER TABLE environments ADD COLUMN project_id TEXT REFERENCES projects(id);

CREATE INDEX idx_environments_project ON environments (project_id);
