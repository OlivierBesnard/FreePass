//! Secret key material (CRYPTO_SPEC.md §3, §6).
//!
//! `SecretKey` holds 32 bytes of key material (masterKey / vaultKey / envKey)
//! and is **zeroized on drop** (THREAT F3). It intentionally implements neither
//! `Debug` nor `Display` nor `Serialize`, so a key can never be accidentally
//! printed, logged, or sent across the IPC boundary.

use rand::rngs::OsRng;
use rand::RngCore;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::error::CryptoError;

/// Length in bytes of every symmetric key in FreePass.
pub const KEY_LEN: usize = 32;

/// A 32-byte symmetric key, wiped from memory when dropped.
///
/// Used for the master-derived key, the vault key, and per-environment keys.
/// No `Debug`/`Serialize` on purpose — keys must never be printed or persisted
/// in the clear (THREAT F2/F3/F5).
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretKey([u8; KEY_LEN]);

impl SecretKey {
    /// Wrap raw bytes (e.g. an Argon2id output) into a zeroizing key.
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Build a key from a slice, checking the length. The caller's slice is the
    /// caller's responsibility to zeroize (typically a `Zeroizing<Vec<u8>>`).
    pub fn from_slice(bytes: &[u8]) -> Result<Self, CryptoError> {
        let arr: [u8; KEY_LEN] = bytes.try_into().map_err(|_| CryptoError::KeyLength)?;
        Ok(Self(arr))
    }

    /// Generate a fresh random key from the OS CSPRNG (CRYPTO_SPEC §5).
    pub fn generate() -> Self {
        let mut bytes = [0u8; KEY_LEN];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Borrow the raw key bytes. `pub(crate)` so only the crypto layer (and its
    /// tests) can reach the material — never exposed to commands/IPC.
    pub(crate) fn expose(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}
