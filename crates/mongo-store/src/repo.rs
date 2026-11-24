//! Database-agnostic repository trait shared across MongoStore (production) and InMemoryStore (tests/dev).
use crate::error::StoreError;
use swaggen_api_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AuditEventRecord, AuthResponse,
    BootstrapOwnerRequest, ChangePasswordRequest, ChangesFeedResponse, CommentThreadRecord,
    CreateCommentRequest, CreateInvitationRequest, CreateInvitationResponse,
    CreateWorkspaceRequest, EntityRevisionRecord, LoginRequest, MemberListResponse, MemberSummary,
    NotificationListResponse, PublishBatchRequest, PublishBatchResponse, ReplyCommentRequest,
    ResolveCommentRequest, RestoreRevisionRequest, SessionSummary, SnapshotResponse,
    TeamWorkspaceSummary, UpdateMemberRoleRequest, UpsertEntityRequest, WorkspaceEntityRecord,
};

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: String,
    pub email: String,
    pub display_name: String,
    pub is_server_owner: bool,
    pub session_id: String,
}

pub fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

#[allow(async_fn_in_trait)]
pub trait WorkspaceRepository: Send + Sync {
    async fn check_ready(&self) -> Result<(), StoreError>;
    async fn bootstrap_required(&self) -> Result<bool, StoreError>;

    async fn bootstrap_owner(
        &self,
        req: BootstrapOwnerRequest,
        now: i64,
    ) -> Result<AuthResponse, StoreError>;

    async fn login(
        &self,
        req: LoginRequest,
        client_key: &str,
        now: i64,
    ) -> Result<AuthResponse, StoreError>;

    async fn firebase_login(
        &self,
        firebase_uid: &str,
        email: &str,
        display_name: &str,
        device_name: &str,
        bootstrap_token: Option<&str>,
        now: i64,
    ) -> Result<AuthResponse, StoreError>;

    async fn authenticate_bearer(
        &self,
        bearer_token: &str,
        now: i64,
    ) -> Result<AuthContext, StoreError>;

    async fn list_sessions(
        &self,
        auth: &AuthContext,
        now: i64,
    ) -> Result<Vec<SessionSummary>, StoreError>;

    async fn revoke_session(
        &self,
        auth: &AuthContext,
        session_id: &str,
        now: i64,
    ) -> Result<(), StoreError>;

    async fn logout_all(&self, auth: &AuthContext, now: i64) -> Result<usize, StoreError>;

    async fn change_password(
        &self,
        auth: &AuthContext,
        req: ChangePasswordRequest,
        now: i64,
    ) -> Result<(), StoreError>;

    async fn create_workspace(
        &self,
        auth: &AuthContext,
        req: CreateWorkspaceRequest,
        now: i64,
    ) -> Result<TeamWorkspaceSummary, StoreError>;

    async fn list_workspaces(
        &self,
        auth: &AuthContext,
    ) -> Result<Vec<TeamWorkspaceSummary>, StoreError>;

    async fn list_members_and_invitations(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<MemberListResponse, StoreError>;

    async fn create_invitation(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: CreateInvitationRequest,
        now: i64,
    ) -> Result<CreateInvitationResponse, StoreError>;

    async fn revoke_invitation(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        invitation_id: &str,
        now: i64,
    ) -> Result<(), StoreError>;

    async fn accept_invitation(
        &self,
        caller: Option<&AuthContext>,
        req: AcceptInvitationRequest,
        now: i64,
    ) -> Result<AcceptInvitationResponse, StoreError>;

    async fn update_member_role(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        target_user_id: &str,
        req: UpdateMemberRoleRequest,
        now: i64,
    ) -> Result<MemberSummary, StoreError>;

    async fn list_entities(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<Vec<WorkspaceEntityRecord>, StoreError>;

    async fn upsert_entity(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: UpsertEntityRequest,
        now: i64,
    ) -> Result<WorkspaceEntityRecord, StoreError>;

    async fn list_audit_events(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        limit: usize,
    ) -> Result<Vec<AuditEventRecord>, StoreError>;

    async fn publish_batch(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: PublishBatchRequest,
        now: i64,
    ) -> Result<PublishBatchResponse, StoreError>;

    async fn get_changes(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        after_cursor: i64,
        limit: usize,
    ) -> Result<ChangesFeedResponse, StoreError>;

    async fn get_snapshot(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<SnapshotResponse, StoreError>;

    async fn list_entity_revisions(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        entity_id: &str,
    ) -> Result<Vec<EntityRevisionRecord>, StoreError>;

    async fn restore_entity_revision(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        entity_id: &str,
        req: RestoreRevisionRequest,
        now: i64,
    ) -> Result<PublishBatchResponse, StoreError>;

    async fn create_comment(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: CreateCommentRequest,
        now: i64,
    ) -> Result<CommentThreadRecord, StoreError>;

    async fn reply_comment(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        comment_id: &str,
        req: ReplyCommentRequest,
        now: i64,
    ) -> Result<CommentThreadRecord, StoreError>;

    async fn resolve_comment(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        comment_id: &str,
        req: ResolveCommentRequest,
        now: i64,
    ) -> Result<CommentThreadRecord, StoreError>;

    async fn list_comments(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        entity_id: Option<&str>,
    ) -> Result<Vec<CommentThreadRecord>, StoreError>;

    async fn list_notifications(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<NotificationListResponse, StoreError>;

    async fn mark_notifications_read(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        ids: &[String],
    ) -> Result<usize, StoreError>;

    async fn set_workspace_archived(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        archived: bool,
        now: i64,
    ) -> Result<TeamWorkspaceSummary, StoreError>;

    async fn transfer_workspace_ownership(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: swaggen_api_contract::TransferOwnershipRequest,
        now: i64,
    ) -> Result<MemberListResponse, StoreError>;

    async fn leave_workspace(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        now: i64,
    ) -> Result<(), StoreError>;
}
