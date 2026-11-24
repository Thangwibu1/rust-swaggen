//! Bounded worker actor owning local storage, DPAPI secrets, and async HTTP runtime off the UI thread.
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender},
    },
};
use swaggen_domain::{
    Error, LocalId, Secret, SecretRef,
    dynamic_vars::apply_response_extractors,
    security::RedactionPolicy,
    workspace::{
        CookieRecord, HistoryEntry, RequestItem, VariableEntry, WorkspaceKind, WorkspaceState,
        folder_chain, new_uid, resolve_request_preview,
    },
};
use swaggen_http_engine::{
    HttpExecutionError, HttpResponseData, TransferProgress, execute_request,
    execute_request_with_progress,
    oauth2::{OAuthTokenSet, PkceChallenge, await_loopback_callback, exchange_token_form},
};
use swaggen_local_store::{
    Snapshot, Store, WorkspaceDirectory, quarantine_corrupt_draft,
    workspace_store::{StorageStats, WorkspaceStore, list_workspaces, migrate_workspace_root},
};
use swaggen_secret_store::SecretStore;
use swaggen_spec_engine::openapi::{SpecDocument, parse_and_validate};
use tokio_util::sync::CancellationToken;

pub enum Command {
    /// Drop uncertain handles and replay durable data before accepting more writes.
    Reopen,
    /// Explicitly quarantine a corrupt/unsupported draft journal after user confirmation.
    QuarantineCorruptDraft,
    /// Legacy P01 plain-text draft save.
    Save {
        generation: u64,
        draft: String,
    },
    /// Persist full P02 workspace state atomically.
    SaveWorkspace {
        generation: u64,
        state: Box<WorkspaceState>,
    },
    SaveSpecs {
        specs: Vec<SpecDocument>,
    },
    FetchSpecUrl {
        name: String,
        url: String,
    },
    SwitchWorkspace {
        id: LocalId,
    },
    CreateWorkspace {
        id: LocalId,
        name: String,
        kind: WorkspaceKind,
    },
    PutSecret {
        name: LocalId,
        value: Secret,
    },
    GetSecret {
        name: LocalId,
    },
    DeleteSecret {
        name: LocalId,
    },
    SendHttp {
        execution_id: u64,
        request: Box<RequestItem>,
        state: Box<WorkspaceState>,
    },
    CancelHttp {
        execution_id: u64,
    },
    DeleteRequestHistory {
        request_id: String,
    },
    ClearHistory,
    CleanCache,
    SaveResponseFile {
        source: PathBuf,
        destination: PathBuf,
    },
    ExportSanitizedWorkspace {
        destination: PathBuf,
        state: Box<WorkspaceState>,
    },
    MigrateDataRoot {
        target_root: PathBuf,
    },
    SaveServerProfiles {
        store: swaggen_domain::server_profiles::ServerProfileStore,
    },
    TestServerConnection {
        profile_id: String,
    },
    BootstrapServerOwner {
        profile_id: String,
        email: String,
        display_name: String,
        password: String,
        bootstrap_token: Option<String>,
    },
    ServerLogin {
        profile_id: String,
        email: String,
        password: String,
    },
    AcceptServerInvitation {
        profile_id: String,
        invite_token: String,
        email: String,
        display_name: String,
        password: Option<String>,
    },
    ListServerSessions {
        profile_id: String,
    },
    RevokeServerSession {
        profile_id: String,
        session_id: String,
    },
    ServerLogout {
        profile_id: String,
    },
    ServerChangePassword {
        profile_id: String,
        current_password: String,
        new_password: String,
        revoke_other_sessions: bool,
    },
    RefreshTeamWorkspaces {
        profile_id: String,
    },
    CreateTeamWorkspace {
        profile_id: String,
        slug: String,
        name: String,
    },
    LoadTeamWorkspaceDetails {
        profile_id: String,
        workspace_id: String,
    },
    CreateServerInvitation {
        profile_id: String,
        workspace_id: String,
        email: String,
        role: swaggen_api_contract::Role,
        expires_in_hours: u32,
    },
    RevokeServerInvitation {
        profile_id: String,
        workspace_id: String,
        invitation_id: String,
    },
    UpdateMemberRole {
        profile_id: String,
        workspace_id: String,
        user_id: String,
        role: swaggen_api_contract::Role,
        expected_membership_revision: i64,
    },
    StageAndPublishEntity {
        profile_id: String,
        workspace_id: String,
        entity_id: String,
        name: String,
        kind: String,
        payload_json: String,
        delete: bool,
        note: String,
    },
    SyncNow {
        profile_id: String,
        workspace_id: String,
    },
    ResolveSyncConflict {
        profile_id: String,
        workspace_id: String,
        operation_id: String,
        choice: swaggen_sync::ConflictChoice,
    },
    LoadEntityRevisions {
        profile_id: String,
        workspace_id: String,
        entity_id: String,
    },
    RestoreEntityRevision {
        profile_id: String,
        workspace_id: String,
        entity_id: String,
        target_revision: i64,
        expected_current_revision: i64,
    },
    LoadCollabData {
        profile_id: String,
        workspace_id: String,
    },
    CreateTeamComment {
        profile_id: String,
        workspace_id: String,
        entity_id: String,
        anchor_revision: i64,
        body: String,
    },
    ResolveTeamComment {
        profile_id: String,
        workspace_id: String,
        comment_id: String,
        resolved: bool,
    },
    MarkTeamNotificationsRead {
        profile_id: String,
        workspace_id: String,
    },
    RunOAuth2Pkce {
        auth_url: String,
        token_url: String,
        client_id: String,
        scope: String,
        downloads_dir: PathBuf,
    },
    RunCollection {
        state: Box<WorkspaceState>,
        specs: Vec<SpecDocument>,
        config: swaggen_runner::collection_runner::RunnerConfig,
    },
    CancelRunner,
    StartLocalMock {
        routes: Vec<swaggen_runner::local_mock::MockRoute>,
    },
    StopLocalMock,
    PollMockTraffic,
    ExecuteGraphQl {
        endpoint: String,
        query: String,
        variables_json: String,
    },
    StartSse {
        url: String,
        max_events: usize,
    },
    StopSse,
    #[doc(hidden)]
    InternalHttpFinished {
        execution_id: u64,
        request: Box<RequestItem>,
        resolved_url: String,
        now_secs: i64,
        result: Box<Result<HttpResponseData, HttpExecutionError>>,
    },
    #[doc(hidden)]
    Shutdown,
}

pub enum Event {
    Loaded {
        snapshot: Snapshot,
        workspace: WorkspaceState,
        specs: Vec<SpecDocument>,
        workspaces: Vec<LocalId>,
        history: Vec<HistoryEntry>,
        secret_names: Vec<LocalId>,
        storage_stats: StorageStats,
        server_profiles: swaggen_domain::server_profiles::ServerProfileStore,
    },
    Recovered {
        snapshot: Snapshot,
        workspace: WorkspaceState,
        specs: Vec<SpecDocument>,
        workspaces: Vec<LocalId>,
        history: Vec<HistoryEntry>,
        secret_names: Vec<LocalId>,
        storage_stats: StorageStats,
        server_profiles: swaggen_domain::server_profiles::ServerProfileStore,
    },
    ServerProfilesUpdated {
        store: swaggen_domain::server_profiles::ServerProfileStore,
        message: String,
    },
    ServerSessionsLoaded(Vec<swaggen_api_contract::SessionSummary>),
    TeamWorkspacesLoaded(Vec<swaggen_api_contract::TeamWorkspaceSummary>),
    TeamWorkspaceDetailsLoaded {
        members: swaggen_api_contract::MemberListResponse,
        audit: Vec<swaggen_api_contract::AuditEventRecord>,
    },
    InvitationCreated(swaggen_api_contract::CreateInvitationResponse),
    SyncStateUpdated {
        state: Box<swaggen_sync::TeamSyncState>,
        message: String,
    },
    EntityRevisionsLoaded(Vec<swaggen_api_contract::EntityRevisionRecord>),
    CollabDataLoaded {
        comments: Vec<swaggen_api_contract::CommentThreadRecord>,
        notifications: Vec<swaggen_api_contract::NotificationRecord>,
        unread_count: usize,
    },
    SpecsSaved,
    SpecFetched(Box<SpecDocument>),
    Saved {
        generation: u64,
        revision: u64,
    },
    WorkspaceSaved {
        generation: u64,
        revision: u64,
        storage_stats: StorageStats,
    },
    SecretSaved {
        secret_names: Vec<LocalId>,
    },
    SecretLoaded(Secret),
    SecretDeleted {
        name: LocalId,
        secret_names: Vec<LocalId>,
    },
    HttpCompleted {
        execution_id: u64,
        request_id: String,
        request_name: String,
        method: swaggen_domain::workspace::HttpMethod,
        resolved_url: String,
        snapshot: Box<RequestItem>,
        response: HttpResponseData,
        extracted: Vec<(String, String, bool)>,
        history: Vec<HistoryEntry>,
        storage_stats: StorageStats,
    },
    HttpProgress {
        execution_id: u64,
        phase: &'static str,
        bytes_done: u64,
        total_bytes: Option<u64>,
    },
    OAuth2PendingAuthorize {
        authorize_url: String,
        redirect_uri: String,
    },
    OAuth2TokenReceived(OAuthTokenSet),
    CollectionRunFinished(Box<swaggen_runner::collection_runner::CollectionRunReport>),
    LocalMockState {
        base_url: Option<String>,
        logs: Vec<swaggen_runner::local_mock::MockTrafficLog>,
    },
    GraphQlFinished(swaggen_runner::protocols_and_tools::GraphQlOutcome),
    SseEventReceived(swaggen_runner::protocols_and_tools::SseEventRecord),
    SseStopped(String),
    HttpFailed {
        execution_id: u64,
        request_id: String,
        request_name: String,
        method: swaggen_domain::workspace::HttpMethod,
        resolved_url: String,
        snapshot: RequestItem,
        error: HttpExecutionError,
        history: Vec<HistoryEntry>,
    },
    HistoryUpdated {
        history: Vec<HistoryEntry>,
        storage_stats: StorageStats,
    },
    CacheCleaned {
        removed_bytes: u64,
        history: Vec<HistoryEntry>,
        storage_stats: StorageStats,
    },
    FileSaved {
        message: String,
    },
    DataMigrated {
        new_root: PathBuf,
    },
    Failed(Error),
    SecretFailed {
        error: Error,
        value: Secret,
    },
    ActionError(String),
}

pub struct Worker {
    pub commands: SyncSender<Command>,
    pub events: Receiver<Event>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.commands.try_send(Command::Shutdown);
    }
}

struct Session {
    store: Store,
    ws_store: WorkspaceStore,
    secrets: SecretStore,
}

fn build_snapshot_event(s: &Session, root: &Path, workspace: &LocalId, recovered: bool) -> Event {
    let snapshot = s.store.snapshot();
    let ws = s.ws_store.state().clone();
    let specs = s.ws_store.specs().to_vec();
    let workspaces = list_workspaces(root).unwrap_or_else(|_| vec![workspace.clone()]);
    let history = s.ws_store.history().to_vec();
    let secret_names = s.secrets.list_names();
    let storage_stats = s.ws_store.storage_stats().unwrap_or(StorageStats {
        workspace_bytes: 0,
        history_bytes: 0,
        secrets_bytes: 0,
        downloads_bytes: 0,
    });
    let server_profiles = crate::server_worker::load_server_profiles(root);
    if recovered {
        Event::Recovered {
            snapshot,
            workspace: ws,
            specs,
            workspaces,
            history,
            secret_names,
            storage_stats,
            server_profiles,
        }
    } else {
        Event::Loaded {
            snapshot,
            workspace: ws,
            specs,
            workspaces,
            history,
            secret_names,
            storage_stats,
            server_profiles,
        }
    }
}

impl Worker {
    pub fn start(
        root: PathBuf,
        initial_workspace: LocalId,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
        let (commands, incoming) = mpsc::sync_channel::<Command>(32);
        let (outgoing, events) = mpsc::sync_channel::<Event>(32);
        let internal_commands = commands.clone();
        let cancels: Arc<Mutex<HashMap<u64, CancellationToken>>> =
            Arc::new(Mutex::new(HashMap::new()));

        std::thread::spawn(move || {
            let mut local_mock_handle: Option<swaggen_runner::local_mock::LocalMockHandle> = None;
            let mut runner_cancel: Option<CancellationToken> = None;
            let mut sse_cancel: Option<CancellationToken> = None;
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("tokio runtime for http-engine");

            let mut current_root = root;
            let mut current_workspace = initial_workspace;

            let open_session = |root_dir: &Path, ws: &LocalId| -> Result<Session, Error> {
                let directory = WorkspaceDirectory::open(root_dir, ws.clone())?;
                Ok(Session {
                    store: Store::open(directory.clone())?,
                    ws_store: WorkspaceStore::open(directory.clone())?,
                    secrets: SecretStore::open(directory)?,
                })
            };

            let mut session = open_session(&current_root, &current_workspace);
            let initial = match &session {
                Ok(s) => build_snapshot_event(s, &current_root, &current_workspace, false),
                Err(error) => Event::Failed(*error),
            };
            if outgoing.send(initial).is_err() {
                return;
            }
            wake();

            while let Ok(command) = incoming.recv() {
                match command {
                    Command::Shutdown => {
                        break;
                    }
                    Command::CancelHttp { execution_id } => {
                        if let Ok(mut map) = cancels.lock()
                            && let Some(tok) = map.remove(&execution_id)
                        {
                            tok.cancel();
                        }
                        continue;
                    }
                    Command::Reopen => {
                        drop(session);
                        session = open_session(&current_root, &current_workspace);
                        let event = match &session {
                            Ok(s) => {
                                build_snapshot_event(s, &current_root, &current_workspace, true)
                            }
                            Err(error) => Event::Failed(*error),
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                        continue;
                    }
                    Command::QuarantineCorruptDraft => {
                        drop(session);
                        let quarantined =
                            WorkspaceDirectory::open(&current_root, current_workspace.clone())
                                .and_then(|dir| quarantine_corrupt_draft(&dir));
                        match quarantined {
                            Ok(_) => {
                                session = open_session(&current_root, &current_workspace);
                                let event = match &session {
                                    Ok(s) => build_snapshot_event(
                                        s,
                                        &current_root,
                                        &current_workspace,
                                        true,
                                    ),
                                    Err(error) => Event::Failed(*error),
                                };
                                if outgoing.send(event).is_err() {
                                    break;
                                }
                            }
                            Err(error) => {
                                session = Err(error);
                                if outgoing.send(Event::Failed(error)).is_err() {
                                    break;
                                }
                            }
                        }
                        wake();
                        continue;
                    }
                    Command::SwitchWorkspace { id } => {
                        drop(session);
                        current_workspace = id;
                        session = open_session(&current_root, &current_workspace);
                        let event = match &session {
                            Ok(s) => {
                                build_snapshot_event(s, &current_root, &current_workspace, false)
                            }
                            Err(error) => Event::Failed(*error),
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                        continue;
                    }
                    Command::CreateWorkspace { id, name, kind } => {
                        drop(session);
                        current_workspace = id.clone();
                        session = open_session(&current_root, &current_workspace);
                        if let Ok(ref mut s) = session
                            && s.ws_store.state().revision == 0
                        {
                            let mut st = WorkspaceState::new_personal(id, name);
                            st.meta.kind = kind;
                            let _ = s.ws_store.save_state(st);
                        }
                        let event = match &session {
                            Ok(s) => {
                                build_snapshot_event(s, &current_root, &current_workspace, false)
                            }
                            Err(error) => Event::Failed(*error),
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                        continue;
                    }
                    Command::MigrateDataRoot { target_root } => {
                        drop(session);
                        let migrated =
                            migrate_workspace_root(&current_root, &target_root, &current_workspace);
                        match migrated {
                            Ok(_) => {
                                current_root = target_root.clone();
                                session = open_session(&current_root, &current_workspace);
                                let _ = outgoing.send(Event::DataMigrated {
                                    new_root: target_root,
                                });
                                let event = match &session {
                                    Ok(s) => build_snapshot_event(
                                        s,
                                        &current_root,
                                        &current_workspace,
                                        true,
                                    ),
                                    Err(error) => Event::Failed(*error),
                                };
                                if outgoing.send(event).is_err() {
                                    break;
                                }
                            }
                            Err(err) => {
                                session = open_session(&current_root, &current_workspace);
                                let _ = outgoing.send(Event::ActionError(err.to_string()));
                            }
                        }
                        wake();
                        continue;
                    }
                    _ => {}
                }

                let dl_dir = session
                    .as_ref()
                    .map(|s| s.ws_store.downloads_dir())
                    .unwrap_or_else(|_| current_root.join("_server_downloads"));
                let command = match crate::server_worker::handle_server_command(
                    &runtime,
                    &current_root,
                    &dl_dir,
                    command,
                ) {
                    Ok(ev) => {
                        if outgoing.send(ev).is_err() {
                            break;
                        }
                        wake();
                        continue;
                    }
                    Err(cmd) => match crate::sync_worker::handle_sync_command(
                        &runtime,
                        &current_root,
                        &dl_dir,
                        cmd,
                    ) {
                        Ok(ev) => {
                            if outgoing.send(ev).is_err() {
                                break;
                            }
                            wake();
                            continue;
                        }
                        Err(cmd2) => cmd2,
                    },
                };

                let sess = match &mut session {
                    Ok(s) => s,
                    Err(error) => {
                        let event = match command {
                            Command::PutSecret { value, .. } => Event::SecretFailed {
                                error: *error,
                                value,
                            },
                            _ => Event::Failed(*error),
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                        continue;
                    }
                };

                match command {
                    Command::Save { generation, draft } => {
                        let event = sess
                            .store
                            .save(draft)
                            .map(|revision| Event::Saved {
                                generation,
                                revision,
                            })
                            .unwrap_or_else(Event::Failed);
                        if matches!(event, Event::Failed(Error::Storage | Error::Recovery)) {
                            session = Err(Error::Recovery);
                        }
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::SaveWorkspace { generation, state } => {
                        let event = match sess.ws_store.save_state(*state) {
                            Ok(revision) => Event::WorkspaceSaved {
                                generation,
                                revision,
                                storage_stats: sess.ws_store.storage_stats().unwrap_or(
                                    StorageStats {
                                        workspace_bytes: 0,
                                        history_bytes: 0,
                                        secrets_bytes: 0,
                                        downloads_bytes: 0,
                                    },
                                ),
                            },
                            Err(err) => Event::Failed(err),
                        };
                        if matches!(event, Event::Failed(Error::Storage | Error::Recovery)) {
                            session = Err(Error::Recovery);
                        }
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::PutSecret { name, value } => {
                        let event = match sess.secrets.put(
                            &SecretRef {
                                workspace: current_workspace.clone(),
                                name,
                            },
                            &value,
                        ) {
                            Ok(()) => Event::SecretSaved {
                                secret_names: sess.secrets.list_names(),
                            },
                            Err(error) => Event::SecretFailed { error, value },
                        };
                        if matches!(
                            event,
                            Event::SecretFailed {
                                error: Error::Storage | Error::Recovery,
                                ..
                            }
                        ) {
                            session = Err(Error::Recovery);
                        }
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::GetSecret { name } => {
                        let event = sess
                            .secrets
                            .get(&SecretRef {
                                workspace: current_workspace.clone(),
                                name,
                            })
                            .map(Event::SecretLoaded)
                            .unwrap_or_else(Event::Failed);
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::DeleteSecret { name } => {
                        let event = sess
                            .secrets
                            .delete(&SecretRef {
                                workspace: current_workspace.clone(),
                                name: name.clone(),
                            })
                            .map(|()| Event::SecretDeleted {
                                name,
                                secret_names: sess.secrets.list_names(),
                            })
                            .unwrap_or_else(Event::Failed);
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::SendHttp {
                        execution_id,
                        request,
                        state,
                    } => {
                        // Retrieve DPAPI secrets inside the worker ONLY for execution (EV05).
                        let mut secret_map = BTreeMap::new();
                        for name in sess.secrets.list_names() {
                            if let Ok(secret) = sess.secrets.get(&SecretRef {
                                workspace: current_workspace.clone(),
                                name: name.clone(),
                            }) && let Ok(text) = std::str::from_utf8(secret.expose())
                            {
                                secret_map.insert(name, text.to_owned());
                            }
                        }
                        let col = state
                            .collections
                            .iter()
                            .find(|c| c.id == request.collection_id);
                        let fchain = folder_chain(&state.folders, request.folder_id.as_deref());
                        let col_vars = col.map(|c| c.variables.as_slice()).unwrap_or(&[]);
                        let ctx = swaggen_domain::workspace::VariableContext {
                            workspace_vars: &state.workspace_vars,
                            collection_vars: col_vars,
                            environment: state.active_environment(),
                            request_vars: &request.local_vars,
                            local_secrets: &secret_map,
                        };
                        let now_secs = time::OffsetDateTime::now_utc().unix_timestamp();
                        let resolved = resolve_request_preview(
                            &request,
                            col,
                            &fchain,
                            &ctx,
                            &state.cookies,
                            now_secs,
                        );
                        let downloads = sess.ws_store.downloads_dir();
                        let token = CancellationToken::new();
                        if let Ok(mut map) = cancels.lock() {
                            map.insert(execution_id, token.clone());
                        }
                        let cancels_done = cancels.clone();
                        let reply_cmd = internal_commands.clone();
                        let out_prog = outgoing.clone();
                        let wake_prog = wake.clone();
                        let (prog_tx, mut prog_rx) =
                            tokio::sync::mpsc::unbounded_channel::<TransferProgress>();
                        runtime.spawn(async move {
                            while let Some(p) = prog_rx.recv().await {
                                let _ = out_prog.try_send(Event::HttpProgress {
                                    execution_id,
                                    phase: p.phase,
                                    bytes_done: p.bytes_done,
                                    total_bytes: p.total_bytes,
                                });
                                wake_prog();
                            }
                        });
                        runtime.spawn(async move {
                            let res = execute_request_with_progress(
                                &resolved,
                                &downloads,
                                token,
                                Some(prog_tx),
                            )
                            .await;
                            if let Ok(mut map) = cancels_done.lock() {
                                map.remove(&execution_id);
                            }
                            let _ = reply_cmd.send(Command::InternalHttpFinished {
                                execution_id,
                                request,
                                resolved_url: resolved.masked_url,
                                now_secs,
                                result: Box::new(res),
                            });
                        });
                    }
                    Command::RunOAuth2Pkce {
                        auth_url,
                        token_url,
                        client_id,
                        scope,
                        downloads_dir,
                    } => {
                        let out = outgoing.clone();
                        let wake_fn = wake.clone();
                        runtime.spawn(async move {
                            match tokio::net::TcpListener::bind("127.0.0.1:0").await {
                                Ok(listener) => {
                                    let port = listener
                                        .local_addr()
                                        .map(|a| a.port())
                                        .unwrap_or(0);
                                    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
                                    let pkce = PkceChallenge::generate();
                                    let authorize_url = pkce.build_authorize_url(
                                        &auth_url,
                                        &client_id,
                                        &redirect_uri,
                                        &scope,
                                    );
                                    let _ = out.send(Event::OAuth2PendingAuthorize {
                                        authorize_url,
                                        redirect_uri: redirect_uri.clone(),
                                    });
                                    wake_fn();
                                    match await_loopback_callback(
                                        listener,
                                        &pkce.state,
                                        std::time::Duration::from_secs(120),
                                    )
                                    .await
                                    {
                                        Ok(code) => {
                                            let form = [
                                                ("grant_type", "authorization_code"),
                                                ("code", code.as_str()),
                                                ("redirect_uri", redirect_uri.as_str()),
                                                ("client_id", client_id.as_str()),
                                                ("code_verifier", pkce.code_verifier.as_str()),
                                            ];
                                            let ev = match exchange_token_form(
                                                &token_url,
                                                &form,
                                                &downloads_dir,
                                            )
                                            .await
                                            {
                                                Ok(tok) => Event::OAuth2TokenReceived(tok),
                                                Err(e) => Event::ActionError(e),
                                            };
                                            let _ = out.send(ev);
                                            wake_fn();
                                        }
                                        Err(e) => {
                                            let _ = out.send(Event::ActionError(e));
                                            wake_fn();
                                        }
                                    }
                                }
                                Err(e) => {
                                    let _ = out.send(Event::ActionError(format!(
                                        "Failed to bind OAuth2 callback listener: {e}"
                                    )));
                                    wake_fn();
                                }
                            }
                        });
                    }
                    Command::RunCollection {
                        state,
                        specs,
                        config,
                    } => {
                        let tok = CancellationToken::new();
                        runner_cancel = Some(tok.clone());
                        let mut secret_map = BTreeMap::new();
                        for name in sess.secrets.list_names() {
                            if let Ok(secret) = sess.secrets.get(&SecretRef {
                                workspace: current_workspace.clone(),
                                name: name.clone(),
                            }) && let Ok(text) = std::str::from_utf8(secret.expose())
                            {
                                secret_map.insert(name, text.to_owned());
                            }
                        }
                        let downloads = sess.ws_store.downloads_dir();
                        let out = outgoing.clone();
                        let wake_fn = wake.clone();
                        runtime.spawn(async move {
                            let rep = swaggen_runner::collection_runner::run_collection(
                                &state, &specs, &secret_map, &downloads, &config, tok,
                            )
                            .await;
                            let _ = out.send(Event::CollectionRunFinished(Box::new(rep)));
                            wake_fn();
                        });
                    }
                    Command::CancelRunner => {
                        if let Some(tok) = runner_cancel.take() {
                            tok.cancel();
                        }
                    }
                    Command::StartLocalMock { routes } => {
                        local_mock_handle = None;
                        let event = match runtime.block_on(swaggen_runner::local_mock::start_local_mock(routes)) {
                            Ok(handle) => {
                                let url = handle.base_url.clone();
                                local_mock_handle = Some(handle);
                                Event::LocalMockState {
                                    base_url: Some(url),
                                    logs: Vec::new(),
                                }
                            }
                            Err(e) => Event::ActionError(e),
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::StopLocalMock => {
                        local_mock_handle = None;
                        if outgoing
                            .send(Event::LocalMockState {
                                base_url: None,
                                logs: Vec::new(),
                            })
                            .is_err()
                        {
                            break;
                        }
                        wake();
                    }
                    Command::PollMockTraffic => {
                        let event = if let Some(h) = &local_mock_handle {
                            let logs = h.traffic_log.lock().map(|g| g.clone()).unwrap_or_default();
                            Event::LocalMockState {
                                base_url: Some(h.base_url.clone()),
                                logs,
                            }
                        } else {
                            Event::LocalMockState {
                                base_url: None,
                                logs: Vec::new(),
                            }
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::ExecuteGraphQl {
                        endpoint,
                        query,
                        variables_json,
                    } => {
                        let downloads = sess.ws_store.downloads_dir();
                        let out = outgoing.clone();
                        let wake_fn = wake.clone();
                        runtime.spawn(async move {
                            let ev = match swaggen_runner::protocols_and_tools::execute_graphql(
                                &endpoint,
                                &query,
                                &variables_json,
                                None,
                                &[],
                                &downloads,
                                CancellationToken::new(),
                            )
                            .await
                            {
                                Ok(res) => Event::GraphQlFinished(res),
                                Err(e) => Event::ActionError(e),
                            };
                            let _ = out.send(ev);
                            wake_fn();
                        });
                    }
                    Command::StartSse { url, max_events } => {
                        let tok = CancellationToken::new();
                        sse_cancel = Some(tok.clone());
                        let out = outgoing.clone();
                        let wake_fn = wake.clone();
                        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                        let out_stream = out.clone();
                        let wake_stream = wake_fn.clone();
                        runtime.spawn(async move {
                            while let Some(ev) = rx.recv().await {
                                let _ = out_stream.try_send(Event::SseEventReceived(ev));
                                wake_stream();
                            }
                        });
                        runtime.spawn(async move {
                            let res = swaggen_runner::protocols_and_tools::stream_sse_bounded(
                                &url,
                                &[],
                                swaggen_runner::protocols_and_tools::SseClientPolicy {
                                    max_events,
                                    max_buffer_bytes: 512 * 1024,
                                },
                                tok,
                                Some(tx),
                            )
                            .await;
                            let msg = match res {
                                Ok(list) => format!("SSE closed ({} event(s) buffered)", list.len()),
                                Err(e) => format!("SSE error: {e}"),
                            };
                            let _ = out.send(Event::SseStopped(msg));
                            wake_fn();
                        });
                    }
                    Command::StopSse => {
                        if let Some(tok) = sse_cancel.take() {
                            tok.cancel();
                        }
                    }
                    Command::InternalHttpFinished {
                        execution_id,
                        request,
                        resolved_url,
                        now_secs,
                        result,
                    } => {
                        let event = match *result {
                            Ok(response) => {
                                let extracted = apply_response_extractors(
                                    &request.extractors,
                                    &response.headers,
                                    &response.preview_bytes,
                                );
                                let entry = HistoryEntry {
                                    id: new_uid(),
                                    timestamp_epoch_secs: now_secs,
                                    request_id: request.id.clone(),
                                    request_name: request.name.clone(),
                                    method: request.method,
                                    resolved_url: resolved_url.clone(),
                                    status_code: Some(response.status_code),
                                    duration_ms: response.duration_ms,
                                    wire_bytes: response.wire_bytes,
                                    decoded_bytes: response.decoded_bytes,
                                    error_kind: None,
                                    snapshot: (*request).clone(),
                                };
                                let _ = sess.ws_store.record_history(entry);
                                if !response.captured_cookies.is_empty() || !extracted.is_empty() {
                                    let mut next = sess.ws_store.state().clone();
                                    for ck in &response.captured_cookies {
                                        upsert_cookie(&mut next.cookies, ck.clone());
                                    }
                                    if let Some(env_id) = next.active_environment_id.clone()
                                        && let Some(env) =
                                            next.environments.iter_mut().find(|e| e.id == env_id)
                                    {
                                        for (var_key, val, is_secret) in &extracted {
                                            if *is_secret
                                                && let Ok(sec_id) = LocalId::parse(
                                                    var_key.to_ascii_lowercase(),
                                                )
                                                && let Ok(sec) = Secret::new(val.as_bytes().to_vec())
                                            {
                                                let _ = sess.secrets.put(
                                                    &SecretRef {
                                                        workspace: current_workspace.clone(),
                                                        name: sec_id.clone(),
                                                    },
                                                    &sec,
                                                );
                                                if let Some(existing) = env
                                                    .variables
                                                    .iter_mut()
                                                    .find(|v| v.key == *var_key)
                                                {
                                                    existing.is_secret = true;
                                                    existing.secret_ref = Some(sec_id);
                                                    existing.shared_value.clear();
                                                } else {
                                                    env.variables.push(VariableEntry::secret(
                                                        var_key, sec_id,
                                                    ));
                                                }
                                            } else if let Some(existing) = env
                                                .variables
                                                .iter_mut()
                                                .find(|v| v.key == *var_key)
                                            {
                                                existing.shared_value = val.clone();
                                            } else {
                                                env.variables.push(VariableEntry::public(
                                                    var_key,
                                                    val.clone(),
                                                ));
                                            }
                                        }
                                    }
                                    let _ = sess.ws_store.save_state(next);
                                }
                                let extracted_public: Vec<(String, String, bool)> = extracted
                                    .into_iter()
                                    .map(|(k, v, is_sec)| {
                                        if is_sec {
                                            (k, String::new(), true)
                                        } else {
                                            (k, v, false)
                                        }
                                    })
                                    .collect();
                                Event::HttpCompleted {
                                    execution_id,
                                    request_id: request.id.clone(),
                                    request_name: request.name.clone(),
                                    method: request.method,
                                    resolved_url,
                                    snapshot: request,
                                    response,
                                    extracted: extracted_public,
                                    history: sess.ws_store.history().to_vec(),
                                    storage_stats: sess.ws_store.storage_stats().unwrap_or(
                                        StorageStats {
                                            workspace_bytes: 0,
                                            history_bytes: 0,
                                            secrets_bytes: 0,
                                            downloads_bytes: 0,
                                        },
                                    ),
                                }
                            }
                            Err(error) => {
                                let entry = HistoryEntry {
                                    id: new_uid(),
                                    timestamp_epoch_secs: now_secs,
                                    request_id: request.id.clone(),
                                    request_name: request.name.clone(),
                                    method: request.method,
                                    resolved_url: resolved_url.clone(),
                                    status_code: None,
                                    duration_ms: error.duration_ms,
                                    wire_bytes: 0,
                                    decoded_bytes: 0,
                                    error_kind: Some(error.kind.as_str().to_owned()),
                                    snapshot: (*request).clone(),
                                };
                                let _ = sess.ws_store.record_history(entry);
                                Event::HttpFailed {
                                    execution_id,
                                    request_id: request.id.clone(),
                                    request_name: request.name.clone(),
                                    method: request.method,
                                    resolved_url,
                                    snapshot: *request,
                                    error,
                                    history: sess.ws_store.history().to_vec(),
                                }
                            }
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::DeleteRequestHistory { request_id } => {
                        let event = match sess.ws_store.delete_history_for_request(&request_id) {
                            Ok(()) => Event::HistoryUpdated {
                                history: sess.ws_store.history().to_vec(),
                                storage_stats: sess.ws_store.storage_stats().unwrap_or(
                                    StorageStats {
                                        workspace_bytes: 0,
                                        history_bytes: 0,
                                        secrets_bytes: 0,
                                        downloads_bytes: 0,
                                    },
                                ),
                            },
                            Err(err) => Event::Failed(err),
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::ClearHistory => {
                        let event = match sess.ws_store.clear_history() {
                            Ok(()) => Event::HistoryUpdated {
                                history: Vec::new(),
                                storage_stats: sess.ws_store.storage_stats().unwrap_or(
                                    StorageStats {
                                        workspace_bytes: 0,
                                        history_bytes: 0,
                                        secrets_bytes: 0,
                                        downloads_bytes: 0,
                                    },
                                ),
                            },
                            Err(err) => Event::Failed(err),
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::CleanCache => {
                        let event = match sess.ws_store.clean_cache_preserving_projects() {
                            Ok(removed_bytes) => Event::CacheCleaned {
                                removed_bytes,
                                history: sess.ws_store.history().to_vec(),
                                storage_stats: sess.ws_store.storage_stats().unwrap_or(
                                    StorageStats {
                                        workspace_bytes: 0,
                                        history_bytes: 0,
                                        secrets_bytes: 0,
                                        downloads_bytes: 0,
                                    },
                                ),
                            },
                            Err(err) => Event::Failed(err),
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::SaveResponseFile {
                        source,
                        destination,
                    } => {
                        let event = match safe_copy_file(&source, &destination) {
                            Ok(()) => Event::FileSaved {
                                message: format!("Saved raw response to {}", destination.display()),
                            },
                            Err(msg) => Event::ActionError(msg),
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::SaveSpecs { specs } => {
                        let event = match sess.ws_store.save_specs(specs) {
                            Ok(()) => Event::SpecsSaved,
                            Err(err) => Event::Failed(err),
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::FetchSpecUrl { name, url } => {
                        let req = RequestItem::new(
                            "spec-fetch",
                            name.clone(),
                            swaggen_domain::workspace::HttpMethod::Get,
                            url,
                        );
                        let ctx = swaggen_domain::workspace::VariableContext {
                            workspace_vars: &[],
                            collection_vars: &[],
                            environment: None,
                            request_vars: &[],
                            local_secrets: &BTreeMap::new(),
                        };
                        let resolved = resolve_request_preview(&req, None, &[], &ctx, &[], 0);
                        let dl = sess.ws_store.downloads_dir();
                        let event = match runtime.block_on(execute_request(
                            &resolved,
                            &dl,
                            CancellationToken::new(),
                        )) {
                            Ok(resp) => {
                                let text =
                                    String::from_utf8_lossy(&resp.preview_bytes).into_owned();
                                let doc = parse_and_validate(new_uid(), name, &text, &[]);
                                Event::SpecFetched(Box::new(doc))
                            }
                            Err(err) => Event::ActionError(format!(
                                "Spec fetch failed ({}): {}",
                                err.kind.as_str(),
                                err.message
                            )),
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::ExportSanitizedWorkspace { destination, state } => {
                        let event = match state
                            .export_sanitized_json(&RedactionPolicy::default())
                            .map_err(|e| e.to_string())
                            .and_then(|json| safe_write_file(&destination, json.as_bytes()))
                        {
                            Ok(()) => Event::FileSaved {
                                message: format!(
                                    "Exported sanitized workspace to {}",
                                    destination.display()
                                ),
                            },
                            Err(msg) => Event::ActionError(msg),
                        };
                        if outgoing.send(event).is_err() {
                            break;
                        }
                        wake();
                    }
                    Command::Reopen
                    | Command::QuarantineCorruptDraft
                    | Command::SwitchWorkspace { .. }
                    | Command::CreateWorkspace { .. }
                    | Command::CancelHttp { .. }
                    | Command::MigrateDataRoot { .. }
                    | Command::SaveServerProfiles { .. }
                    | Command::TestServerConnection { .. }
                    | Command::BootstrapServerOwner { .. }
                    | Command::ServerLogin { .. }
                    | Command::AcceptServerInvitation { .. }
                    | Command::ListServerSessions { .. }
                    | Command::RevokeServerSession { .. }
                    | Command::ServerLogout { .. }
                    | Command::ServerChangePassword { .. }
                    | Command::RefreshTeamWorkspaces { .. }
                    | Command::CreateTeamWorkspace { .. }
                    | Command::LoadTeamWorkspaceDetails { .. }
                    | Command::CreateServerInvitation { .. }
                    | Command::RevokeServerInvitation { .. }
                    | Command::UpdateMemberRole { .. }
                    | Command::StageAndPublishEntity { .. }
                    | Command::SyncNow { .. }
                    | Command::ResolveSyncConflict { .. }
                    | Command::LoadEntityRevisions { .. }
                    | Command::RestoreEntityRevision { .. }
                    | Command::LoadCollabData { .. }
                    | Command::CreateTeamComment { .. }
                    | Command::ResolveTeamComment { .. }
                    | Command::MarkTeamNotificationsRead { .. }
                    | Command::Shutdown => unreachable!(),
                }
            }
        });

        Self { commands, events }
    }
}

fn upsert_cookie(cookies: &mut Vec<CookieRecord>, incoming: CookieRecord) {
    cookies.retain(|c| {
        !(c.domain.eq_ignore_ascii_case(&incoming.domain)
            && c.path == incoming.path
            && c.name == incoming.name)
    });
    cookies.push(incoming);
}

fn safe_copy_file(source: &Path, destination: &Path) -> Result<(), String> {
    if !destination.is_absolute() {
        return Err("Destination path must be absolute".into());
    }
    if let Ok(meta) = fs::symlink_metadata(destination)
        && meta.file_type().is_symlink()
    {
        return Err("Refusing to write to a symlink".into());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(source, destination).map_err(|e| e.to_string())?;
    Ok(())
}

fn safe_write_file(destination: &Path, bytes: &[u8]) -> Result<(), String> {
    if !destination.is_absolute() {
        return Err("Destination path must be absolute".into());
    }
    if let Ok(meta) = fs::symlink_metadata(destination)
        && meta.file_type().is_symlink()
    {
        return Err("Refusing to write to a symlink".into());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(destination, bytes).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn failed_open_can_retry_without_losing_commands_or_resetting_data() {
        let root = std::env::temp_dir().join(format!(
            "swaggen-retry-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("personal"), b"preserve blocker").unwrap();
        let worker = Worker::start(root.clone(), LocalId::parse("personal").unwrap(), || {});
        assert!(matches!(
            worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Failed(_)
        ));
        worker
            .commands
            .send(Command::PutSecret {
                name: LocalId::parse("token").unwrap(),
                value: Secret::new(b"retry-private-canary".to_vec()).unwrap(),
            })
            .unwrap();
        match worker.events.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::SecretFailed { value, .. } => {
                assert_eq!(value.expose(), b"retry-private-canary")
            }
            _ => panic!("failed secret input was not returned"),
        }
        assert_eq!(
            std::fs::read(root.join("personal")).unwrap(),
            b"preserve blocker"
        );
        std::fs::remove_file(root.join("personal")).unwrap();
        worker.commands.send(Command::Reopen).unwrap();
        assert!(matches!(
            worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Recovered {
                snapshot: Snapshot { revision: 0, .. },
                ..
            }
        ));
        worker
            .commands
            .send(Command::Save {
                generation: 42,
                draft: "retained editor overlay".into(),
            })
            .unwrap();
        assert!(matches!(
            worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Saved {
                generation: 42,
                revision: 1
            }
        ));
        worker.commands.send(Command::Reopen).unwrap();
        match worker.events.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::Recovered { snapshot, .. } => {
                assert_eq!(snapshot.revision, 1);
                assert_eq!(snapshot.draft, "retained editor overlay");
            }
            _ => panic!("expected durable recovery"),
        }
        drop(worker);
    }

    #[test]
    fn actual_actor_persists_and_returns_matching_generation() {
        let root = std::env::temp_dir().join(format!(
            "swaggen-p01-actor-{}-{}",
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
            .send(Command::Save {
                generation: 17,
                draft: "actor draft".into(),
            })
            .unwrap();
        assert!(matches!(
            worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Saved {
                generation: 17,
                revision: 1
            }
        ));
        drop(worker);
        let directory =
            WorkspaceDirectory::open(&root, LocalId::parse("personal").unwrap()).unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(5);
        let reopened = loop {
            if let Ok(store) = Store::open(directory.clone()) {
                break store;
            }
            assert!(
                std::time::Instant::now() < until,
                "actor did not release its lock"
            );
            std::thread::yield_now();
        };
        assert_eq!(reopened.snapshot().draft, "actor draft");
        drop(reopened);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn actor_quarantines_corrupt_draft_without_losing_forensic_bytes() {
        let root = std::env::temp_dir().join(format!(
            "swaggen-p01-quarantine-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dir = WorkspaceDirectory::open(&root, LocalId::parse("personal").unwrap()).unwrap();
        std::fs::write(
            dir.draft_path(),
            b"BROKEN-JOURNAL-BYTES-LONG-ENOUGH-TO-FAIL-MAGIC",
        )
        .unwrap();
        let worker = Worker::start(root.clone(), LocalId::parse("personal").unwrap(), || {});
        assert!(matches!(
            worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Failed(Error::Recovery)
        ));
        worker
            .commands
            .send(Command::QuarantineCorruptDraft)
            .unwrap();
        assert!(matches!(
            worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::Recovered {
                snapshot: Snapshot { revision: 0, .. },
                ..
            }
        ));
        drop(worker);
        let has_quarantine = std::fs::read_dir(root.join("personal"))
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("draft.journal.quarantine-")
            });
        assert!(has_quarantine);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn actor_lists_and_deletes_secrets_end_to_end() {
        let root = std::env::temp_dir().join(format!(
            "swaggen-p01-actor-secret-{}-{}",
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
            .send(Command::PutSecret {
                name: LocalId::parse("api-key").unwrap(),
                value: Secret::new(b"disposable-token-1".to_vec()).unwrap(),
            })
            .unwrap();
        match worker.events.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::SecretSaved { secret_names } => {
                assert_eq!(secret_names, vec![LocalId::parse("api-key").unwrap()]);
            }
            _ => panic!("expected SecretSaved"),
        }
        worker
            .commands
            .send(Command::DeleteSecret {
                name: LocalId::parse("api-key").unwrap(),
            })
            .unwrap();
        match worker.events.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::SecretDeleted { secret_names, .. } => assert!(secret_names.is_empty()),
            _ => panic!("expected SecretDeleted"),
        }
        drop(worker);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn actor_resolves_dpapi_secret_and_executes_real_http_recording_history() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let addr = rt.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            tokio::spawn(async move {
                if let Ok((mut sock, _)) = listener.accept().await {
                    let mut buf = vec![0u8; 4096];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let ok = req
                        .to_ascii_lowercase()
                        .contains("authorization: bearer dpapi-secret-xyz");
                    let body = if ok { r#"{"ok":true}"# } else { r#"{"ok":false}"# };
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                }
            });
            addr
        });

        let root = std::env::temp_dir().join(format!(
            "swaggen-p02-http-actor-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let worker = Worker::start(root.clone(), LocalId::parse("personal").unwrap(), || {});
        let mut ws = match worker.events.recv_timeout(Duration::from_secs(5)).unwrap() {
            Event::Loaded { workspace, .. } => workspace,
            _ => panic!("expected Loaded"),
        };
        let sec_id = LocalId::parse("bearer-tok").unwrap();
        worker
            .commands
            .send(Command::PutSecret {
                name: sec_id.clone(),
                value: Secret::new(b"dpapi-secret-xyz".to_vec()).unwrap(),
            })
            .unwrap();
        assert!(matches!(
            worker.events.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::SecretSaved { .. }
        ));

        let env_id = ws.active_environment_id.clone().unwrap();
        let env = ws.environments.iter_mut().find(|e| e.id == env_id).unwrap();
        env.variables
            .push(swaggen_domain::workspace::VariableEntry::secret(
                "api_token",
                sec_id,
            ));
        let mut req = ws.requests[0].clone();
        req.url = format!("http://{addr}/ping");
        req.auth = swaggen_domain::workspace::AuthConfig::Bearer {
            token: "{{api_token}}".into(),
        };
        ws.requests[0] = req.clone();
        worker
            .commands
            .send(Command::SendHttp {
                execution_id: 1,
                request: Box::new(req),
                state: Box::new(ws),
            })
            .unwrap();
        loop {
            match worker.events.recv_timeout(Duration::from_secs(10)).unwrap() {
                Event::HttpProgress { .. } => continue,
                Event::HttpCompleted {
                    response, history, ..
                } => {
                    assert_eq!(response.status_code, 200);
                    assert_eq!(
                        String::from_utf8_lossy(&response.preview_bytes),
                        r#"{"ok":true}"#
                    );
                    assert_eq!(history.len(), 1);
                    assert!(!history[0].resolved_url.contains("dpapi-secret-xyz"));
                    break;
                }
                _ => panic!("expected HttpCompleted"),
            }
        }
        drop(worker);
        let _ = std::fs::remove_dir_all(root);
    }
}
