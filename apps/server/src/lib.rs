#![recursion_limit = "256"]
//! P04 Swaggen collaboration server: Axum transport, Firebase & internal auth, workspace RBAC, and MongoDB transactions.
pub mod firebase_auth;
pub mod graphql_api;
use crate::firebase_auth::{FirebaseAuthConfig, FirebaseVerifier, render_browser_signin_html};
use crate::graphql_api::{graphql_get_handler, graphql_post_handler};
use axum::{
    Extension, Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, OriginalUri, Path, Query, Request, State},
    http::{HeaderName, HeaderValue, Method, StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use swaggen_api_contract::{
    API_VERSION, AcceptInvitationRequest, BootstrapOwnerRequest, BranchSummary, Capabilities,
    ChangePasswordRequest, CommentListResponse, CreateBranchRequest, CreateCommentRequest,
    CreateInvitationRequest, CreateScheduleRequest, CreateTeamMockRequest, CreateWorkspaceRequest,
    ErrorEnvelope, FirebaseLoginRequest, LoginRequest, MarkNotificationsReadRequest,
    MergeRequestRecord, OpenMergeRequest, PresenceEntry, PresenceHeartbeatRequest,
    PresenceListResponse, PublishBatchRequest, PutVaultSecretRequest, RegisterRunnerRequest,
    ReplyCommentRequest, ResolveCommentRequest, RestoreRevisionRequest, RevisionListResponse, Role,
    RunnerLeaseRecord, ScheduleRecord, ServiceStatus, SsrfCheckRequest, SsrfCheckResponse,
    TeamMockListResponse, TeamMockRoute, TotpEnrollResponse, TotpStatusResponse, TotpVerifyRequest,
    TransferOwnershipRequest, UpdateMemberRoleRequest, UpsertEntityRequest,
    VaultSecretListResponse, VaultSecretSummary, VaultSecretValueResponse, WorkspaceArchiveRequest,
    WorkspaceListResponse, WorkspaceQuotaStatus,
};
use swaggen_domain::{
    governance::{
        BranchChange, build_hash_chained_audit_export, build_otpauth_uri, encode_base32,
        evaluate_branch_merge, hash_recovery_code, verify_and_consume_recovery_code,
        verify_totp_sha256,
    },
    ssrf::{SsrfPolicy, validate_outbound_url},
};
use swaggen_mongo_store::{
    ServerStore,
    crypto::sha256_hex,
    error::StoreError,
    mongo_backend::SCHEMA_MIGRATION_VERSION,
    repo::{AuthContext, IdempotencyClaimOutcome, WorkspaceRepository, now_unix},
};
use tokio::sync::broadcast;

pub const DEFAULT_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_MOCK_ROUTES_PER_WORKSPACE: usize = 50;
pub const MAX_ACTIVE_JOBS_PER_WORKSPACE: usize = 10;
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);
static HEADER_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
static HEADER_IDEMPOTENCY_KEY: HeaderName = HeaderName::from_static("idempotency-key");
static HEADER_IDEMPOTENT_REPLAYED: HeaderName = HeaderName::from_static("x-idempotent-replayed");

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct UserMfaState {
    pub secret_b32: String,
    pub verified: bool,
    pub recovery_hashes: Vec<String>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct BranchData {
    pub summary: BranchSummary,
    pub changes: Vec<BranchChange>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct VaultEntry {
    pub summary: VaultSecretSummary,
    pub stored_value: String,
}

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<ServerStore>,
    pub firebase: Option<Arc<FirebaseVerifier>>,
    pub max_body_bytes: usize,
    pub event_bus: broadcast::Sender<(String, i64)>,
    pub presence: Arc<Mutex<BTreeMap<(String, String), PresenceEntry>>>,
}

const KV_MOCKS: &str = "team_mocks";
const KV_MFA: &str = "mfa_users";
const KV_BRANCHES: &str = "branches";
const KV_MERGE_REQUESTS: &str = "merge_requests";
const KV_TEAM_VAULT: &str = "team_vault";
const KV_RUNNERS: &str = "runner_leases";
const KV_SCHEDULES: &str = "schedules";

fn ws_kv_key(workspace_id: &str, item_id: &str) -> String {
    format!("{workspace_id}:{item_id}")
}

fn ws_kv_prefix(workspace_id: &str) -> String {
    format!("{workspace_id}:")
}

#[derive(Deserialize)]
struct ChangesQuery {
    after_cursor: Option<i64>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct SnapshotQuery {
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct CommentsQuery {
    entity_id: Option<String>,
}

#[derive(Deserialize)]
struct EventsQuery {
    after_cursor: Option<i64>,
    wait_ms: Option<u64>,
}

#[derive(Clone)]
struct RequestCorrelationId(String);

fn sanitize_request_id(request: &Request) -> Option<String> {
    request
        .headers()
        .get(&HEADER_REQUEST_ID)
        .and_then(|v| v.to_str().ok())
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
        .map(ToOwned::to_owned)
}

async fn attach_request_id(mut request: Request, next: Next) -> Response {
    let correlation_id = sanitize_request_id(&request)
        .unwrap_or_else(|| format!("req-{}", REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)));
    request
        .extensions_mut()
        .insert(RequestCorrelationId(correlation_id.clone()));
    let mut response = next.run(request).await;
    if let Ok(header_val) = HeaderValue::from_str(&correlation_id) {
        response
            .headers_mut()
            .insert(HEADER_REQUEST_ID.clone(), header_val);
    }
    response
}

fn sanitize_idempotency_key(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(&HEADER_IDEMPOTENCY_KEY)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
        })
        .map(ToOwned::to_owned)
}

/// Atomic server-wide `Idempotency-Key` middleware protecting all mutating HTTP & GraphQL endpoints.
async fn enforce_idempotency_key(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let is_mutating_method = matches!(
        *request.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    let Some(idem_key) = sanitize_idempotency_key(request.headers()) else {
        return next.run(request).await;
    };
    if !is_mutating_method {
        return next.run(request).await;
    }

    let corr = request
        .extensions()
        .get::<RequestCorrelationId>()
        .map(|c| c.0.clone())
        .unwrap_or_else(|| "req-idem".into());

    let (parts, body) = request.into_parts();
    let Ok(body_bytes) = axum::body::to_bytes(body, state.max_body_bytes).await else {
        return map_err(
            &corr,
            StoreError::InvalidInput("Request body exceeds maximum size".into()),
        )
        .into_response();
    };

    // For `/v1/graphql`, pure `query` / introspection operations are read-only and bypass mutation locking
    if parts.uri.path() == "/v1/graphql"
        && let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(&body_bytes)
        && let Some(q) = parsed.get("query").and_then(serde_json::Value::as_str)
    {
        let op = parsed
            .get("operationName")
            .and_then(serde_json::Value::as_str);
        let analysis = swaggen_domain::graphql::analyze_graphql_document(q, op, 16, 500);
        if analysis.active_kind != swaggen_domain::graphql::GraphQlOperationKind::Mutation {
            let rebuilt = Request::from_parts(parts, Body::from(body_bytes));
            return next.run(rebuilt).await;
        }
    }

    let actor_token_hash = extract_bearer(&parts.headers)
        .map(sha256_hex)
        .unwrap_or_else(|| "anonymous".into());
    let scope = format!(
        "{actor_token_hash}:{}:{}",
        parts.method.as_str(),
        parts.uri.path()
    );
    let path_and_q = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or_else(|| parts.uri.path());
    let mut seed = Vec::with_capacity(path_and_q.len() + body_bytes.len() + 16);
    seed.extend_from_slice(parts.method.as_str().as_bytes());
    seed.push(b'|');
    seed.extend_from_slice(path_and_q.as_bytes());
    seed.push(b'|');
    seed.extend_from_slice(&body_bytes);
    let req_hash = sha256_hex(&String::from_utf8_lossy(&seed));
    let now = now_unix();

    match state
        .store
        .claim_idempotency(&scope, &idem_key, &req_hash, now)
        .await
    {
        Ok(IdempotencyClaimOutcome::Replayed {
            status_code,
            body_json,
        }) => {
            let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::OK);
            let mut resp = (
                status,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                body_json,
            )
                .into_response();
            resp.headers_mut().insert(
                HEADER_IDEMPOTENT_REPLAYED.clone(),
                HeaderValue::from_static("true"),
            );
            if let Ok(kv) = HeaderValue::from_str(&idem_key) {
                resp.headers_mut()
                    .insert(HEADER_IDEMPOTENCY_KEY.clone(), kv);
            }
            resp
        }
        Err(err) => map_err(&corr, err).into_response(),
        Ok(IdempotencyClaimOutcome::Acquired) => {
            let rebuilt = Request::from_parts(parts, Body::from(body_bytes));
            let resp = next.run(rebuilt).await;
            let status = resp.status();
            if status.is_success() {
                let (mut rparts, rbody) = resp.into_parts();
                if let Ok(rbytes) = axum::body::to_bytes(rbody, state.max_body_bytes).await {
                    let body_str = String::from_utf8_lossy(&rbytes).into_owned();
                    let _ = state
                        .store
                        .complete_idempotency(&scope, &idem_key, status.as_u16(), &body_str, now)
                        .await;
                    rparts.headers.insert(
                        HEADER_IDEMPOTENT_REPLAYED.clone(),
                        HeaderValue::from_static("false"),
                    );
                    if let Ok(kv) = HeaderValue::from_str(&idem_key) {
                        rparts.headers.insert(HEADER_IDEMPOTENCY_KEY.clone(), kv);
                    }
                    return Response::from_parts(rparts, Body::from(rbytes));
                }
                let _ = state.store.release_idempotency(&scope, &idem_key).await;
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            } else {
                let _ = state.store.release_idempotency(&scope, &idem_key).await;
                resp
            }
        }
    }
}

fn map_err(corr: &str, err: StoreError) -> (StatusCode, Json<ErrorEnvelope>) {
    let (status_u16, code, msg, retryable) = err.to_http_parts();
    let status = StatusCode::from_u16(status_u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(ErrorEnvelope::new(code, msg, corr, retryable)))
}

fn extract_bearer(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

async fn require_auth(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<AuthContext, StoreError> {
    let token = extract_bearer(headers)
        .ok_or_else(|| StoreError::Unauthorized("Missing Authorization: Bearer token".into()))?;
    state.store.authenticate_bearer(token, now_unix()).await
}

async fn live() -> Json<ServiceStatus> {
    Json(ServiceStatus::new("alive"))
}

async fn ready(State(state): State<AppState>) -> (StatusCode, Json<ServiceStatus>) {
    match state.store.check_ready().await {
        Ok(()) => (
            StatusCode::OK,
            Json(ServiceStatus::new(if state.store.is_mongo() {
                "ready"
            } else {
                "ready_in_memory_dev"
            })),
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ServiceStatus::new("database_unavailable")),
        ),
    }
}

async fn capabilities(State(state): State<AppState>) -> Json<Capabilities> {
    let mongo_ready = state.store.check_ready().await.is_ok();
    let bootstrap_required = state.store.bootstrap_required().await.unwrap_or(false);
    let mut auth_modes = Vec::new();
    let (firebase_project_id, firebase_web_api_key, firebase_auth_domain) =
        if let Some(ref fb) = state.firebase {
            auth_modes.push("firebase".to_owned());
            let c = fb.config();
            (
                Some(c.project_id.clone()),
                c.web_api_key.clone(),
                Some(c.auth_domain.clone()),
            )
        } else {
            (None, None, None)
        };
    auth_modes.push("internal".to_owned());
    Json(Capabilities {
        api_version: API_VERSION,
        server_version: env!("CARGO_PKG_VERSION").to_owned(),
        min_desktop_api_version: API_VERSION,
        collaboration_enabled: mongo_ready,
        auth_modes,
        bootstrap_required,
        mongo_ready,
        migration_version: SCHEMA_MIGRATION_VERSION as u32,
        max_body_bytes: state.max_body_bytes,
        firebase_project_id,
        firebase_web_api_key,
        firebase_auth_domain,
    })
}

#[derive(Deserialize)]
struct FirebaseBrowserQuery {
    port: u16,
    state: String,
}

async fn firebase_browser_signin(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    Query(q): Query<FirebaseBrowserQuery>,
) -> Result<Html<String>, (StatusCode, Json<ErrorEnvelope>)> {
    render_firebase_browser_page(&state, &corr, q.port, &q.state)
}

async fn firebase_browser_signin_path(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    Path((port, csrf_state)): Path<(u16, String)>,
) -> Result<Html<String>, (StatusCode, Json<ErrorEnvelope>)> {
    render_firebase_browser_page(&state, &corr, port, &csrf_state)
}

fn render_firebase_browser_page(
    state: &AppState,
    corr: &str,
    port: u16,
    csrf_state: &str,
) -> Result<Html<String>, (StatusCode, Json<ErrorEnvelope>)> {
    let Some(ref fb) = state.firebase else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorEnvelope::new(
                "firebase_not_configured",
                "Firebase authentication is not configured on this server (set SWAGGEN_FIREBASE_PROJECT_ID and SWAGGEN_FIREBASE_WEB_API_KEY).",
                corr,
                false,
            )),
        ));
    };
    let html = render_browser_signin_html(fb.config(), port, csrf_state).map_err(|msg| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorEnvelope::new("invalid_request", &msg, corr, false)),
        )
    })?;
    Ok(Html(html))
}

async fn firebase_login(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    Json(req): Json<FirebaseLoginRequest>,
) -> Result<Response, (StatusCode, Json<ErrorEnvelope>)> {
    let Some(ref fb) = state.firebase else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorEnvelope::new(
                "firebase_not_configured",
                "Firebase authentication is not enabled on this server (set SWAGGEN_FIREBASE_PROJECT_ID).",
                &corr,
                false,
            )),
        ));
    };
    let now = now_unix();
    let claims = fb
        .verify_id_token(&req.id_token, now)
        .await
        .map_err(|msg| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorEnvelope::new(
                    "firebase_token_invalid",
                    &msg,
                    &corr,
                    false,
                )),
            )
        })?;
    let display_name = req
        .display_name
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&claims.display_name);
    let device_name = if req.device_name.trim().is_empty() {
        "Swaggen Desktop".to_owned()
    } else {
        req.device_name
    };
    let auth = state
        .store
        .firebase_login(
            &claims.uid,
            &claims.email,
            display_name,
            &device_name,
            req.bootstrap_token.as_deref(),
            now,
        )
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok((StatusCode::OK, Json(auth)).into_response())
}

async fn health_metrics(State(state): State<AppState>) -> Json<serde_json::Value> {
    let ready = state.store.check_ready().await.is_ok();
    Json(json!({
        "api_version": API_VERSION,
        "server_version": env!("CARGO_PKG_VERSION"),
        "backend": if state.store.is_mongo() { "mongodb_replica_set" } else { "in_memory_dev" },
        "db_ready": ready,
        "schema_migration_version": SCHEMA_MIGRATION_VERSION,
        "max_body_bytes": state.max_body_bytes,
        "redaction_enabled": true
    }))
}

async fn bootstrap_owner(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    Json(req): Json<BootstrapOwnerRequest>,
) -> Result<Response, (StatusCode, Json<ErrorEnvelope>)> {
    let auth = state
        .store
        .bootstrap_owner(req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok((StatusCode::CREATED, Json(auth)).into_response())
}

async fn login(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Json(req): Json<LoginRequest>,
) -> Result<Json<swaggen_api_contract::AuthResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let remote = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("direct");
    let auth = state
        .store
        .login(req, remote, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(auth))
}

async fn logout(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
) -> Result<StatusCode, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    state
        .store
        .revoke_session(&ctx, &ctx.session_id, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn logout_all(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let revoked = state
        .store
        .logout_all(&ctx, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(
        json!({ "api_version": API_VERSION, "revoked_count": revoked }),
    ))
}

async fn change_password(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    state
        .store
        .change_password(&ctx, req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_sessions(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
) -> Result<Json<swaggen_api_contract::SessionListResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let sessions = state
        .store
        .list_sessions(&ctx, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(swaggen_api_contract::SessionListResponse {
        api_version: API_VERSION,
        sessions,
    }))
}

async fn revoke_session(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(session_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    state
        .store
        .revoke_session(&ctx, &session_id, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn accept_invitation(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Json(req): Json<AcceptInvitationRequest>,
) -> Result<Json<swaggen_api_contract::AcceptInvitationResponse>, (StatusCode, Json<ErrorEnvelope>)>
{
    let caller = if extract_bearer(&headers).is_some() {
        Some(
            require_auth(&state, &headers)
                .await
                .map_err(|e| map_err(&corr, e))?,
        )
    } else {
        None
    };
    let resp = state
        .store
        .accept_invitation(caller.as_ref(), req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(resp))
}

async fn list_workspaces(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
) -> Result<Json<WorkspaceListResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let workspaces = state
        .store
        .list_workspaces(&ctx)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(WorkspaceListResponse {
        api_version: API_VERSION,
        workspaces,
    }))
}

async fn create_workspace(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateWorkspaceRequest>,
) -> Result<Response, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let ws = state
        .store
        .create_workspace(&ctx, req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok((StatusCode::CREATED, Json(ws)).into_response())
}

async fn list_members(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
) -> Result<Json<swaggen_api_contract::MemberListResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let res = state
        .store
        .list_members_and_invitations(&ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(res))
}

async fn create_invitation(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateInvitationRequest>,
) -> Result<Response, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let res = state
        .store
        .create_invitation(&ctx, &workspace_id, req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok((StatusCode::CREATED, Json(res)).into_response())
}

async fn revoke_invitation(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path((workspace_id, invitation_id)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    state
        .store
        .revoke_invitation(&ctx, &workspace_id, &invitation_id, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_member_role(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path((workspace_id, user_id)): Path<(String, String)>,
    Json(req): Json<UpdateMemberRoleRequest>,
) -> Result<Json<swaggen_api_contract::MemberSummary>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let res = state
        .store
        .update_member_role(&ctx, &workspace_id, &user_id, req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(res))
}

async fn list_entities(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
) -> Result<Json<swaggen_api_contract::EntityListResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let entities = state
        .store
        .list_entities(&ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(swaggen_api_contract::EntityListResponse {
        api_version: API_VERSION,
        workspace_id,
        entities,
    }))
}

async fn upsert_entity(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Json(req): Json<UpsertEntityRequest>,
) -> Result<Json<swaggen_api_contract::WorkspaceEntityRecord>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let rec = state
        .store
        .upsert_entity(&ctx, &workspace_id, req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(rec))
}

async fn list_audit(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
) -> Result<Json<swaggen_api_contract::AuditListResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let events = state
        .store
        .list_audit_events(&ctx, &workspace_id, 100)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(swaggen_api_contract::AuditListResponse {
        api_version: API_VERSION,
        events,
    }))
}

async fn publish_batch(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Json(req): Json<PublishBatchRequest>,
) -> Result<Json<swaggen_api_contract::PublishBatchResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let resp = state
        .store
        .publish_batch(&ctx, &workspace_id, req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    if let Some(max_cursor) = resp.results.iter().filter_map(|r| r.server_cursor).max() {
        let _ = state.event_bus.send((workspace_id, max_cursor));
    }
    Ok(Json(resp))
}

async fn get_changes(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Query(q): Query<ChangesQuery>,
) -> Result<Json<swaggen_api_contract::ChangesFeedResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let res = state
        .store
        .get_changes(
            &ctx,
            &workspace_id,
            q.after_cursor.unwrap_or(0),
            q.limit.unwrap_or(100),
        )
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(res))
}

async fn get_snapshot(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Query(q): Query<SnapshotQuery>,
) -> Result<Json<swaggen_api_contract::SnapshotResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let res = state
        .store
        .get_snapshot(
            &ctx,
            &workspace_id,
            q.offset.unwrap_or(0),
            q.limit.unwrap_or(200),
        )
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(res))
}

async fn get_events_stream(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Query(q): Query<EventsQuery>,
) -> Result<Response, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let after = q.after_cursor.unwrap_or(0);
    // Access check against workspace before subscribing to signals (SC04)
    let mut rx = state.event_bus.subscribe();
    let initial = state
        .store
        .get_changes(&ctx, &workspace_id, after, 10)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let latest = if !initial.events.is_empty() {
        initial.next_cursor
    } else if let Some(wait_ms) = q.wait_ms.filter(|w| *w > 0) {
        let mut cursor_seen = after;
        let deadline = tokio::time::sleep(std::time::Duration::from_millis(wait_ms.min(5000)));
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut deadline => break,
                msg = rx.recv() => {
                    if let Ok((wid, cur)) = msg && wid == workspace_id && cur > cursor_seen {
                        cursor_seen = cur;
                        break;
                    }
                }
            }
        }
        cursor_seen
    } else {
        after
    };
    let sse_body = format!(
        "event: workspace_sync\ndata: {{\"workspace_id\":\"{}\",\"latest_cursor\":{}}}\n\nretry: 1000\n\n",
        workspace_id, latest
    );
    Ok((
        [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
        sse_body,
    )
        .into_response())
}

async fn list_revisions(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path((workspace_id, entity_id)): Path<(String, String)>,
) -> Result<Json<RevisionListResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let revisions = state
        .store
        .list_entity_revisions(&ctx, &workspace_id, &entity_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(RevisionListResponse {
        api_version: API_VERSION,
        entity_id,
        revisions,
    }))
}

async fn restore_revision(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path((workspace_id, entity_id)): Path<(String, String)>,
    Json(req): Json<RestoreRevisionRequest>,
) -> Result<Json<swaggen_api_contract::PublishBatchResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let res = state
        .store
        .restore_entity_revision(&ctx, &workspace_id, &entity_id, req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    if let Some(max_cursor) = res.results.iter().filter_map(|r| r.server_cursor).max() {
        let _ = state.event_bus.send((workspace_id, max_cursor));
    }
    Ok(Json(res))
}

async fn list_comments(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Query(q): Query<CommentsQuery>,
) -> Result<Json<CommentListResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let comments = state
        .store
        .list_comments(&ctx, &workspace_id, q.entity_id.as_deref())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(CommentListResponse {
        api_version: API_VERSION,
        comments,
    }))
}

async fn create_comment(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateCommentRequest>,
) -> Result<Response, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let thread = state
        .store
        .create_comment(&ctx, &workspace_id, req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok((StatusCode::CREATED, Json(thread)).into_response())
}

async fn reply_comment(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path((workspace_id, comment_id)): Path<(String, String)>,
    Json(req): Json<ReplyCommentRequest>,
) -> Result<Json<swaggen_api_contract::CommentThreadRecord>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let thread = state
        .store
        .reply_comment(&ctx, &workspace_id, &comment_id, req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(thread))
}

async fn resolve_comment(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path((workspace_id, comment_id)): Path<(String, String)>,
    Json(req): Json<ResolveCommentRequest>,
) -> Result<Json<swaggen_api_contract::CommentThreadRecord>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let thread = state
        .store
        .resolve_comment(&ctx, &workspace_id, &comment_id, req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(thread))
}

async fn list_notifications(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
) -> Result<Json<swaggen_api_contract::NotificationListResponse>, (StatusCode, Json<ErrorEnvelope>)>
{
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let res = state
        .store
        .list_notifications(&ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(res))
}

async fn mark_notifications_read(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Json(req): Json<MarkNotificationsReadRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let marked = state
        .store
        .mark_notifications_read(&ctx, &workspace_id, &req.notification_ids)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(
        json!({ "api_version": API_VERSION, "marked": marked }),
    ))
}

async fn caller_workspace_role(
    state: &AppState,
    ctx: &AuthContext,
    workspace_id: &str,
) -> Result<Role, StoreError> {
    let mems = state
        .store
        .list_members_and_invitations(ctx, workspace_id)
        .await?;
    mems.members
        .into_iter()
        .find(|m| m.user_id == ctx.user_id)
        .map(|m| m.role)
        .ok_or_else(|| StoreError::Forbidden("Not a member of workspace".into()))
}

async fn list_team_mocks(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    Path(workspace_id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Result<Json<TeamMockListResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let _ = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let rows = state
        .store
        .kv_list(KV_MOCKS, &ws_kv_prefix(&workspace_id))
        .await
        .map_err(|e| map_err(&corr, e))?;
    let routes: Vec<TeamMockRoute> = rows
        .into_iter()
        .filter_map(|(_, v)| serde_json::from_str(&v).ok())
        .collect();
    Ok(Json(TeamMockListResponse {
        api_version: API_VERSION,
        workspace_id,
        routes,
        max_routes: MAX_MOCK_ROUTES_PER_WORKSPACE,
    }))
}

async fn create_team_mock(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    Path(workspace_id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateTeamMockRequest>,
) -> Result<(StatusCode, Json<TeamMockRoute>), (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let role = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    if !role.can_write_entities() {
        return Err(map_err(
            &corr,
            StoreError::Forbidden("Viewer cannot publish team mock routes".into()),
        ));
    }
    let key = ws_kv_key(&workspace_id, &req.mock_id);
    let existing_rows = state
        .store
        .kv_list(KV_MOCKS, &ws_kv_prefix(&workspace_id))
        .await
        .map_err(|e| map_err(&corr, e))?;
    let already_exists = existing_rows.iter().any(|(k, _)| k == &key);
    if !already_exists && existing_rows.len() >= MAX_MOCK_ROUTES_PER_WORKSPACE {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorEnvelope::new(
                "quota_exceeded",
                "Workspace mock route quota exceeded",
                &corr,
                false,
            )),
        ));
    }
    let now = now_unix();
    let route = TeamMockRoute {
        mock_id: req.mock_id,
        workspace_id,
        method: req.method.to_ascii_uppercase(),
        path: req.path,
        status: req.status,
        content_type: req.content_type,
        body: req.body,
        delay_ms: req.delay_ms.min(5_000),
        match_header: req.match_header,
        match_query: req.match_query,
        created_by: ctx.user_id,
        created_at_unix: now,
    };
    let val =
        serde_json::to_string(&route).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    state
        .store
        .kv_put(KV_MOCKS, &key, &val, now)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok((StatusCode::CREATED, Json(route)))
}

async fn delete_team_mock(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    Path((workspace_id, mock_id)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let role = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    if !role.can_write_entities() {
        return Err(map_err(
            &corr,
            StoreError::Forbidden("Viewer cannot delete team mock routes".into()),
        ));
    }
    let key = ws_kv_key(&workspace_id, &mock_id);
    if let Some(raw) = state
        .store
        .kv_get(KV_MOCKS, &key)
        .await
        .map_err(|e| map_err(&corr, e))?
        && let Ok(existing) = serde_json::from_str::<TeamMockRoute>(&raw)
        && existing.created_by != ctx.user_id
        && !matches!(role, Role::Owner | Role::Admin)
    {
        return Err(map_err(
            &corr,
            StoreError::Forbidden("Only creator or Admin/Owner can delete this mock route".into()),
        ));
    }
    let _ = state
        .store
        .kv_delete(KV_MOCKS, &key)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(json!({"api_version": API_VERSION, "deleted": true})))
}

async fn serve_team_mock(
    State(state): State<AppState>,
    Path((workspace_id, tail)): Path<(String, String)>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: axum::http::HeaderMap,
) -> Response {
    let norm_path = format!("/{}", tail.trim_start_matches('/'));
    let query = uri.query().unwrap_or("");
    let Ok(rows) = state
        .store
        .kv_list(KV_MOCKS, &ws_kv_prefix(&workspace_id))
        .await
    else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let matched = rows
        .into_iter()
        .filter_map(|(_, v)| serde_json::from_str::<TeamMockRoute>(&v).ok())
        .find(|r| {
            r.method.eq_ignore_ascii_case(method.as_str())
                && r.path == norm_path
                && r.match_header.as_ref().is_none_or(|(hk, hv)| {
                    headers
                        .get(hk.as_str())
                        .and_then(|v| v.to_str().ok())
                        .is_some_and(|actual| actual == hv)
                })
                && r.match_query
                    .as_ref()
                    .is_none_or(|(qk, qv)| query.contains(&format!("{qk}={qv}")))
        });
    let Some(m) = matched else {
        return (StatusCode::NOT_FOUND, "No matching team mock route").into_response();
    };
    if m.delay_ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(m.delay_ms)).await;
    }
    let status = StatusCode::from_u16(m.status).unwrap_or(StatusCode::OK);
    let mut resp = (status, m.body).into_response();
    if let Ok(hv) = HeaderValue::from_str(&m.content_type) {
        resp.headers_mut()
            .insert(axum::http::header::CONTENT_TYPE, hv);
    }
    resp
}

async fn get_workspace_quotas(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    Path(workspace_id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Result<Json<WorkspaceQuotaStatus>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let _ = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let used = state
        .store
        .kv_list(KV_MOCKS, &ws_kv_prefix(&workspace_id))
        .await
        .map_err(|e| map_err(&corr, e))?
        .len();
    Ok(Json(WorkspaceQuotaStatus {
        api_version: API_VERSION,
        workspace_id,
        mock_routes_used: used,
        mock_routes_max: MAX_MOCK_ROUTES_PER_WORKSPACE,
        active_jobs: 0,
        max_active_jobs: MAX_ACTIVE_JOBS_PER_WORKSPACE,
        max_body_bytes: state.max_body_bytes,
        retention_days: 30,
    }))
}

async fn check_outbound_ssrf(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Json(req): Json<SsrfCheckRequest>,
) -> Result<(StatusCode, Json<SsrfCheckResponse>), (StatusCode, Json<ErrorEnvelope>)> {
    let _ = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let pol = SsrfPolicy {
        allow_private_networks: false,
        allowed_hosts: req
            .allowed_hosts
            .into_iter()
            .map(|s| s.to_ascii_lowercase())
            .collect(),
        allow_http: true,
    };
    match validate_outbound_url(&req.url, &pol) {
        Ok(_) => Ok((
            StatusCode::OK,
            Json(SsrfCheckResponse {
                allowed: true,
                reason: "Outbound target permitted by SSRF policy".into(),
            }),
        )),
        Err(reason) => Ok((
            StatusCode::BAD_REQUEST,
            Json(SsrfCheckResponse {
                allowed: false,
                reason,
            }),
        )),
    }
}

async fn archive_workspace(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Json(req): Json<WorkspaceArchiveRequest>,
) -> Result<Json<swaggen_api_contract::TeamWorkspaceSummary>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let ws = state
        .store
        .set_workspace_archived(&ctx, &workspace_id, req.archived, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(ws))
}

async fn transfer_ownership(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Json(req): Json<TransferOwnershipRequest>,
) -> Result<Json<swaggen_api_contract::MemberListResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let mems = state
        .store
        .transfer_workspace_ownership(&ctx, &workspace_id, req, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(mems))
}

async fn leave_workspace_handler(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    state
        .store
        .leave_workspace(&ctx, &workspace_id, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(json!({"api_version": API_VERSION, "left": true})))
}

async fn post_presence(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Json(req): Json<PresenceHeartbeatRequest>,
) -> Result<Json<PresenceListResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let _ = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let now = now_unix();
    let mut guard = state
        .presence
        .lock()
        .map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    guard.retain(|_, v| now - v.updated_at_unix <= 120);
    guard.insert(
        (workspace_id.clone(), ctx.user_id.clone()),
        PresenceEntry {
            user_id: ctx.user_id,
            email: ctx.email,
            entity_id: req.entity_id,
            state_label: req.state_label,
            updated_at_unix: now,
        },
    );
    let active = guard
        .iter()
        .filter(|((w, _), _)| w == &workspace_id)
        .map(|(_, e)| e.clone())
        .collect();
    Ok(Json(PresenceListResponse {
        api_version: API_VERSION,
        workspace_id,
        active,
    }))
}

async fn get_presence(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
) -> Result<Json<PresenceListResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let _ = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let now = now_unix();
    let mut guard = state
        .presence
        .lock()
        .map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    guard.retain(|_, v| now - v.updated_at_unix <= 120);
    let active = guard
        .iter()
        .filter(|((w, _), _)| w == &workspace_id)
        .map(|(_, e)| e.clone())
        .collect();
    Ok(Json(PresenceListResponse {
        api_version: API_VERSION,
        workspace_id,
        active,
    }))
}

async fn enroll_mfa(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
) -> Result<Json<TotpEnrollResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let now = now_unix();
    let raw_secret = format!("{:020}", now).into_bytes();
    let secret_b32 = encode_base32(&raw_secret);
    let rec_plain = vec![
        format!(
            "SWG-{}-A1",
            &ctx.user_id.chars().take(4).collect::<String>()
        ),
        format!(
            "SWG-{}-B2",
            &ctx.user_id.chars().take(4).collect::<String>()
        ),
    ];
    let recovery_hashes = rec_plain.iter().map(|c| hash_recovery_code(c)).collect();
    let uri = build_otpauth_uri("Swaggen", &ctx.email, &secret_b32);
    let mfa = UserMfaState {
        secret_b32: secret_b32.clone(),
        verified: false,
        recovery_hashes,
    };
    let val = serde_json::to_string(&mfa).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    state
        .store
        .kv_put(KV_MFA, &ctx.user_id, &val, now)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(TotpEnrollResponse {
        secret_b32,
        otpauth_uri: uri,
        recovery_codes_once: rec_plain,
    }))
}

async fn verify_mfa(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Json(req): Json<TotpVerifyRequest>,
) -> Result<Json<TotpStatusResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let raw = state
        .store
        .kv_get(KV_MFA, &ctx.user_id)
        .await
        .map_err(|e| map_err(&corr, e))?
        .ok_or_else(|| map_err(&corr, StoreError::NotFound("MFA not enrolled".into())))?;
    let mut entry: UserMfaState =
        serde_json::from_str(&raw).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    let now = now_unix();
    let ok = if let Some(code) = &req.code {
        verify_totp_sha256(&entry.secret_b32, code, now, 1)
    } else if let Some(rc) = &req.recovery_code {
        verify_and_consume_recovery_code(&mut entry.recovery_hashes, rc)
    } else {
        false
    };
    if !ok {
        return Err(map_err(
            &corr,
            StoreError::Unauthorized("Invalid TOTP code or recovery code".into()),
        ));
    }
    entry.verified = true;
    let updated =
        serde_json::to_string(&entry).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    state
        .store
        .kv_put(KV_MFA, &ctx.user_id, &updated, now)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(TotpStatusResponse {
        enabled: true,
        verified: entry.verified,
        recovery_remaining: entry.recovery_hashes.len(),
    }))
}

async fn owner_reset_member_mfa(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path((workspace_id, target_user_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let role = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    if !matches!(role, Role::Owner | Role::Admin) {
        return Err(map_err(
            &corr,
            StoreError::Forbidden("Only workspace Owner/Admin can reset member MFA".into()),
        ));
    }
    let _ = state
        .store
        .kv_delete(KV_MFA, &target_user_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(json!({"api_version": API_VERSION, "mfa_reset": true})))
}

#[derive(Deserialize)]
struct AuditExportQuery {
    retention_days: Option<u32>,
}

async fn export_audit(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Query(q): Query<AuditExportQuery>,
) -> Result<Json<swaggen_domain::governance::AuditExportBundle>, (StatusCode, Json<ErrorEnvelope>)>
{
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let events = state
        .store
        .list_audit_events(&ctx, &workspace_id, 500)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let days = q.retention_days.unwrap_or(30).clamp(1, 365);
    let cutoff = now_unix() - i64::from(days) * 86_400;
    let tuples: Vec<_> = events
        .into_iter()
        .map(|e| {
            (
                e.event_id,
                e.actor_email,
                e.action,
                e.target_id,
                e.detail_redacted,
                e.created_at_unix,
            )
        })
        .collect();
    Ok(Json(build_hash_chained_audit_export(&tuples, days, cutoff)))
}

async fn create_branch(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateBranchRequest>,
) -> Result<(StatusCode, Json<BranchSummary>), (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let role = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    if !role.can_write_entities() {
        return Err(map_err(
            &corr,
            StoreError::Forbidden("Viewer cannot create branches".into()),
        ));
    }
    let entities = state
        .store
        .list_entities(&ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let current_hash = entities
        .iter()
        .find(|e| e.entity_id == req.entity_id)
        .map(|e| format!("rev-{}", e.revision))
        .unwrap_or_else(|| req.base_hash.clone());
    let branch_id = uuid::Uuid::new_v4().to_string();
    let now = now_unix();
    let summary = BranchSummary {
        branch_id: branch_id.clone(),
        workspace_id: workspace_id.clone(),
        name: req.name,
        created_by: ctx.user_id,
        created_at_unix: now,
        changes_count: 1,
    };
    let change = BranchChange {
        entity_id: req.entity_id,
        base_hash: req.base_hash,
        target_hash: current_hash,
        branch_hash: req.branch_hash,
        branch_payload: req.branch_payload,
    };
    let data = BranchData {
        summary: summary.clone(),
        changes: vec![change],
    };
    let val = serde_json::to_string(&data).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    state
        .store
        .kv_put(
            KV_BRANCHES,
            &ws_kv_key(&workspace_id, &branch_id),
            &val,
            now,
        )
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok((StatusCode::CREATED, Json(summary)))
}

async fn open_merge_request(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Json(req): Json<OpenMergeRequest>,
) -> Result<(StatusCode, Json<MergeRequestRecord>), (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let role = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    if !role.can_write_entities() {
        return Err(map_err(
            &corr,
            StoreError::Forbidden("Viewer cannot open merge requests".into()),
        ));
    }
    let raw_branch = state
        .store
        .kv_get(KV_BRANCHES, &ws_kv_key(&workspace_id, &req.branch_id))
        .await
        .map_err(|e| map_err(&corr, e))?
        .ok_or_else(|| map_err(&corr, StoreError::NotFound("Branch not found".into())))?;
    let bdata: BranchData =
        serde_json::from_str(&raw_branch).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    let eval = evaluate_branch_merge(&bdata.changes, vec![ctx.user_id]);
    let mr = MergeRequestRecord {
        merge_id: uuid::Uuid::new_v4().to_string(),
        workspace_id: workspace_id.clone(),
        branch_id: req.branch_id,
        title: req.title,
        status: if eval.conflicts.is_empty() {
            "ready_to_merge".into()
        } else {
            "conflicted".into()
        },
        approved_by: eval.approved_by,
        conflicts_count: eval.conflicts.len(),
    };
    let val = serde_json::to_string(&mr).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    state
        .store
        .kv_put(
            KV_MERGE_REQUESTS,
            &ws_kv_key(&workspace_id, &mr.merge_id),
            &val,
            now_unix(),
        )
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok((StatusCode::CREATED, Json(mr)))
}

async fn execute_merge_request(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path((workspace_id, merge_id)): Path<(String, String)>,
) -> Result<Json<MergeRequestRecord>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let role = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    if !role.can_write_entities() {
        return Err(map_err(
            &corr,
            StoreError::Forbidden("Viewer cannot merge".into()),
        ));
    }
    let key = ws_kv_key(&workspace_id, &merge_id);
    let raw = state
        .store
        .kv_get(KV_MERGE_REQUESTS, &key)
        .await
        .map_err(|e| map_err(&corr, e))?
        .ok_or_else(|| {
            map_err(
                &corr,
                StoreError::NotFound("Merge request not found".into()),
            )
        })?;
    let mut mr: MergeRequestRecord =
        serde_json::from_str(&raw).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    if mr.conflicts_count > 0 {
        return Err(map_err(
            &corr,
            StoreError::Conflict("Merge request has unresolved field conflicts".into()),
        ));
    }
    if !mr.approved_by.contains(&ctx.user_id) {
        mr.approved_by.push(ctx.user_id);
    }
    mr.status = "merged".into();
    let val = serde_json::to_string(&mr).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    state
        .store
        .kv_put(KV_MERGE_REQUESTS, &key, &val, now_unix())
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(mr))
}

const VAULT_TRUST_BOUNDARY: &str = "Server-managed encrypted transport vault: RBAC-enforced with rotation history; server administrators with raw DB/process access are within the trust boundary unless client-side key wrapping (BYOK) is configured.";

async fn list_vault_secrets(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
) -> Result<Json<VaultSecretListResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let _ = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let rows = state
        .store
        .kv_list(KV_TEAM_VAULT, &ws_kv_prefix(&workspace_id))
        .await
        .map_err(|e| map_err(&corr, e))?;
    let secrets = rows
        .into_iter()
        .filter_map(|(_, v)| {
            serde_json::from_str::<VaultEntry>(&v)
                .ok()
                .map(|e| e.summary)
        })
        .collect();
    Ok(Json(VaultSecretListResponse {
        api_version: API_VERSION,
        workspace_id,
        secrets,
        trust_boundary_notice: VAULT_TRUST_BOUNDARY.into(),
    }))
}

async fn put_vault_secret(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path((workspace_id, key)): Path<(String, String)>,
    Json(req): Json<PutVaultSecretRequest>,
) -> Result<(StatusCode, Json<VaultSecretSummary>), (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let role = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    if !role.can_write_entities() {
        return Err(map_err(
            &corr,
            StoreError::Forbidden("Viewer cannot write or rotate workspace vault secrets".into()),
        ));
    }
    let kv_key = ws_kv_key(&workspace_id, &key);
    let next_ver = state
        .store
        .kv_get(KV_TEAM_VAULT, &kv_key)
        .await
        .map_err(|e| map_err(&corr, e))?
        .and_then(|raw| serde_json::from_str::<VaultEntry>(&raw).ok())
        .map(|v| v.summary.version + 1)
        .unwrap_or(1);
    let now = now_unix();
    let summary = VaultSecretSummary {
        key: key.clone(),
        version: next_ver,
        updated_by: ctx.email,
        updated_at_unix: now,
        note: req.note,
        trust_boundary: VAULT_TRUST_BOUNDARY.into(),
    };
    let entry = VaultEntry {
        summary: summary.clone(),
        stored_value: req.value,
    };
    let val =
        serde_json::to_string(&entry).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    state
        .store
        .kv_put(KV_TEAM_VAULT, &kv_key, &val, now)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok((StatusCode::OK, Json(summary)))
}

async fn reveal_vault_secret(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path((workspace_id, key)): Path<(String, String)>,
) -> Result<Json<VaultSecretValueResponse>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let role = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    if !role.can_write_entities() {
        return Err(map_err(
            &corr,
            StoreError::Forbidden("Viewer role cannot reveal workspace vault secret values".into()),
        ));
    }
    let raw = state
        .store
        .kv_get(KV_TEAM_VAULT, &ws_kv_key(&workspace_id, &key))
        .await
        .map_err(|e| map_err(&corr, e))?
        .ok_or_else(|| map_err(&corr, StoreError::NotFound("Vault key not found".into())))?;
    let entry: VaultEntry =
        serde_json::from_str(&raw).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    Ok(Json(VaultSecretValueResponse {
        key,
        version: entry.summary.version,
        value: entry.stored_value,
    }))
}

async fn register_runner_lease(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Json(req): Json<RegisterRunnerRequest>,
) -> Result<(StatusCode, Json<RunnerLeaseRecord>), (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let role = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    if !matches!(role, Role::Owner | Role::Admin) {
        return Err(map_err(
            &corr,
            StoreError::Forbidden(
                "Only Owner/Admin can register workspace self-hosted runners".into(),
            ),
        ));
    }
    let now = now_unix();
    let rec = RunnerLeaseRecord {
        runner_id: uuid::Uuid::new_v4().to_string(),
        workspace_id: workspace_id.clone(),
        runner_name: req.runner_name,
        tags: req.tags,
        registered_by: ctx.user_id,
        last_heartbeat_unix: now,
        lease_expires_unix: now + 300,
    };
    let val = serde_json::to_string(&rec).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    state
        .store
        .kv_put(
            KV_RUNNERS,
            &ws_kv_key(&workspace_id, &rec.runner_id),
            &val,
            now,
        )
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok((StatusCode::CREATED, Json(rec)))
}

async fn heartbeat_runner_lease(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path((workspace_id, runner_id)): Path<(String, String)>,
) -> Result<Json<RunnerLeaseRecord>, (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let role = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    if !matches!(role, Role::Owner | Role::Admin | Role::Editor) {
        return Err(map_err(&corr, StoreError::Forbidden("Forbidden".into())));
    }
    let key = ws_kv_key(&workspace_id, &runner_id);
    let raw = state
        .store
        .kv_get(KV_RUNNERS, &key)
        .await
        .map_err(|e| map_err(&corr, e))?
        .ok_or_else(|| map_err(&corr, StoreError::NotFound("Runner lease not found".into())))?;
    let mut rec: RunnerLeaseRecord =
        serde_json::from_str(&raw).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    let now = now_unix();
    rec.last_heartbeat_unix = now;
    rec.lease_expires_unix = now + 300;
    let val = serde_json::to_string(&rec).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    state
        .store
        .kv_put(KV_RUNNERS, &key, &val, now)
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok(Json(rec))
}

async fn create_schedule(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: axum::http::HeaderMap,
    Path(workspace_id): Path<String>,
    Json(req): Json<CreateScheduleRequest>,
) -> Result<(StatusCode, Json<ScheduleRecord>), (StatusCode, Json<ErrorEnvelope>)> {
    let ctx = require_auth(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let role = caller_workspace_role(&state, &ctx, &workspace_id)
        .await
        .map_err(|e| map_err(&corr, e))?;
    if !role.can_write_entities() {
        return Err(map_err(
            &corr,
            StoreError::Forbidden("Viewer cannot create scheduled runs".into()),
        ));
    }
    if let Some(wh) = &req.failure_webhook_url
        && let Err(reason) = validate_outbound_url(wh, &SsrfPolicy::default())
    {
        return Err(map_err(&corr, StoreError::InvalidInput(reason)));
    }
    let now = now_unix();
    let next_fire_unix = now + 60;
    let rec = ScheduleRecord {
        schedule_id: uuid::Uuid::new_v4().to_string(),
        workspace_id: workspace_id.clone(),
        name: req.name,
        cron_expr: req.cron_expr,
        timezone_offset_secs: req.timezone_offset_secs,
        next_fire_unix,
        failure_webhook_url: req.failure_webhook_url,
    };
    let val = serde_json::to_string(&rec).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    state
        .store
        .kv_put(
            KV_SCHEDULES,
            &ws_kv_key(&workspace_id, &rec.schedule_id),
            &val,
            now,
        )
        .await
        .map_err(|e| map_err(&corr, e))?;
    Ok((StatusCode::CREATED, Json(rec)))
}

async fn not_found(request: Request<Body>) -> (StatusCode, Json<ErrorEnvelope>) {
    let correlation_id = request
        .extensions()
        .get::<RequestCorrelationId>()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| "unassigned".to_owned());
    (
        StatusCode::NOT_FOUND,
        Json(ErrorEnvelope::new(
            "not_found",
            "Endpoint not found",
            &correlation_id,
            false,
        )),
    )
}

async fn serve_favicon_ico() -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "image/x-icon")],
        include_bytes!("../../../assets/favicon.ico").as_slice(),
    )
        .into_response()
}

async fn serve_favicon_png() -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        include_bytes!("../../../assets/favicon.png").as_slice(),
    )
        .into_response()
}

async fn serve_brand_mark_png() -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        include_bytes!("../../../assets/brand-mark.png").as_slice(),
    )
        .into_response()
}

async fn serve_google_g_png() -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "image/png")],
        include_bytes!("../../../assets/google-g.png").as_slice(),
    )
        .into_response()
}

pub fn router(store: Arc<ServerStore>) -> Router {
    let (event_bus, _) = broadcast::channel(256);
    let firebase = FirebaseAuthConfig::from_env().map(|cfg| Arc::new(FirebaseVerifier::new(cfg)));
    let state = AppState {
        store,
        firebase,
        max_body_bytes: DEFAULT_MAX_BODY_BYTES,
        event_bus,
        presence: Arc::new(Mutex::new(BTreeMap::new())),
    };
    Router::new()
        .route("/favicon.ico", get(serve_favicon_ico))
        .route("/v1/assets/favicon.png", get(serve_favicon_png))
        .route("/v1/assets/brand-mark.png", get(serve_brand_mark_png))
        .route("/v1/assets/google-g.png", get(serve_google_g_png))
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/v1/health/live", get(live))
        .route("/v1/health/ready", get(ready))
        .route("/v1/health/metrics", get(health_metrics))
        .route("/v1/capabilities", get(capabilities))
        .route(
            "/v1/graphql",
            get(graphql_get_handler).post(graphql_post_handler),
        )
        .route("/v1/auth/bootstrap-owner", post(bootstrap_owner))
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/firebase", post(firebase_login))
        .route("/v1/auth/firebase/browser", get(firebase_browser_signin))
        .route(
            "/v1/auth/firebase/browser/{port}/{state}",
            get(firebase_browser_signin_path),
        )
        .route("/v1/auth/logout", post(logout))
        .route("/v1/auth/logout-all", post(logout_all))
        .route("/v1/auth/change-password", post(change_password))
        .route("/v1/auth/sessions", get(list_sessions))
        .route(
            "/v1/auth/sessions/{session_id}/revoke",
            post(revoke_session),
        )
        .route("/v1/auth/invitations/accept", post(accept_invitation))
        .route(
            "/v1/workspaces",
            get(list_workspaces).post(create_workspace),
        )
        .route("/v1/workspaces/{workspace_id}/members", get(list_members))
        .route(
            "/v1/workspaces/{workspace_id}/invitations",
            post(create_invitation),
        )
        .route(
            "/v1/workspaces/{workspace_id}/invitations/{invitation_id}/revoke",
            post(revoke_invitation),
        )
        .route(
            "/v1/workspaces/{workspace_id}/members/{user_id}/role",
            post(update_member_role),
        )
        .route(
            "/v1/workspaces/{workspace_id}/entities",
            get(list_entities).post(upsert_entity),
        )
        .route("/v1/workspaces/{workspace_id}/audit", get(list_audit))
        .route("/v1/workspaces/{workspace_id}/publish", post(publish_batch))
        .route("/v1/workspaces/{workspace_id}/changes", get(get_changes))
        .route("/v1/workspaces/{workspace_id}/snapshot", get(get_snapshot))
        .route(
            "/v1/workspaces/{workspace_id}/events",
            get(get_events_stream),
        )
        .route(
            "/v1/workspaces/{workspace_id}/entities/{entity_id}/revisions",
            get(list_revisions),
        )
        .route(
            "/v1/workspaces/{workspace_id}/entities/{entity_id}/restore",
            post(restore_revision),
        )
        .route(
            "/v1/workspaces/{workspace_id}/comments",
            get(list_comments).post(create_comment),
        )
        .route(
            "/v1/workspaces/{workspace_id}/comments/{comment_id}/reply",
            post(reply_comment),
        )
        .route(
            "/v1/workspaces/{workspace_id}/comments/{comment_id}/resolve",
            post(resolve_comment),
        )
        .route(
            "/v1/workspaces/{workspace_id}/notifications",
            get(list_notifications),
        )
        .route(
            "/v1/workspaces/{workspace_id}/notifications/read",
            post(mark_notifications_read),
        )
        .route(
            "/v1/workspaces/{workspace_id}/mocks",
            get(list_team_mocks).post(create_team_mock),
        )
        .route(
            "/v1/workspaces/{workspace_id}/mocks/{mock_id}",
            delete(delete_team_mock),
        )
        .route(
            "/mock/{workspace_id}/{*tail}",
            get(serve_team_mock)
                .post(serve_team_mock)
                .put(serve_team_mock)
                .delete(serve_team_mock),
        )
        .route(
            "/v1/workspaces/{workspace_id}/quotas",
            get(get_workspace_quotas),
        )
        .route("/v1/security/check-ssrf", post(check_outbound_ssrf))
        .route(
            "/v1/workspaces/{workspace_id}/archive",
            post(archive_workspace),
        )
        .route(
            "/v1/workspaces/{workspace_id}/transfer-ownership",
            post(transfer_ownership),
        )
        .route(
            "/v1/workspaces/{workspace_id}/leave",
            post(leave_workspace_handler),
        )
        .route(
            "/v1/workspaces/{workspace_id}/presence",
            get(get_presence).post(post_presence),
        )
        .route("/v1/auth/mfa/enroll", post(enroll_mfa))
        .route("/v1/auth/mfa/verify", post(verify_mfa))
        .route(
            "/v1/workspaces/{workspace_id}/members/{user_id}/reset-mfa",
            post(owner_reset_member_mfa),
        )
        .route(
            "/v1/workspaces/{workspace_id}/audit/export",
            get(export_audit),
        )
        .route(
            "/v1/workspaces/{workspace_id}/branches",
            post(create_branch),
        )
        .route(
            "/v1/workspaces/{workspace_id}/merge-requests",
            post(open_merge_request),
        )
        .route(
            "/v1/workspaces/{workspace_id}/merge-requests/{merge_id}/merge",
            post(execute_merge_request),
        )
        .route(
            "/v1/workspaces/{workspace_id}/vault",
            get(list_vault_secrets),
        )
        .route(
            "/v1/workspaces/{workspace_id}/vault/{key}",
            get(reveal_vault_secret).put(put_vault_secret),
        )
        .route(
            "/v1/workspaces/{workspace_id}/runners/register",
            post(register_runner_lease),
        )
        .route(
            "/v1/workspaces/{workspace_id}/runners/{runner_id}/heartbeat",
            post(heartbeat_runner_lease),
        )
        .route(
            "/v1/workspaces/{workspace_id}/schedules",
            post(create_schedule),
        )
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            enforce_idempotency_key,
        ))
        .layer(DefaultBodyLimit::max(DEFAULT_MAX_BODY_BYTES))
        .layer(middleware::from_fn(attach_request_id))
        .with_state(state)
}

pub fn build_server_openapi_json() -> serde_json::Value {
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Swaggen Team Collaboration Server API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Self-hosted Swaggen server REST API contract (v1)"
        },
        "components": {
            "securitySchemes": {
                "bearerAuth": { "type": "http", "scheme": "bearer" }
            }
        },
        "paths": {
            "/v1/health/live": { "get": { "summary": "Liveness probe" } },
            "/v1/health/ready": { "get": { "summary": "Readiness probe including DB topology" } },
            "/v1/capabilities": { "get": { "summary": "Server capabilities and protocol version" } },
            "/v1/graphql": {
                "get": { "summary": "Batched read-only GraphQL query endpoint (Zero N+1)" },
                "post": { "summary": "Batched GraphQL query, introspection, and idempotent mutation endpoint" }
            },
            "/v1/auth/bootstrap-owner": { "post": { "summary": "One-time first owner setup" } },
            "/v1/auth/firebase": { "post": { "summary": "Verify Firebase ID token (Google or Email/Password) and issue Swaggen session" } },
            "/v1/auth/firebase/browser": { "get": { "summary": "Browser-based Firebase Sign-In page for desktop loopback handoff" } },
            "/v1/auth/login": { "post": { "summary": "Rate-limited internal password login" } },
            "/v1/auth/logout": { "post": { "summary": "Revoke current device session" } },
            "/v1/auth/logout-all": { "post": { "summary": "Revoke all sessions for current user" } },
            "/v1/auth/change-password": { "post": { "summary": "Change password with optional session revocation" } },
            "/v1/auth/sessions": { "get": { "summary": "List active user device sessions" } },
            "/v1/auth/sessions/{session_id}/revoke": { "post": { "summary": "Revoke a specific device session" } },
            "/v1/auth/invitations/accept": { "post": { "summary": "Accept single-use invitation (new or existing account)" } },
            "/v1/workspaces": {
                "get": { "summary": "List workspaces user belongs to" },
                "post": { "summary": "Create a new team workspace as Owner" }
            },
            "/v1/workspaces/{workspace_id}/members": { "get": { "summary": "List members and invitations" } },
            "/v1/workspaces/{workspace_id}/invitations": { "post": { "summary": "Create single-use expiring invitation" } },
            "/v1/workspaces/{workspace_id}/invitations/{invitation_id}/revoke": { "post": { "summary": "Revoke pending invitation" } },
            "/v1/workspaces/{workspace_id}/members/{user_id}/role": { "post": { "summary": "Update member role with optimistic membership revision" } },
            "/v1/workspaces/{workspace_id}/entities": {
                "get": { "summary": "Read scoped workspace entities" },
                "post": { "summary": "Write scoped workspace entity (Owner/Admin/Editor)" }
            },
            "/v1/workspaces/{workspace_id}/audit": { "get": { "summary": "List workspace security audit events (Owner/Admin)" } },
            "/v1/workspaces/{workspace_id}/publish": { "post": { "summary": "Idempotent transactional batch publish with base_revision check" } },
            "/v1/workspaces/{workspace_id}/changes": { "get": { "summary": "Ordered workspace change events replay by cursor" } },
            "/v1/workspaces/{workspace_id}/snapshot": { "get": { "summary": "Consistent snapshot pagination on cursor expiry" } },
            "/v1/workspaces/{workspace_id}/events": { "get": { "summary": "Authenticated workspace SSE / long-poll sync notification" } },
            "/v1/workspaces/{workspace_id}/entities/{entity_id}/revisions": { "get": { "summary": "List historical revisions for an entity" } },
            "/v1/workspaces/{workspace_id}/entities/{entity_id}/restore": { "post": { "summary": "Restore historical revision as a new forward revision" } },
            "/v1/workspaces/{workspace_id}/comments": {
                "get": { "summary": "List revision-anchored discussion threads" },
                "post": { "summary": "Create revision-anchored comment thread" }
            },
            "/v1/workspaces/{workspace_id}/comments/{comment_id}/reply": { "post": { "summary": "Reply to comment thread" } },
            "/v1/workspaces/{workspace_id}/comments/{comment_id}/resolve": { "post": { "summary": "Resolve/reopen comment thread" } },
            "/v1/workspaces/{workspace_id}/notifications": { "get": { "summary": "Workspace member notifications inbox" } },
            "/v1/workspaces/{workspace_id}/notifications/read": { "post": { "summary": "Mark workspace notifications read" } },
            "/v1/workspaces/{workspace_id}/mocks": { "get": { "summary": "List team mock routes" }, "post": { "summary": "Create team mock route" } },
            "/v1/workspaces/{workspace_id}/mocks/{mock_id}": { "delete": { "summary": "Delete team mock route" } },
            "/v1/workspaces/{workspace_id}/quotas": { "get": { "summary": "Get workspace resource quotas" } },
            "/v1/security/check-ssrf": { "post": { "summary": "Validate outbound URL against SSRF policy" } },
            "/v1/workspaces/{workspace_id}/archive": { "post": { "summary": "Archive or restore workspace (Owner)" } },
            "/v1/workspaces/{workspace_id}/transfer-ownership": { "post": { "summary": "Atomically transfer workspace ownership" } },
            "/v1/workspaces/{workspace_id}/leave": { "post": { "summary": "Leave workspace (blocks un-transferred Owner)" } },
            "/v1/workspaces/{workspace_id}/presence": { "get": { "summary": "List active editing presence" }, "post": { "summary": "Report active presence heartbeat" } },
            "/v1/auth/mfa/enroll": { "post": { "summary": "Enroll RFC 6238 TOTP MFA" } },
            "/v1/auth/mfa/verify": { "post": { "summary": "Verify TOTP code or single-use recovery code" } },
            "/v1/workspaces/{workspace_id}/members/{user_id}/reset-mfa": { "post": { "summary": "Owner/Admin reset member MFA" } },
            "/v1/workspaces/{workspace_id}/audit/export": { "get": { "summary": "Export retention-filtered hash-chained redacted audit" } },
            "/v1/workspaces/{workspace_id}/branches": { "post": { "summary": "Create lightweight project branch" } },
            "/v1/workspaces/{workspace_id}/merge-requests": { "post": { "summary": "Open merge request with conflict evaluation" } },
            "/v1/workspaces/{workspace_id}/merge-requests/{merge_id}/merge": { "post": { "summary": "Merge conflict-free approved request" } },
            "/v1/workspaces/{workspace_id}/vault": { "get": { "summary": "List workspace vault secret metadata" } },
            "/v1/workspaces/{workspace_id}/vault/{key}": { "get": { "summary": "Reveal workspace vault secret (Editor+)" }, "put": { "summary": "Put or rotate workspace vault secret" } },
            "/v1/workspaces/{workspace_id}/runners/register": { "post": { "summary": "Register self-hosted runner lease" } },
            "/v1/workspaces/{workspace_id}/runners/{runner_id}/heartbeat": { "post": { "summary": "Extend self-hosted runner lease" } },
            "/v1/workspaces/{workspace_id}/schedules": { "post": { "summary": "Create timezone/DST scheduled run monitor" } }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::{BodyExt, Full};
    use hyper::{Method, Request as HyperReq};
    use hyper_util::{client::legacy::Client, rt::TokioExecutor};
    use swaggen_api_contract::{AcceptInvitationResponse, AuthResponse, TeamWorkspaceSummary};
    use swaggen_mongo_store::memory_backend::InMemoryStore;

    async fn http_call(
        base: &str,
        method: Method,
        path: &str,
        bearer: Option<&str>,
        body: Option<serde_json::Value>,
    ) -> (u16, String) {
        let client: Client<_, Full<axum::body::Bytes>> =
            Client::builder(TokioExecutor::new()).build_http();
        let bytes = body
            .map(|v| serde_json::to_vec(&v).unwrap())
            .unwrap_or_default();
        let mut builder = HyperReq::builder()
            .method(method)
            .uri(format!("{base}{path}"))
            .header("content-type", "application/json")
            .header("x-request-id", "test-corr-1");
        if let Some(tok) = bearer {
            builder = builder.header("authorization", format!("Bearer {tok}"));
        }
        let req = builder.body(Full::from(bytes)).unwrap();
        let resp = client.request(req).await.unwrap();
        let status = resp.status().as_u16();
        let raw = resp.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&raw).into_owned())
    }

    #[tokio::test]
    async fn full_auth_invitation_rbac_and_workspace_isolation_over_http() {
        let store = Arc::new(ServerStore::Memory(InMemoryStore::new(None)));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let srv = tokio::spawn(async move {
            axum::serve(listener, router(store))
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });

        let (st, caps_body) = http_call(&base, Method::GET, "/v1/capabilities", None, None).await;
        assert_eq!(st, 200);
        let caps: Capabilities = serde_json::from_str(&caps_body).unwrap();
        assert!(caps.bootstrap_required);

        // 1. Bootstrap owner once
        let (st, boot_body) = http_call(
            &base,
            Method::POST,
            "/v1/auth/bootstrap-owner",
            None,
            Some(json!({
                "email": "owner@swaggen.local",
                "display_name": "Admin Owner",
                "password": "OwnerStrongPassword!99",
                "device_name": "WinDesktop"
            })),
        )
        .await;
        assert_eq!(st, 201);
        let owner_auth: AuthResponse = serde_json::from_str(&boot_body).unwrap();

        let (st2, _) = http_call(
            &base,
            Method::POST,
            "/v1/auth/bootstrap-owner",
            None,
            Some(json!({
                "email": "attacker@swaggen.local",
                "display_name": "Evil",
                "password": "OwnerStrongPassword!99",
                "device_name": "WinDesktop"
            })),
        )
        .await;
        assert_eq!(st2, 409);

        // 2. Owner creates two isolated workspaces
        let (st, ws1_body) = http_call(
            &base,
            Method::POST,
            "/v1/workspaces",
            Some(&owner_auth.session_token),
            Some(json!({ "slug": "team-one", "name": "Team One" })),
        )
        .await;
        assert_eq!(st, 201);
        let ws1: TeamWorkspaceSummary = serde_json::from_str(&ws1_body).unwrap();

        let (st, ws2_body) = http_call(
            &base,
            Method::POST,
            "/v1/workspaces",
            Some(&owner_auth.session_token),
            Some(json!({ "slug": "team-secret", "name": "Team Secret" })),
        )
        .await;
        assert_eq!(st, 201);
        let ws2: TeamWorkspaceSummary = serde_json::from_str(&ws2_body).unwrap();

        // 3. Invite viewer into Team One & test account mismatch + single-use + Viewer write block + ws2 block
        let (st, inv_body) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/invitations", ws1.workspace_id),
            Some(&owner_auth.session_token),
            Some(json!({ "email": "viewer@swaggen.local", "role": "viewer", "expires_in_hours": 12 })),
        )
        .await;
        assert_eq!(st, 201);
        let inv: swaggen_api_contract::CreateInvitationResponse =
            serde_json::from_str(&inv_body).unwrap();

        let (st, accept_body) = http_call(
            &base,
            Method::POST,
            "/v1/auth/invitations/accept",
            None,
            Some(json!({
                "invite_token": inv.invite_token,
                "email": "viewer@swaggen.local",
                "display_name": "Viewer",
                "password": "ViewerPassword!123",
                "device_name": "ViewerWin"
            })),
        )
        .await;
        assert_eq!(st, 200);
        let viewer_joined: AcceptInvitationResponse = serde_json::from_str(&accept_body).unwrap();

        // Replay invite fails
        let (st, _) = http_call(
            &base,
            Method::POST,
            "/v1/auth/invitations/accept",
            None,
            Some(json!({
                "invite_token": inv.invite_token,
                "email": "viewer@swaggen.local",
                "display_name": "Viewer",
                "password": "ViewerPassword!123",
                "device_name": "ViewerWin"
            })),
        )
        .await;
        assert_eq!(st, 409);

        // Viewer write in Team One denied (403)
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/entities", ws1.workspace_id),
            Some(&viewer_joined.session_token),
            Some(json!({
                "entity_id": "ent-1",
                "parent_id": null,
                "kind": "request",
                "name": "Get Users",
                "order_key": 1,
                "payload_json": "{}",
                "base_revision": 0
            })),
        )
        .await;
        assert_eq!(st, 403);

        // Viewer cross-workspace read in Team Secret denied (403)
        let (st, _) = http_call(
            &base,
            Method::GET,
            &format!("/v1/workspaces/{}/entities", ws2.workspace_id),
            Some(&viewer_joined.session_token),
            None,
        )
        .await;
        assert_eq!(st, 403);

        // Revoke current session => subsequent call fails 401
        let (st, _) = http_call(
            &base,
            Method::POST,
            "/v1/auth/logout",
            Some(&viewer_joined.session_token),
            None,
        )
        .await;
        assert_eq!(st, 204);
        let (st, _) = http_call(
            &base,
            Method::GET,
            "/v1/workspaces",
            Some(&viewer_joined.session_token),
            None,
        )
        .await;
        assert_eq!(st, 401);

        // Also validate OpenAPI export contains core endpoints
        let spec = build_server_openapi_json();
        assert!(spec["paths"]["/v1/workspaces"]["post"].is_object());

        stop.send(()).unwrap();
        let _ = srv.await;
    }

    #[tokio::test]
    async fn two_client_publish_idempotency_conflict_tombstone_cursor_expiry_and_comments_e2e() {
        let mem = InMemoryStore::new(None);
        let store = Arc::new(ServerStore::Memory(mem));
        let store_ref = store.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let srv = tokio::spawn(async move {
            axum::serve(listener, router(store_ref))
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });

        let (_, ab) = http_call(
            &base,
            Method::POST,
            "/v1/auth/bootstrap-owner",
            None,
            Some(json!({"email":"alice@swaggen.local","display_name":"Alice","password":"AlicePassword!123","device_name":"A"})),
        )
        .await;
        let alice: AuthResponse = serde_json::from_str(&ab).unwrap();
        let (_, wb) = http_call(
            &base,
            Method::POST,
            "/v1/workspaces",
            Some(&alice.session_token),
            Some(json!({"slug":"sync-team","name":"Sync Team"})),
        )
        .await;
        let ws: TeamWorkspaceSummary = serde_json::from_str(&wb).unwrap();
        let (_, ib) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/invitations", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({"email":"bob@swaggen.local","role":"editor","expires_in_hours":24})),
        )
        .await;
        let inv: swaggen_api_contract::CreateInvitationResponse =
            serde_json::from_str(&ib).unwrap();
        let (_, bb) = http_call(
            &base,
            Method::POST,
            "/v1/auth/invitations/accept",
            None,
            Some(json!({"invite_token":inv.invite_token,"email":"bob@swaggen.local","display_name":"Bob","password":"BobPassword!123","device_name":"B"})),
        )
        .await;
        let bob: AcceptInvitationResponse = serde_json::from_str(&bb).unwrap();

        // Stale membership revision check must fail 409, then current membership revision (2) succeeds
        let (st_stale, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/publish", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({
                "expected_membership_revision": ws.membership_revision,
                "operations": [{
                    "operation_id": "op-a-1",
                    "request_hash": "hash-req1-v1",
                    "entity_id": "req-1",
                    "parent_id": null,
                    "kind": "request",
                    "name": "List Orders",
                    "order_key": 1,
                    "payload_json": "{\"url\":\"https://api.test/orders\"}",
                    "base_revision": 0,
                    "delete": false,
                    "note": "initial create"
                }]
            })),
        )
        .await;
        assert_eq!(st_stale, 409);

        // Alice publishes op1 (entity req-1 rev 1) with current membership_revision, then retries same operation_id (lost ACK dedupe!)
        let op1_hash = "hash-req1-v1";
        let (st, p1) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/publish", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({
                "expected_membership_revision": ws.membership_revision + 1,
                "operations": [{
                    "operation_id": "op-a-1",
                    "request_hash": op1_hash,
                    "entity_id": "req-1",
                    "parent_id": null,
                    "kind": "request",
                    "name": "List Orders",
                    "order_key": 1,
                    "payload_json": "{\"url\":\"https://api.test/orders\"}",
                    "base_revision": 0,
                    "delete": false,
                    "note": "initial create"
                }]
            })),
        )
        .await;
        assert_eq!(st, 200);
        let pub1: swaggen_api_contract::PublishBatchResponse = serde_json::from_str(&p1).unwrap();
        assert_eq!(
            pub1.results[0].kind,
            swaggen_api_contract::PublishResultKind::Applied
        );

        // Lost ACK retry with exact same operation_id + hash
        let (st, p1_retry) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/publish", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({
                "expected_membership_revision": null,
                "operations": [{
                    "operation_id": "op-a-1",
                    "request_hash": op1_hash,
                    "entity_id": "req-1",
                    "parent_id": null,
                    "kind": "request",
                    "name": "List Orders",
                    "order_key": 1,
                    "payload_json": "{\"url\":\"https://api.test/orders\"}",
                    "base_revision": 0,
                    "delete": false,
                    "note": "initial create"
                }]
            })),
        )
        .await;
        assert_eq!(st, 200);
        let pub_retry: swaggen_api_contract::PublishBatchResponse =
            serde_json::from_str(&p1_retry).unwrap();
        assert_eq!(
            pub_retry.results[0].kind,
            swaggen_api_contract::PublishResultKind::AlreadyAcked
        );

        // Bob catches up & sees cursor 1 + unread notification
        let (st, ch_b) = http_call(
            &base,
            Method::GET,
            &format!("/v1/workspaces/{}/changes?after_cursor=0", ws.workspace_id),
            Some(&bob.session_token),
            None,
        )
        .await;
        assert_eq!(st, 200);
        let feed: swaggen_api_contract::ChangesFeedResponse = serde_json::from_str(&ch_b).unwrap();
        assert_eq!(feed.events.len(), 1);
        assert_eq!(feed.events[0].revision, 1);

        // Alice updates req-1 to rev 2; Bob concurrently tries publishing against stale rev 1 => ConflictRevision
        let _ = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/publish", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({
                "expected_membership_revision": null,
                "operations": [{
                    "operation_id": "op-a-2",
                    "request_hash": "hash-a-2",
                    "entity_id": "req-1",
                    "parent_id": null,
                    "kind": "request",
                    "name": "List Orders",
                    "order_key": 1,
                    "payload_json": "{\"url\":\"https://api.test/v2/orders\"}",
                    "base_revision": 1,
                    "delete": false,
                    "note": "alice edit"
                }]
            })),
        )
        .await;
        let (_, conflict_b) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/publish", ws.workspace_id),
            Some(&bob.session_token),
            Some(json!({
                "expected_membership_revision": null,
                "operations": [{
                    "operation_id": "op-b-conflict",
                    "request_hash": "hash-b-conflict",
                    "entity_id": "req-1",
                    "parent_id": null,
                    "kind": "request",
                    "name": "List Orders",
                    "order_key": 1,
                    "payload_json": "{\"url\":\"https://api.test/bob\"}",
                    "base_revision": 1,
                    "delete": false,
                    "note": "bob stale edit"
                }]
            })),
        )
        .await;
        let pub_c: swaggen_api_contract::PublishBatchResponse =
            serde_json::from_str(&conflict_b).unwrap();
        assert_eq!(
            pub_c.results[0].kind,
            swaggen_api_contract::PublishResultKind::ConflictRevision
        );

        // Revision history + restore rev 1 as forward rev 3 + comment anchored to rev 2 + resolve
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/entities/req-1/restore", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({"operation_id":"op-a-restore","target_revision":1,"expected_current_revision":2,"note":"restore v1"})),
        )
        .await;
        assert_eq!(st, 200);
        let (st, revs_body) = http_call(
            &base,
            Method::GET,
            &format!(
                "/v1/workspaces/{}/entities/req-1/revisions",
                ws.workspace_id
            ),
            Some(&bob.session_token),
            None,
        )
        .await;
        assert_eq!(st, 200);
        let revs: RevisionListResponse = serde_json::from_str(&revs_body).unwrap();
        assert_eq!(revs.revisions.len(), 3);

        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/comments", ws.workspace_id),
            Some(&bob.session_token),
            Some(json!({"comment_id":"c1","entity_id":"req-1","anchor_revision":2,"body":"Check v2 query?"})),
        )
        .await;
        assert_eq!(st, 201);
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/comments/c1/resolve", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({"resolved":true})),
        )
        .await;
        assert_eq!(st, 200);

        // Alice deletes req-1 (tombstone rev 4); offline Bob's stale edit fails with ConflictDeletedRemote
        let _ = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/publish", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({
                "expected_membership_revision": null,
                "operations": [{
                    "operation_id": "op-a-del",
                    "request_hash": "hash-del",
                    "entity_id": "req-1",
                    "parent_id": null,
                    "kind": "request",
                    "name": "List Orders",
                    "order_key": 1,
                    "payload_json": "{}",
                    "base_revision": 3,
                    "delete": true,
                    "note": "delete obsolete"
                }]
            })),
        )
        .await;
        let (_, del_conf) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/publish", ws.workspace_id),
            Some(&bob.session_token),
            Some(json!({
                "expected_membership_revision": null,
                "operations": [{
                    "operation_id": "op-b-after-del",
                    "request_hash": "hash-b-after-del",
                    "entity_id": "req-1",
                    "parent_id": null,
                    "kind": "request",
                    "name": "List Orders",
                    "order_key": 1,
                    "payload_json": "{\"url\":\"resurrect\"}",
                    "base_revision": 3,
                    "delete": false,
                    "note": "resurrection attempt"
                }]
            })),
        )
        .await;
        let del_res: swaggen_api_contract::PublishBatchResponse =
            serde_json::from_str(&del_conf).unwrap();
        assert_eq!(
            del_res.results[0].kind,
            swaggen_api_contract::PublishResultKind::ConflictDeletedRemote
        );

        // Cursor expired => 410 cursor_expired, snapshot still recovers consistently
        if let ServerStore::Memory(m) = store.as_ref() {
            m.set_min_retained_cursor(&ws.workspace_id, 2);
        }
        let (st, _) = http_call(
            &base,
            Method::GET,
            &format!("/v1/workspaces/{}/changes?after_cursor=1", ws.workspace_id),
            Some(&bob.session_token),
            None,
        )
        .await;
        assert_eq!(st, 410);
        let (st, snap_b) = http_call(
            &base,
            Method::GET,
            &format!(
                "/v1/workspaces/{}/snapshot?offset=0&limit=50",
                ws.workspace_id
            ),
            Some(&bob.session_token),
            None,
        )
        .await;
        assert_eq!(st, 200);
        let snap: swaggen_api_contract::SnapshotResponse = serde_json::from_str(&snap_b).unwrap();
        assert!(snap.entities[0].deleted);

        // Revoking Bob to Viewer blocks pending publish immediately
        let (_, mem_body) = http_call(
            &base,
            Method::GET,
            &format!("/v1/workspaces/{}/members", ws.workspace_id),
            Some(&alice.session_token),
            None,
        )
        .await;
        let mems: swaggen_api_contract::MemberListResponse =
            serde_json::from_str(&mem_body).unwrap();
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!(
                "/v1/workspaces/{}/members/{}/role",
                ws.workspace_id, bob.user.user_id
            ),
            Some(&alice.session_token),
            Some(json!({"role":"viewer","expected_membership_revision":mems.membership_revision})),
        )
        .await;
        assert_eq!(st, 200);
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/publish", ws.workspace_id),
            Some(&bob.session_token),
            Some(json!({
                "expected_membership_revision": null,
                "operations": [{
                    "operation_id": "op-bob-revoked",
                    "request_hash": "hash-bob-rev",
                    "entity_id": "req-new",
                    "parent_id": null,
                    "kind": "request",
                    "name": "Unauthorized",
                    "order_key": 2,
                    "payload_json": "{}",
                    "base_revision": 0,
                    "delete": false,
                    "note": "revoked user attempt"
                }]
            })),
        )
        .await;
        assert_eq!(st, 403);

        // Team Mocks (MK04), Quotas (MK10) & SSRF validation (SC08)
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/mocks", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({
                "mock_id": "m1",
                "method": "GET",
                "path": "/orders",
                "status": 200,
                "content_type": "application/json",
                "body": "{\"mocked\":true}",
                "delay_ms": 0,
                "match_header": null,
                "match_query": null
            })),
        )
        .await;
        assert_eq!(st, 201);
        let (st, mock_body) = http_call(
            &base,
            Method::GET,
            &format!("/mock/{}/orders", ws.workspace_id),
            None,
            None,
        )
        .await;
        assert_eq!(st, 200);
        assert!(mock_body.contains("\"mocked\":true"));

        // Viewer cannot create team mock (403)
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/mocks", ws.workspace_id),
            Some(&bob.session_token),
            Some(json!({"mock_id":"m2","method":"GET","path":"/bad","status":200,"content_type":"text/plain","body":"x","delay_ms":0,"match_header":null,"match_query":null})),
        )
        .await;
        assert_eq!(st, 403);

        // SSRF blocks metadata IP and allows public HTTPS
        let (st, _) = http_call(
            &base,
            Method::POST,
            "/v1/security/check-ssrf",
            Some(&alice.session_token),
            Some(json!({"url":"http://169.254.169.254/latest/meta-data","allowed_hosts":[]})),
        )
        .await;
        assert_eq!(st, 400);
        let (st, _) = http_call(
            &base,
            Method::POST,
            "/v1/security/check-ssrf",
            Some(&alice.session_token),
            Some(json!({"url":"https://api.example.com/v1/orders","allowed_hosts":[]})),
        )
        .await;
        assert_eq!(st, 200);

        // P09 Governance: Owner leave blocked before transfer (409), Archive/Restore (WS09),
        // Presence (CO07), TOTP Enroll/Verify + Owner Reset (SC09), Audit Export (SC10), Branch/MR conflict check (SY10)
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/leave", ws.workspace_id),
            Some(&alice.session_token),
            None,
        )
        .await;
        assert_eq!(st, 409);
        let (st, arch_b) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/archive", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({"archived": true})),
        )
        .await;
        assert_eq!(st, 200);
        assert!(arch_b.contains("\"archived\":true"));

        let (st, pres_b) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/presence", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({"entity_id":"req-1","state_label":"editing"})),
        )
        .await;
        assert_eq!(st, 200);
        assert!(pres_b.contains(&alice.user.email));

        let (st, enroll_b) = http_call(
            &base,
            Method::POST,
            "/v1/auth/mfa/enroll",
            Some(&bob.session_token),
            None,
        )
        .await;
        assert_eq!(st, 200);
        let enroll: swaggen_api_contract::TotpEnrollResponse =
            serde_json::from_str(&enroll_b).unwrap();
        let (st, _) = http_call(
            &base,
            Method::POST,
            "/v1/auth/mfa/verify",
            Some(&bob.session_token),
            Some(json!({"code": null, "recovery_code": enroll.recovery_codes_once[0]})),
        )
        .await;
        assert_eq!(st, 200);
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!(
                "/v1/workspaces/{}/members/{}/reset-mfa",
                ws.workspace_id, bob.user.user_id
            ),
            Some(&alice.session_token),
            None,
        )
        .await;
        assert_eq!(st, 200);

        let (st, aud_b) = http_call(
            &base,
            Method::GET,
            &format!(
                "/v1/workspaces/{}/audit/export?retention_days=30",
                ws.workspace_id
            ),
            Some(&alice.session_token),
            None,
        )
        .await;
        assert_eq!(st, 200);
        assert!(aud_b.contains("chain_sha256"));

        let (st, br_b) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/branches", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({"name":"feat-auth","entity_id":"req-1","base_hash":"rev-stale","branch_hash":"rev-b1","branch_payload":"{}"})),
        )
        .await;
        assert_eq!(st, 201);
        let br: swaggen_api_contract::BranchSummary = serde_json::from_str(&br_b).unwrap();
        let (st, mr_b) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/merge-requests", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({"branch_id": br.branch_id, "title": "Merge auth"})),
        )
        .await;
        assert_eq!(st, 201);
        let mr: swaggen_api_contract::MergeRequestRecord = serde_json::from_str(&mr_b).unwrap();
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!(
                "/v1/workspaces/{}/merge-requests/{}/merge",
                ws.workspace_id, mr.merge_id
            ),
            Some(&alice.session_token),
            None,
        )
        .await;
        assert_eq!(st, 409);

        // Ownership transfer Alice -> Bob succeeds atomically, then Alice can leave
        let (_, m_body) = http_call(
            &base,
            Method::GET,
            &format!("/v1/workspaces/{}/members", ws.workspace_id),
            Some(&alice.session_token),
            None,
        )
        .await;
        let mems2: swaggen_api_contract::MemberListResponse =
            serde_json::from_str(&m_body).unwrap();
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/transfer-ownership", ws.workspace_id),
            Some(&alice.session_token),
            Some(json!({"target_user_id": bob.user.user_id, "expected_membership_revision": mems2.membership_revision})),
        )
        .await;
        assert_eq!(st, 200);
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/leave", ws.workspace_id),
            Some(&alice.session_token),
            None,
        )
        .await;
        assert_eq!(st, 200);

        // P10 Team Secret Vault (EV10): Owner (Bob) sets & rotates, while Viewer is blocked!
        let (st, v1) = http_call(
            &base,
            Method::PUT,
            &format!("/v1/workspaces/{}/vault/STRIPE_KEY", ws.workspace_id),
            Some(&bob.session_token),
            Some(json!({"value":"sk_live_v1","note":"initial"})),
        )
        .await;
        assert_eq!(st, 200);
        assert!(v1.contains("\"version\":1"));
        let (st, v2) = http_call(
            &base,
            Method::PUT,
            &format!("/v1/workspaces/{}/vault/STRIPE_KEY", ws.workspace_id),
            Some(&bob.session_token),
            Some(json!({"value":"sk_live_v2","note":"rotated"})),
        )
        .await;
        assert_eq!(st, 200);
        assert!(v2.contains("\"version\":2"));
        let (st, val_b) = http_call(
            &base,
            Method::GET,
            &format!("/v1/workspaces/{}/vault/STRIPE_KEY", ws.workspace_id),
            Some(&bob.session_token),
            None,
        )
        .await;
        assert_eq!(st, 200);
        assert!(val_b.contains("sk_live_v2"));
        // Alice already left -> 403 on vault
        let (st, _) = http_call(
            &base,
            Method::GET,
            &format!("/v1/workspaces/{}/vault/STRIPE_KEY", ws.workspace_id),
            Some(&alice.session_token),
            None,
        )
        .await;
        assert_eq!(st, 403);

        // P11 Self-Hosted Runner lease (MK07) & Schedule webhook SSRF guard (MK08/MK09)
        let (st, lease_b) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/runners/register", ws.workspace_id),
            Some(&bob.session_token),
            Some(json!({"runner_name":"ci-win-01","tags":["windows"]})),
        )
        .await;
        assert_eq!(st, 201);
        let lease: swaggen_api_contract::RunnerLeaseRecord =
            serde_json::from_str(&lease_b).unwrap();
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!(
                "/v1/workspaces/{}/runners/{}/heartbeat",
                ws.workspace_id, lease.runner_id
            ),
            Some(&bob.session_token),
            None,
        )
        .await;
        assert_eq!(st, 200);
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/schedules", ws.workspace_id),
            Some(&bob.session_token),
            Some(json!({"name":"BadHook","cron_expr":"0 9 * * *","timezone_offset_secs":0,"failure_webhook_url":"http://169.254.169.254/hook"})),
        )
        .await;
        assert_eq!(st, 400);
        let (st, _) = http_call(
            &base,
            Method::POST,
            &format!("/v1/workspaces/{}/schedules", ws.workspace_id),
            Some(&bob.session_token),
            Some(json!({"name":"Nightly","cron_expr":"0 9 * * *","timezone_offset_secs":0,"failure_webhook_url":"https://alerts.example.com/hook"})),
        )
        .await;
        assert_eq!(st, 201);

        stop.send(()).unwrap();
        let _ = srv.await;
    }

    #[tokio::test]
    async fn graphql_introspection_zero_n_plus_1_batching_and_idempotency_key_e2e() {
        let store = Arc::new(ServerStore::Memory(InMemoryStore::new(None)));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let srv = tokio::spawn(async move {
            axum::serve(listener, router(store))
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });

        let client: Client<_, Full<axum::body::Bytes>> =
            Client::builder(TokioExecutor::new()).build_http();

        // 1. Bootstrap owner
        let (_, owner_raw) = http_call(
            &base,
            Method::POST,
            "/v1/auth/bootstrap-owner",
            None,
            Some(json!({
                "email": "gql-owner@swaggen.local",
                "display_name": "GQL Owner",
                "password": "StrongPassword!123",
                "device_name": "WinPC",
                "bootstrap_token": null
            })),
        )
        .await;
        let owner: AuthResponse = serde_json::from_str(&owner_raw).unwrap();

        // 2. Test Idempotency-Key middleware on REST POST /v1/workspaces (replay + 409 payload mismatch)
        let send_idem = |idem_key: &str, payload: serde_json::Value| {
            let client = client.clone();
            let uri = format!("{base}/v1/workspaces");
            let tok = owner.session_token.clone();
            let idem = idem_key.to_owned();
            async move {
                let req = HyperReq::builder()
                    .method(Method::POST)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {tok}"))
                    .header("idempotency-key", idem)
                    .body(Full::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap();
                let resp = client.request(req).await.unwrap();
                let status = resp.status().as_u16();
                let replayed = resp
                    .headers()
                    .get("x-idempotent-replayed")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_owned();
                let raw = resp.into_body().collect().await.unwrap().to_bytes();
                (status, replayed, String::from_utf8_lossy(&raw).into_owned())
            }
        };

        let (st1, rep1, b1) = send_idem(
            "idem-ws-alpha-001",
            json!({"slug": "alpha-team", "name": "Alpha Team"}),
        )
        .await;
        assert_eq!(st1, 201);
        assert_eq!(rep1, "false");

        // Exact retry with same Idempotency-Key returns cached 201 response without failing on duplicate slug!
        let (st2, rep2, b2) = send_idem(
            "idem-ws-alpha-001",
            json!({"slug": "alpha-team", "name": "Alpha Team"}),
        )
        .await;
        assert_eq!(st2, 201);
        assert_eq!(rep2, "true");
        assert_eq!(b1, b2);

        // Reusing same Idempotency-Key with a DIFFERENT payload is rejected with 409 Conflict!
        let (st_conflict, _, _) = send_idem(
            "idem-ws-alpha-001",
            json!({"slug": "different-slug", "name": "Different"}),
        )
        .await;
        assert_eq!(st_conflict, 409);

        // 3. Create 3 additional workspaces + entities + comments via GraphQL mutations (with Idempotency-Key)
        for slug in ["beta-team", "gamma-team", "delta-team"] {
            let (st, body) = http_call(
                &base,
                Method::POST,
                "/v1/graphql",
                Some(&owner.session_token),
                Some(json!({
                    "query": "mutation Create($slug: String!, $name: String!) { createWorkspace(slug: $slug, name: $name) { workspaceId slug } }",
                    "variables": { "slug": slug, "name": format!("WS {slug}") }
                })),
            )
            .await;
            assert_eq!(st, 200, "createWorkspace GraphQL failed: {body}");
            let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
            let ws_id = parsed["data"]["createWorkspace"]["workspaceId"]
                .as_str()
                .unwrap()
                .to_owned();

            let (st_e, _) = http_call(
                &base,
                Method::POST,
                "/v1/graphql",
                Some(&owner.session_token),
                Some(json!({
                    "query": "mutation Upsert($ws: ID!, $eid: ID!, $name: String!) { upsertEntity(workspaceId: $ws, entityId: $eid, kind: \"request\", name: $name, payloadJson: \"{}\", baseRevision: 0) { entityId revision } }",
                    "variables": { "ws": ws_id, "eid": format!("req-{slug}"), "name": format!("Get {slug}") }
                })),
            )
            .await;
            assert_eq!(st_e, 200);
        }

        // 4. GraphQL __schema introspection works and parses cleanly with domain helper
        let (st_intro, intro_body) = http_call(
            &base,
            Method::POST,
            "/v1/graphql",
            Some(&owner.session_token),
            Some(json!({
                "query": swaggen_domain::graphql::INTROSPECTION_QUERY,
                "operationName": "SwaggenIntrospection"
            })),
        )
        .await;
        assert_eq!(st_intro, 200);
        let summary = swaggen_domain::graphql::parse_introspection_schema(&intro_body)
            .expect("introspection schema should parse cleanly");
        assert!(summary.query_fields.iter().any(|q| q.name == "workspaces"));
        assert!(
            summary
                .mutation_fields
                .iter()
                .any(|m| m.name == "createWorkspace")
        );

        // 5. Mutation over HTTP GET is rejected with 405 per GraphQL-over-HTTP spec
        let (st_get_mut, _) = http_call(
            &base,
            Method::GET,
            "/v1/graphql?query=mutation%20%7B%20createWorkspace(slug%3A%22x%22%2Cname%3A%22x%22)%20%7B%20workspaceId%20%7D%20%7D",
            Some(&owner.session_token),
            None,
        )
        .await;
        assert_eq!(st_get_mut, 405);

        // 6. Batched nested query across ALL 4 workspaces executes in constant O(1) DB queries (Zero N+1!)
        let (st_batch, batch_body) = http_call(
            &base,
            Method::POST,
            "/v1/graphql",
            Some(&owner.session_token),
            Some(json!({
                "query": "query AllWorkspaces { workspaces { workspaceId slug headRevision entities { entityId name } members { email role } invitations { email } comments { body } } }"
            })),
        )
        .await;
        assert_eq!(st_batch, 200);
        let batch_val: serde_json::Value = serde_json::from_str(&batch_body).unwrap();
        let ws_arr = batch_val["data"]["workspaces"].as_array().unwrap();
        assert_eq!(ws_arr.len(), 4);
        let db_queries = batch_val["extensions"]["performance"]["dbQueries"]
            .as_u64()
            .expect("performance telemetry expected");
        assert!(
            db_queries <= 7,
            "Expected constant O(1) batched queries (<= 7) across 4 workspaces with 4 nested lists, got {db_queries}"
        );

        stop.send(()).unwrap();
        let _ = srv.await;
    }
}
