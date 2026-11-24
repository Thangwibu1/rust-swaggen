//! Non-secret local server profile metadata (session tokens live strictly in DPAPI SecretStore).
use crate::{Error, LocalId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub allow_self_signed_tls: bool,
    #[serde(default = "default_min_api")]
    pub expected_min_api_version: u32,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub session_expires_unix: i64,
    #[serde(default)]
    pub last_checked_unix: i64,
    #[serde(default)]
    pub last_server_version: String,
    #[serde(default)]
    pub last_latency_ms: Option<u64>,
    #[serde(default)]
    pub last_tls_status: String,
    #[serde(default)]
    pub last_error: String,
    #[serde(default)]
    pub bootstrap_required: bool,
    #[serde(default)]
    pub mongo_ready: bool,
    #[serde(default)]
    pub firebase_project_id: Option<String>,
    #[serde(default)]
    pub firebase_web_api_key: Option<String>,
    #[serde(default)]
    pub firebase_auth_domain: Option<String>,
}

fn default_min_api() -> u32 {
    1
}

impl ServerProfile {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
        allow_self_signed_tls: bool,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            base_url: base_url.into(),
            allow_self_signed_tls,
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
            firebase_project_id: None,
            firebase_web_api_key: None,
            firebase_auth_domain: None,
        }
    }

    pub fn secret_key_id(&self) -> Result<LocalId, Error> {
        let cleaned: String = self
            .id
            .chars()
            .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-' || *c == '_')
            .take(48)
            .collect();
        let name = if cleaned.is_empty() {
            "srv-default".to_owned()
        } else {
            format!("srv-{cleaned}")
        };
        LocalId::parse(name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ServerProfileStore {
    pub active_profile_id: Option<String>,
    pub profiles: Vec<ServerProfile>,
}

impl ServerProfileStore {
    pub fn active(&self) -> Option<&ServerProfile> {
        let id = self.active_profile_id.as_deref()?;
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn active_mut(&mut self) -> Option<&mut ServerProfile> {
        let id = self.active_profile_id.as_deref()?.to_owned();
        self.profiles.iter_mut().find(|p| p.id == id)
    }

    pub fn upsert(&mut self, profile: ServerProfile) {
        if let Some(existing) = self.profiles.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile;
        } else {
            let set_active = self.active_profile_id.is_none();
            let id = profile.id.clone();
            self.profiles.push(profile);
            if set_active {
                self.active_profile_id = Some(id);
            }
        }
    }
}

/// Validates server base URL policy: requires https:// unless loopback development server.
pub fn validate_server_url(raw: &str) -> Result<(String, String, bool), String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Server URL cannot be empty".to_owned());
    }
    let parsed = reqwest_free_url_parse(trimmed)?;
    let is_tls = parsed.scheme == "https";
    let is_loopback = matches!(parsed.host.as_str(), "127.0.0.1" | "localhost" | "::1");
    if !is_tls && !is_loopback {
        return Err("Remote server URL must use https:// (http:// is allowed only for 127.0.0.1/localhost)".to_owned());
    }
    Ok((trimmed.to_owned(), parsed.host, is_tls))
}

struct ParsedOrigin {
    scheme: String,
    host: String,
}

fn reqwest_free_url_parse(raw: &str) -> Result<ParsedOrigin, String> {
    let (scheme, rest) = raw
        .split_once("://")
        .ok_or_else(|| "Server URL must include https:// or http://".to_owned())?;
    let scheme_lc = scheme.to_ascii_lowercase();
    if scheme_lc != "https" && scheme_lc != "http" {
        return Err("Server URL scheme must be https or http".to_owned());
    }
    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err("Server URL host is missing or contains embedded credentials".to_owned());
    }
    let host = if authority.starts_with('[') {
        authority
            .split(']')
            .next()
            .unwrap_or("")
            .trim_start_matches('[')
            .to_ascii_lowercase()
    } else {
        authority
            .split(':')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
    };
    if host.is_empty() {
        return Err("Server URL host is empty".to_owned());
    }
    Ok(ParsedOrigin {
        scheme: scheme_lc,
        host,
    })
}
