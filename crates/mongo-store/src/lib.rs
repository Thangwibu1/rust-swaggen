//! Server-only MongoDB replica-set repository, internal auth, and workspace RBAC.
pub mod backup;
pub mod crypto;
pub mod error;
pub mod memory_backend;
pub mod mongo_backend;
pub mod repo;

use error::StoreError;
use memory_backend::InMemoryStore;
use mongo_backend::MongoStore;
use repo::{AuthContext, BatchedWorkspaceChildren, IdempotencyClaimOutcome, WorkspaceRepository};
use swaggen_api_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AuditEventRecord, AuthResponse,
    BootstrapOwnerRequest, ChangePasswordRequest, ChangesFeedResponse, CommentThreadRecord,
    CreateCommentRequest, CreateInvitationRequest, CreateInvitationResponse,
    CreatePublicInviteLinkRequest, CreatePublicInviteLinkResponse, CreateWorkspaceRequest,
    EntityRevisionRecord, InvitationSummary, LoginRequest, MemberListResponse, MemberSummary,
    NotificationListResponse, PublicInviteLinkSummary, PublishBatchRequest, PublishBatchResponse,
    ReplyCommentRequest, ResolveCommentRequest, RestoreRevisionRequest, SessionSummary,
    SnapshotResponse, TeamWorkspaceSummary, UpdateMemberRoleRequest, UpsertEntityRequest,
    WorkspaceEntityRecord,
};

#[allow(clippy::large_enum_variant)]
pub enum ServerStore {
    Mongo(MongoStore),
    Memory(InMemoryStore),
}

impl ServerStore {
    pub fn is_mongo(&self) -> bool {
        matches!(self, Self::Mongo(_))
    }

    pub async fn run_doctor_probe(&self) -> Result<(), StoreError> {
        match self {
            Self::Mongo(m) => {
                m.verify_replica_set_topology().await?;
                m.run_transaction_rollback_smoke_test().await
            }
            Self::Memory(_) => Err(StoreError::DatabaseUnavailable),
        }
    }
}

impl WorkspaceRepository for ServerStore {
    async fn check_ready(&self) -> Result<(), StoreError> {
        match self {
            Self::Mongo(m) => m.check_ready().await,
            Self::Memory(m) => m.check_ready().await,
        }
    }
    async fn bootstrap_required(&self) -> Result<bool, StoreError> {
        match self {
            Self::Mongo(m) => m.bootstrap_required().await,
            Self::Memory(m) => m.bootstrap_required().await,
        }
    }
    async fn bootstrap_owner(
        &self,
        req: BootstrapOwnerRequest,
        now: i64,
    ) -> Result<AuthResponse, StoreError> {
        match self {
            Self::Mongo(m) => m.bootstrap_owner(req, now).await,
            Self::Memory(m) => m.bootstrap_owner(req, now).await,
        }
    }
    async fn login(
        &self,
        req: LoginRequest,
        client_key: &str,
        now: i64,
    ) -> Result<AuthResponse, StoreError> {
        match self {
            Self::Mongo(m) => m.login(req, client_key, now).await,
            Self::Memory(m) => m.login(req, client_key, now).await,
        }
    }
    async fn firebase_login(
        &self,
        firebase_uid: &str,
        email: &str,
        display_name: &str,
        device_name: &str,
        bootstrap_token: Option<&str>,
        now: i64,
    ) -> Result<AuthResponse, StoreError> {
        match self {
            Self::Mongo(m) => {
                m.firebase_login(
                    firebase_uid,
                    email,
                    display_name,
                    device_name,
                    bootstrap_token,
                    now,
                )
                .await
            }
            Self::Memory(m) => {
                m.firebase_login(
                    firebase_uid,
                    email,
                    display_name,
                    device_name,
                    bootstrap_token,
                    now,
                )
                .await
            }
        }
    }
    async fn authenticate_bearer(
        &self,
        bearer_token: &str,
        now: i64,
    ) -> Result<AuthContext, StoreError> {
        match self {
            Self::Mongo(m) => m.authenticate_bearer(bearer_token, now).await,
            Self::Memory(m) => m.authenticate_bearer(bearer_token, now).await,
        }
    }
    async fn list_sessions(
        &self,
        auth: &AuthContext,
        now: i64,
    ) -> Result<Vec<SessionSummary>, StoreError> {
        match self {
            Self::Mongo(m) => m.list_sessions(auth, now).await,
            Self::Memory(m) => m.list_sessions(auth, now).await,
        }
    }
    async fn revoke_session(
        &self,
        auth: &AuthContext,
        session_id: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        match self {
            Self::Mongo(m) => m.revoke_session(auth, session_id, now).await,
            Self::Memory(m) => m.revoke_session(auth, session_id, now).await,
        }
    }
    async fn logout_all(&self, auth: &AuthContext, now: i64) -> Result<usize, StoreError> {
        match self {
            Self::Mongo(m) => m.logout_all(auth, now).await,
            Self::Memory(m) => m.logout_all(auth, now).await,
        }
    }
    async fn change_password(
        &self,
        auth: &AuthContext,
        req: ChangePasswordRequest,
        now: i64,
    ) -> Result<(), StoreError> {
        match self {
            Self::Mongo(m) => m.change_password(auth, req, now).await,
            Self::Memory(m) => m.change_password(auth, req, now).await,
        }
    }
    async fn create_workspace(
        &self,
        auth: &AuthContext,
        req: CreateWorkspaceRequest,
        now: i64,
    ) -> Result<TeamWorkspaceSummary, StoreError> {
        match self {
            Self::Mongo(m) => m.create_workspace(auth, req, now).await,
            Self::Memory(m) => m.create_workspace(auth, req, now).await,
        }
    }
    async fn list_workspaces(
        &self,
        auth: &AuthContext,
    ) -> Result<Vec<TeamWorkspaceSummary>, StoreError> {
        match self {
            Self::Mongo(m) => m.list_workspaces(auth).await,
            Self::Memory(m) => m.list_workspaces(auth).await,
        }
    }
    async fn list_members_and_invitations(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<MemberListResponse, StoreError> {
        match self {
            Self::Mongo(m) => m.list_members_and_invitations(auth, workspace_id).await,
            Self::Memory(m) => m.list_members_and_invitations(auth, workspace_id).await,
        }
    }
    async fn create_invitation(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: CreateInvitationRequest,
        now: i64,
    ) -> Result<CreateInvitationResponse, StoreError> {
        match self {
            Self::Mongo(m) => m.create_invitation(auth, workspace_id, req, now).await,
            Self::Memory(m) => m.create_invitation(auth, workspace_id, req, now).await,
        }
    }
    async fn revoke_invitation(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        invitation_id: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        match self {
            Self::Mongo(m) => {
                m.revoke_invitation(auth, workspace_id, invitation_id, now)
                    .await
            }
            Self::Memory(m) => {
                m.revoke_invitation(auth, workspace_id, invitation_id, now)
                    .await
            }
        }
    }
    async fn accept_invitation(
        &self,
        caller: Option<&AuthContext>,
        req: AcceptInvitationRequest,
        now: i64,
    ) -> Result<AcceptInvitationResponse, StoreError> {
        match self {
            Self::Mongo(m) => m.accept_invitation(caller, req, now).await,
            Self::Memory(m) => m.accept_invitation(caller, req, now).await,
        }
    }
    async fn list_my_pending_invitations(
        &self,
        auth: &AuthContext,
        now: i64,
    ) -> Result<Vec<InvitationSummary>, StoreError> {
        match self {
            Self::Mongo(m) => m.list_my_pending_invitations(auth, now).await,
            Self::Memory(m) => m.list_my_pending_invitations(auth, now).await,
        }
    }
    async fn accept_invitation_by_id(
        &self,
        auth: &AuthContext,
        invitation_id: &str,
        device_name: &str,
        now: i64,
    ) -> Result<AcceptInvitationResponse, StoreError> {
        match self {
            Self::Mongo(m) => {
                m.accept_invitation_by_id(auth, invitation_id, device_name, now)
                    .await
            }
            Self::Memory(m) => {
                m.accept_invitation_by_id(auth, invitation_id, device_name, now)
                    .await
            }
        }
    }
    async fn decline_invitation(
        &self,
        auth: &AuthContext,
        invitation_id: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        match self {
            Self::Mongo(m) => m.decline_invitation(auth, invitation_id, now).await,
            Self::Memory(m) => m.decline_invitation(auth, invitation_id, now).await,
        }
    }
    async fn create_public_invite_link(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: CreatePublicInviteLinkRequest,
        now: i64,
    ) -> Result<CreatePublicInviteLinkResponse, StoreError> {
        match self {
            Self::Mongo(m) => {
                m.create_public_invite_link(auth, workspace_id, req, now)
                    .await
            }
            Self::Memory(m) => {
                m.create_public_invite_link(auth, workspace_id, req, now)
                    .await
            }
        }
    }
    async fn list_public_invite_links(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<Vec<PublicInviteLinkSummary>, StoreError> {
        match self {
            Self::Mongo(m) => m.list_public_invite_links(auth, workspace_id).await,
            Self::Memory(m) => m.list_public_invite_links(auth, workspace_id).await,
        }
    }
    async fn revoke_public_invite_link(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        link_id: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        match self {
            Self::Mongo(m) => {
                m.revoke_public_invite_link(auth, workspace_id, link_id, now)
                    .await
            }
            Self::Memory(m) => {
                m.revoke_public_invite_link(auth, workspace_id, link_id, now)
                    .await
            }
        }
    }
    async fn join_by_public_invite_link(
        &self,
        auth: &AuthContext,
        link_or_code: &str,
        device_name: &str,
        now: i64,
    ) -> Result<AcceptInvitationResponse, StoreError> {
        match self {
            Self::Mongo(m) => {
                m.join_by_public_invite_link(auth, link_or_code, device_name, now)
                    .await
            }
            Self::Memory(m) => {
                m.join_by_public_invite_link(auth, link_or_code, device_name, now)
                    .await
            }
        }
    }
    async fn update_member_role(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        target_user_id: &str,
        req: UpdateMemberRoleRequest,
        now: i64,
    ) -> Result<MemberSummary, StoreError> {
        match self {
            Self::Mongo(m) => {
                m.update_member_role(auth, workspace_id, target_user_id, req, now)
                    .await
            }
            Self::Memory(m) => {
                m.update_member_role(auth, workspace_id, target_user_id, req, now)
                    .await
            }
        }
    }
    async fn list_entities(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<Vec<WorkspaceEntityRecord>, StoreError> {
        match self {
            Self::Mongo(m) => m.list_entities(auth, workspace_id).await,
            Self::Memory(m) => m.list_entities(auth, workspace_id).await,
        }
    }
    async fn upsert_entity(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: UpsertEntityRequest,
        now: i64,
    ) -> Result<WorkspaceEntityRecord, StoreError> {
        match self {
            Self::Mongo(m) => m.upsert_entity(auth, workspace_id, req, now).await,
            Self::Memory(m) => m.upsert_entity(auth, workspace_id, req, now).await,
        }
    }
    async fn list_audit_events(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        limit: usize,
    ) -> Result<Vec<AuditEventRecord>, StoreError> {
        match self {
            Self::Mongo(m) => m.list_audit_events(auth, workspace_id, limit).await,
            Self::Memory(m) => m.list_audit_events(auth, workspace_id, limit).await,
        }
    }
    async fn publish_batch(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: PublishBatchRequest,
        now: i64,
    ) -> Result<PublishBatchResponse, StoreError> {
        match self {
            Self::Mongo(m) => m.publish_batch(auth, workspace_id, req, now).await,
            Self::Memory(m) => m.publish_batch(auth, workspace_id, req, now).await,
        }
    }
    async fn get_changes(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        after_cursor: i64,
        limit: usize,
    ) -> Result<ChangesFeedResponse, StoreError> {
        match self {
            Self::Mongo(m) => m.get_changes(auth, workspace_id, after_cursor, limit).await,
            Self::Memory(m) => m.get_changes(auth, workspace_id, after_cursor, limit).await,
        }
    }
    async fn get_snapshot(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<SnapshotResponse, StoreError> {
        match self {
            Self::Mongo(m) => m.get_snapshot(auth, workspace_id, offset, limit).await,
            Self::Memory(m) => m.get_snapshot(auth, workspace_id, offset, limit).await,
        }
    }
    async fn list_entity_revisions(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        entity_id: &str,
    ) -> Result<Vec<EntityRevisionRecord>, StoreError> {
        match self {
            Self::Mongo(m) => m.list_entity_revisions(auth, workspace_id, entity_id).await,
            Self::Memory(m) => m.list_entity_revisions(auth, workspace_id, entity_id).await,
        }
    }
    async fn restore_entity_revision(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        entity_id: &str,
        req: RestoreRevisionRequest,
        now: i64,
    ) -> Result<PublishBatchResponse, StoreError> {
        match self {
            Self::Mongo(m) => {
                m.restore_entity_revision(auth, workspace_id, entity_id, req, now)
                    .await
            }
            Self::Memory(m) => {
                m.restore_entity_revision(auth, workspace_id, entity_id, req, now)
                    .await
            }
        }
    }
    async fn create_comment(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: CreateCommentRequest,
        now: i64,
    ) -> Result<CommentThreadRecord, StoreError> {
        match self {
            Self::Mongo(m) => m.create_comment(auth, workspace_id, req, now).await,
            Self::Memory(m) => m.create_comment(auth, workspace_id, req, now).await,
        }
    }
    async fn reply_comment(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        comment_id: &str,
        req: ReplyCommentRequest,
        now: i64,
    ) -> Result<CommentThreadRecord, StoreError> {
        match self {
            Self::Mongo(m) => {
                m.reply_comment(auth, workspace_id, comment_id, req, now)
                    .await
            }
            Self::Memory(m) => {
                m.reply_comment(auth, workspace_id, comment_id, req, now)
                    .await
            }
        }
    }
    async fn resolve_comment(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        comment_id: &str,
        req: ResolveCommentRequest,
        now: i64,
    ) -> Result<CommentThreadRecord, StoreError> {
        match self {
            Self::Mongo(m) => {
                m.resolve_comment(auth, workspace_id, comment_id, req, now)
                    .await
            }
            Self::Memory(m) => {
                m.resolve_comment(auth, workspace_id, comment_id, req, now)
                    .await
            }
        }
    }
    async fn list_comments(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        entity_id: Option<&str>,
    ) -> Result<Vec<CommentThreadRecord>, StoreError> {
        match self {
            Self::Mongo(m) => m.list_comments(auth, workspace_id, entity_id).await,
            Self::Memory(m) => m.list_comments(auth, workspace_id, entity_id).await,
        }
    }
    async fn list_notifications(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<NotificationListResponse, StoreError> {
        match self {
            Self::Mongo(m) => m.list_notifications(auth, workspace_id).await,
            Self::Memory(m) => m.list_notifications(auth, workspace_id).await,
        }
    }
    async fn mark_notifications_read(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        ids: &[String],
    ) -> Result<usize, StoreError> {
        match self {
            Self::Mongo(m) => m.mark_notifications_read(auth, workspace_id, ids).await,
            Self::Memory(m) => m.mark_notifications_read(auth, workspace_id, ids).await,
        }
    }
    async fn set_workspace_archived(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        archived: bool,
        now: i64,
    ) -> Result<TeamWorkspaceSummary, StoreError> {
        match self {
            Self::Mongo(m) => {
                m.set_workspace_archived(auth, workspace_id, archived, now)
                    .await
            }
            Self::Memory(m) => {
                m.set_workspace_archived(auth, workspace_id, archived, now)
                    .await
            }
        }
    }
    async fn transfer_workspace_ownership(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: swaggen_api_contract::TransferOwnershipRequest,
        now: i64,
    ) -> Result<MemberListResponse, StoreError> {
        match self {
            Self::Mongo(m) => {
                m.transfer_workspace_ownership(auth, workspace_id, req, now)
                    .await
            }
            Self::Memory(m) => {
                m.transfer_workspace_ownership(auth, workspace_id, req, now)
                    .await
            }
        }
    }
    async fn leave_workspace(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        match self {
            Self::Mongo(m) => m.leave_workspace(auth, workspace_id, now).await,
            Self::Memory(m) => m.leave_workspace(auth, workspace_id, now).await,
        }
    }
    async fn kv_get(&self, namespace: &str, key: &str) -> Result<Option<String>, StoreError> {
        match self {
            Self::Mongo(m) => m.kv_get(namespace, key).await,
            Self::Memory(m) => m.kv_get(namespace, key).await,
        }
    }
    async fn kv_put(
        &self,
        namespace: &str,
        key: &str,
        value_json: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        match self {
            Self::Mongo(m) => m.kv_put(namespace, key, value_json, now).await,
            Self::Memory(m) => m.kv_put(namespace, key, value_json, now).await,
        }
    }
    async fn kv_delete(&self, namespace: &str, key: &str) -> Result<bool, StoreError> {
        match self {
            Self::Mongo(m) => m.kv_delete(namespace, key).await,
            Self::Memory(m) => m.kv_delete(namespace, key).await,
        }
    }
    async fn kv_list(
        &self,
        namespace: &str,
        key_prefix: &str,
    ) -> Result<Vec<(String, String)>, StoreError> {
        match self {
            Self::Mongo(m) => m.kv_list(namespace, key_prefix).await,
            Self::Memory(m) => m.kv_list(namespace, key_prefix).await,
        }
    }
    async fn claim_idempotency(
        &self,
        scope: &str,
        key: &str,
        request_hash: &str,
        now: i64,
    ) -> Result<IdempotencyClaimOutcome, StoreError> {
        match self {
            Self::Mongo(m) => m.claim_idempotency(scope, key, request_hash, now).await,
            Self::Memory(m) => m.claim_idempotency(scope, key, request_hash, now).await,
        }
    }
    async fn complete_idempotency(
        &self,
        scope: &str,
        key: &str,
        status_code: u16,
        body_json: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        match self {
            Self::Mongo(m) => {
                m.complete_idempotency(scope, key, status_code, body_json, now)
                    .await
            }
            Self::Memory(m) => {
                m.complete_idempotency(scope, key, status_code, body_json, now)
                    .await
            }
        }
    }
    async fn release_idempotency(&self, scope: &str, key: &str) -> Result<(), StoreError> {
        match self {
            Self::Mongo(m) => m.release_idempotency(scope, key).await,
            Self::Memory(m) => m.release_idempotency(scope, key).await,
        }
    }
    async fn batch_load_workspaces_children(
        &self,
        auth: &AuthContext,
        workspace_ids: &[String],
        include_entities: bool,
        include_members: bool,
        include_comments: bool,
        include_notifications: bool,
    ) -> Result<BatchedWorkspaceChildren, StoreError> {
        match self {
            Self::Mongo(m) => {
                m.batch_load_workspaces_children(
                    auth,
                    workspace_ids,
                    include_entities,
                    include_members,
                    include_comments,
                    include_notifications,
                )
                .await
            }
            Self::Memory(m) => {
                m.batch_load_workspaces_children(
                    auth,
                    workspace_ids,
                    include_entities,
                    include_members,
                    include_comments,
                    include_notifications,
                )
                .await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{hash_password, verify_password};
    use swaggen_api_contract::Role;

    #[test]
    fn argon2id_hashes_with_salts_and_verifies() {
        let h1 = hash_password("Swaggen-Secret-Pass-1!").unwrap();
        let h2 = hash_password("Swaggen-Secret-Pass-1!").unwrap();
        assert_ne!(h1, h2);
        assert!(h1.starts_with("$argon2id$"));
        assert!(verify_password("Swaggen-Secret-Pass-1!", &h1));
        assert!(!verify_password("wrong-password-here", &h1));
    }

    #[tokio::test]
    async fn memory_store_enforces_bootstrap_one_time_invite_rbac_and_isolation() {
        let store = ServerStore::Memory(InMemoryStore::new(None));
        assert!(store.bootstrap_required().await.unwrap());
        let owner = store
            .bootstrap_owner(
                BootstrapOwnerRequest {
                    email: "Owner@Example.com".into(),
                    display_name: "Owner".into(),
                    password: "StrongPassword!123".into(),
                    device_name: "Win11".into(),
                    bootstrap_token: None,
                },
                1000,
            )
            .await
            .unwrap();
        // Bootstrap strictly one-time
        assert_eq!(
            store
                .bootstrap_owner(
                    BootstrapOwnerRequest {
                        email: "other@example.com".into(),
                        display_name: "Second".into(),
                        password: "StrongPassword!123".into(),
                        device_name: "Win11".into(),
                        bootstrap_token: None,
                    },
                    1001,
                )
                .await
                .unwrap_err(),
            StoreError::BootstrapCompleted
        );
        let owner_ctx = store
            .authenticate_bearer(&owner.session_token, 1002)
            .await
            .unwrap();
        let ws_a = store
            .create_workspace(
                &owner_ctx,
                CreateWorkspaceRequest {
                    slug: "alpha-api".into(),
                    name: "Alpha API".into(),
                    description: String::new(),
                },
                1003,
            )
            .await
            .unwrap();
        let ws_b = store
            .create_workspace(
                &owner_ctx,
                CreateWorkspaceRequest {
                    slug: "beta-api".into(),
                    name: "Beta API".into(),
                    description: String::new(),
                },
                1004,
            )
            .await
            .unwrap();

        let inv = store
            .create_invitation(
                &owner_ctx,
                &ws_a.workspace_id,
                CreateInvitationRequest {
                    email: "viewer@example.com".into(),
                    role: Role::Viewer,
                    expires_in_hours: 24,
                },
                1005,
            )
            .await
            .unwrap();

        // Account mismatch blocked
        assert_eq!(
            store
                .accept_invitation(
                    None,
                    AcceptInvitationRequest {
                        invite_token: inv.invite_token.clone(),
                        email: "intruder@example.com".into(),
                        display_name: "Intruder".into(),
                        password: Some("StrongPassword!123".into()),
                        device_name: "Win11".into(),
                    },
                    1006,
                )
                .await
                .unwrap_err(),
            StoreError::InvitationAccountMismatch
        );

        // Expired blocked
        assert_eq!(
            store
                .accept_invitation(
                    None,
                    AcceptInvitationRequest {
                        invite_token: inv.invite_token.clone(),
                        email: "viewer@example.com".into(),
                        display_name: "Viewer".into(),
                        password: Some("StrongPassword!123".into()),
                        device_name: "Win11".into(),
                    },
                    1005 + 25 * 3600,
                )
                .await
                .unwrap_err(),
            StoreError::InvitationExpired
        );

        // Valid accept succeeds once
        let joined = store
            .accept_invitation(
                None,
                AcceptInvitationRequest {
                    invite_token: inv.invite_token.clone(),
                    email: "viewer@example.com".into(),
                    display_name: "Viewer".into(),
                    password: Some("StrongPassword!123".into()),
                    device_name: "Win11".into(),
                },
                1007,
            )
            .await
            .unwrap();

        // Replay of used invite fails
        assert_eq!(
            store
                .accept_invitation(
                    None,
                    AcceptInvitationRequest {
                        invite_token: inv.invite_token.clone(),
                        email: "viewer@example.com".into(),
                        display_name: "Viewer".into(),
                        password: Some("StrongPassword!123".into()),
                        device_name: "Win11".into(),
                    },
                    1008,
                )
                .await
                .unwrap_err(),
            StoreError::InvitationAlreadyUsed
        );

        let viewer_ctx = store
            .authenticate_bearer(&joined.session_token, 1009)
            .await
            .unwrap();
        // Viewer read allowed in ws_a, write blocked, cross-workspace ws_b blocked!
        assert!(
            store
                .list_entities(&viewer_ctx, &ws_a.workspace_id)
                .await
                .is_ok()
        );
        assert!(matches!(
            store
                .upsert_entity(
                    &viewer_ctx,
                    &ws_a.workspace_id,
                    UpsertEntityRequest {
                        entity_id: "req-1".into(),
                        parent_id: None,
                        kind: "request".into(),
                        name: "Test".into(),
                        order_key: 1,
                        payload_json: "{}".into(),
                        base_revision: 0,
                    },
                    1010,
                )
                .await
                .unwrap_err(),
            StoreError::Forbidden(_)
        ));
        assert!(matches!(
            store
                .list_entities(&viewer_ctx, &ws_b.workspace_id)
                .await
                .unwrap_err(),
            StoreError::Forbidden(_)
        ));

        // Login rate limiting after 5 failures
        for i in 0..5 {
            let err = store
                .login(
                    LoginRequest {
                        email: "viewer@example.com".into(),
                        password: "WrongPassword!".into(),
                        device_name: "Win11".into(),
                    },
                    "10.0.0.1",
                    2000 + i,
                )
                .await
                .unwrap_err();
            assert!(matches!(err, StoreError::Unauthorized(_)));
        }
        assert!(matches!(
            store
                .login(
                    LoginRequest {
                        email: "viewer@example.com".into(),
                        password: "StrongPassword!123".into(),
                        device_name: "Win11".into(),
                    },
                    "10.0.0.1",
                    2010,
                )
                .await
                .unwrap_err(),
            StoreError::RateLimited(_)
        ));
    }
}
