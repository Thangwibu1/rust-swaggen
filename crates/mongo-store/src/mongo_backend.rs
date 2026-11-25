//! Real MongoDB replica-set backend using mongodb 3.10.2 ClientSession multi-doc transactions.
use crate::{
    crypto::{
        LoginRateLimiter, SESSION_TTL_SECS, generate_opaque_token, hash_password, normalize_email,
        normalize_slug, sha256_hex, validate_password, verify_password,
    },
    error::StoreError,
    repo::{AuthContext, BatchedWorkspaceChildren, IdempotencyClaimOutcome, WorkspaceRepository},
};
use mongodb::{
    Client, ClientSession, Collection, Database, IndexModel,
    bson::{Document, doc},
    error::{Error as MongoError, ErrorKind, WriteFailure},
    options::{ClientOptions, FindOptions, IndexOptions, ReadConcern, WriteConcern},
};
use std::time::{Duration, Instant};
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
    UpsertEntityRequest, WorkspaceEntityRecord,
};
use uuid::Uuid;

pub const SCHEMA_MIGRATION_VERSION: i32 = 1;

pub struct MongoStore {
    client: Client,
    db: Database,
    bootstrap_secret: Option<String>,
    limiter: LoginRateLimiter,
}

fn parse_role(s: &str) -> Role {
    match s {
        "owner" => Role::Owner,
        "admin" => Role::Admin,
        "editor" => Role::Editor,
        _ => Role::Viewer,
    }
}

fn is_duplicate_key(err: &MongoError) -> bool {
    match err.kind.as_ref() {
        ErrorKind::Write(WriteFailure::WriteError(we)) => we.code == 11000,
        ErrorKind::BulkWrite(bw) => bw.write_errors.iter().any(|(_, we)| we.code == 11000),
        _ => false,
    }
}

fn map_mongo_err(err: &MongoError) -> StoreError {
    if is_duplicate_key(err) {
        StoreError::Conflict("Duplicate unique key rejected by database".into())
    } else if matches!(
        err.kind.as_ref(),
        ErrorKind::Io(_) | ErrorKind::ServerSelection { .. }
    ) || err.contains_label("TransientTransactionError")
        || err.contains_label("UnknownTransactionCommitResult")
    {
        StoreError::DatabaseUnavailable
    } else {
        StoreError::InternalSafe
    }
}

async fn commit_with_retry(session: &mut ClientSession) -> Result<(), StoreError> {
    let start = Instant::now();
    loop {
        match session.commit_transaction().await {
            Ok(()) => return Ok(()),
            Err(e)
                if e.contains_label("UnknownTransactionCommitResult")
                    && start.elapsed() < Duration::from_secs(15) =>
            {
                tokio::time::sleep(Duration::from_millis(80)).await;
            }
            Err(e) => return Err(map_mongo_err(&e)),
        }
    }
}

impl MongoStore {
    pub fn database(&self) -> &Database {
        &self.db
    }

    pub async fn connect(
        uri: &str,
        db_name: &str,
        bootstrap_secret: Option<String>,
    ) -> Result<Self, StoreError> {
        let mut options = ClientOptions::parse(uri)
            .await
            .map_err(|_| StoreError::DatabaseUnavailable)?;
        options.server_selection_timeout = Some(Duration::from_secs(8));
        options.connect_timeout = Some(Duration::from_secs(5));
        options.app_name = Some("swaggen-server".into());
        let client = Client::with_options(options).map_err(|_| StoreError::DatabaseUnavailable)?;
        let db = client.database(db_name);
        let store = Self {
            client,
            db,
            bootstrap_secret,
            limiter: LoginRateLimiter::default(),
        };
        store.verify_replica_set_topology().await?;
        store.ensure_indexes_and_migrations().await?;
        Ok(store)
    }

    pub async fn verify_replica_set_topology(&self) -> Result<(String, String), StoreError> {
        let admin = self.client.database("admin");
        let hello = admin
            .run_command(doc! { "hello": 1 })
            .await
            .map_err(|_| StoreError::DatabaseUnavailable)?;
        let primary = hello.get_bool("isWritablePrimary").unwrap_or(false);
        let set_name = hello.get_str("setName").unwrap_or("").to_owned();
        if !primary || set_name.is_empty() || !hello.contains_key("logicalSessionTimeoutMinutes") {
            return Err(StoreError::DatabaseUnavailable);
        }
        let build = admin
            .run_command(doc! { "buildInfo": 1 })
            .await
            .map_err(|_| StoreError::DatabaseUnavailable)?;
        let version = build.get_str("version").unwrap_or("unknown").to_owned();
        Ok((set_name, version))
    }

    pub async fn ensure_indexes_and_migrations(&self) -> Result<(), StoreError> {
        self.create_unique("schema_migrations", doc! { "version": 1 })
            .await?;
        self.create_unique("users", doc! { "user_id": 1 }).await?;
        self.create_unique("users", doc! { "email_normalized": 1 })
            .await?;
        self.create_unique("sessions", doc! { "session_id": 1 })
            .await?;
        self.create_unique("sessions", doc! { "token_hash": 1 })
            .await?;
        self.create_index("sessions", doc! { "user_id": 1 }).await?;
        self.create_unique("invitations", doc! { "invitation_id": 1 })
            .await?;
        self.create_unique("invitations", doc! { "token_hash": 1 })
            .await?;
        self.create_index(
            "invitations",
            doc! { "workspace_id": 1, "email_normalized": 1 },
        )
        .await?;
        self.create_unique("public_invite_links", doc! { "link_id": 1 })
            .await?;
        self.create_unique("public_invite_links", doc! { "code_hash": 1 })
            .await?;
        self.create_index("public_invite_links", doc! { "workspace_id": 1 })
            .await?;
        self.create_unique("workspaces", doc! { "workspace_id": 1 })
            .await?;
        self.create_unique("workspaces", doc! { "slug": 1 }).await?;
        self.create_unique("memberships", doc! { "workspace_id": 1, "user_id": 1 })
            .await?;
        self.create_index("memberships", doc! { "user_id": 1 })
            .await?;
        self.create_unique("workspace_permission_fences", doc! { "workspace_id": 1 })
            .await?;
        self.create_unique("workspace_counters", doc! { "workspace_id": 1 })
            .await?;
        self.create_unique("entities", doc! { "workspace_id": 1, "entity_id": 1 })
            .await?;
        self.create_index(
            "entities",
            doc! { "workspace_id": 1, "parent_id": 1, "order_key": 1 },
        )
        .await?;
        self.create_unique(
            "revisions",
            doc! { "workspace_id": 1, "entity_id": 1, "revision": 1 },
        )
        .await?;
        self.create_unique(
            "applied_operations",
            doc! { "workspace_id": 1, "actor_id": 1, "operation_id": 1 },
        )
        .await?;
        self.create_unique("change_events", doc! { "workspace_id": 1, "cursor": 1 })
            .await?;
        self.create_unique("comments", doc! { "workspace_id": 1, "comment_id": 1 })
            .await?;
        self.create_index(
            "comments",
            doc! { "workspace_id": 1, "entity_id": 1, "created_at_unix": -1 },
        )
        .await?;
        self.create_index(
            "notifications",
            doc! { "workspace_id": 1, "user_id": 1, "read_at_unix": 1 },
        )
        .await?;
        self.create_unique("attachments", doc! { "workspace_id": 1, "sha256": 1 })
            .await?;
        self.create_index(
            "audit_events",
            doc! { "workspace_id": 1, "created_at_unix": -1 },
        )
        .await?;
        self.create_unique("server_kv_docs", doc! { "namespace": 1, "key": 1 })
            .await?;
        self.create_unique("idempotency_records", doc! { "scope": 1, "key": 1 })
            .await?;

        let mig = self.db.collection::<Document>("schema_migrations");
        if mig
            .find_one(doc! { "version": SCHEMA_MIGRATION_VERSION })
            .await
            .map_err(|e| map_mongo_err(&e))?
            .is_none()
        {
            let _ = mig
                .insert_one(doc! {
                    "version": SCHEMA_MIGRATION_VERSION,
                    "applied_at_unix": crate::repo::now_unix(),
                    "description": "P04 core auth, workspace RBAC, and sync collection indexes",
                })
                .await;
        }
        Ok(())
    }

    async fn create_unique(&self, col_name: &str, keys: Document) -> Result<(), StoreError> {
        let col: Collection<Document> = self.db.collection(col_name);
        let model = IndexModel::builder()
            .keys(keys)
            .options(IndexOptions::builder().unique(true).build())
            .build();
        col.create_index(model)
            .await
            .map(|_| ())
            .map_err(|e| map_mongo_err(&e))
    }

    async fn create_index(&self, col_name: &str, keys: Document) -> Result<(), StoreError> {
        let col: Collection<Document> = self.db.collection(col_name);
        let model = IndexModel::builder().keys(keys).build();
        col.create_index(model)
            .await
            .map(|_| ())
            .map_err(|e| map_mongo_err(&e))
    }

    /// Scoped doctor probe: performs multi-document commit + explicit rollback on a unique doctor marker.
    pub async fn run_transaction_rollback_smoke_test(&self) -> Result<(), StoreError> {
        let col_a: Collection<Document> = self.db.collection("_doctor_tx_probe_a");
        let col_b: Collection<Document> = self.db.collection("_doctor_tx_probe_b");
        let marker = Uuid::new_v4().to_string();
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        col_a
            .insert_one(doc! { "_id": &marker, "kind": "rollback_test" })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        col_b
            .insert_one(doc! { "_id": &marker, "kind": "rollback_test" })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .abort_transaction()
            .await
            .map_err(|e| map_mongo_err(&e))?;

        let a_found = col_a
            .find_one(doc! { "_id": &marker })
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let b_found = col_b
            .find_one(doc! { "_id": &marker })
            .await
            .map_err(|e| map_mongo_err(&e))?;
        if a_found.is_some() || b_found.is_some() {
            return Err(StoreError::InternalSafe);
        }
        Ok(())
    }

    async fn issue_session_tx(
        &self,
        session: &mut ClientSession,
        user_id: &str,
        device_name: &str,
        now: i64,
    ) -> Result<(String, String, i64), StoreError> {
        let session_id = Uuid::new_v4().to_string();
        let token = generate_opaque_token("swg");
        let token_hash = sha256_hex(&token);
        let expires = now + SESSION_TTL_SECS;
        let clean_device: String = if device_name.trim().is_empty() {
            "Swaggen Desktop".to_owned()
        } else {
            device_name.trim().chars().take(80).collect()
        };
        let col: Collection<Document> = self.db.collection("sessions");
        col.insert_one(doc! {
            "session_id": &session_id,
            "token_hash": &token_hash,
            "user_id": user_id,
            "device_name": &clean_device,
            "created_at_unix": now,
            "expires_at_unix": expires,
            "last_seen_unix": now,
            "revoked_at_unix": bson::Bson::Null,
        })
        .session(session)
        .await
        .map_err(|e| map_mongo_err(&e))?;
        Ok((session_id, token, expires))
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_audit_tx(
        &self,
        session: &mut ClientSession,
        workspace_id: Option<&str>,
        actor_user_id: &str,
        actor_email: &str,
        action: &str,
        target_id: &str,
        detail_redacted: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        let col: Collection<Document> = self.db.collection("audit_events");
        col.insert_one(doc! {
            "event_id": Uuid::new_v4().to_string(),
            "workspace_id": workspace_id,
            "actor_user_id": actor_user_id,
            "actor_email": actor_email,
            "action": action,
            "target_id": target_id,
            "detail_redacted": detail_redacted,
            "created_at_unix": now,
        })
        .session(session)
        .await
        .map_err(|e| map_mongo_err(&e))?;
        Ok(())
    }

    async fn require_membership_tx(
        &self,
        session: &mut ClientSession,
        workspace_id: &str,
        user_id: &str,
    ) -> Result<(Document, Role, i64), StoreError> {
        let fences: Collection<Document> = self.db.collection("workspace_permission_fences");
        // Acquire workspace fence lock by touching fence sequence inside the transaction so permission revokes serialize cleanly.
        let fence_doc = fences
            .find_one(doc! { "workspace_id": workspace_id })
            .session(&mut *session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::Forbidden("Workspace access denied".into()))?;
        let mem_rev = fence_doc.get_i64("membership_revision").unwrap_or(1);
        let mem_col: Collection<Document> = self.db.collection("memberships");
        let mem = mem_col
            .find_one(doc! { "workspace_id": workspace_id, "user_id": user_id })
            .session(&mut *session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::Forbidden("Workspace access denied".into()))?;
        let role = parse_role(mem.get_str("role").unwrap_or("viewer"));
        let ws_col: Collection<Document> = self.db.collection("workspaces");
        let ws = ws_col
            .find_one(doc! { "workspace_id": workspace_id })
            .session(session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Workspace not found".into()))?;
        Ok((ws, role, mem_rev))
    }
}

impl WorkspaceRepository for MongoStore {
    async fn check_ready(&self) -> Result<(), StoreError> {
        self.verify_replica_set_topology().await.map(|_| ())
    }

    async fn bootstrap_required(&self) -> Result<bool, StoreError> {
        let users: Collection<Document> = self.db.collection("users");
        let count = users
            .count_documents(doc! {})
            .await
            .map_err(|e| map_mongo_err(&e))?;
        Ok(count == 0)
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

        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;

        let users: Collection<Document> = self.db.collection("users");
        if users
            .find_one(doc! {})
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .is_some()
        {
            let _ = session.abort_transaction().await;
            return Err(StoreError::BootstrapCompleted);
        }
        let user_id = Uuid::new_v4().to_string();
        users
            .insert_one(doc! {
                "user_id": &user_id,
                "email_normalized": &email,
                "display_name": display_name,
                "password_hash": &pw_hash,
                "is_server_owner": true,
                "created_at_unix": now,
            })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (session_id, token, expires) = self
            .issue_session_tx(&mut session, &user_id, &req.device_name, now)
            .await?;
        self.record_audit_tx(
            &mut session,
            None,
            &user_id,
            &email,
            "server.bootstrap_owner",
            &user_id,
            "First server owner initialized",
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
        Ok(AuthResponse {
            api_version: API_VERSION,
            user: swaggen_api_contract::UserProfile {
                user_id,
                email,
                display_name: display_name.to_owned(),
                is_server_owner: true,
                created_at_unix: now,
            },
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

        let users: Collection<Document> = self.db.collection("users");
        let Some(user_doc) = users
            .find_one(doc! { "email_normalized": &email })
            .await
            .map_err(|e| map_mongo_err(&e))?
        else {
            self.limiter.record_failure(&rate_key, now);
            return Err(StoreError::Unauthorized("Invalid email or password".into()));
        };
        let pw_hash = user_doc.get_str("password_hash").unwrap_or("");
        if !verify_password(&req.password, pw_hash) {
            self.limiter.record_failure(&rate_key, now);
            return Err(StoreError::Unauthorized("Invalid email or password".into()));
        }
        self.limiter.clear(&rate_key);
        let user_id = user_doc.get_str("user_id").unwrap_or("").to_owned();
        let display_name = user_doc.get_str("display_name").unwrap_or("").to_owned();
        let is_owner = user_doc.get_bool("is_server_owner").unwrap_or(false);
        let created_at = user_doc.get_i64("created_at_unix").unwrap_or(now);

        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (session_id, token, expires) = self
            .issue_session_tx(&mut session, &user_id, &req.device_name, now)
            .await?;
        self.record_audit_tx(
            &mut session,
            None,
            &user_id,
            &email,
            "auth.login",
            &session_id,
            "Internal account session created",
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
        Ok(AuthResponse {
            api_version: API_VERSION,
            user: swaggen_api_contract::UserProfile {
                user_id,
                email,
                display_name,
                is_server_owner: is_owner,
                created_at_unix: created_at,
            },
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
        let clean_name: String = if display_name.trim().is_empty() {
            email.split('@').next().unwrap_or("User").to_owned()
        } else {
            display_name.trim().chars().take(80).collect()
        };

        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;

        let users: Collection<Document> = self.db.collection("users");
        let existing = match users
            .find_one(doc! { "firebase_uid": uid })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            Some(d) => Some(d),
            None => users
                .find_one(doc! { "email_normalized": &email })
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?,
        };

        let (user_id, final_name, is_owner, created_at, action) = if let Some(doc) = existing {
            let user_id = doc.get_str("user_id").unwrap_or("").to_owned();
            let final_name = doc
                .get_str("display_name")
                .unwrap_or(&clean_name)
                .to_owned();
            let is_owner = doc.get_bool("is_server_owner").unwrap_or(false);
            let created_at = doc.get_i64("created_at_unix").unwrap_or(now);
            if doc.get_str("firebase_uid").ok() != Some(uid) {
                users
                    .update_one(
                        doc! { "user_id": &user_id },
                        doc! { "$set": { "firebase_uid": uid } },
                    )
                    .session(&mut session)
                    .await
                    .map_err(|e| map_mongo_err(&e))?;
            }
            (
                user_id,
                final_name,
                is_owner,
                created_at,
                "auth.firebase_login",
            )
        } else {
            // Auto-provision account seamlessly for any Firebase-verified user
            // (first user becomes server owner automatically; never blocks Google Sign-In).
            let _ = bootstrap_token;
            let is_first = users
                .find_one(doc! {})
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?
                .is_none();
            let user_id = Uuid::new_v4().to_string();
            users
                .insert_one(doc! {
                    "user_id": &user_id,
                    "firebase_uid": uid,
                    "email_normalized": &email,
                    "display_name": &clean_name,
                    "password_hash": "!firebase-only!",
                    "is_server_owner": is_first,
                    "created_at_unix": now,
                })
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
            (user_id, clean_name, is_first, now, "auth.firebase_register")
        };

        let (session_id, token, expires) = self
            .issue_session_tx(&mut session, &user_id, device_name, now)
            .await?;
        self.record_audit_tx(
            &mut session,
            None,
            &user_id,
            &email,
            action,
            &session_id,
            "Firebase verified session created",
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
        Ok(AuthResponse {
            api_version: API_VERSION,
            user: swaggen_api_contract::UserProfile {
                user_id,
                email,
                display_name: final_name,
                is_server_owner: is_owner,
                created_at_unix: created_at,
            },
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
        let sessions: Collection<Document> = self.db.collection("sessions");
        let sess = sessions
            .find_one(doc! { "token_hash": &hash })
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::Unauthorized("Invalid or unknown session".into()))?;
        if !sess
            .get("revoked_at_unix")
            .is_some_and(|b| b.as_null().is_some())
        {
            return Err(StoreError::Unauthorized("Session has been revoked".into()));
        }
        if sess.get_i64("expires_at_unix").unwrap_or(0) <= now {
            return Err(StoreError::Unauthorized("Session has expired".into()));
        }
        let session_id = sess.get_str("session_id").unwrap_or("").to_owned();
        let user_id = sess.get_str("user_id").unwrap_or("").to_owned();
        let _ = sessions
            .update_one(
                doc! { "session_id": &session_id },
                doc! { "$set": { "last_seen_unix": now } },
            )
            .await;
        let users: Collection<Document> = self.db.collection("users");
        let user = users
            .find_one(doc! { "user_id": &user_id })
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::Unauthorized("Account not found".into()))?;
        Ok(AuthContext {
            user_id,
            email: user.get_str("email_normalized").unwrap_or("").to_owned(),
            display_name: user.get_str("display_name").unwrap_or("").to_owned(),
            is_server_owner: user.get_bool("is_server_owner").unwrap_or(false),
            session_id,
        })
    }

    async fn list_sessions(
        &self,
        auth: &AuthContext,
        now: i64,
    ) -> Result<Vec<SessionSummary>, StoreError> {
        let sessions: Collection<Document> = self.db.collection("sessions");
        let mut cursor = sessions
            .find(doc! { "user_id": &auth.user_id, "expires_at_unix": { "$gt": now } })
            .with_options(
                FindOptions::builder()
                    .sort(doc! { "created_at_unix": -1 })
                    .build(),
            )
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut out = Vec::new();
        while cursor.advance().await.map_err(|e| map_mongo_err(&e))? {
            let d = cursor
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            let sid = d.get_str("session_id").unwrap_or("").to_owned();
            out.push(SessionSummary {
                is_current: sid == auth.session_id,
                session_id: sid,
                device_name: d.get_str("device_name").unwrap_or("").to_owned(),
                created_at_unix: d.get_i64("created_at_unix").unwrap_or(0),
                expires_at_unix: d.get_i64("expires_at_unix").unwrap_or(0),
                last_seen_unix: d.get_i64("last_seen_unix").unwrap_or(0),
                revoked_at_unix: d.get_i64("revoked_at_unix").ok(),
            });
        }
        Ok(out)
    }

    async fn revoke_session(
        &self,
        auth: &AuthContext,
        session_id: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        let sessions: Collection<Document> = self.db.collection("sessions");
        let target = sessions
            .find_one(doc! { "session_id": session_id })
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Session not found".into()))?;
        if target.get_str("user_id").unwrap_or("") != auth.user_id && !auth.is_server_owner {
            return Err(StoreError::Forbidden(
                "Cannot revoke another user's session".into(),
            ));
        }
        sessions
            .update_one(
                doc! { "session_id": session_id },
                doc! { "$set": { "revoked_at_unix": now } },
            )
            .await
            .map_err(|e| map_mongo_err(&e))?;
        Ok(())
    }

    async fn logout_all(&self, auth: &AuthContext, now: i64) -> Result<usize, StoreError> {
        let sessions: Collection<Document> = self.db.collection("sessions");
        let res = sessions
            .update_many(
                doc! { "user_id": &auth.user_id, "revoked_at_unix": bson::Bson::Null },
                doc! { "$set": { "revoked_at_unix": now } },
            )
            .await
            .map_err(|e| map_mongo_err(&e))?;
        Ok(res.modified_count as usize)
    }

    async fn change_password(
        &self,
        auth: &AuthContext,
        req: ChangePasswordRequest,
        now: i64,
    ) -> Result<(), StoreError> {
        validate_password(&req.new_password)?;
        let new_hash = hash_password(&req.new_password)?;
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let users: Collection<Document> = self.db.collection("users");
        let user = users
            .find_one(doc! { "user_id": &auth.user_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Account not found".into()))?;
        if !verify_password(
            &req.current_password,
            user.get_str("password_hash").unwrap_or(""),
        ) {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Unauthorized(
                "Current password is incorrect".into(),
            ));
        }
        users
            .update_one(
                doc! { "user_id": &auth.user_id },
                doc! { "$set": { "password_hash": &new_hash } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        if req.revoke_other_sessions {
            let sessions: Collection<Document> = self.db.collection("sessions");
            sessions
                .update_many(
                    doc! { "user_id": &auth.user_id, "session_id": { "$ne": &auth.session_id } },
                    doc! { "$set": { "revoked_at_unix": now } },
                )
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
        }
        self.record_audit_tx(
            &mut session,
            None,
            &auth.user_id,
            &auth.email,
            "auth.password_changed",
            &auth.user_id,
            "Password changed",
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
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
        let workspace_id = Uuid::new_v4().to_string();
        let desc: String = req.description.trim().chars().take(500).collect();

        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let ws_col: Collection<Document> = self.db.collection("workspaces");
        ws_col
            .insert_one(doc! {
                "workspace_id": &workspace_id,
                "slug": &slug,
                "name": name,
                "description": &desc,
                "current_revision": 1i64,
                "archived": false,
                "created_at_unix": now,
            })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let fences: Collection<Document> = self.db.collection("workspace_permission_fences");
        fences
            .insert_one(doc! { "workspace_id": &workspace_id, "membership_revision": 1i64 })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let counters: Collection<Document> = self.db.collection("workspace_counters");
        counters
            .insert_one(doc! { "workspace_id": &workspace_id, "cursor": 0i64 })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mem: Collection<Document> = self.db.collection("memberships");
        mem.insert_one(doc! {
            "workspace_id": &workspace_id,
            "user_id": &auth.user_id,
            "role": "owner",
            "joined_at_unix": now,
        })
        .session(&mut session)
        .await
        .map_err(|e| map_mongo_err(&e))?;
        self.record_audit_tx(
            &mut session,
            Some(&workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.created",
            &workspace_id,
            &format!("Workspace '{name}' created"),
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
        Ok(TeamWorkspaceSummary {
            workspace_id,
            slug,
            name: name.to_owned(),
            description: desc,
            role: Role::Owner,
            member_count: 1,
            current_revision: 1,
            membership_revision: 1,
            archived: false,
        })
    }

    async fn list_workspaces(
        &self,
        auth: &AuthContext,
    ) -> Result<Vec<TeamWorkspaceSummary>, StoreError> {
        let mem_col: Collection<Document> = self.db.collection("memberships");
        let ws_col: Collection<Document> = self.db.collection("workspaces");
        let fences: Collection<Document> = self.db.collection("workspace_permission_fences");

        let mut cursor = mem_col
            .find(doc! { "user_id": &auth.user_id })
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut caller_mems: Vec<(String, Role)> = Vec::new();
        while cursor.advance().await.map_err(|e| map_mongo_err(&e))? {
            let mem = cursor
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            let wid = mem.get_str("workspace_id").unwrap_or("");
            if !wid.is_empty() {
                caller_mems.push((
                    wid.to_owned(),
                    parse_role(mem.get_str("role").unwrap_or("viewer")),
                ));
            }
        }
        if caller_mems.is_empty() {
            return Ok(Vec::new());
        }

        // Batch-fetch workspaces, permission fences, and member counts in O(1) queries (ZERO N+1!).
        let wids: Vec<&str> = caller_mems.iter().map(|(w, _)| w.as_str()).collect();
        let mut ws_map = std::collections::HashMap::with_capacity(wids.len());
        let mut ws_cur = ws_col
            .find(doc! { "workspace_id": { "$in": &wids } })
            .await
            .map_err(|e| map_mongo_err(&e))?;
        while ws_cur.advance().await.map_err(|e| map_mongo_err(&e))? {
            let ws = ws_cur
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            if let Ok(wid) = ws.get_str("workspace_id") {
                ws_map.insert(wid.to_owned(), ws);
            }
        }

        let mut fence_map = std::collections::HashMap::with_capacity(wids.len());
        let mut fence_cur = fences
            .find(doc! { "workspace_id": { "$in": &wids } })
            .await
            .map_err(|e| map_mongo_err(&e))?;
        while fence_cur.advance().await.map_err(|e| map_mongo_err(&e))? {
            let fd = fence_cur
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            if let (Ok(wid), Ok(rev)) = (
                fd.get_str("workspace_id"),
                fd.get_i64("membership_revision"),
            ) {
                fence_map.insert(wid.to_owned(), rev);
            }
        }

        let mut count_map = std::collections::HashMap::with_capacity(wids.len());
        let mut agg_cur = mem_col
            .aggregate([
                doc! { "$match": { "workspace_id": { "$in": &wids } } },
                doc! { "$group": { "_id": "$workspace_id", "count": { "$sum": 1_i32 } } },
            ])
            .await
            .map_err(|e| map_mongo_err(&e))?;
        while agg_cur.advance().await.map_err(|e| map_mongo_err(&e))? {
            let gd = agg_cur
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            if let Ok(wid) = gd.get_str("_id") {
                let cnt = gd
                    .get_i32("count")
                    .map(|v| v as usize)
                    .or_else(|_| gd.get_i64("count").map(|v| v as usize))
                    .unwrap_or(1);
                count_map.insert(wid.to_owned(), cnt);
            }
        }

        let mut out = Vec::with_capacity(caller_mems.len());
        for (wid, role) in caller_mems {
            if let Some(ws) = ws_map.get(&wid) {
                let count = count_map.get(&wid).copied().unwrap_or(1);
                let mem_rev = fence_map.get(&wid).copied().unwrap_or(1);
                out.push(TeamWorkspaceSummary {
                    workspace_id: wid,
                    slug: ws.get_str("slug").unwrap_or("").to_owned(),
                    name: ws.get_str("name").unwrap_or("").to_owned(),
                    description: ws.get_str("description").unwrap_or("").to_owned(),
                    role,
                    member_count: count,
                    current_revision: ws.get_i64("current_revision").unwrap_or(1),
                    membership_revision: mem_rev,
                    archived: ws.get_bool("archived").unwrap_or(false),
                });
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
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (ws, caller_role, mem_rev) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        let mem_col: Collection<Document> = self.db.collection("memberships");
        let users_col: Collection<Document> = self.db.collection("users");
        let mut cursor = mem_col
            .find(doc! { "workspace_id": workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut raw_members: Vec<(String, Role, i64)> = Vec::new();
        while cursor
            .advance(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            let m = cursor
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            let uid = m.get_str("user_id").unwrap_or("");
            if !uid.is_empty() {
                raw_members.push((
                    uid.to_owned(),
                    parse_role(m.get_str("role").unwrap_or("viewer")),
                    m.get_i64("joined_at_unix").unwrap_or(0),
                ));
            }
        }
        drop(cursor);

        // Batch-fetch all member user profiles in ONE `$in` query (ZERO N+1!).
        let mut user_map = std::collections::HashMap::with_capacity(raw_members.len());
        if !raw_members.is_empty() {
            let uids: Vec<&str> = raw_members.iter().map(|(u, _, _)| u.as_str()).collect();
            let mut ucur = users_col
                .find(doc! { "user_id": { "$in": &uids } })
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
            while ucur
                .advance(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?
            {
                let u = ucur.deserialize_current().map_err(|e| map_mongo_err(&e))?;
                if let Ok(uid) = u.get_str("user_id") {
                    user_map.insert(
                        uid.to_owned(),
                        (
                            u.get_str("email_normalized").unwrap_or("").to_owned(),
                            u.get_str("display_name").unwrap_or("").to_owned(),
                        ),
                    );
                }
            }
            drop(ucur);
        }

        let mut members = Vec::with_capacity(raw_members.len());
        for (uid, role, joined_at_unix) in raw_members {
            if let Some((email, display_name)) = user_map.remove(&uid) {
                members.push(MemberSummary {
                    user_id: uid,
                    email,
                    display_name,
                    role,
                    joined_at_unix,
                    membership_revision: mem_rev,
                });
            }
        }
        let mut invitations = Vec::new();
        if caller_role.can_manage_members() {
            let inv_col: Collection<Document> = self.db.collection("invitations");
            let ws_name = ws.get_str("name").unwrap_or("").to_owned();
            let mut ic = inv_col
                .find(doc! { "workspace_id": workspace_id })
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
            while ic
                .advance(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?
            {
                let d = ic.deserialize_current().map_err(|e| map_mongo_err(&e))?;
                invitations.push(InvitationSummary {
                    invitation_id: d.get_str("invitation_id").unwrap_or("").to_owned(),
                    workspace_id: workspace_id.to_owned(),
                    workspace_name: ws_name.clone(),
                    email: d.get_str("email_normalized").unwrap_or("").to_owned(),
                    role: parse_role(d.get_str("role").unwrap_or("viewer")),
                    created_at_unix: d.get_i64("created_at_unix").unwrap_or(0),
                    expires_at_unix: d.get_i64("expires_at_unix").unwrap_or(0),
                    used_at_unix: d.get_i64("used_at_unix").ok(),
                    revoked_at_unix: d.get_i64("revoked_at_unix").ok(),
                });
            }
        }
        commit_with_retry(&mut session).await?;
        Ok(MemberListResponse {
            api_version: API_VERSION,
            workspace_id: workspace_id.to_owned(),
            membership_revision: mem_rev,
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
                "Invitations cannot grant Owner role directly".into(),
            ));
        }
        let email = normalize_email(&req.email)?;
        let hours = req.expires_in_hours.clamp(1, 168) as i64;
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (ws, caller_role, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if !caller_role.can_manage_members() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can create invitations".into(),
            ));
        }
        let ws_name = ws.get_str("name").unwrap_or("").to_owned();
        let invitation_id = Uuid::new_v4().to_string();
        let token = generate_opaque_token("inv");
        let token_hash = sha256_hex(&token);
        let expires = now + hours * 3600;
        let inv_col: Collection<Document> = self.db.collection("invitations");
        inv_col
            .insert_one(doc! {
                "invitation_id": &invitation_id,
                "token_hash": &token_hash,
                "workspace_id": workspace_id,
                "email_normalized": &email,
                "role": req.role.as_str(),
                "created_at_unix": now,
                "expires_at_unix": expires,
                "used_at_unix": bson::Bson::Null,
                "revoked_at_unix": bson::Bson::Null,
            })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        self.record_audit_tx(
            &mut session,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.invitation_created",
            &invitation_id,
            &format!("Invited {email} as {}", req.role.as_str()),
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
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
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, caller_role, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if !caller_role.can_manage_members() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can revoke invitations".into(),
            ));
        }
        let inv_col: Collection<Document> = self.db.collection("invitations");
        let inv = inv_col
            .find_one(doc! { "invitation_id": invitation_id, "workspace_id": workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Invitation not found".into()))?;
        if !inv
            .get("used_at_unix")
            .is_some_and(|b| b.as_null().is_some())
        {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationAlreadyUsed);
        }
        inv_col
            .update_one(
                doc! { "invitation_id": invitation_id },
                doc! { "$set": { "revoked_at_unix": now } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        self.record_audit_tx(
            &mut session,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.invitation_revoked",
            invitation_id,
            "Invitation revoked",
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
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
        let email = normalize_email(&req.email)?;

        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;

        let inv_col: Collection<Document> = self.db.collection("invitations");
        let inv = inv_col
            .find_one(doc! { "token_hash": &token_hash })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Invitation token not recognized".into()))?;
        if !inv
            .get("revoked_at_unix")
            .is_some_and(|b| b.as_null().is_some())
        {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationRevoked);
        }
        if !inv
            .get("used_at_unix")
            .is_some_and(|b| b.as_null().is_some())
        {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationAlreadyUsed);
        }
        if inv.get_i64("expires_at_unix").unwrap_or(0) <= now {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationExpired);
        }
        let inv_email = inv.get_str("email_normalized").unwrap_or("");
        if email != inv_email || caller.is_some_and(|c| c.email != inv_email) {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationAccountMismatch);
        }

        let inv_id = inv.get_str("invitation_id").unwrap_or("").to_owned();
        let workspace_id = inv.get_str("workspace_id").unwrap_or("").to_owned();
        let role = parse_role(inv.get_str("role").unwrap_or("viewer"));

        // Conditional atomic consume inside transaction
        let res = inv_col
            .update_one(
                doc! {
                    "invitation_id": &inv_id,
                    "used_at_unix": bson::Bson::Null,
                    "revoked_at_unix": bson::Bson::Null,
                },
                doc! { "$set": { "used_at_unix": now } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        if res.matched_count != 1 {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationAlreadyUsed);
        }

        let users: Collection<Document> = self.db.collection("users");
        let (user_id, display_name, is_server_owner, created_at) = if let Some(u) = users
            .find_one(doc! { "email_normalized": &email })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            if caller.is_none()
                && !verify_password(
                    req.password.as_deref().unwrap_or(""),
                    u.get_str("password_hash").unwrap_or(""),
                )
            {
                let _ = session.abort_transaction().await;
                return Err(StoreError::Unauthorized(
                    "Existing account password required to accept invitation".into(),
                ));
            }
            (
                u.get_str("user_id").unwrap_or("").to_owned(),
                u.get_str("display_name").unwrap_or("").to_owned(),
                u.get_bool("is_server_owner").unwrap_or(false),
                u.get_i64("created_at_unix").unwrap_or(now),
            )
        } else {
            let dn = req.display_name.trim();
            if dn.is_empty() || dn.len() > 80 {
                let _ = session.abort_transaction().await;
                return Err(StoreError::InvalidInput("Display name is required".into()));
            }
            let pw = req.password.as_deref().ok_or_else(|| {
                StoreError::InvalidInput("Password required for new account".into())
            })?;
            let pw_hash = hash_password(pw)?;
            let uid = Uuid::new_v4().to_string();
            users
                .insert_one(doc! {
                    "user_id": &uid,
                    "email_normalized": &email,
                    "display_name": dn,
                    "password_hash": &pw_hash,
                    "is_server_owner": false,
                    "created_at_unix": now,
                })
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
            (uid, dn.to_owned(), false, now)
        };

        let mem_col: Collection<Document> = self.db.collection("memberships");
        mem_col
            .insert_one(doc! {
                "workspace_id": &workspace_id,
                "user_id": &user_id,
                "role": role.as_str(),
                "joined_at_unix": now,
            })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;

        let fences: Collection<Document> = self.db.collection("workspace_permission_fences");
        fences
            .update_one(
                doc! { "workspace_id": &workspace_id },
                doc! { "$inc": { "membership_revision": 1i64 } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;

        let ws_col: Collection<Document> = self.db.collection("workspaces");
        let ws = ws_col
            .find_one(doc! { "workspace_id": &workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Workspace not found".into()))?;
        let fence_doc = fences
            .find_one(doc! { "workspace_id": &workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mem_rev = fence_doc
            .and_then(|d| d.get_i64("membership_revision").ok())
            .unwrap_or(1);

        let (session_id, session_token, expires) = self
            .issue_session_tx(&mut session, &user_id, &req.device_name, now)
            .await?;
        self.record_audit_tx(
            &mut session,
            Some(&workspace_id),
            &user_id,
            &email,
            "workspace.invitation_accepted",
            &inv_id,
            &format!("Joined workspace as {}", role.as_str()),
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
        let member_count = mem_col
            .count_documents(doc! { "workspace_id": &workspace_id })
            .await
            .unwrap_or(1) as usize;
        Ok(AcceptInvitationResponse {
            api_version: API_VERSION,
            workspace: TeamWorkspaceSummary {
                workspace_id,
                slug: ws.get_str("slug").unwrap_or("").to_owned(),
                name: ws.get_str("name").unwrap_or("").to_owned(),
                description: ws.get_str("description").unwrap_or("").to_owned(),
                role,
                member_count,
                current_revision: ws.get_i64("current_revision").unwrap_or(1),
                membership_revision: mem_rev,
                archived: ws.get_bool("archived").unwrap_or(false),
            },
            user: swaggen_api_contract::UserProfile {
                user_id,
                email,
                display_name,
                is_server_owner,
                created_at_unix: created_at,
            },
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
        let email = normalize_email(&auth.email)?;
        let inv_col: Collection<Document> = self.db.collection("invitations");
        let ws_col: Collection<Document> = self.db.collection("workspaces");
        let mut cursor = inv_col
            .find(doc! {
                "email_normalized": &email,
                "used_at_unix": bson::Bson::Null,
                "revoked_at_unix": bson::Bson::Null,
                "expires_at_unix": { "$gt": now },
            })
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut raw = Vec::new();
        let mut ws_ids = Vec::new();
        while cursor.advance().await.map_err(|e| map_mongo_err(&e))? {
            let d = cursor
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            let wid = d.get_str("workspace_id").unwrap_or("").to_owned();
            if !wid.is_empty() && !ws_ids.contains(&wid) {
                ws_ids.push(wid);
            }
            raw.push(d);
        }
        let mut ws_names: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        if !ws_ids.is_empty() {
            let mut wcur = ws_col
                .find(doc! { "workspace_id": { "$in": &ws_ids } })
                .await
                .map_err(|e| map_mongo_err(&e))?;
            while wcur.advance().await.map_err(|e| map_mongo_err(&e))? {
                let w = wcur.deserialize_current().map_err(|e| map_mongo_err(&e))?;
                if let (Ok(id), Ok(name)) = (w.get_str("workspace_id"), w.get_str("name")) {
                    ws_names.insert(id.to_owned(), name.to_owned());
                }
            }
        }
        let mut out: Vec<InvitationSummary> = raw
            .into_iter()
            .filter_map(|d| {
                let wid = d.get_str("workspace_id").ok()?.to_owned();
                let wname = ws_names.get(&wid)?.clone();
                Some(InvitationSummary {
                    invitation_id: d.get_str("invitation_id").unwrap_or("").to_owned(),
                    workspace_id: wid,
                    workspace_name: wname,
                    email: d.get_str("email_normalized").unwrap_or("").to_owned(),
                    role: parse_role(d.get_str("role").unwrap_or("viewer")),
                    created_at_unix: d.get_i64("created_at_unix").unwrap_or(0),
                    expires_at_unix: d.get_i64("expires_at_unix").unwrap_or(0),
                    used_at_unix: None,
                    revoked_at_unix: None,
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
        let email = normalize_email(&auth.email)?;
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;

        let inv_col: Collection<Document> = self.db.collection("invitations");
        let inv = inv_col
            .find_one(doc! { "invitation_id": invitation_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Invitation not found".into()))?;
        if !inv
            .get("revoked_at_unix")
            .is_some_and(|b| b.as_null().is_some())
        {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationRevoked);
        }
        if !inv
            .get("used_at_unix")
            .is_some_and(|b| b.as_null().is_some())
        {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationAlreadyUsed);
        }
        if inv.get_i64("expires_at_unix").unwrap_or(0) <= now {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationExpired);
        }
        let inv_email = inv.get_str("email_normalized").unwrap_or("");
        if email != inv_email {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationAccountMismatch);
        }

        let workspace_id = inv.get_str("workspace_id").unwrap_or("").to_owned();
        let role = parse_role(inv.get_str("role").unwrap_or("viewer"));

        let res = inv_col
            .update_one(
                doc! {
                    "invitation_id": invitation_id,
                    "used_at_unix": bson::Bson::Null,
                    "revoked_at_unix": bson::Bson::Null,
                },
                doc! { "$set": { "used_at_unix": now } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        if res.matched_count != 1 {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationAlreadyUsed);
        }

        let mem_col: Collection<Document> = self.db.collection("memberships");
        let existing_mem = mem_col
            .find_one(doc! { "workspace_id": &workspace_id, "user_id": &auth.user_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        if existing_mem.is_none() {
            mem_col
                .insert_one(doc! {
                    "workspace_id": &workspace_id,
                    "user_id": &auth.user_id,
                    "role": role.as_str(),
                    "joined_at_unix": now,
                })
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
        }

        let fences: Collection<Document> = self.db.collection("workspace_permission_fences");
        fences
            .update_one(
                doc! { "workspace_id": &workspace_id },
                doc! { "$inc": { "membership_revision": 1i64 } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;

        let ws_col: Collection<Document> = self.db.collection("workspaces");
        let ws = ws_col
            .find_one(doc! { "workspace_id": &workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Workspace not found".into()))?;
        let fence_doc = fences
            .find_one(doc! { "workspace_id": &workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mem_rev = fence_doc
            .and_then(|d| d.get_i64("membership_revision").ok())
            .unwrap_or(1);

        let users: Collection<Document> = self.db.collection("users");
        let created_at = users
            .find_one(doc! { "user_id": &auth.user_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .and_then(|u| u.get_i64("created_at_unix").ok())
            .unwrap_or(now);

        self.record_audit_tx(
            &mut session,
            Some(&workspace_id),
            &auth.user_id,
            &email,
            "workspace.invitation_accepted",
            invitation_id,
            &format!("Accepted in-app invitation as {}", role.as_str()),
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
        let member_count = mem_col
            .count_documents(doc! { "workspace_id": &workspace_id })
            .await
            .unwrap_or(1) as usize;
        Ok(AcceptInvitationResponse {
            api_version: API_VERSION,
            workspace: TeamWorkspaceSummary {
                workspace_id,
                slug: ws.get_str("slug").unwrap_or("").to_owned(),
                name: ws.get_str("name").unwrap_or("").to_owned(),
                description: ws.get_str("description").unwrap_or("").to_owned(),
                role,
                member_count,
                current_revision: ws.get_i64("current_revision").unwrap_or(1),
                membership_revision: mem_rev,
                archived: ws.get_bool("archived").unwrap_or(false),
            },
            user: swaggen_api_contract::UserProfile {
                user_id: auth.user_id.clone(),
                email,
                display_name: auth.display_name.clone(),
                is_server_owner: auth.is_server_owner,
                created_at_unix: created_at,
            },
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
        let email = normalize_email(&auth.email)?;
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let inv_col: Collection<Document> = self.db.collection("invitations");
        let inv = inv_col
            .find_one(doc! { "invitation_id": invitation_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Invitation not found".into()))?;
        if inv.get_str("email_normalized").unwrap_or("") != email {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationAccountMismatch);
        }
        if !inv
            .get("used_at_unix")
            .is_some_and(|b| b.as_null().is_some())
        {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationAlreadyUsed);
        }
        let workspace_id = inv.get_str("workspace_id").unwrap_or("").to_owned();
        inv_col
            .update_one(
                doc! { "invitation_id": invitation_id },
                doc! { "$set": { "revoked_at_unix": now } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        self.record_audit_tx(
            &mut session,
            Some(&workspace_id),
            &auth.user_id,
            &email,
            "workspace.invitation_declined",
            invitation_id,
            "Invited user declined in-app",
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
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
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (ws, caller_role, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if !caller_role.can_manage_members() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can create shareable invite links".into(),
            ));
        }
        let ws_name = ws.get_str("name").unwrap_or("").to_owned();
        let link_id = Uuid::new_v4().to_string();
        let invite_code = generate_opaque_token("lnk");
        let code_hash = sha256_hex(&invite_code);
        let share_link = format!("swaggen://join/{workspace_id}/{invite_code}");
        let expires = now + hours * 3600;
        let links_col: Collection<Document> = self.db.collection("public_invite_links");
        links_col
            .insert_one(doc! {
                "link_id": &link_id,
                "code_hash": &code_hash,
                "invite_code": &invite_code,
                "share_link": &share_link,
                "workspace_id": workspace_id,
                "workspace_name": &ws_name,
                "role": req.role.as_str(),
                "created_at_unix": now,
                "expires_at_unix": expires,
                "max_uses": max_uses.map(|m| m as i64),
                "uses_count": 0i64,
                "revoked_at_unix": bson::Bson::Null,
            })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        self.record_audit_tx(
            &mut session,
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
        )
        .await?;
        commit_with_retry(&mut session).await?;
        Ok(CreatePublicInviteLinkResponse {
            api_version: API_VERSION,
            link: PublicInviteLinkSummary {
                link_id,
                workspace_id: workspace_id.to_owned(),
                workspace_name: ws_name,
                role: req.role,
                invite_code,
                share_link,
                created_at_unix: now,
                expires_at_unix: expires,
                max_uses,
                uses_count: 0,
                revoked_at_unix: None,
            },
        })
    }

    async fn list_public_invite_links(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<Vec<PublicInviteLinkSummary>, StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, caller_role, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if !caller_role.can_manage_members() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can list shareable invite links".into(),
            ));
        }
        let links_col: Collection<Document> = self.db.collection("public_invite_links");
        let mut cursor = links_col
            .find(doc! { "workspace_id": workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut out = Vec::new();
        while cursor
            .advance(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            let d = cursor
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            out.push(PublicInviteLinkSummary {
                link_id: d.get_str("link_id").unwrap_or("").to_owned(),
                workspace_id: workspace_id.to_owned(),
                workspace_name: d.get_str("workspace_name").unwrap_or("").to_owned(),
                role: parse_role(d.get_str("role").unwrap_or("viewer")),
                invite_code: d.get_str("invite_code").unwrap_or("").to_owned(),
                share_link: d.get_str("share_link").unwrap_or("").to_owned(),
                created_at_unix: d.get_i64("created_at_unix").unwrap_or(0),
                expires_at_unix: d.get_i64("expires_at_unix").unwrap_or(0),
                max_uses: d.get_i64("max_uses").ok().map(|v| v as u32),
                uses_count: d.get_i64("uses_count").unwrap_or(0) as u32,
                revoked_at_unix: d.get_i64("revoked_at_unix").ok(),
            });
        }
        commit_with_retry(&mut session).await?;
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
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, caller_role, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if !caller_role.can_manage_members() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can revoke invite links".into(),
            ));
        }
        let links_col: Collection<Document> = self.db.collection("public_invite_links");
        let existing = links_col
            .find_one(doc! { "link_id": link_id, "workspace_id": workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        if existing.is_none() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::NotFound("Invite link not found".into()));
        }
        links_col
            .update_one(
                doc! { "link_id": link_id },
                doc! { "$set": { "revoked_at_unix": now } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        self.record_audit_tx(
            &mut session,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.public_link_revoked",
            link_id,
            "Shareable invite link revoked",
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
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
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;

        let links_col: Collection<Document> = self.db.collection("public_invite_links");
        let ldoc = links_col
            .find_one(doc! { "code_hash": &code_hash })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Invite link not recognized".into()))?;
        if !ldoc
            .get("revoked_at_unix")
            .is_some_and(|b| b.as_null().is_some())
        {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationRevoked);
        }
        if ldoc.get_i64("expires_at_unix").unwrap_or(0) <= now {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationExpired);
        }
        let link_id = ldoc.get_str("link_id").unwrap_or("").to_owned();
        let max_uses = ldoc.get_i64("max_uses").ok().map(|v| v as u32);
        let uses_count = ldoc.get_i64("uses_count").unwrap_or(0) as u32;
        if let Some(max_u) = max_uses
            && uses_count >= max_u
        {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvitationAlreadyUsed);
        }
        let workspace_id = ldoc.get_str("workspace_id").unwrap_or("").to_owned();
        let role = parse_role(ldoc.get_str("role").unwrap_or("viewer"));

        links_col
            .update_one(
                doc! { "link_id": &link_id },
                doc! { "$inc": { "uses_count": 1i64 } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;

        let mem_col: Collection<Document> = self.db.collection("memberships");
        if mem_col
            .find_one(doc! { "workspace_id": &workspace_id, "user_id": &auth.user_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .is_none()
        {
            mem_col
                .insert_one(doc! {
                    "workspace_id": &workspace_id,
                    "user_id": &auth.user_id,
                    "role": role.as_str(),
                    "joined_at_unix": now,
                })
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
        }
        let fences: Collection<Document> = self.db.collection("workspace_permission_fences");
        fences
            .update_one(
                doc! { "workspace_id": &workspace_id },
                doc! { "$inc": { "membership_revision": 1i64 } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let ws_col: Collection<Document> = self.db.collection("workspaces");
        let ws = ws_col
            .find_one(doc! { "workspace_id": &workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Workspace not found".into()))?;
        let fence_doc = fences
            .find_one(doc! { "workspace_id": &workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mem_rev = fence_doc
            .and_then(|d| d.get_i64("membership_revision").ok())
            .unwrap_or(1);
        let users: Collection<Document> = self.db.collection("users");
        let created_at = users
            .find_one(doc! { "user_id": &auth.user_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .and_then(|u| u.get_i64("created_at_unix").ok())
            .unwrap_or(now);

        self.record_audit_tx(
            &mut session,
            Some(&workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.public_link_joined",
            &link_id,
            &format!("Joined via public invite link as {}", role.as_str()),
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
        let member_count = mem_col
            .count_documents(doc! { "workspace_id": &workspace_id })
            .await
            .unwrap_or(1) as usize;
        Ok(AcceptInvitationResponse {
            api_version: API_VERSION,
            workspace: TeamWorkspaceSummary {
                workspace_id,
                slug: ws.get_str("slug").unwrap_or("").to_owned(),
                name: ws.get_str("name").unwrap_or("").to_owned(),
                description: ws.get_str("description").unwrap_or("").to_owned(),
                role,
                member_count,
                current_revision: ws.get_i64("current_revision").unwrap_or(1),
                membership_revision: mem_rev,
                archived: ws.get_bool("archived").unwrap_or(false),
            },
            user: swaggen_api_contract::UserProfile {
                user_id: auth.user_id.clone(),
                email: auth.email.clone(),
                display_name: auth.display_name.clone(),
                is_server_owner: auth.is_server_owner,
                created_at_unix: created_at,
            },
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
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, caller_role, mem_rev) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if !caller_role.can_manage_members() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can change member roles".into(),
            ));
        }
        if mem_rev != req.expected_membership_revision {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Conflict(
                "Membership revision is stale; reload workspace members first".into(),
            ));
        }
        let mem_col: Collection<Document> = self.db.collection("memberships");
        let target_mem = mem_col
            .find_one(doc! { "workspace_id": workspace_id, "user_id": target_user_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Target member not in workspace".into()))?;
        if parse_role(target_mem.get_str("role").unwrap_or("viewer")) == Role::Owner {
            let _ = session.abort_transaction().await;
            return Err(StoreError::OwnerRequired(
                "Workspace Owner cannot be demoted without transferring ownership first".into(),
            ));
        }
        mem_col
            .update_one(
                doc! { "workspace_id": workspace_id, "user_id": target_user_id },
                doc! { "$set": { "role": req.role.as_str() } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let fences: Collection<Document> = self.db.collection("workspace_permission_fences");
        fences
            .update_one(
                doc! { "workspace_id": workspace_id, "membership_revision": mem_rev },
                doc! { "$inc": { "membership_revision": 1i64 } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let users: Collection<Document> = self.db.collection("users");
        let user = users
            .find_one(doc! { "user_id": target_user_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or(StoreError::InternalSafe)?;
        self.record_audit_tx(
            &mut session,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.role_updated",
            target_user_id,
            &format!("Role updated to {}", req.role.as_str()),
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
        Ok(MemberSummary {
            user_id: target_user_id.to_owned(),
            email: user.get_str("email_normalized").unwrap_or("").to_owned(),
            display_name: user.get_str("display_name").unwrap_or("").to_owned(),
            role: req.role,
            joined_at_unix: target_mem.get_i64("joined_at_unix").unwrap_or(0),
            membership_revision: mem_rev + 1,
        })
    }

    async fn list_entities(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<Vec<WorkspaceEntityRecord>, StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, _, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        let entities: Collection<Document> = self.db.collection("entities");
        let mut cursor = entities
            .find(doc! { "workspace_id": workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut out = Vec::new();
        while cursor
            .advance(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            let d = cursor
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            out.push(WorkspaceEntityRecord {
                entity_id: d.get_str("entity_id").unwrap_or("").to_owned(),
                workspace_id: workspace_id.to_owned(),
                parent_id: d.get_str("parent_id").ok().map(ToOwned::to_owned),
                kind: d.get_str("kind").unwrap_or("").to_owned(),
                name: d.get_str("name").unwrap_or("").to_owned(),
                order_key: d.get_i64("order_key").unwrap_or(0),
                payload_json: d.get_str("payload_json").unwrap_or("{}").to_owned(),
                revision: d.get_i64("revision").unwrap_or(1),
                deleted: d.get_bool("deleted").unwrap_or(false),
                updated_at_unix: d.get_i64("updated_at_unix").unwrap_or(0),
                updated_by: d.get_str("updated_by").unwrap_or("").to_owned(),
            });
        }
        drop(cursor);
        commit_with_retry(&mut session).await?;
        Ok(out)
    }

    async fn upsert_entity(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: UpsertEntityRequest,
        now: i64,
    ) -> Result<WorkspaceEntityRecord, StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, caller_role, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if !caller_role.can_write_entities() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Viewer role has read-only access in this workspace".into(),
            ));
        }
        let entities: Collection<Document> = self.db.collection("entities");
        let existing = entities
            .find_one(doc! { "workspace_id": workspace_id, "entity_id": &req.entity_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let next_rev = match existing {
            Some(ex) => {
                if ex.get_i64("revision").unwrap_or(0) != req.base_revision {
                    let _ = session.abort_transaction().await;
                    return Err(StoreError::Conflict(
                        "Entity revision mismatch; refresh before retrying".into(),
                    ));
                }
                req.base_revision + 1
            }
            None => {
                if req.base_revision != 0 {
                    let _ = session.abort_transaction().await;
                    return Err(StoreError::Conflict(
                        "New entity write must start from base_revision 0".into(),
                    ));
                }
                1
            }
        };
        if req.base_revision == 0 {
            entities
                .insert_one(doc! {
                    "workspace_id": workspace_id,
                    "entity_id": &req.entity_id,
                    "parent_id": req.parent_id.as_deref(),
                    "kind": &req.kind,
                    "name": &req.name,
                    "order_key": req.order_key,
                    "payload_json": &req.payload_json,
                    "revision": next_rev,
                    "deleted": false,
                    "updated_at_unix": now,
                    "updated_by": &auth.user_id,
                })
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
        } else {
            let res = entities
                .update_one(
                    doc! { "workspace_id": workspace_id, "entity_id": &req.entity_id, "revision": req.base_revision, "deleted": false },
                    doc! { "$set": {
                        "parent_id": req.parent_id.as_deref(),
                        "kind": &req.kind,
                        "name": &req.name,
                        "order_key": req.order_key,
                        "payload_json": &req.payload_json,
                        "revision": next_rev,
                        "updated_at_unix": now,
                        "updated_by": &auth.user_id,
                    }},
                )
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
            if res.matched_count != 1 {
                let _ = session.abort_transaction().await;
                return Err(StoreError::Conflict("Concurrent revision conflict".into()));
            }
        }
        self.record_audit_tx(
            &mut session,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.entity_upserted",
            &req.entity_id,
            &format!("{} rev {next_rev}", req.name),
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
        Ok(WorkspaceEntityRecord {
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
        })
    }

    async fn list_audit_events(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        limit: usize,
    ) -> Result<Vec<AuditEventRecord>, StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, caller_role, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if !caller_role.can_read_audit() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Only Owner or Admin can view workspace audit logs".into(),
            ));
        }
        let col: Collection<Document> = self.db.collection("audit_events");
        let mut cursor = col
            .find(doc! { "workspace_id": workspace_id })
            .with_options(
                FindOptions::builder()
                    .sort(doc! { "created_at_unix": -1 })
                    .limit(limit.clamp(1, 200) as i64)
                    .build(),
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut out = Vec::new();
        while cursor
            .advance(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            let d = cursor
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            out.push(AuditEventRecord {
                event_id: d.get_str("event_id").unwrap_or("").to_owned(),
                workspace_id: d.get_str("workspace_id").ok().map(ToOwned::to_owned),
                actor_user_id: d.get_str("actor_user_id").unwrap_or("").to_owned(),
                actor_email: d.get_str("actor_email").unwrap_or("").to_owned(),
                action: d.get_str("action").unwrap_or("").to_owned(),
                target_id: d.get_str("target_id").unwrap_or("").to_owned(),
                detail_redacted: d.get_str("detail_redacted").unwrap_or("").to_owned(),
                created_at_unix: d.get_i64("created_at_unix").unwrap_or(0),
            });
        }
        drop(cursor);
        commit_with_retry(&mut session).await?;
        Ok(out)
    }

    async fn publish_batch(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: PublishBatchRequest,
        now: i64,
    ) -> Result<PublishBatchResponse, StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;

        let (_, caller_role, mem_rev) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if !caller_role.can_write_entities() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Viewer role cannot publish workspace changes".into(),
            ));
        }
        if let Some(exp_rev) = req.expected_membership_revision
            && mem_rev != exp_rev
        {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Conflict(
                "Workspace membership changed; refresh permissions before publishing".into(),
            ));
        }

        let applied_col: Collection<Document> = self.db.collection("applied_operations");
        let entities_col: Collection<Document> = self.db.collection("entities");
        let rev_col: Collection<Document> = self.db.collection("revisions");
        let counters_col: Collection<Document> = self.db.collection("workspace_counters");
        let events_col: Collection<Document> = self.db.collection("change_events");
        let ws_col: Collection<Document> = self.db.collection("workspaces");
        let mem_col: Collection<Document> = self.db.collection("memberships");
        let notif_col: Collection<Document> = self.db.collection("notifications");

        if req.operations.is_empty() {
            commit_with_retry(&mut session).await?;
            return Ok(PublishBatchResponse {
                api_version: API_VERSION,
                results: Vec::new(),
            });
        }

        // Batch-prefetch applied_operations, entities, and workspace members in O(1) `$in` queries (ZERO N+1!).
        let op_ids: Vec<&str> = req
            .operations
            .iter()
            .map(|o| o.operation_id.as_str())
            .collect();
        let mut applied_map = std::collections::HashMap::new();
        let mut app_cur = applied_col
            .find(doc! {
                "workspace_id": workspace_id,
                "actor_id": &auth.user_id,
                "operation_id": { "$in": &op_ids }
            })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        while app_cur
            .advance(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            let d = app_cur
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            if let Ok(oid) = d.get_str("operation_id") {
                applied_map.insert(oid.to_owned(), d);
            }
        }
        drop(app_cur);

        let entity_ids: Vec<&str> = req
            .operations
            .iter()
            .map(|o| o.entity_id.as_str())
            .collect();
        let mut entity_map = std::collections::HashMap::new();
        let mut ent_cur = entities_col
            .find(doc! {
                "workspace_id": workspace_id,
                "entity_id": { "$in": &entity_ids }
            })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        while ent_cur
            .advance(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            let d = ent_cur
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            if let Ok(eid) = d.get_str("entity_id") {
                entity_map.insert(eid.to_owned(), d);
            }
        }
        drop(ent_cur);

        let mut mcursor = mem_col
            .find(doc! { "workspace_id": workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut others = Vec::new();
        while mcursor
            .advance(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            let md = mcursor
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            let uid = md.get_str("user_id").unwrap_or("").to_owned();
            if !uid.is_empty() && uid != auth.user_id {
                others.push(uid);
            }
        }
        drop(mcursor);

        let cnt_before = counters_col
            .find_one(doc! { "workspace_id": workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut running_cursor = cnt_before
            .and_then(|d| d.get_i64("cursor").ok())
            .unwrap_or(0);
        let mut applied_write_count = 0i64;

        let mut rev_docs = Vec::new();
        let mut event_docs = Vec::new();
        let mut notif_docs = Vec::new();
        let mut applied_docs = Vec::new();
        let mut results = Vec::with_capacity(req.operations.len());

        for op in req.operations {
            if let Some(prev) = applied_map.get(&op.operation_id) {
                let prev_hash = prev.get_str("request_hash").unwrap_or("");
                if prev_hash == op.request_hash {
                    let stored: PublishOperationResult = prev
                        .get_str("ack_json")
                        .ok()
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or(PublishOperationResult {
                            operation_id: op.operation_id,
                            entity_id: op.entity_id,
                            kind: PublishResultKind::AlreadyAcked,
                            server_revision: prev.get_i64("server_revision").unwrap_or(0),
                            server_cursor: prev.get_i64("server_cursor").ok(),
                            server_entity: None,
                            reason: "Already acknowledged".into(),
                        });
                    let mut ack = stored;
                    ack.kind = PublishResultKind::AlreadyAcked;
                    results.push(ack);
                } else {
                    results.push(PublishOperationResult {
                        operation_id: op.operation_id,
                        entity_id: op.entity_id,
                        kind: PublishResultKind::HashMismatch,
                        server_revision: 0,
                        server_cursor: None,
                        server_entity: None,
                        reason: "Operation ID reused with different request hash".into(),
                    });
                }
                continue;
            }

            let existing = entity_map.get(&op.entity_id).cloned();

            let to_entity = |d: &Document| WorkspaceEntityRecord {
                entity_id: d.get_str("entity_id").unwrap_or("").to_owned(),
                workspace_id: workspace_id.to_owned(),
                parent_id: d.get_str("parent_id").ok().map(ToOwned::to_owned),
                kind: d.get_str("kind").unwrap_or("").to_owned(),
                name: d.get_str("name").unwrap_or("").to_owned(),
                order_key: d.get_i64("order_key").unwrap_or(0),
                payload_json: d.get_str("payload_json").unwrap_or("{}").to_owned(),
                revision: d.get_i64("revision").unwrap_or(1),
                deleted: d.get_bool("deleted").unwrap_or(false),
                updated_at_unix: d.get_i64("updated_at_unix").unwrap_or(0),
                updated_by: d.get_str("updated_by").unwrap_or("").to_owned(),
            };

            let next_rev = match existing {
                Some(ref cur) if cur.get_bool("deleted").unwrap_or(false) => {
                    results.push(PublishOperationResult {
                        operation_id: op.operation_id,
                        entity_id: op.entity_id,
                        kind: PublishResultKind::ConflictDeletedRemote,
                        server_revision: cur.get_i64("revision").unwrap_or(0),
                        server_cursor: None,
                        server_entity: Some(to_entity(cur)),
                        reason: "Entity was deleted on server; tombstone prevents resurrection"
                            .into(),
                    });
                    continue;
                }
                Some(ref cur) if cur.get_i64("revision").unwrap_or(0) != op.base_revision => {
                    results.push(PublishOperationResult {
                        operation_id: op.operation_id,
                        entity_id: op.entity_id,
                        kind: PublishResultKind::ConflictRevision,
                        server_revision: cur.get_i64("revision").unwrap_or(0),
                        server_cursor: None,
                        server_entity: Some(to_entity(cur)),
                        reason: "Base revision mismatch".into(),
                    });
                    continue;
                }
                Some(cur) => cur.get_i64("revision").unwrap_or(0) + 1,
                None => {
                    if op.base_revision != 0 {
                        results.push(PublishOperationResult {
                            operation_id: op.operation_id,
                            entity_id: op.entity_id,
                            kind: PublishResultKind::ConflictRevision,
                            server_revision: 0,
                            server_cursor: None,
                            server_entity: None,
                            reason: "Entity missing for non-zero base revision".into(),
                        });
                        continue;
                    }
                    1
                }
            };

            let new_ent_doc = doc! {
                "workspace_id": workspace_id,
                "entity_id": &op.entity_id,
                "parent_id": op.parent_id.as_deref(),
                "kind": &op.kind,
                "name": &op.name,
                "order_key": op.order_key,
                "payload_json": &op.payload_json,
                "revision": next_rev,
                "deleted": op.delete,
                "updated_at_unix": now,
                "updated_by": &auth.user_id,
            };

            if op.base_revision == 0 {
                entities_col
                    .insert_one(new_ent_doc.clone())
                    .session(&mut session)
                    .await
                    .map_err(|e| map_mongo_err(&e))?;
            } else {
                let res = entities_col
                    .update_one(
                        doc! {
                            "workspace_id": workspace_id,
                            "entity_id": &op.entity_id,
                            "revision": op.base_revision,
                            "deleted": false,
                        },
                        doc! { "$set": {
                            "parent_id": op.parent_id.as_deref(),
                            "kind": &op.kind,
                            "name": &op.name,
                            "order_key": op.order_key,
                            "payload_json": &op.payload_json,
                            "revision": next_rev,
                            "deleted": op.delete,
                            "updated_at_unix": now,
                            "updated_by": &auth.user_id,
                        }},
                    )
                    .session(&mut session)
                    .await
                    .map_err(|e| map_mongo_err(&e))?;
                if res.matched_count != 1 {
                    let _ = session.abort_transaction().await;
                    return Err(StoreError::Conflict(
                        "Concurrent modification detected".into(),
                    ));
                }
            }
            entity_map.insert(op.entity_id.clone(), new_ent_doc);

            running_cursor += 1;
            applied_write_count += 1;
            let cursor = running_cursor;

            rev_docs.push(doc! {
                "workspace_id": workspace_id,
                "entity_id": &op.entity_id,
                "revision": next_rev,
                "deleted": op.delete,
                "name": &op.name,
                "payload_json": &op.payload_json,
                "actor_user_id": &auth.user_id,
                "actor_email": &auth.email,
                "note": &op.note,
                "created_at_unix": now,
            });

            event_docs.push(doc! {
                "cursor": cursor,
                "workspace_id": workspace_id,
                "entity_id": &op.entity_id,
                "entity_kind": &op.kind,
                "entity_name": &op.name,
                "parent_id": op.parent_id.as_deref(),
                "order_key": op.order_key,
                "revision": next_rev,
                "deleted": op.delete,
                "payload_json": if op.delete { bson::Bson::Null } else { bson::Bson::String(op.payload_json.clone()) },
                "operation_id": &op.operation_id,
                "actor_user_id": &auth.user_id,
                "actor_email": &auth.email,
                "note": &op.note,
                "created_at_unix": now,
            });

            for uid in &others {
                notif_docs.push(doc! {
                    "notification_id": Uuid::new_v4().to_string(),
                    "workspace_id": workspace_id,
                    "user_id": uid,
                    "kind": if op.delete { "entity_deleted" } else { "entity_published" },
                    "title": format!("{} {} ({})", auth.email, if op.delete { "deleted" } else { "published" }, op.name),
                    "entity_id": &op.entity_id,
                    "actor_email": &auth.email,
                    "created_at_unix": now,
                    "read": false,
                });
            }

            self.record_audit_tx(
                &mut session,
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
            )
            .await?;

            let srv_rec = WorkspaceEntityRecord {
                entity_id: op.entity_id.clone(),
                workspace_id: workspace_id.to_owned(),
                parent_id: op.parent_id,
                kind: op.kind,
                name: op.name,
                order_key: op.order_key,
                payload_json: op.payload_json,
                revision: next_rev,
                deleted: op.delete,
                updated_at_unix: now,
                updated_by: auth.user_id.clone(),
            };
            let res = PublishOperationResult {
                operation_id: op.operation_id.clone(),
                entity_id: op.entity_id,
                kind: PublishResultKind::Applied,
                server_revision: next_rev,
                server_cursor: Some(cursor),
                server_entity: Some(srv_rec),
                reason: "Published".into(),
            };
            let ack_json = serde_json::to_string(&res).unwrap_or_default();
            let app_doc = doc! {
                "workspace_id": workspace_id,
                "actor_id": &auth.user_id,
                "operation_id": &op.operation_id,
                "request_hash": &op.request_hash,
                "server_revision": next_rev,
                "server_cursor": cursor,
                "ack_json": ack_json,
                "committed_at_unix": now,
            };
            applied_map.insert(op.operation_id, app_doc.clone());
            applied_docs.push(app_doc);
            results.push(res);
        }

        if applied_write_count > 0 {
            counters_col
                .update_one(
                    doc! { "workspace_id": workspace_id },
                    doc! { "$inc": { "cursor": applied_write_count } },
                )
                .upsert(true)
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
            ws_col
                .update_one(
                    doc! { "workspace_id": workspace_id },
                    doc! { "$inc": { "current_revision": applied_write_count } },
                )
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
        }
        if !rev_docs.is_empty() {
            rev_col
                .insert_many(rev_docs)
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
        }
        if !event_docs.is_empty() {
            events_col
                .insert_many(event_docs)
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
        }
        if !notif_docs.is_empty() {
            notif_col
                .insert_many(notif_docs)
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
        }
        if !applied_docs.is_empty() {
            applied_col
                .insert_many(applied_docs)
                .session(&mut session)
                .await
                .map_err(|e| map_mongo_err(&e))?;
        }

        commit_with_retry(&mut session).await?;
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
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, _, mem_rev) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        let col: Collection<Document> = self.db.collection("change_events");
        let max_items = limit.clamp(1, 200);
        let mut cursor = col
            .find(doc! { "workspace_id": workspace_id, "cursor": { "$gt": after_cursor } })
            .with_options(
                FindOptions::builder()
                    .sort(doc! { "cursor": 1 })
                    .limit((max_items + 1) as i64)
                    .build(),
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut events = Vec::new();
        while cursor
            .advance(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            let d = cursor
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            events.push(ChangeEventRecord {
                cursor: d.get_i64("cursor").unwrap_or(0),
                workspace_id: workspace_id.to_owned(),
                entity_id: d.get_str("entity_id").unwrap_or("").to_owned(),
                entity_kind: d.get_str("entity_kind").unwrap_or("").to_owned(),
                entity_name: d.get_str("entity_name").unwrap_or("").to_owned(),
                parent_id: d.get_str("parent_id").ok().map(ToOwned::to_owned),
                order_key: d.get_i64("order_key").unwrap_or(0),
                revision: d.get_i64("revision").unwrap_or(1),
                deleted: d.get_bool("deleted").unwrap_or(false),
                payload_json: d.get_str("payload_json").ok().map(ToOwned::to_owned),
                operation_id: d.get_str("operation_id").unwrap_or("").to_owned(),
                actor_user_id: d.get_str("actor_user_id").unwrap_or("").to_owned(),
                actor_email: d.get_str("actor_email").unwrap_or("").to_owned(),
                note: d.get_str("note").unwrap_or("").to_owned(),
                created_at_unix: d.get_i64("created_at_unix").unwrap_or(0),
            });
        }
        drop(cursor);
        commit_with_retry(&mut session).await?;
        let has_more = events.len() > max_items;
        events.truncate(max_items);
        let next_cursor = events.last().map(|e| e.cursor).unwrap_or(after_cursor);
        Ok(ChangesFeedResponse {
            api_version: API_VERSION,
            workspace_id: workspace_id.to_owned(),
            after_cursor,
            next_cursor,
            has_more,
            membership_revision: mem_rev,
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
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, _, mem_rev) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        let counters: Collection<Document> = self.db.collection("workspace_counters");
        let snapshot_cursor = counters
            .find_one(doc! { "workspace_id": workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .and_then(|d| d.get_i64("cursor").ok())
            .unwrap_or(0);
        let col: Collection<Document> = self.db.collection("entities");
        let page_limit = limit.clamp(1, 500);
        let mut cur = col
            .find(doc! { "workspace_id": workspace_id })
            .with_options(
                FindOptions::builder()
                    .sort(doc! { "entity_id": 1 })
                    .skip(offset as u64)
                    .limit((page_limit + 1) as i64)
                    .build(),
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut entities = Vec::new();
        while cur
            .advance(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            let d = cur.deserialize_current().map_err(|e| map_mongo_err(&e))?;
            entities.push(WorkspaceEntityRecord {
                entity_id: d.get_str("entity_id").unwrap_or("").to_owned(),
                workspace_id: workspace_id.to_owned(),
                parent_id: d.get_str("parent_id").ok().map(ToOwned::to_owned),
                kind: d.get_str("kind").unwrap_or("").to_owned(),
                name: d.get_str("name").unwrap_or("").to_owned(),
                order_key: d.get_i64("order_key").unwrap_or(0),
                payload_json: d.get_str("payload_json").unwrap_or("{}").to_owned(),
                revision: d.get_i64("revision").unwrap_or(1),
                deleted: d.get_bool("deleted").unwrap_or(false),
                updated_at_unix: d.get_i64("updated_at_unix").unwrap_or(0),
                updated_by: d.get_str("updated_by").unwrap_or("").to_owned(),
            });
        }
        drop(cur);
        commit_with_retry(&mut session).await?;
        let has_more = entities.len() > page_limit;
        entities.truncate(page_limit);
        Ok(SnapshotResponse {
            api_version: API_VERSION,
            workspace_id: workspace_id.to_owned(),
            snapshot_cursor,
            snapshot_token: format!("snap-{workspace_id}-{snapshot_cursor}"),
            membership_revision: mem_rev,
            entities,
            next_offset: has_more.then_some(offset + page_limit),
            has_more,
        })
    }

    async fn list_entity_revisions(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        entity_id: &str,
    ) -> Result<Vec<EntityRevisionRecord>, StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, _, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        let col: Collection<Document> = self.db.collection("revisions");
        let mut cur = col
            .find(doc! { "workspace_id": workspace_id, "entity_id": entity_id })
            .with_options(FindOptions::builder().sort(doc! { "revision": -1 }).build())
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut out = Vec::new();
        while cur
            .advance(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            let d = cur.deserialize_current().map_err(|e| map_mongo_err(&e))?;
            out.push(EntityRevisionRecord {
                workspace_id: workspace_id.to_owned(),
                entity_id: entity_id.to_owned(),
                revision: d.get_i64("revision").unwrap_or(1),
                deleted: d.get_bool("deleted").unwrap_or(false),
                name: d.get_str("name").unwrap_or("").to_owned(),
                payload_json: d.get_str("payload_json").unwrap_or("{}").to_owned(),
                actor_user_id: d.get_str("actor_user_id").unwrap_or("").to_owned(),
                actor_email: d.get_str("actor_email").unwrap_or("").to_owned(),
                note: d.get_str("note").unwrap_or("").to_owned(),
                created_at_unix: d.get_i64("created_at_unix").unwrap_or(0),
            });
        }
        drop(cur);
        commit_with_retry(&mut session).await?;
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
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, _, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        let rev_col: Collection<Document> = self.db.collection("revisions");
        let target_doc = rev_col
            .find_one(doc! {
                "workspace_id": workspace_id,
                "entity_id": entity_id,
                "revision": req.target_revision
            })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Target revision not found".into()))?;
        let ent_col: Collection<Document> = self.db.collection("entities");
        let cur_doc = ent_col
            .find_one(doc! { "workspace_id": workspace_id, "entity_id": entity_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        commit_with_retry(&mut session).await?;

        let op = swaggen_api_contract::PublishOperation {
            operation_id: req.operation_id,
            request_hash: format!("restore-{entity_id}-r{}", req.target_revision),
            entity_id: entity_id.to_owned(),
            parent_id: cur_doc
                .as_ref()
                .and_then(|c| c.get_str("parent_id").ok().map(ToOwned::to_owned)),
            kind: cur_doc
                .as_ref()
                .and_then(|c| c.get_str("kind").ok().map(ToOwned::to_owned))
                .unwrap_or_else(|| "request".into()),
            name: target_doc.get_str("name").unwrap_or("").to_owned(),
            order_key: cur_doc
                .as_ref()
                .and_then(|c| c.get_i64("order_key").ok())
                .unwrap_or(0),
            payload_json: target_doc
                .get_str("payload_json")
                .unwrap_or("{}")
                .to_owned(),
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
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, caller_role, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if !caller_role.can_write_entities() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Viewer role cannot post comments".into(),
            ));
        }
        let col: Collection<Document> = self.db.collection("comments");
        if let Some(existing) = col
            .find_one(doc! { "workspace_id": workspace_id, "comment_id": &req.comment_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            commit_with_retry(&mut session).await?;
            return Ok(doc_to_comment_thread(&existing, workspace_id));
        }
        col.insert_one(doc! {
            "comment_id": &req.comment_id,
            "workspace_id": workspace_id,
            "entity_id": &req.entity_id,
            "anchor_revision": req.anchor_revision,
            "author_user_id": &auth.user_id,
            "author_email": &auth.email,
            "body": &req.body,
            "resolved": false,
            "resolved_by_email": bson::Bson::Null,
            "created_at_unix": now,
            "replies": bson::Bson::Array(Vec::new()),
        })
        .session(&mut session)
        .await
        .map_err(|e| map_mongo_err(&e))?;
        self.record_audit_tx(
            &mut session,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.comment_created",
            &req.entity_id,
            &format!("Comment at rev {}", req.anchor_revision),
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
        Ok(CommentThreadRecord {
            comment_id: req.comment_id,
            workspace_id: workspace_id.to_owned(),
            entity_id: req.entity_id,
            anchor_revision: req.anchor_revision,
            author_user_id: auth.user_id.clone(),
            author_email: auth.email.clone(),
            body: req.body,
            resolved: false,
            resolved_by_email: None,
            created_at_unix: now,
            replies: Vec::new(),
        })
    }

    async fn reply_comment(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        comment_id: &str,
        req: ReplyCommentRequest,
        now: i64,
    ) -> Result<CommentThreadRecord, StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, caller_role, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if !caller_role.can_write_entities() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Viewer role cannot reply to comments".into(),
            ));
        }
        let col: Collection<Document> = self.db.collection("comments");
        col.update_one(
            doc! {
                "workspace_id": workspace_id,
                "comment_id": comment_id,
                "replies.reply_id": { "$ne": &req.reply_id }
            },
            doc! { "$push": { "replies": {
                "reply_id": &req.reply_id,
                "author_user_id": &auth.user_id,
                "author_email": &auth.email,
                "body": &req.body,
                "created_at_unix": now,
            }}},
        )
        .session(&mut session)
        .await
        .map_err(|e| map_mongo_err(&e))?;
        let updated = col
            .find_one(doc! { "workspace_id": workspace_id, "comment_id": comment_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Comment thread not found".into()))?;
        commit_with_retry(&mut session).await?;
        Ok(doc_to_comment_thread(&updated, workspace_id))
    }

    async fn resolve_comment(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        comment_id: &str,
        req: ResolveCommentRequest,
        _now: i64,
    ) -> Result<CommentThreadRecord, StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, caller_role, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if !caller_role.can_write_entities() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Viewer role cannot resolve comments".into(),
            ));
        }
        let col: Collection<Document> = self.db.collection("comments");
        col.update_one(
            doc! { "workspace_id": workspace_id, "comment_id": comment_id },
            doc! { "$set": {
                "resolved": req.resolved,
                "resolved_by_email": if req.resolved { bson::Bson::String(auth.email.clone()) } else { bson::Bson::Null },
            }},
        )
        .session(&mut session)
        .await
        .map_err(|e| map_mongo_err(&e))?;
        let updated = col
            .find_one(doc! { "workspace_id": workspace_id, "comment_id": comment_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Comment thread not found".into()))?;
        commit_with_retry(&mut session).await?;
        Ok(doc_to_comment_thread(&updated, workspace_id))
    }

    async fn list_comments(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        entity_id: Option<&str>,
    ) -> Result<Vec<CommentThreadRecord>, StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, _, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        let mut filter = doc! { "workspace_id": workspace_id };
        if let Some(eid) = entity_id {
            filter.insert("entity_id", eid);
        }
        let col: Collection<Document> = self.db.collection("comments");
        let mut cur = col
            .find(filter)
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut out = Vec::new();
        while cur
            .advance(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            let d = cur.deserialize_current().map_err(|e| map_mongo_err(&e))?;
            let replies = d
                .get_array("replies")
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| item.as_document())
                        .map(|rd| CommentReplyRecord {
                            reply_id: rd.get_str("reply_id").unwrap_or("").to_owned(),
                            author_user_id: rd.get_str("author_user_id").unwrap_or("").to_owned(),
                            author_email: rd.get_str("author_email").unwrap_or("").to_owned(),
                            body: rd.get_str("body").unwrap_or("").to_owned(),
                            created_at_unix: rd.get_i64("created_at_unix").unwrap_or(0),
                        })
                        .collect()
                })
                .unwrap_or_default();
            out.push(CommentThreadRecord {
                comment_id: d.get_str("comment_id").unwrap_or("").to_owned(),
                workspace_id: workspace_id.to_owned(),
                entity_id: d.get_str("entity_id").unwrap_or("").to_owned(),
                anchor_revision: d.get_i64("anchor_revision").unwrap_or(1),
                author_user_id: d.get_str("author_user_id").unwrap_or("").to_owned(),
                author_email: d.get_str("author_email").unwrap_or("").to_owned(),
                body: d.get_str("body").unwrap_or("").to_owned(),
                resolved: d.get_bool("resolved").unwrap_or(false),
                resolved_by_email: d.get_str("resolved_by_email").ok().map(ToOwned::to_owned),
                created_at_unix: d.get_i64("created_at_unix").unwrap_or(0),
                replies,
            });
        }
        drop(cur);
        commit_with_retry(&mut session).await?;
        Ok(out)
    }

    async fn list_notifications(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
    ) -> Result<NotificationListResponse, StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, _, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        let col: Collection<Document> = self.db.collection("notifications");
        let mut cur = col
            .find(doc! { "workspace_id": workspace_id, "user_id": &auth.user_id })
            .with_options(
                FindOptions::builder()
                    .sort(doc! { "created_at_unix": -1 })
                    .build(),
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut list = Vec::new();
        while cur
            .advance(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
        {
            let d = cur.deserialize_current().map_err(|e| map_mongo_err(&e))?;
            list.push(NotificationRecord {
                notification_id: d.get_str("notification_id").unwrap_or("").to_owned(),
                workspace_id: workspace_id.to_owned(),
                user_id: auth.user_id.clone(),
                kind: d.get_str("kind").unwrap_or("").to_owned(),
                title: d.get_str("title").unwrap_or("").to_owned(),
                entity_id: d.get_str("entity_id").ok().map(ToOwned::to_owned),
                actor_email: d.get_str("actor_email").unwrap_or("").to_owned(),
                created_at_unix: d.get_i64("created_at_unix").unwrap_or(0),
                read: d.get_bool("read").unwrap_or(false),
            });
        }
        drop(cur);
        commit_with_retry(&mut session).await?;
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
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, _, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        let col: Collection<Document> = self.db.collection("notifications");
        let filter = if ids.is_empty() {
            doc! { "workspace_id": workspace_id, "user_id": &auth.user_id, "read": false }
        } else {
            doc! { "workspace_id": workspace_id, "user_id": &auth.user_id, "notification_id": { "$in": ids } }
        };
        let res = col
            .update_many(filter, doc! { "$set": { "read": true } })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        commit_with_retry(&mut session).await?;
        Ok(res.modified_count as usize)
    }

    async fn set_workspace_archived(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        archived: bool,
        now: i64,
    ) -> Result<TeamWorkspaceSummary, StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, caller_role, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if caller_role != Role::Owner {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Only workspace Owner can archive or restore workspace".into(),
            ));
        }
        let ws_col: Collection<Document> = self.db.collection("workspaces");
        ws_col
            .update_one(
                doc! { "workspace_id": workspace_id },
                doc! { "$set": { "archived": archived } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        self.record_audit_tx(
            &mut session,
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
        )
        .await?;
        let ws_doc = ws_col
            .find_one(doc! { "workspace_id": workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?
            .ok_or_else(|| StoreError::NotFound("Workspace not found".into()))?;
        let mem_col: Collection<Document> = self.db.collection("memberships");
        let member_count = mem_col
            .count_documents(doc! { "workspace_id": workspace_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))? as usize;
        commit_with_retry(&mut session).await?;
        Ok(TeamWorkspaceSummary {
            workspace_id: workspace_id.to_owned(),
            slug: ws_doc.get_str("slug").unwrap_or("").to_owned(),
            name: ws_doc.get_str("name").unwrap_or("").to_owned(),
            description: ws_doc.get_str("description").unwrap_or("").to_owned(),
            role: Role::Owner,
            member_count,
            current_revision: ws_doc.get_i64("current_revision").unwrap_or(0),
            membership_revision: ws_doc.get_i64("membership_revision").unwrap_or(0),
            archived,
        })
    }

    async fn transfer_workspace_ownership(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        req: swaggen_api_contract::TransferOwnershipRequest,
        now: i64,
    ) -> Result<MemberListResponse, StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, caller_role, mems_rev) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if caller_role != Role::Owner {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Forbidden(
                "Only current Owner can transfer ownership".into(),
            ));
        }
        if mems_rev != req.expected_membership_revision {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Conflict("Membership revision conflict".into()));
        }
        if req.target_user_id == auth.user_id {
            let _ = session.abort_transaction().await;
            return Err(StoreError::InvalidInput(
                "Target user is already Owner".into(),
            ));
        }
        let mem_col: Collection<Document> = self.db.collection("memberships");
        let target = mem_col
            .find_one(doc! { "workspace_id": workspace_id, "user_id": &req.target_user_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        if target.is_none() {
            let _ = session.abort_transaction().await;
            return Err(StoreError::NotFound("Target member not found".into()));
        }
        mem_col
            .update_one(
                doc! { "workspace_id": workspace_id, "user_id": &auth.user_id },
                doc! { "$set": { "role": Role::Admin.as_str() } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        mem_col
            .update_one(
                doc! { "workspace_id": workspace_id, "user_id": &req.target_user_id },
                doc! { "$set": { "role": Role::Owner.as_str() } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let ws_col: Collection<Document> = self.db.collection("workspaces");
        ws_col
            .update_one(
                doc! { "workspace_id": workspace_id },
                doc! { "$inc": { "membership_revision": 1_i64 } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let fences: Collection<Document> = self.db.collection("workspace_permission_fences");
        fences
            .update_one(
                doc! { "workspace_id": workspace_id },
                doc! { "$inc": { "membership_revision": 1_i64 } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        self.record_audit_tx(
            &mut session,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.ownership_transferred",
            &req.target_user_id,
            "Transferred workspace ownership atomically",
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
        self.list_members_and_invitations(auth, workspace_id).await
    }

    async fn leave_workspace(
        &self,
        auth: &AuthContext,
        workspace_id: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        let mut session = self
            .client
            .start_session()
            .await
            .map_err(|e| map_mongo_err(&e))?;
        session
            .start_transaction()
            .read_concern(ReadConcern::snapshot())
            .write_concern(WriteConcern::majority())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let (_, caller_role, _) = self
            .require_membership_tx(&mut session, workspace_id, &auth.user_id)
            .await?;
        if caller_role == Role::Owner {
            let _ = session.abort_transaction().await;
            return Err(StoreError::Conflict(
                "Workspace Owner cannot leave without transferring ownership first".into(),
            ));
        }
        let mem_col: Collection<Document> = self.db.collection("memberships");
        mem_col
            .delete_one(doc! { "workspace_id": workspace_id, "user_id": &auth.user_id })
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let ws_col: Collection<Document> = self.db.collection("workspaces");
        ws_col
            .update_one(
                doc! { "workspace_id": workspace_id },
                doc! { "$inc": { "membership_revision": 1_i64 } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let fences: Collection<Document> = self.db.collection("workspace_permission_fences");
        fences
            .update_one(
                doc! { "workspace_id": workspace_id },
                doc! { "$inc": { "membership_revision": 1_i64 } },
            )
            .session(&mut session)
            .await
            .map_err(|e| map_mongo_err(&e))?;
        self.record_audit_tx(
            &mut session,
            Some(workspace_id),
            &auth.user_id,
            &auth.email,
            "workspace.member_left",
            &auth.user_id,
            "Member left workspace",
            now,
        )
        .await?;
        commit_with_retry(&mut session).await?;
        Ok(())
    }

    async fn kv_get(&self, namespace: &str, key: &str) -> Result<Option<String>, StoreError> {
        let col: Collection<Document> = self.db.collection("server_kv_docs");
        let found = col
            .find_one(doc! { "namespace": namespace, "key": key })
            .await
            .map_err(|e| map_mongo_err(&e))?;
        Ok(found.and_then(|d| d.get_str("value_json").ok().map(str::to_owned)))
    }

    async fn kv_put(
        &self,
        namespace: &str,
        key: &str,
        value_json: &str,
        now: i64,
    ) -> Result<(), StoreError> {
        let col: Collection<Document> = self.db.collection("server_kv_docs");
        col.update_one(
            doc! { "namespace": namespace, "key": key },
            doc! {
                "$set": {
                    "namespace": namespace,
                    "key": key,
                    "value_json": value_json,
                    "updated_at_unix": now,
                }
            },
        )
        .upsert(true)
        .await
        .map_err(|e| map_mongo_err(&e))?;
        Ok(())
    }

    async fn kv_delete(&self, namespace: &str, key: &str) -> Result<bool, StoreError> {
        let col: Collection<Document> = self.db.collection("server_kv_docs");
        let res = col
            .delete_one(doc! { "namespace": namespace, "key": key })
            .await
            .map_err(|e| map_mongo_err(&e))?;
        Ok(res.deleted_count > 0)
    }

    async fn kv_list(
        &self,
        namespace: &str,
        key_prefix: &str,
    ) -> Result<Vec<(String, String)>, StoreError> {
        let col: Collection<Document> = self.db.collection("server_kv_docs");
        let filter = if let Some(upper) = prefix_upper_bound(key_prefix) {
            doc! {
                "namespace": namespace,
                "key": { "$gte": key_prefix, "$lt": upper }
            }
        } else {
            doc! { "namespace": namespace }
        };
        let mut cursor = col
            .find(filter)
            .with_options(FindOptions::builder().sort(doc! { "key": 1 }).build())
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut out = Vec::new();
        while cursor.advance().await.map_err(|e| map_mongo_err(&e))? {
            let d = cursor
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            if let (Ok(k), Ok(v)) = (d.get_str("key"), d.get_str("value_json"))
                && k.starts_with(key_prefix)
            {
                out.push((k.to_owned(), v.to_owned()));
            }
        }
        Ok(out)
    }

    async fn claim_idempotency(
        &self,
        scope: &str,
        key: &str,
        request_hash: &str,
        now: i64,
    ) -> Result<IdempotencyClaimOutcome, StoreError> {
        let col: Collection<Document> = self.db.collection("idempotency_records");
        let insert_res = col
            .insert_one(doc! {
                "scope": scope,
                "key": key,
                "request_hash": request_hash,
                "state": "in_progress",
                "created_at_unix": now,
            })
            .await;

        match insert_res {
            Ok(_) => Ok(IdempotencyClaimOutcome::Acquired),
            Err(err) if is_duplicate_key(&err) => {
                let Some(existing) = col
                    .find_one(doc! { "scope": scope, "key": key })
                    .await
                    .map_err(|e| map_mongo_err(&e))?
                else {
                    return Ok(IdempotencyClaimOutcome::Acquired);
                };
                let prev_hash = existing.get_str("request_hash").unwrap_or("");
                if prev_hash != request_hash {
                    return Err(StoreError::Conflict(
                        "Idempotency-Key cannot be reused with a different request payload".into(),
                    ));
                }
                if existing.get_str("state").unwrap_or("") == "completed" {
                    let code = existing.get_i32("status_code").unwrap_or(200) as u16;
                    let body = existing.get_str("body_json").unwrap_or("{}").to_owned();
                    return Ok(IdempotencyClaimOutcome::Replayed {
                        status_code: code,
                        body_json: body,
                    });
                }
                let created = existing.get_i64("created_at_unix").unwrap_or(now);
                if now - created < 30 {
                    return Err(StoreError::Conflict(
                        "Concurrent request with the same Idempotency-Key is still in progress"
                            .into(),
                    ));
                }
                col.update_one(
                    doc! { "scope": scope, "key": key },
                    doc! { "$set": { "created_at_unix": now, "state": "in_progress" } },
                )
                .await
                .map_err(|e| map_mongo_err(&e))?;
                Ok(IdempotencyClaimOutcome::Acquired)
            }
            Err(err) => Err(map_mongo_err(&err)),
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
        let col: Collection<Document> = self.db.collection("idempotency_records");
        col.update_one(
            doc! { "scope": scope, "key": key },
            doc! { "$set": {
                "state": "completed",
                "status_code": status_code as i32,
                "body_json": body_json,
                "completed_at_unix": now,
            }},
        )
        .await
        .map_err(|e| map_mongo_err(&e))?;
        Ok(())
    }

    async fn release_idempotency(&self, scope: &str, key: &str) -> Result<(), StoreError> {
        let col: Collection<Document> = self.db.collection("idempotency_records");
        col.delete_one(doc! { "scope": scope, "key": key, "state": "in_progress" })
            .await
            .map_err(|e| map_mongo_err(&e))?;
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
        let mut out = BatchedWorkspaceChildren::default();
        if workspace_ids.is_empty() {
            return Ok(out);
        }
        // 1. Verify membership across requested workspaces in ONE `$in` query.
        let mem_col: Collection<Document> = self.db.collection("memberships");
        out.db_queries_executed += 1;
        let mut allowed_cur = mem_col
            .find(doc! {
                "user_id": &auth.user_id,
                "workspace_id": { "$in": workspace_ids }
            })
            .await
            .map_err(|e| map_mongo_err(&e))?;
        let mut allowed_wids: Vec<String> = Vec::new();
        while allowed_cur.advance().await.map_err(|e| map_mongo_err(&e))? {
            let d = allowed_cur
                .deserialize_current()
                .map_err(|e| map_mongo_err(&e))?;
            if let Ok(wid) = d.get_str("workspace_id") {
                allowed_wids.push(wid.to_owned());
            }
        }
        if allowed_wids.is_empty() {
            return Ok(out);
        }

        // 2. Entities across ALL workspaces in ONE `$in` query!
        if include_entities {
            out.db_queries_executed += 1;
            let ent_col: Collection<Document> = self.db.collection("entities");
            let mut cur = ent_col
                .find(doc! {
                    "workspace_id": { "$in": &allowed_wids },
                    "deleted": false
                })
                .await
                .map_err(|e| map_mongo_err(&e))?;
            while cur.advance().await.map_err(|e| map_mongo_err(&e))? {
                let d = cur.deserialize_current().map_err(|e| map_mongo_err(&e))?;
                let wid = d.get_str("workspace_id").unwrap_or("").to_owned();
                out.entities_by_ws
                    .entry(wid.clone())
                    .or_default()
                    .push(WorkspaceEntityRecord {
                        entity_id: d.get_str("entity_id").unwrap_or("").to_owned(),
                        workspace_id: wid,
                        parent_id: d.get_str("parent_id").ok().map(ToOwned::to_owned),
                        kind: d.get_str("kind").unwrap_or("").to_owned(),
                        name: d.get_str("name").unwrap_or("").to_owned(),
                        order_key: d.get_i64("order_key").unwrap_or(0),
                        payload_json: d.get_str("payload_json").unwrap_or("{}").to_owned(),
                        revision: d.get_i64("revision").unwrap_or(1),
                        deleted: false,
                        updated_at_unix: d.get_i64("updated_at_unix").unwrap_or(0),
                        updated_by: d.get_str("updated_by").unwrap_or("").to_owned(),
                    });
            }
        }

        // 3. Members + Users across ALL workspaces in TWO `$in` queries!
        if include_members {
            out.db_queries_executed += 1;
            let mut mcur = mem_col
                .find(doc! { "workspace_id": { "$in": &allowed_wids } })
                .await
                .map_err(|e| map_mongo_err(&e))?;
            let mut raw_mems: Vec<(String, String, Role, i64)> = Vec::new();
            let mut uid_set = std::collections::HashSet::new();
            while mcur.advance().await.map_err(|e| map_mongo_err(&e))? {
                let d = mcur.deserialize_current().map_err(|e| map_mongo_err(&e))?;
                let wid = d.get_str("workspace_id").unwrap_or("").to_owned();
                let uid = d.get_str("user_id").unwrap_or("").to_owned();
                if !wid.is_empty() && !uid.is_empty() {
                    uid_set.insert(uid.clone());
                    raw_mems.push((
                        wid,
                        uid,
                        parse_role(d.get_str("role").unwrap_or("viewer")),
                        d.get_i64("joined_at_unix").unwrap_or(0),
                    ));
                }
            }
            let unique_uids: Vec<String> = uid_set.into_iter().collect();
            let mut user_map = std::collections::HashMap::with_capacity(unique_uids.len());
            if !unique_uids.is_empty() {
                out.db_queries_executed += 1;
                let users_col: Collection<Document> = self.db.collection("users");
                let mut ucur = users_col
                    .find(doc! { "user_id": { "$in": &unique_uids } })
                    .await
                    .map_err(|e| map_mongo_err(&e))?;
                while ucur.advance().await.map_err(|e| map_mongo_err(&e))? {
                    let u = ucur.deserialize_current().map_err(|e| map_mongo_err(&e))?;
                    if let Ok(uid) = u.get_str("user_id") {
                        user_map.insert(
                            uid.to_owned(),
                            (
                                u.get_str("email_normalized").unwrap_or("").to_owned(),
                                u.get_str("display_name").unwrap_or("").to_owned(),
                            ),
                        );
                    }
                }
            }
            for (wid, uid, role, joined_at_unix) in raw_mems {
                if let Some((email, display_name)) = user_map.get(&uid) {
                    out.members_by_ws
                        .entry(wid)
                        .or_default()
                        .push(MemberSummary {
                            user_id: uid,
                            email: email.clone(),
                            display_name: display_name.clone(),
                            role,
                            joined_at_unix,
                            membership_revision: 1,
                        });
                }
            }
        }

        // 4. Comments across ALL workspaces in ONE `$in` query!
        if include_comments {
            out.db_queries_executed += 1;
            let com_col: Collection<Document> = self.db.collection("comments");
            let mut ccur = com_col
                .find(doc! { "workspace_id": { "$in": &allowed_wids } })
                .await
                .map_err(|e| map_mongo_err(&e))?;
            while ccur.advance().await.map_err(|e| map_mongo_err(&e))? {
                let d = ccur.deserialize_current().map_err(|e| map_mongo_err(&e))?;
                let wid = d.get_str("workspace_id").unwrap_or("").to_owned();
                let rec = doc_to_comment_thread(&d, &wid);
                out.comments_by_ws.entry(wid).or_default().push(rec);
            }
        }

        // 5. Notifications across ALL workspaces in ONE `$in` query!
        if include_notifications {
            out.db_queries_executed += 1;
            let not_col: Collection<Document> = self.db.collection("notifications");
            let mut ncur = not_col
                .find(doc! {
                    "workspace_id": { "$in": &allowed_wids },
                    "user_id": &auth.user_id
                })
                .await
                .map_err(|e| map_mongo_err(&e))?;
            while ncur.advance().await.map_err(|e| map_mongo_err(&e))? {
                let d = ncur.deserialize_current().map_err(|e| map_mongo_err(&e))?;
                let wid = d.get_str("workspace_id").unwrap_or("").to_owned();
                out.notifications_by_ws
                    .entry(wid.clone())
                    .or_default()
                    .push(NotificationRecord {
                        notification_id: d.get_str("notification_id").unwrap_or("").to_owned(),
                        workspace_id: wid,
                        user_id: auth.user_id.clone(),
                        kind: d.get_str("kind").unwrap_or("").to_owned(),
                        title: d.get_str("title").unwrap_or("").to_owned(),
                        entity_id: d.get_str("entity_id").ok().map(ToOwned::to_owned),
                        actor_email: d.get_str("actor_email").unwrap_or("").to_owned(),
                        created_at_unix: d.get_i64("created_at_unix").unwrap_or(0),
                        read: d.get_bool("read").unwrap_or(false),
                    });
            }
        }
        Ok(out)
    }
}

fn doc_to_comment_thread(d: &Document, workspace_id: &str) -> CommentThreadRecord {
    let replies = d
        .get_array("replies")
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_document())
                .map(|rd| CommentReplyRecord {
                    reply_id: rd.get_str("reply_id").unwrap_or("").to_owned(),
                    author_user_id: rd.get_str("author_user_id").unwrap_or("").to_owned(),
                    author_email: rd.get_str("author_email").unwrap_or("").to_owned(),
                    body: rd.get_str("body").unwrap_or("").to_owned(),
                    created_at_unix: rd.get_i64("created_at_unix").unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default();
    CommentThreadRecord {
        comment_id: d.get_str("comment_id").unwrap_or("").to_owned(),
        workspace_id: workspace_id.to_owned(),
        entity_id: d.get_str("entity_id").unwrap_or("").to_owned(),
        anchor_revision: d.get_i64("anchor_revision").unwrap_or(1),
        author_user_id: d.get_str("author_user_id").unwrap_or("").to_owned(),
        author_email: d.get_str("author_email").unwrap_or("").to_owned(),
        body: d.get_str("body").unwrap_or("").to_owned(),
        resolved: d.get_bool("resolved").unwrap_or(false),
        resolved_by_email: d.get_str("resolved_by_email").ok().map(ToOwned::to_owned),
        created_at_unix: d.get_i64("created_at_unix").unwrap_or(0),
        replies,
    }
}

fn prefix_upper_bound(prefix: &str) -> Option<String> {
    if prefix.is_empty() {
        return None;
    }
    let mut chars: Vec<char> = prefix.chars().collect();
    while let Some(last) = chars.pop() {
        if let Some(next_char) = char::from_u32((last as u32).saturating_add(1))
            && next_char > last
        {
            chars.push(next_char);
            return Some(chars.into_iter().collect());
        }
    }
    None
}
