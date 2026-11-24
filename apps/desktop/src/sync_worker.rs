//! Desktop P05 Sync worker: durable outbox/cursor persistence, HTTP publish,
//! cursor catch-up & snapshot fallback, 3-way conflict resolution, revisions, comments, and notifications.
use crate::{
    server_client::{server_call, server_call_empty},
    server_worker::{load_server_profiles, read_session_token},
    worker::{Command, Event},
};
use std::path::Path;
use swaggen_api_contract::{
    ChangesFeedResponse, CommentListResponse, CommentThreadRecord, CreateCommentRequest,
    MarkNotificationsReadRequest, NotificationListResponse, PublishBatchRequest,
    PublishBatchResponse, ResolveCommentRequest, RestoreRevisionRequest, RevisionListResponse,
    SnapshotResponse,
};
use swaggen_domain::{LocalId, server_profiles::ServerProfile, workspace::HttpMethod};
use swaggen_local_store::{WorkspaceDirectory, sync_store::SyncJournalStore};
use swaggen_sync::{LocalDraft, TeamSyncState};
use uuid::Uuid;

fn sync_dir_id(workspace_id: &str) -> LocalId {
    let cleaned: String = workspace_id
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-' || *c == '_')
        .take(40)
        .collect();
    let candidate = if cleaned.is_empty() {
        "team-default".to_owned()
    } else {
        format!("team-{cleaned}")
    };
    LocalId::parse(candidate).unwrap_or_else(|_| LocalId::parse("team-default").unwrap())
}

fn open_sync_journal(
    root: &Path,
    profile_id: &str,
    workspace_id: &str,
) -> Result<SyncJournalStore, String> {
    let dir = WorkspaceDirectory::open(root, sync_dir_id(workspace_id)).map_err(|e| e.to_string())?;
    SyncJournalStore::open(&dir, profile_id, workspace_id).map_err(|e| e.to_string())
}

fn profile_and_token(
    root: &Path,
    profile_id: &str,
) -> Result<(ServerProfile, String), String> {
    let store = load_server_profiles(root);
    let prof = store
        .profiles
        .into_iter()
        .find(|p| p.id == profile_id)
        .ok_or_else(|| "Server profile not found".to_owned())?;
    let tok = read_session_token(root, &prof)?;
    Ok((prof, tok))
}

fn now_secs() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

#[allow(clippy::result_large_err)]
pub fn handle_sync_command(
    runtime: &tokio::runtime::Runtime,
    root: &Path,
    downloads: &Path,
    cmd: Command,
) -> Result<Event, Command> {
    match cmd {
        Command::StageAndPublishEntity {
            profile_id,
            workspace_id,
            entity_id,
            name,
            kind,
            payload_json,
            delete,
            note,
        } => {
            let now = now_secs();
            let mut journal = match open_sync_journal(root, &profile_id, &workspace_id) {
                Ok(j) => j,
                Err(e) => return Ok(Event::ActionError(e)),
            };
            let mut st = journal.state().clone();
            let base_rev = st
                .base_entities
                .get(&entity_id)
                .map(|b| b.revision)
                .unwrap_or(0);
            st.save_local_draft(LocalDraft {
                entity_id: entity_id.clone(),
                parent_id: None,
                kind,
                name,
                order_key: 0,
                payload_json,
                base_revision: base_rev,
                updated_at_unix: now,
            });
            if let Err(e) = st.enqueue_publish(&entity_id, delete, &note, now) {
                return Ok(Event::ActionError(e));
            }
            // Durably commit draft + outbox BEFORE attempting network publish (SY01)
            if let Err(e) = journal.save(st.clone()) {
                return Ok(Event::ActionError(e.to_string()));
            }
            let (synced, msg) = flush_outbox_and_catch_up(runtime, root, downloads, &profile_id, &workspace_id, st);
            if let Err(e) = journal.save(synced.clone()) {
                return Ok(Event::ActionError(e.to_string()));
            }
            Ok(Event::SyncStateUpdated {
                state: Box::new(synced),
                message: msg,
            })
        }
        Command::SyncNow {
            profile_id,
            workspace_id,
        } => {
            let mut journal = match open_sync_journal(root, &profile_id, &workspace_id) {
                Ok(j) => j,
                Err(e) => return Ok(Event::ActionError(e)),
            };
            let st = journal.state().clone();
            let (synced, msg) =
                flush_outbox_and_catch_up(runtime, root, downloads, &profile_id, &workspace_id, st);
            if let Err(e) = journal.save(synced.clone()) {
                return Ok(Event::ActionError(e.to_string()));
            }
            Ok(Event::SyncStateUpdated {
                state: Box::new(synced),
                message: msg,
            })
        }
        Command::ResolveSyncConflict {
            profile_id,
            workspace_id,
            operation_id,
            choice,
        } => {
            let now = now_secs();
            let mut journal = match open_sync_journal(root, &profile_id, &workspace_id) {
                Ok(j) => j,
                Err(e) => return Ok(Event::ActionError(e)),
            };
            let mut st = journal.state().clone();
            if let Err(e) = st.resolve_conflict(&operation_id, choice, now) {
                return Ok(Event::ActionError(e));
            }
            if let Err(e) = journal.save(st.clone()) {
                return Ok(Event::ActionError(e.to_string()));
            }
            let (synced, msg) =
                flush_outbox_and_catch_up(runtime, root, downloads, &profile_id, &workspace_id, st);
            let _ = journal.save(synced.clone());
            Ok(Event::SyncStateUpdated {
                state: Box::new(synced),
                message: msg,
            })
        }
        Command::LoadEntityRevisions {
            profile_id,
            workspace_id,
            entity_id,
        } => {
            let (prof, tok) = match profile_and_token(root, &profile_id) {
                Ok(v) => v,
                Err(e) => return Ok(Event::ActionError(e)),
            };
            let path = format!("/v1/workspaces/{workspace_id}/entities/{entity_id}/revisions");
            Ok(
                match runtime.block_on(server_call::<(), RevisionListResponse>(
                    &prof,
                    HttpMethod::Get,
                    &path,
                    Some(&tok),
                    None,
                    downloads,
                )) {
                    Ok((resp, _)) => Event::EntityRevisionsLoaded(resp.revisions),
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::RestoreEntityRevision {
            profile_id,
            workspace_id,
            entity_id,
            target_revision,
            expected_current_revision,
        } => {
            let (prof, tok) = match profile_and_token(root, &profile_id) {
                Ok(v) => v,
                Err(e) => return Ok(Event::ActionError(e)),
            };
            let path = format!("/v1/workspaces/{workspace_id}/entities/{entity_id}/restore");
            let body = RestoreRevisionRequest {
                operation_id: Uuid::new_v4().to_string(),
                target_revision,
                expected_current_revision,
                note: format!("Restored revision {target_revision}"),
            };
            Ok(
                match runtime.block_on(server_call::<RestoreRevisionRequest, PublishBatchResponse>(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    Some(&body),
                    downloads,
                )) {
                    Ok((_, _)) => Event::FileSaved {
                        message: format!("Restored revision {target_revision} as a new version"),
                    },
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::LoadCollabData {
            profile_id,
            workspace_id,
        } => {
            let (prof, tok) = match profile_and_token(root, &profile_id) {
                Ok(v) => v,
                Err(e) => return Ok(Event::ActionError(e)),
            };
            let cpath = format!("/v1/workspaces/{workspace_id}/comments");
            let comments = runtime
                .block_on(server_call::<(), CommentListResponse>(
                    &prof,
                    HttpMethod::Get,
                    &cpath,
                    Some(&tok),
                    None,
                    downloads,
                ))
                .map(|(c, _)| c.comments)
                .unwrap_or_default();
            let npath = format!("/v1/workspaces/{workspace_id}/notifications");
            let notifs = runtime
                .block_on(server_call::<(), NotificationListResponse>(
                    &prof,
                    HttpMethod::Get,
                    &npath,
                    Some(&tok),
                    None,
                    downloads,
                ))
                .unwrap_or((
                    NotificationListResponse {
                        api_version: 1,
                        unread_count: 0,
                        notifications: Vec::new(),
                    },
                    0,
                ))
                .0;
            Ok(Event::CollabDataLoaded {
                comments,
                notifications: notifs.notifications,
                unread_count: notifs.unread_count,
            })
        }
        Command::CreateTeamComment {
            profile_id,
            workspace_id,
            entity_id,
            anchor_revision,
            body,
        } => {
            let (prof, tok) = match profile_and_token(root, &profile_id) {
                Ok(v) => v,
                Err(e) => return Ok(Event::ActionError(e)),
            };
            let path = format!("/v1/workspaces/{workspace_id}/comments");
            let req = CreateCommentRequest {
                comment_id: Uuid::new_v4().to_string(),
                entity_id,
                anchor_revision,
                body,
            };
            Ok(
                match runtime.block_on(server_call::<CreateCommentRequest, CommentThreadRecord>(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    Some(&req),
                    downloads,
                )) {
                    Ok(_) => Event::FileSaved {
                        message: "Comment posted".into(),
                    },
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::ResolveTeamComment {
            profile_id,
            workspace_id,
            comment_id,
            resolved,
        } => {
            let (prof, tok) = match profile_and_token(root, &profile_id) {
                Ok(v) => v,
                Err(e) => return Ok(Event::ActionError(e)),
            };
            let path = format!("/v1/workspaces/{workspace_id}/comments/{comment_id}/resolve");
            let req = ResolveCommentRequest { resolved };
            Ok(
                match runtime.block_on(server_call::<ResolveCommentRequest, CommentThreadRecord>(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    Some(&req),
                    downloads,
                )) {
                    Ok(_) => Event::FileSaved {
                        message: "Comment updated".into(),
                    },
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::MarkTeamNotificationsRead {
            profile_id,
            workspace_id,
        } => {
            let (prof, tok) = match profile_and_token(root, &profile_id) {
                Ok(v) => v,
                Err(e) => return Ok(Event::ActionError(e)),
            };
            let path = format!("/v1/workspaces/{workspace_id}/notifications/read");
            let req = MarkNotificationsReadRequest {
                notification_ids: Vec::new(),
            };
            Ok(
                match runtime.block_on(server_call_empty(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    Some(&req),
                    downloads,
                )) {
                    Ok(()) => Event::FileSaved {
                        message: "Marked all notifications read".into(),
                    },
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        other => Err(other),
    }
}

fn flush_outbox_and_catch_up(
    runtime: &tokio::runtime::Runtime,
    root: &Path,
    downloads: &Path,
    profile_id: &str,
    workspace_id: &str,
    mut st: TeamSyncState,
) -> (TeamSyncState, String) {
    let now = now_secs();
    let (prof, tok) = match profile_and_token(root, profile_id) {
        Ok(v) => v,
        Err(msg) => {
            let pending_ids: Vec<String> = st
                .outbox
                .iter()
                .filter(|e| e.status.is_sendable(now))
                .map(|e| e.operation_id.clone())
                .collect();
            st.apply_network_retry(&pending_ids, &msg, now);
            return (st, msg);
        }
    };

    // 1. Replay missed events (or fallback to snapshot on HTTP 410 cursor_expired)
    let mut catch_up_pages = 0;
    loop {
        if catch_up_pages >= 20 {
            break;
        }
        catch_up_pages += 1;
        let path = format!(
            "/v1/workspaces/{workspace_id}/changes?after_cursor={}&limit=100",
            st.last_cursor
        );
        match runtime.block_on(server_call::<(), ChangesFeedResponse>(
            &prof,
            HttpMethod::Get,
            &path,
            Some(&tok),
            None,
            downloads,
        )) {
            Ok((feed, _)) => {
                let more = feed.has_more;
                st.apply_changes_batch(&feed, now);
                if !more {
                    break;
                }
            }
            Err(err) if err.contains("cursor_expired") => {
                st.cursor_expired_notice = true;
                let mut pages = Vec::new();
                let mut offset = 0usize;
                loop {
                    let spath = format!(
                        "/v1/workspaces/{workspace_id}/snapshot?offset={offset}&limit=200"
                    );
                    match runtime.block_on(server_call::<(), SnapshotResponse>(
                        &prof,
                        HttpMethod::Get,
                        &spath,
                        Some(&tok),
                        None,
                        downloads,
                    )) {
                        Ok((page, _)) => {
                            let next = page.next_offset;
                            pages.push(page);
                            if let Some(off) = next {
                                offset = off;
                            } else {
                                break;
                            }
                        }
                        Err(e) => return (st, format!("Snapshot resync failed: {e}")),
                    }
                }
                if let Err(e) = st.stage_and_swap_snapshot(&pages, now) {
                    return (st, e);
                }
                break;
            }
            Err(err) => {
                let send_ids: Vec<String> = st
                    .outbox
                    .iter()
                    .filter(|e| e.status.is_sendable(now))
                    .map(|e| e.operation_id.clone())
                    .collect();
                st.apply_network_retry(&send_ids, &err, now);
                return (st, format!("Offline / sync catch-up deferred: {err}"));
            }
        }
    }

    // 2. Send durable pending outbox operations with idempotency keys
    let ops = st.prepare_send_batch(now);
    if ops.is_empty() {
        let cur = st.last_cursor;
        return (st, format!("Synced cleanly at cursor {cur}"));
    }
    let op_ids: Vec<String> = ops.iter().map(|o| o.operation_id.clone()).collect();
    let req = PublishBatchRequest {
        expected_membership_revision: Some(st.membership_revision),
        operations: ops,
    };
    let pub_path = format!("/v1/workspaces/{workspace_id}/publish");
    match runtime.block_on(server_call::<PublishBatchRequest, PublishBatchResponse>(
        &prof,
        HttpMethod::Post,
        &pub_path,
        Some(&tok),
        Some(&req),
        downloads,
    )) {
        Ok((resp, _)) => {
            st.apply_publish_response(&resp, now);
            let conflicts = resp
                .results
                .iter()
                .filter(|r| {
                    matches!(
                        r.kind,
                        swaggen_api_contract::PublishResultKind::ConflictRevision
                            | swaggen_api_contract::PublishResultKind::ConflictDeletedRemote
                    )
                })
                .count();
            let msg = if conflicts > 0 {
                format!("{conflicts} conflict(s) detected; review 3-way diff")
            } else {
                format!("Published {} operation(s)", resp.results.len())
            };
            (st, msg)
        }
        Err(err) if err.contains("403") || err.contains("forbidden") => {
            st.apply_permission_blocked(&op_ids, &err, now);
            (st, "Workspace write permission revoked; drafts kept locally".into())
        }
        Err(err) => {
            st.apply_network_retry(&op_ids, &err, now);
            (st, format!("Publish deferred to retry queue: {err}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::Worker;
    use std::time::Duration;

    #[test]
    fn offline_publish_persists_pending_outbox_across_worker_restart() {
        let root = std::env::temp_dir().join(format!(
            "swaggen-p05-offline-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let worker = Worker::start(root.clone(), LocalId::parse("personal").unwrap(), || {});
        assert!(matches!(
            worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Loaded { .. }
        ));
        worker
            .commands
            .send(Command::StageAndPublishEntity {
                profile_id: "offline-prof".into(),
                workspace_id: "ws-team-1".into(),
                entity_id: "req-offline".into(),
                name: "Offline Request".into(),
                kind: "request".into(),
                payload_json: r#"{"url":"https://api.test/offline"}"#.into(),
                delete: false,
                note: "queued while disconnected".into(),
            })
            .unwrap();
        match worker.events.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::SyncStateUpdated { state, .. } => {
                assert_eq!(state.outbox.len(), 1);
                assert_eq!(state.outbox[0].status.label(), "retry_wait");
            }
            _ => panic!("expected SyncStateUpdated"),
        }
        drop(worker);

        // Reopen journal directly and verify pending outbox & draft survived restart (SY01 / AC05)
        let reopened = open_sync_journal(&root, "offline-prof", "ws-team-1").unwrap();
        assert_eq!(reopened.state().outbox.len(), 1);
        assert_eq!(reopened.state().outbox[0].entity_id, "req-offline");
        assert!(reopened.state().drafts.contains_key("req-offline"));
        drop(reopened);
        let _ = std::fs::remove_dir_all(root);
    }
}
