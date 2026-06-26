//! Password generator (CRYPTO_SPEC §5). Uses the OS CSPRNG (`OsRng`) with
//! **unbiased** rejection sampling — never `% len` on raw bytes, never a seeded
//! PRNG. The result is a `Zeroizing<String>` so the generated secret is wiped
//! from memory when dropped.

use rand::rngs::OsRng;
use rand::RngCore;
use zeroize::Zeroizing;

use crate::error::{AppError, AppResult};

pub const MIN_LENGTH: usize = 6;
pub const MAX_LENGTH: usize = 128;

const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &[u8] = b"0123456789";
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{}:;,.?";

pub struct GenOptions {
    pub length: usize,
    pub lowercase: bool,
    pub uppercase: bool,
    pub digits: bool,
    pub symbols: bool,
}

/// Generate a password from the selected character classes.
pub fn generate(opts: &GenOptions) -> AppResult<Zeroizing<String>> {
    let mut alphabet: Vec<u8> = Vec::new();
    if opts.lowercase {
        alphabet.extend_from_slice(LOWER);
    }
    if opts.uppercase {
        alphabet.extend_from_slice(UPPER);
    }
    if opts.digits {
        alphabet.extend_from_slice(DIGITS);
    }
    if opts.symbols {
        alphabet.extend_from_slice(SYMBOLS);
    }
    if alphabet.is_empty() {
        return Err(AppError::Conflict(
            "sélectionnez au moins une classe de caractères".into(),
        ));
    }

    let len = opts.length.clamp(MIN_LENGTH, MAX_LENGTH);
    let mut out = String::with_capacity(len);
    for _ in 0..len {
        let idx = unbiased_index(alphabet.len());
        out.push(alphabet[idx] as char);
    }
    Ok(Zeroizing::new(out))
}

/// Uniform index in `0..n` via rejection sampling over a full `u32`, avoiding
/// the modulo bias of `x % n` on the raw range.
fn unbiased_index(n: usize) -> usize {
    let n = n as u32;
    // Largest multiple of n that fits in a u32; reject draws at/above it.
    let zone = (u32::MAX / n) * n;
    loop {
        let mut bytes = [0u8; 4];
        OsRng.fill_bytes(&mut bytes);
        let x = u32::from_le_bytes(bytes);
        if x < zone {
            return (x % n) as usize;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_classes(length: usize) -> GenOptions {
        GenOptions { length, lowercase: true, uppercase: true, digits: true, symbols: true }
    }

    #[test]
    fn respects_requested_length() {
        let pw = generate(&all_classes(24)).unwrap();
        assert_eq!(pw.chars().count(), 24);
    }

    #[test]
    fn length_is_clamped_to_bounds() {
        assert_eq!(generate(&all_classes(1)).unwrap().len(), MIN_LENGTH);
        assert_eq!(generate(&all_classes(9999)).unwrap().len(), MAX_LENGTH);
    }

    #[test]
    fn only_selected_classes_appear() {
        let opts = GenOptions { length: 200, lowercase: false, uppercase: false, digits: true, symbols: false };
        let pw = generate(&opts).unwrap();
        assert!(pw.chars().all(|c| c.is_ascii_digit()), "non-digit in digits-only password");
    }

    #[test]
    fn empty_alphabet_is_rejected() {
        let opts = GenOptions { length: 16, lowercase: false, uppercase: false, digits: false, symbols: false };
        assert!(matches!(generate(&opts), Err(AppError::Conflict(_))));
    }

    #[test]
    fn two_passwords_are_almost_surely_different() {
        let a = generate(&all_classes(32)).unwrap();
        let b = generate(&all_classes(32)).unwrap();
        assert_ne!(*a, *b);
    }

    #[test]
    fn unbiased_index_stays_in_range() {
        for _ in 0..1000 {
            assert!(unbiased_index(84) < 84);
        }
    }
}
