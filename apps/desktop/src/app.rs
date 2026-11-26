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
    GraphQl,
    Params,
    Headers,
    Body,
    Auth,
    Scripts,
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
    pub workspace_summaries: Vec<crate::worker::WorkspaceListEntry>,
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
    pub close_after_save: bool,
    pub firebase_auth_pending: bool,
    pub request_config_collapsed: bool,
    pub response_maximized: bool,
    pub dragging_request_id: Option<String>,

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
    pub pending_invitations: Vec<swaggen_api_contract::InvitationSummary>,
    pub team_workspaces: Vec<swaggen_api_contract::TeamWorkspaceSummary>,
    pub selected_team_workspace_id: Option<String>,
    pub team_ws_slug: String,
    pub team_ws_name: String,
    pub team_member_details: Option<swaggen_api_contract::MemberListResponse>,
    pub team_audit_events: Vec<swaggen_api_contract::AuditEventRecord>,
    pub invite_email_input: String,
    pub invite_role: swaggen_api_contract::Role,
    pub last_created_invite_token: String,
    pub public_invite_links: Vec<swaggen_api_contract::PublicInviteLinkSummary>,
    pub invite_link_role: swaggen_api_contract::Role,
    pub invite_link_expiry_hours: u32,
    pub invite_link_max_uses_input: String,
    pub invite_link_paste_input: String,
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
    pub publish_history: Vec<swaggen_api_contract::ChangeEventRecord>,
    pub preview_history_cursor: Option<i64>,
    pub expanded_history_cursors: std::collections::BTreeSet<i64>,
    pub cherry_pick_selections:
        HashMap<i64, std::collections::BTreeSet<swaggen_domain::workspace::RequestCherryPickHunk>>,

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
    pub graphql_schemas: HashMap<String, swaggen_domain::graphql::GraphQlSchemaSummary>,
    pub graphql_introspecting_id: Option<String>,
    pub gql_split_ratio: f32,
    pub sse_url: String,
    pub sse_max_events: usize,
    pub sse_running: bool,
    pub sse_events: Vec<swaggen_runner::protocols_and_tools::SseEventRecord>,
    pub dev_util_input: String,
    pub dev_util_output: String,

    // P09 UX B, Governance, Command Palette, Theme/Locale & Deep Link / Update Verification
    pub show_command_palette: bool,
    pub command_palette_query: String,
    pub locale: swaggen_ui_kit::UiLocale,
    pub deep_link_input: String,
    pub update_feed_url: String,
    pub update_check: Option<crate::updater::UpdateCheckOutcome>,
    pub local_update_path: String,
    pub local_update_sha256: String,
    pub update_silent_install: bool,
    pub update_manifest_json: String,
    pub update_signing_key: String,
    pub update_payload_text: String,

    // P10 Sandboxed Script + Extensions + v0.1.16 UX/Automation UI state
    pub sandbox_script_text: String,
    pub sandbox_report: Option<swaggen_runner::sandbox::SandboxExecutionReport>,
    pub extensions: Vec<swaggen_domain::extensions::ExtensionManifest>,
    pub script_subtab_post: bool,
    pub script_reports: HashMap<
        String,
        (
            Option<swaggen_runner::sandbox::SandboxExecutionReport>,
            Option<swaggen_runner::sandbox::SandboxExecutionReport>,
        ),
    >,
    pub env_scope_tab: usize,
    pub selected_collection_var_idx: usize,
    pub quick_var_name: String,
    pub quick_var_value: String,
    pub spec_view_mode: usize,
    pub spec_local_file_path: String,
    pub runner_test_kind: usize,
    pub runner_selected_requests: std::collections::BTreeSet<String>,
    pub runner_concurrency: usize,
    pub runner_selected_step_idx: usize,
    pub runner_trace_tab: usize,
    pub mock_port_input: String,
    pub mock_scenarios: Vec<swaggen_runner::advanced_protocols::MockScenarioRule>,
    pub protocol_subtab: usize,
    pub ws_url_input: String,
    pub ws_message_input: String,
    pub ws_logs: Vec<String>,
    pub grpc_endpoint_input: String,
    pub grpc_method_input: String,
    pub grpc_payload_input: String,
    pub grpc_output: String,
    pub import_hub_tab: usize,
    pub import_source_kind: usize,
    pub import_file_path: String,
    pub show_sidebar_trash: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, root: PathBuf, workspace_id: LocalId) -> Self {
        swaggen_ui_kit::install(&cc.egui_ctx);
        let context = cc.egui_ctx.clone();
        let worker = Worker::start(root.clone(), workspace_id.clone(), move || {
            context.request_repaint();
        });
        let default_display_name = if workspace_id.as_str() == "personal" {
            "Personal".to_owned()
        } else {
            workspace_id.as_str().to_owned()
        };
        let initial_state =
            WorkspaceState::new_personal(workspace_id.clone(), default_display_name);
        Self {
            worker,
            root: root.clone(),
            workspace_id: workspace_id.clone(),
            available_workspaces: vec![workspace_id.clone()],
            workspace_summaries: Vec::new(),
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
            close_after_save: false,
            firebase_auth_pending: false,
            request_config_collapsed: false,
            response_maximized: false,
            dragging_request_id: None,
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
            pending_invitations: Vec::new(),
            team_workspaces: Vec::new(),
            selected_team_workspace_id: None,
            team_ws_slug: "team-api".into(),
            team_ws_name: "Team API".into(),
            team_member_details: None,
            team_audit_events: Vec::new(),
            invite_email_input: String::new(),
            invite_role: swaggen_api_contract::Role::Editor,
            last_created_invite_token: String::new(),
            public_invite_links: Vec::new(),
            invite_link_role: swaggen_api_contract::Role::Editor,
            invite_link_expiry_hours: 72,
            invite_link_max_uses_input: String::new(),
            invite_link_paste_input: String::new(),
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
            publish_history: Vec::new(),
            preview_history_cursor: None,
            expanded_history_cursors: std::collections::BTreeSet::new(),
            cherry_pick_selections: HashMap::new(),
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
            graphql_schemas: HashMap::new(),
            graphql_introspecting_id: None,
            gql_split_ratio: 0.60,
            sse_url: "http://127.0.0.1:8080/events".into(),
            sse_max_events: 200,
            sse_running: false,
            sse_events: Vec::new(),
            dev_util_input: "hello: swaggen".into(),
            dev_util_output: String::new(),
            show_command_palette: false,
            command_palette_query: String::new(),
            locale: swaggen_ui_kit::UiLocale::En,
            deep_link_input: "swaggen://workspace/personal/mode/runner".into(),
            update_feed_url: crate::updater::DEFAULT_UPDATE_FEED_URL.into(),
            update_check: None,
            local_update_path: String::new(),
            local_update_sha256: String::new(),
            update_silent_install: true,
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
            script_subtab_post: true,
            script_reports: HashMap::new(),
            env_scope_tab: 0,
            selected_collection_var_idx: 0,
            quick_var_name: String::new(),
            quick_var_value: String::new(),
            spec_view_mode: 0,
            spec_local_file_path: String::new(),
            runner_test_kind: 0,
            runner_selected_requests: std::collections::BTreeSet::new(),
            runner_concurrency: 1,
            runner_selected_step_idx: 0,
            runner_trace_tab: 0,
            mock_port_input: "0".into(),
            mock_scenarios: Vec::new(),
            protocol_subtab: 0,
            ws_url_input: "ws://127.0.0.1:8080/ws".into(),
            ws_message_input: "{\"ping\":1}".into(),
            ws_logs: Vec::new(),
            grpc_endpoint_input: "http://127.0.0.1:50051".into(),
            grpc_method_input: "greeter.Greeter/SayHello".into(),
            grpc_payload_input: "Swaggen".into(),
            grpc_output: String::new(),
            import_hub_tab: 0,
            import_source_kind: 0,
            import_file_path: String::new(),
            show_sidebar_trash: false,
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

    pub fn navigate_to(&mut self, mode: MainMode) {
        self.hide_secret();
        self.show_workspace_modal = false;
        self.show_command_palette = false;
        self.show_production_confirm = false;
        self.mode = mode;
        if mode == MainMode::Workbench
            && self.active_request_id().is_none()
            && let Some(first_id) = self.state.requests.first().map(|r| r.id.clone())
        {
            self.state.open_or_focus_tab(&first_id);
        }
        if mode == MainMode::TeamSync
            && let Some((pid, wid)) = self.active_team_target()
        {
            self.submit(Command::LoadPublishHistory {
                profile_id: pid,
                workspace_id: wid,
            });
        }
    }

    pub fn active_team_target(&self) -> Option<(String, String)> {
        if let swaggen_domain::workspace::WorkspaceKind::Team {
            server_profile_id: Some(spid),
            remote_workspace_id: Some(rwid),
        } = &self.state.meta.kind
        {
            let signed_in = self
                .server_profiles
                .profiles
                .iter()
                .any(|p| p.id == *spid && !p.session_id.is_empty());
            if signed_in {
                return Some((spid.clone(), rwid.clone()));
            }
        }
        let prof = self
            .server_profiles
            .active()
            .filter(|p| !p.session_id.is_empty())?;
        let wid = self.selected_team_workspace_id.as_ref()?;
        Some((prof.id.clone(), wid.clone()))
    }

    pub fn open_team_workspace(
        &mut self,
        profile_id: &str,
        workspace_id: &str,
        workspace_name: &str,
    ) {
        self.selected_team_workspace_id = Some(workspace_id.to_owned());
        self.state.meta.kind = swaggen_domain::workspace::WorkspaceKind::Team {
            server_profile_id: Some(profile_id.to_owned()),
            remote_workspace_id: Some(workspace_id.to_owned()),
        };
        if !workspace_name.trim().is_empty() {
            self.state.meta.name = workspace_name.to_owned();
        }
        self.submit(Command::LoadTeamWorkspaceDetails {
            profile_id: profile_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
        });
        self.submit(Command::LoadPublishHistory {
            profile_id: profile_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
        });
    }

    pub fn publish_current_on_save(&mut self, custom_note: Option<&str>) {
        let Some((profile_id, workspace_id)) = self.active_team_target() else {
            return;
        };
        let note = custom_note
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Ctrl+S")
            .to_owned();
        if let Some(req) = self.active_request().cloned() {
            let payload_json = serde_json::to_string(&req).unwrap_or_else(|_| "{}".into());
            self.submit(Command::PublishSequentialEvent {
                profile_id,
                workspace_id,
                entity_id: req.id.clone(),
                kind: "request".into(),
                name: format!("{} {}", req.method.as_str(), req.name),
                payload_json,
                note,
            });
        } else {
            let payload_json = serde_json::to_string(&self.state).unwrap_or_else(|_| "{}".into());
            self.submit(Command::PublishSequentialEvent {
                profile_id,
                workspace_id,
                entity_id: "workspace-snapshot".into(),
                kind: "workspace".into(),
                name: self.state.meta.name.clone(),
                payload_json,
                note,
            });
        }
    }

    pub fn cherry_pick_publish_event(&mut self, ev: &swaggen_api_contract::ChangeEventRecord) {
        let Some(raw) = ev.payload_json.as_deref() else {
            self.error = true;
            self.status = format!("Event #{} has no payload to restore.", ev.cursor);
            return;
        };
        if ev.entity_kind == "request" {
            match serde_json::from_str::<RequestItem>(raw) {
                Ok(mut req) => {
                    if !self
                        .state
                        .collections
                        .iter()
                        .any(|c| c.id == req.collection_id)
                    {
                        req.collection_id = self
                            .state
                            .collections
                            .first()
                            .map(|c| c.id.clone())
                            .unwrap_or_else(|| "default".into());
                    }
                    if let Some(existing) = self.state.requests.iter_mut().find(|r| r.id == req.id)
                    {
                        *existing = req.clone();
                    } else {
                        self.state.requests.push(req.clone());
                    }
                    self.open_request_in_workbench(&req.id);
                    self.mark_dirty();
                    self.save_workspace_local_only();
                    if let Some((profile_id, workspace_id)) = self.active_team_target() {
                        self.submit(Command::PublishSequentialEvent {
                            profile_id,
                            workspace_id,
                            entity_id: req.id.clone(),
                            kind: "request".into(),
                            name: format!("{} {}", req.method.as_str(), req.name),
                            payload_json: serde_json::to_string(&req)
                                .unwrap_or_else(|_| "{}".into()),
                            note: format!("Cherry-pick #{} (r{})", ev.cursor, ev.revision),
                        });
                    }
                    self.error = false;
                    self.status = format!(
                        "Cherry-picked #{} ({}) into Workbench",
                        ev.cursor, ev.entity_name
                    );
                }
                Err(e) => {
                    self.error = true;
                    self.status =
                        format!("Failed to parse request payload from #{}: {e}", ev.cursor);
                }
            }
        } else if ev.entity_kind == "workspace" {
            match serde_json::from_str::<WorkspaceState>(raw) {
                Ok(mut ws) => {
                    ws.meta = self.state.meta.clone();
                    self.state = ws;
                    if self.active_request_id().is_none()
                        && let Some(first_id) = self.state.requests.first().map(|r| r.id.clone())
                    {
                        self.state.open_or_focus_tab(&first_id);
                    }
                    self.mark_dirty();
                    self.save_workspace_local_only();
                    self.error = false;
                    self.status = format!("Cherry-picked workspace snapshot #{}", ev.cursor);
                }
                Err(e) => {
                    self.error = true;
                    self.status = format!("Failed to parse workspace snapshot #{}: {e}", ev.cursor);
                }
            }
        } else {
            self.error = true;
            self.status = format!("Unsupported cherry-pick kind '{}'", ev.entity_kind);
        }
    }

    pub fn cherry_pick_request_hunks(
        &mut self,
        ev: &swaggen_api_contract::ChangeEventRecord,
        hunks: &[swaggen_domain::workspace::RequestCherryPickHunk],
    ) {
        if hunks.is_empty() {
            self.error = true;
            self.status = "Select at least one changed section to cherry-pick.".into();
            return;
        }
        let Some(raw) = ev.payload_json.as_deref() else {
            self.error = true;
            self.status = format!("Event #{} has no payload to cherry-pick.", ev.cursor);
            return;
        };
        let source = match serde_json::from_str::<RequestItem>(raw) {
            Ok(req) => req,
            Err(e) => {
                self.error = true;
                self.status = format!("Failed to parse request from #{}: {e}", ev.cursor);
                return;
            }
        };

        let target_idx =
            if let Some(pos) = self.state.requests.iter().position(|r| r.id == source.id) {
                pos
            } else {
                let mut new_req = RequestItem::new(
                    self.state
                        .collections
                        .iter()
                        .find(|c| c.id == source.collection_id)
                        .map(|c| c.id.clone())
                        .or_else(|| self.state.collections.first().map(|c| c.id.clone()))
                        .unwrap_or_else(|| "default".into()),
                    source.name.clone(),
                    swaggen_domain::workspace::HttpMethod::Get,
                    "",
                );
                new_req.id = source.id.clone();
                self.state.requests.push(new_req);
                self.state.requests.len() - 1
            };

        let applied_labels = swaggen_domain::workspace::apply_request_cherry_pick_hunks(
            &mut self.state.requests[target_idx],
            &source,
            hunks,
        );
        let target_req = self.state.requests[target_idx].clone();
        self.open_request_in_workbench(&target_req.id);
        self.mark_dirty();
        self.save_workspace_local_only();

        let summary = applied_labels.join(", ");
        if let Some((profile_id, workspace_id)) = self.active_team_target() {
            self.submit(Command::PublishSequentialEvent {
                profile_id,
                workspace_id,
                entity_id: target_req.id.clone(),
                kind: "request".into(),
                name: format!("{} {}", target_req.method.as_str(), target_req.name),
                payload_json: serde_json::to_string(&target_req).unwrap_or_else(|_| "{}".into()),
                note: format!("Cherry-pick #{} ({summary})", ev.cursor),
            });
        }
        self.error = false;
        self.status = format!(
            "Cherry-picked [{summary}] from #{} into Workbench",
            ev.cursor
        );
    }

    pub fn open_request_in_workbench(&mut self, req_id: &str) {
        self.hide_secret();
        self.show_workspace_modal = false;
        self.show_command_palette = false;
        self.show_production_confirm = false;
        self.state.open_or_focus_tab(req_id);
        if let Some(req) = self.state.requests.iter().find(|r| r.id == req_id) {
            if req.is_graphql() {
                self.request_subtab = RequestSubTab::GraphQl;
            } else if self.request_subtab == RequestSubTab::GraphQl {
                self.request_subtab = RequestSubTab::Params;
            }
        }
        self.mode = MainMode::Workbench;
    }

    pub fn slugify_workspace_id(raw_name: &str) -> Option<LocalId> {
        let mut slug = String::with_capacity(64);
        let mut prev_dash = false;
        for ch in raw_name.trim().chars() {
            let mapped = match ch {
                'a'..='z' | '0'..='9' => Some(ch),
                'A'..='Z' => Some(ch.to_ascii_lowercase()),
                'à' | 'á' | 'ạ' | 'ả' | 'ã' | 'â' | 'ầ' | 'ấ' | 'ậ' | 'ẩ' | 'ẫ' | 'ă' | 'ằ'
                | 'ắ' | 'ặ' | 'ẳ' | 'ẵ' | 'À' | 'Á' | 'Ạ' | 'Ả' | 'Ã' | 'Â' | 'Ầ' | 'Ấ' | 'Ậ'
                | 'Ẩ' | 'Ẫ' | 'Ă' | 'Ằ' | 'Ắ' | 'Ặ' | 'Ẳ' | 'Ẵ' => Some('a'),
                'è' | 'é' | 'ẹ' | 'ẻ' | 'ẽ' | 'ê' | 'ề' | 'ế' | 'ệ' | 'ể' | 'ễ' | 'È' | 'É'
                | 'Ẹ' | 'Ẻ' | 'Ẽ' | 'Ê' | 'Ề' | 'Ế' | 'Ệ' | 'Ể' | 'Ễ' => Some('e'),
                'ì' | 'í' | 'ị' | 'ỉ' | 'ĩ' | 'Ì' | 'Í' | 'Ị' | 'Ỉ' | 'Ĩ' => {
                    Some('i')
                }
                'ò' | 'ó' | 'ọ' | 'ỏ' | 'õ' | 'ô' | 'ồ' | 'ố' | 'ộ' | 'ổ' | 'ỗ' | 'ơ' | 'ờ'
                | 'ớ' | 'ợ' | 'ở' | 'ỡ' | 'Ò' | 'Ó' | 'Ọ' | 'Ỏ' | 'Õ' | 'Ô' | 'Ồ' | 'Ố' | 'Ộ'
                | 'Ổ' | 'Ỗ' | 'Ơ' | 'Ờ' | 'Ớ' | 'Ợ' | 'Ở' | 'Ỡ' => Some('o'),
                'ù' | 'ú' | 'ụ' | 'ủ' | 'ũ' | 'ư' | 'ừ' | 'ứ' | 'ự' | 'ử' | 'ữ' | 'Ù' | 'Ú'
                | 'Ụ' | 'Ủ' | 'Ũ' | 'Ư' | 'Ừ' | 'Ứ' | 'Ự' | 'Ử' | 'Ữ' => Some('u'),
                'ỳ' | 'ý' | 'ỵ' | 'ỷ' | 'ỹ' | 'Ỳ' | 'Ý' | 'Ỵ' | 'Ỷ' | 'Ỹ' => {
                    Some('y')
                }
                'đ' | 'Đ' => Some('d'),
                '-' | '_' | ' ' | '/' | '.' => None,
                _ => None,
            };
            if let Some(c) = mapped {
                slug.push(c);
                prev_dash = false;
            } else if !slug.is_empty() && !prev_dash {
                slug.push('-');
                prev_dash = true;
            }
            if slug.len() >= 56 {
                break;
            }
        }
        let trimmed = slug.trim_matches('-').to_owned();
        if trimmed.is_empty() {
            return None;
        }
        LocalId::parse(trimmed.clone())
            .ok()
            .or_else(|| LocalId::parse(format!("ws-{trimmed}")).ok())
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

    pub fn save_workspace_local_only(&mut self) {
        if self.loaded && !self.pending && !self.recovery_needed {
            self.submit(Command::SaveWorkspace {
                generation: self.generation,
                state: Box::new(self.state.clone()),
            });
        }
    }

    pub fn save_workspace(&mut self) {
        if self.loaded && !self.pending && !self.recovery_needed {
            self.save_workspace_local_only();
            self.publish_current_on_save(None);
        }
    }

    pub fn send_active_request(&mut self, confirmed_production: bool) {
        let Some(req) = self.active_request().cloned() else {
            return;
        };
        // Idempotent UI guard: prevent accidental double-click concurrent dispatches of the same request.
        if self.inflight_request_id.as_deref() == Some(req.id.as_str()) {
            return;
        }
        let is_gql_mutation = match &req.body {
            swaggen_domain::workspace::RequestBody::GraphQl {
                query,
                operation_name,
                ..
            } => {
                let analysis = swaggen_domain::graphql::analyze_graphql_document(
                    query,
                    operation_name.as_deref(),
                    8,
                    120,
                );
                analysis.active_kind == swaggen_domain::graphql::GraphQlOperationKind::Mutation
            }
            _ => false,
        };
        let is_mutating_call = if req.is_graphql() {
            is_gql_mutation
        } else {
            req.method.is_mutating()
        };
        if !confirmed_production
            && self
                .state
                .active_environment()
                .is_some_and(|e| e.is_production)
            && is_mutating_call
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

    pub fn start_firebase_browser_login(
        &mut self,
        profile_id: String,
        bootstrap_token: Option<String>,
    ) {
        match self
            .worker
            .commands
            .try_send(Command::FirebaseBrowserLogin {
                profile_id,
                bootstrap_token,
            }) {
            Ok(()) => {
                self.firebase_auth_pending = true;
                self.error = false;
                self.status = "Waiting for Google sign-in in your browser…".into();
            }
            Err(_) => {
                self.error = true;
                self.status = "Worker busy; please try signing in again.".into();
            }
        }
    }

    pub fn cancel_firebase_browser_login(&mut self) {
        if self.firebase_auth_pending {
            self.firebase_auth_pending = false;
            let _ = self.worker.commands.try_send(Command::CancelFirebaseLogin);
            self.error = false;
            self.status = "Google sign-in cancelled".into();
        }
    }

    fn shutdown_and_close(&mut self, ctx: &egui::Context) {
        self.cancel_inflight();
        if self.firebase_auth_pending {
            self.firebase_auth_pending = false;
            let _ = self.worker.commands.try_send(Command::CancelFirebaseLogin);
        }
        let _ = self.worker.commands.try_send(Command::Shutdown);
        self.close_dialog = false;
        self.close_after_save = false;
        self.allow_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
                    workspace_summaries,
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
                    self.workspace_summaries = workspace_summaries;
                    self.history = history;
                    self.secret_names = secret_names;
                    self.storage_stats = storage_stats;
                    if let Some(active) = server_profiles.active()
                        && !active.session_id.is_empty()
                    {
                        let pid = active.id.clone();
                        self.server_profiles = server_profiles;
                        self.submit(Command::RefreshTeamWorkspaces {
                            profile_id: pid.clone(),
                        });
                        self.submit(Command::RefreshPendingInvitations { profile_id: pid });
                    } else {
                        self.server_profiles = server_profiles;
                    }
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
                    workspace_summaries,
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
                    self.workspace_summaries = workspace_summaries;
                    self.history = history;
                    self.secret_names = secret_names;
                    self.storage_stats = storage_stats;
                    if let Some(active) = server_profiles.active()
                        && !active.session_id.is_empty()
                    {
                        let pid = active.id.clone();
                        self.server_profiles = server_profiles;
                        self.submit(Command::RefreshTeamWorkspaces {
                            profile_id: pid.clone(),
                        });
                        self.submit(Command::RefreshPendingInvitations { profile_id: pid });
                    } else {
                        self.server_profiles = server_profiles;
                    }
                    self.loaded = true;
                    self.recovery_needed = false;
                    self.confirm_quarantine = false;
                    self.status = "Durable data reopened".into();
                }
                Event::ServerProfilesUpdated { store, message } => {
                    self.firebase_auth_pending = false;
                    let prev_uid = self
                        .server_profiles
                        .active()
                        .filter(|p| !p.session_id.is_empty())
                        .map(|p| p.user_id.clone());
                    let next_active = store.active().filter(|p| !p.session_id.is_empty());
                    let signed_in_id = next_active.map(|p| p.id.clone());
                    let next_uid = next_active.map(|p| p.user_id.clone());
                    let account_switched = prev_uid != next_uid;
                    self.server_profiles = store;
                    if signed_in_id.is_none() || account_switched {
                        self.team_workspaces.clear();
                        self.pending_invitations.clear();
                        self.public_invite_links.clear();
                        self.team_member_details = None;
                        self.team_audit_events.clear();
                        self.team_notifications.clear();
                        self.team_unread_notifications = 0;
                        self.publish_history.clear();
                        self.preview_history_cursor = None;
                        self.expanded_history_cursors.clear();
                        self.cherry_pick_selections.clear();
                        self.selected_team_workspace_id = None;
                    }
                    if signed_in_id.is_none()
                        && matches!(
                            self.state.meta.kind,
                            swaggen_domain::workspace::WorkspaceKind::Team { .. }
                        )
                    {
                        self.show_welcome = true;
                    } else if let Some(pid) = signed_in_id {
                        self.submit(Command::RefreshTeamWorkspaces {
                            profile_id: pid.clone(),
                        });
                        self.submit(Command::RefreshPendingInvitations { profile_id: pid });
                    }
                    self.error = false;
                    self.status = message;
                }
                Event::FirebaseLoginCancelled => {
                    self.firebase_auth_pending = false;
                    self.status = "Google sign-in cancelled".into();
                }
                Event::ServerSessionsLoaded(sessions) => {
                    self.server_sessions = sessions;
                    self.status = "Loaded active device sessions".into();
                }
                Event::TeamWorkspacesLoaded(list) => {
                    self.team_workspaces = list;
                    self.status = format!("Loaded {} team workspaces", self.team_workspaces.len());
                }
                Event::TeamWorkspaceDetailsLoaded {
                    members,
                    audit,
                    public_links,
                } => {
                    self.team_member_details = Some(members);
                    self.team_audit_events = audit;
                    self.public_invite_links = public_links;
                    self.status = "Loaded workspace members & audit".into();
                }
                Event::InvitationCreated(inv) => {
                    self.last_created_invite_token = inv.invite_token;
                    self.status = format!(
                        "Sent in-app workspace invitation to {}",
                        inv.invitation.email
                    );
                }
                Event::PendingInvitationsLoaded(invs) => {
                    self.pending_invitations = invs;
                    if !self.pending_invitations.is_empty() {
                        self.status = format!(
                            "You have {} pending in-app workspace invitation(s)",
                            self.pending_invitations.len()
                        );
                    }
                }
                Event::PublicInviteLinkCreated(link) => {
                    self.public_invite_links
                        .retain(|existing| existing.link_id != link.link_id);
                    self.public_invite_links.insert(0, link.clone());
                    self.status =
                        format!("Created public expiring invite link ({})", link.share_link);
                }
                Event::PublicInviteLinksLoaded(links) => {
                    self.public_invite_links = links;
                    self.status = format!(
                        "Loaded {} public invite link(s)",
                        self.public_invite_links.len()
                    );
                }
                Event::InAppInvitationAccepted(joined) => {
                    self.pending_invitations
                        .retain(|i| i.workspace_id != joined.workspace.workspace_id);
                    if !self
                        .team_workspaces
                        .iter()
                        .any(|w| w.workspace_id == joined.workspace.workspace_id)
                    {
                        self.team_workspaces.push(joined.workspace.clone());
                    }
                    self.invite_link_paste_input.clear();
                    self.selected_team_workspace_id = Some(joined.workspace.workspace_id.clone());
                    self.state.meta.name = joined.workspace.name.clone();
                    self.status = format!(
                        "Joined team workspace '{}' as {}",
                        joined.workspace.name,
                        joined.workspace.role.as_str()
                    );
                    if let Some(pid) = self.server_profiles.active().map(|p| p.id.clone()) {
                        self.submit(Command::RefreshTeamWorkspaces {
                            profile_id: pid.clone(),
                        });
                        self.submit(Command::RefreshPendingInvitations {
                            profile_id: pid.clone(),
                        });
                        self.submit(Command::LoadTeamWorkspaceDetails {
                            profile_id: pid,
                            workspace_id: joined.workspace.workspace_id,
                        });
                    }
                }
                Event::WorkspaceMembersUpdated(members) => {
                    self.team_member_details = Some(members);
                    self.status = "Workspace members updated".into();
                }
                Event::SyncStateUpdated { state, message } => {
                    self.sync_state = *state;
                    self.status = message;
                }
                Event::PublishHistoryLoaded {
                    events,
                    state,
                    message,
                } => {
                    if let Some(newest) = events.first().map(|e| e.cursor) {
                        self.expanded_history_cursors.insert(newest);
                    }
                    self.publish_history = events;
                    if let Some(st) = state {
                        self.sync_state = *st;
                    }
                    self.error = false;
                    self.status = message;
                }
                Event::EntityRevisionsLoaded(revs) => {
                    self.team_revisions = revs;
                    self.status =
                        format!("Loaded {} historical revisions", self.team_revisions.len());
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
                        self.transfer_progress =
                            Some((execution_id, phase, bytes_done, total_bytes));
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
                    if let Some(rid) = self.active_request_id()
                        && let Some(req) = self.state.requests.iter_mut().find(|r| r.id == rid)
                        && let swaggen_domain::workspace::AuthConfig::OAuth2 {
                            access_token, ..
                        } = &mut req.auth
                    {
                        *access_token = tok.access_token.clone();
                        self.mark_dirty();
                    }
                    if let Ok(id) = LocalId::parse("oauth_access_token")
                        && let Ok(sec) = Secret::new(tok.access_token.as_bytes().to_vec())
                    {
                        self.submit(Command::PutSecret {
                            name: id,
                            value: sec,
                        });
                    }
                    self.status = format!(
                        "OAuth2 token received ({}; attached & saved locally as 'oauth_access_token')",
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
                        Some(u) => format!(
                            "Local mock server active at {u} ({} log entries)",
                            logs.len()
                        ),
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
                Event::GraphQlSchemaFetched { request_id, result } => {
                    if self.graphql_introspecting_id.as_deref() == Some(request_id.as_str()) {
                        self.graphql_introspecting_id = None;
                    }
                    match result {
                        Ok(schema) => {
                            self.status = format!(
                                "GraphQL schema loaded ({} queries · {} mutations · {} types)",
                                schema.query_fields.len(),
                                schema.mutation_fields.len(),
                                schema.type_fields.len()
                            );
                            self.graphql_schemas.insert(request_id, schema);
                        }
                        Err(err) => {
                            self.error = true;
                            self.status = format!("GraphQL introspection failed: {err}");
                        }
                    }
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
                Event::WebSocketFinished(res) => match res {
                    Ok(reply) => {
                        self.ws_logs.push(format!("← {reply}"));
                        self.status = "WebSocket frame exchanged cleanly".into();
                    }
                    Err(err) => {
                        self.error = true;
                        self.ws_logs.push(format!("✖ {err}"));
                        self.status = format!("WebSocket error: {err}");
                    }
                },
                Event::GrpcUnaryFinished(res) => match res {
                    Ok(out) => {
                        self.grpc_output = out;
                        self.status = "gRPC unary call finished".into();
                    }
                    Err(err) => {
                        self.error = true;
                        self.grpc_output = err.clone();
                        self.status = err;
                    }
                },
                Event::UpdateCheckResult(outcome) => {
                    self.status = if outcome.update_available {
                        format!(
                            "Update available: v{} (current v{})",
                            outcome.latest.version, outcome.current_version
                        )
                    } else {
                        format!("Swaggen v{} is up to date", outcome.current_version)
                    };
                    self.update_check = Some(outcome);
                }
                Event::UpdateInstallLaunched(msg) => {
                    self.status = msg;
                }
                Event::HttpCompleted {
                    execution_id,
                    request_id,
                    snapshot,
                    response,
                    extracted,
                    pre_script_report,
                    post_script_report,
                    mutated_env,
                    mutated_collection_vars,
                    mutated_globals,
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
                    crate::worker::apply_merged_mutations(
                        &mut self.state,
                        &snapshot.collection_id,
                        &mutated_env,
                        &mutated_collection_vars,
                        &mutated_globals,
                    );
                    let script_var_count =
                        mutated_env.len() + mutated_collection_vars.len() + mutated_globals.len();
                    let script_tests_count = pre_script_report
                        .as_ref()
                        .map(|r| r.test_results.len())
                        .unwrap_or(0)
                        + post_script_report
                            .as_ref()
                            .map(|r| r.test_results.len())
                            .unwrap_or(0);
                    if pre_script_report.is_some() || post_script_report.is_some() {
                        self.script_reports
                            .insert(request_id.clone(), (pre_script_report, post_script_report));
                    }
                    let extracted_count = extracted.len() + script_var_count;
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
                        "HTTP {} ({} ms · {} bytes{}{})",
                        response.status_code,
                        response.duration_ms,
                        response.decoded_bytes,
                        if extracted_count > 0 {
                            format!(" · {extracted_count} var(s) updated")
                        } else {
                            String::new()
                        },
                        if script_tests_count > 0 {
                            format!(" · {script_tests_count} script test(s)")
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
                    self.firebase_auth_pending = false;
                    self.close_after_save = false;
                    self.error = true;
                    self.status = error.to_string();
                    self.recovery_needed = !self.loaded
                        || matches!(error, Error::Storage | Error::Recovery | Error::Version);
                }
                Event::ActionError(msg) => {
                    self.firebase_auth_pending = false;
                    self.close_after_save = false;
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

    pub fn active_workspace_display_name(&self) -> String {
        if let Some(tid) = &self.selected_team_workspace_id
            && let Some(tw) = self.team_workspaces.iter().find(|w| w.workspace_id == *tid)
            && !tw.name.trim().is_empty()
        {
            return tw.name.clone();
        }
        if let Some(summary) = self
            .workspace_summaries
            .iter()
            .find(|w| w.id == self.workspace_id)
            && !summary.name.trim().is_empty()
        {
            if summary.id.as_str() == "personal" && summary.name == "personal" {
                return "Personal".into();
            }
            return summary.name.clone();
        }
        let raw = self.state.meta.name.trim();
        if raw.is_empty()
            || (raw == "personal" && self.workspace_id.as_str() == "personal")
            || raw.starts_with("team-")
        {
            if self.workspace_id.as_str() == "personal" {
                "Personal".into()
            } else {
                self.workspace_id.as_str().to_owned()
            }
        } else {
            raw.to_owned()
        }
    }

    pub fn screen_breadcrumb(&self) -> String {
        let leaf = match self.mode {
            MainMode::Workbench => self
                .active_request_id()
                .and_then(|id| self.state.requests.iter().find(|r| r.id == id))
                .map(|r| r.name.clone())
                .unwrap_or_else(|| "API Workbench".into()),
            MainMode::Runner => "Testing & Developer Tools".into(),
            MainMode::Server => "Team Collaboration & Workspaces".into(),
            MainMode::TeamSync => "Publish History".into(),
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
        format!("{} / {leaf}", self.active_workspace_display_name())
    }

    /// Smart multi-format file importer shared by OS Drag-and-Drop and Import/Export Hub (`CSV`, `Postman`, `OpenAPI/Swagger`, `HAR`, `.http`).
    pub fn import_dropped_or_loaded_file(&mut self, name: &str, raw: &str) {
        let low_name = name.to_ascii_lowercase();
        let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name).trim();
        let stem = if stem.is_empty() {
            "Imported APIs"
        } else {
            stem
        };

        // 1. CSV API import -> ready-to-test Collection & Requests in Workbench
        if low_name.ends_with(".csv") {
            match swaggen_spec_engine::importers::import_requests_from_csv(stem, raw) {
                Ok((col, reqs, _rep)) => {
                    let count = reqs.len();
                    let first_id = reqs.first().map(|r| r.id.clone());
                    self.state.collections.push(col);
                    self.state.requests.extend(reqs);
                    if let Some(rid) = first_id {
                        self.open_request_in_workbench(&rid);
                    } else {
                        self.mode = MainMode::Workbench;
                    }
                    self.mark_dirty();
                    self.last_import_summary =
                        format!("Imported {count} API request(s) from '{name}'");
                    self.status = self.last_import_summary.clone();
                }
                Err(e) => {
                    self.error = true;
                    self.status = format!("CSV import failed ({name}): {e}");
                }
            }
            return;
        }

        // 2. `.http` REST Client file
        if low_name.ends_with(".http") || low_name.ends_with(".rest") {
            let cid = self
                .state
                .collections
                .first()
                .map(|c| c.id.clone())
                .unwrap_or_else(|| "default".into());
            if let Ok((reqs, vars, _)) = swaggen_spec_engine::har_http::import_dot_http(raw, &cid)
                && !reqs.is_empty()
            {
                let count = reqs.len();
                let first_id = reqs.first().map(|r| r.id.clone());
                self.state.requests.extend(reqs);
                self.state.workspace_vars.extend(
                    vars.into_iter()
                        .map(|(k, v)| swaggen_domain::workspace::VariableEntry::public(k, v)),
                );
                if let Some(rid) = first_id {
                    self.open_request_in_workbench(&rid);
                }
                self.mark_dirty();
                self.last_import_summary =
                    format!("Imported {count} request(s) from '{name}' into Workbench");
                self.status = self.last_import_summary.clone();
                return;
            }
        }

        let trimmed = raw.trim_start();
        if trimmed.starts_with('{')
            && let Ok(val) = serde_json::from_str::<serde_json::Value>(raw)
        {
            // 3. Postman Environment JSON
            let is_pm_env = val
                .get("_postman_variable_scope")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|s| s == "environment")
                || (val
                    .get("values")
                    .and_then(serde_json::Value::as_array)
                    .is_some()
                    && val.get("item").is_none()
                    && val.get("paths").is_none());
            if is_pm_env
                && let Ok((env, lines)) =
                    swaggen_spec_engine::importers::import_postman_environment(raw)
            {
                let eid = env.id.clone();
                self.last_import_summary = format!(
                    "Imported Environment '{}' ({} vars): {}",
                    env.name,
                    env.variables.len(),
                    lines.join(" · ")
                );
                self.state.environments.push(env);
                self.state.active_environment_id = Some(eid);
                self.mark_dirty();
                self.navigate_to(MainMode::Environments);
                self.status = self.last_import_summary.clone();
                return;
            }

            // 4. Postman Collection v2.1 JSON -> Collection + Folders + Requests for immediate testing
            if val
                .get("item")
                .and_then(serde_json::Value::as_array)
                .is_some()
                && val.get("paths").is_none()
                && let Ok((col, folders, reqs, _rep)) =
                    swaggen_spec_engine::importers::import_postman_collection_v2_1(raw)
            {
                let count = reqs.len();
                let cname = col.name.clone();
                let first_id = reqs.first().map(|r| r.id.clone());
                self.state.collections.push(col);
                self.state.folders.extend(folders);
                self.state.requests.extend(reqs);
                if let Some(rid) = first_id {
                    self.open_request_in_workbench(&rid);
                } else {
                    self.mode = MainMode::Workbench;
                }
                self.mark_dirty();
                self.last_import_summary = format!(
                    "Imported Postman Collection '{cname}' ({count} request(s)) into Workbench"
                );
                self.status = self.last_import_summary.clone();
                return;
            }

            // 5. HAR 1.2 JSON
            if val.pointer("/log/entries").is_some() {
                let col_id = swaggen_domain::workspace::new_uid();
                if let Ok((reqs, _)) = swaggen_spec_engine::har_http::import_har(raw, &col_id)
                    && !reqs.is_empty()
                {
                    let count = reqs.len();
                    let first_id = reqs.first().map(|r| r.id.clone());
                    self.state
                        .collections
                        .push(swaggen_domain::workspace::Collection {
                            id: col_id,
                            name: stem.to_owned(),
                            description: format!("Imported from HAR '{name}'"),
                            color: None,
                            auth: Default::default(),
                            variables: Vec::new(),
                            pre_request_script: String::new(),
                            post_response_script: String::new(),
                            order_key: self.state.collections.len() as u64,
                        });
                    self.state.requests.extend(reqs);
                    if let Some(rid) = first_id {
                        self.open_request_in_workbench(&rid);
                    }
                    self.mark_dirty();
                    self.status = format!("Imported {count} API request(s) from HAR '{name}'");
                    return;
                }
            }
        }

        // 6. OpenAPI / Swagger JSON/YAML/HTML -> add to Docs Studio AND create runnable test requests when dropped in Import/Workbench
        let doc = swaggen_spec_engine::openapi::parse_api_documentation(
            swaggen_domain::workspace::new_uid(),
            name.to_owned(),
            raw,
            &[],
        );
        let ops_count = doc.operations.len();
        if ops_count > 0 && self.mode != MainMode::Specs {
            let col_id = swaggen_domain::workspace::new_uid();
            let col_title = if !doc.title.trim().is_empty() && doc.title != "Untitled" {
                doc.title.clone()
            } else {
                stem.to_owned()
            };
            let mut gen_reqs = Vec::with_capacity(ops_count);
            for op in &doc.operations {
                gen_reqs.push(swaggen_spec_engine::openapi::generate_linked_request(
                    &doc, op, &col_id,
                ));
            }
            let first_id = gen_reqs.first().map(|r| r.id.clone());
            self.state
                .collections
                .push(swaggen_domain::workspace::Collection {
                    id: col_id,
                    name: col_title.clone(),
                    description: format!("Generated from '{name}'"),
                    color: None,
                    auth: Default::default(),
                    variables: Vec::new(),
                    pre_request_script: String::new(),
                    post_response_script: String::new(),
                    order_key: self.state.collections.len() as u64,
                });
            self.state.requests.extend(gen_reqs);
            self.spec_editor_text = doc.raw_text.clone();
            self.specs.push(doc);
            self.selected_spec_idx = self.specs.len() - 1;
            self.submit(Command::SaveSpecs {
                specs: self.specs.clone(),
            });
            if let Some(rid) = first_id {
                self.open_request_in_workbench(&rid);
            }
            self.mark_dirty();
            self.last_import_summary = format!(
                "Imported '{col_title}': {ops_count} API request(s) ready to test + Docs added"
            );
            self.status = self.last_import_summary.clone();
        } else {
            self.spec_editor_text = doc.raw_text.clone();
            self.specs.push(doc);
            self.selected_spec_idx = self.specs.len() - 1;
            self.mode = MainMode::Specs;
            self.submit(Command::SaveSpecs {
                specs: self.specs.clone(),
            });
            self.status = format!("Imported '{name}' ({ops_count} endpoint(s)) into Docs");
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.events();

        if self.close_after_save && !self.dirty() && !self.pending {
            self.shutdown_and_close(&ctx);
            return;
        }

        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            let has_unsaved = self.dirty() || !self.secret_input.is_empty();
            if has_unsaved {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_dialog = true;
            } else {
                self.shutdown_and_close(&ctx);
                return;
            }
        }
        if self.close_dialog {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                self.close_dialog = false;
                self.close_after_save = false;
                return;
            }
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
                            ui.horizontal_wrapped(|ui| {
                                if swaggen_ui_kit::secondary_button(
                                    ui,
                                    true,
                                    "Return to workspace",
                                )
                                .clicked()
                                {
                                    self.close_dialog = false;
                                    self.close_after_save = false;
                                }
                                if self.dirty() && self.loaded && !self.recovery_needed
                                    && swaggen_ui_kit::primary_button(
                                        ui,
                                        !self.pending,
                                        if self.close_after_save {
                                            "Saving…"
                                        } else {
                                            "Save & close"
                                        },
                                    )
                                    .clicked()
                                {
                                    self.close_after_save = true;
                                    self.save_workspace();
                                }
                                if swaggen_ui_kit::danger_button(
                                    ui,
                                    true,
                                    "Close without saving",
                                )
                                .clicked()
                                {
                                    self.shutdown_and_close(&ctx);
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
        // OS file drag-and-drop smart import: CSV, Postman JSON, OpenAPI/Swagger, HAR 1.2, `.http` (UX08).
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for df in dropped {
            let path = df.path();
            if let Ok(raw) = std::fs::read_to_string(path) {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("dropped-file")
                    .to_owned();
                self.import_dropped_or_loaded_file(&name, &raw);
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S)) {
            if self.mode == MainMode::Specs
                && let Some(cur_spec) = self.specs.get(self.selected_spec_idx)
            {
                let updated = swaggen_spec_engine::openapi::parse_api_documentation(
                    cur_spec.id.clone(),
                    cur_spec.name.clone(),
                    &self.spec_editor_text,
                    &[],
                );
                self.specs[self.selected_spec_idx] = updated;
                self.submit(Command::SaveSpecs {
                    specs: self.specs.clone(),
                });
            }
            self.save_workspace();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Enter)) {
            self.send_active_request(false);
        }
        if self.mode == MainMode::Workbench
            && !self.show_command_palette
            && !self.show_workspace_modal
            && !self.show_production_confirm
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
        {
            self.send_active_request(false);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::B)) {
            self.state.layout.sidebar_collapsed = !self.state.layout.sidebar_collapsed;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::P)) {
            self.mode = MainMode::Workbench;
            self.state.layout.sidebar_collapsed = false;
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

        // 1. Compact Statusbar (24px height, outer top 1px divider, explicit Local PC execution notice).
        let status_bar_frame = egui::Frame::new()
            .fill(swaggen_ui_kit::surface_bg(ui))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(14, 4));
        let status_resp = egui::Panel::bottom("status")
            .show_separator_line(false)
            .frame(status_bar_frame)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if self.pending || self.inflight_execution_id.is_some() {
                        ui.spinner();
                    }
                    let left_status = if self.error {
                        self.status.clone()
                    } else if self.selected_team_workspace_id.is_some()
                        && let Some(prof) = self.server_profiles.active()
                    {
                        let host = prof
                            .base_url
                            .strip_prefix("https://")
                            .or_else(|| prof.base_url.strip_prefix("http://"))
                            .unwrap_or(&prof.base_url);
                        format!("{} · Direct from PC (Team sync: {host})", self.status)
                    } else {
                        format!("{} · Direct from this PC", self.status)
                    };
                    if self.error {
                        ui.label(
                            egui::RichText::new(left_status)
                                .size(10.5)
                                .color(Tokens::TEXT_DANGER),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(left_status)
                                .size(10.5)
                                .color(swaggen_ui_kit::text_primary(ui)),
                        );
                    }
                    if self.dirty() {
                        ui.label(
                            egui::RichText::new(" · Unsaved*")
                                .size(10.5)
                                .color(Tokens::TEXT_WARNING),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "Rev {}  ·  Local Rust HTTP Engine  ·  Windows",
                                self.state.revision
                            ))
                            .size(10.5)
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
        swaggen_ui_kit::paint_edge_divider(ui, status_resp.response.rect, Edge::Top);

        // 2. Left 48px Compact Activity Rail (outer right 1px divider — never clipped).
        let rail_w = 48.0;
        let rail_frame = egui::Frame::new()
            .fill(swaggen_ui_kit::surface_bg(ui))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(7, 10));
        let rail_resp = egui::Panel::left("activity_rail")
            .exact_size(rail_w)
            .resizable(false)
            .show_separator_line(false)
            .frame(rail_frame)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    if swaggen_ui_kit::brand_mark(ui, 26.0).clicked() {
                        self.show_welcome = true;
                        self.show_workspace_modal = false;
                        self.show_command_palette = false;
                    }
                    ui.add_space(10.0);
                    for (mode, icon, tip) in [
                        (
                            MainMode::Workbench,
                            IconKind::Blocks,
                            "API Workbench (Ctrl+P)",
                        ),
                        (
                            MainMode::Specs,
                            IconKind::BookOpen,
                            "API Definitions & Docs",
                        ),
                        (
                            MainMode::Runner,
                            IconKind::Play,
                            "Testing, Mocks & Protocols",
                        ),
                        (
                            MainMode::TeamSync,
                            IconKind::GitBranch,
                            "Publish History (Ctrl+S & Cherry-Pick)",
                        ),
                        (
                            MainMode::Server,
                            IconKind::Users,
                            "Team Collaboration (Workspaces & Members)",
                        ),
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
                            MainMode::Privacy,
                            IconKind::Settings,
                            "Settings, Updates & Privacy",
                        ),
                    ] {
                        let active = match mode {
                            MainMode::Privacy => matches!(
                                self.mode,
                                MainMode::Privacy
                                    | MainMode::Storage
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
                            _ => self.mode == mode && !self.show_workspace_modal,
                        };
                        if swaggen_ui_kit::rail_icon_button(ui, active, icon, tip).clicked() {
                            self.navigate_to(mode);
                        }
                        ui.add_space(6.0);
                    }
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                        let initials = self.user_initials();
                        if swaggen_ui_kit::avatar_circle(ui, &initials, AvatarTone::Cream)
                            .on_hover_text("Workspace switcher & account")
                            .clicked()
                        {
                            self.hide_secret();
                            self.show_workspace_modal = false;
                            self.show_command_palette = false;
                            if self
                                .server_profiles
                                .active()
                                .is_none_or(|p| p.session_id.is_empty())
                            {
                                self.show_welcome = true;
                            } else {
                                self.navigate_to(MainMode::Server);
                            }
                        }
                    });
                });
            });
        swaggen_ui_kit::paint_edge_divider(ui, rail_resp.response.rect, Edge::Right);

        // 3. Left Project Sidebar + Razor-Thin 1px Resizer Strip (responsive to half-screen snap).
        let window_total_w = ctx.content_rect().width().max(480.0);
        let responsive_sidebar_max = (window_total_w * 0.38).clamp(192.0, 480.0);
        if !self.state.layout.sidebar_collapsed {
            let cur_sidebar_w = self
                .state
                .layout
                .sidebar_width
                .clamp(186.0, responsive_sidebar_max);
            let sidebar_frame = egui::Frame::new()
                .fill(swaggen_ui_kit::surface_bg(ui))
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(8, 8));
            egui::Panel::left("sidebar")
                .resizable(false)
                .show_separator_line(false)
                .exact_size(cur_sidebar_w)
                .frame(sidebar_frame)
                .show(ui, |ui| {
                    views::sidebar::show(self, ui);
                });

            let resizer_frame = egui::Frame::new()
                .fill(swaggen_ui_kit::border_color(ui))
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::ZERO);
            egui::Panel::left("sidebar_resizer")
                .resizable(false)
                .show_separator_line(false)
                .exact_size(1.0)
                .frame(resizer_frame)
                .show(ui, |ui| {
                    let full_rect = ui.max_rect();
                    let hit_rect = full_rect.expand2(egui::vec2(3.0, 0.0));
                    let id = egui::Id::unique("swaggen_sidebar_edge_drag");
                    let resp = ui.interact(hit_rect, id, egui::Sense::click_and_drag());
                    let active = resp.hovered() || resp.dragged();
                    if active {
                        ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    if resp.dragged()
                        && let Some(pos) = ctx.pointer_interact_pos()
                    {
                        self.state.layout.sidebar_width =
                            (pos.x - rail_w).clamp(186.0, responsive_sidebar_max);
                        ctx.request_repaint();
                    }
                    if resp
                        .on_hover_text("Drag left/right to resize Sidebar · Double-click to reset")
                        .double_clicked()
                    {
                        self.state.layout.sidebar_width = 248.0;
                    }
                    if active {
                        let lx = full_rect.center().x;
                        ui.painter().line_segment(
                            [
                                egui::pos2(lx, full_rect.top()),
                                egui::pos2(lx, full_rect.bottom()),
                            ],
                            egui::Stroke::new(2.0, Tokens::STATUS_INFO),
                        );
                    }
                });
        }

        // 4. Compact Workspace Header (36px height, outer bottom 1px divider).
        let header_frame = egui::Frame::new()
            .fill(swaggen_ui_kit::surface_bg(ui))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(14, 5));
        let signed_in = self
            .server_profiles
            .active()
            .is_some_and(|p| !p.session_id.is_empty());
        let header_resp = egui::Panel::top("workspace_header")
            .show_separator_line(false)
            .frame(header_frame)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let toggle_lbl = if self.state.layout.sidebar_collapsed {
                        "»"
                    } else {
                        "«"
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(toggle_lbl)
                                    .size(12.0)
                                    .color(Tokens::TEXT_SECONDARY)
                                    .strong(),
                            )
                            .fill(Tokens::BG_SURFACE)
                            .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
                            .min_size(egui::vec2(22.0, 22.0))
                            .corner_radius(5),
                        )
                        .on_hover_text(if self.state.layout.sidebar_collapsed {
                            "Expand project sidebar (Ctrl+B)"
                        } else {
                            "Collapse project sidebar (Ctrl+B)"
                        })
                        .clicked()
                    {
                        self.state.layout.sidebar_collapsed = !self.state.layout.sidebar_collapsed;
                    }
                    ui.add_space(2.0);
                    ui.label(swaggen_ui_kit::bold_text(
                        &self.screen_breadcrumb(),
                        12.0,
                        swaggen_ui_kit::text_primary(ui),
                    ));
                    if self.state.meta.archived {
                        swaggen_ui_kit::pill_badge(ui, Tokens::TEXT_WARNING, "Archived");
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.selected_team_workspace_id.is_some()
                            && swaggen_ui_kit::secondary_button(
                                ui,
                                !self.pending,
                                "Publish changes",
                            )
                            .clicked()
                        {
                            self.sync_subtab = 0;
                            self.navigate_to(MainMode::TeamSync);
                        }
                        if signed_in {
                            let initials = self.user_initials();
                            if swaggen_ui_kit::avatar_circle(ui, &initials, AvatarTone::Salmon)
                                .on_hover_text("Account & sessions")
                                .clicked()
                            {
                                self.server_subtab = 3;
                                self.navigate_to(MainMode::Server);
                            }
                        } else if swaggen_ui_kit::primary_button(ui, true, "Sign in").clicked() {
                            self.show_welcome = true;
                        }
                        if let Some(ref uc) = self.update_check
                            && uc.update_available
                            && swaggen_ui_kit::primary_button(
                                ui,
                                !self.pending,
                                &format!("Update v{}", uc.latest.version),
                            )
                            .clicked()
                        {
                            self.navigate_to(MainMode::Privacy);
                        }
                        if self.dirty()
                            && self.mode != MainMode::Workbench
                            && swaggen_ui_kit::secondary_button(
                                ui,
                                self.loaded && !self.pending && !self.recovery_needed,
                                "Save*",
                            )
                            .clicked()
                        {
                            self.save_workspace();
                        }
                        let (sync_color, sync_label) = match self.server_profiles.active() {
                            _ if self.selected_team_workspace_id.is_none() => {
                                (Tokens::STATUS_SUCCESS, "Local only")
                            }
                            Some(p) if !p.session_id.is_empty() && p.mongo_ready => {
                                (Tokens::STATUS_SUCCESS, "Synced")
                            }
                            Some(p) if p.mongo_ready => (Tokens::STATUS_SUCCESS, "Server OK"),
                            Some(_) => (Tokens::STATUS_INFO, "Configured"),
                            None => (Tokens::TEXT_SECONDARY, "Local"),
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(sync_label)
                                        .size(11.5)
                                        .color(sync_color)
                                        .strong(),
                                )
                                .fill(swaggen_ui_kit::surface_bg(ui))
                                .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                                .corner_radius(6),
                            )
                            .on_hover_text(if self.selected_team_workspace_id.is_some() {
                                "Open team sync & collaboration"
                            } else {
                                "Switch workspace or collaboration mode"
                            })
                            .clicked()
                        {
                            if self.selected_team_workspace_id.is_some() {
                                self.navigate_to(MainMode::TeamSync);
                            } else {
                                self.show_workspace_modal = true;
                            }
                        }
                        let active_env = self.state.active_environment();
                        let env_color = match active_env.map(|e| e.effective_tier()) {
                            Some(swaggen_domain::workspace::EnvTier::Production) => {
                                Tokens::TEXT_DANGER
                            }
                            Some(swaggen_domain::workspace::EnvTier::Staging) => {
                                Tokens::TEXT_WARNING
                            }
                            Some(
                                swaggen_domain::workspace::EnvTier::Development
                                | swaggen_domain::workspace::EnvTier::Local,
                            ) => Tokens::STATUS_SUCCESS,
                            Some(swaggen_domain::workspace::EnvTier::Custom) => Tokens::STATUS_INFO,
                            None => swaggen_ui_kit::text_secondary(ui),
                        };
                        let env_btn_label = active_env
                            .map(|e| format!("● {} ▾", e.name))
                            .unwrap_or_else(|| "○ No Environment ▾".into());
                        let env_btn = ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(env_btn_label)
                                        .size(11.5)
                                        .color(env_color)
                                        .strong(),
                                )
                                .fill(swaggen_ui_kit::surface_bg(ui))
                                .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                                .corner_radius(6),
                            )
                            .on_hover_text(
                                "Active Environment for {{variables}} (click to switch)",
                            );
                        egui::Popup::menu(&env_btn)
                            .id(egui::Id::unique("swaggen_top_env_picker"))
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .show(|ui| {
                                ui.set_min_width(210.0);
                                ui.label(
                                    egui::RichText::new("ACTIVE ENVIRONMENT")
                                        .size(10.0)
                                        .color(swaggen_ui_kit::text_secondary(ui)),
                                );
                                if ui
                                    .selectable_label(
                                        self.state.active_environment_id.is_none(),
                                        "No Environment",
                                    )
                                    .clicked()
                                {
                                    self.state.active_environment_id = None;
                                    self.mark_dirty();
                                    ui.close();
                                }
                                let env_items: Vec<(String, String, bool, usize)> = self
                                    .state
                                    .environments
                                    .iter()
                                    .map(|e| {
                                        (
                                            e.id.clone(),
                                            e.name.clone(),
                                            e.is_production,
                                            e.variables.len(),
                                        )
                                    })
                                    .collect();
                                for (idx, (eid, ename, is_prod, vcount)) in
                                    env_items.into_iter().enumerate()
                                {
                                    let is_act =
                                        self.state.active_environment_id.as_deref() == Some(&eid);
                                    let badge = if is_prod { " [PROD]" } else { "" };
                                    if ui
                                        .selectable_label(
                                            is_act,
                                            format!("{ename}{badge}  ({vcount} vars)"),
                                        )
                                        .clicked()
                                    {
                                        self.state.active_environment_id = Some(eid);
                                        self.selected_env_idx = idx;
                                        self.mark_dirty();
                                        ui.close();
                                    }
                                }
                                ui.separator();
                                if ui.button("⚙ Manage Environments & Variables…").clicked() {
                                    self.navigate_to(MainMode::Environments);
                                    ui.close();
                                }
                            });
                        let show_cmd_label = if window_total_w < 820.0 {
                            "Ctrl+K"
                        } else {
                            "Search / Commands  Ctrl+K"
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(show_cmd_label)
                                        .size(11.0)
                                        .color(swaggen_ui_kit::text_secondary(ui)),
                                )
                                .fill(swaggen_ui_kit::subtle_bg(ui))
                                .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                                .corner_radius(6),
                            )
                            .on_hover_text("Open Quick Command Palette (Ctrl+K)")
                            .clicked()
                        {
                            self.show_command_palette = true;
                            self.command_palette_query.clear();
                        }
                    });
                });
            });
        swaggen_ui_kit::paint_edge_divider(ui, header_resp.response.rect, Edge::Bottom);

        let main_frame = egui::Frame::new()
            .fill(swaggen_ui_kit::page_bg(ui))
            .inner_margin(egui::Margin::symmetric(14, 10));

        // 5. Main Central Workspace Canvas with 1:1 Pointer-Locked Request <-> Response Splitters.
        egui::CentralPanel::default()
            .frame(main_frame)
            .show(ui, |ui| match self.mode {
                MainMode::Workbench => {
                    let Some(req_idx) = views::request_pane::show_top_bar(self, ui) else {
                        return;
                    };
                    if self.response_maximized {
                        views::response_pane::show(self, ui, &ctx);
                    } else if self.state.layout.workbench_side_by_side {
                        let split_rect = ui.available_rect_before_wrap();
                        let total_w = split_rect.width().max(420.0);
                        let total_h = split_rect.height().max(150.0);
                        let ratio = self.state.layout.response_split_ratio.clamp(0.20, 0.80);
                        let req_w = ((total_w - 10.0) * (1.0 - ratio))
                            .clamp(210.0, (total_w - 220.0).max(210.0));
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            ui.allocate_ui_with_layout(
                                egui::vec2(req_w, total_h),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_min_width(req_w);
                                    ui.set_max_width(req_w);
                                    ui.set_min_height(total_h);
                                    ui.set_max_height(total_h);
                                    views::request_pane::show_config_panel(
                                        self, ui, req_idx, total_h,
                                    );
                                },
                            );
                            let (split_resp, _) =
                                swaggen_ui_kit::vertical_splitter(ui, "wb_main_vsplit", total_h);
                            if split_resp.dragged()
                                && let Some(pos) = ctx.pointer_interact_pos()
                            {
                                let left_frac =
                                    ((pos.x - split_rect.left()) / total_w).clamp(0.20, 0.80);
                                self.state.layout.response_split_ratio = 1.0 - left_frac;
                            }
                            if split_resp
                                .on_hover_text("Drag left/right to resize · Double-click to reset")
                                .double_clicked()
                            {
                                self.state.layout.response_split_ratio = 0.50;
                            }
                            let resp_w = ui.available_width().max(180.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(resp_w, total_h),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_min_width(resp_w);
                                    ui.set_max_width(resp_w);
                                    views::response_pane::show(self, ui, &ctx);
                                },
                            );
                        });
                    } else if self.request_config_collapsed {
                        views::request_pane::show_config_panel(self, ui, req_idx, 0.0);
                        ui.add_space(4.0);
                        views::response_pane::show(self, ui, &ctx);
                    } else {
                        let split_rect = ui.available_rect_before_wrap();
                        let total_h = split_rect.height().max(210.0);
                        let total_w = split_rect.width().max(320.0);
                        let ratio = self.state.layout.response_split_ratio.clamp(0.14, 0.86);
                        let req_h = ((total_h - 10.0) * (1.0 - ratio))
                            .clamp(84.0, (total_h - 104.0).max(84.0));

                        ui.allocate_ui_with_layout(
                            egui::vec2(total_w, req_h),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_min_height(req_h);
                                ui.set_max_height(req_h);
                                views::request_pane::show_config_panel(self, ui, req_idx, req_h);
                            },
                        );

                        let (split_resp, _) =
                            swaggen_ui_kit::horizontal_splitter(ui, "wb_main_hsplit");
                        if split_resp.dragged()
                            && let Some(pos) = ctx.pointer_interact_pos()
                        {
                            let top_frac =
                                ((pos.y - split_rect.top()) / total_h).clamp(0.14, 0.86);
                            self.state.layout.response_split_ratio =
                                (1.0 - top_frac).clamp(0.14, 0.86);
                        }
                        if split_resp
                            .on_hover_text(
                                "Drag up/down to resize Request vs Response · Double-click to reset",
                            )
                            .double_clicked()
                        {
                            self.state.layout.response_split_ratio = 0.58;
                        }
                        views::response_pane::show(self, ui, &ctx);
                    }
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
                MainMode::Shortcuts => views::panels::show_shortcuts(self, ui),
            });

        // 6. Context-Preserving Floating Modal Overlays (backdrop + centered card, Esc/click-outside dismiss).
        if self.show_production_confirm || self.show_command_palette || self.show_workspace_modal {
            let screen = ctx.content_rect();
            let backdrop_resp = egui::Area::new(egui::Id::unique("swaggen_modal_backdrop"))
                .order(egui::Order::Middle)
                .fixed_pos(screen.min)
                .show(&ctx, |ui| {
                    let (_, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
                    ui.painter().rect_filled(
                        screen,
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(15, 23, 42, 72),
                    );
                    resp
                });
            if backdrop_resp.inner.clicked() {
                self.show_production_confirm = false;
                self.show_command_palette = false;
                self.show_workspace_modal = false;
            }
            let modal_max_w = if self.show_workspace_modal {
                (screen.width() - 80.0).clamp(480.0, 920.0)
            } else {
                (screen.width() - 60.0).clamp(400.0, 580.0)
            };
            let modal_max_h = (screen.height() - 72.0).max(260.0);
            egui::Area::new(egui::Id::unique("swaggen_modal_card"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(&ctx, |ui| {
                    ui.set_max_width(modal_max_w);
                    ui.set_max_height(modal_max_h);
                    egui::Frame::new()
                        .fill(Tokens::BG_SURFACE)
                        .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
                        .corner_radius(12)
                        .inner_margin(egui::Margin::same(18))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 8],
                            blur: 24,
                            spread: 0,
                            color: egui::Color32::from_rgba_unmultiplied(15, 23, 42, 36),
                        })
                        .show(ui, |ui| {
                            if self.show_production_confirm {
                                views::panels::show_production_confirm(self, ui);
                            } else if self.show_command_palette {
                                views::panels::show_command_palette(self, ui);
                            } else if self.show_workspace_modal {
                                views::panels::show_workspace_manager(self, ui);
                            }
                        });
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_workspace_id_normalizes_vietnamese_and_punctuation() {
        assert_eq!(
            App::slugify_workspace_id("Dự án API cá nhân")
                .unwrap()
                .as_str(),
            "du-an-api-ca-nhan"
        );
        assert_eq!(
            App::slugify_workspace_id("  Team Backend / v2  ")
                .unwrap()
                .as_str(),
            "team-backend-v2"
        );
        assert!(App::slugify_workspace_id("   ").is_none());
    }

    #[test]
    fn navigation_helpers_clear_modals_and_preserve_clean_dirty_state() {
        let root = std::env::temp_dir().join(format!(
            "swaggen-nav-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let worker = Worker::start(root.clone(), LocalId::parse("personal").unwrap(), || {});
        let ws = match worker
            .events
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
        {
            Event::Loaded { workspace, .. } => workspace,
            _ => panic!("expected Loaded"),
        };
        let req_id = ws.requests[0].id.clone();
        let mut app = App {
            worker,
            root: root.clone(),
            workspace_id: LocalId::parse("personal").unwrap(),
            available_workspaces: vec![LocalId::parse("personal").unwrap()],
            workspace_summaries: Vec::new(),
            state: ws,
            legacy_snapshot: Snapshot {
                version: 1,
                workspace: LocalId::parse("personal").unwrap(),
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
            loaded: true,
            pending: false,
            generation: 0,
            saved_generation: 0,
            status: String::new(),
            error: false,
            recovery_needed: false,
            confirm_quarantine: false,
            close_dialog: false,
            allow_close: false,
            close_after_save: false,
            firebase_auth_pending: false,
            request_config_collapsed: false,
            response_maximized: false,
            dragging_request_id: None,
            search_query: String::new(),
            filter_method: None,
            filter_tag: String::new(),
            focus_search: false,
            show_sidebar_filters: false,
            show_new_collection_input: false,
            new_collection_name: String::new(),
            new_workspace_id: String::new(),
            new_workspace_name: String::new(),
            show_workspace_modal: true,
            show_production_confirm: true,
            move_target_collection: String::new(),
            move_target_folder: String::new(),
            next_execution_id: 1,
            inflight_execution_id: None,
            inflight_request_id: None,
            responses: HashMap::new(),
            response_search: String::new(),
            save_response_path: String::new(),
            example_name_input: String::new(),
            example_extra_fields: String::new(),
            selected_env_idx: 0,
            new_env_name: String::new(),
            new_env_prod: false,
            inspect_var_key: String::new(),
            cookie_domain: String::new(),
            cookie_path: String::new(),
            cookie_name: String::new(),
            cookie_value: String::new(),
            history_filter: String::new(),
            export_workspace_path: String::new(),
            migrate_root_path: String::new(),
            secret_name: String::new(),
            secret_input: String::new(),
            revealed: None,
            sanitizer_input: String::new(),
            sanitizer_extra_fields: String::new(),
            untrusted_input: String::new(),
            last_diag_export: None,
            collapsed_json_paths: Default::default(),
            specs: Vec::new(),
            selected_spec_idx: 0,
            spec_editor_text: String::new(),
            spec_import_url: String::new(),
            spec_find: String::new(),
            spec_replace: String::new(),
            spec_doc_search: String::new(),
            spec_new_method: HttpMethod::Get,
            spec_new_path: String::new(),
            spec_new_op_id: String::new(),
            spec_diffs: Vec::new(),
            curl_input: String::new(),
            postman_input: String::new(),
            last_import_summary: String::new(),
            show_welcome: false,
            firebase_create_account: false,
            server_subtab: 0,
            server_profiles: Default::default(),
            server_name_input: String::new(),
            server_url_input: String::new(),
            server_allow_self_signed: false,
            auth_email: String::new(),
            auth_display_name: String::new(),
            auth_password: String::new(),
            auth_bootstrap_token: String::new(),
            invite_token_input: String::new(),
            pending_invitations: Vec::new(),
            team_workspaces: Vec::new(),
            selected_team_workspace_id: None,
            team_ws_slug: String::new(),
            team_ws_name: String::new(),
            team_member_details: None,
            team_audit_events: Vec::new(),
            invite_email_input: String::new(),
            invite_role: swaggen_api_contract::Role::Editor,
            last_created_invite_token: String::new(),
            public_invite_links: Vec::new(),
            invite_link_role: swaggen_api_contract::Role::Editor,
            invite_link_expiry_hours: 72,
            invite_link_max_uses_input: String::new(),
            invite_link_paste_input: String::new(),
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
            publish_history: Vec::new(),
            preview_history_cursor: None,
            expanded_history_cursors: std::collections::BTreeSet::new(),
            cherry_pick_selections: HashMap::new(),
            transfer_progress: None,
            previous_responses: HashMap::new(),
            diff_ignore_paths: String::new(),
            oauth_auth_url: String::new(),
            oauth_token_url: String::new(),
            oauth_client_id: String::new(),
            oauth_scope: String::new(),
            oauth_authorize_url: String::new(),
            spec_bundle_dir: String::new(),
            spec_lint_issues: Vec::new(),
            spec_contract_report: None,
            spec_drift_items: Vec::new(),
            spec_schema_tree: None,
            git_export_dir: String::new(),
            attach_files_csv: String::new(),
            attach_shared_dir: String::new(),
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
            gql_endpoint: String::new(),
            gql_query: String::new(),
            gql_vars: String::new(),
            gql_result: None,
            graphql_schemas: HashMap::new(),
            graphql_introspecting_id: None,
            gql_split_ratio: 0.60,
            sse_url: String::new(),
            sse_max_events: 200,
            sse_running: false,
            sse_events: Vec::new(),
            dev_util_input: String::new(),
            dev_util_output: String::new(),
            show_command_palette: true,
            command_palette_query: String::new(),
            locale: swaggen_ui_kit::UiLocale::En,
            deep_link_input: String::new(),
            update_feed_url: String::new(),
            update_check: None,
            local_update_path: String::new(),
            local_update_sha256: String::new(),
            update_silent_install: true,
            update_manifest_json: String::new(),
            update_signing_key: String::new(),
            update_payload_text: String::new(),
            sandbox_script_text: String::new(),
            sandbox_report: None,
            extensions: Vec::new(),
            script_subtab_post: true,
            script_reports: HashMap::new(),
            env_scope_tab: 0,
            selected_collection_var_idx: 0,
            quick_var_name: String::new(),
            quick_var_value: String::new(),
            spec_view_mode: 0,
            spec_local_file_path: String::new(),
            runner_test_kind: 0,
            runner_selected_requests: Default::default(),
            runner_concurrency: 1,
            runner_selected_step_idx: 0,
            runner_trace_tab: 0,
            mock_port_input: "0".into(),
            mock_scenarios: Vec::new(),
            protocol_subtab: 0,
            ws_url_input: String::new(),
            ws_message_input: String::new(),
            ws_logs: Vec::new(),
            grpc_endpoint_input: String::new(),
            grpc_method_input: String::new(),
            grpc_payload_input: String::new(),
            grpc_output: String::new(),
            import_hub_tab: 0,
            import_source_kind: 0,
            import_file_path: String::new(),
            show_sidebar_trash: false,
        };

        app.navigate_to(MainMode::Environments);
        assert_eq!(app.mode, MainMode::Environments);
        assert!(!app.show_workspace_modal);
        assert!(!app.show_command_palette);
        assert!(!app.show_production_confirm);

        app.show_workspace_modal = true;
        app.open_request_in_workbench(&req_id);
        assert_eq!(app.mode, MainMode::Workbench);
        assert!(!app.show_workspace_modal);
        assert!(
            !app.dirty(),
            "opening a request must not mark workspace dirty"
        );

        let mut old_req = app.state.requests[0].clone();
        old_req.url = "https://cherry-picked.example.test/v1/items".into();
        let ev = swaggen_api_contract::ChangeEventRecord {
            cursor: 7,
            workspace_id: "ws-team-1".into(),
            entity_id: old_req.id.clone(),
            entity_kind: "request".into(),
            entity_name: format!("GET {}", old_req.name),
            parent_id: None,
            order_key: 0,
            revision: 3,
            deleted: false,
            payload_json: Some(serde_json::to_string(&old_req).unwrap()),
            operation_id: "op-7".into(),
            actor_user_id: "u-1".into(),
            actor_email: "alice@swaggen.local".into(),
            note: "Ctrl+S".into(),
            created_at_unix: 1_700_000_000,
        };
        app.cherry_pick_publish_event(&ev);
        assert_eq!(
            app.state.requests[0].url,
            "https://cherry-picked.example.test/v1/items"
        );
        assert_eq!(app.mode, MainMode::Workbench);

        drop(app);
        let _ = std::fs::remove_dir_all(root);
    }
}
