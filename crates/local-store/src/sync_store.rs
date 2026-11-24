//! Durable local persistence of TeamSyncState (cursor, cached shared base, drafts, and outbox).
use crate::{
    WorkspaceDirectory,
    journal::{Batch, Journal},
};
use serde::{Deserialize, Serialize};
use swaggen_domain::Error;
use swaggen_sync::TeamSyncState;

const SYNC_FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SyncEnvelope {
    version: u32,
    state: TeamSyncState,
}

pub struct SyncJournalStore {
    journal: Journal,
    state: TeamSyncState,
    frames: usize,
}

impl SyncJournalStore {
    pub fn open(dir: &WorkspaceDirectory, profile_id: &str, workspace_id: &str) -> Result<Self, Error> {
        dir.check()?;
        let path = dir.root_path().join("sync.journal");
        let (journal, batches) = Journal::open(&path).map_err(|_| Error::Recovery)?;
        let mut state = TeamSyncState::new(profile_id, workspace_id);
        for b in &batches {
            if !b.outbox.is_empty() {
                return Err(Error::Version);
            }
            let env: SyncEnvelope =
                serde_json::from_slice(&b.draft).map_err(|_| Error::Recovery)?;
            if env.version != SYNC_FORMAT_VERSION {
                return Err(Error::Version);
            }
            state = env.state;
        }
        Ok(Self {
            journal,
            state,
            frames: batches.len(),
        })
    }

    pub fn state(&self) -> &TeamSyncState {
        &self.state
    }

    pub fn save(&mut self, next: TeamSyncState) -> Result<(), Error> {
        let bytes = serde_json::to_vec(&SyncEnvelope {
            version: SYNC_FORMAT_VERSION,
            state: next.clone(),
        })
        .map_err(|_| Error::Limit)?;
        let batch = Batch {
            draft: bytes,
            outbox: Vec::new(),
        };
        if self.frames >= 48 {
            self.journal
                .compact(&batch)
                .map_err(|_| Error::Storage)?;
            self.frames = 1;
        } else {
            self.journal.append(&batch).map_err(|_| Error::Storage)?;
            self.frames += 1;
        }
        self.state = next;
        Ok(())
    }
}
