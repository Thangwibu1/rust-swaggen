//! Core personal/team workspace domain model, variables, auth inheritance,
//! cookie jar, resolved request preview, history, examples, and tabs (P02).
use crate::LocalId;
use crate::security::{RedactionPolicy, sanitize_headers, sanitize_text, sanitize_url_query};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub const PREVIEW_CAP_BYTES: usize = 1024 * 1024;
pub const DEFAULT_HISTORY_RETENTION: usize = 500;

pub fn new_uid() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    pub const ALL: [Self; 7] = [
        Self::Get,
        Self::Post,
        Self::Put,
        Self::Patch,
        Self::Delete,
        Self::Head,
        Self::Options,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }

    pub fn is_mutating(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch | Self::Delete)
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "GET" => Some(Self::Get),
            "POST" => Some(Self::Post),
            "PUT" => Some(Self::Put),
            "PATCH" => Some(Self::Patch),
            "DELETE" => Some(Self::Delete),
            "HEAD" => Some(Self::Head),
            "OPTIONS" => Some(Self::Options),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceKind {
    Personal,
    Team {
        server_profile_id: Option<String>,
        remote_workspace_id: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectDefaults {
    pub default_base_url: String,
    pub default_environment_id: Option<String>,
    pub naming_convention: String,
    pub onboarding_guide: String,
}

impl Default for ProjectDefaults {
    fn default() -> Self {
        Self {
            default_base_url: "https://api.example.test".into(),
            default_environment_id: None,
            naming_convention: "Verb + noun (e.g., List tasks, Create task)".into(),
            onboarding_guide:
                "Select an environment, set local secrets in Personal secrets, and send.".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    pub id: LocalId,
    pub name: String,
    pub description: String,
    pub kind: WorkspaceKind,
    pub archived: bool,
    pub defaults: ProjectDefaults,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyValueRow {
    pub key: String,
    pub value: String,
    pub enabled: bool,
    pub description: String,
}

impl KeyValueRow {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            enabled: true,
            description: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultipartValueKind {
    Text(String),
    File {
        /// Local filesystem path; never exported or synced to teammates (RQ05).
        local_path: String,
        file_name: String,
        content_type: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultipartField {
    pub name: String,
    pub value: MultipartValueKind,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RequestBody {
    #[default]
    None,
    Json(String),
    GraphQl {
        query: String,
        variables: String,
        #[serde(default)]
        operation_name: Option<String>,
    },
    Text(String),
    Xml(String),
    FormUrlEncoded(Vec<KeyValueRow>),
    Multipart(Vec<MultipartField>),
    Binary {
        local_path: Option<String>,
        content_type: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum IdempotencyMode {
    /// Automatically attaches a deterministic `Idempotency-Key` on `POST`/`PATCH` and GraphQL mutations.
    #[default]
    AutoOnMutating,
    /// Always attaches `Idempotency-Key` regardless of HTTP method.
    Always,
    /// Does not auto-generate `Idempotency-Key`.
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiKeyPlacement {
    Header,
    Query,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OAuthGrantType {
    #[default]
    AuthorizationCodePkce,
    ClientCredentials,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AuthConfig {
    #[default]
    Inherit,
    NoAuth,
    Bearer {
        token: String,
    },
    Basic {
        username: String,
        password: String,
    },
    ApiKey {
        key: String,
        value: String,
        placement: ApiKeyPlacement,
    },
    OAuth2 {
        #[serde(default)]
        grant_type: OAuthGrantType,
        auth_url: String,
        token_url: String,
        client_id: String,
        #[serde(default)]
        client_secret: String,
        #[serde(default)]
        scope: String,
        #[serde(default)]
        access_token: String,
    },
    AwsSigV4 {
        access_key: String,
        secret_key: String,
        region: String,
        service: String,
        #[serde(default)]
        session_token: String,
    },
    Digest {
        username: String,
        password: String,
        realm: String,
        nonce: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HttpVersionPolicy {
    #[default]
    Auto,
    Http1Only,
    Http2PriorKnowledge,
}

impl HttpVersionPolicy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto (HTTP/1.1 + TLS ALPN)",
            Self::Http1Only => "HTTP/1.1 only",
            Self::Http2PriorKnowledge => "HTTP/2 (ALPN h2 / h2c)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveAuth {
    pub config: AuthConfig,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProxyPolicy {
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
    pub bypass_hosts: Vec<String>,
    #[serde(default)]
    pub proxy_auth_secret_ref: Option<LocalId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientCertMapping {
    pub host: String,
    pub port: u16,
    pub cert_pem_path: String,
    pub key_pem_path: String,
    pub expires_at_unix: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestConnectionPolicy {
    pub timeout_ms: u64,
    #[serde(default)]
    pub http_version: HttpVersionPolicy,
    pub follow_redirects: bool,
    pub max_redirects: usize,
    pub send_auth_on_cross_host_redirect: bool,
    pub tls_host_exceptions: Vec<String>,
    #[serde(default)]
    pub proxy: ProxyPolicy,
    #[serde(default)]
    pub custom_ca_pem_paths: Vec<String>,
    #[serde(default)]
    pub client_certs: Vec<ClientCertMapping>,
}

impl Default for RequestConnectionPolicy {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            http_version: HttpVersionPolicy::Auto,
            follow_redirects: true,
            max_redirects: 10,
            send_auth_on_cross_host_redirect: false,
            tls_host_exceptions: Vec::new(),
            proxy: ProxyPolicy::default(),
            custom_ca_pem_paths: Vec::new(),
            client_certs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertionRule {
    pub id: String,
    pub label: String,
    pub target: AssertionTarget,
    pub expected: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssertionTarget {
    StatusEquals,
    MaxDurationMs,
    HeaderEquals { name: String },
    JsonPathEquals { path: String },
    BodyContains,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestItem {
    pub id: String,
    pub collection_id: String,
    pub folder_id: Option<String>,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub method: HttpMethod,
    pub url: String,
    pub path_params: Vec<KeyValueRow>,
    pub query_params: Vec<KeyValueRow>,
    pub headers: Vec<KeyValueRow>,
    pub body: RequestBody,
    pub auth: AuthConfig,
    pub connection: RequestConnectionPolicy,
    pub local_vars: Vec<VariableEntry>,
    pub assertions: Vec<AssertionRule>,
    #[serde(default)]
    pub extractors: Vec<crate::dynamic_vars::ResponseExtractorRule>,
    #[serde(default)]
    pub pre_request_script: String,
    #[serde(default)]
    pub post_response_script: String,
    #[serde(default)]
    pub idempotency_mode: IdempotencyMode,
    #[serde(default)]
    pub custom_idempotency_key: String,
    #[serde(default)]
    pub idempotency_nonce: u64,
    pub linked_operation_uid: Option<String>,
    pub shared_revision: u64,
    pub dirty_vs_shared: bool,
    pub order_key: u64,
}

impl RequestItem {
    pub fn new(
        collection_id: impl Into<String>,
        name: impl Into<String>,
        method: HttpMethod,
        url: impl Into<String>,
    ) -> Self {
        let mut item = Self {
            id: new_uid(),
            collection_id: collection_id.into(),
            folder_id: None,
            name: name.into(),
            description: String::new(),
            tags: Vec::new(),
            method,
            url: url.into(),
            path_params: Vec::new(),
            query_params: Vec::new(),
            headers: Vec::new(),
            body: RequestBody::None,
            auth: AuthConfig::Inherit,
            connection: RequestConnectionPolicy::default(),
            local_vars: Vec::new(),
            assertions: Vec::new(),
            extractors: Vec::new(),
            pre_request_script: String::new(),
            post_response_script: String::new(),
            idempotency_mode: IdempotencyMode::AutoOnMutating,
            custom_idempotency_key: String::new(),
            idempotency_nonce: 0,
            linked_operation_uid: None,
            shared_revision: 0,
            dirty_vs_shared: false,
            order_key: 0,
        };
        item.sync_params_from_url();
        item
    }

    pub fn new_graphql(
        collection_id: impl Into<String>,
        name: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        let mut item = Self::new(collection_id, name, HttpMethod::Post, url);
        item.body = RequestBody::GraphQl {
            query: "query ExampleQuery {\n  __typename\n}".into(),
            variables: "{\n  \n}".into(),
            operation_name: None,
        };
        item
    }

    pub fn is_graphql(&self) -> bool {
        matches!(self.body, RequestBody::GraphQl { .. })
    }

    /// Parses the current `url` and synchronizes `query_params` and `path_params` live,
    /// preserving disabled query rows and existing path parameter values/descriptions.
    pub fn sync_params_from_url(&mut self) {
        // 1. Sync query parameters from URL `?query` string.
        let parsed_query = extract_url_query_rows(&self.url);
        let mut old_enabled: Vec<KeyValueRow> = self
            .query_params
            .iter()
            .filter(|r| r.enabled)
            .cloned()
            .collect();
        let disabled_rows: Vec<KeyValueRow> = self
            .query_params
            .iter()
            .filter(|r| !r.enabled)
            .cloned()
            .collect();

        let mut next_query = Vec::with_capacity(parsed_query.len() + disabled_rows.len());
        for mut row in parsed_query {
            if let Some(pos) = old_enabled.iter().position(|old| old.key == row.key) {
                let prev = old_enabled.remove(pos);
                row.description = prev.description;
            }
            next_query.push(row);
        }
        next_query.extend(disabled_rows);
        self.query_params = next_query;

        // 2. Sync path parameters (`:param` and `{param}`, excluding `{{env_var}}` and `:port`).
        let param_names = extract_url_path_param_names(&self.url);
        let old_paths = std::mem::take(&mut self.path_params);
        let same_arity = old_paths.len() == param_names.len();
        let mut used_old = vec![false; old_paths.len()];
        let mut next_paths = Vec::with_capacity(param_names.len());

        for (idx, name) in param_names.iter().enumerate() {
            if let Some(old_idx) = old_paths
                .iter()
                .enumerate()
                .position(|(i, r)| !used_old[i] && r.key == *name)
            {
                used_old[old_idx] = true;
                next_paths.push(old_paths[old_idx].clone());
            } else if same_arity && !used_old[idx] && !param_names.contains(&old_paths[idx].key) {
                // User is actively renaming `:id` -> `:userId` in-place in the URL bar: keep value!
                used_old[idx] = true;
                let mut preserved = old_paths[idx].clone();
                preserved.key = name.clone();
                next_paths.push(preserved);
            } else {
                next_paths.push(KeyValueRow::new(name.clone(), ""));
            }
        }
        self.path_params = next_paths;
    }

    /// Rebuilds the `?query` portion of `url` from `query_params` while keeping `{{vars}}`,
    /// `:path_params`, `{path_params}`, and `#fragment` intact.
    pub fn sync_url_from_query_params(&mut self) {
        let (base, _old_query, fragment) = split_url_parts(&self.url);
        let base_owned = base.to_owned();
        let frag_owned = fragment.map(ToOwned::to_owned);

        let mut encoded_pairs = Vec::new();
        for row in &self.query_params {
            if !row.enabled || (row.key.is_empty() && row.value.is_empty()) {
                continue;
            }
            let ek = encode_query_component_preserving_templates(&row.key);
            let ev = encode_query_component_preserving_templates(&row.value);
            encoded_pairs.push(format!("{ek}={ev}"));
        }

        let mut next_url = base_owned;
        if !encoded_pairs.is_empty() {
            next_url.push('?');
            next_url.push_str(&encoded_pairs.join("&"));
        }
        if let Some(frag) = frag_owned {
            next_url.push('#');
            next_url.push_str(&frag);
        }
        self.url = next_url;
    }

    /// Hydrates `query_params` / `path_params` for legacy saved requests whose `url`
    /// already contains inline `?query` or `:param`/`{param}` tokens not yet in the tables.
    pub fn reconcile_missing_params_from_url(&mut self) -> bool {
        let mut hydrated = false;
        if self.query_params.is_empty() {
            let parsed_q = extract_url_query_rows(&self.url);
            if !parsed_q.is_empty() {
                self.query_params = parsed_q;
                hydrated = true;
            }
        }
        let detected_paths = extract_url_path_param_names(&self.url);
        for name in detected_paths {
            if !self.path_params.iter().any(|p| p.key == name) {
                self.path_params.push(KeyValueRow::new(name, ""));
                hydrated = true;
            }
        }
        hydrated
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub collection_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub description: String,
    pub auth: AuthConfig,
    #[serde(default)]
    pub pre_request_script: String,
    #[serde(default)]
    pub post_response_script: String,
    pub order_key: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub color: Option<String>,
    pub auth: AuthConfig,
    pub variables: Vec<VariableEntry>,
    #[serde(default)]
    pub pre_request_script: String,
    #[serde(default)]
    pub post_response_script: String,
    pub order_key: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableEntry {
    pub key: String,
    pub shared_value: String,
    pub is_secret: bool,
    pub secret_ref: Option<LocalId>,
    pub enabled: bool,
}

impl VariableEntry {
    pub fn public(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            shared_value: value.into(),
            is_secret: false,
            secret_ref: None,
            enabled: true,
        }
    }

    pub fn secret(key: impl Into<String>, secret_ref: LocalId) -> Self {
        Self {
            key: key.into(),
            shared_value: String::new(),
            is_secret: true,
            secret_ref: Some(secret_ref),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonalVarOverride {
    pub value: String,
    pub is_secret: bool,
    pub secret_ref: Option<LocalId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EnvTier {
    Local,
    #[default]
    Development,
    Staging,
    Production,
    Custom,
}

impl EnvTier {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::Development => "Development",
            Self::Staging => "Staging",
            Self::Production => "Production",
            Self::Custom => "Custom",
        }
    }

    pub fn infer(name: &str, is_production: bool) -> Self {
        if is_production {
            return Self::Production;
        }
        let lower = name.trim().to_ascii_lowercase();
        if lower.contains("prod") {
            Self::Production
        } else if lower.contains("stag") || lower.contains("uat") || lower.contains("qa") {
            Self::Staging
        } else if lower.contains("local") {
            Self::Local
        } else if lower.contains("dev") {
            Self::Development
        } else {
            Self::Custom
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub is_production: bool,
    #[serde(default)]
    pub tier: Option<EnvTier>,
    pub variables: Vec<VariableEntry>,
    pub personal_overrides: BTreeMap<String, PersonalVarOverride>,
}

impl Environment {
    pub fn new(name: impl Into<String>, is_production: bool) -> Self {
        let name_str = name.into();
        let inferred = EnvTier::infer(&name_str, is_production);
        Self {
            id: new_uid(),
            name: name_str,
            is_production,
            tier: Some(inferred),
            variables: Vec::new(),
            personal_overrides: BTreeMap::new(),
        }
    }

    pub fn effective_tier(&self) -> EnvTier {
        self.tier
            .unwrap_or_else(|| EnvTier::infer(&self.name, self.is_production))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CookieRecord {
    pub id: String,
    pub domain: String,
    pub path: String,
    pub name: String,
    pub value: String,
    pub expires_epoch_secs: Option<i64>,
    pub secure: bool,
    pub http_only: bool,
}

impl CookieRecord {
    pub fn matches(&self, host: &str, req_path: &str, is_https: bool, now_epoch_secs: i64) -> bool {
        if let Some(exp) = self.expires_epoch_secs
            && exp <= now_epoch_secs
        {
            return false;
        }
        if self.secure && !is_https {
            return false;
        }
        let domain = self.domain.trim_start_matches('.').to_ascii_lowercase();
        let host_l = host.to_ascii_lowercase();
        let host_ok = host_l == domain || host_l.ends_with(&format!(".{domain}"));
        let path_ok = req_path.starts_with(&self.path);
        host_ok && path_ok
    }

    pub fn parse_set_cookie(
        header_val: &str,
        default_host: &str,
        now_epoch_secs: i64,
    ) -> Option<Self> {
        let mut parts = header_val.split(';');
        let (name, value) = parts.next()?.split_once('=')?;
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let mut domain = default_host.to_owned();
        let mut path = "/".to_owned();
        let mut secure = false;
        let mut http_only = false;
        let mut expires_epoch_secs = None;
        for attr in parts {
            let attr = attr.trim();
            if attr.eq_ignore_ascii_case("secure") {
                secure = true;
            } else if attr.eq_ignore_ascii_case("httponly") {
                http_only = true;
            } else if let Some((k, v)) = attr.split_once('=') {
                let k = k.trim();
                let v = v.trim();
                if k.eq_ignore_ascii_case("domain") && !v.is_empty() {
                    domain = v.trim_start_matches('.').to_owned();
                } else if k.eq_ignore_ascii_case("path") && v.starts_with('/') {
                    path = v.to_owned();
                } else if k.eq_ignore_ascii_case("max-age")
                    && let Ok(secs) = v.parse::<i64>()
                {
                    expires_epoch_secs = Some(now_epoch_secs.saturating_add(secs));
                }
            }
        }
        Some(Self {
            id: new_uid(),
            domain,
            path,
            name: name.to_owned(),
            value: value.trim().to_owned(),
            expires_epoch_secs,
            secure,
            http_only,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedExample {
    pub id: String,
    pub request_id: String,
    pub name: String,
    pub status_code: u16,
    pub headers: Vec<(String, String)>,
    pub body_preview: String,
    pub sanitized_redactions: usize,
    pub created_epoch_secs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub timestamp_epoch_secs: i64,
    pub request_id: String,
    pub request_name: String,
    pub method: HttpMethod,
    pub resolved_url: String,
    pub status_code: Option<u16>,
    pub duration_ms: u64,
    pub wire_bytes: u64,
    pub decoded_bytes: u64,
    pub error_kind: Option<String>,
    pub snapshot: RequestItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestTab {
    pub id: String,
    pub request_id: String,
    pub pinned: bool,
}

fn default_sidebar_footer_height() -> f32 {
    166.0
}

fn default_kv_key_ratio() -> f32 {
    0.32
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutPreferences {
    pub sidebar_width: f32,
    pub response_split_ratio: f32,
    #[serde(default)]
    pub sidebar_collapsed: bool,
    #[serde(default)]
    pub sidebar_personal_collapsed: bool,
    #[serde(default)]
    pub workbench_side_by_side: bool,
    #[serde(default = "default_sidebar_footer_height")]
    pub sidebar_footer_height: f32,
    #[serde(default = "default_kv_key_ratio")]
    pub kv_key_ratio: f32,
}

impl Default for LayoutPreferences {
    fn default() -> Self {
        Self {
            sidebar_width: 260.0,
            response_split_ratio: 0.56,
            sidebar_collapsed: false,
            sidebar_personal_collapsed: false,
            workbench_side_by_side: false,
            sidebar_footer_height: default_sidebar_footer_height(),
            kv_key_ratio: default_kv_key_ratio(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveVariable {
    pub key: String,
    pub value: String,
    pub is_secret: bool,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableSuggestion {
    pub key: String,
    pub value_preview: String,
    pub is_secret: bool,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableError {
    Missing(String),
    Circular(Vec<String>),
}

impl std::fmt::Display for VariableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "Missing variable: {{{{{name}}}}}"),
            Self::Circular(chain) => {
                write!(f, "Circular variable reference: {}", chain.join(" -> "))
            }
        }
    }
}

pub struct VariableContext<'a> {
    pub workspace_vars: &'a [VariableEntry],
    pub collection_vars: &'a [VariableEntry],
    pub environment: Option<&'a Environment>,
    pub request_vars: &'a [VariableEntry],
    pub local_secrets: &'a BTreeMap<LocalId, String>,
}

impl<'a> VariableContext<'a> {
    /// Precedence (EV02): Request > Environment (Personal override > Shared) > Collection > Workspace.
    pub fn inspect(&self, key: &str) -> Option<EffectiveVariable> {
        if let Some(v) = self
            .request_vars
            .iter()
            .rfind(|v| v.enabled && v.key == key)
        {
            return Some(self.materialize(v, "Request", None));
        }
        if let Some(env) = self.environment {
            if let Some(ov) = env.personal_overrides.get(key) {
                let value = if ov.is_secret {
                    ov.secret_ref
                        .as_ref()
                        .and_then(|r| self.local_secrets.get(r))
                        .cloned()
                        .unwrap_or_else(|| ov.value.clone())
                } else {
                    ov.value.clone()
                };
                return Some(EffectiveVariable {
                    key: key.to_owned(),
                    value,
                    is_secret: ov.is_secret,
                    source: format!("Environment ({}) · personal override", env.name),
                });
            }
            if let Some(v) = env.variables.iter().rfind(|v| v.enabled && v.key == key) {
                return Some(self.materialize(v, &format!("Environment ({})", env.name), None));
            }
        }
        if let Some(v) = self
            .collection_vars
            .iter()
            .rfind(|v| v.enabled && v.key == key)
        {
            return Some(self.materialize(v, "Collection", None));
        }
        if let Some(v) = self
            .workspace_vars
            .iter()
            .rfind(|v| v.enabled && v.key == key)
        {
            return Some(self.materialize(v, "Workspace", None));
        }
        None
    }

    /// Collects all unique variables visible in the current scope in precedence order,
    /// followed by built-in `$` dynamic tokens for `{` / `{{` editor autocomplete.
    pub fn available_suggestions(&self) -> Vec<VariableSuggestion> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();

        let mut push_entry = |key: &str, value: &str, is_secret: bool, scope: String| {
            let trimmed = key.trim();
            if trimmed.is_empty() || !seen.insert(trimmed.to_owned()) {
                return;
            }
            let preview = if is_secret {
                if value.is_empty() {
                    "(secret not set)".to_owned()
                } else {
                    "••••••••".to_owned()
                }
            } else {
                value.to_owned()
            };
            out.push(VariableSuggestion {
                key: trimmed.to_owned(),
                value_preview: preview,
                is_secret,
                scope,
            });
        };

        for v in self.request_vars.iter().rev().filter(|v| v.enabled) {
            let eff = self.materialize(v, "Request", None);
            push_entry(&eff.key, &eff.value, eff.is_secret, "Request".into());
        }
        if let Some(env) = self.environment {
            for (k, ov) in &env.personal_overrides {
                let val = if ov.is_secret {
                    ov.secret_ref
                        .as_ref()
                        .and_then(|r| self.local_secrets.get(r))
                        .cloned()
                        .unwrap_or_else(|| ov.value.clone())
                } else {
                    ov.value.clone()
                };
                push_entry(k, &val, ov.is_secret, format!("{} · Personal", env.name));
            }
            for v in env.variables.iter().rev().filter(|v| v.enabled) {
                let eff = self.materialize(v, &env.name, None);
                push_entry(&eff.key, &eff.value, eff.is_secret, env.name.clone());
            }
        }
        for v in self.collection_vars.iter().rev().filter(|v| v.enabled) {
            let eff = self.materialize(v, "Collection", None);
            push_entry(&eff.key, &eff.value, eff.is_secret, "Collection".into());
        }
        for v in self.workspace_vars.iter().rev().filter(|v| v.enabled) {
            let eff = self.materialize(v, "Global", None);
            push_entry(&eff.key, &eff.value, eff.is_secret, "Global".into());
        }

        for (dyn_key, desc) in [
            ("$uuid", "Random UUID v4"),
            ("$timestamp", "Unix timestamp (sec)"),
            ("$isoTimestamp", "UTC ISO-8601 timestamp"),
            ("$randomInt", "Random 0..99999"),
            ("$randomHex", "16-char random hex"),
        ] {
            push_entry(dyn_key, desc, false, "Dynamic".into());
        }
        out
    }

    fn materialize(
        &self,
        entry: &VariableEntry,
        source: &str,
        fallback: Option<&str>,
    ) -> EffectiveVariable {
        let value = if entry.is_secret {
            entry
                .secret_ref
                .as_ref()
                .and_then(|r| self.local_secrets.get(r))
                .cloned()
                .unwrap_or_else(|| fallback.unwrap_or(&entry.shared_value).to_owned())
        } else {
            entry.shared_value.clone()
        };
        EffectiveVariable {
            key: entry.key.clone(),
            value,
            is_secret: entry.is_secret,
            source: source.to_owned(),
        }
    }

    pub fn substitute(&self, template: &str, masked: bool) -> Result<String, VariableError> {
        let mut stack = Vec::new();
        self.substitute_inner(template, masked, &mut stack)
    }

    fn substitute_inner(
        &self,
        template: &str,
        masked: bool,
        stack: &mut Vec<String>,
    ) -> Result<String, VariableError> {
        let mut out = String::with_capacity(template.len());
        let mut rest = template;
        while let Some(start) = rest.find("{{") {
            out.push_str(&rest[..start]);
            let after_open = &rest[start + 2..];
            let Some(end) = after_open.find("}}") else {
                out.push_str("{{");
                rest = after_open;
                continue;
            };
            let var_name = after_open[..end].trim();
            if var_name.starts_with('$')
                && let Some(dyn_val) = crate::dynamic_vars::eval_dynamic_token(
                    var_name,
                    (stack.len() as u64).wrapping_add(out.len() as u64 + 1),
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                )
            {
                if masked {
                    out.push_str(&format!("<{var_name}>"));
                } else {
                    out.push_str(&dyn_val);
                }
                rest = &after_open[end + 2..];
                continue;
            }
            if stack.iter().any(|s| s == var_name) {
                let mut cycle = stack.clone();
                cycle.push(var_name.to_owned());
                return Err(VariableError::Circular(cycle));
            }
            let Some(eff) = self.inspect(var_name) else {
                return Err(VariableError::Missing(var_name.to_owned()));
            };
            if eff.is_secret && masked {
                out.push_str("••••••••");
            } else {
                stack.push(var_name.to_owned());
                let resolved = self.substitute_inner(&eff.value, masked, stack)?;
                stack.pop();
                out.push_str(&resolved);
            }
            rest = &after_open[end + 2..];
        }
        out.push_str(rest);
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedHeader {
    pub name: String,
    pub value: String,
    pub generated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRequest {
    pub method: HttpMethod,
    pub url: String,
    pub masked_url: String,
    pub headers: Vec<ResolvedHeader>,
    pub masked_headers: Vec<ResolvedHeader>,
    pub body: RequestBody,
    pub masked_body_preview: String,
    pub auth_source: String,
    pub warnings: Vec<String>,
    pub blocking_errors: Vec<String>,
    pub requires_production_confirm: bool,
    pub connection: RequestConnectionPolicy,
}

pub fn resolve_effective_auth(
    collection: Option<&Collection>,
    folder_chain: &[&Folder],
    request: &RequestItem,
) -> EffectiveAuth {
    if !matches!(request.auth, AuthConfig::Inherit) {
        return EffectiveAuth {
            config: request.auth.clone(),
            source: "Request override".into(),
        };
    }
    for folder in folder_chain.iter().rev() {
        if !matches!(folder.auth, AuthConfig::Inherit) {
            return EffectiveAuth {
                config: folder.auth.clone(),
                source: format!("Folder ({})", folder.name),
            };
        }
    }
    if let Some(col) = collection
        && !matches!(col.auth, AuthConfig::Inherit)
    {
        return EffectiveAuth {
            config: col.auth.clone(),
            source: format!("Collection ({})", col.name),
        };
    }
    EffectiveAuth {
        config: AuthConfig::NoAuth,
        source: "Default (No Auth)".into(),
    }
}

pub fn resolve_request_preview(
    request: &RequestItem,
    collection: Option<&Collection>,
    folder_chain: &[&Folder],
    vars: &VariableContext<'_>,
    cookies: &[CookieRecord],
    now_epoch_secs: i64,
) -> ResolvedRequest {
    let mut warnings = Vec::new();
    let mut blocking_errors = Vec::new();

    // 1. Substitute URL & path params.
    // When `request.query_params` has rows, strip the inline `?query` mirror from `request.url`
    // so synced URLs never double-append query parameters on send/preview. If `request.url`
    // carries a freshly hand-edited `?query` that diverges from `request.query_params`, honor it.
    let (url_base_part, url_inline_query, url_fragment) = split_url_parts(&request.url);
    let parsed_inline_rows = if url_inline_query.is_some() {
        extract_url_query_rows(&request.url)
    } else {
        Vec::new()
    };
    let inline_diverges_from_table = url_inline_query.is_some() && {
        let enabled_table: Vec<(&str, &str)> = request
            .query_params
            .iter()
            .filter(|q| q.enabled && !q.key.is_empty())
            .map(|q| (q.key.as_str(), q.value.as_str()))
            .collect();
        let inline_pairs: Vec<(&str, &str)> = parsed_inline_rows
            .iter()
            .filter(|q| !q.key.is_empty())
            .map(|q| (q.key.as_str(), q.value.as_str()))
            .collect();
        enabled_table != inline_pairs
    };
    let effective_query_rows: &[KeyValueRow] = if inline_diverges_from_table {
        &parsed_inline_rows
    } else {
        &request.query_params
    };
    let effective_url_template = if !effective_query_rows.is_empty() {
        url_base_part.to_owned()
    } else if let Some(inline_q) = url_inline_query {
        format!("{url_base_part}?{inline_q}")
    } else {
        url_base_part.to_owned()
    };

    let mut raw_url = match vars.substitute(&effective_url_template, false) {
        Ok(u) => u,
        Err(err) => {
            blocking_errors.push(err.to_string());
            effective_url_template.clone()
        }
    };
    let mut masked_url_base = match vars.substitute(&effective_url_template, true) {
        Ok(u) => u,
        Err(_) => effective_url_template,
    };
    for p in request
        .path_params
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
    {
        let val = match vars.substitute(&p.value, false) {
            Ok(v) => v,
            Err(err) => {
                blocking_errors.push(err.to_string());
                p.value.clone()
            }
        };
        let masked_val = vars
            .substitute(&p.value, true)
            .unwrap_or_else(|_| p.value.clone());
        let encoded = encode_path_segment_value(&val);
        let encoded_masked = encode_path_segment_value(&masked_val);
        raw_url = replace_path_param_tokens(&raw_url, &p.key, &encoded);
        masked_url_base = replace_path_param_tokens(&masked_url_base, &p.key, &encoded_masked);
    }

    // 2. Append ordered query params (preserving duplicates, encoding cleanly).
    let mut query_pairs = Vec::new();
    let mut masked_query_pairs = Vec::new();
    for q in effective_query_rows
        .iter()
        .filter(|q| q.enabled && !q.key.is_empty())
    {
        let key = vars.substitute(&q.key, false).unwrap_or_else(|e| {
            blocking_errors.push(e.to_string());
            q.key.clone()
        });
        let val = vars.substitute(&q.value, false).unwrap_or_else(|e| {
            blocking_errors.push(e.to_string());
            q.value.clone()
        });
        let mval = vars
            .substitute(&q.value, true)
            .unwrap_or_else(|_| q.value.clone());
        query_pairs.push((key.clone(), val));
        masked_query_pairs.push((key, mval));
    }

    // 3. Resolve Auth & inject header or query API key (AU04, AU08, AU10).
    let eff_auth = resolve_effective_auth(collection, folder_chain, request);
    let mut headers = Vec::new();
    let mut masked_headers = Vec::new();
    let mut deferred_sigv4: Option<(String, String, String, String, String)> = None;
    let mut deferred_digest: Option<(String, String, String, String)> = None;
    match &eff_auth.config {
        AuthConfig::Inherit | AuthConfig::NoAuth => {}
        AuthConfig::Bearer { token } => {
            if token.trim().is_empty() {
                warnings.push(
                    "Bearer auth selected with empty token — enter a token or {{variable}} in Auth."
                        .into(),
                );
            } else {
                let real = vars.substitute(token, false).unwrap_or_else(|e| {
                    blocking_errors.push(e.to_string());
                    token.clone()
                });
                headers.push(ResolvedHeader {
                    name: "Authorization".into(),
                    value: format!("Bearer {real}"),
                    generated: true,
                });
                masked_headers.push(ResolvedHeader {
                    name: "Authorization".into(),
                    value: "Bearer ••••••••".into(),
                    generated: true,
                });
            }
        }
        AuthConfig::OAuth2 { access_token, .. } => {
            if access_token.trim().is_empty() {
                warnings.push(
                    "OAuth 2.0 selected without an active access token — fetch a token in Auth."
                        .into(),
                );
            } else {
                let real = vars.substitute(access_token, false).unwrap_or_else(|e| {
                    blocking_errors.push(e.to_string());
                    access_token.clone()
                });
                headers.push(ResolvedHeader {
                    name: "Authorization".into(),
                    value: format!("Bearer {real}"),
                    generated: true,
                });
                masked_headers.push(ResolvedHeader {
                    name: "Authorization".into(),
                    value: "Bearer ••••••••".into(),
                    generated: true,
                });
            }
        }
        AuthConfig::Basic { username, password } => {
            let u = vars.substitute(username, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                username.clone()
            });
            let p = vars.substitute(password, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                password.clone()
            });
            let encoded = encode_basic(&u, &p);
            headers.push(ResolvedHeader {
                name: "Authorization".into(),
                value: format!("Basic {encoded}"),
                generated: true,
            });
            masked_headers.push(ResolvedHeader {
                name: "Authorization".into(),
                value: "Basic ••••••••".into(),
                generated: true,
            });
        }
        AuthConfig::ApiKey {
            key,
            value,
            placement,
        } => {
            let real_k = vars.substitute(key, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                key.clone()
            });
            let real_v = vars.substitute(value, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                value.clone()
            });
            match placement {
                ApiKeyPlacement::Header => {
                    if !real_k.trim().is_empty() {
                        headers.push(ResolvedHeader {
                            name: real_k.clone(),
                            value: real_v,
                            generated: true,
                        });
                        masked_headers.push(ResolvedHeader {
                            name: real_k,
                            value: "••••••••".into(),
                            generated: true,
                        });
                    }
                }
                ApiKeyPlacement::Query => {
                    if !real_k.trim().is_empty() {
                        warnings.push("API key in query string (H05): visible in server logs and scrubbed on default export.".into());
                        query_pairs.push((real_k.clone(), real_v));
                        masked_query_pairs.push((real_k, "••••••••".into()));
                    }
                }
            }
        }
        AuthConfig::AwsSigV4 {
            access_key,
            secret_key,
            region,
            service,
            session_token,
        } => {
            let ak = vars.substitute(access_key, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                access_key.clone()
            });
            let sk = vars.substitute(secret_key, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                secret_key.clone()
            });
            let reg = vars.substitute(region, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                region.clone()
            });
            let svc = vars.substitute(service, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                service.clone()
            });
            let st = vars
                .substitute(session_token, false)
                .unwrap_or_else(|_| session_token.clone());
            deferred_sigv4 = Some((ak, sk, reg, svc, st));
        }
        AuthConfig::Digest {
            username,
            password,
            realm,
            nonce,
        } => {
            let u = vars.substitute(username, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                username.clone()
            });
            let p = vars.substitute(password, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                password.clone()
            });
            let r = vars.substitute(realm, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                realm.clone()
            });
            let n = vars.substitute(nonce, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                nonce.clone()
            });
            deferred_digest = Some((u, p, r, n));
        }
    }

    let mut final_url = append_query_pairs(&raw_url, &query_pairs);
    let mut final_masked_url = append_query_pairs(&masked_url_base, &masked_query_pairs);
    if let Some(frag) = url_fragment {
        if let Ok(rf) = vars.substitute(frag, false) {
            final_url.push('#');
            final_url.push_str(&rf);
        }
        if let Ok(mf) = vars.substitute(frag, true) {
            final_masked_url.push('#');
            final_masked_url.push_str(&mf);
        }
    }

    if let Err(err) = url::Url::parse(&final_url) {
        blocking_errors.push(format!("Invalid URL: {err}"));
    } else if let Ok(parsed) = url::Url::parse(&final_url)
        && let Some(host) = parsed.host_str()
    {
        let is_https = parsed.scheme() == "https";
        let cookie_pairs: Vec<String> = cookies
            .iter()
            .filter(|c| c.matches(host, parsed.path(), is_https, now_epoch_secs))
            .map(|c| format!("{}={}", c.name, c.value))
            .collect();
        if !cookie_pairs.is_empty() {
            let joined = cookie_pairs.join("; ");
            headers.push(ResolvedHeader {
                name: "Cookie".into(),
                value: joined,
                generated: true,
            });
            masked_headers.push(ResolvedHeader {
                name: "Cookie".into(),
                value: format!("({} cookie(s) hidden)", cookie_pairs.len()),
                generated: true,
            });
        }
    }

    // 4. User-specified headers + HTTP header validation (Q05).
    for h in request
        .headers
        .iter()
        .filter(|h| h.enabled && !h.key.is_empty())
    {
        if !is_valid_header_name(&h.key) {
            blocking_errors.push(format!("Invalid HTTP header name: '{}'", h.key));
        }
        let val = vars.substitute(&h.value, false).unwrap_or_else(|e| {
            blocking_errors.push(e.to_string());
            h.value.clone()
        });
        let mval = vars
            .substitute(&h.value, true)
            .unwrap_or_else(|_| h.value.clone());
        if val.contains('\r') || val.contains('\n') {
            blocking_errors.push(format!(
                "Header '{}' contains illegal CR/LF characters",
                h.key
            ));
        }
        let is_sensitive = matches!(
            h.key.trim().to_ascii_lowercase().as_str(),
            "authorization" | "proxy-authorization" | "cookie" | "x-api-key"
        );
        headers.push(ResolvedHeader {
            name: h.key.clone(),
            value: val,
            generated: false,
        });
        masked_headers.push(ResolvedHeader {
            name: h.key.clone(),
            value: if is_sensitive {
                "••••••••".into()
            } else {
                mval
            },
            generated: false,
        });
    }

    // 5. Body & GraphQL resolution + JSON/XML/file validation + Idempotency-Key injection.
    let mut is_graphql_mutation = false;
    let (resolved_body, masked_body_preview) = match &request.body {
        RequestBody::GraphQl {
            query,
            variables,
            operation_name,
        } => {
            let real_q = vars.substitute(query, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                query.clone()
            });
            let masked_q = vars
                .substitute(query, true)
                .unwrap_or_else(|_| query.clone());
            let real_vars = vars.substitute(variables, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                variables.clone()
            });
            let masked_vars = vars
                .substitute(variables, true)
                .unwrap_or_else(|_| variables.clone());
            let real_op = operation_name
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|op| {
                    vars.substitute(op.trim(), false)
                        .unwrap_or_else(|_| op.trim().to_owned())
                });

            let parsed_vars = if real_vars.trim().is_empty() {
                serde_json::json!({})
            } else {
                match serde_json::from_str::<serde_json::Value>(&real_vars) {
                    Ok(v) if v.is_object() || v.is_null() => v,
                    Ok(_) => {
                        blocking_errors
                            .push("GraphQL variables must be a JSON object ({ ... })".into());
                        serde_json::json!({})
                    }
                    Err(err) => {
                        blocking_errors.push(format!("Invalid GraphQL variables JSON: {err}"));
                        serde_json::json!({})
                    }
                }
            };
            let masked_parsed_vars = serde_json::from_str::<serde_json::Value>(&masked_vars)
                .unwrap_or_else(|_| serde_json::json!({}));

            let analysis =
                crate::graphql::analyze_graphql_document(&real_q, real_op.as_deref(), 10, 150);
            is_graphql_mutation =
                analysis.active_kind == crate::graphql::GraphQlOperationKind::Mutation;
            for w in analysis.complexity_warnings {
                warnings.push(format!("GraphQL complexity: {w}"));
            }
            if !analysis.potential_n_plus_1_paths.is_empty() {
                warnings.push(format!(
                    "GraphQL nested list path(s) [{}] — ensure backend uses batched DataLoader/$in queries to avoid N+1",
                    analysis.potential_n_plus_1_paths.join(", ")
                ));
            }

            if !headers
                .iter()
                .any(|h| h.name.eq_ignore_ascii_case("accept"))
            {
                let accept_val = "application/graphql-response+json, application/json";
                headers.push(ResolvedHeader {
                    name: "Accept".into(),
                    value: accept_val.into(),
                    generated: true,
                });
                masked_headers.push(ResolvedHeader {
                    name: "Accept".into(),
                    value: accept_val.into(),
                    generated: true,
                });
            }

            if request.method == HttpMethod::Get {
                if is_graphql_mutation {
                    blocking_errors.push(
                        "GraphQL mutations are not allowed over HTTP GET (switch method to POST)"
                            .into(),
                    );
                }
                let mut gql_qp = vec![("query".to_owned(), real_q)];
                let mut gql_masked_qp = vec![("query".to_owned(), masked_q)];
                if parsed_vars.as_object().is_some_and(|m| !m.is_empty()) {
                    gql_qp.push(("variables".to_owned(), parsed_vars.to_string()));
                    gql_masked_qp.push(("variables".to_owned(), masked_parsed_vars.to_string()));
                }
                if let Some(ref op) = real_op {
                    gql_qp.push(("operationName".to_owned(), op.clone()));
                    gql_masked_qp.push(("operationName".to_owned(), op.clone()));
                }
                final_url = append_query_pairs(&final_url, &gql_qp);
                final_masked_url = append_query_pairs(&final_masked_url, &gql_masked_qp);
                (RequestBody::None, String::new())
            } else if request.method == HttpMethod::Head {
                (RequestBody::None, String::new())
            } else {
                let mut env_obj = serde_json::Map::new();
                env_obj.insert("query".into(), serde_json::Value::String(real_q));
                env_obj.insert("variables".into(), parsed_vars);
                if let Some(ref op) = real_op {
                    env_obj.insert(
                        "operationName".into(),
                        serde_json::Value::String(op.clone()),
                    );
                }
                let mut masked_obj = serde_json::Map::new();
                masked_obj.insert("query".into(), serde_json::Value::String(masked_q));
                masked_obj.insert("variables".into(), masked_parsed_vars);
                if let Some(op) = real_op {
                    masked_obj.insert("operationName".into(), serde_json::Value::String(op));
                }
                let real_json =
                    serde_json::to_string(&serde_json::Value::Object(env_obj)).unwrap_or_default();
                let masked_json =
                    serde_json::to_string_pretty(&serde_json::Value::Object(masked_obj))
                        .unwrap_or_default();
                (RequestBody::Json(real_json), masked_json)
            }
        }
        _ if matches!(request.method, HttpMethod::Get | HttpMethod::Head) => {
            (RequestBody::None, String::new())
        }
        RequestBody::None => (RequestBody::None, String::new()),
        RequestBody::Json(raw) => {
            let real = vars.substitute(raw, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                raw.clone()
            });
            let masked = vars.substitute(raw, true).unwrap_or_else(|_| raw.clone());
            if !real.trim().is_empty()
                && let Err(err) = serde_json::from_str::<serde_json::Value>(&real)
            {
                blocking_errors.push(format!("Invalid JSON body: {err}"));
            }
            (RequestBody::Json(real), masked)
        }
        RequestBody::Text(raw) => {
            let real = vars.substitute(raw, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                raw.clone()
            });
            let masked = vars.substitute(raw, true).unwrap_or_else(|_| raw.clone());
            (RequestBody::Text(real), masked)
        }
        RequestBody::Xml(raw) => {
            let real = vars.substitute(raw, false).unwrap_or_else(|e| {
                blocking_errors.push(e.to_string());
                raw.clone()
            });
            let masked = vars.substitute(raw, true).unwrap_or_else(|_| raw.clone());
            if !real.trim().is_empty()
                && let Err(err) = roxmltree::Document::parse(&real)
            {
                warnings.push(format!("XML parse note: {err}"));
            }
            (RequestBody::Xml(real), masked)
        }
        RequestBody::FormUrlEncoded(rows) => {
            let mut real_rows = Vec::new();
            let mut preview_parts = Vec::new();
            for row in rows.iter().filter(|r| r.enabled && !r.key.is_empty()) {
                let k = vars.substitute(&row.key, false).unwrap_or_else(|e| {
                    blocking_errors.push(e.to_string());
                    row.key.clone()
                });
                let v = vars.substitute(&row.value, false).unwrap_or_else(|e| {
                    blocking_errors.push(e.to_string());
                    row.value.clone()
                });
                let mv = vars
                    .substitute(&row.value, true)
                    .unwrap_or_else(|_| row.value.clone());
                preview_parts.push(format!("{k}={mv}"));
                real_rows.push(KeyValueRow::new(k, v));
            }
            (
                RequestBody::FormUrlEncoded(real_rows),
                preview_parts.join("&"),
            )
        }
        RequestBody::Multipart(fields) => {
            let mut resolved_fields = Vec::new();
            let mut preview = Vec::new();
            for f in fields.iter().filter(|f| f.enabled && !f.name.is_empty()) {
                match &f.value {
                    MultipartValueKind::Text(v) => {
                        let real = vars.substitute(v, false).unwrap_or_else(|e| {
                            blocking_errors.push(e.to_string());
                            v.clone()
                        });
                        let mv = vars.substitute(v, true).unwrap_or_else(|_| v.clone());
                        preview.push(format!("{}={mv}", f.name));
                        resolved_fields.push(MultipartField {
                            name: f.name.clone(),
                            value: MultipartValueKind::Text(real),
                            enabled: true,
                        });
                    }
                    MultipartValueKind::File {
                        local_path,
                        file_name,
                        content_type,
                    } => {
                        if !std::path::Path::new(local_path).is_file() {
                            blocking_errors.push(format!(
                                "Local upload file missing for '{}': {file_name}",
                                f.name
                            ));
                        }
                        preview.push(format!("{}=<local file: {file_name}>", f.name));
                        resolved_fields.push(MultipartField {
                            name: f.name.clone(),
                            value: MultipartValueKind::File {
                                local_path: local_path.clone(),
                                file_name: file_name.clone(),
                                content_type: content_type.clone(),
                            },
                            enabled: true,
                        });
                    }
                }
            }
            (RequestBody::Multipart(resolved_fields), preview.join("\n"))
        }
        RequestBody::Binary {
            local_path,
            content_type,
        } => match local_path {
            Some(path) if std::path::Path::new(path).is_file() => (
                RequestBody::Binary {
                    local_path: Some(path.clone()),
                    content_type: content_type.clone(),
                },
                format!("<binary file: {path} ({content_type})>"),
            ),
            Some(_) => {
                blocking_errors.push("Local binary body file is missing".into());
                (request.body.clone(), "<missing binary file>".into())
            }
            None => {
                blocking_errors.push("Select a local binary file before sending".into());
                (request.body.clone(), "<no binary file selected>".into())
            }
        },
    };

    // 5b. Deferred AWS SigV4 / HTTP Digest signing using final URL & resolved body (AU10).
    if let Some((ak, sk, reg, svc, st)) = deferred_sigv4
        && !ak.trim().is_empty()
        && let Ok(parsed) = url::Url::parse(&final_url)
    {
        let host = match parsed.port() {
            Some(p) => format!("{}:{p}", parsed.host_str().unwrap_or_default()),
            None => parsed.host_str().unwrap_or_default().to_owned(),
        };
        let payload_bytes = match &resolved_body {
            RequestBody::Json(s) | RequestBody::Text(s) | RequestBody::Xml(s) => s.as_bytes(),
            _ => b"",
        };
        let dt = time::OffsetDateTime::from_unix_timestamp(now_epoch_secs)
            .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
        let amz_date = format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
            dt.year(),
            dt.month() as u8,
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second()
        );
        if let Some((auth_hdr, payload_sha)) = compute_aws_sigv4_headers(
            request.method.as_str(),
            &host,
            parsed.path(),
            parsed.query().unwrap_or(""),
            &reg,
            &svc,
            &ak,
            &sk,
            &amz_date,
            payload_bytes,
        ) {
            headers.push(ResolvedHeader {
                name: "x-amz-date".into(),
                value: amz_date.clone(),
                generated: true,
            });
            masked_headers.push(ResolvedHeader {
                name: "x-amz-date".into(),
                value: amz_date,
                generated: true,
            });
            headers.push(ResolvedHeader {
                name: "x-amz-content-sha256".into(),
                value: payload_sha.clone(),
                generated: true,
            });
            masked_headers.push(ResolvedHeader {
                name: "x-amz-content-sha256".into(),
                value: payload_sha,
                generated: true,
            });
            if !st.trim().is_empty() {
                headers.push(ResolvedHeader {
                    name: "x-amz-security-token".into(),
                    value: st,
                    generated: true,
                });
                masked_headers.push(ResolvedHeader {
                    name: "x-amz-security-token".into(),
                    value: "••••••••".into(),
                    generated: true,
                });
            }
            headers.push(ResolvedHeader {
                name: "Authorization".into(),
                value: auth_hdr,
                generated: true,
            });
            masked_headers.push(ResolvedHeader {
                name: "Authorization".into(),
                value: "AWS4-HMAC-SHA256 ••••••••".into(),
                generated: true,
            });
        }
    }
    if let Some((u, p, realm, nonce)) = deferred_digest
        && !u.trim().is_empty()
        && let Ok(parsed) = url::Url::parse(&final_url)
    {
        let mut uri_path = parsed.path().to_owned();
        if let Some(q) = parsed.query() {
            uri_path.push('?');
            uri_path.push_str(q);
        }
        let digest_hdr = compute_digest_sha256_header(
            &u,
            &p,
            &realm,
            &nonce,
            request.method.as_str(),
            &uri_path,
        );
        headers.push(ResolvedHeader {
            name: "Authorization".into(),
            value: digest_hdr,
            generated: true,
        });
        masked_headers.push(ResolvedHeader {
            name: "Authorization".into(),
            value: "Digest ••••••••".into(),
            generated: true,
        });
    }

    // 6. Idempotency-Key generation for mutating HTTP (`POST` / `PATCH`) & GraphQL `mutation` requests.
    let should_inject_idempotency = match request.idempotency_mode {
        IdempotencyMode::Disabled => false,
        IdempotencyMode::Always => true,
        IdempotencyMode::AutoOnMutating => {
            if request.is_graphql() {
                is_graphql_mutation
            } else {
                matches!(request.method, HttpMethod::Post | HttpMethod::Patch)
            }
        }
    };
    if should_inject_idempotency
        && !headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("idempotency-key"))
    {
        let idem_val = if !request.custom_idempotency_key.trim().is_empty() {
            vars.substitute(request.custom_idempotency_key.trim(), false)
                .unwrap_or_else(|_| request.custom_idempotency_key.trim().to_owned())
        } else {
            compute_deterministic_idempotency_key(
                &request.id,
                request.method,
                &final_url,
                &resolved_body,
                request.idempotency_nonce,
            )
        };
        headers.push(ResolvedHeader {
            name: "Idempotency-Key".into(),
            value: idem_val.clone(),
            generated: true,
        });
        masked_headers.push(ResolvedHeader {
            name: "Idempotency-Key".into(),
            value: idem_val,
            generated: true,
        });
    }

    let is_mutating_call = if request.is_graphql() {
        is_graphql_mutation
    } else {
        request.method.is_mutating()
    };
    let requires_production_confirm =
        vars.environment.is_some_and(|e| e.is_production) && is_mutating_call;

    ResolvedRequest {
        method: request.method,
        url: final_url,
        masked_url: final_masked_url,
        headers,
        masked_headers,
        body: resolved_body,
        masked_body_preview,
        auth_source: eff_auth.source,
        warnings,
        blocking_errors,
        requires_production_confirm,
        connection: request.connection.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn compute_aws_sigv4_headers(
    method: &str,
    host: &str,
    path: &str,
    query: &str,
    region: &str,
    service: &str,
    access_key: &str,
    secret_key: &str,
    amz_date: &str,
    payload: &[u8],
) -> Option<(String, String)> {
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::{Digest, Sha256};
    type HmacSha256 = Hmac<Sha256>;
    let hmac_bytes = |key: &[u8], data: &[u8]| -> Option<Vec<u8>> {
        let mut mac = HmacSha256::new_from_slice(key).ok()?;
        mac.update(data);
        Some(mac.finalize().into_bytes().to_vec())
    };
    let date_stamp = amz_date.get(..8)?;
    let payload_hash = hex::encode(Sha256::digest(payload));
    let canonical_headers = format!("host:{}\nx-amz-date:{amz_date}\n", host.trim());
    let signed_headers = "host;x-amz-date";
    let canonical_req = format!(
        "{}\n{}\n{query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}",
        method.to_ascii_uppercase(),
        if path.is_empty() { "/" } else { path },
    );
    let scope = format!(
        "{date_stamp}/{}/{}/aws4_request",
        if region.trim().is_empty() {
            "us-east-1"
        } else {
            region.trim()
        },
        if service.trim().is_empty() {
            "execute-api"
        } else {
            service.trim()
        }
    );
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        hex::encode(Sha256::digest(canonical_req.as_bytes()))
    );
    let k_date = hmac_bytes(
        format!("AWS4{secret_key}").as_bytes(),
        date_stamp.as_bytes(),
    )?;
    let k_region = hmac_bytes(
        &k_date,
        if region.trim().is_empty() {
            b"us-east-1"
        } else {
            region.trim().as_bytes()
        },
    )?;
    let k_service = hmac_bytes(
        &k_region,
        if service.trim().is_empty() {
            b"execute-api"
        } else {
            service.trim().as_bytes()
        },
    )?;
    let k_signing = hmac_bytes(&k_service, b"aws4_request")?;
    let sig = hex::encode(hmac_bytes(&k_signing, string_to_sign.as_bytes())?);
    let auth = format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders={signed_headers}, Signature={sig}"
    );
    Some((auth, payload_hash))
}

fn compute_digest_sha256_header(
    username: &str,
    password: &str,
    realm: &str,
    nonce: &str,
    method: &str,
    uri: &str,
) -> String {
    use sha2::{Digest, Sha256};
    let nc = "00000001";
    let cnonce = "swaggen01";
    let ha1 = hex::encode(Sha256::digest(
        format!("{username}:{realm}:{password}").as_bytes(),
    ));
    let ha2 = hex::encode(Sha256::digest(format!("{method}:{uri}").as_bytes()));
    let resp = hex::encode(Sha256::digest(
        format!("{ha1}:{nonce}:{nc}:{cnonce}:auth:{ha2}").as_bytes(),
    ));
    format!(
        "Digest username=\"{username}\", realm=\"{realm}\", nonce=\"{nonce}\", uri=\"{uri}\", algorithm=SHA-256, qop=auth, nc={nc}, cnonce=\"{cnonce}\", response=\"{resp}\""
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencedVariableStatus {
    pub name: String,
    pub resolved_value: Option<String>,
    pub masked_value: Option<String>,
    pub source: Option<String>,
    pub is_secret: bool,
    pub error: Option<String>,
}

pub fn extract_template_var_names(input: &str, out: &mut Vec<String>) {
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            break;
        };
        let name = after[..end].trim();
        if !name.is_empty() && !out.iter().any(|x| x == name) {
            out.push(name.to_owned());
        }
        rest = &after[end + 2..];
    }
}

/// Inspects every `{{variable}}` referenced in the request's URL, params, headers, body, and auth,
/// returning live resolution values, scope sources, or missing/circular warnings for the UI!
pub fn inspect_request_variables(
    request: &RequestItem,
    collection: Option<&Collection>,
    folder_chain: &[&Folder],
    vars: &VariableContext<'_>,
) -> Vec<ReferencedVariableStatus> {
    let mut names = Vec::new();
    extract_template_var_names(&request.url, &mut names);
    for p in request.path_params.iter().filter(|r| r.enabled) {
        extract_template_var_names(&p.value, &mut names);
    }
    for q in request.query_params.iter().filter(|r| r.enabled) {
        extract_template_var_names(&q.key, &mut names);
        extract_template_var_names(&q.value, &mut names);
    }
    for h in request.headers.iter().filter(|r| r.enabled) {
        extract_template_var_names(&h.value, &mut names);
    }
    match &request.body {
        RequestBody::None | RequestBody::Binary { .. } => {}
        RequestBody::Json(s) | RequestBody::Text(s) | RequestBody::Xml(s) => {
            extract_template_var_names(s, &mut names);
        }
        RequestBody::GraphQl {
            query,
            variables,
            operation_name,
        } => {
            extract_template_var_names(query, &mut names);
            extract_template_var_names(variables, &mut names);
            if let Some(op) = operation_name {
                extract_template_var_names(op, &mut names);
            }
        }
        RequestBody::FormUrlEncoded(rows) => {
            for r in rows.iter().filter(|r| r.enabled) {
                extract_template_var_names(&r.key, &mut names);
                extract_template_var_names(&r.value, &mut names);
            }
        }
        RequestBody::Multipart(fields) => {
            for f in fields.iter().filter(|f| f.enabled) {
                if let MultipartValueKind::Text(v) = &f.value {
                    extract_template_var_names(v, &mut names);
                }
            }
        }
    }
    let eff_auth = resolve_effective_auth(collection, folder_chain, request);
    match &eff_auth.config {
        AuthConfig::Inherit | AuthConfig::NoAuth => {}
        AuthConfig::Bearer { token } => extract_template_var_names(token, &mut names),
        AuthConfig::OAuth2 {
            auth_url,
            token_url,
            client_id,
            client_secret,
            access_token,
            ..
        } => {
            extract_template_var_names(auth_url, &mut names);
            extract_template_var_names(token_url, &mut names);
            extract_template_var_names(client_id, &mut names);
            extract_template_var_names(client_secret, &mut names);
            extract_template_var_names(access_token, &mut names);
        }
        AuthConfig::Basic { username, password } => {
            extract_template_var_names(username, &mut names);
            extract_template_var_names(password, &mut names);
        }
        AuthConfig::ApiKey { key, value, .. } => {
            extract_template_var_names(key, &mut names);
            extract_template_var_names(value, &mut names);
        }
        AuthConfig::AwsSigV4 {
            access_key,
            secret_key,
            region,
            service,
            session_token,
        } => {
            extract_template_var_names(access_key, &mut names);
            extract_template_var_names(secret_key, &mut names);
            extract_template_var_names(region, &mut names);
            extract_template_var_names(service, &mut names);
            extract_template_var_names(session_token, &mut names);
        }
        AuthConfig::Digest {
            username,
            password,
            realm,
            nonce,
        } => {
            extract_template_var_names(username, &mut names);
            extract_template_var_names(password, &mut names);
            extract_template_var_names(realm, &mut names);
            extract_template_var_names(nonce, &mut names);
        }
    }

    names
        .into_iter()
        .map(|name| {
            if name.starts_with('$')
                && let Some(dyn_val) = crate::dynamic_vars::eval_dynamic_token(
                    &name,
                    1,
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                )
            {
                return ReferencedVariableStatus {
                    name,
                    resolved_value: Some(dyn_val.clone()),
                    masked_value: Some(dyn_val),
                    source: Some("Dynamic Generator".into()),
                    is_secret: false,
                    error: None,
                };
            }
            let tpl = format!("{{{{{name}}}}}");
            let eff = vars.inspect(&name);
            match vars.substitute(&tpl, false) {
                Ok(resolved_val) => {
                    let masked_val = vars
                        .substitute(&tpl, true)
                        .unwrap_or_else(|_| "••••••••".into());
                    ReferencedVariableStatus {
                        name,
                        resolved_value: Some(resolved_val),
                        masked_value: Some(masked_val),
                        source: eff.as_ref().map(|e| e.source.clone()),
                        is_secret: eff.as_ref().is_some_and(|e| e.is_secret),
                        error: None,
                    }
                }
                Err(err) => ReferencedVariableStatus {
                    name,
                    resolved_value: None,
                    masked_value: None,
                    source: eff.as_ref().map(|e| e.source.clone()),
                    is_secret: eff.as_ref().is_some_and(|e| e.is_secret),
                    error: Some(err.to_string()),
                },
            }
        })
        .collect()
}

fn compute_deterministic_idempotency_key(
    request_id: &str,
    method: HttpMethod,
    resolved_url: &str,
    resolved_body: &RequestBody,
    nonce: u64,
) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(request_id.as_bytes());
    hasher.update(b"|");
    hasher.update(method.as_str().as_bytes());
    hasher.update(b"|");
    hasher.update(resolved_url.as_bytes());
    hasher.update(b"|");
    hasher.update(nonce.to_le_bytes());
    hasher.update(b"|");
    match resolved_body {
        RequestBody::None => {}
        RequestBody::Json(s) | RequestBody::Text(s) | RequestBody::Xml(s) => {
            hasher.update(s.as_bytes());
        }
        RequestBody::GraphQl {
            query,
            variables,
            operation_name,
        } => {
            hasher.update(query.as_bytes());
            hasher.update(variables.as_bytes());
            if let Some(op) = operation_name {
                hasher.update(op.as_bytes());
            }
        }
        RequestBody::FormUrlEncoded(rows) => {
            for r in rows {
                hasher.update(r.key.as_bytes());
                hasher.update(b"=");
                hasher.update(r.value.as_bytes());
                hasher.update(b"&");
            }
        }
        RequestBody::Multipart(fields) => {
            for f in fields {
                hasher.update(f.name.as_bytes());
            }
        }
        RequestBody::Binary { local_path, .. } => {
            if let Some(p) = local_path {
                hasher.update(p.as_bytes());
            }
        }
    }
    let digest = hex::encode(hasher.finalize());
    format!("swg-idem-{}", &digest[..24])
}

fn append_query_pairs(base: &str, pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return base.to_owned();
    }
    let mut encoded = Vec::with_capacity(pairs.len());
    for (k, v) in pairs {
        let ek: String = url::form_urlencoded::byte_serialize(k.as_bytes()).collect();
        let ev: String = url::form_urlencoded::byte_serialize(v.as_bytes()).collect();
        encoded.push(format!("{ek}={ev}"));
    }
    let sep = if base.contains('?') { '&' } else { '?' };
    format!("{base}{sep}{}", encoded.join("&"))
}

/// Infers query string filter / pagination parameter suggestions from a JSON response payload
/// (e.g. MockAPI / REST collection endpoints returning `[ { "id": "1", "name": "...", ... } ]`).
pub fn infer_query_param_suggestions(json_bytes: &[u8]) -> Vec<(String, String)> {
    let Ok(val) = serde_json::from_slice::<serde_json::Value>(json_bytes) else {
        return Vec::new();
    };
    let mut suggestions = Vec::new();
    let mut push_unique = |k: &str, v: &str| {
        if !k.is_empty() && !suggestions.iter().any(|(ek, _)| ek == k) {
            suggestions.push((k.to_owned(), v.to_owned()));
        }
    };

    match &val {
        serde_json::Value::Array(items) => {
            push_unique("page", "1");
            push_unique("limit", "10");
            push_unique("search", "");
            if let Some(serde_json::Value::Object(first_obj)) = items.first() {
                if let Some(first_key) = first_obj.keys().next() {
                    push_unique("sortBy", first_key);
                    push_unique("order", "asc");
                }
                for (k, v) in first_obj.iter().take(6) {
                    let sample = match v {
                        serde_json::Value::String(s) if s.len() <= 24 && !s.starts_with("http") => {
                            s.clone()
                        }
                        serde_json::Value::Bool(b) => b.to_string(),
                        serde_json::Value::Number(n) => n.to_string(),
                        _ => String::new(),
                    };
                    push_unique(k, &sample);
                }
            }
        }
        serde_json::Value::Object(map) => {
            let list_candidate = map
                .get("data")
                .or_else(|| map.get("items"))
                .or_else(|| map.get("results"));
            if let Some(serde_json::Value::Array(items)) = list_candidate {
                push_unique("page", "1");
                push_unique("limit", "10");
                push_unique("search", "");
                if let Some(serde_json::Value::Object(first_obj)) = items.first() {
                    for (k, _) in first_obj.iter().take(5) {
                        push_unique(k, "");
                    }
                }
            } else {
                for (k, _) in map.iter().take(5) {
                    push_unique(k, "");
                }
            }
        }
        _ => {}
    }
    suggestions
}

/// Splits a raw or templated URL into `(base_without_query_or_fragment, query, fragment)`,
/// ignoring `?` and `#` characters inside `{{...}}` template tokens.
pub fn split_url_parts(url: &str) -> (&str, Option<&str>, Option<&str>) {
    let bytes = url.as_bytes();
    let mut template_depth = 0usize;
    let mut query_pos = None;
    let mut frag_pos = None;
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            template_depth += 1;
            i += 2;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'}' && bytes[i + 1] == b'}' {
            template_depth = template_depth.saturating_sub(1);
            i += 2;
            continue;
        }
        if template_depth == 0 {
            if bytes[i] == b'#' {
                frag_pos = Some(i);
                break;
            }
            if bytes[i] == b'?' && query_pos.is_none() {
                query_pos = Some(i);
            }
        }
        i += 1;
    }

    match (query_pos, frag_pos) {
        (Some(q), Some(f)) => (&url[..q], Some(&url[q + 1..f]), Some(&url[f + 1..])),
        (Some(q), None) => (&url[..q], Some(&url[q + 1..]), None),
        (None, Some(f)) => (&url[..f], None, Some(&url[f + 1..])),
        (None, None) => (url, None, None),
    }
}

/// Extracts ordered `KeyValueRow` query parameters from a URL's `?query` section.
pub fn extract_url_query_rows(url: &str) -> Vec<KeyValueRow> {
    let (_, Some(query), _) = split_url_parts(url) else {
        return Vec::new();
    };
    if query.is_empty() {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for segment in split_outside_templates(query, b'&') {
        if segment.is_empty() {
            continue;
        }
        let (raw_k, raw_v) = split_once_outside_templates(segment, b'=').unwrap_or((segment, ""));
        let key = decode_query_component_preserving_templates(raw_k);
        let val = decode_query_component_preserving_templates(raw_v);
        if !key.is_empty() || !val.is_empty() {
            rows.push(KeyValueRow::new(key, val));
        }
    }
    rows
}

/// Extracts ordered unique path parameter names (`:param` and `{param}`) from a URL,
/// skipping `{{env_var}}` double-brace templates, URL schemes (`https:`), and host `:port`.
pub fn extract_url_path_param_names(url: &str) -> Vec<String> {
    let (base, _, _) = split_url_parts(url);
    if base.is_empty() {
        return Vec::new();
    }
    let (authority_prefix, path_part) = split_authority_and_path(base);
    let mut out = Vec::new();

    // Allow single-brace `{tenant}` in authority if used, while never matching `{{var}}` or `:port`.
    collect_single_brace_params(authority_prefix, &mut out);

    // Scan path segments left-to-right so `:id` and `{orderId}` appear in exact URL order.
    for seg in path_part.split('/') {
        if seg.is_empty() {
            continue;
        }
        if let Some(after_colon) = seg.strip_prefix(':') {
            let ident: String = after_colon
                .chars()
                .take_while(|c| is_param_ident_char(*c))
                .collect();
            if is_valid_colon_param_name(&ident) && !out.contains(&ident) {
                out.push(ident);
            }
        }
        collect_single_brace_params(seg, &mut out);
    }
    out
}

/// Replaces `:old_key` or `{old_key}` with `:new_key` / `{new_key}` inside `url` when a user
/// renames a path parameter key in the Path parameters table.
pub fn rename_url_path_param_key(url: &str, old_key: &str, new_key: &str) -> String {
    if old_key.is_empty() || new_key.is_empty() || old_key == new_key {
        return url.to_owned();
    }
    let (base, query, frag) = split_url_parts(url);
    let (authority, path_part) = split_authority_and_path(base);
    let new_auth = replace_single_brace_token(authority, old_key, &format!("{{{new_key}}}"));
    let new_path = replace_colon_and_brace_tokens(
        path_part,
        old_key,
        &format!(":{new_key}"),
        &format!("{{{new_key}}}"),
    );
    let mut rebuilt = format!("{new_auth}{new_path}");
    if let Some(q) = query {
        rebuilt.push('?');
        rebuilt.push_str(q);
    }
    if let Some(f) = frag {
        rebuilt.push('#');
        rebuilt.push_str(f);
    }
    rebuilt
}

/// Removes `:removed_key` or `{removed_key}` from `url` when a user deletes a row
/// in the Path parameters table.
pub fn remove_url_path_param_key(url: &str, removed_key: &str) -> String {
    if removed_key.is_empty() {
        return url.to_owned();
    }
    let (base, query, frag) = split_url_parts(url);
    let (authority, path_part) = split_authority_and_path(base);
    let new_auth = replace_single_brace_token(authority, removed_key, "");
    let mut kept_segments = Vec::new();
    for (idx, seg) in path_part.split('/').enumerate() {
        if idx == 0 && seg.is_empty() {
            kept_segments.push(String::new());
            continue;
        }
        if let Some(after_colon) = seg.strip_prefix(':') {
            let ident_len = after_colon
                .chars()
                .take_while(|c| is_param_ident_char(*c))
                .map(char::len_utf8)
                .sum::<usize>();
            if &after_colon[..ident_len] == removed_key && ident_len == after_colon.len() {
                continue;
            }
        }
        if seg == format!("{{{removed_key}}}") {
            continue;
        }
        let cleaned = replace_colon_and_brace_tokens(seg, removed_key, "", "");
        kept_segments.push(cleaned);
    }
    let new_path = kept_segments.join("/");
    let mut rebuilt = format!("{new_auth}{new_path}");
    if let Some(q) = query {
        rebuilt.push('?');
        rebuilt.push_str(q);
    }
    if let Some(f) = frag {
        rebuilt.push('#');
        rebuilt.push_str(f);
    }
    rebuilt
}

fn split_authority_and_path(base: &str) -> (&str, &str) {
    if let Some(scheme_sep) = base.find("://") {
        let after_scheme = &base[scheme_sep + 3..];
        if let Some(slash_offset) = after_scheme.find('/') {
            let split_at = scheme_sep + 3 + slash_offset;
            (&base[..split_at], &base[split_at..])
        } else {
            (base, "")
        }
    } else if base.starts_with('/') {
        ("", base)
    } else if let Some(slash_at) = base.find('/') {
        (&base[..slash_at], &base[slash_at..])
    } else {
        (base, "")
    }
}

fn is_param_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn is_valid_colon_param_name(name: &str) -> bool {
    let Some(first) = name.chars().next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_') && name.chars().all(is_param_ident_char)
}

fn collect_single_brace_params(input: &str, out: &mut Vec<String>) {
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                // Skip `{{...}}` environment/collection variable template completely.
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'}' && bytes[i + 1] == b'}') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                continue;
            }
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end] != b'}' && bytes[end] != b'{' {
                end += 1;
            }
            if end < bytes.len()
                && bytes[end] == b'}'
                && !(end + 1 < bytes.len() && bytes[end + 1] == b'}')
            {
                let candidate = input[start..end].trim();
                if !candidate.is_empty()
                    && candidate.chars().all(is_param_ident_char)
                    && !out.iter().any(|existing| existing == candidate)
                {
                    out.push(candidate.to_owned());
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
}

fn encode_path_segment_value(val: &str) -> String {
    let mut out = String::with_capacity(val.len());
    for b in val.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn replace_path_param_tokens(url: &str, key: &str, encoded_val: &str) -> String {
    let (base, query, frag) = split_url_parts(url);
    let (authority, path_part) = split_authority_and_path(base);
    let new_auth = replace_single_brace_token(authority, key, encoded_val);
    let new_path = replace_colon_and_brace_tokens(path_part, key, encoded_val, encoded_val);
    let mut out = format!("{new_auth}{new_path}");
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = frag {
        out.push('#');
        out.push_str(f);
    }
    out
}

fn replace_single_brace_token(input: &str, key: &str, replacement: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut last = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'}' && bytes[i + 1] == b'}') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                continue;
            }
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end] != b'}' && bytes[end] != b'{' {
                end += 1;
            }
            if end < bytes.len()
                && bytes[end] == b'}'
                && !(end + 1 < bytes.len() && bytes[end + 1] == b'}')
            {
                if input[start..end].trim() == key {
                    out.push_str(&input[last..i]);
                    out.push_str(replacement);
                    last = end + 1;
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&input[last..]);
    out
}

fn replace_colon_and_brace_tokens(
    path_part: &str,
    key: &str,
    colon_replacement: &str,
    brace_replacement: &str,
) -> String {
    let with_braces = replace_single_brace_token(path_part, key, brace_replacement);
    if path_part.is_empty() {
        return with_braces;
    }
    let mut seg_out = Vec::new();
    for seg in with_braces.split('/') {
        if let Some(after_colon) = seg.strip_prefix(':') {
            let ident_len = after_colon
                .chars()
                .take_while(|c| is_param_ident_char(*c))
                .map(char::len_utf8)
                .sum::<usize>();
            if &after_colon[..ident_len] == key {
                seg_out.push(format!("{colon_replacement}{}", &after_colon[ident_len..]));
                continue;
            }
        }
        seg_out.push(seg.to_owned());
    }
    seg_out.join("/")
}

fn split_outside_templates(input: &str, sep: u8) -> Vec<&str> {
    let bytes = input.as_bytes();
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            depth += 1;
            i += 2;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'}' && bytes[i + 1] == b'}' {
            depth = depth.saturating_sub(1);
            i += 2;
            continue;
        }
        if depth == 0 && bytes[i] == sep {
            parts.push(&input[start..i]);
            start = i + 1;
        }
        i += 1;
    }
    parts.push(&input[start..]);
    parts
}

fn split_once_outside_templates(input: &str, sep: u8) -> Option<(&str, &str)> {
    let bytes = input.as_bytes();
    let mut depth = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            depth += 1;
            i += 2;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'}' && bytes[i + 1] == b'}' {
            depth = depth.saturating_sub(1);
            i += 2;
            continue;
        }
        if depth == 0 && bytes[i] == sep {
            return Some((&input[..i], &input[i + 1..]));
        }
        i += 1;
    }
    None
}

fn decode_query_component_preserving_templates(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find("{{") {
        out.push_str(&percent_decode_component(&rest[..start]));
        let after = &rest[start..];
        if let Some(end) = after.find("}}") {
            out.push_str(&after[..end + 2]);
            rest = &after[end + 2..];
        } else {
            out.push_str(after);
            return out;
        }
    }
    if !rest.is_empty() {
        out.push_str(&percent_decode_component(rest));
    }
    out
}

fn percent_decode_component(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            decoded.push(b' ');
            i += 1;
        } else if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(val) = u8::from_str_radix(hex, 16)
        {
            decoded.push(val);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn encode_query_component_preserving_templates(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find("{{") {
        encode_plain_query_slice(&rest[..start], &mut out);
        let after = &rest[start..];
        if let Some(end) = after.find("}}") {
            out.push_str(&after[..end + 2]);
            rest = &after[end + 2..];
        } else {
            encode_plain_query_slice(after, &mut out);
            return out;
        }
    }
    encode_plain_query_slice(rest, &mut out);
    out
}

fn encode_plain_query_slice(slice: &str, out: &mut String) {
    for b in slice.bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'_' | b'.' | b'~' | b':' | b'/' | b'@' | b',')
        {
            out.push(b as char);
        } else if b == b' ' {
            out.push_str("%20");
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
}

fn is_valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

fn encode_basic(username: &str, password: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = format!("{username}:{password}").into_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

pub fn build_sanitized_example(
    request_id: &str,
    name: &str,
    status_code: u16,
    headers: &[(String, String)],
    body_preview: &str,
    policy: &RedactionPolicy,
    now_epoch_secs: i64,
) -> SharedExample {
    let (clean_headers, hdr_count) = sanitize_headers(headers, policy);
    let body_preview_clean = sanitize_text(body_preview, policy);
    SharedExample {
        id: new_uid(),
        request_id: request_id.to_owned(),
        name: name.to_owned(),
        status_code,
        headers: clean_headers,
        body_preview: body_preview_clean.sanitized_text,
        sanitized_redactions: hdr_count + body_preview_clean.redacted_count,
        created_epoch_secs: now_epoch_secs,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub version: u32,
    pub meta: WorkspaceMeta,
    pub collections: Vec<Collection>,
    pub folders: Vec<Folder>,
    pub requests: Vec<RequestItem>,
    #[serde(default)]
    pub trashed_collections: Vec<Collection>,
    #[serde(default)]
    pub trashed_folders: Vec<Folder>,
    #[serde(default)]
    pub trashed_requests: Vec<RequestItem>,
    #[serde(default)]
    pub trashed_environments: Vec<Environment>,
    pub environments: Vec<Environment>,
    pub active_environment_id: Option<String>,
    pub workspace_vars: Vec<VariableEntry>,
    pub cookies: Vec<CookieRecord>,
    pub examples: Vec<SharedExample>,
    pub open_tabs: Vec<RequestTab>,
    pub active_tab_id: Option<String>,
    pub layout: LayoutPreferences,
    pub history_retention: usize,
    pub revision: u64,
}

impl WorkspaceState {
    pub fn new_personal(id: LocalId, name: impl Into<String>) -> Self {
        let mut col = Collection {
            id: new_uid(),
            name: "Getting Started".into(),
            description: "Default offline personal collection".into(),
            color: None,
            auth: AuthConfig::Inherit,
            variables: Vec::new(),
            pre_request_script: String::new(),
            post_response_script: String::new(),
            order_key: 0,
        };
        let req = RequestItem::new(
            col.id.clone(),
            "Health check",
            HttpMethod::Get,
            "{{base_url}}/health/live",
        );
        col.order_key = 1;
        let mut dev = Environment::new("Development", false);
        dev.variables
            .push(VariableEntry::public("base_url", "http://127.0.0.1:8080"));
        let mut staging = Environment::new("Staging", false);
        staging.variables.push(VariableEntry::public(
            "base_url",
            "https://staging.api.example.test",
        ));
        let mut prod = Environment::new("Production", true);
        prod.variables.push(VariableEntry::public(
            "base_url",
            "https://api.example.test",
        ));
        let tab = RequestTab {
            id: new_uid(),
            request_id: req.id.clone(),
            pinned: false,
        };
        let active_tab_id = Some(tab.id.clone());
        let active_env_id = Some(dev.id.clone());
        Self {
            version: 1,
            meta: WorkspaceMeta {
                id,
                name: name.into(),
                description: "Personal offline API workspace".into(),
                kind: WorkspaceKind::Personal,
                archived: false,
                defaults: ProjectDefaults::default(),
            },
            collections: vec![col],
            folders: Vec::new(),
            requests: vec![req],
            trashed_collections: Vec::new(),
            trashed_folders: Vec::new(),
            trashed_requests: Vec::new(),
            trashed_environments: Vec::new(),
            environments: vec![dev, staging, prod],
            active_environment_id: active_env_id,
            workspace_vars: Vec::new(),
            cookies: Vec::new(),
            examples: Vec::new(),
            open_tabs: vec![tab],
            active_tab_id,
            layout: LayoutPreferences::default(),
            history_retention: DEFAULT_HISTORY_RETENTION,
            revision: 0,
        }
    }

    pub fn active_environment(&self) -> Option<&Environment> {
        let id = self.active_environment_id.as_deref()?;
        self.environments.iter().find(|e| e.id == id)
    }

    pub fn search_requests(
        &self,
        query: &str,
        method_filter: Option<HttpMethod>,
        tag_filter: Option<&str>,
    ) -> Vec<&RequestItem> {
        let q = query.trim().to_ascii_lowercase();
        let tag = tag_filter
            .map(|t| t.trim().to_ascii_lowercase())
            .filter(|t| !t.is_empty());
        self.requests
            .iter()
            .filter(|r| {
                if let Some(m) = method_filter
                    && r.method != m
                {
                    return false;
                }
                if let Some(ref tg) = tag
                    && !r.tags.iter().any(|t| t.to_ascii_lowercase() == *tg)
                {
                    return false;
                }
                if q.is_empty() {
                    return true;
                }
                r.name.to_ascii_lowercase().contains(&q)
                    || r.url.to_ascii_lowercase().contains(&q)
                    || r.description.to_ascii_lowercase().contains(&q)
                    || r.method.as_str().to_ascii_lowercase().contains(&q)
                    || r.tags.iter().any(|t| t.to_ascii_lowercase().contains(&q))
            })
            .collect()
    }

    pub fn duplicate_request(&mut self, request_id: &str) -> Option<String> {
        let orig = self.requests.iter().find(|r| r.id == request_id)?.clone();
        let mut copy = orig;
        copy.id = new_uid();
        copy.name = format!("{} (copy)", copy.name);
        copy.shared_revision = 0;
        copy.dirty_vs_shared = false;
        copy.order_key = self.requests.len() as u64;
        let new_id = copy.id.clone();
        self.requests.push(copy);
        Some(new_id)
    }

    pub fn move_request(
        &mut self,
        request_id: &str,
        target_collection_id: &str,
        target_folder_id: Option<String>,
    ) -> bool {
        if !self
            .collections
            .iter()
            .any(|c| c.id == target_collection_id)
        {
            return false;
        }
        if let Some(ref fid) = target_folder_id
            && !self
                .folders
                .iter()
                .any(|f| f.id == *fid && f.collection_id == target_collection_id)
        {
            return false;
        }
        if let Some(req) = self.requests.iter_mut().find(|r| r.id == request_id) {
            req.collection_id = target_collection_id.to_owned();
            req.folder_id = target_folder_id;
            true
        } else {
            false
        }
    }

    pub fn delete_request(&mut self, request_id: &str) -> bool {
        let Some(pos) = self.requests.iter().position(|r| r.id == request_id) else {
            return false;
        };
        let removed = self.requests.remove(pos);
        if self.trashed_requests.len() >= 100 {
            self.trashed_requests.remove(0);
        }
        self.trashed_requests.push(removed);
        self.open_tabs.retain(|t| t.request_id != request_id);
        if let Some(ref active) = self.active_tab_id
            && !self.open_tabs.iter().any(|t| &t.id == active)
        {
            self.active_tab_id = self.open_tabs.first().map(|t| t.id.clone());
        }
        true
    }

    pub fn restore_trashed_request(&mut self, request_id: &str) -> Option<String> {
        let pos = self
            .trashed_requests
            .iter()
            .position(|r| r.id == request_id)?;
        let mut restored = self.trashed_requests.remove(pos);
        if !self
            .collections
            .iter()
            .any(|c| c.id == restored.collection_id)
        {
            if let Some(first_col) = self.collections.first() {
                restored.collection_id = first_col.id.clone();
                restored.folder_id = None;
            }
        } else if let Some(ref fid) = restored.folder_id
            && !self.folders.iter().any(|f| f.id == *fid)
        {
            restored.folder_id = None;
        }
        let id = restored.id.clone();
        self.requests.push(restored);
        Some(id)
    }

    pub fn purge_trashed_request(&mut self, request_id: &str) -> bool {
        let before = self.trashed_requests.len();
        self.trashed_requests.retain(|r| r.id != request_id);
        self.trashed_requests.len() != before
    }

    pub fn empty_trash(&mut self) {
        self.trashed_collections.clear();
        self.trashed_folders.clear();
        self.trashed_requests.clear();
        self.trashed_environments.clear();
    }

    pub fn reorder_request(&mut self, request_id: &str, move_up: bool) -> bool {
        let Some(idx) = self.requests.iter().position(|r| r.id == request_id) else {
            return false;
        };
        let col_id = self.requests[idx].collection_id.clone();
        let folder_id = self.requests[idx].folder_id.clone();
        let peer_indices: Vec<usize> = self
            .requests
            .iter()
            .enumerate()
            .filter(|(_, r)| r.collection_id == col_id && r.folder_id == folder_id)
            .map(|(i, _)| i)
            .collect();
        let Some(pos_in_peers) = peer_indices.iter().position(|&i| i == idx) else {
            return false;
        };
        let swap_idx = if move_up {
            if pos_in_peers == 0 {
                return false;
            }
            peer_indices[pos_in_peers - 1]
        } else {
            if pos_in_peers + 1 >= peer_indices.len() {
                return false;
            }
            peer_indices[pos_in_peers + 1]
        };
        self.requests.swap(idx, swap_idx);
        for (k, r) in self.requests.iter_mut().enumerate() {
            r.order_key = k as u64;
        }
        true
    }

    pub fn delete_folder(&mut self, folder_id: &str) -> bool {
        if !self.folders.iter().any(|f| f.id == folder_id) {
            return false;
        }
        let mut to_remove = vec![folder_id.to_owned()];
        let mut idx = 0usize;
        while idx < to_remove.len() {
            let parent = to_remove[idx].clone();
            for child in &self.folders {
                if child.parent_folder_id.as_deref() == Some(parent.as_str()) {
                    to_remove.push(child.id.clone());
                }
            }
            idx += 1;
        }
        let mut kept_folders = Vec::with_capacity(self.folders.len());
        for f in std::mem::take(&mut self.folders) {
            if to_remove.contains(&f.id) {
                if self.trashed_folders.len() >= 100 {
                    self.trashed_folders.remove(0);
                }
                self.trashed_folders.push(f);
            } else {
                kept_folders.push(f);
            }
        }
        self.folders = kept_folders;
        let mut removed_req_ids = BTreeSet::new();
        let mut kept_reqs = Vec::with_capacity(self.requests.len());
        for r in std::mem::take(&mut self.requests) {
            if r.folder_id
                .as_ref()
                .is_some_and(|fid| to_remove.contains(fid))
            {
                removed_req_ids.insert(r.id.clone());
                if self.trashed_requests.len() >= 100 {
                    self.trashed_requests.remove(0);
                }
                self.trashed_requests.push(r);
            } else {
                kept_reqs.push(r);
            }
        }
        self.requests = kept_reqs;
        self.open_tabs
            .retain(|t| !removed_req_ids.contains(&t.request_id));
        if let Some(ref active) = self.active_tab_id
            && !self.open_tabs.iter().any(|t| &t.id == active)
        {
            self.active_tab_id = self.open_tabs.first().map(|t| t.id.clone());
        }
        true
    }

    pub fn restore_trashed_folder(&mut self, folder_id: &str) -> bool {
        let Some(pos) = self.trashed_folders.iter().position(|f| f.id == folder_id) else {
            return false;
        };
        let mut restored = self.trashed_folders.remove(pos);
        if !self
            .collections
            .iter()
            .any(|c| c.id == restored.collection_id)
            && let Some(first) = self.collections.first()
        {
            restored.collection_id = first.id.clone();
            restored.parent_folder_id = None;
        }
        self.folders.push(restored);
        let mut restored_reqs = Vec::new();
        let mut kept_trash = Vec::with_capacity(self.trashed_requests.len());
        for r in std::mem::take(&mut self.trashed_requests) {
            if r.folder_id.as_deref() == Some(folder_id) {
                restored_reqs.push(r);
            } else {
                kept_trash.push(r);
            }
        }
        self.trashed_requests = kept_trash;
        self.requests.extend(restored_reqs);
        true
    }

    pub fn delete_collection(&mut self, collection_id: &str) -> bool {
        if self.collections.len() <= 1 {
            return false;
        }
        let Some(pos) = self.collections.iter().position(|c| c.id == collection_id) else {
            return false;
        };
        let removed_col = self.collections.remove(pos);
        if self.trashed_collections.len() >= 50 {
            self.trashed_collections.remove(0);
        }
        self.trashed_collections.push(removed_col);

        let mut kept_folders = Vec::with_capacity(self.folders.len());
        for f in std::mem::take(&mut self.folders) {
            if f.collection_id == collection_id {
                if self.trashed_folders.len() >= 100 {
                    self.trashed_folders.remove(0);
                }
                self.trashed_folders.push(f);
            } else {
                kept_folders.push(f);
            }
        }
        self.folders = kept_folders;

        let mut removed_reqs = BTreeSet::new();
        let mut kept_reqs = Vec::with_capacity(self.requests.len());
        for r in std::mem::take(&mut self.requests) {
            if r.collection_id == collection_id {
                removed_reqs.insert(r.id.clone());
                if self.trashed_requests.len() >= 100 {
                    self.trashed_requests.remove(0);
                }
                self.trashed_requests.push(r);
            } else {
                kept_reqs.push(r);
            }
        }
        self.requests = kept_reqs;
        self.open_tabs
            .retain(|t| !removed_reqs.contains(&t.request_id));
        if let Some(ref active) = self.active_tab_id
            && !self.open_tabs.iter().any(|t| &t.id == active)
        {
            self.active_tab_id = self.open_tabs.first().map(|t| t.id.clone());
        }
        true
    }

    pub fn restore_trashed_collection(&mut self, collection_id: &str) -> bool {
        let Some(pos) = self
            .trashed_collections
            .iter()
            .position(|c| c.id == collection_id)
        else {
            return false;
        };
        let restored_col = self.trashed_collections.remove(pos);
        self.collections.push(restored_col);

        let mut kept_f = Vec::with_capacity(self.trashed_folders.len());
        for f in std::mem::take(&mut self.trashed_folders) {
            if f.collection_id == collection_id {
                self.folders.push(f);
            } else {
                kept_f.push(f);
            }
        }
        self.trashed_folders = kept_f;

        let mut kept_r = Vec::with_capacity(self.trashed_requests.len());
        for r in std::mem::take(&mut self.trashed_requests) {
            if r.collection_id == collection_id {
                self.requests.push(r);
            } else {
                kept_r.push(r);
            }
        }
        self.trashed_requests = kept_r;
        true
    }

    pub fn delete_environment(&mut self, env_id: &str) -> bool {
        let Some(pos) = self.environments.iter().position(|e| e.id == env_id) else {
            return false;
        };
        let removed = self.environments.remove(pos);
        if self.trashed_environments.len() >= 50 {
            self.trashed_environments.remove(0);
        }
        self.trashed_environments.push(removed);
        if self.active_environment_id.as_deref() == Some(env_id) {
            self.active_environment_id = self.environments.first().map(|e| e.id.clone());
        }
        true
    }

    pub fn restore_trashed_environment(&mut self, env_id: &str) -> bool {
        let Some(pos) = self
            .trashed_environments
            .iter()
            .position(|e| e.id == env_id)
        else {
            return false;
        };
        let restored = self.trashed_environments.remove(pos);
        self.environments.push(restored);
        true
    }

    pub fn fork_collection(&mut self, collection_id: &str) -> Option<String> {
        let orig_col = self
            .collections
            .iter()
            .find(|c| c.id == collection_id)?
            .clone();
        let new_col_id = new_uid();
        let mut forked_col = orig_col;
        forked_col.id = new_col_id.clone();
        forked_col.name = format!("{} (fork)", forked_col.name);
        forked_col.order_key = self.collections.len() as u64;
        self.collections.push(forked_col);

        let mut folder_map = BTreeMap::new();
        let orig_folders: Vec<Folder> = self
            .folders
            .iter()
            .filter(|f| f.collection_id == collection_id)
            .cloned()
            .collect();
        for f in &orig_folders {
            folder_map.insert(f.id.clone(), new_uid());
        }
        for mut f in orig_folders {
            f.id = folder_map[&f.id].clone();
            f.collection_id = new_col_id.clone();
            f.parent_folder_id = f.parent_folder_id.and_then(|p| folder_map.get(&p).cloned());
            self.folders.push(f);
        }
        let orig_reqs: Vec<RequestItem> = self
            .requests
            .iter()
            .filter(|r| r.collection_id == collection_id)
            .cloned()
            .collect();
        for mut r in orig_reqs {
            r.id = new_uid();
            r.collection_id = new_col_id.clone();
            r.folder_id = r.folder_id.and_then(|fid| folder_map.get(&fid).cloned());
            r.shared_revision = 0;
            self.requests.push(r);
        }
        Some(new_col_id)
    }

    pub fn open_or_focus_tab(&mut self, request_id: &str) -> String {
        if let Some(existing) = self.open_tabs.iter().find(|t| t.request_id == request_id) {
            let id = existing.id.clone();
            self.active_tab_id = Some(id.clone());
            return id;
        }
        let tab = RequestTab {
            id: new_uid(),
            request_id: request_id.to_owned(),
            pinned: false,
        };
        let id = tab.id.clone();
        self.open_tabs.push(tab);
        self.active_tab_id = Some(id.clone());
        id
    }

    pub fn close_unpinned_tabs(&mut self) {
        self.open_tabs.retain(|t| t.pinned);
        if let Some(ref active) = self.active_tab_id
            && !self.open_tabs.iter().any(|t| &t.id == active)
        {
            self.active_tab_id = self.open_tabs.first().map(|t| t.id.clone());
        }
    }

    /// Exports a sanitized workspace snapshot with personal secrets and local file paths stripped (EV06 / RQ05 / SC02).
    pub fn export_sanitized_json(
        &self,
        policy: &RedactionPolicy,
    ) -> Result<String, serde_json::Error> {
        let mut clone = self.clone();
        for env in &mut clone.environments {
            env.personal_overrides.clear();
            for v in &mut env.variables {
                if v.is_secret {
                    v.shared_value = format!("{{{{{}}}}}", v.key);
                }
            }
        }
        for v in &mut clone.workspace_vars {
            if v.is_secret {
                v.shared_value = format!("{{{{{}}}}}", v.key);
            }
        }
        clone.cookies.clear();
        for req in &mut clone.requests {
            let (clean_url, _) = sanitize_url_query(&req.url, policy);
            req.url = clean_url;
            for h in &mut req.headers {
                if policy.is_sensitive_header(&h.key) {
                    h.value = "{{SECRET_REDACTED}}".into();
                }
            }
            if let RequestBody::Multipart(fields) = &mut req.body {
                for f in fields {
                    if let MultipartValueKind::File { local_path, .. } = &mut f.value {
                        *local_path = String::new();
                    }
                }
            }
            if let RequestBody::Binary { local_path, .. } = &mut req.body {
                *local_path = None;
            }
        }
        serde_json::to_string_pretty(&clone)
    }

    pub fn all_tags(&self) -> Vec<String> {
        let mut set = BTreeSet::new();
        for req in &self.requests {
            for t in &req.tags {
                if !t.trim().is_empty() {
                    set.insert(t.clone());
                }
            }
        }
        set.into_iter().collect()
    }
}

pub fn folder_chain<'a>(folders: &'a [Folder], start_folder_id: Option<&str>) -> Vec<&'a Folder> {
    let mut chain = Vec::new();
    let mut current = start_folder_id;
    let mut depth = 0usize;
    while let Some(fid) = current {
        if depth >= 32 {
            break;
        }
        if let Some(f) = folders.iter().find(|f| f.id == fid) {
            chain.push(f);
            current = f.parent_folder_id.as_deref();
            depth += 1;
        } else {
            break;
        }
    }
    chain.reverse();
    chain
}

/// Detects an unclosed `{` or `{{` variable token immediately before `cursor_char_idx`,
/// returning `(open_brace_char_idx, typed_prefix)` for editor autocomplete.
pub fn extract_brace_var_trigger(text: &str, cursor_char_idx: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let cur = cursor_char_idx.min(chars.len());
    let mut i = cur;
    while i > 0 {
        i -= 1;
        let ch = chars[i];
        if ch == '}'
            || ch.is_whitespace()
            || matches!(
                ch,
                '?' | '&' | '/' | '=' | '#' | '"' | '\'' | ',' | ';' | '(' | ')' | '[' | ']'
            )
        {
            return None;
        }
        if ch == '{' {
            let open_idx = if i > 0 && chars[i - 1] == '{' {
                i - 1
            } else {
                i
            };
            let prefix_start = if open_idx + 1 < cur && chars[open_idx + 1] == '{' {
                open_idx + 2
            } else {
                open_idx + 1
            };
            let prefix: String = chars[prefix_start..cur].iter().collect();
            return Some((open_idx, prefix));
        }
    }
    None
}

/// Replaces `{prefix` or `{{prefix` (plus any immediately adjacent closing braces) with `{{var_name}}`
/// and returns `(updated_text, new_cursor_char_idx)`.
pub fn apply_brace_var_completion(
    text: &str,
    open_char_idx: usize,
    cursor_char_idx: usize,
    var_name: &str,
) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let start = open_char_idx.min(chars.len());
    let mut end = cursor_char_idx.clamp(start, chars.len());
    let mut closed = 0usize;
    while end < chars.len() && closed < 2 && chars[end] == '}' {
        end += 1;
        closed += 1;
    }
    let insertion = format!("{{{{{var_name}}}}}");
    let new_cursor = start + insertion.chars().count();
    let mut out = String::with_capacity(text.len() + insertion.len());
    out.extend(chars[..start].iter());
    out.push_str(&insertion);
    out.extend(chars[end..].iter());
    (out, new_cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_precedence_circular_and_missing_detection_work() {
        let mut env = Environment::new("Staging", false);
        env.variables
            .push(VariableEntry::public("host", "staging.example.test"));
        env.variables.push(VariableEntry::public("a", "{{b}}"));
        env.variables.push(VariableEntry::public("b", "{{a}}"));
        let mut secrets = BTreeMap::new();
        let tok_ref = LocalId::parse("api-token").unwrap();
        secrets.insert(tok_ref.clone(), "real-secret-999".into());
        env.personal_overrides.insert(
            "token".into(),
            PersonalVarOverride {
                value: String::new(),
                is_secret: true,
                secret_ref: Some(tok_ref),
            },
        );
        let ws_vars = vec![VariableEntry::public("host", "ws.example.test")];
        let col_vars = vec![VariableEntry::public("host", "col.example.test")];
        let req_vars = vec![VariableEntry::public("path", "items")];
        let ctx = VariableContext {
            workspace_vars: &ws_vars,
            collection_vars: &col_vars,
            environment: Some(&env),
            request_vars: &req_vars,
            local_secrets: &secrets,
        };
        assert_eq!(ctx.inspect("host").unwrap().value, "staging.example.test");
        assert!(ctx.inspect("host").unwrap().source.contains("Environment"));
        assert_eq!(ctx.inspect("path").unwrap().source, "Request");
        assert_eq!(
            ctx.substitute("https://{{host}}/{{path}}?k={{token}}", true)
                .unwrap(),
            "https://staging.example.test/items?k=••••••••"
        );
        assert_eq!(
            ctx.substitute("https://{{host}}/{{path}}?k={{token}}", false)
                .unwrap(),
            "https://staging.example.test/items?k=real-secret-999"
        );
        assert!(matches!(
            ctx.substitute("{{missing_var}}", false),
            Err(VariableError::Missing(_))
        ));
        assert!(matches!(
            ctx.substitute("{{a}}", false),
            Err(VariableError::Circular(_))
        ));
    }

    #[test]
    fn auth_inheritance_and_production_confirmation_work() {
        let col = Collection {
            id: "c1".into(),
            name: "Core".into(),
            description: String::new(),
            color: None,
            auth: AuthConfig::Bearer {
                token: "{{token}}".into(),
            },
            variables: Vec::new(),
            pre_request_script: String::new(),
            post_response_script: String::new(),
            order_key: 0,
        };
        let folder = Folder {
            id: "f1".into(),
            collection_id: "c1".into(),
            parent_folder_id: None,
            name: "Admin".into(),
            description: String::new(),
            auth: AuthConfig::Inherit,
            pre_request_script: String::new(),
            post_response_script: String::new(),
            order_key: 0,
        };
        let mut req = RequestItem::new(
            "c1",
            "Create task",
            HttpMethod::Post,
            "https://api.example.test/tasks",
        );
        req.folder_id = Some("f1".into());
        let prod = Environment::new("Production", true);
        let secrets = BTreeMap::from([(LocalId::parse("tok").unwrap(), "abc".into())]);
        let mut env = prod;
        env.variables.push(VariableEntry::secret(
            "token",
            LocalId::parse("tok").unwrap(),
        ));
        let ctx = VariableContext {
            workspace_vars: &[],
            collection_vars: &[],
            environment: Some(&env),
            request_vars: &[],
            local_secrets: &secrets,
        };
        let resolved = resolve_request_preview(&req, Some(&col), &[&folder], &ctx, &[], 100);
        assert!(resolved.requires_production_confirm);
        assert_eq!(resolved.auth_source, "Collection (Core)");
        assert_eq!(resolved.headers[0].value, "Bearer abc");
        assert_eq!(resolved.masked_headers[0].value, "Bearer ••••••••");
    }

    #[test]
    fn duplicate_query_params_cookie_match_and_export_strip_work() {
        let mut ws = WorkspaceState::new_personal(LocalId::parse("personal").unwrap(), "Personal");
        ws.requests[0].url = "https://api.example.test/items/:id".into();
        ws.requests[0]
            .path_params
            .push(KeyValueRow::new("id", "42"));
        ws.requests[0]
            .query_params
            .push(KeyValueRow::new("tag", "a"));
        ws.requests[0]
            .query_params
            .push(KeyValueRow::new("tag", "b"));
        ws.requests[0]
            .headers
            .push(KeyValueRow::new("Authorization", "Bearer raw-secret"));
        let cookie = CookieRecord::parse_set_cookie(
            "sid=secret-cookie; Path=/; HttpOnly; Secure; Max-Age=3600",
            "api.example.test",
            1000,
        )
        .unwrap();
        ws.cookies.push(cookie);
        let ctx = VariableContext {
            workspace_vars: &[],
            collection_vars: &[],
            environment: None,
            request_vars: &[],
            local_secrets: &BTreeMap::new(),
        };
        let resolved = resolve_request_preview(&ws.requests[0], None, &[], &ctx, &ws.cookies, 1500);
        assert_eq!(
            resolved.url,
            "https://api.example.test/items/42?tag=a&tag=b"
        );
        assert!(
            resolved
                .headers
                .iter()
                .any(|h| h.name == "Cookie" && h.value == "sid=secret-cookie")
        );

        let exported = ws
            .export_sanitized_json(&RedactionPolicy::default())
            .unwrap();
        assert!(!exported.contains("raw-secret"));
        assert!(!exported.contains("secret-cookie"));
    }

    #[test]
    fn url_query_and_path_params_bidirectional_sync_works() {
        let mut req = RequestItem::new(
            "c1",
            "Get order",
            HttpMethod::Get,
            "{{base_url}}:8080/v1/users/:userId/orders/{order_id}?page=1&limit=20&q={{search_term}}",
        );
        // 1. Auto-detected on creation without confusing `{{base_url}}`, `:8080`, or `{{search_term}}`
        assert_eq!(req.path_params.len(), 2);
        assert_eq!(req.path_params[0].key, "userId");
        assert_eq!(req.path_params[1].key, "order_id");
        assert_eq!(req.query_params.len(), 3);
        assert_eq!(req.query_params[0].key, "page");
        assert_eq!(req.query_params[0].value, "1");
        assert_eq!(req.query_params[1].key, "limit");
        assert_eq!(req.query_params[1].value, "20");
        assert_eq!(req.query_params[2].key, "q");
        assert_eq!(req.query_params[2].value, "{{search_term}}");

        // 2. Fill path param values, rename `:userId` -> `:memberId` in URL preserving value!
        req.path_params[0].value = "u-99".into();
        req.path_params[1].value = "ord-7".into();
        req.url =
            "{{base_url}}:8080/v1/users/:memberId/orders/{order_id}?page=2&limit=20&q={{search_term}}"
                .into();
        req.sync_params_from_url();
        assert_eq!(req.path_params[0].key, "memberId");
        assert_eq!(req.path_params[0].value, "u-99");
        assert_eq!(req.query_params[0].value, "2");

        // 3. Disable `limit` in query table -> removed from URL bar while kept in table; no duplicate query on resolve!
        req.query_params[1].enabled = false;
        req.sync_url_from_query_params();
        assert_eq!(
            req.url,
            "{{base_url}}:8080/v1/users/:memberId/orders/{order_id}?page=2&q={{search_term}}"
        );
        assert_eq!(req.query_params.len(), 3);

        let ws_vars = vec![
            VariableEntry::public("base_url", "https://api.example.test"),
            VariableEntry::public("search_term", "shoes"),
        ];
        let ctx = VariableContext {
            workspace_vars: &ws_vars,
            collection_vars: &[],
            environment: None,
            request_vars: &[],
            local_secrets: &BTreeMap::new(),
        };
        let resolved = resolve_request_preview(&req, None, &[], &ctx, &[], 1000);
        assert_eq!(
            resolved.url,
            "https://api.example.test:8080/v1/users/u-99/orders/ord-7?page=2&q=shoes"
        );
    }

    #[test]
    fn graphql_resolution_and_deterministic_idempotency_key_work() {
        let mut req =
            RequestItem::new_graphql("c1", "Create Ws", "https://api.example.test/v1/graphql");
        req.body = RequestBody::GraphQl {
            query: "mutation CreateWs($slug: String!, $name: String!) {\n  createWorkspace(slug: $slug, name: $name) { workspaceId }\n}".into(),
            variables: "{\"slug\":\"team-a\",\"name\":\"Team A\"}".into(),
            operation_name: Some("CreateWs".into()),
        };
        let ctx = VariableContext {
            workspace_vars: &[],
            collection_vars: &[],
            environment: None,
            request_vars: &[],
            local_secrets: &BTreeMap::new(),
        };

        // 1. POST GraphQL mutation builds valid JSON envelope + deterministic Idempotency-Key
        let r1 = resolve_request_preview(&req, None, &[], &ctx, &[], 1000);
        let r2 = resolve_request_preview(&req, None, &[], &ctx, &[], 1005);
        assert!(r1.blocking_errors.is_empty());
        let k1 = r1
            .headers
            .iter()
            .find(|h| h.name == "Idempotency-Key")
            .expect("mutation should inject Idempotency-Key")
            .value
            .clone();
        let k2 = r2
            .headers
            .iter()
            .find(|h| h.name == "Idempotency-Key")
            .unwrap()
            .value
            .clone();
        assert_eq!(
            k1, k2,
            "retries of identical payload keep stable Idempotency-Key"
        );
        assert!(k1.starts_with("swg-idem-"));
        if let RequestBody::Json(ref payload) = r1.body {
            let parsed: serde_json::Value = serde_json::from_str(payload).unwrap();
            assert_eq!(parsed["operationName"], "CreateWs");
            assert_eq!(parsed["variables"]["slug"], "team-a");
        } else {
            panic!("expected resolved JSON envelope for POST GraphQL");
        }

        // 2. Rotating nonce produces a fresh distinct Idempotency-Key
        req.idempotency_nonce += 1;
        let r3 = resolve_request_preview(&req, None, &[], &ctx, &[], 1010);
        let k3 = r3
            .headers
            .iter()
            .find(|h| h.name == "Idempotency-Key")
            .unwrap()
            .value
            .clone();
        assert_ne!(k1, k3);

        // 3. GraphQL mutation over HTTP GET is rejected per GraphQL-over-HTTP spec
        req.method = HttpMethod::Get;
        let r_get_mut = resolve_request_preview(&req, None, &[], &ctx, &[], 1010);
        assert!(
            r_get_mut
                .blocking_errors
                .iter()
                .any(|e| e.contains("mutations are not allowed over HTTP GET"))
        );

        // 4. Read-only GraphQL query over POST does NOT inject Idempotency-Key in AutoOnMutating mode
        req.method = HttpMethod::Post;
        req.body = RequestBody::GraphQl {
            query: "query Health { health { status } }".into(),
            variables: "{}".into(),
            operation_name: Some("Health".into()),
        };
        let r_query = resolve_request_preview(&req, None, &[], &ctx, &[], 1010);
        assert!(r_query.blocking_errors.is_empty());
        assert!(
            !r_query
                .headers
                .iter()
                .any(|h| h.name.eq_ignore_ascii_case("Idempotency-Key")),
            "read-only GraphQL query must not inject Idempotency-Key under AutoOnMutating"
        );
    }

    #[test]
    fn v0_1_16_auth_sigv4_digest_variable_inspector_and_soft_trash() {
        let mut ws = WorkspaceState::new_personal(LocalId::parse("personal").unwrap(), "Personal");
        assert_eq!(ws.environments.len(), 3);
        // Every starter environment now includes a base_url variable so switching environments never breaks {{base_url}}.
        for env in &ws.environments {
            assert!(env.variables.iter().any(|v| v.key == "base_url"));
        }

        // Add a second request, test reordering and soft-delete trash + restore.
        let col_id = ws.collections[0].id.clone();
        let r2 = RequestItem::new(
            col_id,
            "Second Endpoint",
            HttpMethod::Post,
            "{{base_url}}/items/{{missing_id}}",
        );
        let r2_id = r2.id.clone();
        ws.requests.push(r2);
        assert!(ws.reorder_request(&r2_id, true));
        assert_eq!(ws.requests[0].id, r2_id);

        // Inspect variables: {{base_url}} resolves, {{missing_id}} reports missing error without crashing.
        let active_env = ws.active_environment().cloned();
        let col = ws.collections.first();
        let var_ctx = VariableContext {
            workspace_vars: &ws.workspace_vars,
            collection_vars: col.map(|c| c.variables.as_slice()).unwrap_or(&[]),
            environment: active_env.as_ref(),
            request_vars: &[],
            local_secrets: &BTreeMap::new(),
        };
        let statuses = inspect_request_variables(&ws.requests[0], col, &[], &var_ctx);
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].name, "base_url");
        assert!(statuses[0].error.is_none());
        assert_eq!(statuses[1].name, "missing_id");
        assert!(statuses[1].error.is_some());

        // AWS SigV4 & Digest auth header generation
        let mut sig_req = RequestItem::new(
            "col-starter",
            "SigV4 Check",
            HttpMethod::Get,
            "https://s3.us-east-1.amazonaws.com/bucket/key",
        );
        sig_req.auth = AuthConfig::AwsSigV4 {
            access_key: "AKIAIOSFODNN7EXAMPLE".into(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into(),
            region: "us-east-1".into(),
            service: "s3".into(),
            session_token: "tok-123".into(),
        };
        let resolved_sig =
            resolve_request_preview(&sig_req, None, &[], &var_ctx, &[], 1_700_000_000);
        assert!(resolved_sig.blocking_errors.is_empty());
        assert!(
            resolved_sig
                .headers
                .iter()
                .any(|h| h.name == "Authorization" && h.value.starts_with("AWS4-HMAC-SHA256 "))
        );
        assert!(
            resolved_sig
                .headers
                .iter()
                .any(
                    |h| h.name.eq_ignore_ascii_case("x-amz-security-token") && h.value == "tok-123"
                )
        );

        let mut dig_req = RequestItem::new(
            "col-starter",
            "Digest Check",
            HttpMethod::Get,
            "https://api.example.test/protected",
        );
        dig_req.auth = AuthConfig::Digest {
            username: "alice".into(),
            password: "secret".into(),
            realm: "api".into(),
            nonce: "dcd98b7102dd2f0e8b11d0f600bfb0c093".into(),
        };
        let resolved_dig =
            resolve_request_preview(&dig_req, None, &[], &var_ctx, &[], 1_700_000_000);
        assert!(
            resolved_dig
                .headers
                .iter()
                .any(|h| h.name == "Authorization"
                    && h.value.starts_with("Digest username=\"alice\""))
        );

        // Soft-delete to Trash & Restore
        assert!(ws.delete_request(&r2_id));
        assert_eq!(ws.trashed_requests.len(), 1);
        assert!(!ws.requests.iter().any(|r| r.id == r2_id));
        assert!(ws.restore_trashed_request(&r2_id).is_some());
        assert!(ws.trashed_requests.is_empty());
        assert!(ws.requests.iter().any(|r| r.id == r2_id));
    }

    #[test]
    fn brace_variable_trigger_completion_and_suggestions_work() {
        let mut env = Environment::new("Development", false);
        env.variables.push(VariableEntry::public(
            "base_url",
            "https://api.example.test",
        ));
        let sec_id = crate::LocalId::parse("sec-1").unwrap();
        env.variables
            .push(VariableEntry::secret("api_token", sec_id.clone()));
        let col_vars = vec![
            VariableEntry::public("base_url", "https://ignored.collection.test"),
            VariableEntry::public("user_id", "42"),
        ];
        let ws_vars = vec![VariableEntry::public("tenant", "acme")];
        let mut secrets = BTreeMap::new();
        secrets.insert(sec_id, "super-secret".into());

        let ctx = VariableContext {
            workspace_vars: &ws_vars,
            collection_vars: &col_vars,
            environment: Some(&env),
            request_vars: &[],
            local_secrets: &secrets,
        };
        let suggestions = ctx.available_suggestions();
        let base_s = suggestions.iter().find(|s| s.key == "base_url").unwrap();
        assert_eq!(base_s.scope, "Development");
        assert_eq!(base_s.value_preview, "https://api.example.test");
        let sec_s = suggestions.iter().find(|s| s.key == "api_token").unwrap();
        assert_eq!(sec_s.value_preview, "••••••••");
        assert!(
            suggestions
                .iter()
                .any(|s| s.key == "tenant" && s.scope == "Global")
        );
        assert!(
            suggestions
                .iter()
                .any(|s| s.key == "$uuid" && s.scope == "Dynamic")
        );

        assert_eq!(
            extract_brace_var_trigger("https://host/{", 14),
            Some((13, String::new()))
        );
        assert_eq!(
            extract_brace_var_trigger("https://host/{{ba", 17),
            Some((13, "ba".into()))
        );
        assert_eq!(
            extract_brace_var_trigger("{{base_url}}/users/{u", 21),
            Some((19, "u".into()))
        );
        assert_eq!(extract_brace_var_trigger("{{base_url}}/users", 18), None);
        assert_eq!(extract_brace_var_trigger("{\n  \"id\": 1", 11), None);

        let (done, new_cur) = apply_brace_var_completion("{/users", 0, 1, "base_url");
        assert_eq!(done, "{{base_url}}/users");
        assert_eq!(new_cur, 12);

        let (done2, new_cur2) = apply_brace_var_completion("{{ba}}/users", 0, 4, "base_url");
        assert_eq!(done2, "{{base_url}}/users");
        assert_eq!(new_cur2, 12);
    }
}
