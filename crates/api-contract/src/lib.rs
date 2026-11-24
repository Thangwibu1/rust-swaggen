//! Versioned public transport types; never include internal errors, DB types, or credentials.
use serde::{Deserialize, Serialize};

pub const API_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Viewer,
    Editor,
    Admin,
    Owner,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }

    pub fn can_manage_workspace(self) -> bool {
        matches!(self, Self::Owner)
    }

    pub fn can_manage_members(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }

    pub fn can_write_entities(self) -> bool {
        matches!(self, Self::Owner | Self::Admin | Self::Editor)
    }

    pub fn can_read_workspace(self) -> bool {
        true
    }

    pub fn can_read_audit(self) -> bool {
        matches!(self, Self::Owner | Self::Admin)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ServiceStatus {
    pub api_version: u32,
    pub status: String,
}

impl ServiceStatus {
    pub fn new(status: &str) -> Self {
        Self {
            api_version: API_VERSION,
            status: status.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub api_version: u32,
    pub server_version: String,
    pub min_desktop_api_version: u32,
    pub collaboration_enabled: bool,
    pub auth_modes: Vec<String>,
    #[serde(default)]
    pub bootstrap_required: bool,
    #[serde(default)]
    pub mongo_ready: bool,
    #[serde(default)]
    pub migration_version: u32,
    #[serde(default)]
    pub max_body_bytes: usize,
    #[serde(default)]
    pub firebase_project_id: Option<String>,
    #[serde(default)]
    pub firebase_web_api_key: Option<String>,
    #[serde(default)]
    pub firebase_auth_domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ErrorEnvelope {
    pub api_version: u32,
    pub code: String,
    pub safe_message: String,
    pub correlation_id: String,
    pub retryable: bool,
}

impl ErrorEnvelope {
    pub fn new(code: &str, safe_message: &str, correlation_id: &str, retryable: bool) -> Self {
        Self {
            api_version: API_VERSION,
            code: code.into(),
            safe_message: safe_message.into(),
            correlation_id: correlation_id.into(),
            retryable,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UserProfile {
    pub user_id: String,
    pub email: String,
    pub display_name: String,
    pub is_server_owner: bool,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BootstrapOwnerRequest {
    pub email: String,
    pub display_name: String,
    pub password: String,
    pub device_name: String,
    #[serde(default)]
    pub bootstrap_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FirebaseLoginRequest {
    pub id_token: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub bootstrap_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthResponse {
    pub api_version: u32,
    pub user: UserProfile,
    pub session_id: String,
    pub session_token: String,
    pub expires_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionSummary {
    pub session_id: String,
    pub device_name: String,
    pub created_at_unix: i64,
    pub expires_at_unix: i64,
    pub last_seen_unix: i64,
    pub revoked_at_unix: Option<i64>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionListResponse {
    pub api_version: u32,
    pub sessions: Vec<SessionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
    #[serde(default)]
    pub revoke_other_sessions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateWorkspaceRequest {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TeamWorkspaceSummary {
    pub workspace_id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub role: Role,
    pub member_count: usize,
    pub current_revision: i64,
    pub membership_revision: i64,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceListResponse {
    pub api_version: u32,
    pub workspaces: Vec<TeamWorkspaceSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateInvitationRequest {
    pub email: String,
    pub role: Role,
    pub expires_in_hours: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InvitationSummary {
    pub invitation_id: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub email: String,
    pub role: Role,
    pub created_at_unix: i64,
    pub expires_at_unix: i64,
    pub used_at_unix: Option<i64>,
    pub revoked_at_unix: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateInvitationResponse {
    pub api_version: u32,
    pub invitation: InvitationSummary,
    pub invite_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AcceptInvitationRequest {
    pub invite_token: String,
    pub email: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub password: Option<String>,
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AcceptInvitationResponse {
    pub api_version: u32,
    pub workspace: TeamWorkspaceSummary,
    pub user: UserProfile,
    pub session_id: String,
    pub session_token: String,
    pub expires_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MemberSummary {
    pub user_id: String,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub joined_at_unix: i64,
    pub membership_revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MemberListResponse {
    pub api_version: u32,
    pub workspace_id: String,
    pub membership_revision: i64,
    pub members: Vec<MemberSummary>,
    pub invitations: Vec<InvitationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UpdateMemberRoleRequest {
    pub role: Role,
    pub expected_membership_revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceEntityRecord {
    pub entity_id: String,
    pub workspace_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub name: String,
    pub order_key: i64,
    pub payload_json: String,
    pub revision: i64,
    pub deleted: bool,
    pub updated_at_unix: i64,
    pub updated_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UpsertEntityRequest {
    pub entity_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub name: String,
    pub order_key: i64,
    pub payload_json: String,
    pub base_revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EntityListResponse {
    pub api_version: u32,
    pub workspace_id: String,
    pub entities: Vec<WorkspaceEntityRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuditEventRecord {
    pub event_id: String,
    pub workspace_id: Option<String>,
    pub actor_user_id: String,
    pub actor_email: String,
    pub action: String,
    pub target_id: String,
    pub detail_redacted: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuditListResponse {
    pub api_version: u32,
    pub events: Vec<AuditEventRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PermissionMatrixRow {
    pub action: &'static str,
    pub owner: bool,
    pub admin: bool,
    pub editor: bool,
    pub viewer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PublishOperation {
    pub operation_id: String,
    pub request_hash: String,
    pub entity_id: String,
    pub parent_id: Option<String>,
    pub kind: String,
    pub name: String,
    pub order_key: i64,
    pub payload_json: String,
    pub base_revision: i64,
    pub delete: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PublishBatchRequest {
    pub expected_membership_revision: Option<i64>,
    pub operations: Vec<PublishOperation>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PublishResultKind {
    Applied,
    AlreadyAcked,
    ConflictRevision,
    ConflictDeletedRemote,
    HashMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PublishOperationResult {
    pub operation_id: String,
    pub entity_id: String,
    pub kind: PublishResultKind,
    pub server_revision: i64,
    pub server_cursor: Option<i64>,
    pub server_entity: Option<WorkspaceEntityRecord>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PublishBatchResponse {
    pub api_version: u32,
    pub results: Vec<PublishOperationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChangeEventRecord {
    pub cursor: i64,
    pub workspace_id: String,
    pub entity_id: String,
    pub entity_kind: String,
    pub entity_name: String,
    pub parent_id: Option<String>,
    pub order_key: i64,
    pub revision: i64,
    pub deleted: bool,
    pub payload_json: Option<String>,
    pub operation_id: String,
    pub actor_user_id: String,
    pub actor_email: String,
    pub note: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChangesFeedResponse {
    pub api_version: u32,
    pub workspace_id: String,
    pub after_cursor: i64,
    pub next_cursor: i64,
    pub has_more: bool,
    pub membership_revision: i64,
    pub events: Vec<ChangeEventRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SnapshotResponse {
    pub api_version: u32,
    pub workspace_id: String,
    pub snapshot_cursor: i64,
    pub snapshot_token: String,
    pub membership_revision: i64,
    pub entities: Vec<WorkspaceEntityRecord>,
    pub next_offset: Option<usize>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EntityRevisionRecord {
    pub workspace_id: String,
    pub entity_id: String,
    pub revision: i64,
    pub deleted: bool,
    pub name: String,
    pub payload_json: String,
    pub actor_user_id: String,
    pub actor_email: String,
    pub note: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RevisionListResponse {
    pub api_version: u32,
    pub entity_id: String,
    pub revisions: Vec<EntityRevisionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RestoreRevisionRequest {
    pub operation_id: String,
    pub target_revision: i64,
    pub expected_current_revision: i64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CommentReplyRecord {
    pub reply_id: String,
    pub author_user_id: String,
    pub author_email: String,
    pub body: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CommentThreadRecord {
    pub comment_id: String,
    pub workspace_id: String,
    pub entity_id: String,
    pub anchor_revision: i64,
    pub author_user_id: String,
    pub author_email: String,
    pub body: String,
    pub resolved: bool,
    pub resolved_by_email: Option<String>,
    pub created_at_unix: i64,
    pub replies: Vec<CommentReplyRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateCommentRequest {
    pub comment_id: String,
    pub entity_id: String,
    pub anchor_revision: i64,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReplyCommentRequest {
    pub reply_id: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResolveCommentRequest {
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CommentListResponse {
    pub api_version: u32,
    pub comments: Vec<CommentThreadRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NotificationRecord {
    pub notification_id: String,
    pub workspace_id: String,
    pub user_id: String,
    pub kind: String,
    pub title: String,
    pub entity_id: Option<String>,
    pub actor_email: String,
    pub created_at_unix: i64,
    pub read: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NotificationListResponse {
    pub api_version: u32,
    pub unread_count: usize,
    pub notifications: Vec<NotificationRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MarkNotificationsReadRequest {
    pub notification_ids: Vec<String>,
}

/// P08 Team Mock Route (MK04).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TeamMockRoute {
    pub mock_id: String,
    pub workspace_id: String,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub content_type: String,
    pub body: String,
    pub delay_ms: u64,
    pub match_header: Option<(String, String)>,
    pub match_query: Option<(String, String)>,
    pub created_by: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateTeamMockRequest {
    pub mock_id: String,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub content_type: String,
    pub body: String,
    pub delay_ms: u64,
    pub match_header: Option<(String, String)>,
    pub match_query: Option<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TeamMockListResponse {
    pub api_version: u32,
    pub workspace_id: String,
    pub routes: Vec<TeamMockRoute>,
    pub max_routes: usize,
}

/// P08 Server Resource Quotas & Retention Limits (MK10).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceQuotaStatus {
    pub api_version: u32,
    pub workspace_id: String,
    pub mock_routes_used: usize,
    pub mock_routes_max: usize,
    pub active_jobs: usize,
    pub max_active_jobs: usize,
    pub max_body_bytes: usize,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SsrfCheckRequest {
    pub url: String,
    pub allowed_hosts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SsrfCheckResponse {
    pub allowed: bool,
    pub reason: String,
}

/// P09 Governance, Ownership Transfer, Presence, TOTP MFA & Branching (WS09, WS10, CO07, SY10, SC09).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceArchiveRequest {
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TransferOwnershipRequest {
    pub target_user_id: String,
    pub expected_membership_revision: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresenceHeartbeatRequest {
    pub entity_id: Option<String>,
    pub state_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresenceEntry {
    pub user_id: String,
    pub email: String,
    pub entity_id: Option<String>,
    pub state_label: String,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresenceListResponse {
    pub api_version: u32,
    pub workspace_id: String,
    pub active: Vec<PresenceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TotpEnrollResponse {
    pub secret_b32: String,
    pub otpauth_uri: String,
    pub recovery_codes_once: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TotpVerifyRequest {
    pub code: Option<String>,
    pub recovery_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TotpStatusResponse {
    pub enabled: bool,
    pub verified: bool,
    pub recovery_remaining: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BranchSummary {
    pub branch_id: String,
    pub workspace_id: String,
    pub name: String,
    pub created_by: String,
    pub created_at_unix: i64,
    pub changes_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateBranchRequest {
    pub name: String,
    pub entity_id: String,
    pub base_hash: String,
    pub branch_hash: String,
    pub branch_payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MergeRequestRecord {
    pub merge_id: String,
    pub workspace_id: String,
    pub branch_id: String,
    pub title: String,
    pub status: String,
    pub approved_by: Vec<String>,
    pub conflicts_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OpenMergeRequest {
    pub branch_id: String,
    pub title: String,
}

/// P10 Team Secret Vault DTOs with explicit trust-boundary disclosure (EV10).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PutVaultSecretRequest {
    pub value: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VaultSecretSummary {
    pub key: String,
    pub version: u64,
    pub updated_by: String,
    pub updated_at_unix: i64,
    pub note: String,
    pub trust_boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VaultSecretListResponse {
    pub api_version: u32,
    pub workspace_id: String,
    pub secrets: Vec<VaultSecretSummary>,
    pub trust_boundary_notice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VaultSecretValueResponse {
    pub key: String,
    pub version: u64,
    pub value: String,
}

/// P11 Self-Hosted Runner Lease & Schedule Monitor DTOs (MK07, MK08, MK09).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RegisterRunnerRequest {
    pub runner_name: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunnerLeaseRecord {
    pub runner_id: String,
    pub workspace_id: String,
    pub runner_name: String,
    pub tags: Vec<String>,
    pub registered_by: String,
    pub last_heartbeat_unix: i64,
    pub lease_expires_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateScheduleRequest {
    pub name: String,
    pub cron_expr: String,
    pub timezone_offset_secs: i64,
    pub failure_webhook_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScheduleRecord {
    pub schedule_id: String,
    pub workspace_id: String,
    pub name: String,
    pub cron_expr: String,
    pub timezone_offset_secs: i64,
    pub next_fire_unix: i64,
    pub failure_webhook_url: Option<String>,
}

pub fn permission_matrix() -> Vec<PermissionMatrixRow> {
    vec![
        PermissionMatrixRow {
            action: "Read collections, requests, specs & environments",
            owner: true,
            admin: true,
            editor: true,
            viewer: true,
        },
        PermissionMatrixRow {
            action: "Publish/edit requests, collections, specs & comments",
            owner: true,
            admin: true,
            editor: true,
            viewer: false,
        },
        PermissionMatrixRow {
            action: "Create/revoke invitations & change Editor/Viewer roles",
            owner: true,
            admin: true,
            editor: false,
            viewer: false,
        },
        PermissionMatrixRow {
            action: "View workspace security audit log",
            owner: true,
            admin: true,
            editor: false,
            viewer: false,
        },
        PermissionMatrixRow {
            action: "Transfer ownership, archive or delete workspace",
            owner: true,
            admin: false,
            editor: false,
            viewer: false,
        },
    ]
}
