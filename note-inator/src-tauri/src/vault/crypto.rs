use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use std::time::Instant;
use zeroize::Zeroizing;

use super::error::{Result, VaultError};

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;
pub const KEY_LEN: usize = 32;

// Bounds enforced when reading a header, so a crafted file cannot make us allocate gigabytes.
const M_COST_MIN: u32 = 8 * 1024; // KiB
const M_COST_MAX: u32 = 1024 * 1024; // 1 GiB
const T_COST_MAX: u32 = 32;
const P_COST_MAX: u32 = 16;

/// Target unlock time on the machine that creates / re-keys the vault.
pub const TARGET_UNLOCK_MS: u128 = 750;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory cost in KiB.
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    pub fn validate(&self) -> Result<()> {
        let ok = (M_COST_MIN..=M_COST_MAX).contains(&self.m_cost)
            && (1..=T_COST_MAX).contains(&self.t_cost)
            && (1..=P_COST_MAX).contains(&self.p_cost);
        if ok {
            Ok(())
        } else {
            Err(VaultError::BadHeader)
        }
    }
}

pub fn derive_key(
    password: &str,
    salt: &[u8; SALT_LEN],
    p: &KdfParams,
) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    p.validate()?;
    let params =
        Params::new(p.m_cost, p.t_cost, p.p_cost, Some(KEY_LEN)).map_err(|_| VaultError::Crypto)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|_| VaultError::Crypto)?;
    Ok(key)
}

/// Picks Argon2id parameters that take roughly `TARGET_UNLOCK_MS` on this machine:
/// 256 MiB, 4 lanes, with the pass count tuned from a one-pass timing.
pub fn calibrate() -> KdfParams {
    let mut p = KdfParams { m_cost: 256 * 1024, t_cost: 1, p_cost: 4 };
    let salt = [0u8; SALT_LEN];
    let start = Instant::now();
    if derive_key("calibration", &salt, &p).is_err() {
        p.t_cost = 3;
        return p;
    }
    let one_pass = start.elapsed().as_millis().max(1);
    p.t_cost = ((TARGET_UNLOCK_MS + one_pass - 1) / one_pass).clamp(2, 12) as u32;
    p
}

pub fn random_salt() -> [u8; SALT_LEN] {
    let mut s = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut s);
    s
}

pub fn random_nonce() -> [u8; NONCE_LEN] {
    let mut n = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut n);
    n
}

pub fn encrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .encrypt(XNonce::from_slice(nonce), Payload { msg: plaintext, aad })
        .map_err(|_| VaultError::Crypto)
}

pub fn decrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ciphertext, aad })
        .map(Zeroizing::new)
        .map_err(|_| VaultError::AuthFailed)
}

pub fn keys_equal(a: &[u8; KEY_LEN], b: &[u8; KEY_LEN]) -> bool {
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
