-- Phase 2 — environments + entries schema for the local vault.
--
-- Key hierarchy (CRYPTO_SPEC.md §3): masterKey -> vaultKey -> envKey. Each
-- environment owns a random envKey, sealed under the vaultKey. Entry fields are
-- encrypted under their environment's envKey (CRYPTO_SPEC §4), each field as an
-- independent {nonce, ciphertext} bound by AAD to env_id + entry_id + field_name.
--
-- Mono-user invariant (DESIGN §6): no per-user / owner / tenant columns. Only
-- env_id / entry_id appear, which are object identifiers, not principals.

CREATE TABLE environments (
    id              TEXT PRIMARY KEY NOT NULL,   -- UUID v4
    name            TEXT NOT NULL,               -- clear metadata (e.g. "Personnel")
    env_key_wrapped BLOB NOT NULL,               -- envKey sealed under vaultKey
    env_key_nonce   BLOB NOT NULL,               -- AEAD nonce for env_key_wrapped
    created_at      TEXT NOT NULL,               -- RFC3339
    updated_at      TEXT NOT NULL,
    archived_at     TEXT                         -- soft-delete (nullable)
);

CREATE TABLE entries (
    id          TEXT PRIMARY KEY NOT NULL,       -- UUID v4
    env_id      TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    type        TEXT NOT NULL,                   -- 'login' | 'secret' | 'env_var'
    title       TEXT NOT NULL,                   -- clear metadata
    url         TEXT,                            -- clear metadata (login: autofill match)
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    archived_at TEXT
);

CREATE INDEX idx_entries_env ON entries (env_id);

-- One row per encrypted field. field_name is clear (e.g. 'username', 'password',
-- 'notes', 'value', or an env-var key); only the value is encrypted. The AAD that
-- protects each row binds env_id + entry_id + field_name (anti-swap, THREAT F8).
CREATE TABLE entry_fields (
    entry_id   TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    field_name TEXT NOT NULL,
    nonce      BLOB NOT NULL,
    ciphertext BLOB NOT NULL,
    PRIMARY KEY (entry_id, field_name)
);
