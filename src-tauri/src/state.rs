use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use sqlx::SqlitePool;

use crate::crypto::SecretKey;

/// Shared application state managed by `tauri::Builder::manage`.
pub struct AppState {
    pub pool: SqlitePool,
    /// OS app-data directory
    /// (e.g. `%APPDATA%\com.freepass.desktop` on Windows).
    pub app_data_dir: PathBuf,
    /// In-memory vault session. Inner `None` means locked; keys live here only
    /// while unlocked and are zeroized on lock/quit (THREAT F3). Never persisted.
    pub session: Mutex<VaultSession>,
}

impl AppState {
    pub fn new(pool: SqlitePool, app_data_dir: PathBuf) -> Self {
        Self {
            pool,
            app_data_dir,
            session: Mutex::new(VaultSession::default()),
        }
    }
}

/// Holds the unlocked vault key (and a lazy cache of environment keys) for as
/// long as the vault is unlocked. Dropping the inner `UnlockedVault` zeroizes
/// every key it owns (`SecretKey` is `ZeroizeOnDrop`).
#[derive(Default)]
pub struct VaultSession {
    inner: Option<UnlockedVault>,
}

struct UnlockedVault {
    vault_key: SecretKey,
    /// env_id -> envKey, unwrapped lazily as environments are touched.
    env_keys: HashMap<String, SecretKey>,
}

impl VaultSession {
    /// Open the session with a freshly recovered vault key.
    pub fn unlock(&mut self, vault_key: SecretKey) {
        self.inner = Some(UnlockedVault {
            vault_key,
            env_keys: HashMap::new(),
        });
    }

    /// Lock: drop all key material (zeroized on drop, THREAT F3).
    pub fn lock(&mut self) {
        self.inner = None;
    }

    pub fn is_unlocked(&self) -> bool {
        self.inner.is_some()
    }

    /// Borrow the vault key if unlocked.
    pub fn vault_key(&self) -> Option<&SecretKey> {
        self.inner.as_ref().map(|u| &u.vault_key)
    }

    /// Cache an unwrapped environment key for reuse until lock.
    pub fn cache_env_key(&mut self, env_id: String, env_key: SecretKey) {
        if let Some(inner) = self.inner.as_mut() {
            inner.env_keys.insert(env_id, env_key);
        }
    }

    /// Borrow a cached environment key if present.
    pub fn env_key(&self, env_id: &str) -> Option<&SecretKey> {
        self.inner.as_ref().and_then(|u| u.env_keys.get(env_id))
    }
}
