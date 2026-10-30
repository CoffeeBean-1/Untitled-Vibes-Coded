use thiserror::Error;

pub type Result<T> = std::result::Result<T, VaultError>;

#[derive(Debug, Error)]
pub enum VaultError {
    /// AEAD cannot distinguish a wrong password from a modified file, so one message covers both.
    #[error("Incorrect password, or the vault file has been modified or damaged.")]
    AuthFailed,
    #[error("This is not a Note-inator vault file.")]
    BadMagic,
    #[error("This vault was created by an unsupported version of Note-inator.")]
    UnsupportedVersion,
    #[error("The vault file is truncated or its header is invalid.")]
    BadHeader,
    #[error("A vault already exists at this location.")]
    AlreadyExists,
    #[error("The password must be at least 8 characters long.")]
    WeakPassword,
    #[error("The vault is locked.")]
    Locked,
    #[error("Item not found.")]
    NotFound,
    #[error("Cryptography error.")]
    Crypto,
    #[error("Vault data error: {0}")]
    Data(String),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
}
