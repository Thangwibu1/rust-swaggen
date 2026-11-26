//! Web Admin Portal (`/admin`) & Global Release Update Broadcast (`/v1/releases/latest`).
//!
//! Admin credentials are read exclusively from `SWAGGEN_ADMIN_USERNAME` and `SWAGGEN_ADMIN_PASSWORD`
//! environment variables (configured in `.env` or Coolify Environment Variables).
//! All database writes use soft deactivation (`active: false`, `revoked: true`) via `kv_put`.

use crate::{AppState, RequestCorrelationId, map_err};
use axum::{
    Extension, Json,
    extract::State,
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{AUTHORIZATION, COOKIE, SET_COOKIE},
    },
    response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use swaggen_api_contract::ErrorEnvelope;
use swaggen_mongo_store::{
    crypto::sha256_hex,
    error::StoreError,
    repo::{WorkspaceRepository, now_unix},
};

pub const KV_ADMIN_SESSIONS: &str = "admin_sessions";
pub const KV_SYSTEM_RELEASES: &str = "system_releases";
const ADMIN_COOKIE_NAME: &str = "swaggen_admin_session";
const ADMIN_SESSION_TTL_SECS: i64 = 12 * 3600;

#[derive(Debug, Clone)]
pub struct AdminAuthConfig {
    pub username: String,
    pub password: String,
}

impl AdminAuthConfig {
    pub fn from_env() -> Option<Self> {
        let username = std::env::var("SWAGGEN_ADMIN_USERNAME")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())?;
        let password = std::env::var("SWAGGEN_ADMIN_PASSWORD")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())?;
        Some(Self { username, password })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AdminSessionRecord {
    pub username: String,
    pub created_at_unix: i64,
    pub expires_at_unix: i64,
    #[serde(default)]
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastReleaseRecord {
    pub active: bool,
    pub broadcast_id: String,
    pub version: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub installer_url: String,
    #[serde(default)]
    pub installer_sha256: String,
    #[serde(default)]
    pub msi_url: String,
    #[serde(default)]
    pub msi_sha256: String,
    #[serde(default)]
    pub portable_url: String,
    #[serde(default)]
    pub portable_sha256: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub auto_install: bool,
    #[serde(default)]
    pub broadcast_by: String,
    pub updated_at_unix: i64,
}

#[derive(Debug, Deserialize)]
pub struct AdminLoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct BroadcastReleaseInput {
    pub version: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub installer_url: String,
    #[serde(default)]
    pub installer_sha256: String,
    #[serde(default)]
    pub portable_url: String,
    #[serde(default)]
    pub portable_sha256: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub auto_install: bool,
}

fn constant_time_str_eq(a: &str, b: &str) -> bool {
    let ha = sha256_hex(a);
    let hb = sha256_hex(b);
    let ab = ha.as_bytes();
    let bb = hb.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (&x, &y) in ab.iter().zip(bb.iter()) {
        diff |= x ^ y;
    }
    diff == 0 && a.len() == b.len()
}

fn resolve_admin_config(state: &AppState) -> Option<AdminAuthConfig> {
    state.admin_auth.clone().or_else(AdminAuthConfig::from_env)
}

fn extract_admin_token(headers: &HeaderMap) -> Option<String> {
    if let Some(bearer) = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Some(bearer.to_owned());
    }
    if let Some(cookie_hdr) = headers.get(COOKIE).and_then(|v| v.to_str().ok()) {
        for part in cookie_hdr.split(';') {
            let trimmed = part.trim();
            if let Some((k, v)) = trimmed.split_once('=')
                && k.trim() == ADMIN_COOKIE_NAME
            {
                let val = v.trim();
                if !val.is_empty() {
                    return Some(val.to_owned());
                }
            }
        }
    }
    None
}

async fn require_admin_session(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AdminSessionRecord, StoreError> {
    let Some(token) = extract_admin_token(headers) else {
        return Err(StoreError::Unauthorized("Admin session required".into()));
    };
    let key = sha256_hex(&token);
    let Some(raw) = state.store.kv_get(KV_ADMIN_SESSIONS, &key).await? else {
        return Err(StoreError::Unauthorized("Admin session not found".into()));
    };
    let rec: AdminSessionRecord = serde_json::from_str(&raw)
        .map_err(|_| StoreError::Unauthorized("Invalid admin session".into()))?;
    let now = now_unix();
    if rec.revoked || rec.expires_at_unix <= now {
        return Err(StoreError::Unauthorized("Admin session expired".into()));
    }
    Ok(rec)
}

fn validate_release_url(url: &str) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if trimmed.starts_with("https://")
        || trimmed.starts_with("http://127.0.0.1")
        || trimmed.starts_with("http://localhost")
    {
        Ok(())
    } else {
        Err("Download URLs must use HTTPS (or localhost for local testing).".into())
    }
}

pub async fn admin_login(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    Json(req): Json<AdminLoginRequest>,
) -> Result<Response, (StatusCode, Json<ErrorEnvelope>)> {
    let Some(cfg) = resolve_admin_config(&state) else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorEnvelope::new(
                "admin_not_configured",
                "Admin credentials are not configured. Set SWAGGEN_ADMIN_USERNAME and SWAGGEN_ADMIN_PASSWORD in .env or Coolify Environment Variables.",
                &corr,
                false,
            )),
        ));
    };

    let user_ok = constant_time_str_eq(req.username.trim(), &cfg.username);
    let pass_ok = constant_time_str_eq(&req.password, &cfg.password);
    if !user_ok || !pass_ok {
        return Err(map_err(
            &corr,
            StoreError::Unauthorized("Invalid admin username or password".into()),
        ));
    }

    let raw_token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let token_hash = sha256_hex(&raw_token);
    let now = now_unix();
    let expires_at = now + ADMIN_SESSION_TTL_SECS;
    let session = AdminSessionRecord {
        username: cfg.username,
        created_at_unix: now,
        expires_at_unix: expires_at,
        revoked: false,
    };
    let val =
        serde_json::to_string(&session).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    state
        .store
        .kv_put(KV_ADMIN_SESSIONS, &token_hash, &val, now)
        .await
        .map_err(|e| map_err(&corr, e))?;

    let cookie = format!(
        "{ADMIN_COOKIE_NAME}={raw_token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={ADMIN_SESSION_TTL_SECS}"
    );
    let mut resp = Json(json!({
        "ok": true,
        "token": raw_token,
        "expires_at_unix": expires_at,
    }))
    .into_response();
    if let Ok(hv) = HeaderValue::from_str(&cookie) {
        resp.headers_mut().insert(SET_COOKIE, hv);
    }
    Ok(resp)
}

pub async fn admin_logout(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, Json<ErrorEnvelope>)> {
    if let Some(token) = extract_admin_token(&headers) {
        let key = sha256_hex(&token);
        if let Some(raw) = state
            .store
            .kv_get(KV_ADMIN_SESSIONS, &key)
            .await
            .map_err(|e| map_err(&corr, e))?
            && let Ok(mut rec) = serde_json::from_str::<AdminSessionRecord>(&raw)
        {
            rec.revoked = true;
            if let Ok(val) = serde_json::to_string(&rec) {
                let _ = state
                    .store
                    .kv_put(KV_ADMIN_SESSIONS, &key, &val, now_unix())
                    .await;
            }
        }
    }
    let clear_cookie =
        format!("{ADMIN_COOKIE_NAME}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0");
    let mut resp = Json(json!({ "ok": true })).into_response();
    if let Ok(hv) = HeaderValue::from_str(&clear_cookie) {
        resp.headers_mut().insert(SET_COOKIE, hv);
    }
    Ok(resp)
}

pub async fn admin_get_state(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorEnvelope>)> {
    let session = require_admin_session(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;

    let latest = state
        .store
        .kv_get(KV_SYSTEM_RELEASES, "latest")
        .await
        .map_err(|e| map_err(&corr, e))?
        .and_then(|raw| serde_json::from_str::<BroadcastReleaseRecord>(&raw).ok());

    let mut history: Vec<BroadcastReleaseRecord> = state
        .store
        .kv_list(KV_SYSTEM_RELEASES, "history:")
        .await
        .map_err(|e| map_err(&corr, e))?
        .into_iter()
        .filter_map(|(_, raw)| serde_json::from_str::<BroadcastReleaseRecord>(&raw).ok())
        .collect();
    history.sort_by_key(|r| std::cmp::Reverse(r.updated_at_unix));
    history.truncate(15);

    Ok(Json(json!({
        "admin_username": session.username,
        "server_version": env!("CARGO_PKG_VERSION"),
        "latest": latest,
        "history": history,
    })))
}

pub async fn admin_broadcast_release(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: HeaderMap,
    Json(input): Json<BroadcastReleaseInput>,
) -> Result<Json<BroadcastReleaseRecord>, (StatusCode, Json<ErrorEnvelope>)> {
    let session = require_admin_session(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;

    let version = input
        .version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_owned();
    if version.is_empty() {
        return Err(map_err(
            &corr,
            StoreError::InvalidInput("Version is required (e.g. 0.1.23)".into()),
        ));
    }
    if input.installer_url.trim().is_empty() && input.portable_url.trim().is_empty() {
        return Err(map_err(
            &corr,
            StoreError::InvalidInput(
                "Provide at least an Installer (.exe) URL or Portable (.zip) URL.".into(),
            ),
        ));
    }
    if let Err(msg) = validate_release_url(&input.installer_url) {
        return Err(map_err(&corr, StoreError::InvalidInput(msg)));
    }
    if let Err(msg) = validate_release_url(&input.portable_url) {
        return Err(map_err(&corr, StoreError::InvalidInput(msg)));
    }

    let now = now_unix();
    let tag = if input.tag.trim().is_empty() {
        format!("v{version}")
    } else {
        input.tag.trim().to_owned()
    };
    let published_at = time::OffsetDateTime::from_unix_timestamp(now)
        .ok()
        .map(|dt| {
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                dt.year(),
                dt.month() as u8,
                dt.day(),
                dt.hour(),
                dt.minute(),
                dt.second()
            )
        })
        .unwrap_or_else(|| now.to_string());
    let broadcast_id = uuid::Uuid::new_v4().to_string();

    let rec = BroadcastReleaseRecord {
        active: true,
        broadcast_id: broadcast_id.clone(),
        version,
        tag,
        published_at,
        installer_url: input.installer_url.trim().to_owned(),
        installer_sha256: input.installer_sha256.trim().to_ascii_lowercase(),
        msi_url: String::new(),
        msi_sha256: String::new(),
        portable_url: input.portable_url.trim().to_owned(),
        portable_sha256: input.portable_sha256.trim().to_ascii_lowercase(),
        notes: input.notes.trim().to_owned(),
        auto_install: input.auto_install,
        broadcast_by: session.username,
        updated_at_unix: now,
    };

    let raw = serde_json::to_string(&rec).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
    state
        .store
        .kv_put(KV_SYSTEM_RELEASES, "latest", &raw, now)
        .await
        .map_err(|e| map_err(&corr, e))?;
    let hist_key = format!("history:{now}:{broadcast_id}");
    let _ = state
        .store
        .kv_put(KV_SYSTEM_RELEASES, &hist_key, &raw, now)
        .await;

    let _ = state.event_bus.send(("__global_release__".into(), now));
    Ok(Json(rec))
}

pub async fn admin_deactivate_release(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorEnvelope>)> {
    let _session = require_admin_session(&state, &headers)
        .await
        .map_err(|e| map_err(&corr, e))?;

    if let Some(raw) = state
        .store
        .kv_get(KV_SYSTEM_RELEASES, "latest")
        .await
        .map_err(|e| map_err(&corr, e))?
        && let Ok(mut rec) = serde_json::from_str::<BroadcastReleaseRecord>(&raw)
    {
        let now = now_unix();
        rec.active = false;
        rec.updated_at_unix = now;
        let next_raw =
            serde_json::to_string(&rec).map_err(|_| map_err(&corr, StoreError::InternalSafe))?;
        state
            .store
            .kv_put(KV_SYSTEM_RELEASES, "latest", &next_raw, now)
            .await
            .map_err(|e| map_err(&corr, e))?;
    }
    Ok(Json(json!({ "ok": true, "active": false })))
}

/// Public endpoint polled by all Swaggen Desktop clients to receive live update broadcasts.
pub async fn public_latest_release(
    State(state): State<AppState>,
    Extension(RequestCorrelationId(corr)): Extension<RequestCorrelationId>,
) -> Result<Response, (StatusCode, Json<ErrorEnvelope>)> {
    let Some(raw) = state
        .store
        .kv_get(KV_SYSTEM_RELEASES, "latest")
        .await
        .map_err(|e| map_err(&corr, e))?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let Ok(rec) = serde_json::from_str::<BroadcastReleaseRecord>(&raw) else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    if !rec.active {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    Ok(Json(rec).into_response())
}

pub async fn serve_admin_html(State(state): State<AppState>) -> Html<String> {
    let configured = resolve_admin_config(&state).is_some();
    let server_ver = env!("CARGO_PKG_VERSION");
    let config_banner = if configured {
        ""
    } else {
        r#"<div class="alert alert-warn"><strong>Admin credentials not configured:</strong> Set <code>SWAGGEN_ADMIN_USERNAME</code> and <code>SWAGGEN_ADMIN_PASSWORD</code> in your <code>.env</code> or Coolify Environment Variables, then restart the server.</div>"#
    };
    let html = ADMIN_HTML_TEMPLATE
        .replace("__SERVER_VERSION__", server_ver)
        .replace("__CONFIG_BANNER__", config_banner);
    Html(html)
}

const ADMIN_HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Swaggen Admin · Release Broadcast</title>
  <link rel="icon" href="/favicon.ico" />
  <style>
    :root {
      --bg: #f8fafc;
      --surface: #ffffff;
      --border: #e2e8f0;
      --text: #0f172a;
      --muted: #64748b;
      --primary: #2563eb;
      --primary-hover: #1d4ed8;
      --success: #059669;
      --danger: #dc2626;
      --warn: #d97706;
    }
    * { box-sizing: border-box; font-family: 'JetBrains Mono', ui-monospace, SFMono-Regular, Consolas, monospace; }
    body { margin: 0; background: var(--bg); color: var(--text); font-size: 13px; line-height: 1.55; }
    .topbar { background: var(--surface); border-bottom: 1px solid var(--border); padding: 14px 24px; display: flex; align-items: center; justify-content: space-between; }
    .brand { display: flex; align-items: center; gap: 10px; font-weight: 700; font-size: 15px; }
    .brand img { width: 24px; height: 24px; }
    .pill { display: inline-block; padding: 2px 8px; border-radius: 999px; font-size: 11px; font-weight: 600; border: 1px solid var(--border); background: #f1f5f9; color: var(--muted); }
    .pill-live { background: #ecfdf5; border-color: #a7f3d0; color: var(--success); }
    .container { max-width: 880px; margin: 28px auto; padding: 0 18px 48px; }
    .card { background: var(--surface); border: 1px solid var(--border); border-radius: 10px; padding: 22px; margin-bottom: 18px; box-shadow: 0 1px 2px rgba(15, 23, 42, 0.03); }
    h2 { margin: 0 0 6px; font-size: 16px; }
    .sub { color: var(--muted); font-size: 12px; margin-bottom: 16px; }
    .field { margin-bottom: 12px; }
    .field label { display: block; font-size: 11.5px; font-weight: 600; margin-bottom: 4px; color: #334155; }
    input[type="text"], input[type="password"], textarea {
      width: 100%; padding: 9px 11px; border: 1px solid var(--border); border-radius: 7px; font-size: 12.5px; color: var(--text); background: #fff;
    }
    input:focus, textarea:focus { outline: none; border-color: var(--primary); box-shadow: 0 0 0 2px rgba(37,99,235,0.12); }
    .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
    @media (max-width: 680px) { .grid2 { grid-template-columns: 1fr; } }
    .btn { cursor: pointer; border: 1px solid transparent; border-radius: 7px; padding: 9px 15px; font-size: 12.5px; font-weight: 600; transition: 0.12s; }
    .btn-primary { background: var(--primary); color: #fff; }
    .btn-primary:hover { background: var(--primary-hover); }
    .btn-secondary { background: #fff; border-color: var(--border); color: var(--text); }
    .btn-secondary:hover { background: #f1f5f9; }
    .btn-danger { background: #fef2f2; border-color: #fecaca; color: var(--danger); }
    .alert { padding: 11px 14px; border-radius: 8px; margin-bottom: 16px; font-size: 12px; }
    .alert-err { background: #fef2f2; border: 1px solid #fecaca; color: var(--danger); }
    .alert-ok { background: #ecfdf5; border: 1px solid #a7f3d0; color: var(--success); }
    .alert-warn { background: #fffbeb; border: 1px solid #fde68a; color: var(--warn); }
    .hidden { display: none !important; }
    .row { display: flex; align-items: center; justify-content: space-between; gap: 10px; flex-wrap: wrap; }
    table { width: 100%; border-collapse: collapse; font-size: 11.5px; margin-top: 10px; }
    th, td { text-align: left; padding: 8px 10px; border-bottom: 1px solid var(--border); }
    th { color: var(--muted); font-size: 10.5px; text-transform: uppercase; }
  </style>
</head>
<body>
  <div class="topbar">
    <div class="brand">
      <img src="/v1/assets/brand-mark.png" alt="Swaggen" />
      <span>Swaggen Admin Portal</span>
      <span class="pill">Server v__SERVER_VERSION__</span>
    </div>
    <div id="adminUserBar" class="row hidden">
      <span id="adminWho" class="pill pill-live"></span>
      <button class="btn btn-secondary" onclick="doLogout()">Sign out</button>
    </div>
  </div>

  <div class="container">
    __CONFIG_BANNER__
    <div id="flashBox" class="hidden"></div>

    <!-- Login Card -->
    <div id="loginCard" class="card" style="max-width: 440px; margin: 36px auto;">
      <h2>Admin Sign In</h2>
      <div class="sub">Authenticate with <code>SWAGGEN_ADMIN_USERNAME</code> and <code>SWAGGEN_ADMIN_PASSWORD</code> from server environment.</div>
      <form onsubmit="doLogin(event)">
        <div class="field">
          <label>Username</label>
          <input id="loginUser" type="text" autocomplete="username" required placeholder="admin" />
        </div>
        <div class="field">
          <label>Password</label>
          <input id="loginPass" type="password" autocomplete="current-password" required placeholder="••••••••••••" />
        </div>
        <button type="submit" class="btn btn-primary" style="width: 100%; margin-top: 6px;">Sign in to Admin</button>
      </form>
    </div>

    <!-- Dashboard -->
    <div id="dashboard" class="hidden">
      <div class="card">
        <div class="row">
          <div>
            <h2>Live Update Broadcast Status</h2>
            <div class="sub" style="margin-bottom: 0;">All online Swaggen Desktop apps poll <code>GET /v1/releases/latest</code> and receive update notifications automatically.</div>
          </div>
          <button id="stopBtn" class="btn btn-danger hidden" onclick="deactivateBroadcast()">Stop Active Broadcast</button>
        </div>
        <div id="activeStatusBox" style="margin-top: 14px; padding: 12px; border-radius: 8px; background: #f8fafc; border: 1px solid var(--border);">
          Checking current broadcast…
        </div>
      </div>

      <div class="card">
        <div class="row" style="margin-bottom: 12px;">
          <div>
            <h2>Push New Version to All Users</h2>
            <div class="sub" style="margin-bottom: 0;">Fill release URLs (or auto-fill from GitHub Release) and click Broadcast to notify &amp; upgrade all clients.</div>
          </div>
          <button type="button" class="btn btn-secondary" onclick="autofillGitHub()">Auto-fill GitHub URLs from Version</button>
        </div>

        <form onsubmit="publishBroadcast(event)">
          <div class="grid2">
            <div class="field">
              <label>Target Version *</label>
              <input id="fVersion" type="text" required placeholder="0.1.23" />
            </div>
            <div class="field">
              <label>Git Tag (optional)</label>
              <input id="fTag" type="text" placeholder="v0.1.23" />
            </div>
          </div>

          <div class="grid2">
            <div class="field">
              <label>Windows Setup Installer URL (.exe)</label>
              <input id="fInstallerUrl" type="text" placeholder="https://github.com/Thangwibu1/rust-swaggen/releases/download/v0.1.23/Swaggen-Setup-0.1.23-x64.exe" />
            </div>
            <div class="field">
              <label>Installer SHA-256 (recommended)</label>
              <input id="fInstallerSha" type="text" placeholder="64-char hex SHA-256 checksum" />
            </div>
          </div>

          <div class="grid2">
            <div class="field">
              <label>Portable Bundle URL (.zip)</label>
              <input id="fPortableUrl" type="text" placeholder="https://github.com/Thangwibu1/rust-swaggen/releases/download/v0.1.23/Swaggen-0.1.23-win64-portable.zip" />
            </div>
            <div class="field">
              <label>Portable ZIP SHA-256 (optional)</label>
              <input id="fPortableSha" type="text" placeholder="64-char hex SHA-256 checksum" />
            </div>
          </div>

          <div class="field">
            <label>Release Notes (shown in user notification banner)</label>
            <textarea id="fNotes" rows="3" placeholder="Bug fixes, Cookie Jar support, cleaner API Docs & Home screen..."></textarea>
          </div>

          <div class="field" style="display: flex; align-items: center; gap: 8px; margin: 14px 0;">
            <input id="fAutoInstall" type="checkbox" checked style="width: 16px; height: 16px;" />
            <label for="fAutoInstall" style="margin: 0; cursor: pointer;">
              <strong>Auto-download &amp; silent upgrade:</strong> Clients automatically fetch, verify SHA-256, install, and restart into the new version.
            </label>
          </div>

          <button type="submit" class="btn btn-primary">Broadcast Update to All Users</button>
        </form>
      </div>

      <div class="card">
        <h2>Recent Broadcast History</h2>
        <table>
          <thead>
            <tr><th>Version</th><th>Mode</th><th>By</th><th>Timestamp (UTC)</th><th>Notes</th></tr>
          </thead>
          <tbody id="historyBody">
            <tr><td colspan="5" style="color: var(--muted);">No broadcasts recorded yet.</td></tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>

  <script>
    function showFlash(msg, isErr) {
      const el = document.getElementById('flashBox');
      el.className = 'alert ' + (isErr ? 'alert-err' : 'alert-ok');
      el.textContent = msg;
      el.classList.remove('hidden');
    }

    async function loadState() {
      try {
        const r = await fetch('/v1/admin/state', { credentials: 'same-origin' });
        if (!r.ok) {
          document.getElementById('loginCard').classList.remove('hidden');
          document.getElementById('dashboard').classList.add('hidden');
          document.getElementById('adminUserBar').classList.add('hidden');
          return;
        }
        const data = await r.json();
        document.getElementById('loginCard').classList.add('hidden');
        document.getElementById('dashboard').classList.remove('hidden');
        document.getElementById('adminUserBar').classList.remove('hidden');
        document.getElementById('adminWho').textContent = 'Admin: ' + data.admin_username;

        const box = document.getElementById('activeStatusBox');
        const stopBtn = document.getElementById('stopBtn');
        if (data.latest && data.latest.active) {
          stopBtn.classList.remove('hidden');
          box.innerHTML = '<span class="pill pill-live">LIVE BROADCAST</span> <strong>v' + esc(data.latest.version) + '</strong> · '
            + (data.latest.auto_install ? 'Auto-Install ON' : 'Banner Prompt')
            + ' · Published ' + esc(data.latest.published_at)
            + (data.latest.notes ? '<div style="margin-top:6px;color:#475569;">' + esc(data.latest.notes) + '</div>' : '');
        } else {
          stopBtn.classList.add('hidden');
          box.innerHTML = '<span class="pill">IDLE</span> No active update broadcast. Clients remain on their current version.';
        }

        const tbody = document.getElementById('historyBody');
        if (data.history && data.history.length > 0) {
          tbody.innerHTML = data.history.map(h =>
            '<tr><td><strong>v' + esc(h.version) + '</strong></td><td>' + (h.auto_install ? 'Auto-install' : 'Prompt') + '</td><td>' + esc(h.broadcast_by || '-') + '</td><td>' + esc(h.published_at || '') + '</td><td>' + esc(h.notes || '-') + '</td></tr>'
          ).join('');
        }
      } catch (e) {
        showFlash('Network error: ' + e, true);
      }
    }

    function esc(s) {
      return String(s || '').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
    }

    async function doLogin(ev) {
      ev.preventDefault();
      const username = document.getElementById('loginUser').value.trim();
      const password = document.getElementById('loginPass').value;
      const r = await fetch('/v1/admin/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'same-origin',
        body: JSON.stringify({ username, password })
      });
      const data = await r.json().catch(() => ({}));
      if (!r.ok) {
        showFlash((data.error && data.error.message) || 'Invalid admin username or password.', true);
        return;
      }
      document.getElementById('loginPass').value = '';
      document.getElementById('flashBox').classList.add('hidden');
      await loadState();
    }

    async function doLogout() {
      await fetch('/v1/admin/logout', { method: 'POST', credentials: 'same-origin' });
      await loadState();
    }

    async function autofillGitHub() {
      let ver = document.getElementById('fVersion').value.trim().replace(/^[vV]/, '');
      if (!ver) {
        showFlash('Enter a Target Version first (e.g. 0.1.23), then click Auto-fill.', true);
        return;
      }
      const tag = 'v' + ver;
      document.getElementById('fVersion').value = ver;
      document.getElementById('fTag').value = tag;
      const base = 'https://github.com/Thangwibu1/rust-swaggen/releases/download/' + tag;
      document.getElementById('fInstallerUrl').value = base + '/Swaggen-Setup-' + ver + '-x64.exe';
      document.getElementById('fPortableUrl').value = base + '/Swaggen-' + ver + '-win64-portable.zip';
      try {
        const r = await fetch(base + '/latest.json');
        if (r.ok) {
          const m = await r.json();
          if (m.installer_sha256) document.getElementById('fInstallerSha').value = m.installer_sha256;
          if (m.portable_sha256) document.getElementById('fPortableSha').value = m.portable_sha256;
          if (m.notes && !document.getElementById('fNotes').value) document.getElementById('fNotes').value = m.notes;
          showFlash('Auto-filled URLs & SHA-256 checksums from GitHub Release ' + tag + '!', false);
          return;
        }
      } catch (_) {}
      showFlash('Pre-filled GitHub Release download URLs for ' + tag + '. Paste SHA-256 from SHA256SUMS.txt if available.', false);
    }

    async function publishBroadcast(ev) {
      ev.preventDefault();
      const payload = {
        version: document.getElementById('fVersion').value.trim(),
        tag: document.getElementById('fTag').value.trim(),
        installer_url: document.getElementById('fInstallerUrl').value.trim(),
        installer_sha256: document.getElementById('fInstallerSha').value.trim(),
        portable_url: document.getElementById('fPortableUrl').value.trim(),
        portable_sha256: document.getElementById('fPortableSha').value.trim(),
        notes: document.getElementById('fNotes').value.trim(),
        auto_install: document.getElementById('fAutoInstall').checked
      };
      const r = await fetch('/v1/admin/releases/broadcast', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'same-origin',
        body: JSON.stringify(payload)
      });
      const data = await r.json().catch(() => ({}));
      if (!r.ok) {
        showFlash((data.error && data.error.message) || 'Failed to broadcast release.', true);
        return;
      }
      showFlash('Broadcast live! All connected Swaggen users are now notified of v' + data.version + '.', false);
      await loadState();
    }

    async function deactivateBroadcast() {
      const r = await fetch('/v1/admin/releases/deactivate', { method: 'POST', credentials: 'same-origin' });
      if (r.ok) {
        showFlash('Active update broadcast stopped.', false);
        await loadState();
      }
    }

    loadState();
  </script>
</body>
</html>
"#;
