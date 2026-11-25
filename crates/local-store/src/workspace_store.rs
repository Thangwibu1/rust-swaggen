//! Durable workspace & history persistence over the P00/P01 framed journal,
//! plus local storage accounting, cache cleanup, and safe data-dir migration (UX10 / Z08).
use crate::journal::{Batch, Journal};
use crate::{WorkspaceDirectory, reject_links};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use swaggen_domain::{
    Error, LocalId,
    workspace::{DEFAULT_HISTORY_RETENTION, HistoryEntry, WorkspaceState},
};
use swaggen_spec_engine::openapi::SpecDocument;

const SNAPSHOT_VERSION: u32 = 1;
const MAX_STATE_BYTES: usize = 950_000;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceEnvelope {
    version: u32,
    workspace: LocalId,
    state: WorkspaceState,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryEnvelope {
    version: u32,
    workspace: LocalId,
    entries: Vec<HistoryEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecsEnvelope {
    version: u32,
    workspace: LocalId,
    specs: Vec<SpecDocument>,
}

pub struct WorkspaceStore {
    directory: WorkspaceDirectory,
    ws_journal: Journal,
    hist_journal: Journal,
    specs_journal: Journal,
    state: WorkspaceState,
    history: Vec<HistoryEntry>,
    specs: Vec<SpecDocument>,
    ws_frames: usize,
    hist_frames: usize,
    specs_frames: usize,
}

impl WorkspaceStore {
    pub fn open(directory: WorkspaceDirectory) -> Result<Self, Error> {
        directory.check()?;
        fs::create_dir_all(directory.downloads_dir()).map_err(|_| Error::Storage)?;
        let (ws_journal, ws_batches) =
            Journal::open(&directory.workspace_path()).map_err(|_| Error::Recovery)?;
        let (hist_journal, hist_batches) =
            Journal::open(&directory.history_path()).map_err(|_| Error::Recovery)?;
        let (specs_journal, specs_batches) =
            Journal::open(&directory.specs_path()).map_err(|_| Error::Recovery)?;

        let default_display_name = if directory.workspace().as_str() == "personal" {
            "Personal".to_owned()
        } else {
            directory.workspace().as_str().to_owned()
        };
        let mut state =
            WorkspaceState::new_personal(directory.workspace().clone(), default_display_name);
        for batch in &ws_batches {
            if !batch.outbox.is_empty() {
                return Err(Error::Version);
            }
            let env: WorkspaceEnvelope =
                serde_json::from_slice(&batch.draft).map_err(|_| Error::Recovery)?;
            if env.version != SNAPSHOT_VERSION {
                return Err(Error::Version);
            }
            if &env.workspace != directory.workspace() {
                return Err(Error::Workspace);
            }
            if env.state.revision < state.revision {
                return Err(Error::Recovery);
            }
            state = env.state;
        }

        let mut history = Vec::new();
        for batch in &hist_batches {
            if !batch.outbox.is_empty() {
                return Err(Error::Version);
            }
            let env: HistoryEnvelope =
                serde_json::from_slice(&batch.draft).map_err(|_| Error::Recovery)?;
            if env.version != SNAPSHOT_VERSION {
                return Err(Error::Version);
            }
            if &env.workspace != directory.workspace() {
                return Err(Error::Workspace);
            }
            history = env.entries;
        }

        let mut specs = Vec::new();
        for batch in &specs_batches {
            if !batch.outbox.is_empty() {
                return Err(Error::Version);
            }
            let env: SpecsEnvelope =
                serde_json::from_slice(&batch.draft).map_err(|_| Error::Recovery)?;
            if env.version != SNAPSHOT_VERSION {
                return Err(Error::Version);
            }
            if &env.workspace != directory.workspace() {
                return Err(Error::Workspace);
            }
            specs = env.specs;
        }

        Ok(Self {
            directory,
            ws_journal,
            hist_journal,
            specs_journal,
            state,
            history,
            specs,
            ws_frames: ws_batches.len(),
            hist_frames: hist_batches.len(),
            specs_frames: specs_batches.len(),
        })
    }

    pub fn state(&self) -> &WorkspaceState {
        &self.state
    }

    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }

    pub fn specs(&self) -> &[SpecDocument] {
        &self.specs
    }

    pub fn save_specs(&mut self, specs: Vec<SpecDocument>) -> Result<(), Error> {
        self.directory.check()?;
        let draft = serde_json::to_vec(&SpecsEnvelope {
            version: SNAPSHOT_VERSION,
            workspace: self.directory.workspace().clone(),
            specs: specs.clone(),
        })
        .map_err(|_| Error::Storage)?;
        if draft.len() > MAX_STATE_BYTES {
            return Err(Error::Limit);
        }
        let batch = Batch {
            draft,
            outbox: Vec::new(),
        };
        if self.specs_frames >= 48 {
            let current_draft = serde_json::to_vec(&SpecsEnvelope {
                version: SNAPSHOT_VERSION,
                workspace: self.directory.workspace().clone(),
                specs: self.specs.clone(),
            })
            .map_err(|_| Error::Storage)?;
            self.specs_journal
                .compact(&Batch {
                    draft: current_draft,
                    outbox: Vec::new(),
                })
                .map_err(|_| Error::Storage)?;
            self.specs_frames = 1;
        }
        self.specs_journal
            .append(&batch)
            .map_err(|_| Error::Storage)?;
        self.specs = specs;
        self.specs_frames += 1;
        Ok(())
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.directory.downloads_dir()
    }

    pub fn save_state(&mut self, mut next: WorkspaceState) -> Result<u64, Error> {
        self.directory.check()?;
        if &next.meta.id != self.directory.workspace() {
            return Err(Error::Workspace);
        }
        next.revision = self.state.revision.checked_add(1).ok_or(Error::Limit)?;
        let batch = self.encode_ws(&next)?;
        if self.ws_frames >= 48 {
            let current = self.encode_ws(&self.state)?;
            self.ws_journal
                .compact(&current)
                .map_err(|_| Error::Storage)?;
            self.ws_frames = 1;
        }
        self.ws_journal.append(&batch).map_err(|_| Error::Storage)?;
        self.state = next;
        self.ws_frames += 1;
        Ok(self.state.revision)
    }

    pub fn record_history(&mut self, entry: HistoryEntry) -> Result<(), Error> {
        self.directory.check()?;
        let mut entries = self.history.clone();
        entries.insert(0, entry);
        let retention = self
            .state
            .history_retention
            .clamp(1, DEFAULT_HISTORY_RETENTION * 4);
        entries.truncate(retention);
        self.persist_history(entries)
    }

    pub fn delete_history_for_request(&mut self, request_id: &str) -> Result<(), Error> {
        let mut entries = self.history.clone();
        entries.retain(|e| e.request_id != request_id);
        self.persist_history(entries)
    }

    pub fn clear_history(&mut self) -> Result<(), Error> {
        self.persist_history(Vec::new())
    }

    fn persist_history(&mut self, mut entries: Vec<HistoryEntry>) -> Result<(), Error> {
        self.directory.check()?;
        let mut batch = self.encode_history(&entries)?;
        while batch.draft.len() > MAX_STATE_BYTES && !entries.is_empty() {
            entries.pop();
            batch = self.encode_history(&entries)?;
        }
        if self.hist_frames >= 48 {
            let current = self.encode_history(&self.history)?;
            self.hist_journal
                .compact(&current)
                .map_err(|_| Error::Storage)?;
            self.hist_frames = 1;
        }
        self.hist_journal
            .append(&batch)
            .map_err(|_| Error::Storage)?;
        self.history = entries;
        self.hist_frames += 1;
        Ok(())
    }

    pub fn clean_cache_preserving_projects(&mut self) -> Result<u64, Error> {
        self.directory.check()?;
        let mut removed_bytes = 0u64;
        let dl = self.directory.downloads_dir();
        if dl.exists() {
            for entry in fs::read_dir(&dl).map_err(|_| Error::Storage)? {
                let entry = entry.map_err(|_| Error::Storage)?;
                let path = entry.path();
                reject_links(&path)?;
                if let Ok(meta) = fs::metadata(&path)
                    && meta.is_file()
                {
                    removed_bytes = removed_bytes.saturating_add(meta.len());
                    fs::remove_file(&path).map_err(|_| Error::Storage)?;
                }
            }
        }
        self.clear_history()?;
        Ok(removed_bytes)
    }

    pub fn storage_stats(&self) -> Result<StorageStats, Error> {
        self.directory.check()?;
        let workspace_bytes = file_len(&self.directory.workspace_path());
        let history_bytes = file_len(&self.directory.history_path());
        let secrets_bytes = file_len(&self.directory.secret_path());
        let mut downloads_bytes = 0u64;
        let dl = self.directory.downloads_dir();
        if dl.exists() {
            for entry in fs::read_dir(&dl).map_err(|_| Error::Storage)? {
                if let Ok(entry) = entry
                    && let Ok(meta) = entry.metadata()
                    && meta.is_file()
                {
                    downloads_bytes = downloads_bytes.saturating_add(meta.len());
                }
            }
        }
        Ok(StorageStats {
            workspace_bytes,
            history_bytes,
            secrets_bytes,
            downloads_bytes,
        })
    }

    fn encode_ws(&self, state: &WorkspaceState) -> Result<Batch, Error> {
        let draft = serde_json::to_vec(&WorkspaceEnvelope {
            version: SNAPSHOT_VERSION,
            workspace: self.directory.workspace().clone(),
            state: state.clone(),
        })
        .map_err(|_| Error::Storage)?;
        if draft.len() > MAX_STATE_BYTES {
            return Err(Error::Limit);
        }
        Ok(Batch {
            draft,
            outbox: Vec::new(),
        })
    }

    fn encode_history(&self, entries: &[HistoryEntry]) -> Result<Batch, Error> {
        let draft = serde_json::to_vec(&HistoryEnvelope {
            version: SNAPSHOT_VERSION,
            workspace: self.directory.workspace().clone(),
            entries: entries.to_vec(),
        })
        .map_err(|_| Error::Storage)?;
        Ok(Batch {
            draft,
            outbox: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageStats {
    pub workspace_bytes: u64,
    pub history_bytes: u64,
    pub secrets_bytes: u64,
    pub downloads_bytes: u64,
}

fn file_len(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Lists valid workspace IDs in a data root directory without touching symlinks.
pub fn list_workspaces(root: &Path) -> Result<Vec<LocalId>, Error> {
    if !root.is_absolute() {
        return Err(Error::Storage);
    }
    reject_links(root)?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for entry in fs::read_dir(root).map_err(|_| Error::Storage)? {
        let entry = entry.map_err(|_| Error::Storage)?;
        let path = entry.path();
        reject_links(&path)?;
        if path.is_dir()
            && let Some(name) = entry.file_name().to_str()
            && let Ok(id) = LocalId::parse(name)
        {
            ids.push(id);
        }
    }
    ids.sort();
    Ok(ids)
}

/// Lists valid workspace IDs along with their stored display name and WorkspaceKind (`Personal` / `Team`).
pub fn list_workspace_summaries(
    root: &Path,
) -> Vec<(LocalId, String, swaggen_domain::workspace::WorkspaceKind)> {
    let Ok(ids) = list_workspaces(root) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let fallback_name = if id.as_str() == "personal" {
            "Personal".to_owned()
        } else {
            id.as_str().to_owned()
        };
        if let Ok(dir) = WorkspaceDirectory::open(root, id.clone())
            && let Ok(store) = WorkspaceStore::open(dir)
        {
            let st = store.state();
            let name = if st.meta.name.trim().is_empty() || st.meta.name == "personal" {
                fallback_name
            } else {
                st.meta.name.clone()
            };
            out.push((id, name, st.meta.kind.clone()));
        } else {
            out.push((
                id,
                fallback_name,
                swaggen_domain::workspace::WorkspaceKind::Personal,
            ));
        }
    }
    out
}

/// Migrates a workspace directory into a new absolute root directory cleanly,
/// verifying byte lengths and preserving the source until confirmed (Z08).
pub fn migrate_workspace_root(
    from_root: &Path,
    to_root: &Path,
    workspace: &LocalId,
) -> Result<PathBuf, Error> {
    if !from_root.is_absolute() || !to_root.is_absolute() || from_root == to_root {
        return Err(Error::Storage);
    }
    let src_dir = WorkspaceDirectory::open(from_root, workspace.clone())?;
    let dst_dir = WorkspaceDirectory::open(to_root, workspace.clone())?;
    for file in [
        "workspace.journal",
        "history.journal",
        "specs.journal",
        "secrets.journal",
        "draft.journal",
    ] {
        let src = src_dir.root_path().join(file);
        if src.exists() {
            let dst = dst_dir.root_path().join(file);
            let bytes = fs::read(&src).map_err(|_| Error::Storage)?;
            fs::write(&dst, &bytes).map_err(|_| Error::Storage)?;
            if fs::metadata(&dst).map(|m| m.len()).unwrap_or(u64::MAX) != bytes.len() as u64 {
                return Err(Error::Storage);
            }
        }
    }
    Ok(dst_dir.root_path().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use swaggen_domain::workspace::{HttpMethod, RequestItem};

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "swaggen-ws-store-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn workspace_and_history_persist_and_cache_cleanup_preserves_collections() {
        let root = temp_root("persist");
        let id = LocalId::parse("personal").unwrap();
        let dir = WorkspaceDirectory::open(&root, id.clone()).unwrap();
        let mut store = WorkspaceStore::open(dir.clone()).unwrap();
        let mut state = store.state().clone();
        let col_id = state.collections[0].id.clone();
        let req = RequestItem::new(
            col_id,
            "List tasks",
            HttpMethod::Get,
            "https://api.example.test/tasks",
        );
        state.requests.push(req.clone());
        let rev = store.save_state(state).unwrap();
        assert_eq!(rev, 1);
        store
            .record_history(HistoryEntry {
                id: "h1".into(),
                timestamp_epoch_secs: 100,
                request_id: req.id.clone(),
                request_name: req.name.clone(),
                method: req.method,
                resolved_url: req.url.clone(),
                status_code: Some(200),
                duration_ms: 15,
                wire_bytes: 120,
                decoded_bytes: 120,
                error_kind: None,
                snapshot: req,
            })
            .unwrap();
        fs::write(store.downloads_dir().join("cached.bin"), b"123456").unwrap();
        let stats = store.storage_stats().unwrap();
        assert_eq!(stats.downloads_bytes, 6);
        let removed = store.clean_cache_preserving_projects().unwrap();
        assert_eq!(removed, 6);
        assert!(store.history().is_empty());
        drop(store);

        let reopened = WorkspaceStore::open(dir).unwrap();
        assert_eq!(reopened.state().requests.len(), 2);
        assert!(reopened.history().is_empty());
        drop(reopened);

        let migrated_root = temp_root("migrated");
        let dst = migrate_workspace_root(&root, &migrated_root, &id).unwrap();
        assert!(dst.join("workspace.journal").is_file());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(migrated_root);
    }
}
