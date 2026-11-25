//! Transactional-invariant in-memory implementation for offline tests and explicit `--in-memory-dev`.
use crate::{
    crypto::{
        LoginRateLimiter, SESSION_TTL_SECS, generate_opaque_token, hash_password, normalize_email,
        normalize_slug, sha256_hex, validate_password, verify_password,
    },
    error::StoreError,
    repo::{AuthContext, BatchedWorkspaceChildren, IdempotencyClaimOutcome, WorkspaceRepository},
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Mutex,
};
use swaggen_api_contract::{
    API_VERSION, AcceptInvitationRequest, AcceptInvitationResponse, AuditEventRecord, AuthResponse,
    BootstrapOwnerRequest, ChangeEventRecord, ChangePasswordRequest, ChangesFeedResponse,
    CommentReplyRecord, CommentThreadRecord, CreateCommentRequest, CreateInvitationRequest,
    CreateInvitationResponse, CreatePublicInviteLinkRequest, CreatePublicInviteLinkResponse,
    CreateWorkspaceRequest, EntityRevisionRecord, InvitationSummary, LoginRequest,
    MemberListResponse, MemberSummary, NotificationListResponse, NotificationRecord,
    PublicInviteLinkSummary, PublishBatchRequest, PublishBatchResponse, PublishOperationResult,
    PublishResultKind, ReplyCommentRequest, ResolveCommentRequest, RestoreRevisionRequest, Role,
    SessionSummary, SnapshotResponse, TeamWorkspaceSummary, UpdateMemberRoleRequest,
    UpsertEntityRequest, UserProfile, WorkspaceEntityRecord,
};
use uuid::Uuid;

#[derive(Clone)]
struct UserRec {
    user_id: String,
    email_normalized: String,
    display_name: String,
    password_hash: String,
    is_server_owner: bool,
    created_at_unix: i64,
}

#[derive(Clone)]
struct SessionRec {
    session_id: String,
    user_id: String,
    device_name: String,
    created_at_unix: i64,
    expires_at_unix: i64,
    last_seen_unix: i64,
    revoked_at_unix: Option<i64>,
}

#[derive(Clone)]
struct WorkspaceRec {
    workspace_id: String,
    slug: String,
    name: String,
    description: String,
    current_revision: i64,
    membership_revision: i64,
    archived: bool,
}

#[derive(Clone)]
struct MembershipRec {
    role: Role,
    joined_at_unix: i64,
}

#[derive(Clone)]
struct InvitationRec {
    invitation_id: String,
    workspace_id: String,
    email_normalized: String,
    role: Role,
    created_at_unix: i64,
    expires_at_unix: i64,
    used_at_unix: Option<i64>,
    revoked_at_unix: Option<i64>,
}

#[derive(Clone)]
struct AppliedOpRec {
    request_hash: String,
    result: PublishOperationResult,
}

#[derive(Clone)]
struct IdempotencyRec {
    request_hash: String,
    completed: Option<(u16, String)>,
    created_at_unix: i64,
}

#[derive(Default)]
struct State {
    users_by_id: HashMap<String, UserRec>,
    user_id_by_email: HashMap<String, String>,
    user_id_by_firebase_uid: HashMap<String, String>,
    sessions_by_id: HashMap<String, SessionRec>,
    session_id_by_hash: HashMap<String, String>,
    workspaces_by_id: HashMap<String, WorkspaceRec>,
    workspace_id_by_slug: HashMap<String, String>,
    memberships: BTreeMap<(String, String), MembershipRec>,
    invitations_by_id: HashMap<String, InvitationRec>,
    invitation_id_by_hash: HashMap<String, String>,
    public_links_by_id: HashMap<String, PublicInviteLinkSummary>,
    public_link_id_by_hash: HashMap<String, String>,
    entities: BTreeMap<(String, String), WorkspaceEntityRecord>,
    revisions: BTreeMap<(String, String, i64), EntityRevisionRecord>,
    applied_ops: HashMap<(String, String, String), AppliedOpRec>,
    cursors: HashMap<String, i64>,
    min_retained_cursor: HashMap<String, i64>,
    change_events: BTreeMap<(String, i64), ChangeEventRecord>,
    comments: BTreeMap<(String, String), CommentThreadRecord>,
    notifications: Vec<NotificationRecord>,
    audit_events: Vec<AuditEventRecord>,
    kv_docs: BTreeMap<(String, String), String>,
    idempotency_records: HashMap<(String, String), IdempotencyRec>,
}

impl InMemoryStore {
    pub fn set_min_retained_cursor(&self, workspace_id: &str, min_cursor: i64) {
        if let Ok(mut guard) = self.state.lock() {
            guard
                .min_retained_cursor
                .insert(workspace_id.to_owned(), min_cursor);
        }
    }
}

pub struct InMemoryStore {
    bootstrap_secret: Option<String>,
    limiter: LoginRateLimiter,
    state: Mutex<State>,
}

impl InMemoryStore {
    pub fn new(bootstrap_secret: Option<String>) -> Self {
        Self {
            bootstrap_secret,
            limiter: LoginRateLimiter::default(),
            state: Mutex::new(State::default()),
        }
    }

    fn issue_session(
        state: &mut State,
        user_id: &str,
        device_name: &str,
        now: i64,
    ) -> (String, String, i64) {
        let session_id = Uuid::new_v4().to_string();
        let token = generate_opaque_token("swg");
        let token_hash = sha256_hex(&token);
        let expires = now + SESSION_TTL_SECS;
        state
            .session_id_by_hash
            .insert(token_hash.clone(), session_id.clone());
        state.sessions_by_id.insert(
            session_id.clone(),
            SessionRec {
                session_id: session_id.clone(),
                user_id: user_id.to_owned(),
                device_name: if device_name.trim().is_empty() {
                    "Swaggen Desktop".to_owned()
                } else {
                    device_name.trim().chars().take(80).collect()
                },
                created_at_unix: now,
                expires_at_unix: expires,
                last_seen_unix: now,
                revoked_at_unix: None,
            },
        );
        (session_id, token, expires)
    }

    #[allow(clippy::too_many_arguments)]
    fn record_audit(
        state: &mut State,
        workspace_id: Option<&str>,
        actor_user_id: &str,
        actor_email: &str,
        action: &str,
        target_id: &str,
        detail_redacted: &str,
        now: i64,
    ) {
        state.audit_events.push(AuditEventRecord {
            event_id: Uuid::new_v4().to_string(),
            workspace_id: workspace_id.map(ToOwned::to_owned),
            actor_user_id: actor_user_id.to_owned(),
            actor_email: actor_email.to_owned(),
            action: action.to_owned(),
            target_id: target_id.to_owned(),
            detail_redacted: detail_redacted.to_owned(),
            created_at_unix: now,
        });
    }

    fn user_profile(u: &UserRec) -> UserProfile {
        UserProfile {
            user_id: u.user_id.clone(),
            email: u.email_normalized.clone(),
            display_name: u.display_name.clone(),
            is_server_owner: u.is_server_owner,
            created_at_unix: u.created_at_unix,
        }
    }

    fn require_membership<'a>(
        state: &'a State,
        workspace_id: &str,
        user_id: &str,
    ) -> Result<(&'a WorkspaceRec, &'a MembershipRec), StoreError> {
        let m = state
            .memberships
            .get(&(workspace_id.to_owned(), user_id.to_owned()))
            .ok_or_else(|| StoreError::Forbidden("No access to this workspace".into()))?;
        let ws = state
            .workspaces_by_id
            .get(workspace_id)
            .ok_or_else(|| StoreError::NotFound("Workspace not found".into()))?;
        Ok((ws, m))
    }

    fn workspace_summary(state: &State, ws: &WorkspaceRec, role: Role) -> TeamWorkspaceSummary {
        let member_count = state
            .memberships
            .keys()
            .filter(|(wid, _)| wid == &ws.workspace_id)
            .count();
        TeamWorkspaceSummary {
            workspace_id: ws.workspace_id.clone(),
            slug: ws.slug.clone(),
            name: ws.name.clone(),
            description: ws.description.clone(),
            role,
            member_count,
            current_revision: ws.current_revision,
            membership_revision: ws.membership_revision,
            archived: ws.archived,
        }
    }
}

impl WorkspaceRepository for InMemoryStore {
    async fn check_ready(&self) -> Result<(), StoreError> {
        Ok(())
    }

    async fn bootstrap_required(&self) -> Result<bool, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        Ok(guard.users_by_id.is_empty())
    }

    async fn bootstrap_owner(
        &self,
        req: BootstrapOwnerRequest,
        now: i64,
    ) -> Result<AuthResponse, StoreError> {
        if let Some(expected) = &self.bootstrap_secret
            && req.bootstrap_token.as_deref() != Some(expected.as_str())
        {
            return Err(StoreError::Unauthorized(
                "Invalid bootstrap setup token".into(),
            ));
        }
        let email = normalize_email(&req.email)?;
        let display_name = req.display_name.trim();
        if display_name.is_empty() || display_name.len() > 80 {
            return Err(StoreError::InvalidInput(
                "Display name must be 1-80 characters".into(),
            ));
        }
        let pw_hash = hash_password(&req.password)?;
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        if !guard.users_by_id.is_empty() {
            return Err(StoreError::BootstrapCompleted);
        }
        let user_id = Uuid::new_v4().to_string();
        let user = UserRec {
            user_id: user_id.clone(),
            email_normalized: email.clone(),
            display_name: display_name.to_owned(),
            password_hash: pw_hash,
            is_server_owner: true,
            created_at_unix: now,
        };
        guard.user_id_by_email.insert(email, user_id.clone());
        guard.users_by_id.insert(user_id.clone(), user.clone());
        let (session_id, token, expires) =
            Self::issue_session(&mut guard, &user_id, &req.device_name, now);
        Self::record_audit(
            &mut guard,
            None,
            &user_id,
            &user.email_normalized,
            "server.bootstrap_owner",
            &user_id,
            "First server owner account initialized",
            now,
        );
        Ok(AuthResponse {
            api_version: API_VERSION,
            user: Self::user_profile(&user),
            session_id,
            session_token: token,
            expires_at_unix: expires,
        })
    }

    async fn login(
        &self,
        req: LoginRequest,
        client_key: &str,
        now: i64,
    ) -> Result<AuthResponse, StoreError> {
        let email = normalize_email(&req.email)?;
        let rate_key = format!("{email}|{client_key}");
        self.limiter.check_allowed(&rate_key, now)?;

        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let user = guard
            .user_id_by_email
            .get(&email)
            .and_then(|id| guard.users_by_id.get(id))
            .cloned();
        let Some(user) = user else {
            self.limiter.record_failure(&rate_key, now);
            return Err(StoreError::Unauthorized("Invalid email or password".into()));
        };
        if !verify_password(&req.password, &user.password_hash) {
            self.limiter.record_failure(&rate_key, now);
            return Err(StoreError::Unauthorized("Invalid email or password".into()));
        }
        self.limiter.clear(&rate_key);
        let (session_id, token, expires) =
            Self::issue_session(&mut guard, &user.user_id, &req.device_name, now);
        Self::record_audit(
            &mut guard,
            None,
            &user.user_id,
            &user.email_normalized,
            "auth.login",
            &session_id,
            "Internal account session created",
            now,
        );
        Ok(AuthResponse {
            api_version: API_VERSION,
            user: Self::user_profile(&user),
            session_id,
            session_token: token,
            expires_at_unix: expires,
        })
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
        let uid = firebase_uid.trim();
        if uid.is_empty() || uid.len() > 128 {
            return Err(StoreError::Unauthorized("Invalid Firebase UID".into()));
        }
        let email = normalize_email(email)?;
        let clean_name = if display_name.trim().is_empty() {
            email.split('@').next().unwrap_or("User").to_owned()
        } else {
            display_name.trim().chars().take(80).collect()
        };

        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let existing_id = guard
            .user_id_by_firebase_uid
            .get(uid)
            .cloned()
            .or_else(|| guard.user_id_by_email.get(&email).cloned());

        let (user, action) = if let Some(user_id) = existing_id {
            guard
                .user_id_by_firebase_uid
                .insert(uid.to_owned(), user_id.clone());
            let u = guard
                .users_by_id
                .get(&user_id)
                .cloned()
                .ok_or(StoreError::InternalSafe)?;
            (u, "auth.firebase_login")
        } else {
            // Auto-provision account seamlessly for any Firebase-verified user.
            let _ = bootstrap_token;
            let is_first = guard.users_by_id.is_empty();
            let user_id = Uuid::new_v4().to_string();
            let rec = UserRec {
                user_id: user_id.clone(),
                email_normalized: email.clone(),
                display_name: clean_name,
                password_hash: "!firebase-only!".to_owned(),
                is_server_owner: is_first,
                created_at_unix: now,
            };
            guard
                .user_id_by_email
                .insert(email.clone(), user_id.clone());
            guard
                .user_id_by_firebase_uid
                .insert(uid.to_owned(), user_id.clone());
            guard.users_by_id.insert(user_id, rec.clone());
            (rec, "auth.firebase_register")
        };

        let (session_id, token, expires) =
            Self::issue_session(&mut guard, &user.user_id, device_name, now);
        Self::record_audit(
            &mut guard,
            None,
            &user.user_id,
            &user.email_normalized,
            action,
            &session_id,
            "Firebase verified session created",
            now,
        );
        Ok(AuthResponse {
            api_version: API_VERSION,
            user: Self::user_profile(&user),
            session_id,
            session_token: token,
            expires_at_unix: expires,
        })
    }

    async fn authenticate_bearer(
        &self,
        bearer_token: &str,
        now: i64,
    ) -> Result<AuthContext, StoreError> {
        let token = bearer_token.trim();
        if token.is_empty() {
            return Err(StoreError::Unauthorized(
                "Missing bearer session token".into(),
            ));
        }
        let hash = sha256_hex(token);
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let sid = guard
            .session_id_by_hash
            .get(&hash)
            .cloned()
            .ok_or_else(|| StoreError::Unauthorized("Invalid or unknown session".into()))?;
        let sess = guard
            .sessions_by_id
            .get_mut(&sid)
            .ok_or_else(|| StoreError::Unauthorized("Invalid session".into()))?;
        if sess.revoked_at_unix.is_some() {
            return Err(StoreError::Unauthorized("Session has been revoked".into()));
        }
        if sess.expires_at_unix <= now {
            return Err(StoreError::Unauthorized("Session has expired".into()));
        }
        sess.last_seen_unix = now;
        let user_id = sess.user_id.clone();
        let user = guard
            .users_by_id
            .get(&user_id)
            .ok_or_else(|| StoreError::Unauthorized("Account not found".into()))?;
        Ok(AuthContext {
            user_id: user.user_id.clone(),
            email: user.email_normalized.clone(),
            display_name: user.display_name.clone(),
            is_server_owner: user.is_server_owner,
            session_id: sid,
        })
    }

    async fn list_sessions(
        &self,
        auth: &AuthContext,
        now: i64,
    ) -> Result<Vec<SessionSummary>, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let mut list: Vec<SessionSummary> = guard
            .sessions_by_id
            .values()
            .filter(|s| s.user_id == auth.user_id && s.expires_at_unix > now)
            .map(|s| SessionSummary {
                session_id: s.session_id.clone(),
                device_name: s.device_name.clone(),
                created_at_unix: s.created_at_unix,
                expires_at_unix: s.expires_at_unix,
                last_seen_unix: s.last_seen_unix,
                revoked_at_unix: s.revoked_at_unix,
                is_current: s.session_id == auth.session_id,
            })
            .collect();
        list.sort_by_key(|a| std::cmp::Reverse(a.created_at_unix));
        Ok(list)
    }

    async fn revoke_session(
        &self,
        auth: &AuthContext,
        session_id: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let sess = guard
            .sessions_by_id
            .get_mut(session_id)
            .ok_or_else(|| StoreError::NotFound("Session not found".into()))?;
        if sess.user_id != auth.user_id && !auth.is_server_owner {
            return Err(StoreError::Forbidden(
                "Cannot revoke another user's session".into(),
            ));
        }
        sess.revoked_at_unix = Some(now);
        Self::record_audit(
            &mut guard,
            None,
            &auth.user_id,
            &auth.email,
            "auth.session_revoked",
            session_id,
            "Device session revoked",
            now,
        );
        Ok(())
    }

    async fn logout_all(&self, auth: &AuthContext, now: i64) -> Result<usize, StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let mut count = 0;
        for s in guard.sessions_by_id.values_mut() {
            if s.user_id == auth.user_id && s.revoked_at_unix.is_none() {
                s.revoked_at_unix = Some(now);
                count += 1;
            }
        }
        Self::record_audit(
            &mut guard,
            None,
            &auth.user_id,
            &auth.email,
            "auth.logout_all",
            &auth.user_id,
            "All user sessions revoked",
            now,
        );
        Ok(count)
    }

    async fn change_password(
        &self,
        auth: &AuthContext,
        req: ChangePasswordRequest,
        now: i64,
    ) -> Result<(), StoreError> {
        validate_password(&req.new_password)?;
        let new_hash = hash_password(&req.new_password)?;
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let user = guard
            .users_by_id
            .get_mut(&auth.user_id)
            .ok_or_else(|| StoreError::NotFound("Account not found".into()))?;
        if !verify_password(&req.current_password, &user.password_hash) {
            return Err(StoreError::Unauthorized(
                "Current password is incorrect".into(),
            ));
        }
        user.password_hash = new_hash;
        if req.revoke_other_sessions {
            for s in guard.sessions_by_id.values_mut() {
                if s.user_id == auth.user_id
                    && s.session_id != auth.session_id
                    && s.revoked_at_unix.is_none()
                {
                    s.revoked_at_unix = Some(now);
                }
            }
        }
        Self::record_audit(
            &mut guard,
            None,
            &auth.user_id,
            &auth.email,
            "auth.password_changed",
            &auth.user_id,
            "Password changed",
            now,
        );
        Ok(())
    }

    async fn create_workspace(
        &self,
        auth: &AuthContext,
        req: CreateWorkspaceRequest,
        now: i64,
    ) -> Result<TeamWorkspaceSummary, StoreError> {
        let slug = normalize_slug(&req.slug)?;
        let name = req.name.trim();
        if name.is_empty() || name.len() > 80 {
            return Err(StoreError::InvalidInput(
                "Workspace name must be 1-80 characters".into(),
            ));
        }
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        if guard.workspace_id_by_slug.contains_key(&slug) {
            return Err(StoreError::Conflict("Workspace slug already exists".into()));
        }
        let workspace_id = Uuid::new_v4().to_string();
        let ws = WorkspaceRec {
            workspace_id: workspace_id.clone(),
            slug: slug.clone(),
            name: name.to_owned(),
            description: req.description.trim().chars().take(500).collect(),
            current_revision: 1,
            membership_revision: 1,
            archived: false,
        };
        guard
            .workspace_id_by_slug
            .insert(slug, workspace_id.clone());
        guard
            .workspaces_by_id
            .insert(workspace_id.clone(), ws.clone());
        guard.memberships.insert(
            (workspace_id.clone(), auth.user_id.clone()),
            MembershipRec {
                role: Role::Owner,
                joined_at_unix: now,
            },
        );
        Self::record_audit(
            &mut guard,
            Some(&workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.created",
            &workspace_id,
            &format!("Workspace '{}' created", ws.name),
            now,
        );
        Ok(Self::workspace_summary(&guard, &ws, Role::Owner))
    }

    async fn list_workspaces(
        &self,
        auth: &AuthContext,
    ) -> Result<Vec<TeamWorkspaceSummary>, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let mut out = Vec::new();
        for ((wid, uid), mem) in &guard.memberships {
            if uid == &auth.user_id
                && let Some(ws) = guard.workspaces_by_id.get(wid)
            {
                out.push(Self::workspace_summary(&guard, ws, mem.role));
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    async fn list_members_and_invitations(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<MemberListResponse, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (ws, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        let mut members = Vec::new();
        for ((wid, uid), mem) in &guard.memberships {
            if wid == workspace_id
                && let Some(user) = guard.users_by_id.get(uid)
            {
                members.push(MemberSummary {
                    user_id: user.user_id.clone(),
                    email: user.email_normalized.clone(),
                    display_name: user.display_name.clone(),
                    role: mem.role,
                    joined_at_unix: mem.joined_at_unix,
                    membership_revision: ws.membership_revision,
                });
            }
        }
        let invitations = if caller_mem.role.can_manage_members() {
            guard
                .invitations_by_id
                .values()
                .filter(|inv| inv.workspace_id == workspace_id)
                .map(|inv| InvitationSummary {
                    invitation_id: inv.invitation_id.clone(),
                    workspace_id: inv.workspace_id.clone(),
                    workspace_name: ws.name.clone(),
                    email: inv.email_normalized.clone(),
                    role: inv.role,
                    created_at_unix: inv.created_at_unix,
                    expires_at_unix: inv.expires_at_unix,
                    used_at_unix: inv.used_at_unix,
                    revoked_at_unix: inv.revoked_at_unix,
                })
                .collect()
        } else {
            Vec::new()
        };
        Ok(MemberListResponse {
            api_version: API_VERSION,
            workspace_id: ws.workspace_id.clone(),
            membership_revision: ws.membership_revision,
            members,
            invitations,
        })
    }

    async fn create_invitation(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: CreateInvitationRequest,
        now: i64,
    ) -> Result<CreateInvitationResponse, StoreError> {
        if req.role == Role::Owner {
            return Err(StoreError::InvalidInput(
                "Invitations cannot grant Owner role directly; use ownership transfer".into(),
            ));
        }
        let email = normalize_email(&req.email)?;
        let hours = req.expires_in_hours.clamp(1, 168) as i64;
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (ws, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if !caller_mem.role.can_manage_members() {
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can create workspace invitations".into(),
            ));
        }
        let ws_name = ws.name.clone();
        let invitation_id = Uuid::new_v4().to_string();
        let token = generate_opaque_token("inv");
        let token_hash = sha256_hex(&token);
        let expires = now + hours * 3600;
        let rec = InvitationRec {
            invitation_id: invitation_id.clone(),
            workspace_id: workspace_id.to_owned(),
            email_normalized: email.clone(),
            role: req.role,
            created_at_unix: now,
            expires_at_unix: expires,
            used_at_unix: None,
            revoked_at_unix: None,
        };
        guard
            .invitation_id_by_hash
            .insert(token_hash, invitation_id.clone());
        guard
            .invitations_by_id
            .insert(invitation_id.clone(), rec.clone());
        Self::record_audit(
            &mut guard,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.invitation_created",
            &invitation_id,
            &format!("Invited {email} as {}", req.role.as_str()),
            now,
        );
        Ok(CreateInvitationResponse {
            api_version: API_VERSION,
            invitation: InvitationSummary {
                invitation_id,
                workspace_id: workspace_id.to_owned(),
                workspace_name: ws_name,
                email,
                role: req.role,
                created_at_unix: now,
                expires_at_unix: expires,
                used_at_unix: None,
                revoked_at_unix: None,
            },
            invite_token: token,
        })
    }

    async fn revoke_invitation(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        invitation_id: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if !caller_mem.role.can_manage_members() {
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can revoke invitations".into(),
            ));
        }
        let inv = guard
            .invitations_by_id
            .get_mut(invitation_id)
            .ok_or_else(|| StoreError::NotFound("Invitation not found".into()))?;
        if inv.workspace_id != workspace_id {
            return Err(StoreError::Forbidden(
                "Invitation belongs to another workspace".into(),
            ));
        }
        if inv.used_at_unix.is_some() {
            return Err(StoreError::InvitationAlreadyUsed);
        }
        inv.revoked_at_unix = Some(now);
        Self::record_audit(
            &mut guard,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.invitation_revoked",
            invitation_id,
            "Invitation revoked",
            now,
        );
        Ok(())
    }

    async fn accept_invitation(
        &self,
        caller: Option<&AuthContext>,
        req: AcceptInvitationRequest,
        now: i64,
    ) -> Result<AcceptInvitationResponse, StoreError> {
        let token = req.invite_token.trim();
        if token.is_empty() {
            return Err(StoreError::InvalidInput(
                "Invitation token is required".into(),
            ));
        }
        let token_hash = sha256_hex(token);
        let supplied_email = normalize_email(&req.email)?;
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let inv_id = guard
            .invitation_id_by_hash
            .get(&token_hash)
            .cloned()
            .ok_or_else(|| StoreError::NotFound("Invitation token not recognized".into()))?;
        let inv = guard
            .invitations_by_id
            .get(&inv_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound("Invitation not found".into()))?;
        if inv.revoked_at_unix.is_some() {
            return Err(StoreError::InvitationRevoked);
        }
        if inv.used_at_unix.is_some() {
            return Err(StoreError::InvitationAlreadyUsed);
        }
        if inv.expires_at_unix <= now {
            return Err(StoreError::InvitationExpired);
        }
        if supplied_email != inv.email_normalized {
            return Err(StoreError::InvitationAccountMismatch);
        }
        if let Some(auth) = caller
            && auth.email != inv.email_normalized
        {
            return Err(StoreError::InvitationAccountMismatch);
        }

        let user = if let Some(existing_uid) = guard.user_id_by_email.get(&supplied_email).cloned()
        {
            let u = guard
                .users_by_id
                .get(&existing_uid)
                .cloned()
                .ok_or(StoreError::InternalSafe)?;
            if caller.is_none() {
                let pw = req.password.as_deref().unwrap_or("");
                if !verify_password(pw, &u.password_hash) {
                    return Err(StoreError::Unauthorized(
                        "Existing account password required to accept invitation".into(),
                    ));
                }
            }
            u
        } else {
            let display_name = req.display_name.trim();
            if display_name.is_empty() || display_name.len() > 80 {
                return Err(StoreError::InvalidInput("Display name is required".into()));
            }
            let pw = req.password.as_deref().ok_or_else(|| {
                StoreError::InvalidInput("Password is required for new account".into())
            })?;
            let pw_hash = hash_password(pw)?;
            let uid = Uuid::new_v4().to_string();
            let u = UserRec {
                user_id: uid.clone(),
                email_normalized: supplied_email.clone(),
                display_name: display_name.to_owned(),
                password_hash: pw_hash,
                is_server_owner: false,
                created_at_unix: now,
            };
            guard
                .user_id_by_email
                .insert(supplied_email.clone(), uid.clone());
            guard.users_by_id.insert(uid, u.clone());
            u
        };

        if let Some(inv_mut) = guard.invitations_by_id.get_mut(&inv_id) {
            inv_mut.used_at_unix = Some(now);
        }
        guard.memberships.insert(
            (inv.workspace_id.clone(), user.user_id.clone()),
            MembershipRec {
                role: inv.role,
                joined_at_unix: now,
            },
        );
        let ws = guard
            .workspaces_by_id
            .get_mut(&inv.workspace_id)
            .ok_or_else(|| StoreError::NotFound("Workspace not found".into()))?;
        ws.membership_revision += 1;
        let ws_clone = ws.clone();
        let (session_id, session_token, expires) =
            Self::issue_session(&mut guard, &user.user_id, &req.device_name, now);
        Self::record_audit(
            &mut guard,
            Some(&inv.workspace_id),
            &user.user_id,
            &user.email_normalized,
            "workspace.invitation_accepted",
            &inv_id,
            &format!("Joined workspace as {}", inv.role.as_str()),
            now,
        );
        let workspace = Self::workspace_summary(&guard, &ws_clone, inv.role);
        Ok(AcceptInvitationResponse {
            api_version: API_VERSION,
            workspace,
            user: Self::user_profile(&user),
            session_id,
            session_token,
            expires_at_unix: expires,
        })
    }

    async fn list_my_pending_invitations(
        &self,
        auth: &AuthContext,
        now: i64,
    ) -> Result<Vec<InvitationSummary>, StoreError> {
        let target_email = normalize_email(&auth.email)?;
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let mut out: Vec<InvitationSummary> = guard
            .invitations_by_id
            .values()
            .filter(|inv| {
                inv.email_normalized == target_email
                    && inv.used_at_unix.is_none()
                    && inv.revoked_at_unix.is_none()
                    && inv.expires_at_unix > now
            })
            .filter_map(|inv| {
                let ws = guard.workspaces_by_id.get(&inv.workspace_id)?;
                Some(InvitationSummary {
                    invitation_id: inv.invitation_id.clone(),
                    workspace_id: inv.workspace_id.clone(),
                    workspace_name: ws.name.clone(),
                    email: inv.email_normalized.clone(),
                    role: inv.role,
                    created_at_unix: inv.created_at_unix,
                    expires_at_unix: inv.expires_at_unix,
                    used_at_unix: inv.used_at_unix,
                    revoked_at_unix: inv.revoked_at_unix,
                })
            })
            .collect();
        out.sort_by_key(|a| std::cmp::Reverse(a.created_at_unix));
        Ok(out)
    }

    async fn accept_invitation_by_id(
        &self,
        auth: &AuthContext,
        invitation_id: &str,
        _device_name: &str,
        now: i64,
    ) -> Result<AcceptInvitationResponse, StoreError> {
        let caller_email = normalize_email(&auth.email)?;
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let inv = guard
            .invitations_by_id
            .get(invitation_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound("Invitation not found".into()))?;
        if inv.revoked_at_unix.is_some() {
            return Err(StoreError::InvitationRevoked);
        }
        if inv.used_at_unix.is_some() {
            return Err(StoreError::InvitationAlreadyUsed);
        }
        if inv.expires_at_unix <= now {
            return Err(StoreError::InvitationExpired);
        }
        if inv.email_normalized != caller_email {
            return Err(StoreError::InvitationAccountMismatch);
        }
        let user = guard
            .users_by_id
            .get(&auth.user_id)
            .cloned()
            .ok_or(StoreError::Unauthorized("Account not found".into()))?;
        if let Some(inv_mut) = guard.invitations_by_id.get_mut(invitation_id) {
            inv_mut.used_at_unix = Some(now);
        }
        guard
            .memberships
            .entry((inv.workspace_id.clone(), user.user_id.clone()))
            .or_insert(MembershipRec {
                role: inv.role,
                joined_at_unix: now,
            });
        let ws = guard
            .workspaces_by_id
            .get_mut(&inv.workspace_id)
            .ok_or_else(|| StoreError::NotFound("Workspace not found".into()))?;
        ws.membership_revision += 1;
        let ws_clone = ws.clone();
        Self::record_audit(
            &mut guard,
            Some(&inv.workspace_id),
            &user.user_id,
            &user.email_normalized,
            "workspace.invitation_accepted",
            invitation_id,
            &format!("Accepted in-app invitation as {}", inv.role.as_str()),
            now,
        );
        let workspace = Self::workspace_summary(&guard, &ws_clone, inv.role);
        Ok(AcceptInvitationResponse {
            api_version: API_VERSION,
            workspace,
            user: Self::user_profile(&user),
            session_id: auth.session_id.clone(),
            session_token: String::new(),
            expires_at_unix: now + SESSION_TTL_SECS,
        })
    }

    async fn decline_invitation(
        &self,
        auth: &AuthContext,
        invitation_id: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        let caller_email = normalize_email(&auth.email)?;
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let inv = guard
            .invitations_by_id
            .get_mut(invitation_id)
            .ok_or_else(|| StoreError::NotFound("Invitation not found".into()))?;
        if inv.email_normalized != caller_email {
            return Err(StoreError::InvitationAccountMismatch);
        }
        if inv.used_at_unix.is_some() {
            return Err(StoreError::InvitationAlreadyUsed);
        }
        let ws_id = inv.workspace_id.clone();
        inv.revoked_at_unix = Some(now);
        Self::record_audit(
            &mut guard,
            Some(&ws_id),
            &auth.user_id,
            &auth.email,
            "workspace.invitation_declined",
            invitation_id,
            "Invited user declined in-app",
            now,
        );
        Ok(())
    }

    async fn create_public_invite_link(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: CreatePublicInviteLinkRequest,
        now: i64,
    ) -> Result<CreatePublicInviteLinkResponse, StoreError> {
        if req.role == Role::Owner {
            return Err(StoreError::InvalidInput(
                "Invite links cannot grant Owner role directly".into(),
            ));
        }
        let hours = req.expires_in_hours.clamp(1, 720) as i64;
        let max_uses = req.max_uses.filter(|&m| m > 0);
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (ws, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if !caller_mem.role.can_manage_members() {
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can create shareable invite links".into(),
            ));
        }
        let ws_name = ws.name.clone();
        let link_id = Uuid::new_v4().to_string();
        let invite_code = generate_opaque_token("lnk");
        let code_hash = sha256_hex(&invite_code);
        let share_link = format!("swaggen://join/{workspace_id}/{invite_code}");
        let link = PublicInviteLinkSummary {
            link_id: link_id.clone(),
            workspace_id: workspace_id.to_owned(),
            workspace_name: ws_name,
            role: req.role,
            invite_code,
            share_link,
            created_at_unix: now,
            expires_at_unix: now + hours * 3600,
            max_uses,
            uses_count: 0,
            revoked_at_unix: None,
        };
        guard
            .public_link_id_by_hash
            .insert(code_hash, link_id.clone());
        guard
            .public_links_by_id
            .insert(link_id.clone(), link.clone());
        Self::record_audit(
            &mut guard,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.public_link_created",
            &link_id,
            &format!(
                "Created shareable {} invite link (expires in {hours}h)",
                req.role.as_str()
            ),
            now,
        );
        Ok(CreatePublicInviteLinkResponse {
            api_version: API_VERSION,
            link,
        })
    }

    async fn list_public_invite_links(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<Vec<PublicInviteLinkSummary>, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if !caller_mem.role.can_manage_members() {
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can view shareable invite links".into(),
            ));
        }
        let mut out: Vec<PublicInviteLinkSummary> = guard
            .public_links_by_id
            .values()
            .filter(|l| l.workspace_id == workspace_id)
            .cloned()
            .collect();
        out.sort_by_key(|a| std::cmp::Reverse(a.created_at_unix));
        Ok(out)
    }

    async fn revoke_public_invite_link(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        link_id: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if !caller_mem.role.can_manage_members() {
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can revoke invite links".into(),
            ));
        }
        let l = guard
            .public_links_by_id
            .get_mut(link_id)
            .ok_or_else(|| StoreError::NotFound("Invite link not found".into()))?;
        if l.workspace_id != workspace_id {
            return Err(StoreError::Forbidden(
                "Invite link belongs to another workspace".into(),
            ));
        }
        l.revoked_at_unix = Some(now);
        Self::record_audit(
            &mut guard,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.public_link_revoked",
            link_id,
            "Shareable invite link revoked",
            now,
        );
        Ok(())
    }

    async fn join_by_public_invite_link(
        &self,
        auth: &AuthContext,
        link_or_code: &str,
        _device_name: &str,
        now: i64,
    ) -> Result<AcceptInvitationResponse, StoreError> {
        let trimmed = link_or_code.trim();
        let code = trimmed
            .rsplit('/')
            .next()
            .unwrap_or(trimmed)
            .split('?')
            .next()
            .unwrap_or(trimmed)
            .trim();
        if code.is_empty() {
            return Err(StoreError::InvalidInput(
                "Invite link or code is required".into(),
            ));
        }
        let code_hash = sha256_hex(code);
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let link_id = guard
            .public_link_id_by_hash
            .get(&code_hash)
            .cloned()
            .ok_or_else(|| StoreError::NotFound("Invite link not recognized".into()))?;
        let link = guard
            .public_links_by_id
            .get(&link_id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound("Invite link not found".into()))?;
        if link.revoked_at_unix.is_some() {
            return Err(StoreError::InvitationRevoked);
        }
        if link.expires_at_unix <= now {
            return Err(StoreError::InvitationExpired);
        }
        if let Some(max_u) = link.max_uses
            && link.uses_count >= max_u
        {
            return Err(StoreError::InvitationAlreadyUsed);
        }
        let user = guard
            .users_by_id
            .get(&auth.user_id)
            .cloned()
            .ok_or(StoreError::Unauthorized("Account not found".into()))?;
        if let Some(lm) = guard.public_links_by_id.get_mut(&link_id) {
            lm.uses_count += 1;
        }
        guard
            .memberships
            .entry((link.workspace_id.clone(), user.user_id.clone()))
            .or_insert(MembershipRec {
                role: link.role,
                joined_at_unix: now,
            });
        let ws = guard
            .workspaces_by_id
            .get_mut(&link.workspace_id)
            .ok_or_else(|| StoreError::NotFound("Workspace not found".into()))?;
        ws.membership_revision += 1;
        let ws_clone = ws.clone();
        Self::record_audit(
            &mut guard,
            Some(&link.workspace_id),
            &user.user_id,
            &user.email_normalized,
            "workspace.public_link_joined",
            &link_id,
            &format!("Joined via public invite link as {}", link.role.as_str()),
            now,
        );
        let workspace = Self::workspace_summary(&guard, &ws_clone, link.role);
        Ok(AcceptInvitationResponse {
            api_version: API_VERSION,
            workspace,
            user: Self::user_profile(&user),
            session_id: auth.session_id.clone(),
            session_token: String::new(),
            expires_at_unix: now + SESSION_TTL_SECS,
        })
    }

    async fn update_member_role(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        target_user_id: &str,
        req: UpdateMemberRoleRequest,
        now: i64,
    ) -> Result<MemberSummary, StoreError> {
        if req.role == Role::Owner {
            return Err(StoreError::OwnerRequired(
                "Cannot assign Owner directly; use ownership transfer".into(),
            ));
        }
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (ws, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if !caller_mem.role.can_manage_members() {
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can change member roles".into(),
            ));
        }
        if ws.membership_revision != req.expected_membership_revision {
            return Err(StoreError::Conflict(
                "Membership revision is stale; reload workspace members first".into(),
            ));
        }
        let target_mem = guard
            .memberships
            .get(&(workspace_id.to_owned(), target_user_id.to_owned()))
            .cloned()
            .ok_or_else(|| StoreError::NotFound("Target member not in workspace".into()))?;
        if target_mem.role == Role::Owner {
            return Err(StoreError::OwnerRequired(
                "Workspace Owner cannot be demoted without transferring ownership first".into(),
            ));
        }
        if let Some(m) = guard
            .memberships
            .get_mut(&(workspace_id.to_owned(), target_user_id.to_owned()))
        {
            m.role = req.role;
        }
        let ws_mut = guard
            .workspaces_by_id
            .get_mut(workspace_id)
            .ok_or_else(|| StoreError::NotFound("Workspace not found".into()))?;
        ws_mut.membership_revision += 1;
        let mem_rev = ws_mut.membership_revision;
        let user = guard
            .users_by_id
            .get(target_user_id)
            .cloned()
            .ok_or(StoreError::InternalSafe)?;
        Self::record_audit(
            &mut guard,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.role_updated",
            target_user_id,
            &format!("Role updated to {}", req.role.as_str()),
            now,
        );
        Ok(MemberSummary {
            user_id: user.user_id,
            email: user.email_normalized,
            display_name: user.display_name,
            role: req.role,
            joined_at_unix: target_mem.joined_at_unix,
            membership_revision: mem_rev,
        })
    }

    async fn list_entities(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<Vec<WorkspaceEntityRecord>, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, _) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        Ok(guard
            .entities
            .iter()
            .filter(|((wid, _), _)| wid == workspace_id)
            .map(|(_, rec)| rec.clone())
            .collect())
    }

    async fn upsert_entity(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: UpsertEntityRequest,
        now: i64,
    ) -> Result<WorkspaceEntityRecord, StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if !caller_mem.role.can_write_entities() {
            return Err(StoreError::Forbidden(
                "Viewer role has read-only access in this workspace".into(),
            ));
        }
        let key = (workspace_id.to_owned(), req.entity_id.clone());
        let next_rev = match guard.entities.get(&key) {
            Some(existing) => {
                if existing.revision != req.base_revision {
                    return Err(StoreError::Conflict(
                        "Entity revision mismatch; refresh before retrying".into(),
                    ));
                }
                existing.revision + 1
            }
            None => {
                if req.base_revision != 0 {
                    return Err(StoreError::Conflict(
                        "New entity write must start from base_revision 0".into(),
                    ));
                }
                1
            }
        };
        let record = WorkspaceEntityRecord {
            entity_id: req.entity_id,
            workspace_id: workspace_id.to_owned(),
            parent_id: req.parent_id,
            kind: req.kind,
            name: req.name,
            order_key: req.order_key,
            payload_json: req.payload_json,
            revision: next_rev,
            deleted: false,
            updated_at_unix: now,
            updated_by: auth.user_id.clone(),
        };
        guard.entities.insert(key, record.clone());
        Self::record_audit(
            &mut guard,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.entity_upserted",
            &record.entity_id,
            &format!("{} rev {}", record.name, record.revision),
            now,
        );
        Ok(record)
    }

    async fn list_audit_events(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        limit: usize,
    ) -> Result<Vec<AuditEventRecord>, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if !caller_mem.role.can_read_audit() {
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can view workspace audit logs".into(),
            ));
        }
        let mut events: Vec<AuditEventRecord> = guard
            .audit_events
            .iter()
            .filter(|e| e.workspace_id.as_deref() == Some(workspace_id))
            .cloned()
            .collect();
        events.reverse();
        events.truncate(limit.clamp(1, 200));
        Ok(events)
    }

    async fn publish_batch(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: PublishBatchRequest,
        now: i64,
    ) -> Result<PublishBatchResponse, StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (ws, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if !caller_mem.role.can_write_entities() {
            return Err(StoreError::Forbidden(
                "Viewer role cannot publish workspace changes".into(),
            ));
        }
        if let Some(exp_rev) = req.expected_membership_revision
            && ws.membership_revision != exp_rev
        {
            return Err(StoreError::Conflict(
                "Workspace membership changed; refresh permissions before publishing".into(),
            ));
        }

        let mut results = Vec::new();
        for op in req.operations {
            let op_key = (
                workspace_id.to_owned(),
                auth.user_id.clone(),
                op.operation_id.clone(),
            );
            if let Some(prev) = guard.applied_ops.get(&op_key) {
                if prev.request_hash == op.request_hash {
                    let mut ack = prev.result.clone();
                    ack.kind = PublishResultKind::AlreadyAcked;
                    results.push(ack);
                    continue;
                } else {
                    results.push(PublishOperationResult {
                        operation_id: op.operation_id,
                        entity_id: op.entity_id,
                        kind: PublishResultKind::HashMismatch,
                        server_revision: 0,
                        server_cursor: None,
                        server_entity: None,
                        reason: "Operation ID already committed with a different payload hash"
                            .into(),
                    });
                    continue;
                }
            }

            let ent_key = (workspace_id.to_owned(), op.entity_id.clone());
            let existing = guard.entities.get(&ent_key).cloned();
            let next_rev = match existing {
                Some(ref cur) if cur.deleted => {
                    results.push(PublishOperationResult {
                        operation_id: op.operation_id,
                        entity_id: op.entity_id,
                        kind: PublishResultKind::ConflictDeletedRemote,
                        server_revision: cur.revision,
                        server_cursor: None,
                        server_entity: Some(cur.clone()),
                        reason:
                            "Target entity was deleted on server; tombstone prevents resurrection"
                                .into(),
                    });
                    continue;
                }
                Some(ref cur) if cur.revision != op.base_revision => {
                    results.push(PublishOperationResult {
                        operation_id: op.operation_id,
                        entity_id: op.entity_id,
                        kind: PublishResultKind::ConflictRevision,
                        server_revision: cur.revision,
                        server_cursor: None,
                        server_entity: Some(cur.clone()),
                        reason: format!(
                            "Base revision {} is stale; server revision is {}",
                            op.base_revision, cur.revision
                        ),
                    });
                    continue;
                }
                Some(cur) => cur.revision + 1,
                None => {
                    if op.base_revision != 0 {
                        results.push(PublishOperationResult {
                            operation_id: op.operation_id,
                            entity_id: op.entity_id,
                            kind: PublishResultKind::ConflictRevision,
                            server_revision: 0,
                            server_cursor: None,
                            server_entity: None,
                            reason: "Entity does not exist at expected base revision".into(),
                        });
                        continue;
                    }
                    1
                }
            };

            let record = WorkspaceEntityRecord {
                entity_id: op.entity_id.clone(),
                workspace_id: workspace_id.to_owned(),
                parent_id: op.parent_id.clone(),
                kind: op.kind.clone(),
                name: op.name.clone(),
                order_key: op.order_key,
                payload_json: op.payload_json.clone(),
                revision: next_rev,
                deleted: op.delete,
                updated_at_unix: now,
                updated_by: auth.user_id.clone(),
            };
            guard.entities.insert(ent_key, record.clone());
            guard.revisions.insert(
                (workspace_id.to_owned(), op.entity_id.clone(), next_rev),
                EntityRevisionRecord {
                    workspace_id: workspace_id.to_owned(),
                    entity_id: op.entity_id.clone(),
                    revision: next_rev,
                    deleted: op.delete,
                    name: op.name.clone(),
                    payload_json: op.payload_json.clone(),
                    actor_user_id: auth.user_id.clone(),
                    actor_email: auth.email.clone(),
                    note: op.note.clone(),
                    created_at_unix: now,
                },
            );
            let cursor = {
                let c = guard.cursors.entry(workspace_id.to_owned()).or_insert(0);
                *c += 1;
                *c
            };
            if let Some(ws_mut) = guard.workspaces_by_id.get_mut(workspace_id) {
                ws_mut.current_revision += 1;
            }
            guard.change_events.insert(
                (workspace_id.to_owned(), cursor),
                ChangeEventRecord {
                    cursor,
                    workspace_id: workspace_id.to_owned(),
                    entity_id: op.entity_id.clone(),
                    entity_kind: op.kind.clone(),
                    entity_name: op.name.clone(),
                    parent_id: op.parent_id.clone(),
                    order_key: op.order_key,
                    revision: next_rev,
                    deleted: op.delete,
                    payload_json: if op.delete {
                        None
                    } else {
                        Some(op.payload_json.clone())
                    },
                    operation_id: op.operation_id.clone(),
                    actor_user_id: auth.user_id.clone(),
                    actor_email: auth.email.clone(),
                    note: op.note.clone(),
                    created_at_unix: now,
                },
            );
            let recipients: Vec<String> = guard
                .memberships
                .keys()
                .filter(|(wid, uid)| wid == workspace_id && uid != &auth.user_id)
                .map(|(_, uid)| uid.clone())
                .collect();
            for uid in recipients {
                guard.notifications.push(NotificationRecord {
                    notification_id: Uuid::new_v4().to_string(),
                    workspace_id: workspace_id.to_owned(),
                    user_id: uid,
                    kind: if op.delete {
                        "entity_deleted"
                    } else {
                        "entity_published"
                    }
                    .into(),
                    title: format!(
                        "{} {} ({})",
                        auth.email,
                        if op.delete { "deleted" } else { "published" },
                        op.name
                    ),
                    entity_id: Some(op.entity_id.clone()),
                    actor_email: auth.email.clone(),
                    created_at_unix: now,
                    read: false,
                });
            }
            Self::record_audit(
                &mut guard,
                Some(workspace_id),
                &auth.user_id,
                &auth.email,
                if op.delete {
                    "workspace.entity_deleted"
                } else {
                    "workspace.published"
                },
                &op.entity_id,
                &format!("{} rev {} ({})", op.name, next_rev, op.note),
                now,
            );

            let res = PublishOperationResult {
                operation_id: op.operation_id,
                entity_id: op.entity_id,
                kind: PublishResultKind::Applied,
                server_revision: next_rev,
                server_cursor: Some(cursor),
                server_entity: Some(record),
                reason: "Published".into(),
            };
            guard.applied_ops.insert(
                op_key,
                AppliedOpRec {
                    request_hash: op.request_hash,
                    result: res.clone(),
                },
            );
            results.push(res);
        }

        Ok(PublishBatchResponse {
            api_version: API_VERSION,
            results,
        })
    }

    async fn get_changes(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        after_cursor: i64,
        limit: usize,
    ) -> Result<ChangesFeedResponse, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (ws, _) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        let min_retained = guard
            .min_retained_cursor
            .get(workspace_id)
            .copied()
            .unwrap_or(0);
        if after_cursor < min_retained {
            return Err(StoreError::CursorExpired(min_retained));
        }
        let max_items = limit.clamp(1, 200);
        let all: Vec<ChangeEventRecord> = guard
            .change_events
            .range((workspace_id.to_owned(), after_cursor + 1)..)
            .take_while(|((wid, _), _)| wid == workspace_id)
            .map(|(_, ev)| ev.clone())
            .collect();
        let has_more = all.len() > max_items;
        let events: Vec<ChangeEventRecord> = all.into_iter().take(max_items).collect();
        let next_cursor = events.last().map(|e| e.cursor).unwrap_or(after_cursor);
        Ok(ChangesFeedResponse {
            api_version: API_VERSION,
            workspace_id: workspace_id.to_owned(),
            after_cursor,
            next_cursor,
            has_more,
            membership_revision: ws.membership_revision,
            events,
        })
    }

    async fn get_snapshot(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<SnapshotResponse, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (ws, _) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        let snapshot_cursor = guard.cursors.get(workspace_id).copied().unwrap_or(0);
        let all: Vec<WorkspaceEntityRecord> = guard
            .entities
            .iter()
            .filter(|((wid, _), _)| wid == workspace_id)
            .map(|(_, r)| r.clone())
            .collect();
        let page_limit = limit.clamp(1, 500);
        let end = (offset + page_limit).min(all.len());
        let has_more = end < all.len();
        let entities = if offset < all.len() {
            all[offset..end].to_vec()
        } else {
            Vec::new()
        };
        Ok(SnapshotResponse {
            api_version: API_VERSION,
            workspace_id: workspace_id.to_owned(),
            snapshot_cursor,
            snapshot_token: format!("snap-{workspace_id}-{snapshot_cursor}"),
            membership_revision: ws.membership_revision,
            entities,
            next_offset: has_more.then_some(end),
            has_more,
        })
    }

    async fn list_entity_revisions(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        entity_id: &str,
    ) -> Result<Vec<EntityRevisionRecord>, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, _) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        let mut out: Vec<EntityRevisionRecord> = guard
            .revisions
            .range((workspace_id.to_owned(), entity_id.to_owned(), 0)..)
            .take_while(|((wid, eid, _), _)| wid == workspace_id && eid == entity_id)
            .map(|(_, rev)| rev.clone())
            .collect();
        out.reverse();
        Ok(out)
    }

    async fn restore_entity_revision(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        entity_id: &str,
        req: RestoreRevisionRequest,
        now: i64,
    ) -> Result<PublishBatchResponse, StoreError> {
        let target = {
            let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
            let (_, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
            if !caller_mem.role.can_write_entities() {
                return Err(StoreError::Forbidden(
                    "Viewer role cannot restore revisions".into(),
                ));
            }
            guard
                .revisions
                .get(&(
                    workspace_id.to_owned(),
                    entity_id.to_owned(),
                    req.target_revision,
                ))
                .cloned()
                .ok_or_else(|| StoreError::NotFound("Target revision not found".into()))?
        };
        let (parent_id, kind, order_key) = {
            let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
            guard
                .entities
                .get(&(workspace_id.to_owned(), entity_id.to_owned()))
                .map(|e| (e.parent_id.clone(), e.kind.clone(), e.order_key))
                .unwrap_or((None, "request".into(), 0))
        };
        let op = swaggen_api_contract::PublishOperation {
            operation_id: req.operation_id,
            request_hash: format!("restore-{entity_id}-r{}", req.target_revision),
            entity_id: entity_id.to_owned(),
            parent_id,
            kind,
            name: target.name,
            order_key,
            payload_json: target.payload_json,
            base_revision: req.expected_current_revision,
            delete: false,
            note: if req.note.trim().is_empty() {
                format!("Restored revision {}", req.target_revision)
            } else {
                req.note
            },
        };
        self.publish_batch(
            auth,
            workspace_id,
            PublishBatchRequest {
                expected_membership_revision: None,
                operations: vec![op],
            },
            now,
        )
        .await
    }

    async fn create_comment(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: CreateCommentRequest,
        now: i64,
    ) -> Result<CommentThreadRecord, StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if !caller_mem.role.can_write_entities() {
            return Err(StoreError::Forbidden(
                "Viewer role cannot post comments in this workspace".into(),
            ));
        }
        let key = (workspace_id.to_owned(), req.comment_id.clone());
        if let Some(existing) = guard.comments.get(&key) {
            return Ok(existing.clone());
        }
        let thread = CommentThreadRecord {
            comment_id: req.comment_id.clone(),
            workspace_id: workspace_id.to_owned(),
            entity_id: req.entity_id.clone(),
            anchor_revision: req.anchor_revision,
            author_user_id: auth.user_id.clone(),
            author_email: auth.email.clone(),
            body: req.body,
            resolved: false,
            resolved_by_email: None,
            created_at_unix: now,
            replies: Vec::new(),
        };
        guard.comments.insert(key, thread.clone());
        let recipients: Vec<String> = guard
            .memberships
            .keys()
            .filter(|(wid, uid)| wid == workspace_id && uid != &auth.user_id)
            .map(|(_, uid)| uid.clone())
            .collect();
        let mention_targets: std::collections::BTreeSet<String> = thread
            .body
            .split_whitespace()
            .filter_map(|t| t.strip_prefix('@'))
            .map(|s| {
                s.trim_end_matches(&[',', '.', ';', ':', '!', '?', ')', ']'][..])
                    .to_ascii_lowercase()
            })
            .collect();
        for uid in recipients {
            let is_mention = guard
                .users_by_id
                .get(&uid)
                .is_some_and(|u| mention_targets.contains(&u.email_normalized));
            guard.notifications.push(NotificationRecord {
                notification_id: Uuid::new_v4().to_string(),
                workspace_id: workspace_id.to_owned(),
                user_id: uid,
                kind: if is_mention {
                    "mention".into()
                } else {
                    "comment".into()
                },
                title: if is_mention {
                    format!("{} mentioned you on {}", auth.email, req.entity_id)
                } else {
                    format!("{} commented on {}", auth.email, req.entity_id)
                },
                entity_id: Some(req.entity_id.clone()),
                actor_email: auth.email.clone(),
                created_at_unix: now,
                read: false,
            });
        }
        Self::record_audit(
            &mut guard,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.comment_created",
            &req.entity_id,
            &format!("Comment anchored at rev {}", req.anchor_revision),
            now,
        );
        Ok(thread)
    }

    async fn reply_comment(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        comment_id: &str,
        req: ReplyCommentRequest,
        now: i64,
    ) -> Result<CommentThreadRecord, StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if !caller_mem.role.can_write_entities() {
            return Err(StoreError::Forbidden(
                "Viewer role cannot reply to comments".into(),
            ));
        }
        let thread = guard
            .comments
            .get_mut(&(workspace_id.to_owned(), comment_id.to_owned()))
            .ok_or_else(|| StoreError::NotFound("Comment thread not found".into()))?;
        if !thread.replies.iter().any(|r| r.reply_id == req.reply_id) {
            thread.replies.push(CommentReplyRecord {
                reply_id: req.reply_id,
                author_user_id: auth.user_id.clone(),
                author_email: auth.email.clone(),
                body: req.body,
                created_at_unix: now,
            });
        }
        Ok(thread.clone())
    }

    async fn resolve_comment(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        comment_id: &str,
        req: ResolveCommentRequest,
        _now: i64,
    ) -> Result<CommentThreadRecord, StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if !caller_mem.role.can_write_entities() {
            return Err(StoreError::Forbidden(
                "Viewer role cannot resolve comments".into(),
            ));
        }
        let thread = guard
            .comments
            .get_mut(&(workspace_id.to_owned(), comment_id.to_owned()))
            .ok_or_else(|| StoreError::NotFound("Comment thread not found".into()))?;
        thread.resolved = req.resolved;
        thread.resolved_by_email = req.resolved.then(|| auth.email.clone());
        Ok(thread.clone())
    }

    async fn list_comments(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        entity_id: Option<&str>,
    ) -> Result<Vec<CommentThreadRecord>, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, _) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        Ok(guard
            .comments
            .iter()
            .filter(|((wid, _), c)| {
                wid == workspace_id && entity_id.is_none_or(|eid| c.entity_id == eid)
            })
            .map(|(_, c)| c.clone())
            .collect())
    }

    async fn list_notifications(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<NotificationListResponse, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, _) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        let mut list: Vec<NotificationRecord> = guard
            .notifications
            .iter()
            .filter(|n| n.workspace_id == workspace_id && n.user_id == auth.user_id)
            .cloned()
            .collect();
        list.reverse();
        let unread_count = list.iter().filter(|n| !n.read).count();
        Ok(NotificationListResponse {
            api_version: API_VERSION,
            unread_count,
            notifications: list,
        })
    }

    async fn mark_notifications_read(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        ids: &[String],
    ) -> Result<usize, StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, _) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        let mut marked = 0;
        for n in &mut guard.notifications {
            if n.workspace_id == workspace_id
                && n.user_id == auth.user_id
                && !n.read
                && (ids.is_empty() || ids.contains(&n.notification_id))
            {
                n.read = true;
                marked += 1;
            }
        }
        Ok(marked)
    }

    async fn set_workspace_archived(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        archived: bool,
        now: i64,
    ) -> Result<TeamWorkspaceSummary, StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let caller_role = Self::require_membership(&guard, workspace_id, &auth.user_id)?
            .1
            .role;
        if caller_role != Role::Owner {
            return Err(StoreError::Forbidden(
                "Only workspace Owner can archive or restore this workspace".into(),
            ));
        }
        let member_count = guard
            .memberships
            .keys()
            .filter(|(w, _)| w == workspace_id)
            .count();
        let summary = {
            let ws = guard
                .workspaces_by_id
                .get_mut(workspace_id)
                .ok_or_else(|| StoreError::NotFound("Workspace not found".into()))?;
            ws.archived = archived;
            TeamWorkspaceSummary {
                workspace_id: ws.workspace_id.clone(),
                slug: ws.slug.clone(),
                name: ws.name.clone(),
                description: ws.description.clone(),
                role: Role::Owner,
                member_count,
                current_revision: ws.current_revision,
                membership_revision: ws.membership_revision,
                archived: ws.archived,
            }
        };
        Self::record_audit(
            &mut guard,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            if archived {
                "workspace.archived"
            } else {
                "workspace.restored"
            },
            workspace_id,
            "Workspace archive state updated",
            now,
        );
        Ok(summary)
    }

    async fn transfer_workspace_ownership(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: swaggen_api_contract::TransferOwnershipRequest,
        now: i64,
    ) -> Result<MemberListResponse, StoreError> {
        {
            let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
            let (ws_snap, caller_mem) =
                Self::require_membership(&guard, workspace_id, &auth.user_id)?;
            if caller_mem.role != Role::Owner {
                return Err(StoreError::Forbidden(
                    "Only current workspace Owner can transfer ownership".into(),
                ));
            }
            if ws_snap.membership_revision != req.expected_membership_revision {
                return Err(StoreError::Conflict(
                    "Membership revision conflict; refresh members before transferring ownership"
                        .into(),
                ));
            }
            if req.target_user_id == auth.user_id {
                return Err(StoreError::InvalidInput(
                    "Target user is already Owner".into(),
                ));
            }
            if !guard
                .memberships
                .contains_key(&(workspace_id.to_owned(), req.target_user_id.clone()))
            {
                return Err(StoreError::NotFound(
                    "Target member not found in workspace".into(),
                ));
            }
            if let Some(m) = guard
                .memberships
                .get_mut(&(workspace_id.to_owned(), auth.user_id.clone()))
            {
                m.role = Role::Admin;
            }
            if let Some(m) = guard
                .memberships
                .get_mut(&(workspace_id.to_owned(), req.target_user_id.clone()))
            {
                m.role = Role::Owner;
            }
            if let Some(ws) = guard.workspaces_by_id.get_mut(workspace_id) {
                ws.membership_revision += 1;
            }
            Self::record_audit(
                &mut guard,
                Some(workspace_id),
                &auth.user_id,
                &auth.email,
                "workspace.ownership_transferred",
                &req.target_user_id,
                "Transferred workspace ownership atomically",
                now,
            );
        }
        self.list_members_and_invitations(auth, workspace_id).await
    }

    async fn leave_workspace(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let (_, caller_mem) = Self::require_membership(&guard, workspace_id, &auth.user_id)?;
        if caller_mem.role == Role::Owner {
            return Err(StoreError::Conflict(
                "Workspace Owner cannot leave without transferring ownership first".into(),
            ));
        }
        guard
            .memberships
            .remove(&(workspace_id.to_owned(), auth.user_id.clone()));
        if let Some(ws) = guard.workspaces_by_id.get_mut(workspace_id) {
            ws.membership_revision += 1;
        }
        Self::record_audit(
            &mut guard,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.member_left",
            &auth.user_id,
            "Member left workspace",
            now,
        );
        Ok(())
    }

    async fn kv_get(&self, namespace: &str, key: &str) -> Result<Option<String>, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        Ok(guard
            .kv_docs
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    async fn kv_put(
        &self,
        namespace: &str,
        key: &str,
        value_json: &str,
        _now: i64,
    ) -> Result<(), StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        guard.kv_docs.insert(
            (namespace.to_owned(), key.to_owned()),
            value_json.to_owned(),
        );
        Ok(())
    }

    async fn kv_delete(&self, namespace: &str, key: &str) -> Result<bool, StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        Ok(guard
            .kv_docs
            .remove(&(namespace.to_owned(), key.to_owned()))
            .is_some())
    }

    async fn kv_list(
        &self,
        namespace: &str,
        key_prefix: &str,
    ) -> Result<Vec<(String, String)>, StoreError> {
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        Ok(guard
            .kv_docs
            .iter()
            .filter(|((ns, k), _)| ns == namespace && k.starts_with(key_prefix))
            .map(|((_, k), v)| (k.clone(), v.clone()))
            .collect())
    }

    async fn claim_idempotency(
        &self,
        scope: &str,
        key: &str,
        request_hash: &str,
        now: i64,
    ) -> Result<IdempotencyClaimOutcome, StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let map_key = (scope.to_owned(), key.to_owned());
        if let Some(existing) = guard.idempotency_records.get(&map_key) {
            if existing.request_hash != request_hash {
                return Err(StoreError::Conflict(
                    "Idempotency-Key cannot be reused with a different request payload".into(),
                ));
            }
            if let Some((code, ref body)) = existing.completed {
                return Ok(IdempotencyClaimOutcome::Replayed {
                    status_code: code,
                    body_json: body.clone(),
                });
            }
            if now - existing.created_at_unix < 30 {
                return Err(StoreError::Conflict(
                    "Concurrent request with the same Idempotency-Key is still in progress".into(),
                ));
            }
        }
        guard.idempotency_records.insert(
            map_key,
            IdempotencyRec {
                request_hash: request_hash.to_owned(),
                completed: None,
                created_at_unix: now,
            },
        );
        Ok(IdempotencyClaimOutcome::Acquired)
    }

    async fn complete_idempotency(
        &self,
        scope: &str,
        key: &str,
        status_code: u16,
        body_json: &str,
        _now: i64,
    ) -> Result<(), StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        if let Some(rec) = guard
            .idempotency_records
            .get_mut(&(scope.to_owned(), key.to_owned()))
        {
            rec.completed = Some((status_code, body_json.to_owned()));
        }
        Ok(())
    }

    async fn release_idempotency(&self, scope: &str, key: &str) -> Result<(), StoreError> {
        let mut guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let map_key = (scope.to_owned(), key.to_owned());
        if guard
            .idempotency_records
            .get(&map_key)
            .is_some_and(|r| r.completed.is_none())
        {
            guard.idempotency_records.remove(&map_key);
        }
        Ok(())
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
        let guard = self.state.lock().map_err(|_| StoreError::InternalSafe)?;
        let mut out = BatchedWorkspaceChildren::default();
        if workspace_ids.is_empty() {
            return Ok(out);
        }
        let allowed: std::collections::HashSet<&str> = workspace_ids
            .iter()
            .map(String::as_str)
            .filter(|wid| {
                guard
                    .memberships
                    .contains_key(&((*wid).to_owned(), auth.user_id.clone()))
            })
            .collect();
        if allowed.is_empty() {
            return Ok(out);
        }
        if include_entities {
            out.db_queries_executed += 1;
            for ((wid, _), rec) in &guard.entities {
                if !rec.deleted && allowed.contains(wid.as_str()) {
                    out.entities_by_ws
                        .entry(wid.clone())
                        .or_default()
                        .push(rec.clone());
                }
            }
        }
        if include_members {
            out.db_queries_executed += 2; // 1 memberships batch + 1 users batch
            for ((wid, uid), mrec) in &guard.memberships {
                if allowed.contains(wid.as_str())
                    && let Some(u) = guard.users_by_id.get(uid)
                {
                    let mem_rev = guard
                        .workspaces_by_id
                        .get(wid)
                        .map(|w| w.membership_revision)
                        .unwrap_or(1);
                    out.members_by_ws
                        .entry(wid.clone())
                        .or_default()
                        .push(MemberSummary {
                            user_id: uid.clone(),
                            email: u.email_normalized.clone(),
                            display_name: u.display_name.clone(),
                            role: mrec.role,
                            joined_at_unix: mrec.joined_at_unix,
                            membership_revision: mem_rev,
                        });
                }
            }
        }
        if include_comments {
            out.db_queries_executed += 1;
            for ((wid, _), c) in &guard.comments {
                if allowed.contains(wid.as_str()) {
                    out.comments_by_ws
                        .entry(wid.clone())
                        .or_default()
                        .push(c.clone());
                }
            }
        }
        if include_notifications {
            out.db_queries_executed += 1;
            for n in &guard.notifications {
                if n.user_id == auth.user_id && allowed.contains(n.workspace_id.as_str()) {
                    out.notifications_by_ws
                        .entry(n.workspace_id.clone())
                        .or_default()
                        .push(n.clone());
                }
            }
        }
        Ok(out)
    }
}
