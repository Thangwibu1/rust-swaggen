//! Durable client sync state machine: outbox queue, idempotent hashing, cursor catch-up,
//! snapshot resync staging, tombstones, and 3-way conflict resolution (SY01–SY09).
pub mod crdt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use swaggen_api_contract::{
    ChangesFeedResponse, PublishBatchResponse, PublishOperation, PublishResultKind,
    SnapshotResponse, WorkspaceEntityRecord,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutboxStatus {
    Pending,
    Sending,
    Acked,
    RetryWait {
        attempts: u32,
        next_retry_unix: i64,
        last_error: String,
    },
    Conflicted {
        server_revision: i64,
        server_payload_json: String,
        reason: String,
    },
    DeletedRemoteConflict {
        server_revision: i64,
        reason: String,
    },
    BlockedPermission {
        reason: String,
    },
    FailedActionRequired {
        reason: String,
    },
}

impl OutboxStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Sending => "syncing",
            Self::Acked => "synced",
            Self::RetryWait { .. } => "retry_wait",
            Self::Conflicted { .. } => "conflict",
            Self::DeletedRemoteConflict { .. } => "deleted_remote",
            Self::BlockedPermission { .. } => "blocked_permission",
            Self::FailedActionRequired { .. } => "failed",
        }
    }

    pub fn is_sendable(&self, now_unix: i64) -> bool {
        match self {
            Self::Pending => true,
            Self::RetryWait {
                next_retry_unix, ..
            } => now_unix >= *next_retry_unix,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxEntry {
    pub operation_id: String,
    pub workspace_id: String,
    pub entity_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub name: String,
    pub order_key: i64,
    pub base_revision: i64,
    pub base_payload_json: String,
    pub local_payload_json: String,
    pub request_hash: String,
    pub delete: bool,
    pub note: String,
    pub status: OutboxStatus,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

impl OutboxEntry {
    pub fn to_publish_operation(&self) -> PublishOperation {
        PublishOperation {
            operation_id: self.operation_id.clone(),
            request_hash: self.request_hash.clone(),
            entity_id: self.entity_id.clone(),
            parent_id: self.parent_id.clone(),
            kind: self.kind.clone(),
            name: self.name.clone(),
            order_key: self.order_key,
            payload_json: self.local_payload_json.clone(),
            base_revision: self.base_revision,
            delete: self.delete,
            note: self.note.clone(),
        }
    }
}

pub fn compute_request_hash(
    entity_id: &str,
    parent_id: Option<&str>,
    kind: &str,
    name: &str,
    order_key: i64,
    payload_json: &str,
    delete: bool,
) -> String {
    let mut h = Sha256::new();
    h.update(entity_id.as_bytes());
    h.update(b"\0");
    h.update(parent_id.unwrap_or("").as_bytes());
    h.update(b"\0");
    h.update(kind.as_bytes());
    h.update(b"\0");
    h.update(name.as_bytes());
    h.update(b"\0");
    h.update(order_key.to_le_bytes());
    h.update(b"\0");
    h.update(payload_json.as_bytes());
    h.update(b"\0");
    h.update([u8::from(delete)]);
    hex::encode(h.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalDraft {
    pub entity_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub name: String,
    pub order_key: i64,
    pub payload_json: String,
    pub base_revision: i64,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamSyncState {
    pub profile_id: String,
    pub workspace_id: String,
    pub last_cursor: i64,
    pub membership_revision: i64,
    pub base_entities: BTreeMap<String, WorkspaceEntityRecord>,
    pub drafts: BTreeMap<String, LocalDraft>,
    pub outbox: Vec<OutboxEntry>,
    pub cursor_expired_notice: bool,
}

impl TeamSyncState {
    pub fn new(profile_id: impl Into<String>, workspace_id: impl Into<String>) -> Self {
        Self {
            profile_id: profile_id.into(),
            workspace_id: workspace_id.into(),
            last_cursor: 0,
            membership_revision: 1,
            base_entities: BTreeMap::new(),
            drafts: BTreeMap::new(),
            outbox: Vec::new(),
            cursor_expired_notice: false,
        }
    }

    pub fn save_local_draft(&mut self, draft: LocalDraft) {
        self.drafts.insert(draft.entity_id.clone(), draft);
    }

    /// Explicitly queues a local draft or tombstone delete into the durable outbox (SY01/SY02/SY06).
    /// Autosaves stay in `drafts` until user clicks Publish.
    pub fn enqueue_publish(
        &mut self,
        entity_id: &str,
        delete: bool,
        note: &str,
        now_unix: i64,
    ) -> Result<&OutboxEntry, String> {
        let base = self.base_entities.get(entity_id);
        let (parent_id, kind, name, order_key, payload_json, base_revision) =
            if let Some(d) = self.drafts.get(entity_id) {
                (
                    d.parent_id.clone(),
                    d.kind.clone(),
                    d.name.clone(),
                    d.order_key,
                    d.payload_json.clone(),
                    d.base_revision,
                )
            } else if let Some(b) = base {
                (
                    b.parent_id.clone(),
                    b.kind.clone(),
                    b.name.clone(),
                    b.order_key,
                    b.payload_json.clone(),
                    b.revision,
                )
            } else {
                return Err("No local draft or synced entity found to publish".into());
            };
        let base_payload_json = base
            .map(|b| b.payload_json.clone())
            .unwrap_or_else(|| "{}".into());
        let request_hash = compute_request_hash(
            entity_id,
            parent_id.as_deref(),
            &kind,
            &name,
            order_key,
            &payload_json,
            delete,
        );
        let op = OutboxEntry {
            operation_id: Uuid::new_v4().to_string(),
            workspace_id: self.workspace_id.clone(),
            entity_id: entity_id.to_owned(),
            parent_id,
            kind,
            name,
            order_key,
            base_revision,
            base_payload_json,
            local_payload_json: payload_json,
            request_hash,
            delete,
            note: note.trim().to_owned(),
            status: OutboxStatus::Pending,
            created_at_unix: now_unix,
            updated_at_unix: now_unix,
        };
        self.outbox.push(op);
        Ok(self.outbox.last().expect("just pushed"))
    }

    pub fn prepare_send_batch(&mut self, now_unix: i64) -> Vec<PublishOperation> {
        let mut ops = Vec::new();
        for entry in &mut self.outbox {
            if entry.status.is_sendable(now_unix) {
                entry.status = OutboxStatus::Sending;
                entry.updated_at_unix = now_unix;
                ops.push(entry.to_publish_operation());
            }
        }
        ops
    }

    pub fn apply_network_retry(&mut self, operation_ids: &[String], err: &str, now_unix: i64) {
        for entry in &mut self.outbox {
            if operation_ids.contains(&entry.operation_id) {
                let prev_attempts = match entry.status {
                    OutboxStatus::RetryWait { attempts, .. } => attempts,
                    _ => 0,
                };
                let next_attempts = prev_attempts.saturating_add(1);
                let backoff = (1i64 << next_attempts.min(5)) * 2;
                entry.status = OutboxStatus::RetryWait {
                    attempts: next_attempts,
                    next_retry_unix: now_unix + backoff,
                    last_error: err.to_owned(),
                };
                entry.updated_at_unix = now_unix;
            }
        }
    }

    pub fn apply_permission_blocked(
        &mut self,
        operation_ids: &[String],
        reason: &str,
        now_unix: i64,
    ) {
        for entry in &mut self.outbox {
            if operation_ids.contains(&entry.operation_id) {
                entry.status = OutboxStatus::BlockedPermission {
                    reason: reason.to_owned(),
                };
                entry.updated_at_unix = now_unix;
            }
        }
    }

    pub fn apply_publish_response(&mut self, resp: &PublishBatchResponse, now_unix: i64) {
        for res in &resp.results {
            if let Some(entry) = self
                .outbox
                .iter_mut()
                .find(|e| e.operation_id == res.operation_id)
            {
                entry.updated_at_unix = now_unix;
                match res.kind {
                    PublishResultKind::Applied | PublishResultKind::AlreadyAcked => {
                        entry.status = OutboxStatus::Acked;
                        if let Some(ref srv) = res.server_entity {
                            self.base_entities
                                .insert(srv.entity_id.clone(), srv.clone());
                            // Drop draft only if unchanged relative to published hash
                            if let Some(d) = self.drafts.get(&srv.entity_id) {
                                let d_hash = compute_request_hash(
                                    &d.entity_id,
                                    d.parent_id.as_deref(),
                                    &d.kind,
                                    &d.name,
                                    d.order_key,
                                    &d.payload_json,
                                    false,
                                );
                                if d_hash == entry.request_hash {
                                    self.drafts.remove(&srv.entity_id);
                                }
                            }
                        }
                    }
                    PublishResultKind::ConflictRevision => {
                        let srv_payload = res
                            .server_entity
                            .as_ref()
                            .map(|s| s.payload_json.clone())
                            .unwrap_or_else(|| "{}".into());
                        if let Some(ref srv) = res.server_entity {
                            self.base_entities
                                .insert(srv.entity_id.clone(), srv.clone());
                        }
                        entry.status = OutboxStatus::Conflicted {
                            server_revision: res.server_revision,
                            server_payload_json: srv_payload,
                            reason: res.reason.clone(),
                        };
                    }
                    PublishResultKind::ConflictDeletedRemote => {
                        if let Some(ref srv) = res.server_entity {
                            self.base_entities
                                .insert(srv.entity_id.clone(), srv.clone());
                        }
                        entry.status = OutboxStatus::DeletedRemoteConflict {
                            server_revision: res.server_revision,
                            reason: res.reason.clone(),
                        };
                    }
                    PublishResultKind::HashMismatch => {
                        entry.status = OutboxStatus::FailedActionRequired {
                            reason: res.reason.clone(),
                        };
                    }
                }
            }
        }
    }

    pub fn apply_changes_batch(&mut self, feed: &ChangesFeedResponse, now_unix: i64) {
        self.membership_revision = feed.membership_revision;
        for ev in &feed.events {
            if ev.cursor <= self.last_cursor {
                continue;
            }
            let payload = ev.payload_json.clone().unwrap_or_else(|| "{}".into());
            let record = WorkspaceEntityRecord {
                entity_id: ev.entity_id.clone(),
                workspace_id: ev.workspace_id.clone(),
                parent_id: ev.parent_id.clone(),
                kind: ev.entity_kind.clone(),
                name: ev.entity_name.clone(),
                order_key: ev.order_key,
                payload_json: payload.clone(),
                revision: ev.revision,
                deleted: ev.deleted,
                updated_at_unix: ev.created_at_unix,
                updated_by: ev.actor_user_id.clone(),
            };
            self.base_entities.insert(ev.entity_id.clone(), record);
            self.last_cursor = ev.cursor;

            // Detect deleted-vs-edit or remote revision advance on pending outbox entries
            for entry in &mut self.outbox {
                if entry.entity_id == ev.entity_id
                    && entry.operation_id != ev.operation_id
                    && matches!(
                        entry.status,
                        OutboxStatus::Pending | OutboxStatus::RetryWait { .. }
                    )
                {
                    if ev.deleted {
                        entry.status = OutboxStatus::DeletedRemoteConflict {
                            server_revision: ev.revision,
                            reason: format!(
                                "Deleted remotely by {} at rev {}",
                                ev.actor_email, ev.revision
                            ),
                        };
                        entry.updated_at_unix = now_unix;
                    } else if ev.revision != entry.base_revision {
                        entry.status = OutboxStatus::Conflicted {
                            server_revision: ev.revision,
                            server_payload_json: payload.clone(),
                            reason: format!(
                                "Updated remotely by {} to rev {}",
                                ev.actor_email, ev.revision
                            ),
                        };
                        entry.updated_at_unix = now_unix;
                    }
                }
            }
        }
        if feed.next_cursor > self.last_cursor {
            self.last_cursor = feed.next_cursor;
        }
    }

    /// Cursor-expired consistent multi-page snapshot staging (SY05): refuses mixed-cursor pages.
    pub fn stage_and_swap_snapshot(
        &mut self,
        pages: &[SnapshotResponse],
        now_unix: i64,
    ) -> Result<(), String> {
        if pages.is_empty() {
            return Err("Empty snapshot response".into());
        }
        let cut_cursor = pages[0].snapshot_cursor;
        let mem_rev = pages[0].membership_revision;
        let mut staged = BTreeMap::new();
        for p in pages {
            if p.snapshot_cursor != cut_cursor {
                return Err("Snapshot pages had inconsistent snapshot_cursor; aborted swap".into());
            }
            for e in &p.entities {
                staged.insert(e.entity_id.clone(), e.clone());
            }
        }
        self.base_entities = staged;
        self.last_cursor = cut_cursor;
        self.membership_revision = mem_rev;
        self.cursor_expired_notice = false;

        // Reconcile pending outbox against swapped snapshot without discarding drafts
        for entry in &mut self.outbox {
            if matches!(
                entry.status,
                OutboxStatus::Pending | OutboxStatus::RetryWait { .. }
            ) && let Some(srv) = self.base_entities.get(&entry.entity_id)
            {
                if srv.deleted {
                    entry.status = OutboxStatus::DeletedRemoteConflict {
                        server_revision: srv.revision,
                        reason: "Entity was deleted on server before catch-up snapshot".into(),
                    };
                    entry.updated_at_unix = now_unix;
                } else if srv.revision != entry.base_revision {
                    entry.status = OutboxStatus::Conflicted {
                        server_revision: srv.revision,
                        server_payload_json: srv.payload_json.clone(),
                        reason: format!(
                            "Snapshot revision ({}) differs from local base ({})",
                            srv.revision, entry.base_revision
                        ),
                    };
                    entry.updated_at_unix = now_unix;
                }
            }
        }
        Ok(())
    }

    /// Resolve a conflict via 3-way choice while preserving losing version as a safe local draft (SY04).
    pub fn resolve_conflict(
        &mut self,
        operation_id: &str,
        choice: ConflictChoice,
        now_unix: i64,
    ) -> Result<(), String> {
        let idx = self
            .outbox
            .iter()
            .position(|e| e.operation_id == operation_id)
            .ok_or_else(|| "Conflict operation not found".to_owned())?;
        let old = self.outbox[idx].clone();
        let (srv_rev, srv_payload) = match &old.status {
            OutboxStatus::Conflicted {
                server_revision,
                server_payload_json,
                ..
            } => (*server_revision, server_payload_json.clone()),
            OutboxStatus::DeletedRemoteConflict {
                server_revision, ..
            } => (*server_revision, "{}".to_owned()),
            _ => return Err("Operation is not in conflict state".into()),
        };

        match choice {
            ConflictChoice::TakeServerKeepMyDraft => {
                // Losing local edit stays safely in `drafts` for manual export/reference (SY04)
                self.drafts.insert(
                    old.entity_id.clone(),
                    LocalDraft {
                        entity_id: old.entity_id.clone(),
                        parent_id: old.parent_id,
                        kind: old.kind,
                        name: old.name,
                        order_key: old.order_key,
                        payload_json: old.local_payload_json,
                        base_revision: srv_rev,
                        updated_at_unix: now_unix,
                    },
                );
                self.outbox[idx].status = OutboxStatus::Acked;
                self.outbox[idx].updated_at_unix = now_unix;
            }
            ConflictChoice::RebaseWithPayload {
                merged_payload_json,
                note,
            } => {
                let hash = compute_request_hash(
                    &old.entity_id,
                    old.parent_id.as_deref(),
                    &old.kind,
                    &old.name,
                    old.order_key,
                    &merged_payload_json,
                    false,
                );
                // Mark superseded conflict resolved & enqueue fresh idempotent operation against `srv_rev`
                self.outbox[idx].status = OutboxStatus::Acked;
                self.outbox[idx].updated_at_unix = now_unix;
                self.drafts.insert(
                    old.entity_id.clone(),
                    LocalDraft {
                        entity_id: old.entity_id.clone(),
                        parent_id: old.parent_id.clone(),
                        kind: old.kind.clone(),
                        name: old.name.clone(),
                        order_key: old.order_key,
                        payload_json: merged_payload_json.clone(),
                        base_revision: srv_rev,
                        updated_at_unix: now_unix,
                    },
                );
                self.outbox.push(OutboxEntry {
                    operation_id: Uuid::new_v4().to_string(),
                    workspace_id: old.workspace_id,
                    entity_id: old.entity_id,
                    parent_id: old.parent_id,
                    kind: old.kind,
                    name: old.name,
                    order_key: old.order_key,
                    base_revision: srv_rev,
                    base_payload_json: srv_payload,
                    local_payload_json: merged_payload_json,
                    request_hash: hash,
                    delete: false,
                    note,
                    status: OutboxStatus::Pending,
                    created_at_unix: now_unix,
                    updated_at_unix: now_unix,
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ConflictChoice {
    TakeServerKeepMyDraft,
    RebaseWithPayload {
        merged_payload_json: String,
        note: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDiff {
    pub path: String,
    pub base_val: String,
    pub local_val: String,
    pub server_val: String,
}

fn flatten_json(val: &Value, prefix: &str, out: &mut BTreeMap<String, String>) {
    match val {
        Value::Object(map) => {
            for (k, v) in map {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_json(v, &p, out);
            }
        }
        Value::Array(arr) => {
            for (idx, v) in arr.iter().enumerate() {
                let p = format!("{prefix}[{idx}]");
                flatten_json(v, &p, out);
            }
        }
        other => {
            out.insert(
                if prefix.is_empty() { "$" } else { prefix }.to_owned(),
                other.to_string(),
            );
        }
    }
}

pub fn compute_three_way_diff(
    base_json: &str,
    local_json: &str,
    server_json: &str,
) -> Vec<FieldDiff> {
    let mut b_map = BTreeMap::new();
    let mut l_map = BTreeMap::new();
    let mut s_map = BTreeMap::new();
    if let Ok(v) = serde_json::from_str::<Value>(base_json) {
        flatten_json(&v, "", &mut b_map);
    } else {
        b_map.insert("raw".into(), base_json.to_owned());
    }
    if let Ok(v) = serde_json::from_str::<Value>(local_json) {
        flatten_json(&v, "", &mut l_map);
    } else {
        l_map.insert("raw".into(), local_json.to_owned());
    }
    if let Ok(v) = serde_json::from_str::<Value>(server_json) {
        flatten_json(&v, "", &mut s_map);
    } else {
        s_map.insert("raw".into(), server_json.to_owned());
    }
    let mut keys = BTreeSet::new();
    keys.extend(b_map.keys().cloned());
    keys.extend(l_map.keys().cloned());
    keys.extend(s_map.keys().cloned());

    keys.into_iter()
        .filter_map(|k| {
            let bv = b_map.get(&k).cloned().unwrap_or_else(|| "<unset>".into());
            let lv = l_map.get(&k).cloned().unwrap_or_else(|| "<unset>".into());
            let sv = s_map.get(&k).cloned().unwrap_or_else(|| "<unset>".into());
            if lv != sv || bv != lv || bv != sv {
                Some(FieldDiff {
                    path: k,
                    base_val: bv,
                    local_val: lv,
                    server_val: sv,
                })
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use swaggen_api_contract::ChangeEventRecord;

    #[test]
    fn outbox_handles_idempotent_retry_conflict_tombstone_and_snapshot_swap() {
        let mut sync = TeamSyncState::new("p1", "ws1");
        sync.base_entities.insert(
            "req1".into(),
            WorkspaceEntityRecord {
                entity_id: "req1".into(),
                workspace_id: "ws1".into(),
                parent_id: None,
                kind: "request".into(),
                name: "Get Health".into(),
                order_key: 1,
                payload_json: r#"{"url":"https://a.test/v1","timeout":1000}"#.into(),
                revision: 1,
                deleted: false,
                updated_at_unix: 100,
                updated_by: "u1".into(),
            },
        );
        sync.save_local_draft(LocalDraft {
            entity_id: "req1".into(),
            parent_id: None,
            kind: "request".into(),
            name: "Get Health".into(),
            order_key: 1,
            payload_json: r#"{"url":"https://a.test/v2","timeout":1000}"#.into(),
            base_revision: 1,
            updated_at_unix: 101,
        });
        let op_id = sync
            .enqueue_publish("req1", false, "bump url", 102)
            .unwrap()
            .operation_id
            .clone();
        let batch = sync.prepare_send_batch(103);
        assert_eq!(batch.len(), 1);

        // Simulated network error moves to RetryWait, preserving op_id
        sync.apply_network_retry(std::slice::from_ref(&op_id), "timeout", 104);
        assert_eq!(sync.outbox[0].status.label(), "retry_wait");

        // Conflict when server updated independently
        sync.apply_publish_response(
            &PublishBatchResponse {
                api_version: 1,
                results: vec![swaggen_api_contract::PublishOperationResult {
                    operation_id: op_id.clone(),
                    entity_id: "req1".into(),
                    kind: PublishResultKind::ConflictRevision,
                    server_revision: 2,
                    server_cursor: None,
                    server_entity: Some(WorkspaceEntityRecord {
                        entity_id: "req1".into(),
                        workspace_id: "ws1".into(),
                        parent_id: None,
                        kind: "request".into(),
                        name: "Get Health".into(),
                        order_key: 1,
                        payload_json: r#"{"url":"https://a.test/v1","timeout":5000}"#.into(),
                        revision: 2,
                        deleted: false,
                        updated_at_unix: 105,
                        updated_by: "u2".into(),
                    }),
                    reason: "stale base_revision".into(),
                }],
            },
            106,
        );
        let diffs = match &sync.outbox[0].status {
            OutboxStatus::Conflicted {
                server_payload_json,
                ..
            } => compute_three_way_diff(
                &sync.outbox[0].base_payload_json,
                &sync.outbox[0].local_payload_json,
                server_payload_json,
            ),
            other => panic!("expected Conflicted, got {other:?}"),
        };
        assert!(diffs.iter().any(|d| d.path == "url"));
        assert!(diffs.iter().any(|d| d.path == "timeout"));

        // Rebase merge preserves merged payload as a fresh Pending operation at rev 2
        sync.resolve_conflict(
            &op_id,
            ConflictChoice::RebaseWithPayload {
                merged_payload_json: r#"{"url":"https://a.test/v2","timeout":5000}"#.into(),
                note: "merged url + timeout".into(),
            },
            107,
        )
        .unwrap();
        assert_eq!(sync.outbox[1].base_revision, 2);

        // Tombstone arrival flags DeletedRemoteConflict cleanly rather than silently overwriting
        sync.apply_changes_batch(
            &ChangesFeedResponse {
                api_version: 1,
                workspace_id: "ws1".into(),
                after_cursor: 0,
                next_cursor: 5,
                has_more: false,
                membership_revision: 2,
                events: vec![ChangeEventRecord {
                    cursor: 5,
                    workspace_id: "ws1".into(),
                    entity_id: "req1".into(),
                    entity_kind: "request".into(),
                    entity_name: "Get Health".into(),
                    parent_id: None,
                    order_key: 1,
                    revision: 3,
                    deleted: true,
                    payload_json: None,
                    operation_id: "remote-del".into(),
                    actor_user_id: "u2".into(),
                    actor_email: "b@test".into(),
                    note: "removed".into(),
                    created_at_unix: 108,
                }],
            },
            109,
        );
        assert_eq!(sync.outbox[1].status.label(), "deleted_remote");

        // Consistent snapshot check rejects mixed cursors
        let mixed = vec![
            SnapshotResponse {
                api_version: 1,
                workspace_id: "ws1".into(),
                snapshot_cursor: 10,
                snapshot_token: "t1".into(),
                membership_revision: 2,
                entities: Vec::new(),
                next_offset: Some(1),
                has_more: true,
            },
            SnapshotResponse {
                api_version: 1,
                workspace_id: "ws1".into(),
                snapshot_cursor: 11,
                snapshot_token: "t1".into(),
                membership_revision: 2,
                entities: Vec::new(),
                next_offset: None,
                has_more: false,
            },
        ];
        assert!(sync.stage_and_swap_snapshot(&mixed, 110).is_err());
    }
}
