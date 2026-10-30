use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;
use zeroize::Zeroizing;

use super::crypto::{self, KdfParams, KEY_LEN, SALT_LEN};
use super::error::{Result, VaultError};
use super::format::{Header, HEADER_LEN};
use super::model::VaultData;

const MIN_PASSWORD_LEN: usize = 8;

/// An unlocked vault. The derived key lives here and is zeroized when the value is dropped.
pub struct Vault {
    path: PathBuf,
    key: Zeroizing<[u8; KEY_LEN]>,
    salt: [u8; SALT_LEN],
    params: KdfParams,
    pub data: VaultData,
}

fn check_password(password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        Err(VaultError::WeakPassword)
    } else {
        Ok(())
    }
}

impl Vault {
    pub fn create(path: &Path, password: &str) -> Result<Vault> {
        check_password(password)?;
        Self::create_with_params(path, password, crypto::calibrate())
    }

    pub fn create_with_params(path: &Path, password: &str, params: KdfParams) -> Result<Vault> {
        check_password(password)?;
        if path.exists() {
            return Err(VaultError::AlreadyExists);
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let salt = crypto::random_salt();
        let key = crypto::derive_key(password, &salt, &params)?;
        let vault = Vault { path: path.to_path_buf(), key, salt, params, data: VaultData::default() };
        vault.save()?;
        Ok(vault)
    }

    pub fn open(path: &Path, password: &str) -> Result<Vault> {
        let bytes = fs::read(path)?;
        let header = Header::decode(&bytes)?;
        let key = crypto::derive_key(password, &header.salt, &header.params)?;
        let plain = crypto::decrypt(&key, &header.nonce, &bytes[..HEADER_LEN], &bytes[HEADER_LEN..])?;
        let data: VaultData =
            serde_json::from_slice(&plain).map_err(|e| VaultError::Data(e.to_string()))?;
        Ok(Vault { path: path.to_path_buf(), key, salt: header.salt, params: header.params, data })
    }

    /// Encrypts the whole vault under a fresh random nonce and replaces the file atomically.
    pub fn save(&self) -> Result<()> {
        let header = Header { params: self.params, salt: self.salt, nonce: crypto::random_nonce() };
        let header_bytes = header.encode();
        let plain = Zeroizing::new(
            serde_json::to_vec(&self.data).map_err(|e| VaultError::Data(e.to_string()))?,
        );
        let ct = crypto::encrypt(&self.key, &header.nonce, &header_bytes, &plain)?;
        let mut out = Vec::with_capacity(HEADER_LEN + ct.len());
        out.extend_from_slice(&header_bytes);
        out.extend_from_slice(&ct);
        atomic_write(&self.path, &out)?;
        Ok(())
    }

    /// Verifies `old`, then re-encrypts under a new salt, freshly calibrated parameters and key.
    pub fn change_password(&mut self, old: &str, new: &str) -> Result<()> {
        check_password(new)?;
        let old_key = crypto::derive_key(old, &self.salt, &self.params)?;
        if !crypto::keys_equal(&old_key, &self.key) {
            return Err(VaultError::AuthFailed);
        }
        self.rekey(new, crypto::calibrate())
    }

    pub(crate) fn rekey(&mut self, new: &str, params: KdfParams) -> Result<()> {
        let salt = crypto::random_salt();
        let key = crypto::derive_key(new, &salt, &params)?;
        let prev = (std::mem::replace(&mut self.key, key), self.salt, self.params);
        self.salt = salt;
        self.params = params;
        if let Err(e) = self.save() {
            self.key = prev.0;
            self.salt = prev.1;
            self.params = prev.2;
            return Err(e);
        }
        Ok(())
    }
}

/// Checks that `path` looks like a vault (magic, version, sane KDF parameters).
/// It cannot check the password; that only happens on unlock.
pub fn validate_backup(path: &Path) -> Result<()> {
    Header::decode(&fs::read(path)?).map(|_| ())
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

fn rename_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    // Antivirus / indexers can briefly hold a handle on Windows.
    let mut last = None;
    for _ in 0..5 {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = Some(e);
                sleep(Duration::from_millis(60));
            }
        }
    }
    Err(last.unwrap())
}

/// Writes `bytes` (already encrypted) next to `path` as `<path>.tmp`, flushes it to disk, keeps the
/// previous file as `<path>.bak`, then renames the new file over `path`. At every point `path` is
/// either the complete old file or the complete new file.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = sibling(path, ".tmp");
    let result = (|| {
        {
            let mut f = File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        if path.exists() {
            let bak_tmp = sibling(path, ".bak.tmp");
            fs::copy(path, &bak_tmp)?;
            rename_retry(&bak_tmp, &sibling(path, ".bak"))?;
        }
        rename_retry(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::format::{HEADER_LEN, MAGIC};
    use tempfile::tempdir;

    const PW: &str = "correct horse battery staple";
    const SECRET: &str = "TOP-SECRET-MARKER-9f3a";

    fn fast() -> KdfParams {
        KdfParams { m_cost: 8 * 1024, t_cost: 1, p_cost: 1 }
    }

    fn vault_with_secret(path: &Path) -> Vault {
        let mut v = Vault::create_with_params(path, PW, fast()).unwrap();
        let nb = v.data.add_notebook("Notebook");
        let sec = v.data.add_section(&nb, "Section").unwrap();
        let pg = v.data.add_page(&sec, SECRET).unwrap();
        v.data.page_mut(&pg).unwrap().content = serde_json::json!({ "text": SECRET });
        v.save().unwrap();
        v
    }

    #[test]
    fn round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.dat");
        vault_with_secret(&path);
        let opened = Vault::open(&path, PW).unwrap();
        assert_eq!(opened.data.notebooks[0].name, "Notebook");
        assert_eq!(opened.data.notebooks[0].sections[0].pages[0].title, SECRET);
    }

    #[test]
    fn file_contains_no_plaintext() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.dat");
        vault_with_secret(&path);
        let bytes = fs::read(&path).unwrap();
        assert!(&bytes[..4] == MAGIC);
        let hay = String::from_utf8_lossy(&bytes);
        for needle in [SECRET, "Notebook", "Section", "notebooks", "title"] {
            assert!(!hay.contains(needle), "plaintext leaked: {needle}");
        }
        for f in fs::read_dir(dir.path()).unwrap() {
            let b = fs::read(f.unwrap().path()).unwrap();
            assert!(!String::from_utf8_lossy(&b).contains(SECRET));
        }
    }

    #[test]
    fn wrong_password_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.dat");
        vault_with_secret(&path);
        assert!(matches!(Vault::open(&path, "not the password"), Err(VaultError::AuthFailed)));
    }

    #[test]
    fn tampered_ciphertext_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.dat");
        vault_with_secret(&path);
        let clean = fs::read(&path).unwrap();
        // Flip one bit in the ciphertext, then in the tag.
        for idx in [HEADER_LEN + 3, clean.len() - 1] {
            let mut bad = clean.clone();
            bad[idx] ^= 0x01;
            fs::write(&path, &bad).unwrap();
            assert!(matches!(Vault::open(&path, PW), Err(VaultError::AuthFailed)), "byte {idx}");
        }
    }

    #[test]
    fn tampered_header_rejected() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.dat");
        vault_with_secret(&path);
        let clean = fs::read(&path).unwrap();
        // Salt, nonce, and a KDF parameter (t_cost 1 -> 2) are all covered by the AEAD.
        for idx in [20usize, 40, 11] {
            let mut bad = clean.clone();
            bad[idx] ^= if idx == 11 { 0x03 } else { 0x01 };
            fs::write(&path, &bad).unwrap();
            assert!(Vault::open(&path, PW).is_err(), "header byte {idx}");
        }
    }

    #[test]
    fn malformed_files_fail_cleanly() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.dat");
        vault_with_secret(&path);
        let clean = fs::read(&path).unwrap();

        fs::write(&path, b"hello, this is a text file").unwrap();
        assert!(matches!(Vault::open(&path, PW), Err(VaultError::BadMagic)));

        fs::write(&path, &clean[..20]).unwrap();
        assert!(matches!(Vault::open(&path, PW), Err(VaultError::BadHeader)));

        fs::write(&path, &clean[..clean.len() - 5]).unwrap();
        assert!(matches!(Vault::open(&path, PW), Err(VaultError::AuthFailed)));

        // Absurd memory cost must be refused before any allocation.
        let mut huge = clean.clone();
        huge[7..11].copy_from_slice(&u32::MAX.to_le_bytes());
        fs::write(&path, &huge).unwrap();
        assert!(matches!(Vault::open(&path, PW), Err(VaultError::BadHeader)));

        let mut ver = clean.clone();
        ver[4] = 9;
        fs::write(&path, &ver).unwrap();
        assert!(matches!(Vault::open(&path, PW), Err(VaultError::UnsupportedVersion)));
    }

    #[test]
    fn fresh_nonce_on_every_save() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.dat");
        let v = vault_with_secret(&path);
        let a = fs::read(&path).unwrap();
        v.save().unwrap();
        let b = fs::read(&path).unwrap();
        assert_ne!(&a[35..59], &b[35..59], "nonce reused");
        assert_ne!(a[HEADER_LEN..], b[HEADER_LEN..]);
        assert_eq!(&a[19..35], &b[19..35], "salt is per-vault");
    }

    #[test]
    fn atomic_save_keeps_backup_and_leaves_no_temp() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.dat");
        let mut v = vault_with_secret(&path);
        let before_count = v.data.notebooks.len();
        v.data.add_notebook("Second");
        v.save().unwrap();

        let bak = dir.path().join("vault.dat.bak");
        assert!(bak.exists());
        assert!(!dir.path().join("vault.dat.tmp").exists());
        assert!(!dir.path().join("vault.dat.bak.tmp").exists());
        // .bak is the previous version; main file is the new one.
        assert_eq!(Vault::open(&bak, PW).unwrap().data.notebooks.len(), before_count);
        assert_eq!(Vault::open(&path, PW).unwrap().data.notebooks.len(), before_count + 1);
    }

    #[test]
    fn crash_leftovers_do_not_corrupt_vault() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.dat");
        let mut v = vault_with_secret(&path);
        // A crash mid-write leaves a partial temp file; the real vault must be unaffected.
        fs::write(dir.path().join("vault.dat.tmp"), b"partial garbage").unwrap();
        assert!(Vault::open(&path, PW).is_ok());
        v.data.add_notebook("After crash");
        v.save().unwrap();
        assert!(!dir.path().join("vault.dat.tmp").exists());
        assert_eq!(Vault::open(&path, PW).unwrap().data.notebooks.len(), 2);
    }

    #[test]
    fn change_password_reencrypts() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.dat");
        let mut v = vault_with_secret(&path);
        let old_salt = fs::read(&path).unwrap()[19..35].to_vec();

        assert!(matches!(v.change_password("wrong old password", "new password 123"), Err(VaultError::AuthFailed)));
        assert!(matches!(v.change_password(PW, "short"), Err(VaultError::WeakPassword)));
        // Unchanged after failed attempts.
        assert!(Vault::open(&path, PW).is_ok());

        v.rekey("new password 123", fast()).unwrap();
        assert_ne!(fs::read(&path).unwrap()[19..35].to_vec(), old_salt);
        assert!(matches!(Vault::open(&path, PW), Err(VaultError::AuthFailed)));
        let reopened = Vault::open(&path, "new password 123").unwrap();
        assert_eq!(reopened.data.notebooks[0].sections[0].pages[0].title, SECRET);
    }

    #[test]
    fn create_refuses_to_overwrite_and_weak_passwords() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.dat");
        assert!(matches!(Vault::create_with_params(&path, "short", fast()), Err(VaultError::WeakPassword)));
        vault_with_secret(&path);
        assert!(matches!(Vault::create_with_params(&path, PW, fast()), Err(VaultError::AlreadyExists)));
    }

    #[test]
    fn backup_validation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("vault.dat");
        vault_with_secret(&path);
        assert!(validate_backup(&path).is_ok());
        let junk = dir.path().join("junk.dat");
        fs::write(&junk, b"nope").unwrap();
        assert!(validate_backup(&junk).is_err());
    }

    #[test]
    fn calibrated_params_are_valid() {
        let p = crypto::calibrate();
        assert!(p.validate().is_ok());
    }
}
