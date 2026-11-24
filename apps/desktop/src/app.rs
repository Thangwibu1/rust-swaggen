//! Main desktop application state coordinating the white-surface UI with the bounded worker.
use crate::{
    diagnostics::{self, DiagnosticPreview},
    views,
    worker::{Command, Event, Worker},
};
use eframe::egui;
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::mpsc::TrySendError,
};
use swaggen_domain::{
    Error, LocalId, Secret,
    security::DesktopPolicy,
    workspace::{HistoryEntry, HttpMethod, RequestItem, WorkspaceState},
};
use swaggen_http_engine::{HttpExecutionError, HttpResponseData};
use swaggen_local_store::{Snapshot, workspace_store::StorageStats};
use swaggen_spec_engine::openapi::{SpecDiffEntry, SpecDocument};
use swaggen_ui_kit::{AvatarTone, Edge, IconKind, NoticeKind, Tokens};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMode {
    Workbench,
    Runner,
    Server,
    TeamSync,
    Specs,
    ImportExport,
    Environments,
    History,
    Cookies,
    Secrets,
    Sanitizer,
    Storage,
    Privacy,
    Shortcuts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestSubTab {
    Params,
    Headers,
    Body,
    Auth,
    Assertions,
    Connection,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseSubTab {
    Body,
    HeadersCookies,
    Assertions,
    Redirects,
    Diff,
    SaveExample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseBodyFormat {
    PrettyJson,
    Raw,
    Xml,
    Binary,
}

pub struct App {
    pub worker: Worker,
    pub root: PathBuf,
    pub workspace_id: LocalId,
    pub available_workspaces: Vec<LocalId>,
    pub state: WorkspaceState,
    pub legacy_snapshot: Snapshot,
    pub history: Vec<HistoryEntry>,
    pub secret_names: Vec<LocalId>,
    pub storage_stats: StorageStats,
    pub policy: DesktopPolicy,

    pub mode: MainMode,
    pub request_subtab: RequestSubTab,
    pub response_subtab: ResponseSubTab,
    pub response_format: ResponseBodyFormat,

    pub loaded: bool,
    pub pending: bool,
    pub generation: u64,
    pub saved_generation: u64,
    pub status: String,
    pub error: bool,
    pub recovery_needed: bool,
    pub confirm_quarantine: bool,
    pub close_dialog: bool,
    pub allow_close: bool,

    // Search & tree UI state
    pub search_query: String,
    pub filter_method: Option<HttpMethod>,
    pub filter_tag: String,
    pub focus_search: bool,
    pub show_sidebar_filters: bool,
    pub show_new_collection_input: bool,
    pub new_collection_name: String,
    pub new_workspace_id: String,
    pub new_workspace_name: String,
    pub show_workspace_modal: bool,
    pub show_production_confirm: bool,
    pub move_target_collection: String,
    pub move_target_folder: String,

    // Execution & response UI state
    pub next_execution_id: u64,
    pub inflight_execution_id: Option<u64>,
    pub inflight_request_id: Option<String>,
    pub responses: HashMap<String, Result<HttpResponseData, HttpExecutionError>>,
    pub response_search: String,
    pub save_response_path: String,
    pub example_name_input: String,
    pub example_extra_fields: String,

    // Environment & cookie editor UI state
    pub selected_env_idx: usize,
    pub new_env_name: String,
    pub new_env_prod: bool,
    pub inspect_var_key: String,
    pub cookie_domain: String,
    pub cookie_path: String,
    pub cookie_name: String,
    pub cookie_value: String,
    pub history_filter: String,
    pub export_workspace_path: String,
    pub migrate_root_path: String,

    // Secrets & sanitizer & privacy UI state
    pub secret_name: String,
    pub secret_input: String,
    pub revealed: Option<Secret>,
    pub sanitizer_input: String,
    pub sanitizer_extra_fields: String,
    pub untrusted_input: String,
    pub last_diag_export: Option<String>,
    pub collapsed_json_paths: std::collections::BTreeSet<String>,

    // P03 Specs & Import/Export UI state
    pub specs: Vec<SpecDocument>,
    pub selected_spec_idx: usize,
    pub spec_editor_text: String,
    pub spec_import_url: String,
    pub spec_find: String,
    pub spec_replace: String,
    pub spec_doc_search: String,
    pub spec_new_method: HttpMethod,
    pub spec_new_path: String,
    pub spec_new_op_id: String,
    pub spec_diffs: Vec<SpecDiffEntry>,
    pub curl_input: String,
    pub postman_input: String,
    pub last_import_summary: String,

    // P04 Team Server & Firebase Auth state
    pub show_welcome: bool,
    pub firebase_create_account: bool,
    pub server_subtab: usize,
    pub server_profiles: swaggen_domain::server_profiles::ServerProfileStore,
    pub server_name_input: String,
    pub server_url_input: String,
    pub server_allow_self_signed: bool,
    pub auth_email: String,
    pub auth_display_name: String,
    pub auth_password: String,
    pub auth_bootstrap_token: String,
    pub invite_token_input: String,
    pub team_workspaces: Vec<swaggen_api_contract::TeamWorkspaceSummary>,
    pub selected_team_workspace_id: Option<String>,
    pub team_ws_slug: String,
    pub team_ws_name: String,
    pub team_member_details: Option<swaggen_api_contract::MemberListResponse>,
    pub team_audit_events: Vec<swaggen_api_contract::AuditEventRecord>,
    pub invite_email_input: String,
    pub invite_role: swaggen_api_contract::Role,
    pub last_created_invite_token: String,
    pub server_sessions: Vec<swaggen_api_contract::SessionSummary>,
    pub pw_current_input: String,
    pub pw_new_input: String,
    pub pw_revoke_others: bool,

    // P05 Sync & Collaboration UI state
    pub sync_subtab: usize,
    pub sync_state: swaggen_sync::TeamSyncState,
    pub publish_note_input: String,
    pub history_entity_id_input: String,
    pub team_revisions: Vec<swaggen_api_contract::EntityRevisionRecord>,
    pub team_comments: Vec<swaggen_api_contract::CommentThreadRecord>,
    pub comment_body_input: String,
    pub team_notifications: Vec<swaggen_api_contract::NotificationRecord>,
    pub team_unread_notifications: usize,

    // P07 Version B core UI state
    pub transfer_progress: Option<(u64, &'static str, u64, Option<u64>)>,
    pub previous_responses: HashMap<String, HttpResponseData>,
    pub diff_ignore_paths: String,
    pub oauth_auth_url: String,
    pub oauth_token_url: String,
    pub oauth_client_id: String,
    pub oauth_scope: String,
    pub oauth_authorize_url: String,
    pub spec_bundle_dir: String,
    pub spec_lint_issues: Vec<swaggen_spec_engine::openapi::ValidationIssue>,
    pub spec_contract_report: Option<swaggen_spec_engine::advanced::ContractValidationReport>,
    pub spec_drift_items: Vec<swaggen_spec_engine::advanced::DriftItem>,
    pub spec_schema_tree: Option<swaggen_spec_engine::advanced::SchemaTreeNode>,
    pub git_export_dir: String,
    pub attach_files_csv: String,
    pub attach_shared_dir: String,
    pub p07_export_preview: String,

    // P08 Runner, Mock Server, GraphQL/SSE, Snippets & Dev Utils UI state
    pub snippet_target: swaggen_runner::protocols_and_tools::SnippetTarget,
    pub runner_subtab: usize,
    pub runner_iterations: usize,
    pub runner_delay_ms: u64,
    pub runner_stop_on_fail: bool,
    pub runner_validate_contract: bool,
    pub runner_dataset_text: String,
    pub runner_running: bool,
    pub last_run_report: Option<swaggen_runner::collection_runner::CollectionRunReport>,
    pub mock_seed: u64,
    pub local_mock_url: Option<String>,
    pub local_mock_logs: Vec<swaggen_runner::local_mock::MockTrafficLog>,
    pub gql_endpoint: String,
    pub gql_query: String,
    pub gql_vars: String,
    pub gql_result: Option<swaggen_runner::protocols_and_tools::GraphQlOutcome>,
    pub sse_url: String,
    pub sse_max_events: usize,
    pub sse_running: bool,
    pub sse_events: Vec<swaggen_runner::protocols_and_tools::SseEventRecord>,
    pub dev_util_input: String,
    pub dev_util_output: String,

    // P09 UX B, Governance, Command Palette, Theme/Locale & Deep Link / Update Verification
    pub show_command_palette: bool,
    pub command_palette_query: String,
    pub theme: swaggen_ui_kit::UiTheme,
    pub locale: swaggen_ui_kit::UiLocale,
    pub deep_link_input: String,
    pub update_manifest_json: String,
    pub update_signing_key: String,
    pub update_payload_text: String,

    // P10 Sandboxed Script + Extensions UI state
    pub sandbox_script_text: String,
    pub sandbox_report: Option<swaggen_runner::sandbox::SandboxExecutionReport>,
    pub extensions: Vec<swaggen_domain::extensions::ExtensionManifest>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, root: PathBuf, workspace_id: LocalId) -> Self {
        swaggen_ui_kit::install(&cc.egui_ctx);
        let context = cc.egui_ctx.clone();
        let worker = Worker::start(root.clone(), workspace_id.clone(), move || {
            context.request_repaint();
        });
        let initial_state =
            WorkspaceState::new_personal(workspace_id.clone(), workspace_id.as_str().to_owned());
        Self {
            worker,
            root: root.clone(),
            workspace_id: workspace_id.clone(),
            available_workspaces: vec![workspace_id.clone()],
            state: initial_state,
            legacy_snapshot: Snapshot {
                version: 1,
                workspace: workspace_id,
                revision: 0,
                draft: String::new(),
            },
            history: Vec::new(),
            secret_names: Vec::new(),
            storage_stats: StorageStats {
                workspace_bytes: 0,
                history_bytes: 0,
                secrets_bytes: 0,
                downloads_bytes: 0,
            },
            policy: DesktopPolicy::default(),
            mode: MainMode::Workbench,
            request_subtab: RequestSubTab::Params,
            response_subtab: ResponseSubTab::Body,
            response_format: ResponseBodyFormat::PrettyJson,
            loaded: false,
            pending: true,
            generation: 0,
            saved_generation: 0,
            status: "Opening local workspace…".into(),
            error: false,
            recovery_needed: false,
            confirm_quarantine: false,
            close_dialog: false,
            allow_close: false,
            search_query: String::new(),
            filter_method: None,
            filter_tag: String::new(),
            focus_search: false,
            show_sidebar_filters: false,
            show_new_collection_input: false,
            new_collection_name: String::new(),
            new_workspace_id: String::new(),
            new_workspace_name: String::new(),
            show_workspace_modal: false,
            show_production_confirm: false,
            move_target_collection: String::new(),
            move_target_folder: String::new(),
            next_execution_id: 1,
            inflight_execution_id: None,
            inflight_request_id: None,
            responses: HashMap::new(),
            response_search: String::new(),
            save_response_path: root.join("response-body.bin").display().to_string(),
            example_name_input: "200 OK example".into(),
            example_extra_fields: String::new(),
            selected_env_idx: 0,
            new_env_name: String::new(),
            new_env_prod: false,
            inspect_var_key: "base_url".into(),
            cookie_domain: "127.0.0.1".into(),
            cookie_path: "/".into(),
            cookie_name: String::new(),
            cookie_value: String::new(),
            history_filter: String::new(),
            export_workspace_path: root.join("workspace-export.json").display().to_string(),
            migrate_root_path: String::new(),
            secret_name: String::new(),
            secret_input: String::new(),
            revealed: None,
            sanitizer_input: "GET https://api.example.test/v1/tasks?api_key=demo-key-123 HTTP/1.1\nAuthorization: Bearer demo-token-456\nCookie: session=demo-session\n".into(),
            sanitizer_extra_fields: "x-custom-secret, tenant_pin".into(),
            untrusted_input: "<script>alert('xss')</script>\n# Example API Note\n<img src=\"https://example.invalid/tracker.png\" onerror=\"steal()\">".into(),
            last_diag_export: None,
            collapsed_json_paths: std::collections::BTreeSet::new(),
            specs: Vec::new(),
            selected_spec_idx: 0,
            spec_editor_text: String::new(),
            spec_import_url: String::new(),
            spec_find: String::new(),
            spec_replace: String::new(),
            spec_doc_search: String::new(),
            spec_new_method: HttpMethod::Get,
            spec_new_path: "/items/{id}".into(),
            spec_new_op_id: "getItem".into(),
            spec_diffs: Vec::new(),
            curl_input: "curl -X GET \"https://api.example.test/health\"".into(),
            postman_input: String::new(),
            last_import_summary: String::new(),
            show_welcome: true,
            firebase_create_account: false,
            server_subtab: 0,
            server_profiles: Default::default(),
            server_name_input: crate::server_worker::DEFAULT_SERVER_NAME.into(),
            server_url_input: crate::server_worker::default_server_url(),
            server_allow_self_signed: false,
            auth_email: String::new(),
            auth_display_name: String::new(),
            auth_password: String::new(),
            auth_bootstrap_token: String::new(),
            invite_token_input: String::new(),
            team_workspaces: Vec::new(),
            selected_team_workspace_id: None,
            team_ws_slug: "team-api".into(),
            team_ws_name: "Team API".into(),
            team_member_details: None,
            team_audit_events: Vec::new(),
            invite_email_input: String::new(),
            invite_role: swaggen_api_contract::Role::Editor,
            last_created_invite_token: String::new(),
            server_sessions: Vec::new(),
            pw_current_input: String::new(),
            pw_new_input: String::new(),
            pw_revoke_others: false,
            sync_subtab: 0,
            sync_state: Default::default(),
            publish_note_input: String::new(),
            history_entity_id_input: String::new(),
            team_revisions: Vec::new(),
            team_comments: Vec::new(),
            comment_body_input: String::new(),
            team_notifications: Vec::new(),
            team_unread_notifications: 0,
            transfer_progress: None,
            previous_responses: HashMap::new(),
            diff_ignore_paths: "timestamp, request_id".into(),
            oauth_auth_url: "https://auth.example.test/authorize".into(),
            oauth_token_url: "https://auth.example.test/token".into(),
            oauth_client_id: "swaggen-desktop".into(),
            oauth_scope: "openid profile".into(),
            oauth_authorize_url: String::new(),
            spec_bundle_dir: root.join("openapi-bundle").display().to_string(),
            spec_lint_issues: Vec::new(),
            spec_contract_report: None,
            spec_drift_items: Vec::new(),
            spec_schema_tree: None,
            git_export_dir: root.join("git-export").display().to_string(),
            attach_files_csv: String::new(),
            attach_shared_dir: root.join("shared-assets").display().to_string(),
            p07_export_preview: String::new(),
            snippet_target: swaggen_runner::protocols_and_tools::SnippetTarget::Curl,
            runner_subtab: 0,
            runner_iterations: 1,
            runner_delay_ms: 0,
            runner_stop_on_fail: false,
            runner_validate_contract: false,
            runner_dataset_text: String::new(),
            runner_running: false,
            last_run_report: None,
            mock_seed: 42,
            local_mock_url: None,
            local_mock_logs: Vec::new(),
            gql_endpoint: "https://api.example.test/graphql".into(),
            gql_query: "query Health {\n  health\n}".into(),
            gql_vars: "{}".into(),
            gql_result: None,
            sse_url: "http://127.0.0.1:8080/events".into(),
            sse_max_events: 200,
            sse_running: false,
            sse_events: Vec::new(),
            dev_util_input: "hello: swaggen".into(),
            dev_util_output: String::new(),
            show_command_palette: false,
            command_palette_query: String::new(),
            theme: swaggen_ui_kit::UiTheme::Light,
            locale: swaggen_ui_kit::UiLocale::En,
            deep_link_input: "swaggen://workspace/personal/mode/runner".into(),
            update_manifest_json: String::new(),
            update_signing_key: String::new(),
            update_payload_text: String::new(),
            sandbox_script_text: "const p = pm.response.json();\npm.test(\"ok\", () => {\n  pm.expect(pm.response.code).to.eql(200);\n});".into(),
            sandbox_report: None,
            extensions: vec![swaggen_domain::extensions::ExtensionManifest::install_disabled(
                "hdr-tagger",
                "Trace Header Tagger",
                "1.0.0",
                [swaggen_domain::extensions::ExtensionCapability::TransformBody]
                    .into_iter()
                    .collect(),
            )],
        }
    }

    pub fn dirty(&self) -> bool {
        self.generation != self.saved_generation
    }

    pub fn mark_dirty(&mut self) {
        self.generation = self.generation.saturating_add(1);
    }

    pub fn hide_secret(&mut self) {
        if self.revealed.take().is_some() {
            self.status = "Secret value hidden".into();
        }
    }

    pub fn active_request_id(&self) -> Option<String> {
        let tab_id = self.state.active_tab_id.as_deref()?;
        self.state
            .open_tabs
            .iter()
            .find(|t| t.id == tab_id)
            .map(|t| t.request_id.clone())
    }

    pub fn active_request(&self) -> Option<&RequestItem> {
        let rid = self.active_request_id()?;
        self.state.requests.iter().find(|r| r.id == rid)
    }

    pub fn submit(&mut self, command: Command) {
        match self.worker.commands.try_send(command) {
            Ok(()) => {
                self.pending = true;
                self.error = false;
                self.status = "Working locally…".into();
            }
            Err(TrySendError::Full(command)) => {
                self.restore_secret(command);
                self.error = true;
                self.status = "Worker busy; unsaved changes kept. Try again.".into();
            }
            Err(TrySendError::Disconnected(command)) => {
                self.restore_secret(command);
                self.error = true;
                self.status =
                    "Local service stopped. Keep unsaved edits; restart to recover.".into();
            }
        }
    }

    fn restore_secret(&mut self, command: Command) {
        if let Command::PutSecret { value, .. } = command {
            self.secret_input = String::from_utf8_lossy(value.expose()).into_owned();
        }
    }

    pub fn save_workspace(&mut self) {
        if self.loaded && !self.pending && !self.recovery_needed {
            self.submit(Command::SaveWorkspace {
                generation: self.generation,
                state: Box::new(self.state.clone()),
            });
        }
    }

    pub fn send_active_request(&mut self, confirmed_production: bool) {
        let Some(req) = self.active_request().cloned() else {
            return;
        };
        if !confirmed_production
            && self
                .state
                .active_environment()
                .is_some_and(|e| e.is_production)
            && req.method.is_mutating()
        {
            self.show_production_confirm = true;
            return;
        }
        self.show_production_confirm = false;
        let exec_id = self.next_execution_id;
        self.next_execution_id = self.next_execution_id.saturating_add(1);
        self.inflight_execution_id = Some(exec_id);
        self.inflight_request_id = Some(req.id.clone());
        let state = self.state.clone();
        self.submit(Command::SendHttp {
            execution_id: exec_id,
            request: Box::new(req),
            state: Box::new(state),
        });
    }

    pub fn cancel_inflight(&mut self) {
        if let Some(exec_id) = self.inflight_execution_id {
            let _ = self.worker.commands.try_send(Command::CancelHttp {
                execution_id: exec_id,
            });
            self.status = "Canceling request…".into();
        }
    }

    pub fn events(&mut self) {
        while let Ok(event) = self.worker.events.try_recv() {
            self.pending = false;
            self.error = false;
            match event {
                Event::Loaded {
                    snapshot,
                    workspace,
                    specs,
                    workspaces,
                    history,
                    secret_names,
                    storage_stats,
                    server_profiles,
                } => {
                    self.legacy_snapshot = snapshot;
                    self.workspace_id = workspace.meta.id.clone();
                    self.state = workspace;
                    self.specs = specs;
                    self.spec_editor_text = self
                        .specs
                        .first()
                        .map(|s| s.raw_text.clone())
                        .unwrap_or_default();
                    self.available_workspaces = workspaces;
                    self.history = history;
                    self.secret_names = secret_names;
                    self.storage_stats = storage_stats;
                    if server_profiles.active().is_some_and(|p| !p.session_id.is_empty()) {
                        self.show_welcome = false;
                    }
                    self.server_profiles = server_profiles;
                    self.generation = 0;
                    self.saved_generation = 0;
                    self.loaded = true;
                    self.status = "Local workspace ready".into();
                }
                Event::Recovered {
                    snapshot,
                    workspace,
                    specs,
                    workspaces,
                    history,
                    secret_names,
                    storage_stats,
                    server_profiles,
                } => {
                    self.legacy_snapshot = snapshot;
                    if !self.loaded || !self.dirty() {
                        self.state = workspace;
                        self.specs = specs;
                        self.saved_generation = self.generation;
                    }
                    self.available_workspaces = workspaces;
                    self.history = history;
                    self.secret_names = secret_names;
                    self.storage_stats = storage_stats;
                    if server_profiles.active().is_some_and(|p| !p.session_id.is_empty()) {
                        self.show_welcome = false;
                    }
                    self.server_profiles = server_profiles;
                    self.loaded = true;
                    self.recovery_needed = false;
                    self.confirm_quarantine = false;
                    self.status = "Durable data reopened".into();
                }
                Event::ServerProfilesUpdated { store, message } => {
                    if store.active().is_some_and(|p| !p.session_id.is_empty()) {
                        self.show_welcome = false;
                    }
                    self.server_profiles = store;
                    self.status = message;
                }
                Event::ServerSessionsLoaded(sessions) => {
                    self.server_sessions = sessions;
                    self.status = "Loaded active device sessions".into();
                }
                Event::TeamWorkspacesLoaded(list) => {
                    self.team_workspaces = list;
                    self.status = format!("Loaded {} team workspaces", self.team_workspaces.len());
                }
                Event::TeamWorkspaceDetailsLoaded { members, audit } => {
                    self.team_member_details = Some(members);
                    self.team_audit_events = audit;
                    self.status = "Loaded workspace members & audit".into();
                }
                Event::InvitationCreated(inv) => {
                    self.last_created_invite_token = inv.invite_token;
                    self.status = format!("Created one-time invitation for {}", inv.invitation.email);
                }
                Event::SyncStateUpdated { state, message } => {
                    self.sync_state = *state;
                    self.status = message;
                }
                Event::EntityRevisionsLoaded(revs) => {
                    self.team_revisions = revs;
                    self.status = format!("Loaded {} historical revisions", self.team_revisions.len());
                }
                Event::CollabDataLoaded {
                    comments,
                    notifications,
                    unread_count,
                } => {
                    self.team_comments = comments;
                    self.team_notifications = notifications;
                    self.team_unread_notifications = unread_count;
                    self.status = "Loaded workspace comments & notifications".into();
                }
                Event::SpecsSaved => {
                    self.status = "OpenAPI specifications saved locally".into();
                }
                Event::SpecFetched(doc) => {
                    self.spec_editor_text = doc.raw_text.clone();
                    self.specs.push(*doc);
                    self.selected_spec_idx = self.specs.len() - 1;
                    self.submit(Command::SaveSpecs {
                        specs: self.specs.clone(),
                    });
                    self.status = "Fetched specification from URL".into();
                }
                Event::Saved {
                    generation,
                    revision,
                } => {
                    self.saved_generation = generation;
                    self.legacy_snapshot.revision = revision;
                    self.status = "Draft saved locally".into();
                }
                Event::WorkspaceSaved {
                    generation,
                    revision,
                    storage_stats,
                } => {
                    self.saved_generation = generation;
                    self.state.revision = revision;
                    self.storage_stats = storage_stats;
                    self.status = if self.dirty() {
                        "Earlier version saved; newer edits are unsaved"
                    } else {
                        "All changes saved locally"
                    }
                    .into();
                }
                Event::SecretSaved { secret_names } => {
                    self.secret_names = secret_names;
                    self.status = "Secret saved with Windows account protection".into();
                }
                Event::SecretLoaded(secret) => {
                    self.revealed = Some(secret);
                    self.status = "Secret revealed locally; hide when finished".into();
                }
                Event::SecretDeleted { name, secret_names } => {
                    self.secret_names = secret_names;
                    self.status = format!("Deleted local secret '{}'", name.as_str());
                }
                Event::HttpProgress {
                    execution_id,
                    phase,
                    bytes_done,
                    total_bytes,
                } => {
                    if self.inflight_execution_id == Some(execution_id) {
                        self.transfer_progress = Some((execution_id, phase, bytes_done, total_bytes));
                    }
                }
                Event::OAuth2PendingAuthorize {
                    authorize_url,
                    redirect_uri,
                } => {
                    self.oauth_authorize_url = authorize_url;
                    self.status = format!("OAuth2 PKCE callback listening at {redirect_uri}");
                }
                Event::OAuth2TokenReceived(tok) => {
                    if let Ok(id) = LocalId::parse("oauth_access_token")
                        && let Ok(sec) = Secret::new(tok.access_token.as_bytes().to_vec())
                    {
                        self.submit(Command::PutSecret {
                            name: id,
                            value: sec,
                        });
                    }
                    self.status = format!(
                        "OAuth2 token received ({}; saved locally as secret 'oauth_access_token')",
                        tok.token_type
                    );
                }
                Event::CollectionRunFinished(rep) => {
                    self.runner_running = false;
                    self.status = format!(
                        "Runner complete: passed={} failed_assert={} failed_contract={} skipped={} infra={}",
                        rep.passed,
                        rep.failed_assertions,
                        rep.failed_contract,
                        rep.skipped,
                        rep.infra_errors
                    );
                    self.last_run_report = Some(*rep);
                }
                Event::LocalMockState { base_url, logs } => {
                    self.status = match &base_url {
                        Some(u) => format!("Local mock server active at {u} ({} log entries)", logs.len()),
                        None => "Local mock server stopped".into(),
                    };
                    self.local_mock_url = base_url;
                    self.local_mock_logs = logs;
                }
                Event::GraphQlFinished(res) => {
                    self.status = format!(
                        "GraphQL HTTP {} ({} ms · {} error(s))",
                        res.http_status,
                        res.duration_ms,
                        res.errors.len()
                    );
                    self.gql_result = Some(res);
                }
                Event::SseEventReceived(ev) => {
                    if self.sse_events.len() >= self.sse_max_events.max(10) {
                        self.sse_events.remove(0);
                    }
                    self.sse_events.push(ev);
                }
                Event::SseStopped(msg) => {
                    self.sse_running = false;
                    self.status = msg;
                }
                Event::HttpCompleted {
                    execution_id,
                    request_id,
                    response,
                    extracted,
                    history,
                    storage_stats,
                    ..
                } => {
                    if self.inflight_execution_id == Some(execution_id) {
                        self.inflight_execution_id = None;
                        self.inflight_request_id = None;
                        self.transfer_progress = None;
                    }
                    self.history = history;
                    self.storage_stats = storage_stats;
                    if !response.captured_cookies.is_empty() {
                        for ck in &response.captured_cookies {
                            self.state.cookies.retain(|c| {
                                !(c.domain.eq_ignore_ascii_case(&ck.domain)
                                    && c.path == ck.path
                                    && c.name == ck.name)
                            });
                            self.state.cookies.push(ck.clone());
                        }
                    }
                    let extracted_count = extracted.len();
                    if extracted_count > 0
                        && let Some(env_id) = self.state.active_environment_id.clone()
                        && let Some(env) =
                            self.state.environments.iter_mut().find(|e| e.id == env_id)
                    {
                        for (k, v, is_secret) in &extracted {
                            if *is_secret || v.is_empty() {
                                continue;
                            }
                            if let Some(existing) =
                                env.variables.iter_mut().find(|row| row.key == *k)
                            {
                                existing.shared_value = v.clone();
                            } else {
                                env.variables.push(
                                    swaggen_domain::workspace::VariableEntry::public(k, v.clone()),
                                );
                            }
                        }
                    }
                    self.response_format = if response.is_binary {
                        ResponseBodyFormat::Binary
                    } else {
                        let txt = String::from_utf8_lossy(&response.preview_bytes);
                        let trimmed = txt.trim_start();
                        if trimmed.starts_with('{') || trimmed.starts_with('[') {
                            ResponseBodyFormat::PrettyJson
                        } else if trimmed.starts_with('<') {
                            ResponseBodyFormat::Xml
                        } else {
                            ResponseBodyFormat::Raw
                        }
                    };
                    self.status = format!(
                        "HTTP {} ({} ms · {} bytes{})",
                        response.status_code,
                        response.duration_ms,
                        response.decoded_bytes,
                        if extracted_count > 0 {
                            format!(" · {extracted_count} var(s) extracted")
                        } else {
                            String::new()
                        }
                    );
                    if let Some(Ok(prev)) = self.responses.remove(&request_id) {
                        self.previous_responses.insert(request_id.clone(), prev);
                    }
                    self.responses.insert(request_id, Ok(response));
                }
                Event::HttpFailed {
                    execution_id,
                    request_id,
                    error,
                    history,
                    ..
                } => {
                    if self.inflight_execution_id == Some(execution_id) {
                        self.inflight_execution_id = None;
                        self.inflight_request_id = None;
                        self.transfer_progress = None;
                    }
                    self.history = history;
                    self.error = true;
                    self.status = format!("{}: {}", error.kind.as_str(), error.message);
                    self.responses.insert(request_id, Err(error));
                }
                Event::HistoryUpdated {
                    history,
                    storage_stats,
                } => {
                    self.history = history;
                    self.storage_stats = storage_stats;
                    self.status = "Request history updated".into();
                }
                Event::CacheCleaned {
                    removed_bytes,
                    history,
                    storage_stats,
                } => {
                    self.history = history;
                    self.storage_stats = storage_stats;
                    self.status = format!(
                        "Cleaned {removed_bytes} bytes of cache/history (projects & secrets preserved)"
                    );
                }
                Event::FileSaved { message } => {
                    self.status = message;
                }
                Event::DataMigrated { new_root } => {
                    self.root = new_root;
                    self.status = "Workspace data folder migrated safely".into();
                }
                Event::SecretFailed { error, value } => {
                    self.secret_input = String::from_utf8_lossy(value.expose()).into_owned();
                    self.error = true;
                    self.status = error.to_string();
                    self.recovery_needed = matches!(error, Error::Storage | Error::Recovery);
                }
                Event::Failed(error) => {
                    self.error = true;
                    self.status = error.to_string();
                    self.recovery_needed = !self.loaded
                        || matches!(error, Error::Storage | Error::Recovery | Error::Version);
                }
                Event::ActionError(msg) => {
                    self.error = true;
                    self.status = msg;
                }
            }
        }
    }

    pub fn diagnostic_preview(&self) -> DiagnosticPreview {
        diagnostics::build_preview(
            &self.workspace_id,
            self.loaded,
            self.pending,
            self.recovery_needed,
            self.state.revision,
            self.secret_names.len(),
            &self.policy,
        )
    }

    pub fn masked_secret_placeholders(&self) -> BTreeMap<LocalId, String> {
        let mut map = BTreeMap::new();
        for id in &self.secret_names {
            map.insert(id.clone(), "••••••••".into());
        }
        map
    }

    pub fn user_initials(&self) -> String {
        if let Some(prof) = self.server_profiles.active() {
            let source = if !prof.display_name.trim().is_empty() {
                prof.display_name.trim()
            } else if !prof.email.trim().is_empty() {
                prof.email.trim()
            } else {
                ""
            };
            if !source.is_empty() {
                let letters: String = source
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.chars().next())
                    .take(2)
                    .collect::<String>()
                    .to_ascii_uppercase();
                if !letters.is_empty() {
                    return letters;
                }
            }
        }
        "SW".into()
    }

    pub fn screen_breadcrumb(&self) -> String {
        let leaf = match self.mode {
            MainMode::Workbench => self
                .active_request_id()
                .and_then(|id| self.state.requests.iter().find(|r| r.id == id))
                .map(|r| r.name.clone())
                .unwrap_or_else(|| "API Workbench".into()),
            MainMode::Runner => "Testing & Developer Tools".into(),
            MainMode::Server => "Team Server & Accounts".into(),
            MainMode::TeamSync => "Collaboration & Sync".into(),
            MainMode::Specs => "API Definitions & Docs".into(),
            MainMode::ImportExport => "Import & Export".into(),
            MainMode::Environments => "Environments".into(),
            MainMode::History => "Request History".into(),
            MainMode::Cookies => "Workspace Cookies".into(),
            MainMode::Secrets => "Personal Secrets".into(),
            MainMode::Sanitizer => "Secret Sanitizer".into(),
            MainMode::Storage => "Storage & Retention".into(),
            MainMode::Privacy => "Privacy & Updates".into(),
            MainMode::Shortcuts => "Keyboard Shortcuts".into(),
        };
        format!("{} / {leaf}", self.state.meta.name)
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.events();

        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && (self.dirty() || self.pending || !self.secret_input.is_empty())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_dialog = true;
        }
        if self.close_dialog {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(swaggen_ui_kit::page_bg(ui))
                        .inner_margin(48),
                )
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.set_max_width(520.0);
                        swaggen_ui_kit::surface_card(ui, |ui| {
                            swaggen_ui_kit::section(
                                ui,
                                "Keep your unsaved changes?",
                                "Return to the workspace and save before closing, or discard unsaved edits.",
                            );
                            ui.horizontal(|ui| {
                                if swaggen_ui_kit::primary_button(ui, true, "Return to workspace")
                                    .clicked()
                                {
                                    self.close_dialog = false;
                                }
                                if swaggen_ui_kit::danger_button(
                                    ui,
                                    !self.pending,
                                    "Close without saving",
                                )
                                .clicked()
                                {
                                    self.allow_close = true;
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            });
                        });
                    });
                });
            return;
        }

        // Figma A01 / Welcome to Swaggen (Firebase Google + Email Sign-In).
        if self.show_welcome {
            views::welcome::show(self, ui);
            return;
        }

        // Keyboard shortcuts (UX03, UX04, UX07).
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::K)) {
            self.show_command_palette = !self.show_command_palette;
            self.command_palette_query.clear();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.show_command_palette = false;
            self.show_workspace_modal = false;
        }
        // OS file drag-and-drop safe OpenAPI/Postman import (UX08).
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for df in dropped {
            let path = df.path();
            if let Ok(raw) = std::fs::read_to_string(path) {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("dropped-spec")
                    .to_owned();
                let doc = swaggen_spec_engine::openapi::parse_and_validate(
                    swaggen_domain::workspace::new_uid(),
                    name.clone(),
                    &raw,
                    &[],
                );
                self.spec_editor_text = doc.raw_text.clone();
                self.specs.push(doc);
                self.selected_spec_idx = self.specs.len() - 1;
                self.mode = MainMode::Specs;
                self.status = format!("Imported dropped file '{name}' safely (UX08)");
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S)) {
            self.save_workspace();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Enter)) {
            self.send_active_request(false);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::P)) {
            self.mode = MainMode::Workbench;
            self.focus_search = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::W))
            && let Some(active_id) = self.state.active_tab_id.clone()
        {
            self.state
                .open_tabs
                .retain(|t| t.id != active_id || t.pinned);
            self.state.active_tab_id = self.state.open_tabs.first().map(|t| t.id.clone());
        }

        // 1. Full-width Figma Statusbar (28px height, single top 1px divider).
        let status_bar_frame = egui::Frame::new()
            .fill(swaggen_ui_kit::surface_bg(ui))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(20, 6));
        egui::Panel::bottom("status")
            .frame(status_bar_frame)
            .show(ui, |ui| {
                swaggen_ui_kit::paint_edge_divider(ui, ui.max_rect(), Edge::Top);
                ui.horizontal(|ui| {
                    if self.pending || self.inflight_execution_id.is_some() {
                        ui.spinner();
                    }
                    let left_status = if let Some(prof) = self.server_profiles.active() {
                        if self.error {
                            self.status.clone()
                        } else {
                            let host = prof
                                .base_url
                                .strip_prefix("https://")
                                .or_else(|| prof.base_url.strip_prefix("http://"))
                                .unwrap_or(&prof.base_url);
                            format!("{} · {host}", self.status)
                        }
                    } else {
                        self.status.clone()
                    };
                    if self.error {
                        ui.label(
                            egui::RichText::new(left_status)
                                .size(11.0)
                                .color(Tokens::TEXT_DANGER),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(left_status)
                                .size(11.0)
                                .color(swaggen_ui_kit::text_primary(ui)),
                        );
                    }
                    if self.dirty() {
                        ui.label(
                            egui::RichText::new(" · Unsaved draft")
                                .size(11.0)
                                .color(Tokens::TEXT_WARNING),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "Rev {}  /  Rust engine  /  UTF-8  /  Windows",
                                self.state.revision
                            ))
                            .size(11.0)
                            .color(swaggen_ui_kit::text_secondary(ui)),
                        );
                    });
                });
                if self.recovery_needed {
                    ui.add_space(6.0);
                    swaggen_ui_kit::notice_banner(
                        ui,
                        NoticeKind::Warning,
                        "Local recovery",
                        "Editor state is preserved. Reopen verifies durable files without overwriting.",
                    );
                    ui.horizontal_wrapped(|ui| {
                        if swaggen_ui_kit::primary_button(ui, !self.pending, "Retry local recovery")
                            .clicked()
                        {
                            self.hide_secret();
                            self.submit(Command::Reopen);
                        }
                        if !self.confirm_quarantine {
                            if swaggen_ui_kit::danger_button(
                                ui,
                                !self.pending,
                                "Quarantine corrupt journal…",
                            )
                            .clicked()
                            {
                                self.confirm_quarantine = true;
                            }
                        } else {
                            if swaggen_ui_kit::danger_button(
                                ui,
                                !self.pending,
                                "Confirm quarantine & start clean",
                            )
                            .clicked()
                            {
                                self.hide_secret();
                                self.submit(Command::QuarantineCorruptDraft);
                            }
                            if swaggen_ui_kit::secondary_button(ui, true, "Cancel").clicked() {
                                self.confirm_quarantine = false;
                            }
                        }
                    });
                }
            });

        // 2. Left 64px Figma Activity Rail (single right 1px divider).
        let rail_frame = egui::Frame::new()
            .fill(swaggen_ui_kit::surface_bg(ui))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(12, 16));
        egui::Panel::left("activity_rail")
            .exact_size(64.0)
            .resizable(false)
            .frame(rail_frame)
            .show(ui, |ui| {
                swaggen_ui_kit::paint_edge_divider(ui, ui.max_rect(), Edge::Right);
                ui.vertical_centered(|ui| {
                    swaggen_ui_kit::brand_mark(ui, 32.0);
                    ui.add_space(16.0);
                    for (mode, icon, tip) in [
                        (
                            MainMode::Workbench,
                            IconKind::Blocks,
                            "API Workbench (Ctrl+P)",
                        ),
                        (MainMode::Specs, IconKind::BookOpen, "API Definitions & Docs"),
                        (
                            MainMode::Runner,
                            IconKind::Play,
                            "Testing, Mocks & Protocols",
                        ),
                        (
                            MainMode::TeamSync,
                            IconKind::GitBranch,
                            "Sync, Branches & Comments",
                        ),
                        (MainMode::Server, IconKind::Users, "Team Server & Accounts"),
                        (
                            MainMode::Environments,
                            IconKind::Globe,
                            "Environments & Variables",
                        ),
                        (
                            MainMode::ImportExport,
                            IconKind::Terminal,
                            "Import, Export & Developer Tools",
                        ),
                        (
                            MainMode::Storage,
                            IconKind::Settings,
                            "Settings, Privacy & Shortcuts",
                        ),
                    ] {
                        let active = match mode {
                            MainMode::Storage => matches!(
                                self.mode,
                                MainMode::Storage
                                    | MainMode::Privacy
                                    | MainMode::Shortcuts
                                    | MainMode::Sanitizer
                            ),
                            MainMode::Environments => matches!(
                                self.mode,
                                MainMode::Environments
                                    | MainMode::Secrets
                                    | MainMode::Cookies
                                    | MainMode::History
                            ),
                            _ => self.mode == mode,
                        };
                        if swaggen_ui_kit::rail_icon_button(ui, active, icon, tip).clicked() {
                            self.hide_secret();
                            self.mode = mode;
                        }
                        ui.add_space(8.0);
                    }
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                        let initials = self.user_initials();
                        if swaggen_ui_kit::avatar_circle(ui, &initials, AvatarTone::Cream)
                            .on_hover_text("Firebase account & sessions")
                            .clicked()
                        {
                            self.hide_secret();
                            if self
                                .server_profiles
                                .active()
                                .is_none_or(|p| p.session_id.is_empty())
                            {
                                self.show_welcome = true;
                            } else {
                                self.mode = MainMode::Server;
                            }
                        }
                    });
                });
            });

        // 3. Left 248px Figma Project Sidebar (single right 1px divider).
        let sidebar_frame = egui::Frame::new()
            .fill(swaggen_ui_kit::surface_bg(ui))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(16, 18));
        let sidebar_resp = egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(self.state.layout.sidebar_width.clamp(228.0, 340.0))
            .min_size(220.0)
            .max_size(360.0)
            .frame(sidebar_frame)
            .show(ui, |ui| {
                swaggen_ui_kit::paint_edge_divider(ui, ui.max_rect(), Edge::Right);
                views::sidebar::show(self, ui);
            });
        let new_w = sidebar_resp.response.rect.width();
        if (new_w - self.state.layout.sidebar_width).abs() > 2.0 {
            self.state.layout.sidebar_width = new_w;
        }

        // 4. Right Workspace Header (64px height matching Figma Shell/Workspace header, single bottom 1px divider).
        let header_frame = egui::Frame::new()
            .fill(swaggen_ui_kit::surface_bg(ui))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(24, 14));
        let signed_in = self
            .server_profiles
            .active()
            .is_some_and(|p| !p.session_id.is_empty());
        egui::Panel::top("workspace_header")
            .frame(header_frame)
            .show(ui, |ui| {
                swaggen_ui_kit::paint_edge_divider(ui, ui.max_rect(), Edge::Bottom);
                ui.horizontal(|ui| {
                    ui.label(swaggen_ui_kit::bold_text(
                        &self.screen_breadcrumb(),
                        13.0,
                        swaggen_ui_kit::text_primary(ui),
                    ));
                    if self.state.meta.archived {
                        swaggen_ui_kit::pill_badge(ui, Tokens::TEXT_WARNING, "Archived");
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if signed_in {
                            let initials = self.user_initials();
                            if swaggen_ui_kit::avatar_circle(ui, &initials, AvatarTone::Salmon)
                                .on_hover_text("Signed-in Firebase account")
                                .clicked()
                            {
                                self.mode = MainMode::Server;
                                self.server_subtab = 3;
                            }
                        } else if swaggen_ui_kit::primary_button(ui, true, "Sign in").clicked() {
                            self.show_welcome = true;
                        }
                        if swaggen_ui_kit::secondary_button(
                            ui,
                            self.loaded && !self.pending && !self.recovery_needed,
                            if self.dirty() {
                                "Save*"
                            } else {
                                "Save"
                            },
                        )
                        .clicked()
                        {
                            self.save_workspace();
                        }
                        if swaggen_ui_kit::secondary_button(
                            ui,
                            true,
                            swaggen_ui_kit::tr(self.locale, "Workspaces", "Không gian"),
                        )
                        .clicked()
                        {
                            self.show_workspace_modal = !self.show_workspace_modal;
                        }
                        if swaggen_ui_kit::secondary_button(
                            ui,
                            true,
                            if self.locale == swaggen_ui_kit::UiLocale::En {
                                "VI"
                            } else {
                                "EN"
                            },
                        )
                        .clicked()
                        {
                            self.locale = if self.locale == swaggen_ui_kit::UiLocale::En {
                                swaggen_ui_kit::UiLocale::Vi
                            } else {
                                swaggen_ui_kit::UiLocale::En
                            };
                        }
                        if swaggen_ui_kit::secondary_button(
                            ui,
                            true,
                            if self.theme == swaggen_ui_kit::UiTheme::Light {
                                "Dark"
                            } else {
                                "Light"
                            },
                        )
                        .clicked()
                        {
                            self.theme = if self.theme == swaggen_ui_kit::UiTheme::Light {
                                swaggen_ui_kit::UiTheme::Dark
                            } else {
                                swaggen_ui_kit::UiTheme::Light
                            };
                            swaggen_ui_kit::install_with_theme(&ctx, self.theme);
                        }
                        let (sync_color, sync_label) = match self.server_profiles.active() {
                            Some(p) if !p.session_id.is_empty() && p.mongo_ready => {
                                (Tokens::STATUS_SUCCESS, "Synced")
                            }
                            Some(p) if p.mongo_ready => (Tokens::STATUS_SUCCESS, "Server OK"),
                            Some(_) => (Tokens::STATUS_INFO, "Configured"),
                            None => (swaggen_ui_kit::text_secondary(ui), "Local"),
                        };
                        swaggen_ui_kit::pill_badge(ui, sync_color, sync_label);
                        let env_label = self
                            .state
                            .active_environment()
                            .map(|e| e.name.as_str())
                            .unwrap_or("No env");
                        swaggen_ui_kit::pill_badge(
                            ui,
                            swaggen_ui_kit::text_primary(ui),
                            env_label,
                        );
                    });
                });
            });

        let main_frame = egui::Frame::new()
            .fill(swaggen_ui_kit::page_bg(ui))
            .inner_margin(egui::Margin::symmetric(24, 20));

        // Production request confirmation modal (R16 / EV07).
        if self.show_production_confirm {
            egui::CentralPanel::default().frame(main_frame).show(ui, |ui| {
                views::panels::show_production_confirm(self, ui);
            });
            return;
        }

        // Command Palette modal (UX04 / M03).
        if self.show_command_palette {
            egui::CentralPanel::default().frame(main_frame).show(ui, |ui| {
                views::panels::show_command_palette(self, ui);
            });
            return;
        }

        // Workspace management modal (WS01 / A08 / A09).
        if self.show_workspace_modal {
            egui::CentralPanel::default().frame(main_frame).show(ui, |ui| {
                views::panels::show_workspace_manager(self, ui);
            });
            return;
        }

        // 5. Main Central Workspace Canvas.
        egui::CentralPanel::default()
            .frame(main_frame)
            .show(ui, |ui| match self.mode {
                MainMode::Workbench => {
                    egui::ScrollArea::vertical()
                        .id_salt("workbench-scroll")
                        .show(ui, |ui| {
                            views::request_pane::show(self, ui);
                            ui.add_space(14.0);
                            views::response_pane::show(self, ui, &ctx);
                        });
                }
                MainMode::Runner => views::runner::show(self, ui, &ctx),
                MainMode::Server => views::server::show(self, ui),
                MainMode::TeamSync => views::sync::show(self, ui),
                MainMode::Specs => views::specs::show(self, ui),
                MainMode::ImportExport => views::import_export::show(self, ui, &ctx),
                MainMode::Environments => views::panels::show_environments(self, ui),
                MainMode::History => views::panels::show_history(self, ui),
                MainMode::Cookies => views::panels::show_cookies(self, ui),
                MainMode::Secrets => views::panels::show_secrets(self, ui),
                MainMode::Sanitizer => views::panels::show_sanitizer(self, ui),
                MainMode::Storage => views::panels::show_storage(self, ui),
                MainMode::Privacy => views::panels::show_privacy(self, ui, &ctx),
                MainMode::Shortcuts => views::panels::show_shortcuts(ui),
            });
    }
}
