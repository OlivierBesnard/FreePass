//! Crypto error type (CRYPTO_SPEC.md).
//!
//! Variants are deliberately coarse and their `Display` is **generic**: a
//! decryption failure must never reveal *why* it failed (wrong password vs
//! tampered blob vs wrong AAD), which would be an oracle (THREAT_MODEL F5).
//! The internal variant distinction exists for control flow and `Debug`/logs,
//! never for a user-facing message.

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// Key derivation (Argon2id) failed for a non-policy reason.
    #[error("key derivation failed")]
    Kdf,

    /// Argon2id parameters are below the security floor (CRYPTO_SPEC §2). The
    /// client must refuse them rather than derive a weak key (THREAT F4).
    #[error("argon2 parameters below the security floor")]
    WeakParams,

    /// AEAD encryption failed.
    #[error("encryption failed")]
    Encrypt,

    /// AEAD decryption/authentication failed. **Generic on purpose** (F5): do
    /// not distinguish wrong key, tampered ciphertext, or wrong AAD here.
    #[error("decryption failed")]
    Decrypt,

    /// A key/slice did not have the expected length (32 bytes for keys).
    #[error("invalid key length")]
    KeyLength,
}
