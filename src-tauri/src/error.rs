use std::path::PathBuf;

use serde::{Serialize, Serializer};

pub type AppResult<T> = std::result::Result<T, AppError>;

/// Application error type. Crosses the Tauri IPC boundary, so it serializes to a
/// plain JSON string (the human-readable message). It must NEVER carry secret
/// material (master password, keys, plaintext secrets) — cf. CLAUDE.md security
/// rules. Phase 0 has no crypto yet, but the rule is enforced from the start.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),

    #[error("could not resolve app data directory")]
    AppDataDirUnavailable,

    #[error("invalid path: {0}")]
    InvalidPath(PathBuf),

    #[error("le coffre est déjà initialisé")]
    VaultAlreadyInitialized,

    #[error("aucun coffre n'a encore été créé")]
    VaultNotInitialized,

    #[error("le coffre est verrouillé")]
    VaultLocked,

    #[error("environnement introuvable")]
    EnvironmentNotFound,

    #[error("élément introuvable")]
    NotFound,

    #[error("conflit : {0}")]
    Conflict(String),

    /// Any crypto failure. The message is **generic on purpose** (anti-oracle,
    /// THREAT F5): the underlying `CryptoError` (wrong password vs tampered blob
    /// vs weak params) is kept as the error source for logs/`Debug` but never
    /// surfaced to the IPC boundary. Phase 2 maps this to a single neutral
    /// user-facing string at the command layer.
    #[error("opération de coffre invalide")]
    Crypto(#[from] crate::crypto::CryptoError),

    #[error("{0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_a_plain_json_string() {
        let err = AppError::Other("boom".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"boom\"");
    }

    #[test]
    fn database_variant_includes_underlying_error_message() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let err = AppError::from(sqlx_err);
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("database error"));
    }

    #[test]
    fn app_data_dir_unavailable_has_a_human_message() {
        let err = AppError::AppDataDirUnavailable;
        assert_eq!(err.to_string(), "could not resolve app data directory");
    }

    #[test]
    fn invalid_path_keeps_the_path_in_the_message() {
        let err = AppError::InvalidPath(PathBuf::from("/no/such/place"));
        assert!(err.to_string().contains("/no/such/place"));
    }

    #[test]
    fn io_error_converts_via_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "disk on fire");
        let err: AppError = io_err.into();
        let s = err.to_string();
        assert!(s.contains("filesystem error"));
        assert!(s.contains("disk on fire"));
    }
}
