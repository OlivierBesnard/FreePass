//! Argon2id key derivation (CRYPTO_SPEC.md §2-3).
//!
//! Derives the 32-byte `masterKey` from the master password + a per-vault salt.
//! Enforces the parameter floor: derivation **refuses** parameters weaker than
//! the floor rather than producing a brute-forceable key (THREAT F4).

use argon2::{Algorithm, Argon2, Params, Version};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::crypto::error::CryptoError;
use crate::crypto::keys::{SecretKey, KEY_LEN};

/// Salt length in bytes (CRYPTO_SPEC §2).
pub const SALT_LEN: usize = 16;

// Default Argon2id parameters (CRYPTO_SPEC §2). m in KiB.
pub const DEFAULT_M_COST_KIB: u32 = 65_536; // 64 MiB
pub const DEFAULT_T_COST: u32 = 3;
pub const DEFAULT_P_COST: u32 = 1;

// Security floor — derivation refuses anything below these (CRYPTO_SPEC §2).
pub const MIN_M_COST_KIB: u32 = 19_456; // 19 MiB (OWASP floor)
pub const MIN_T_COST: u32 = 2;
pub const MIN_P_COST: u32 = 1;

/// Argon2id parameters, persisted (in the clear) alongside the vault so the
/// cost can be raised later. Stored as JSON in the `vault.kdf_params` column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            m_cost_kib: DEFAULT_M_COST_KIB,
            t_cost: DEFAULT_T_COST,
            p_cost: DEFAULT_P_COST,
        }
    }
}

impl KdfParams {
    /// Reject parameters below the security floor (THREAT F4). Called on every
    /// derivation, so weak params stored in a tampered vault are refused at unlock.
    pub fn validate_floor(&self) -> Result<(), CryptoError> {
        if self.m_cost_kib < MIN_M_COST_KIB
            || self.t_cost < MIN_T_COST
            || self.p_cost < MIN_P_COST
        {
            return Err(CryptoError::WeakParams);
        }
        Ok(())
    }
}

/// Generate a fresh per-vault salt from the OS CSPRNG (CRYPTO_SPEC §5).
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Derive the 32-byte master key from the password and salt with Argon2id.
///
/// Refuses parameters below the floor (THREAT F4). The returned `SecretKey`
/// zeroizes on drop; the caller should also zeroize `password`.
pub fn derive_master_key(
    password: &[u8],
    salt: &[u8],
    params: &KdfParams,
) -> Result<SecretKey, CryptoError> {
    params.validate_floor()?;

    let argon_params = Params::new(
        params.m_cost_kib,
        params.t_cost,
        params.p_cost,
        Some(KEY_LEN),
    )
    .map_err(|_| CryptoError::Kdf)?;

    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);

    let mut out = [0u8; KEY_LEN];
    argon
        .hash_password_into(password, salt, &mut out)
        .map_err(|_| CryptoError::Kdf)?;

    Ok(SecretKey::from_bytes(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A floor-level params set keeps tests fast while staying above the floor.
    fn fast_params() -> KdfParams {
        KdfParams {
            m_cost_kib: MIN_M_COST_KIB,
            t_cost: MIN_T_COST,
            p_cost: MIN_P_COST,
        }
    }

    #[test]
    fn derivation_is_deterministic_for_fixed_inputs() {
        let salt = [7u8; SALT_LEN];
        let p = fast_params();
        let a = derive_master_key(b"correct horse battery staple", &salt, &p).unwrap();
        let b = derive_master_key(b"correct horse battery staple", &salt, &p).unwrap();
        assert_eq!(a.expose(), b.expose(), "same inputs must yield same key");
    }

    #[test]
    fn different_salt_yields_different_key() {
        let p = fast_params();
        let a = derive_master_key(b"pw", &[1u8; SALT_LEN], &p).unwrap();
        let b = derive_master_key(b"pw", &[2u8; SALT_LEN], &p).unwrap();
        assert_ne!(a.expose(), b.expose());
    }

    #[test]
    fn different_password_yields_different_key() {
        let salt = [3u8; SALT_LEN];
        let p = fast_params();
        let a = derive_master_key(b"pw-a", &salt, &p).unwrap();
        let b = derive_master_key(b"pw-b", &salt, &p).unwrap();
        assert_ne!(a.expose(), b.expose());
    }

    #[test]
    fn refuses_params_below_the_floor() {
        let salt = [0u8; SALT_LEN];
        let weak_mem = KdfParams { m_cost_kib: MIN_M_COST_KIB - 1, t_cost: 3, p_cost: 1 };
        let weak_time = KdfParams { m_cost_kib: DEFAULT_M_COST_KIB, t_cost: 1, p_cost: 1 };
        assert!(matches!(
            derive_master_key(b"pw", &salt, &weak_mem),
            Err(CryptoError::WeakParams)
        ));
        assert!(matches!(
            derive_master_key(b"pw", &salt, &weak_time),
            Err(CryptoError::WeakParams)
        ));
    }

    #[test]
    fn default_params_are_above_the_floor() {
        assert!(KdfParams::default().validate_floor().is_ok());
    }
}
