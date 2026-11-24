//! Single-file, exclusive-writer journal spike. A frame atomically groups draft and outbox.
//! CRC detects accidental corruption; it provides no encryption or authenticity.
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

const MAGIC: &[u8; 8] = b"SWAGJ001";
const COMMIT: &[u8; 8] = b"COMMIT01";
const HEADER: usize = 28;
const MAX_PAYLOAD: usize = 1024 * 1024;
const MAX_JOURNAL: u64 = 32 * 1024 * 1024;
const MAX_BATCHES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    pub draft: Vec<u8>,
    pub outbox: Vec<u8>,
}

/// Observation points for the isolated P00 compaction crash probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionStage {
    Created,
    PartialWrite,
    Written,
    Synced,
    Replaced,
}

pub struct Journal {
    file: Option<File>,
    // Stable sidecar survives atomic replacement of the journal's file identity.
    _writer_lock: File,
    path: PathBuf,
    sequence: u64,
    poisoned: bool,
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "journal invalid; recovery required",
    )
}

#[cfg(windows)]
fn replace_journal(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let mut from: Vec<u16> = from.as_os_str().encode_wide().collect();
    let mut to: Vec<u16> = to.as_os_str().encode_wide().collect();
    if from.contains(&0) || to.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NUL in journal path",
        ));
    }
    from.push(0);
    to.push(0);
    // SAFETY: both buffers are live, NUL-terminated UTF-16 strings. No pointer is
    // retained by the API. COPY_ALLOWED and reboot-delay flags are never used.
    let result = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_journal(from: &Path, to: &Path) -> io::Result<()> {
    // Only the Windows store is under P00 acceptance. Directory fsync on other
    // platforms must be designed before adopting this engine there.
    std::fs::rename(from, to)
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut value = !0u32;
    for byte in bytes {
        value ^= u32::from(*byte);
        for _ in 0..8 {
            value = (value >> 1) ^ (0xedb88320u32 & (0u32.wrapping_sub(value & 1)));
        }
    }
    !value
}

fn frame(sequence: u64, batch: &Batch) -> io::Result<Vec<u8>> {
    let size = batch
        .draft
        .len()
        .checked_add(batch.outbox.len())
        .and_then(|n| n.checked_add(8))
        .ok_or_else(invalid)?;
    if size > MAX_PAYLOAD {
        return Err(invalid());
    }
    let mut payload = Vec::with_capacity(size);
    payload.extend_from_slice(&(batch.draft.len() as u32).to_le_bytes());
    payload.extend_from_slice(&batch.draft);
    payload.extend_from_slice(&(batch.outbox.len() as u32).to_le_bytes());
    payload.extend_from_slice(&batch.outbox);
    let mut record = Vec::with_capacity(HEADER + size + 8);
    record.extend_from_slice(MAGIC);
    record.extend_from_slice(&sequence.to_le_bytes());
    record.extend_from_slice(&(size as u32).to_le_bytes());
    // Include sequence/length in checksum, not just the body.
    let mut checksum_input = record[8..20].to_vec();
    checksum_input.extend_from_slice(&payload);
    record.extend_from_slice(&crc32(&checksum_input).to_le_bytes());
    record.extend_from_slice(&crc32(&record).to_le_bytes());
    record.extend_from_slice(&payload);
    record.extend_from_slice(COMMIT);
    Ok(record)
}

impl Journal {
    /// Locks before recovery. Truncated final frames are discarded; intact corruption fails closed.
    /// The stable sidecar is locked for the full writer lifetime; compaction may
    /// close and reopen the data handle while that writer fence remains held.
    pub fn open(path: &Path) -> io::Result<(Self, Vec<Batch>)> {
        let mut lock_path = path.as_os_str().to_os_string();
        lock_path.push(".lock");
        let writer_lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(Path::new(&lock_path))?;
        writer_lock.try_lock().map_err(io::Error::other)?;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        // The stable sidecar is the exclusive writer fence. A second byte-range
        // lock on the data handle prevents Windows write-through replacement.
        let mut batches = Vec::new();
        let mut good_end = 0u64;
        let file_len = file.metadata()?.len();
        if file_len > MAX_JOURNAL {
            return Err(io::Error::other(
                "journal quota exceeded; explicit recovery required",
            ));
        }
        let mut sequence = 0u64;
        while file_len - good_end >= HEADER as u64 {
            if batches.len() >= MAX_BATCHES {
                return Err(io::Error::other("journal replay quota exceeded"));
            }
            let mut header = [0u8; HEADER];
            file.read_exact(&mut header)?;
            if &header[..8] != MAGIC {
                return Err(invalid());
            }
            let header_crc = u32::from_le_bytes(header[24..28].try_into().unwrap());
            if crc32(&header[..24]) != header_crc {
                return Err(invalid());
            }
            let seq = u64::from_le_bytes(header[8..16].try_into().unwrap());
            if seq != sequence.checked_add(1).ok_or_else(invalid)? {
                return Err(invalid());
            }
            let size = u32::from_le_bytes(header[16..20].try_into().unwrap()) as usize;
            if !(8..=MAX_PAYLOAD).contains(&size) {
                return Err(invalid());
            }
            if file_len - good_end < (HEADER + size + 8) as u64 {
                break;
            }
            let mut payload = vec![0; size];
            file.read_exact(&mut payload)?;
            let mut marker = [0; 8];
            file.read_exact(&mut marker)?;
            let mut checksum_input = header[8..20].to_vec();
            checksum_input.extend_from_slice(&payload);
            let expected = u32::from_le_bytes(header[20..24].try_into().unwrap());
            if &marker != COMMIT || crc32(&checksum_input) != expected {
                return Err(invalid());
            }
            let draft_len = u32::from_le_bytes(payload[..4].try_into().unwrap()) as usize;
            let split = 4usize.checked_add(draft_len).ok_or_else(invalid)?;
            if split + 4 > size {
                return Err(invalid());
            }
            let outbox_len =
                u32::from_le_bytes(payload[split..split + 4].try_into().unwrap()) as usize;
            if split + 4 + outbox_len != size {
                return Err(invalid());
            }
            batches.push(Batch {
                draft: payload[4..split].to_vec(),
                outbox: payload[split + 4..].to_vec(),
            });
            sequence = seq;
            good_end += (HEADER + size + 8) as u64;
        }
        if good_end < file_len {
            file.set_len(good_end)?;
            file.sync_all()?;
        }
        file.seek(SeekFrom::Start(good_end))?;
        Ok((
            Self {
                file: Some(file),
                _writer_lock: writer_lock,
                path: path.to_owned(),
                sequence,
                poisoned: false,
            },
            batches,
        ))
    }

    /// Returns only after sync_all. Any I/O failure poisons writer until recovery/reopen.
    pub fn append(&mut self, batch: &Batch) -> io::Result<u64> {
        self.append_inner(batch, |file, bytes| file.write_all(bytes))
    }

    fn append_inner(
        &mut self,
        batch: &Batch,
        write_record: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> io::Result<u64> {
        if self.poisoned {
            return Err(io::Error::other("writer requires recovery"));
        }
        let next = self.sequence.checked_add(1).ok_or_else(invalid)?;
        let bytes = frame(next, batch)?;
        if next > MAX_BATCHES as u64
            || self
                .file
                .as_ref()
                .ok_or_else(|| io::Error::other("writer requires recovery"))?
                .metadata()?
                .len()
                .saturating_add(bytes.len() as u64)
                > MAX_JOURNAL
        {
            return Err(io::Error::other(
                "journal quota exceeded; compact before append",
            ));
        }
        self.poisoned = true;
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("writer requires recovery"))?;
        write_record(file, &bytes)?;
        file.sync_all()?;
        self.sequence = next;
        self.poisoned = false;
        Ok(next)
    }

    /// Atomically replaces history with a caller-supplied complete draft+outbox snapshot.
    /// The caller must supply full state, not just the latest delta. No production migration semantics implied.
    pub fn compact(&mut self, complete_state: &Batch) -> io::Result<()> {
        self.compact_inner(complete_state, |_| {}, File::sync_all)
    }

    /// Fault-observation hooks are deliberately opt-in for the P00 probe binary.
    #[cfg(feature = "fault-injection")]
    pub fn compact_with_probe(
        &mut self,
        complete_state: &Batch,
        observe: impl FnMut(CompactionStage),
    ) -> io::Result<()> {
        self.compact_inner(complete_state, observe, File::sync_all)
    }

    fn compact_inner(
        &mut self,
        complete_state: &Batch,
        mut observe: impl FnMut(CompactionStage),
        final_sync: impl FnOnce(&File) -> io::Result<()>,
    ) -> io::Result<()> {
        if self.poisoned {
            return Err(io::Error::other("writer requires recovery"));
        }
        let bytes = frame(1, complete_state)?;
        let mut pending = self.path.as_os_str().to_os_string();
        pending.push(format!(
            ".compact-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos()
        ));
        let pending = PathBuf::from(pending);
        let mut replacement = Some(
            OpenOptions::new()
                .create_new(true)
                .read(true)
                .write(true)
                .open(&pending)?,
        );
        observe(CompactionStage::Created);
        let mut was_replaced = false;
        let prepared = (|| {
            let split = bytes.len() / 2;
            replacement.as_mut().unwrap().write_all(&bytes[..split])?;
            observe(CompactionStage::PartialWrite);
            replacement.as_mut().unwrap().write_all(&bytes[split..])?;
            observe(CompactionStage::Written);
            replacement.as_ref().unwrap().sync_all()?;
            observe(CompactionStage::Synced);
            // MoveFileEx WRITE_THROUGH needs closed data handles. The stable
            // sidecar fence remains locked across close/replace/reopen.
            self.poisoned = true;
            drop(self.file.take());
            drop(replacement.take());
            replace_journal(&pending, &self.path)?;
            was_replaced = true;
            observe(CompactionStage::Replaced);
            let mut reopened = OpenOptions::new().read(true).write(true).open(&self.path)?;
            reopened.seek(SeekFrom::End(0))?;
            final_sync(&reopened)?;
            replacement = Some(reopened);
            Ok(())
        })();
        if let Err(error) = prepared {
            // Once replacement succeeded, appending to the old handle would ACK
            // an unlinked file. Force reopen if the final flush did not succeed.
            if was_replaced {
                self.poisoned = true;
            }
            drop(replacement);
            let _ = std::fs::remove_file(&pending);
            return Err(error);
        }
        self.file = replacement;
        self.sequence = 1;
        self.poisoned = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "swaggen-p00-{}-{}.journal",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    fn batch(n: u8) -> Batch {
        Batch {
            draft: vec![n; 23],
            outbox: vec![n; 19],
        }
    }
    #[test]
    fn crc_known_vector() {
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
    }
    #[test]
    fn failed_final_compaction_flush_never_acks_writes_to_old_handle() {
        let p = path();
        let (mut journal, _) = Journal::open(&p).unwrap();
        journal.append(&batch(1)).unwrap();
        assert!(
            journal
                .compact_inner(
                    &batch(2),
                    |_| {},
                    |_| Err(io::Error::other("injected final flush failure"))
                )
                .is_err()
        );
        assert!(journal.append(&batch(3)).is_err());
        drop(journal);
        let (mut recovered, batches) = Journal::open(&p).unwrap();
        assert_eq!(batches, vec![batch(2)]);
        recovered.append(&batch(4)).unwrap();
        drop(recovered);
        fs::remove_file(p).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn native_replace_rejects_embedded_nul_before_api_call() {
        assert_eq!(
            replace_journal(Path::new("invalid\0name"), Path::new("unused"))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
    #[cfg(windows)]
    #[test]
    fn native_write_through_replace_closed_files() {
        let target = path();
        let source = target.with_extension("replacement");
        fs::write(&source, b"replacement").unwrap();
        fs::write(&target, b"old").unwrap();
        replace_journal(&source, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"replacement");
        fs::remove_file(target).unwrap();
    }
    #[test]
    fn every_torn_tail_recovers_whole_batches_and_allows_append() {
        let first = frame(1, &batch(1)).unwrap();
        let second = frame(2, &batch(2)).unwrap();
        for cut in 0..second.len() {
            let p = path();
            let mut bytes = first.clone();
            bytes.extend_from_slice(&second[..cut]);
            fs::write(&p, bytes).unwrap();
            let (mut j, recovered) = Journal::open(&p).unwrap();
            assert_eq!(recovered, vec![batch(1)], "cut={cut}");
            assert_eq!(j.append(&batch(3)).unwrap(), 2);
            drop(j);
            let (j, recovered) = Journal::open(&p).unwrap();
            assert_eq!(recovered, vec![batch(1), batch(3)]);
            drop(j);
            fs::remove_file(p).unwrap();
        }
    }
    #[test]
    fn corrupt_complete_frame_is_preserved_and_rejected() {
        let original = frame(1, &batch(1)).unwrap();
        for index in 0..original.len() {
            let p = path();
            let mut bytes = original.clone();
            bytes[index] ^= 0x80;
            fs::write(&p, &bytes).unwrap();
            assert!(Journal::open(&p).is_err(), "offset={index}");
            assert_eq!(fs::read(&p).unwrap(), bytes);
            fs::remove_file(p).unwrap();
        }
    }
    #[test]
    fn second_writer_is_rejected() {
        let p = path();
        let (j, _) = Journal::open(&p).unwrap();
        assert!(Journal::open(&p).is_err());
        drop(j);
        let (j, _) = Journal::open(&p).unwrap();
        drop(j);
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn oversized_batch_does_not_damage_existing_data() {
        let p = path();
        let (mut j, _) = Journal::open(&p).unwrap();
        j.append(&batch(1)).unwrap();
        assert!(
            j.append(&Batch {
                draft: vec![0; MAX_PAYLOAD],
                outbox: vec![]
            })
            .is_err()
        );
        assert_eq!(j.append(&batch(2)).unwrap(), 2);
        drop(j);
        let (j, got) = Journal::open(&p).unwrap();
        assert_eq!(got, vec![batch(1), batch(2)]);
        drop(j);
        fs::remove_file(p).unwrap();
    }

    #[test]
    fn compaction_reopens_full_state_and_preserves_writer_fence() {
        let p = path();
        let (mut j, _) = Journal::open(&p).unwrap();
        j.append(&batch(1)).unwrap();
        j.append(&batch(2)).unwrap();
        let before = fs::metadata(&p).unwrap().len();
        j.compact(&batch(3)).unwrap();
        assert!(fs::metadata(&p).unwrap().len() < before);
        assert!(
            Journal::open(&p).is_err(),
            "replacement must not bypass writer fence"
        );
        assert_eq!(j.append(&batch(4)).unwrap(), 2);
        drop(j);
        let (j, got) = Journal::open(&p).unwrap();
        assert_eq!(got, vec![batch(3), batch(4)]);
        drop(j);
        fs::remove_file(p).unwrap();
    }

    #[test]
    fn failed_compaction_keeps_acknowledged_data() {
        let p = path();
        let (mut j, _) = Journal::open(&p).unwrap();
        j.append(&batch(1)).unwrap();
        assert!(
            j.compact(&Batch {
                draft: vec![0; MAX_PAYLOAD],
                outbox: vec![]
            })
            .is_err()
        );
        drop(j);
        let (j, got) = Journal::open(&p).unwrap();
        assert_eq!(got, vec![batch(1)]);
        drop(j);
        fs::remove_file(p).unwrap();
    }

    #[test]
    fn oversized_file_is_rejected_before_replay_without_truncation() {
        let p = path();
        let file = File::create(&p).unwrap();
        file.set_len(MAX_JOURNAL + 1).unwrap();
        drop(file);
        assert!(Journal::open(&p).is_err());
        assert_eq!(fs::metadata(&p).unwrap().len(), MAX_JOURNAL + 1);
        fs::remove_file(p).unwrap();
    }

    #[test]
    fn unknown_format_version_is_rejected_without_migration_guess() {
        let p = path();
        let mut bytes = frame(1, &batch(1)).unwrap();
        bytes[7] = b'2';
        fs::write(&p, &bytes).unwrap();
        assert!(Journal::open(&p).is_err());
        assert_eq!(fs::read(&p).unwrap(), bytes);
        fs::remove_file(p).unwrap();
    }

    #[test]
    fn actual_write_error_poisoning_requires_reopen_and_preserves_ack() {
        let p = path();
        let (mut j, _) = Journal::open(&p).unwrap();
        j.append(&batch(1)).unwrap();
        // Real OS write failure from a read-only handle; keep the stable writer fence.
        j.file = Some(File::open(&p).unwrap());
        assert!(j.append(&batch(2)).is_err());
        assert!(j.poisoned);
        assert!(j.append(&batch(3)).is_err());
        drop(j);
        let (j, got) = Journal::open(&p).unwrap();
        assert_eq!(got, vec![batch(1)]);
        drop(j);
        fs::remove_file(p).unwrap();
    }

    #[test]
    fn injected_partial_write_errors_never_ack_or_split_draft_and_outbox() {
        let record_size = frame(2, &batch(2)).unwrap().len();
        for kind in [io::ErrorKind::StorageFull, io::ErrorKind::WriteZero] {
            for cut in 0..record_size {
                let p = path();
                let (mut j, _) = Journal::open(&p).unwrap();
                j.append(&batch(1)).unwrap();
                let error = j
                    .append_inner(&batch(2), |file, bytes| {
                        file.write_all(&bytes[..cut])?;
                        // Deterministic disk-full/short-write injection, not a physical full disk.
                        Err(io::Error::new(kind, "injected record write failure"))
                    })
                    .unwrap_err();
                assert_eq!(error.kind(), kind);
                assert!(j.poisoned);
                assert_eq!(
                    j.sequence, 1,
                    "failed writes must never advance the acknowledged sequence"
                );
                assert!(j.append(&batch(3)).is_err());
                drop(j);
                let (mut j, recovered) = Journal::open(&p).unwrap();
                assert_eq!(recovered, vec![batch(1)], "kind={kind:?} cut={cut}");
                assert_eq!(j.append(&batch(3)).unwrap(), 2);
                drop(j);
                let (j, recovered) = Journal::open(&p).unwrap();
                assert_eq!(recovered, vec![batch(1), batch(3)]);
                drop(j);
                fs::remove_file(p).unwrap();
            }
        }
    }
}
