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
    Text(String),
    Xml(String),
    FormUrlEncoded(Vec<KeyValueRow>),
    Multipart(Vec<MultipartField>),
    Binary {
        local_path: Option<String>,
        content_type: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiKeyPlacement {
    Header,
    Query,
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
            linked_operation_uid: None,
            shared_revision: 0,
            dirty_vs_shared: false,
            order_key: 0,
        };
        item.sync_params_from_url();
        item
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
    pub order_key: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub description: String,
    pub auth: AuthConfig,
    pub variables: Vec<VariableEntry>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub is_production: bool,
    pub variables: Vec<VariableEntry>,
    pub personal_overrides: BTreeMap<String, PersonalVarOverride>,
}

impl Environment {
    pub fn new(name: impl Into<String>, is_production: bool) -> Self {
        Self {
            id: new_uid(),
            name: name.into(),
            is_production,
            variables: Vec::new(),
            personal_overrides: BTreeMap::new(),
        }
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
    // so synced URLs never double-append query parameters on send/preview.
    let (url_base_part, url_inline_query, url_fragment) = split_url_parts(&request.url);
    let effective_url_template = if !request.query_params.is_empty() {
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
    for q in request
        .query_params
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

    // 3. Resolve Auth & inject header or query API key.
    let eff_auth = resolve_effective_auth(collection, folder_chain, request);
    let mut headers = Vec::new();
    let mut masked_headers = Vec::new();
    match &eff_auth.config {
        AuthConfig::Inherit | AuthConfig::NoAuth => {}
        AuthConfig::Bearer { token } => {
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
                ApiKeyPlacement::Query => {
                    warnings.push("API key in query string (H05): visible in server logs and scrubbed on default export.".into());
                    query_pairs.push((real_k.clone(), real_v));
                    masked_query_pairs.push((real_k, "••••••••".into()));
                }
            }
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

    // 5. Body resolution + JSON/XML/file validation (Q10, Q17).
    // GET and HEAD requests never transmit a request body.
    let (resolved_body, masked_body_preview) =
        if matches!(request.method, HttpMethod::Get | HttpMethod::Head) {
            (RequestBody::None, String::new())
        } else {
            match &request.body {
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
            }
        };

    let requires_production_confirm =
        vars.environment.is_some_and(|e| e.is_production) && request.method.is_mutating();

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
            auth: AuthConfig::Inherit,
            variables: Vec::new(),
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
        let staging = Environment::new("Staging", false);
        let prod = Environment::new("Production", true);
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
        let before = self.requests.len();
        self.requests.retain(|r| r.id != request_id);
        if self.requests.len() == before {
            return false;
        }
        self.open_tabs.retain(|t| t.request_id != request_id);
        if let Some(ref active) = self.active_tab_id
            && !self.open_tabs.iter().any(|t| &t.id == active)
        {
            self.active_tab_id = self.open_tabs.first().map(|t| t.id.clone());
        }
        true
    }

    pub fn delete_collection(&mut self, collection_id: &str) -> bool {
        if self.collections.len() <= 1 {
            return false;
        }
        let before = self.collections.len();
        self.collections.retain(|c| c.id != collection_id);
        if self.collections.len() == before {
            return false;
        }
        self.folders.retain(|f| f.collection_id != collection_id);
        let removed_reqs: BTreeSet<String> = self
            .requests
            .iter()
            .filter(|r| r.collection_id == collection_id)
            .map(|r| r.id.clone())
            .collect();
        self.requests.retain(|r| r.collection_id != collection_id);
        self.open_tabs
            .retain(|t| !removed_reqs.contains(&t.request_id));
        if let Some(ref active) = self.active_tab_id
            && !self.open_tabs.iter().any(|t| &t.id == active)
        {
            self.active_tab_id = self.open_tabs.first().map(|t| t.id.clone());
        }
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
            auth: AuthConfig::Bearer {
                token: "{{token}}".into(),
            },
            variables: Vec::new(),
            order_key: 0,
        };
        let folder = Folder {
            id: "f1".into(),
            collection_id: "c1".into(),
            parent_folder_id: None,
            name: "Admin".into(),
            description: String::new(),
            auth: AuthConfig::Inherit,
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
}
