//! Windows current-user DPAPI; no machine-wide key, remote vault or plaintext file.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use swaggen_domain::{Error, LocalId, Secret, SecretRef};
use swaggen_local_store::{
    WorkspaceDirectory,
    journal::{Batch, Journal},
};

#[cfg(windows)]
fn crypt(input: &[u8], binding: &[u8], protect: bool) -> Result<Vec<u8>, Error> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };
    let data = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(input.len()).map_err(|_| Error::Limit)?,
        pbData: input.as_ptr().cast_mut(),
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(binding.len()).map_err(|_| Error::Limit)?,
        pbData: binding.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: buffers live for synchronous calls; DPAPI does not mutate inputs.
    // Prompt, description and reserved pointers are null; output is LocalAlloc.
    let ok = unsafe {
        if protect {
            CryptProtectData(
                &data,
                std::ptr::null(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &data,
                std::ptr::null_mut(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err(Error::Protection);
    }
    // SAFETY: DPAPI returned cbData bytes in an owned live allocation. Wipe the
    // decrypted OS allocation before releasing; return buffer gets Secret Drop.
    let result = unsafe {
        let bytes = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let result = bytes.to_vec();
        if !protect {
            for b in bytes {
                std::ptr::write_volatile(b, 0);
            }
        }
        LocalFree(output.pbData.cast());
        result
    };
    Ok(result)
}
#[cfg(not(windows))]
fn crypt(_: &[u8], _: &[u8], _: bool) -> Result<Vec<u8>, Error> {
    Err(Error::Protection)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Vault {
    version: u32,
    workspace: LocalId,
    values: BTreeMap<LocalId, Vec<u8>>,
}
pub struct SecretStore {
    directory: WorkspaceDirectory,
    journal: Journal,
    vault: Vault,
    frames: usize,
}
impl SecretStore {
    pub fn open(directory: WorkspaceDirectory) -> Result<Self, Error> {
        directory.check()?;
        let (journal, batches) =
            Journal::open(&directory.secret_path()).map_err(|_| Error::Recovery)?;
        let mut vault = Vault {
            version: 1,
            workspace: directory.workspace().clone(),
            values: BTreeMap::new(),
        };
        for batch in &batches {
            if !batch.outbox.is_empty() {
                return Err(Error::Version);
            }
            let next: Vault = serde_json::from_slice(&batch.draft).map_err(|_| Error::Recovery)?;
            if next.version != 1 {
                return Err(Error::Version);
            }
            if &next.workspace != directory.workspace() {
                return Err(Error::Workspace);
            }
            if next.values.len() > 32 || next.values.values().any(|v| v.len() > 32 * 1024) {
                return Err(Error::Limit);
            }
            vault = next;
        }
        Ok(Self {
            directory,
            journal,
            vault,
            frames: batches.len(),
        })
    }
    pub fn put(&mut self, reference: &SecretRef, secret: &Secret) -> Result<(), Error> {
        self.check(reference)?;
        if self.vault.values.len() >= 32 && !self.vault.values.contains_key(&reference.name) {
            return Err(Error::Limit);
        }
        let encrypted = crypt(secret.expose(), &reference.binding(), true)?;
        if self.frames >= 32 {
            self.journal
                .compact(&Self::batch(&self.vault)?)
                .map_err(|_| Error::Storage)?;
            self.frames = 1;
        }
        let mut values = self.vault.values.clone();
        values.insert(reference.name.clone(), encrypted);
        let next = Vault {
            version: 1,
            workspace: self.vault.workspace.clone(),
            values,
        };
        self.journal
            .append(&Self::batch(&next)?)
            .map_err(|_| Error::Storage)?;
        self.vault = next;
        self.frames += 1;
        Ok(())
    }
    pub fn get(&self, reference: &SecretRef) -> Result<Secret, Error> {
        self.check(reference)?;
        let encrypted = self
            .vault
            .values
            .get(&reference.name)
            .ok_or(Error::Missing)?;
        Secret::new(crypt(encrypted, &reference.binding(), false)?)
    }
    pub fn list_names(&self) -> Vec<LocalId> {
        self.vault.values.keys().cloned().collect()
    }
    pub fn delete(&mut self, reference: &SecretRef) -> Result<(), Error> {
        self.check(reference)?;
        if !self.vault.values.contains_key(&reference.name) {
            return Err(Error::Missing);
        }
        if self.frames >= 32 {
            self.journal
                .compact(&Self::batch(&self.vault)?)
                .map_err(|_| Error::Storage)?;
            self.frames = 1;
        }
        let mut values = self.vault.values.clone();
        values.remove(&reference.name);
        let next = Vault {
            version: 1,
            workspace: self.vault.workspace.clone(),
            values,
        };
        self.journal
            .append(&Self::batch(&next)?)
            .map_err(|_| Error::Storage)?;
        self.vault = next;
        self.frames += 1;
        Ok(())
    }
    fn check(&self, reference: &SecretRef) -> Result<(), Error> {
        if reference.workspace != self.vault.workspace {
            return Err(Error::Workspace);
        }
        self.directory.check()
    }
    fn batch(vault: &Vault) -> Result<Batch, Error> {
        let draft = serde_json::to_vec(vault).map_err(|_| Error::Storage)?;
        if draft.len() > 900_000 {
            return Err(Error::Limit);
        }
        Ok(Batch {
            draft,
            outbox: Vec::new(),
        })
    }
}
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn real_dpapi_persistence_binding_and_plaintext_absence() {
        let root = std::env::temp_dir().join(format!(
            "swaggen-p01-dpapi-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dir = WorkspaceDirectory::open(&root, LocalId::parse("alpha").unwrap()).unwrap();
        let reference = SecretRef {
            workspace: dir.workspace().clone(),
            name: LocalId::parse("token").unwrap(),
        };
        let canary = b"p01-disposable-secret-7d2e5a";
        let mut store = SecretStore::open(dir.clone()).unwrap();
        store
            .put(&reference, &Secret::new(canary.to_vec()).unwrap())
            .unwrap();
        drop(store);
        let bytes = std::fs::read(dir.secret_path()).unwrap();
        assert!(!bytes.windows(canary.len()).any(|b| b == canary));
        let store = SecretStore::open(dir).unwrap();
        assert_eq!(store.get(&reference).unwrap().expose(), canary);
        let wrong = SecretRef {
            workspace: LocalId::parse("beta").unwrap(),
            name: reference.name.clone(),
        };
        assert!(matches!(store.get(&wrong), Err(Error::Workspace)));
        let encrypted = crypt(canary, &reference.binding(), true).unwrap();
        assert!(crypt(&encrypted, &wrong.binding(), false).is_err());
        assert!(
            crypt(
                &encrypted[..encrypted.len() / 2],
                &reference.binding(),
                false
            )
            .is_err()
        );
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn list_names_and_delete_persist_across_reopen() {
        let root = std::env::temp_dir().join(format!(
            "swaggen-p01-dpapi-del-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dir = WorkspaceDirectory::open(&root, LocalId::parse("alpha").unwrap()).unwrap();
        let ref_a = SecretRef {
            workspace: dir.workspace().clone(),
            name: LocalId::parse("alpha-key").unwrap(),
        };
        let ref_b = SecretRef {
            workspace: dir.workspace().clone(),
            name: LocalId::parse("beta-key").unwrap(),
        };
        let mut store = SecretStore::open(dir.clone()).unwrap();
        store
            .put(&ref_b, &Secret::new(b"secret-b".to_vec()).unwrap())
            .unwrap();
        store
            .put(&ref_a, &Secret::new(b"secret-a".to_vec()).unwrap())
            .unwrap();
        assert_eq!(
            store.list_names(),
            vec![ref_a.name.clone(), ref_b.name.clone()]
        );
        store.delete(&ref_a).unwrap();
        assert!(matches!(store.delete(&ref_a), Err(Error::Missing)));
        assert_eq!(store.list_names(), vec![ref_b.name.clone()]);
        drop(store);
        let reopened = SecretStore::open(dir).unwrap();
        assert_eq!(reopened.list_names(), vec![ref_b.name]);
        assert!(matches!(reopened.get(&ref_a), Err(Error::Missing)));
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn corrupt_secret_journal_is_preserved_and_rejected() {
        let root = std::env::temp_dir().join(format!(
            "swaggen-p01-dpapi-corrupt-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dir = WorkspaceDirectory::open(&root, LocalId::parse("alpha").unwrap()).unwrap();
        let raw = b"INVALID-SECRET-JOURNAL-LONG-ENOUGH-TO-FAIL-MAGIC";
        std::fs::write(dir.secret_path(), raw).unwrap();
        assert!(matches!(
            SecretStore::open(dir.clone()),
            Err(Error::Recovery)
        ));
        assert_eq!(std::fs::read(dir.secret_path()).unwrap(), raw);
        std::fs::remove_dir_all(root).unwrap();
    }
}
