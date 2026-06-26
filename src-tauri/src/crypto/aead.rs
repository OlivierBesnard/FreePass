//! XChaCha20-Poly1305 AEAD primitives (CRYPTO_SPEC.md §1, §4).
//!
//! Low-level seal/open over a 32-byte key, a fresh 24-byte random nonce, and
//! associated data (AAD). All higher-level operations (wrapping keys, encrypting
//! entry fields) are built on these in `crypto::mod`.

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305, XNonce};
use rand::rngs::OsRng;
use rand::RngCore;
use zeroize::Zeroizing;

use crate::crypto::error::CryptoError;
use crate::crypto::keys::SecretKey;

/// XChaCha20 nonce length in bytes (CRYPTO_SPEC §4).
pub const NONCE_LEN: usize = 24;

/// Generate a fresh 24-byte random nonce from the OS CSPRNG (CRYPTO_SPEC §5).
/// A fresh nonce is mandatory for every seal — never reuse (THREAT F10).
pub fn generate_nonce() -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

/// Seal `plaintext` under `key` + `nonce`, binding `aad`. Returns the ciphertext
/// (which includes the Poly1305 tag).
pub fn seal(
    key: &SecretKey,
    nonce: &[u8; NONCE_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key.expose())
        .map_err(|_| CryptoError::KeyLength)?;
    cipher
        .encrypt(XNonce::from_slice(nonce), Payload { msg: plaintext, aad })
        .map_err(|_| CryptoError::Encrypt)
}

/// Open `ciphertext` under `key` + `nonce`, verifying `aad`. Returns a zeroizing
/// plaintext. Any mismatch (wrong key, tampered ciphertext/nonce, wrong AAD)
/// fails as a **generic** `Decrypt` error — no oracle (THREAT F5, F8).
pub fn open(
    key: &SecretKey,
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key.expose())
        .map_err(|_| CryptoError::KeyLength)?;
    let plaintext = cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ciphertext, aad })
        .map_err(|_| CryptoError::Decrypt)?;
    Ok(Zeroizing::new(plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_then_open_roundtrips() {
        let key = SecretKey::from_bytes([9u8; 32]);
        let nonce = [4u8; NONCE_LEN];
        let ct = seal(&key, &nonce, b"hello", b"aad").unwrap();
        let pt = open(&key, &nonce, &ct, b"aad").unwrap();
        assert_eq!(&pt[..], b"hello");
    }

    #[test]
    fn seal_is_deterministic_for_fixed_key_nonce_aad() {
        // Locks the algorithm + nonce handling: identical inputs -> identical bytes.
        let key = SecretKey::from_bytes([1u8; 32]);
        let nonce = [2u8; NONCE_LEN];
        let a = seal(&key, &nonce, b"msg", b"aad").unwrap();
        let b = seal(&key, &nonce, b"msg", b"aad").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn open_fails_on_wrong_aad() {
        let key = SecretKey::from_bytes([9u8; 32]);
        let nonce = [4u8; NONCE_LEN];
        let ct = seal(&key, &nonce, b"hello", b"aad-1").unwrap();
        assert!(matches!(
            open(&key, &nonce, &ct, b"aad-2"),
            Err(CryptoError::Decrypt)
        ));
    }

    #[test]
    fn open_fails_on_tampered_ciphertext() {
        let key = SecretKey::from_bytes([9u8; 32]);
        let nonce = [4u8; NONCE_LEN];
        let mut ct = seal(&key, &nonce, b"hello", b"aad").unwrap();
        ct[0] ^= 0x01; // flip one bit
        assert!(open(&key, &nonce, &ct, b"aad").is_err());
    }

    #[test]
    fn open_fails_on_tampered_nonce() {
        let key = SecretKey::from_bytes([9u8; 32]);
        let nonce = [4u8; NONCE_LEN];
        let ct = seal(&key, &nonce, b"hello", b"aad").unwrap();
        let mut bad_nonce = nonce;
        bad_nonce[0] ^= 0x01;
        assert!(open(&key, &bad_nonce, &ct, b"aad").is_err());
    }

    #[test]
    fn open_fails_on_wrong_key() {
        let nonce = [4u8; NONCE_LEN];
        let ct = seal(&SecretKey::from_bytes([9u8; 32]), &nonce, b"hello", b"aad").unwrap();
        assert!(open(&SecretKey::from_bytes([8u8; 32]), &nonce, &ct, b"aad").is_err());
    }
}
