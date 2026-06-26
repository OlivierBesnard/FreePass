-- Phase 0 — minimal base schema for the mono-user, local vault.
--
-- This migration creates the singleton `vault` row that will, in Phase 2, hold
-- the Argon2id KDF parameters and the wrapped vaultKey. In Phase 0 there is NO
-- crypto: every crypto-metadata column is nullable and stays NULL. The table
-- exists only so the DB has a base schema and a place to record the vault's
-- "initialized" state later.
--
-- Mono-user invariant (DESIGN §6): no user_id / owner_id / tenant_id /
-- created_by / author_id / assignee — a guard test scans this file.

CREATE TABLE vault (
    -- Singleton: there is exactly one vault per database file.
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),

    -- Crypto metadata — populated at vault init (Phase 2), NULL for now.
    -- No values are written here in Phase 0; the schema is just reserved.
    kdf_salt          BLOB,    -- Argon2id salt
    kdf_params        TEXT,    -- Argon2id parameters (JSON: m, t, p)
    vault_key_wrapped BLOB,    -- vaultKey sealed under the master-derived key
    vault_key_nonce   BLOB,    -- AEAD nonce for vault_key_wrapped

    -- Lifecycle metadata (clear).
    initialized_at TEXT,       -- RFC3339, set when the vault is created
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);

-- Seed the single vault row. Crypto columns intentionally left NULL until the
-- vault is initialized with a master password (Phase 2).
INSERT INTO vault (id, created_at, updated_at)
VALUES (1, '1970-01-01T00:00:00Z', '1970-01-01T00:00:00Z');
