//! FreePass crypto core (CRYPTO_SPEC.md).
//!
//! Implements the frozen v1 contract: a 3-level key hierarchy
//! `masterKey -> vaultKey -> envKey` and per-field AEAD whose AAD binds
//! `env_id + entry_id + field_name` (anti-swap/rollback, THREAT F8).
//!
//! Phase 1 ships only this module + its tests — no Tauri commands, no DB wiring
//! (those land in Phase 2). RustCrypto only; no homemade crypto (THREAT F12).

pub mod aead;
pub mod error;
pub mod kdf;
pub mod keys;

pub use error::CryptoError;
pub use kdf::{
    derive_master_key, generate_salt, KdfParams, DEFAULT_M_COST_KIB, DEFAULT_P_COST,
    DEFAULT_T_COST, SALT_LEN,
};
pub use keys::{SecretKey, KEY_LEN};

use aead::NONCE_LEN;
use zeroize::Zeroizing;

// === AAD domain separators (CRYPTO_SPEC §3-4). Binding the identifiers into
// the AAD is what makes a swap/rollback fail to authenticate. ===

/// AAD for the wrapped vault key.
const AAD_VAULT_KEY: &[u8] = b"freepass:v1:vaultkey";

/// AAD for a wrapped environment key: `freepass:v1:envkey:<env_id>`.
fn aad_env_key(env_id: &str) -> Vec<u8> {
    format!("freepass:v1:envkey:{env_id}").into_bytes()
}

/// AAD for an entry field: `freepass:v1:entry:<env_id>:<entry_id>:<field_name>`.
fn aad_entry_field(env_id: &str, entry_id: &str, field_name: &str) -> Vec<u8> {
    format!("freepass:v1:entry:{env_id}:{entry_id}:{field_name}").into_bytes()
}

/// A nonce + ciphertext pair as stored on disk. Neither value is secret; the
/// confidentiality comes from the key, the integrity from the Poly1305 tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sealed {
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
}

fn seal_with(key: &SecretKey, plaintext: &[u8], aad: &[u8]) -> Result<Sealed, CryptoError> {
    let nonce = aead::generate_nonce();
    let ciphertext = aead::seal(key, &nonce, plaintext, aad)?;
    Ok(Sealed { nonce, ciphertext })
}

// === Level 2: vaultKey wrapped by masterKey (CRYPTO_SPEC §3) ===

/// Seal a freshly generated `vault_key` under the `master_key`.
pub fn wrap_vault_key(master_key: &SecretKey, vault_key: &SecretKey) -> Result<Sealed, CryptoError> {
    seal_with(master_key, vault_key.expose(), AAD_VAULT_KEY)
}

/// Recover the `vault_key` from its wrapped form using the `master_key`.
/// A wrong master password fails as a generic `Decrypt` error (THREAT F5).
pub fn unwrap_vault_key(master_key: &SecretKey, wrapped: &Sealed) -> Result<SecretKey, CryptoError> {
    let raw = aead::open(master_key, &wrapped.nonce, &wrapped.ciphertext, AAD_VAULT_KEY)?;
    SecretKey::from_slice(&raw)
}

// === Level 3: envKey wrapped by vaultKey, bound to env_id (CRYPTO_SPEC §3) ===

/// Seal an environment key under the `vault_key`, binding the `env_id`.
pub fn wrap_env_key(
    vault_key: &SecretKey,
    env_id: &str,
    env_key: &SecretKey,
) -> Result<Sealed, CryptoError> {
    seal_with(vault_key, env_key.expose(), &aad_env_key(env_id))
}

/// Recover an environment key. Using the wrong `env_id` fails authentication.
pub fn unwrap_env_key(
    vault_key: &SecretKey,
    env_id: &str,
    wrapped: &Sealed,
) -> Result<SecretKey, CryptoError> {
    let raw = aead::open(vault_key, &wrapped.nonce, &wrapped.ciphertext, &aad_env_key(env_id))?;
    SecretKey::from_slice(&raw)
}

// === Entry field encryption under envKey (CRYPTO_SPEC §4) ===

/// Encrypt one entry field under its environment key, binding
/// `env_id + entry_id + field_name` so a swap/rollback can't authenticate (F8).
pub fn encrypt_field(
    env_key: &SecretKey,
    env_id: &str,
    entry_id: &str,
    field_name: &str,
    plaintext: &[u8],
) -> Result<Sealed, CryptoError> {
    seal_with(env_key, plaintext, &aad_entry_field(env_id, entry_id, field_name))
}

/// Decrypt one entry field. The `env_id`/`entry_id`/`field_name` must match the
/// values used at encryption or authentication fails (THREAT F8).
pub fn decrypt_field(
    env_key: &SecretKey,
    env_id: &str,
    entry_id: &str,
    field_name: &str,
    sealed: &Sealed,
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    aead::open(
        env_key,
        &sealed.nonce,
        &sealed.ciphertext,
        &aad_entry_field(env_id, entry_id, field_name),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast_params() -> KdfParams {
        KdfParams { m_cost_kib: kdf::MIN_M_COST_KIB, t_cost: kdf::MIN_T_COST, p_cost: kdf::MIN_P_COST }
    }

    #[test]
    fn full_unlock_chain_roundtrips() {
        // master <- password; vaultKey wrapped by master; envKey wrapped by vault;
        // field encrypted under env. Then walk it all back.
        let salt = generate_salt();
        let master = derive_master_key(b"master-pw", &salt, &fast_params()).unwrap();

        let vault_key = SecretKey::generate();
        let wrapped_vault = wrap_vault_key(&master, &vault_key).unwrap();
        let vault_back = unwrap_vault_key(&master, &wrapped_vault).unwrap();
        assert_eq!(vault_key.expose(), vault_back.expose());

        let env_id = "11111111-1111-4111-8111-111111111111";
        let env_key = SecretKey::generate();
        let wrapped_env = wrap_env_key(&vault_back, env_id, &env_key).unwrap();
        let env_back = unwrap_env_key(&vault_back, env_id, &wrapped_env).unwrap();
        assert_eq!(env_key.expose(), env_back.expose());

        let entry_id = "22222222-2222-4222-8222-222222222222";
        let sealed = encrypt_field(&env_back, env_id, entry_id, "password", b"hunter2").unwrap();
        let pt = decrypt_field(&env_back, env_id, entry_id, "password", &sealed).unwrap();
        assert_eq!(&pt[..], b"hunter2");
    }

    #[test]
    fn wrong_master_password_fails_generically() {
        let salt = generate_salt();
        let master = derive_master_key(b"right", &salt, &fast_params()).unwrap();
        let vault_key = SecretKey::generate();
        let wrapped = wrap_vault_key(&master, &vault_key).unwrap();

        let wrong = derive_master_key(b"wrong", &salt, &fast_params()).unwrap();
        assert!(matches!(
            unwrap_vault_key(&wrong, &wrapped),
            Err(CryptoError::Decrypt)
        ));
    }

    #[test]
    fn env_key_does_not_unwrap_under_a_different_env_id() {
        let vault_key = SecretKey::generate();
        let env_key = SecretKey::generate();
        let wrapped = wrap_env_key(&vault_key, "env-A", &env_key).unwrap();
        assert!(unwrap_env_key(&vault_key, "env-B", &wrapped).is_err());
    }

    #[test]
    fn field_does_not_decrypt_under_a_swapped_entry_id() {
        // Anti-swap (F8): the same ciphertext bound to entry X must not open as Y.
        let env_key = SecretKey::generate();
        let sealed = encrypt_field(&env_key, "env", "entry-X", "password", b"secret").unwrap();
        assert!(decrypt_field(&env_key, "env", "entry-Y", "password", &sealed).is_err());
    }

    #[test]
    fn field_does_not_decrypt_under_a_swapped_field_name() {
        let env_key = SecretKey::generate();
        let sealed = encrypt_field(&env_key, "env", "entry", "password", b"secret").unwrap();
        assert!(decrypt_field(&env_key, "env", "entry", "username", &sealed).is_err());
    }

    #[test]
    fn field_does_not_decrypt_under_a_swapped_env_id() {
        let env_key = SecretKey::generate();
        let sealed = encrypt_field(&env_key, "env-A", "entry", "password", b"secret").unwrap();
        assert!(decrypt_field(&env_key, "env-B", "entry", "password", &sealed).is_err());
    }

    #[test]
    fn field_does_not_decrypt_after_one_byte_tamper() {
        let env_key = SecretKey::generate();
        let mut sealed = encrypt_field(&env_key, "env", "entry", "password", b"secret").unwrap();
        sealed.ciphertext[0] ^= 0x01;
        assert!(decrypt_field(&env_key, "env", "entry", "password", &sealed).is_err());
    }

    #[test]
    fn aad_builders_have_the_spec_shape() {
        assert_eq!(aad_env_key("E"), b"freepass:v1:envkey:E");
        assert_eq!(
            aad_entry_field("E", "X", "password"),
            b"freepass:v1:entry:E:X:password"
        );
    }
}
