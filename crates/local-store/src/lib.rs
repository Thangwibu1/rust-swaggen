//! Product adapter over the P00-qualified frame/flush primitives.
//! No sync/publish ACK is implied by a local save.
pub mod journal;
pub mod server_profiles_store;
pub mod sync_store;
pub mod workspace_store;
use journal::{Batch, Journal};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use swaggen_domain::{DRAFT_LIMIT, Error, LocalId};

#[derive(Clone)]
pub struct WorkspaceDirectory {
    path: PathBuf,
    workspace: LocalId,
}
impl WorkspaceDirectory {
    pub fn open(root: &Path, workspace: LocalId) -> Result<Self, Error> {
        if !root.is_absolute() {
            return Err(Error::Storage);
        }
        let path = root.join(workspace.as_str());
        // Reject symlinks/junctions at every existing component, including files.
        reject_links(&path)?;
        fs::create_dir_all(&path).map_err(|_| Error::Storage)?;
        reject_links(&path)?;
        Ok(Self { path, workspace })
    }
    pub fn workspace(&self) -> &LocalId {
        &self.workspace
    }
    pub fn draft_path(&self) -> PathBuf {
        self.path.join("draft.journal")
    }
    pub fn secret_path(&self) -> PathBuf {
        self.path.join("secrets.journal")
    }
    pub fn workspace_path(&self) -> PathBuf {
        self.path.join("workspace.journal")
    }
    pub fn history_path(&self) -> PathBuf {
        self.path.join("history.journal")
    }
    pub fn specs_path(&self) -> PathBuf {
        self.path.join("specs.journal")
    }
    pub fn downloads_dir(&self) -> PathBuf {
        self.path.join("downloads")
    }
    pub fn root_path(&self) -> &Path {
        &self.path
    }
    pub fn check(&self) -> Result<(), Error> {
        reject_links(&self.path)?;
        for file in [
            "draft.journal",
            "draft.journal.lock",
            "secrets.journal",
            "secrets.journal.lock",
            "workspace.journal",
            "workspace.journal.lock",
            "history.journal",
            "history.journal.lock",
            "specs.journal",
            "specs.journal.lock",
            "downloads",
        ] {
            reject_links(&self.path.join(file))?;
        }
        Ok(())
    }
}
fn reject_links(path: &Path) -> Result<(), Error> {
    for p in path.ancestors() {
        match fs::symlink_metadata(p) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(Error::Storage);
                }
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if meta.file_attributes() & 0x400 != 0 {
                        return Err(Error::Storage);
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::Storage),
        }
    }
    Ok(())
}
pub const CURRENT_FORMAT_VERSION: u32 = 1;

/// Moves an unreadable or incompatible journal file to a timestamped quarantine
/// file in the same validated workspace directory so forensics are preserved
/// while allowing explicit user-confirmed recovery. Never overwrites existing files.
pub fn quarantine_corrupt_draft(directory: &WorkspaceDirectory) -> Result<Option<PathBuf>, Error> {
    quarantine_file(directory, &directory.draft_path(), "draft.journal")
}

pub fn quarantine_corrupt_secrets(
    directory: &WorkspaceDirectory,
) -> Result<Option<PathBuf>, Error> {
    quarantine_file(directory, &directory.secret_path(), "secrets.journal")
}

fn quarantine_file(
    directory: &WorkspaceDirectory,
    source: &Path,
    base_name: &str,
) -> Result<Option<PathBuf>, Error> {
    directory.check()?;
    if !source.exists() {
        return Ok(None);
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::Storage)?
        .as_nanos();
    let target = directory.path.join(format!(
        "{base_name}.quarantine-{}-{stamp}",
        std::process::id()
    ));
    reject_links(&target)?;
    fs::rename(source, &target).map_err(|_| Error::Storage)?;
    Ok(Some(target))
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub version: u32,
    pub workspace: LocalId,
    pub revision: u64,
    pub draft: String,
}
impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Snapshot")
            .field("workspace", &self.workspace)
            .field("revision", &self.revision)
            .field("draft", &"[REDACTED]")
            .finish()
    }
}
pub struct Store {
    directory: WorkspaceDirectory,
    journal: Journal,
    state: Snapshot,
    frames: usize,
}
impl Store {
    pub fn open(directory: WorkspaceDirectory) -> Result<Self, Error> {
        directory.check()?;
        let (journal, batches) =
            Journal::open(&directory.draft_path()).map_err(|_| Error::Recovery)?;
        let mut state = Snapshot {
            version: 1,
            workspace: directory.workspace.clone(),
            revision: 0,
            draft: String::new(),
        };
        for batch in &batches {
            if !batch.outbox.is_empty() {
                return Err(Error::Version);
            }
            let next: Snapshot =
                serde_json::from_slice(&batch.draft).map_err(|_| Error::Recovery)?;
            if next.version != 1 {
                return Err(Error::Version);
            }
            if next.workspace != directory.workspace {
                return Err(Error::Workspace);
            }
            if next.draft.len() > DRAFT_LIMIT || next.revision <= state.revision {
                return Err(Error::Recovery);
            }
            state = next;
        }
        Ok(Self {
            directory,
            journal,
            state,
            frames: batches.len(),
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state.clone()
    }
    pub fn save(&mut self, draft: String) -> Result<u64, Error> {
        if draft.len() > DRAFT_LIMIT {
            return Err(Error::Limit);
        }
        self.directory.check()?;
        if self.frames >= 64 {
            self.journal
                .compact(&self.batch(&self.state)?)
                .map_err(|_| Error::Storage)?;
            self.frames = 1;
        }
        let next = Snapshot {
            revision: self.state.revision.checked_add(1).ok_or(Error::Limit)?,
            draft,
            ..self.state.clone()
        };
        self.journal
            .append(&self.batch(&next)?)
            .map_err(|_| Error::Storage)?;
        self.state = next;
        self.frames += 1;
        Ok(self.state.revision)
    }
    fn batch(&self, state: &Snapshot) -> Result<Batch, Error> {
        Ok(Batch {
            draft: serde_json::to_vec(state).map_err(|_| Error::Storage)?,
            outbox: Vec::new(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "swaggen-p01-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[test]
    fn workspace_isolation_reopen_and_compaction() {
        let root = root("isolation");
        let alpha = WorkspaceDirectory::open(&root, LocalId::parse("alpha").unwrap()).unwrap();
        let beta = WorkspaceDirectory::open(&root, LocalId::parse("beta").unwrap()).unwrap();
        let mut a = Store::open(alpha.clone()).unwrap();
        let b = Store::open(beta).unwrap();
        assert!(Store::open(alpha.clone()).is_err());
        for n in 0..70 {
            a.save(format!("Tiếng Việt {n}")).unwrap();
        }
        assert!(b.snapshot().draft.is_empty());
        drop(a);
        let reopened = Store::open(alpha).unwrap();
        assert_eq!(reopened.snapshot().revision, 70);
        assert_eq!(reopened.snapshot().draft, "Tiếng Việt 69");
        drop((b, reopened));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_version_and_workspace_are_not_overwritten() {
        for (label, version, workspace, expected) in [
            ("version", 2, "alpha", Error::Version),
            ("workspace", 1, "beta", Error::Workspace),
        ] {
            let root = root(label);
            let dir = WorkspaceDirectory::open(&root, LocalId::parse("alpha").unwrap()).unwrap();
            let (mut j, _) = Journal::open(&dir.draft_path()).unwrap();
            let value = Snapshot {
                version,
                workspace: LocalId::parse(workspace).unwrap(),
                revision: 1,
                draft: "preserve".into(),
            };
            j.append(&Batch {
                draft: serde_json::to_vec(&value).unwrap(),
                outbox: vec![],
            })
            .unwrap();
            drop(j);
            let before = fs::read(dir.draft_path()).unwrap();
            assert!(matches!(Store::open(dir.clone()), Err(e) if e==expected));
            assert_eq!(before, fs::read(dir.draft_path()).unwrap());
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn limit_failure_preserves_acknowledged_state() {
        let root = root("limit");
        let dir = WorkspaceDirectory::open(&root, LocalId::parse("personal").unwrap()).unwrap();
        let mut s = Store::open(dir).unwrap();
        s.save("keep".into()).unwrap();
        assert_eq!(s.save("x".repeat(DRAFT_LIMIT + 1)), Err(Error::Limit));
        assert_eq!(s.snapshot().draft, "keep");
        assert!(!format!("{:?}", s.snapshot()).contains("keep"));
        drop(s);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn relative_root_and_links_are_rejected() {
        assert!(matches!(
            WorkspaceDirectory::open(
                Path::new("relative/root"),
                LocalId::parse("personal").unwrap()
            ),
            Err(Error::Storage)
        ));
        #[cfg(windows)]
        {
            let root = root("junction");
            let target = root.join("outside");
            fs::create_dir_all(&target).unwrap();
            let link = root.join("alpha");
            let status = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&link)
                .arg(&target)
                .output();
            if let Ok(output) = status
                && output.status.success()
            {
                assert!(matches!(
                    WorkspaceDirectory::open(&root, LocalId::parse("alpha").unwrap()),
                    Err(Error::Storage)
                ));
            }
            let _ = fs::remove_dir_all(root);
        }
    }
    #[test]
    fn quarantine_preserves_corrupt_bytes_and_allows_clean_reopen() {
        let root = root("quarantine");
        let dir = WorkspaceDirectory::open(&root, LocalId::parse("personal").unwrap()).unwrap();
        let corrupt = b"CORRUPT-HEADER-BYTES-LONG-ENOUGH-TO-FAIL-MAGIC";
        fs::write(dir.draft_path(), corrupt).unwrap();
        assert_eq!(Store::open(dir.clone()).err(), Some(Error::Recovery));
        assert_eq!(fs::read(dir.draft_path()).unwrap(), corrupt);
        let moved = quarantine_corrupt_draft(&dir).unwrap().unwrap();
        assert_eq!(fs::read(&moved).unwrap(), corrupt);
        let mut reopened = Store::open(dir).unwrap();
        assert_eq!(reopened.snapshot().revision, 0);
        assert_eq!(reopened.save("fresh".into()).unwrap(), 1);
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn readonly_draft_file_fails_closed_and_recovers_after_reopen() {
        let root = root("readonly");
        let dir = WorkspaceDirectory::open(&root, LocalId::parse("personal").unwrap()).unwrap();
        let mut s = Store::open(dir.clone()).unwrap();
        assert_eq!(s.save("first".into()).unwrap(), 1);
        drop(s);
        let mut perms = fs::metadata(dir.draft_path()).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(dir.draft_path(), perms).unwrap();
        assert!(matches!(Store::open(dir.clone()), Err(Error::Recovery)));
        let mut perms = fs::metadata(dir.draft_path()).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        fs::set_permissions(dir.draft_path(), perms).unwrap();
        let reopened = Store::open(dir).unwrap();
        assert_eq!(reopened.snapshot().revision, 1);
        assert_eq!(reopened.snapshot().draft, "first");
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }
}
