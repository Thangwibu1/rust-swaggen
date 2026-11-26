//! Server HTTP + DPAPI session + ServerProfileJournal command handlers for the desktop worker.
use crate::{
    server_client::{server_call, server_call_empty},
    worker::{Command, Event},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc::SyncSender},
};
use swaggen_api_contract::{
    AcceptDirectInvitationRequest, AcceptInvitationRequest, AcceptInvitationResponse,
    AuditEventRecord, AuditListResponse, AuthResponse, BootstrapOwnerRequest, Capabilities,
    ChangePasswordRequest, CreateInvitationRequest, CreateInvitationResponse,
    CreatePublicInviteLinkRequest, CreatePublicInviteLinkResponse, CreateWorkspaceRequest,
    FirebaseLoginRequest, JoinByInviteLinkRequest, LoginRequest, MemberListResponse,
    PendingInvitationsResponse, PublicInviteLinkListResponse, SessionListResponse,
    TeamWorkspaceSummary, UpdateMemberRoleRequest, WorkspaceListResponse,
};
use swaggen_domain::{
    Error, LocalId, Secret, SecretRef,
    server_profiles::{ServerProfile, ServerProfileStore, validate_server_url},
    workspace::{
        HttpMethod, RequestBody, RequestConnectionPolicy, ResolvedHeader, ResolvedRequest,
    },
};
use swaggen_http_engine::execute_request;
use swaggen_local_store::{WorkspaceDirectory, server_profiles_store::ServerProfileJournal};
use swaggen_secret_store::SecretStore;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

pub const DEFAULT_SERVER_NAME: &str = "Swaggen Production";
pub const DEFAULT_SERVER_URL: &str = "https://swaggen-api.ngocthang.io.vn";

pub fn default_server_url() -> String {
    std::env::var("SWAGGEN_SERVER_URL")
        .ok()
        .and_then(|raw| validate_server_url(&raw).ok().map(|(url, _, _)| url))
        .unwrap_or_else(|| DEFAULT_SERVER_URL.to_owned())
}

pub fn load_server_profiles(root: &Path) -> ServerProfileStore {
    let mut store = ServerProfileJournal::open(root)
        .map(|j| j.store().clone())
        .unwrap_or_default();
    if store.profiles.is_empty() {
        store.upsert(ServerProfile::new(
            "production",
            DEFAULT_SERVER_NAME,
            default_server_url(),
            false,
        ));
    }
    store
}

fn open_auth_secrets(root: &Path) -> Result<(LocalId, SecretStore), String> {
    let ws = LocalId::parse("server-auth").map_err(|e| e.to_string())?;
    let dir = WorkspaceDirectory::open(root, ws.clone()).map_err(|e| e.to_string())?;
    let store = SecretStore::open(dir).map_err(|e| e.to_string())?;
    Ok((ws, store))
}

fn save_session_token(root: &Path, profile: &ServerProfile, token: &str) -> Result<(), String> {
    let (ws, mut store) = open_auth_secrets(root)?;
    let key = profile.secret_key_id().map_err(|e| e.to_string())?;
    let secret = Secret::new(token.as_bytes().to_vec()).map_err(|e| e.to_string())?;
    store
        .put(
            &SecretRef {
                workspace: ws,
                name: key,
            },
            &secret,
        )
        .map_err(|e| e.to_string())
}

pub fn read_session_token(root: &Path, profile: &ServerProfile) -> Result<String, String> {
    let (ws, store) = open_auth_secrets(root)?;
    let key = profile.secret_key_id().map_err(|e| e.to_string())?;
    let sec = store
        .get(&SecretRef {
            workspace: ws,
            name: key,
        })
        .map_err(|e| match e {
            Error::Missing => "Not signed in to this server profile".to_owned(),
            other => other.to_string(),
        })?;
    String::from_utf8(sec.expose().to_vec()).map_err(|_| "Corrupt local session token".to_owned())
}

fn delete_session_token(root: &Path, profile: &ServerProfile) {
    if let Ok((ws, mut store)) = open_auth_secrets(root)
        && let Ok(key) = profile.secret_key_id()
    {
        let _ = store.delete(&SecretRef {
            workspace: ws,
            name: key,
        });
    }
}

fn save_profiles(root: &Path, store: ServerProfileStore) -> Result<ServerProfileStore, String> {
    let mut journal = ServerProfileJournal::open(root).map_err(|e| e.to_string())?;
    journal.save(store.clone()).map_err(|e| e.to_string())?;
    Ok(store)
}

fn get_profile(
    root: &Path,
    profile_id: &str,
) -> Result<(ServerProfileStore, ServerProfile), String> {
    let store = load_server_profiles(root);
    let prof = store
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .cloned()
        .ok_or_else(|| "Server profile not found".to_owned())?;
    Ok((store, prof))
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_firebase_browser_login(
    runtime: &tokio::runtime::Runtime,
    root: PathBuf,
    downloads: PathBuf,
    profile_id: String,
    bootstrap_token: Option<String>,
    cancel: CancellationToken,
    outgoing: SyncSender<Event>,
    wake: Arc<dyn Fn() + Send + Sync>,
) {
    runtime.spawn(async move {
        let (mut store, mut prof) = match get_profile(&root, &profile_id) {
            Ok(v) => v,
            Err(m) => {
                let _ = outgoing.send(Event::ActionError(m));
                wake();
                return;
            }
        };
        let result: Result<(AuthResponse, Capabilities), String> = async {
            let (caps, _) = server_call::<(), Capabilities>(
                &prof,
                HttpMethod::Get,
                "/v1/capabilities",
                None,
                None,
                &downloads,
            )
            .await?;
            if cancel.is_cancelled() {
                return Err("__CANCELLED__".into());
            }
            if caps.firebase_project_id.is_none() || caps.firebase_web_api_key.is_none() {
                return Err(
                    "Server does not have SWAGGEN_FIREBASE_PROJECT_ID and SWAGGEN_FIREBASE_WEB_API_KEY configured."
                        .into(),
                );
            }
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(|e| format!("Failed to bind local auth callback: {e}"))?;
            let port = listener
                .local_addr()
                .map_err(|e| e.to_string())?
                .port();
            let state_token = uuid::Uuid::new_v4().simple().to_string();
            let base = prof.base_url.trim_end_matches('/');
            let browser_url = format!(
                "{base}/v1/auth/firebase/browser?port={port}&state={state_token}"
            );
            open_system_browser(&browser_url)?;

            let (id_token, browser_display_name) =
                await_firebase_loopback_callback(listener, &state_token, &cancel).await?;

            if cancel.is_cancelled() {
                return Err("__CANCELLED__".into());
            }

            let req = FirebaseLoginRequest {
                id_token,
                display_name: browser_display_name.filter(|s| !s.trim().is_empty()),
                device_name: "Swaggen Desktop (Windows · Firebase)".into(),
                bootstrap_token,
            };
            let (auth, _) = server_call::<FirebaseLoginRequest, AuthResponse>(
                &prof,
                HttpMethod::Post,
                "/v1/auth/firebase",
                None,
                Some(&req),
                &downloads,
            )
            .await?;
            Ok((auth, caps))
        }
        .await;

        let ev = match result {
            Ok((auth, caps)) => {
                if let Err(e) = save_session_token(&root, &prof, &auth.session_token) {
                    Event::ActionError(e)
                } else {
                    prof.email = auth.user.email.clone();
                    prof.user_id = auth.user.user_id.clone();
                    prof.display_name = auth.user.display_name.clone();
                    prof.session_id = auth.session_id.clone();
                    prof.session_expires_unix = auth.expires_at_unix;
                    prof.bootstrap_required = false;
                    prof.mongo_ready = caps.mongo_ready;
                    prof.last_server_version = caps.server_version;
                    prof.firebase_project_id = caps.firebase_project_id;
                    prof.firebase_web_api_key = caps.firebase_web_api_key;
                    prof.firebase_auth_domain = caps.firebase_auth_domain;
                    prof.last_error.clear();
                    store.upsert(prof);
                    match save_profiles(&root, store) {
                        Ok(saved) => Event::ServerProfilesUpdated {
                            store: saved,
                            message: format!("Signed in via Firebase as {}", auth.user.email),
                        },
                        Err(e) => Event::ActionError(e),
                    }
                }
            }
            Err(msg) if msg == "__CANCELLED__" || cancel.is_cancelled() => {
                return;
            }
            Err(msg) => Event::ActionError(msg),
        };
        let _ = outgoing.send(ev);
        wake();
    });
}

#[allow(clippy::result_large_err)]
pub fn handle_server_command(
    runtime: &tokio::runtime::Runtime,
    root: &Path,
    downloads: &Path,
    cmd: Command,
) -> Result<Event, Command> {
    match cmd {
        Command::SaveServerProfiles { store } => Ok(match save_profiles(root, store) {
            Ok(saved) => Event::ServerProfilesUpdated {
                store: saved,
                message: "Server profiles saved locally".into(),
            },
            Err(msg) => Event::ActionError(msg),
        }),
        Command::TestServerConnection { profile_id } => {
            let (mut store, mut prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let res: Result<(Capabilities, u64), String> = (|| {
                let (_, _, is_tls) = validate_server_url(&prof.base_url)?;
                let (caps, latency) = runtime.block_on(server_call::<(), Capabilities>(
                    &prof,
                    HttpMethod::Get,
                    "/v1/capabilities",
                    None,
                    None,
                    downloads,
                ))?;
                if caps.api_version < prof.expected_min_api_version {
                    return Err(format!(
                        "Server API v{} is older than required v{}",
                        caps.api_version, prof.expected_min_api_version
                    ));
                }
                prof.last_tls_status = if is_tls {
                    if prof.allow_self_signed_tls {
                        "TLS verified (host-scoped exception)".into()
                    } else {
                        "TLS verified (system roots)".into()
                    }
                } else {
                    "Loopback HTTP dev transport".into()
                };
                Ok((caps, latency))
            })();
            prof.last_checked_unix = time::OffsetDateTime::now_utc().unix_timestamp();
            let message = match res {
                Ok((caps, latency)) => {
                    prof.last_server_version = caps.server_version.clone();
                    prof.last_latency_ms = Some(latency);
                    prof.last_error.clear();
                    prof.bootstrap_required = caps.bootstrap_required;
                    prof.mongo_ready = caps.mongo_ready;
                    prof.firebase_project_id = caps.firebase_project_id.clone();
                    prof.firebase_web_api_key = caps.firebase_web_api_key.clone();
                    prof.firebase_auth_domain = caps.firebase_auth_domain.clone();
                    let fb_note = if caps.firebase_project_id.is_some() {
                        " · Firebase Auth active"
                    } else {
                        ""
                    };
                    format!(
                        "Connected to {} (server v{}, {} ms{fb_note})",
                        prof.name, caps.server_version, latency
                    )
                }
                Err(err) => {
                    prof.last_error = err.clone();
                    format!("Connection failed: {err}")
                }
            };
            store.upsert(prof);
            Ok(match save_profiles(root, store) {
                Ok(saved) => Event::ServerProfilesUpdated {
                    store: saved,
                    message,
                },
                Err(e) => Event::ActionError(e),
            })
        }
        Command::FirebaseEmailAuth {
            profile_id,
            email,
            password,
            display_name,
            create_account,
            bootstrap_token,
        } => {
            let (mut store, mut prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let result: Result<(AuthResponse, Capabilities), String> = runtime.block_on(async {
                let (caps, _) = server_call::<(), Capabilities>(
                    &prof,
                    HttpMethod::Get,
                    "/v1/capabilities",
                    None,
                    None,
                    downloads,
                )
                .await?;
                let api_key = caps.firebase_web_api_key.as_deref().ok_or_else(|| {
                    "Server does not expose SWAGGEN_FIREBASE_WEB_API_KEY in /v1/capabilities."
                        .to_owned()
                })?;
                let id_token = firebase_rest_email_signin(
                    api_key,
                    &email,
                    &password,
                    create_account,
                    downloads,
                )
                .await?;
                let req = FirebaseLoginRequest {
                    id_token,
                    display_name: if display_name.trim().is_empty() {
                        None
                    } else {
                        Some(display_name)
                    },
                    device_name: "Swaggen Desktop (Windows · Firebase)".into(),
                    bootstrap_token,
                };
                let (auth, _) = server_call::<FirebaseLoginRequest, AuthResponse>(
                    &prof,
                    HttpMethod::Post,
                    "/v1/auth/firebase",
                    None,
                    Some(&req),
                    downloads,
                )
                .await?;
                Ok((auth, caps))
            });
            match result {
                Ok((auth, caps)) => {
                    if let Err(e) = save_session_token(root, &prof, &auth.session_token) {
                        return Ok(Event::ActionError(e));
                    }
                    prof.email = auth.user.email.clone();
                    prof.user_id = auth.user.user_id.clone();
                    prof.display_name = auth.user.display_name.clone();
                    prof.session_id = auth.session_id.clone();
                    prof.session_expires_unix = auth.expires_at_unix;
                    prof.bootstrap_required = false;
                    prof.mongo_ready = caps.mongo_ready;
                    prof.last_server_version = caps.server_version;
                    prof.firebase_project_id = caps.firebase_project_id;
                    prof.firebase_web_api_key = caps.firebase_web_api_key;
                    prof.firebase_auth_domain = caps.firebase_auth_domain;
                    prof.last_error.clear();
                    store.upsert(prof);
                    Ok(match save_profiles(root, store) {
                        Ok(saved) => Event::ServerProfilesUpdated {
                            store: saved,
                            message: format!("Signed in via Firebase as {}", auth.user.email),
                        },
                        Err(e) => Event::ActionError(e),
                    })
                }
                Err(msg) => Ok(Event::ActionError(msg)),
            }
        }
        Command::BootstrapServerOwner {
            profile_id,
            email,
            display_name,
            password,
            bootstrap_token,
        } => {
            let (mut store, mut prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let body = BootstrapOwnerRequest {
                email,
                display_name,
                password,
                device_name: "Swaggen Desktop Windows".into(),
                bootstrap_token,
            };
            let res = runtime.block_on(server_call::<BootstrapOwnerRequest, AuthResponse>(
                &prof,
                HttpMethod::Post,
                "/v1/auth/bootstrap-owner",
                None,
                Some(&body),
                downloads,
            ));
            Ok(match res {
                Ok((auth, _)) => {
                    if let Err(e) = save_session_token(root, &prof, &auth.session_token) {
                        return Ok(Event::ActionError(e));
                    }
                    prof.email = auth.user.email.clone();
                    prof.user_id = auth.user.user_id;
                    prof.display_name = auth.user.display_name;
                    prof.session_id = auth.session_id;
                    prof.session_expires_unix = auth.expires_at_unix;
                    prof.bootstrap_required = false;
                    prof.last_error.clear();
                    store.upsert(prof);
                    match save_profiles(root, store) {
                        Ok(saved) => Event::ServerProfilesUpdated {
                            store: saved,
                            message: format!(
                                "Owner initialized and signed in as {}",
                                auth.user.email
                            ),
                        },
                        Err(e) => Event::ActionError(e),
                    }
                }
                Err(err) => Event::ActionError(err),
            })
        }
        Command::ServerLogin {
            profile_id,
            email,
            password,
        } => {
            let (mut store, mut prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let body = LoginRequest {
                email,
                password,
                device_name: "Swaggen Desktop Windows".into(),
            };
            let res = runtime.block_on(server_call::<LoginRequest, AuthResponse>(
                &prof,
                HttpMethod::Post,
                "/v1/auth/login",
                None,
                Some(&body),
                downloads,
            ));
            Ok(match res {
                Ok((auth, _)) => {
                    if let Err(e) = save_session_token(root, &prof, &auth.session_token) {
                        return Ok(Event::ActionError(e));
                    }
                    prof.email = auth.user.email.clone();
                    prof.user_id = auth.user.user_id;
                    prof.display_name = auth.user.display_name;
                    prof.session_id = auth.session_id;
                    prof.session_expires_unix = auth.expires_at_unix;
                    prof.last_error.clear();
                    store.upsert(prof);
                    match save_profiles(root, store) {
                        Ok(saved) => Event::ServerProfilesUpdated {
                            store: saved,
                            message: format!("Signed in as {}", auth.user.email),
                        },
                        Err(e) => Event::ActionError(e),
                    }
                }
                Err(err) => Event::ActionError(err),
            })
        }
        Command::AcceptServerInvitation {
            profile_id,
            invite_token,
            email,
            display_name,
            password,
        } => {
            let (mut store, mut prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let existing_tok = read_session_token(root, &prof).ok();
            let body = AcceptInvitationRequest {
                invite_token,
                email,
                display_name,
                password,
                device_name: "Swaggen Desktop Windows".into(),
            };
            let res = runtime.block_on(server_call::<
                AcceptInvitationRequest,
                AcceptInvitationResponse,
            >(
                &prof,
                HttpMethod::Post,
                "/v1/auth/invitations/accept",
                existing_tok.as_deref(),
                Some(&body),
                downloads,
            ));
            Ok(match res {
                Ok((joined, _)) => {
                    if !joined.session_token.is_empty()
                        && let Err(e) = save_session_token(root, &prof, &joined.session_token)
                    {
                        return Ok(Event::ActionError(e));
                    }
                    prof.email = joined.user.email.clone();
                    prof.user_id = joined.user.user_id;
                    prof.display_name = joined.user.display_name;
                    prof.session_id = joined.session_id;
                    prof.session_expires_unix = joined.expires_at_unix;
                    store.upsert(prof);
                    match save_profiles(root, store) {
                        Ok(saved) => Event::ServerProfilesUpdated {
                            store: saved,
                            message: format!(
                                "Joined workspace '{}' as {}",
                                joined.workspace.name,
                                joined.workspace.role.as_str()
                            ),
                        },
                        Err(e) => Event::ActionError(e),
                    }
                }
                Err(err) => Event::ActionError(err),
            })
        }
        Command::RefreshPendingInvitations { profile_id } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            Ok(
                match runtime.block_on(server_call::<(), PendingInvitationsResponse>(
                    &prof,
                    HttpMethod::Get,
                    "/v1/auth/invitations/pending",
                    Some(&tok),
                    None,
                    downloads,
                )) {
                    Ok((resp, _)) => Event::PendingInvitationsLoaded(resp.invitations),
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::AcceptInAppInvitation {
            profile_id,
            invitation_id,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let path = format!("/v1/auth/invitations/{invitation_id}/accept");
            let body = AcceptDirectInvitationRequest {
                device_name: "Swaggen Desktop Windows".into(),
            };
            Ok(
                match runtime.block_on(server_call::<
                    AcceptDirectInvitationRequest,
                    AcceptInvitationResponse,
                >(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    Some(&body),
                    downloads,
                )) {
                    Ok((joined, _)) => Event::InAppInvitationAccepted(joined),
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::DeclineInAppInvitation {
            profile_id,
            invitation_id,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let path = format!("/v1/auth/invitations/{invitation_id}/decline");
            Ok(
                match runtime.block_on(server_call_empty::<()>(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    None,
                    downloads,
                )) {
                    Ok(()) => Event::FileSaved {
                        message: "Declined workspace invitation".into(),
                    },
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::ServerLogout { profile_id } => {
            let (mut store, mut prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            if let Ok(tok) = read_session_token(root, &prof) {
                let _ = runtime.block_on(server_call_empty::<()>(
                    &prof,
                    HttpMethod::Post,
                    "/v1/auth/logout",
                    Some(&tok),
                    None,
                    downloads,
                ));
            }
            delete_session_token(root, &prof);
            prof.session_id.clear();
            prof.email.clear();
            prof.user_id.clear();
            prof.display_name.clear();
            prof.last_error.clear();
            store.upsert(prof);
            Ok(match save_profiles(root, store) {
                Ok(saved) => Event::ServerProfilesUpdated {
                    store: saved,
                    message: "Signed out and session cleared from DPAPI".into(),
                },
                Err(e) => Event::ActionError(e),
            })
        }
        Command::ListServerSessions { profile_id } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            Ok(
                match runtime.block_on(server_call::<(), SessionListResponse>(
                    &prof,
                    HttpMethod::Get,
                    "/v1/auth/sessions",
                    Some(&tok),
                    None,
                    downloads,
                )) {
                    Ok((resp, _)) => Event::ServerSessionsLoaded(resp.sessions),
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::RevokeServerSession {
            profile_id,
            session_id,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let path = format!("/v1/auth/sessions/{session_id}/revoke");
            Ok(
                match runtime.block_on(server_call_empty::<()>(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    None,
                    downloads,
                )) {
                    Ok(()) => Event::FileSaved {
                        message: "Device session revoked".into(),
                    },
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::ServerChangePassword {
            profile_id,
            current_password,
            new_password,
            revoke_other_sessions,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let body = ChangePasswordRequest {
                current_password,
                new_password,
                revoke_other_sessions,
            };
            Ok(
                match runtime.block_on(server_call_empty(
                    &prof,
                    HttpMethod::Post,
                    "/v1/auth/change-password",
                    Some(&tok),
                    Some(&body),
                    downloads,
                )) {
                    Ok(()) => Event::FileSaved {
                        message: "Password updated".into(),
                    },
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::RefreshTeamWorkspaces { profile_id } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            Ok(
                match runtime.block_on(server_call::<(), WorkspaceListResponse>(
                    &prof,
                    HttpMethod::Get,
                    "/v1/workspaces",
                    Some(&tok),
                    None,
                    downloads,
                )) {
                    Ok((resp, _)) => Event::TeamWorkspacesLoaded(resp.workspaces),
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::CreateTeamWorkspace {
            profile_id,
            slug,
            name,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let body = CreateWorkspaceRequest {
                slug,
                name,
                description: String::new(),
            };
            Ok(
                match runtime.block_on(server_call::<CreateWorkspaceRequest, TeamWorkspaceSummary>(
                    &prof,
                    HttpMethod::Post,
                    "/v1/workspaces",
                    Some(&tok),
                    Some(&body),
                    downloads,
                )) {
                    Ok((ws, _)) => match runtime.block_on(server_call::<(), WorkspaceListResponse>(
                        &prof,
                        HttpMethod::Get,
                        "/v1/workspaces",
                        Some(&tok),
                        None,
                        downloads,
                    )) {
                        Ok((resp, _)) => Event::TeamWorkspacesLoaded(resp.workspaces),
                        Err(_) => Event::TeamWorkspacesLoaded(vec![ws]),
                    },
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::LoadTeamWorkspaceDetails {
            profile_id,
            workspace_id,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let members_path = format!("/v1/workspaces/{workspace_id}/members");
            let members = match runtime.block_on(server_call::<(), MemberListResponse>(
                &prof,
                HttpMethod::Get,
                &members_path,
                Some(&tok),
                None,
                downloads,
            )) {
                Ok((m, _)) => m,
                Err(e) => return Ok(Event::ActionError(e)),
            };
            let audit_path = format!("/v1/workspaces/{workspace_id}/audit");
            let audit: Vec<AuditEventRecord> = runtime
                .block_on(server_call::<(), AuditListResponse>(
                    &prof,
                    HttpMethod::Get,
                    &audit_path,
                    Some(&tok),
                    None,
                    downloads,
                ))
                .map(|(a, _)| a.events)
                .unwrap_or_default();
            let links_path = format!("/v1/workspaces/{workspace_id}/invite-links");
            let public_links = runtime
                .block_on(server_call::<(), PublicInviteLinkListResponse>(
                    &prof,
                    HttpMethod::Get,
                    &links_path,
                    Some(&tok),
                    None,
                    downloads,
                ))
                .map(|(r, _)| r.links)
                .unwrap_or_default();
            Ok(Event::TeamWorkspaceDetailsLoaded {
                members,
                audit,
                public_links,
            })
        }
        Command::CreateServerInvitation {
            profile_id,
            workspace_id,
            email,
            role,
            expires_in_hours,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let body = CreateInvitationRequest {
                email,
                role,
                expires_in_hours,
            };
            let path = format!("/v1/workspaces/{workspace_id}/invitations");
            Ok(
                match runtime.block_on(server_call::<
                    CreateInvitationRequest,
                    CreateInvitationResponse,
                >(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    Some(&body),
                    downloads,
                )) {
                    Ok((inv, _)) => Event::InvitationCreated(inv),
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::RevokeServerInvitation {
            profile_id,
            workspace_id,
            invitation_id,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let path = format!("/v1/workspaces/{workspace_id}/invitations/{invitation_id}/revoke");
            Ok(
                match runtime.block_on(server_call_empty::<()>(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    None,
                    downloads,
                )) {
                    Ok(()) => Event::FileSaved {
                        message: "Invitation revoked".into(),
                    },
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::CreatePublicInviteLink {
            profile_id,
            workspace_id,
            role,
            expires_in_hours,
            max_uses,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let body = CreatePublicInviteLinkRequest {
                role,
                expires_in_hours,
                max_uses,
            };
            let path = format!("/v1/workspaces/{workspace_id}/invite-links");
            Ok(
                match runtime.block_on(server_call::<
                    CreatePublicInviteLinkRequest,
                    CreatePublicInviteLinkResponse,
                >(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    Some(&body),
                    downloads,
                )) {
                    Ok((resp, _)) => Event::PublicInviteLinkCreated(resp.link),
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::LoadPublicInviteLinks {
            profile_id,
            workspace_id,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let path = format!("/v1/workspaces/{workspace_id}/invite-links");
            Ok(
                match runtime.block_on(server_call::<(), PublicInviteLinkListResponse>(
                    &prof,
                    HttpMethod::Get,
                    &path,
                    Some(&tok),
                    None,
                    downloads,
                )) {
                    Ok((resp, _)) => Event::PublicInviteLinksLoaded(resp.links),
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::RevokePublicInviteLink {
            profile_id,
            workspace_id,
            link_id,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let path = format!("/v1/workspaces/{workspace_id}/invite-links/{link_id}/revoke");
            Ok(
                match runtime.block_on(server_call_empty::<()>(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    None,
                    downloads,
                )) {
                    Ok(()) => Event::FileSaved {
                        message: "Public invite link revoked".into(),
                    },
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::JoinByPublicInviteLink {
            profile_id,
            link_or_code,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let body = JoinByInviteLinkRequest {
                link_or_code,
                device_name: "Swaggen Desktop Windows".into(),
            };
            Ok(
                match runtime.block_on(server_call::<
                    JoinByInviteLinkRequest,
                    AcceptInvitationResponse,
                >(
                    &prof,
                    HttpMethod::Post,
                    "/v1/auth/invite-links/join",
                    Some(&tok),
                    Some(&body),
                    downloads,
                )) {
                    Ok((joined, _)) => Event::InAppInvitationAccepted(joined),
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::UpdateMemberRole {
            profile_id,
            workspace_id,
            user_id,
            role,
            expected_membership_revision,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let body = UpdateMemberRoleRequest {
                role,
                expected_membership_revision,
            };
            let path = format!("/v1/workspaces/{workspace_id}/members/{user_id}/role");
            Ok(
                match runtime.block_on(server_call::<UpdateMemberRoleRequest, serde_json::Value>(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    Some(&body),
                    downloads,
                )) {
                    Ok(_) => Event::FileSaved {
                        message: format!("Member role updated to {}", role.as_str()),
                    },
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        Command::RemoveWorkspaceMember {
            profile_id,
            workspace_id,
            user_id,
        } => {
            let (_, prof) = match get_profile(root, &profile_id) {
                Ok(v) => v,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let tok = match read_session_token(root, &prof) {
                Ok(t) => t,
                Err(m) => return Ok(Event::ActionError(m)),
            };
            let path = format!("/v1/workspaces/{workspace_id}/members/{user_id}/remove");
            Ok(
                match runtime.block_on(server_call::<(), MemberListResponse>(
                    &prof,
                    HttpMethod::Post,
                    &path,
                    Some(&tok),
                    None,
                    downloads,
                )) {
                    Ok((members, _)) => Event::WorkspaceMembersUpdated(members),
                    Err(e) => Event::ActionError(e),
                },
            )
        }
        other => Err(other),
    }
}

fn open_system_browser(url: &str) -> Result<(), String> {
    let valid = url.starts_with("https://")
        || url.starts_with("http://127.0.0.1")
        || url.starts_with("http://localhost");
    if !valid {
        return Err("Refusing to open non-HTTPS sign-in URL in browser.".into());
    }
    #[cfg(target_os = "windows")]
    {
        // Use rundll32 url.dll directly (NOT cmd /C start) so '&' in query strings
        // like ?port=53124&state=... is NEVER split by cmd.exe!
        std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
            .map_err(|e| format!("Failed to open default browser: {e}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Failed to open default browser: {e}"))?;
        Ok(())
    }
}

#[derive(serde::Deserialize)]
struct FirebaseCallbackPayload {
    id_token: String,
    state: String,
    #[serde(default)]
    display_name: Option<String>,
}

async fn await_firebase_loopback_callback(
    listener: tokio::net::TcpListener,
    expected_state: &str,
    cancel: &CancellationToken,
) -> Result<(String, Option<String>), String> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(180);
    loop {
        if cancel.is_cancelled() {
            return Err("__CANCELLED__".into());
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(
                "Timed out waiting for Firebase sign-in in browser (180s). Please try again."
                    .into(),
            );
        }
        let accept_res = tokio::select! {
            _ = cancel.cancelled() => return Err("__CANCELLED__".into()),
            res = tokio::time::timeout(remaining, listener.accept()) => {
                res.map_err(|_| {
                    "Timed out waiting for Firebase sign-in in browser (180s). Please try again."
                        .to_owned()
                })?
            }
        };
        let (mut sock, _) = accept_res.map_err(|e| format!("Loopback callback error: {e}"))?;

        let mut buf = Vec::with_capacity(4096);
        let mut tmp = [0u8; 4096];
        let mut header_end: Option<usize> = None;
        let mut content_len: usize = 0;

        loop {
            let n = tokio::time::timeout(std::time::Duration::from_secs(5), sock.read(&mut tmp))
                .await
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or(0);
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            if buf.len() > 65_536 {
                break;
            }
            if header_end.is_none()
                && let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n")
            {
                let end = pos + 4;
                header_end = Some(end);
                let head_str = String::from_utf8_lossy(&buf[..pos]);
                for line in head_str.lines().skip(1) {
                    if let Some((k, v)) = line.split_once(':')
                        && k.trim().eq_ignore_ascii_case("content-length")
                    {
                        content_len = v.trim().parse::<usize>().unwrap_or(0);
                    }
                }
            }
            if let Some(end) = header_end
                && buf.len() >= end + content_len
            {
                break;
            }
        }

        let Some(end) = header_end else {
            continue;
        };
        let head = String::from_utf8_lossy(&buf[..end]);
        let first_line = head.lines().next().unwrap_or("");
        let cors = "Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n";

        if first_line.starts_with("OPTIONS ") {
            let resp = format!("HTTP/1.1 204 No Content\r\n{cors}Content-Length: 0\r\n\r\n");
            let _ = sock.write_all(resp.as_bytes()).await;
            continue;
        }
        if !first_line.starts_with("POST /callback") {
            let body = r#"{"error":"not_found"}"#;
            let resp = format!(
                "HTTP/1.1 404 Not Found\r\n{cors}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let _ = sock.write_all(resp.as_bytes()).await;
            continue;
        }

        let body_bytes = &buf[end..buf.len().min(end + content_len)];
        match serde_json::from_slice::<FirebaseCallbackPayload>(body_bytes) {
            Ok(payload)
                if payload.state == expected_state && !payload.id_token.trim().is_empty() =>
            {
                let ok_body = r#"{"ok":true}"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\n{cors}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{ok_body}",
                    ok_body.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                return Ok((payload.id_token, payload.display_name));
            }
            _ => {
                let err_body = r#"{"error":"invalid_state_or_token"}"#;
                let resp = format!(
                    "HTTP/1.1 400 Bad Request\r\n{cors}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{err_body}",
                    err_body.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                return Err(
                    "Firebase callback rejected due to invalid state or missing ID token.".into(),
                );
            }
        }
    }
}

async fn firebase_rest_email_signin(
    api_key: &str,
    email: &str,
    password: &str,
    create_account: bool,
    scratch_dir: &Path,
) -> Result<String, String> {
    let endpoint = if create_account {
        "accounts:signUp"
    } else {
        "accounts:signInWithPassword"
    };
    let url = format!("https://identitytoolkit.googleapis.com/v1/{endpoint}?key={api_key}");
    let payload = serde_json::json!({
        "email": email.trim(),
        "password": password,
        "returnSecureToken": true
    })
    .to_string();

    let headers = vec![
        ResolvedHeader {
            name: "Content-Type".into(),
            value: "application/json".into(),
            generated: true,
        },
        ResolvedHeader {
            name: "Accept".into(),
            value: "application/json".into(),
            generated: true,
        },
    ];
    let resolved = ResolvedRequest {
        method: HttpMethod::Post,
        url: url.clone(),
        masked_url: format!("https://identitytoolkit.googleapis.com/v1/{endpoint}?key=••••"),
        headers: headers.clone(),
        masked_headers: headers,
        body: RequestBody::Json(payload),
        masked_body_preview: String::new(),
        auth_source: "FirebaseIdentityToolkit".into(),
        warnings: Vec::new(),
        blocking_errors: Vec::new(),
        requires_production_confirm: false,
        connection: RequestConnectionPolicy {
            timeout_ms: 15_000,
            follow_redirects: false,
            max_redirects: 0,
            send_auth_on_cross_host_redirect: false,
            ..Default::default()
        },
    };

    let cancel = CancellationToken::new();
    let resp = execute_request(&resolved, scratch_dir, cancel)
        .await
        .map_err(|e| format!("Firebase Identity Toolkit request failed: {}", e.message))?;

    let val: serde_json::Value =
        serde_json::from_slice(&resp.preview_bytes).unwrap_or_else(|_| serde_json::json!({}));
    if !(200..300).contains(&resp.status_code) {
        let fb_msg = val
            .pointer("/error/message")
            .and_then(|v| v.as_str())
            .unwrap_or("Firebase authentication rejected");
        return Err(format!(
            "Firebase Auth error ({}): {fb_msg}",
            resp.status_code
        ));
    }
    val.get("idToken")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "Firebase response did not contain an idToken.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::Worker;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn worker_saves_profiles_tests_connection_and_isolates_token_in_dpapi() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let addr = rt.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            tokio::spawn(async move {
                for i in 0..2 {
                    if let Ok((mut sock, _)) = listener.accept().await {
                        let mut buf = vec![0u8; 4096];
                        let _ = sock.read(&mut buf).await;
                        let body = if i == 0 {
                            r#"{"api_version":1,"server_version":"0.1.0","min_desktop_api_version":1,"collaboration_enabled":true,"auth_modes":["internal"],"bootstrap_required":true,"mongo_ready":true,"migration_version":1,"max_body_bytes":2097152}"#
                        } else {
                            r#"{"api_version":1,"user":{"user_id":"u1","email":"owner@example.test","display_name":"Owner","is_server_owner":true,"created_at_unix":1000},"session_id":"s1","session_token":"swg_dpapi_secret_token_999","expires_at_unix":9999999}"#
                        };
                        let status = if i == 0 { "200 OK" } else { "201 Created" };
                        let resp = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = sock.write_all(resp.as_bytes()).await;
                    }
                }
            });
            addr
        });

        let root = std::env::temp_dir().join(format!(
            "swaggen-p04-srv-worker-{}-{}",
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

        let mut store = ServerProfileStore::default();
        store.upsert(ServerProfile::new(
            "local-dev",
            "Local Dev",
            format!("http://{addr}"),
            false,
        ));
        worker
            .commands
            .send(Command::SaveServerProfiles { store })
            .unwrap();
        assert!(matches!(
            worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::ServerProfilesUpdated { .. }
        ));

        worker
            .commands
            .send(Command::TestServerConnection {
                profile_id: "local-dev".into(),
            })
            .unwrap();
        match worker.events.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::ServerProfilesUpdated { store, .. } => {
                assert!(store.active().unwrap().bootstrap_required);
                assert!(store.active().unwrap().mongo_ready);
            }
            _ => panic!("expected ServerProfilesUpdated"),
        }

        worker
            .commands
            .send(Command::BootstrapServerOwner {
                profile_id: "local-dev".into(),
                email: "owner@example.test".into(),
                display_name: "Owner".into(),
                password: "StrongPassword!123".into(),
                bootstrap_token: None,
            })
            .unwrap();
        match worker.events.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::ServerProfilesUpdated { store, .. } => {
                assert_eq!(store.active().unwrap().email, "owner@example.test");
            }
            Event::ActionError(msg) => panic!("ActionError during bootstrap: {msg}"),
            _ => panic!("expected ServerProfilesUpdated"),
        }

        // Security invariant: raw session token MUST NOT exist in servers.journal!
        let servers_bytes = std::fs::read(root.join("servers.journal")).unwrap();
        assert!(
            !String::from_utf8_lossy(&servers_bytes).contains("swg_dpapi_secret_token_999"),
            "Plaintext session token leaked into servers.journal"
        );
        // Must exist inside DPAPI SecretStore instead
        let tok = read_session_token(&root, &load_server_profiles(&root).profiles[0]).unwrap();
        assert_eq!(tok, "swg_dpapi_secret_token_999");

        drop(worker);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn empty_workspace_seeds_default_production_server_profile() {
        let root = std::env::temp_dir().join(format!(
            "swaggen-default-srv-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = load_server_profiles(&root);
        let active = store
            .active()
            .expect("default production profile should be active");
        assert_eq!(active.base_url, DEFAULT_SERVER_URL);
        assert!(!active.allow_self_signed_tls);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn firebase_loopback_callback_accepts_matching_state_and_rejects_mismatch() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 1. Valid state + OPTIONS preflight + POST /callback
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let client = tokio::spawn(async move {
                let mut s1 = tokio::net::TcpStream::connect(addr).await.unwrap();
                s1.write_all(b"OPTIONS /callback HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
                    .await
                    .unwrap();
                let mut r1 = vec![0u8; 512];
                let n1 = s1.read(&mut r1).await.unwrap();
                assert!(String::from_utf8_lossy(&r1[..n1]).contains("204 No Content"));

                let mut s2 = tokio::net::TcpStream::connect(addr).await.unwrap();
                let body = r#"{"id_token":"fb.jwt.token","state":"state-ok","display_name":"Minh Tran"}"#;
                let req = format!(
                    "POST /callback HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                s2.write_all(req.as_bytes()).await.unwrap();
                let mut r2 = vec![0u8; 512];
                let n2 = s2.read(&mut r2).await.unwrap();
                assert!(String::from_utf8_lossy(&r2[..n2]).contains("200 OK"));
            });
            let cancel = CancellationToken::new();
            let (tok, name) = await_firebase_loopback_callback(listener, "state-ok", &cancel)
                .await
                .unwrap();
            assert_eq!(tok, "fb.jwt.token");
            assert_eq!(name.as_deref(), Some("Minh Tran"));
            client.await.unwrap();

            // 2. Mismatched state is rejected with error
            let listener2 = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr2 = listener2.local_addr().unwrap();
            let client2 = tokio::spawn(async move {
                let mut s = tokio::net::TcpStream::connect(addr2).await.unwrap();
                let body = r#"{"id_token":"fb.jwt.token","state":"wrong-state"}"#;
                let req = format!(
                    "POST /callback HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                s.write_all(req.as_bytes()).await.unwrap();
                let mut r = vec![0u8; 512];
                let n = s.read(&mut r).await.unwrap();
                assert!(String::from_utf8_lossy(&r[..n]).contains("400 Bad Request"));
            });
            let err = await_firebase_loopback_callback(listener2, "expected-state", &cancel)
                .await
                .unwrap_err();
            assert!(err.contains("invalid state"));
            client2.await.unwrap();
        });
    }

    #[test]
    fn firebase_loopback_callback_cancels_immediately() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let cancel = CancellationToken::new();
            let cancel_clone = cancel.clone();
            let started = std::time::Instant::now();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(30)).await;
                cancel_clone.cancel();
            });
            let err = await_firebase_loopback_callback(listener, "any-state", &cancel)
                .await
                .unwrap_err();
            assert_eq!(err, "__CANCELLED__");
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "loopback callback must cancel immediately instead of waiting 180s"
            );
        });
    }
}
