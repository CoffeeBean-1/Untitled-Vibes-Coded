//! On-disk layout (all integers little-endian):
//!
//! ```text
//! offset  size  field
//!  0       4    magic "SNVT"
//!  4       2    format version (1)
//!  6       1    KDF id (1 = Argon2id)
//!  7       4    Argon2 memory cost (KiB)
//! 11       4    Argon2 time cost
//! 15       4    Argon2 parallelism
//! 19      16    Argon2 salt
//! 35      24    XChaCha20-Poly1305 nonce (fresh on every save)
//! 59       n    ciphertext || 16-byte Poly1305 tag
//! ```
//!
//! The 59-byte header is passed as AEAD associated data, so any edit to it
//! (including the KDF parameters) makes decryption fail.

use super::crypto::{KdfParams, NONCE_LEN, SALT_LEN};
use super::error::{Result, VaultError};

pub const MAGIC: &[u8; 4] = b"SNVT";
pub const VERSION: u16 = 1;
pub const KDF_ARGON2ID: u8 = 1;
pub const HEADER_LEN: usize = 4 + 2 + 1 + 4 + 4 + 4 + SALT_LEN + NONCE_LEN;
pub const TAG_LEN: usize = 16;

#[derive(Debug, Clone)]
pub struct Header {
    pub params: KdfParams,
    pub salt: [u8; SALT_LEN],
    pub nonce: [u8; NONCE_LEN],
}

impl Header {
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut b = [0u8; HEADER_LEN];
        b[0..4].copy_from_slice(MAGIC);
        b[4..6].copy_from_slice(&VERSION.to_le_bytes());
        b[6] = KDF_ARGON2ID;
        b[7..11].copy_from_slice(&self.params.m_cost.to_le_bytes());
        b[11..15].copy_from_slice(&self.params.t_cost.to_le_bytes());
        b[15..19].copy_from_slice(&self.params.p_cost.to_le_bytes());
        b[19..35].copy_from_slice(&self.salt);
        b[35..59].copy_from_slice(&self.nonce);
        b
    }

    /// Parses and sanity-checks the header of a whole file image.
    pub fn decode(file: &[u8]) -> Result<Header> {
        if file.len() < 4 || &file[0..4] != MAGIC {
            return Err(VaultError::BadMagic);
        }
        if file.len() < HEADER_LEN + TAG_LEN {
            return Err(VaultError::BadHeader);
        }
        let version = u16::from_le_bytes([file[4], file[5]]);
        if version != VERSION || file[6] != KDF_ARGON2ID {
            return Err(VaultError::UnsupportedVersion);
        }
        let u32_at = |i: usize| u32::from_le_bytes([file[i], file[i + 1], file[i + 2], file[i + 3]]);
        let params = KdfParams { m_cost: u32_at(7), t_cost: u32_at(11), p_cost: u32_at(15) };
        params.validate()?;
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&file[19..35]);
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&file[35..59]);
        Ok(Header { params, salt, nonce })
    }
}
