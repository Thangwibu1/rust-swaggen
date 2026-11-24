//! Durable root-level storage for server connection profiles (never contains secrets/tokens).
use crate::journal::{Batch, Journal};
use crate::reject_links;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use swaggen_domain::{Error, server_profiles::ServerProfileStore};

const PROFILE_FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileEnvelope {
    version: u32,
    store: ServerProfileStore,
}

pub struct ServerProfileJournal {
    journal: Journal,
    store: ServerProfileStore,
    frames: usize,
}

impl ServerProfileJournal {
    pub fn open(root: &Path) -> Result<Self, Error> {
        if !root.is_absolute() {
            return Err(Error::Storage);
        }
        reject_links(root)?;
        fs::create_dir_all(root).map_err(|_| Error::Storage)?;
        let path = root.join("servers.journal");
        reject_links(&path)?;
        let (journal, batches) = Journal::open(&path).map_err(|_| Error::Recovery)?;
        let mut store = ServerProfileStore::default();
        for b in &batches {
            if !b.outbox.is_empty() {
                return Err(Error::Version);
            }
            let env: ProfileEnvelope =
                serde_json::from_slice(&b.draft).map_err(|_| Error::Recovery)?;
            if env.version != PROFILE_FORMAT_VERSION {
                return Err(Error::Version);
            }
            store = env.store;
        }
        Ok(Self {
            journal,
            store,
            frames: batches.len(),
        })
    }

    pub fn store(&self) -> &ServerProfileStore {
        &self.store
    }

    pub fn save(&mut self, next: ServerProfileStore) -> Result<(), Error> {
        let bytes = serde_json::to_vec(&ProfileEnvelope {
            version: PROFILE_FORMAT_VERSION,
            store: next.clone(),
        })
        .map_err(|_| Error::Limit)?;
        let batch = Batch {
            draft: bytes,
            outbox: Vec::new(),
        };
        if self.frames >= 64 {
            self.journal
                .compact(&batch)
                .map_err(|_| Error::Storage)?;
            self.frames = 1;
        } else {
            self.journal.append(&batch).map_err(|_| Error::Storage)?;
            self.frames += 1;
        }
        self.store = next;
        Ok(())
    }
}
