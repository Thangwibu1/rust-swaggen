//! Server HTTP + DPAPI session + ServerProfileJournal command handlers for the desktop worker.
use crate::{
    server_client::{server_call, server_call_empty},
    worker::{Command, Event},
};
use std::path::Path;
use swaggen_api_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AuditEventRecord, AuditListResponse,
    AuthResponse, BootstrapOwnerRequest, Capabilities, ChangePasswordRequest,
    CreateInvitationRequest, CreateInvitationResponse, CreateWorkspaceRequest, LoginRequest,
    MemberListResponse, SessionListResponse, TeamWorkspaceSummary, UpdateMemberRoleRequest,
    WorkspaceListResponse,
};
use swaggen_domain::{
    Error, LocalId, Secret, SecretRef,
    server_profiles::{ServerProfile, ServerProfileStore, validate_server_url},
    workspace::HttpMethod,
};
use swaggen_local_store::{WorkspaceDirectory, server_profiles_store::ServerProfileJournal};
use swaggen_secret_store::SecretStore;

pub fn load_server_profiles(root: &Path) -> ServerProfileStore {
    ServerProfileJournal::open(root)
        .map(|j| j.store().clone())
        .unwrap_or_default()
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
        .put(&SecretRef { workspace: ws, name: key }, &secret)
        .map_err(|e| e.to_string())
}

pub fn read_session_token(root: &Path, profile: &ServerProfile) -> Result<String, String> {
    let (ws, store) = open_auth_secrets(root)?;
    let key = profile.secret_key_id().map_err(|e| e.to_string())?;
    let sec = store
        .get(&SecretRef { workspace: ws, name: key })
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
        let _ = store.delete(&SecretRef { workspace: ws, name: key });
    }
}

fn save_profiles(root: &Path, store: ServerProfileStore) -> Result<ServerProfileStore, String> {
    let mut journal = ServerProfileJournal::open(root).map_err(|e| e.to_string())?;
    journal.save(store.clone()).map_err(|e| e.to_string())?;
    Ok(store)
}

fn get_profile(root: &Path, profile_id: &str) -> Result<(ServerProfileStore, ServerProfile), String> {
    let store = load_server_profiles(root);
    let prof = store
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .cloned()
        .ok_or_else(|| "Server profile not found".to_owned())?;
    Ok((store, prof))
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
                    format!(
                        "Connected to {} (server v{}, {} ms)",
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
                            message: format!("Owner initialized and signed in as {}", auth.user.email),
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
            let res = runtime.block_on(server_call::<AcceptInvitationRequest, AcceptInvitationResponse>(
                &prof,
                HttpMethod::Post,
                "/v1/auth/invitations/accept",
                existing_tok.as_deref(),
                Some(&body),
                downloads,
            ));
            Ok(match res {
                Ok((joined, _)) => {
                    if let Err(e) = save_session_token(root, &prof, &joined.session_token) {
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
            Ok(Event::TeamWorkspaceDetailsLoaded { members, audit })
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
                match runtime.block_on(
                    server_call::<CreateInvitationRequest, CreateInvitationResponse>(
                        &prof,
                        HttpMethod::Post,
                        &path,
                        Some(&tok),
                        Some(&body),
                        downloads,
                    ),
                ) {
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
        other => Err(other),
    }
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
        store.upsert(ServerProfile {
            id: "local-dev".into(),
            name: "Local Dev".into(),
            base_url: format!("http://{addr}"),
            allow_self_signed_tls: false,
            expected_min_api_version: 1,
            email: String::new(),
            user_id: String::new(),
            display_name: String::new(),
            session_id: String::new(),
            session_expires_unix: 0,
            last_checked_unix: 0,
            last_server_version: String::new(),
            last_latency_ms: None,
            last_tls_status: String::new(),
            last_error: String::new(),
            bootstrap_required: false,
            mongo_ready: false,
        });
        worker.commands.send(Command::SaveServerProfiles { store }).unwrap();
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
}

