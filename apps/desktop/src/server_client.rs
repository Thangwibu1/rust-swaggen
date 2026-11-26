//! Server HTTP client adapter over swaggen-http-engine (desktop never links mongodb).
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;
use swaggen_api_contract::ErrorEnvelope;
use swaggen_domain::{
    server_profiles::{ServerProfile, validate_server_url},
    workspace::{
        HttpMethod, RequestBody, RequestConnectionPolicy, ResolvedHeader, ResolvedRequest,
    },
};
use swaggen_http_engine::execute_request;
use tokio_util::sync::CancellationToken;

fn is_retry_safe_server_call(method: HttpMethod, path: &str) -> bool {
    method == HttpMethod::Get
        || matches!(
            path,
            "/v1/auth/firebase" | "/v1/auth/login" | "/v1/auth/logout"
        )
}

fn summarize_plain_http_error(status: u16, body: &str) -> String {
    let mut cleaned = String::new();
    let mut in_tag = false;
    for ch in body.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                cleaned.push(' ');
            }
            _ if !in_tag => cleaned.push(ch),
            _ => {}
        }
        if cleaned.len() >= 180 {
            break;
        }
    }
    let snippet: String = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let base = match status {
        502..=504 => {
            format!(
                "Server returned HTTP {status} (server restarting or temporarily unavailable — please retry)"
            )
        }
        _ => format!("Server returned HTTP {status}"),
    };
    if snippet.is_empty() {
        base
    } else {
        let short: String = snippet.chars().take(100).collect();
        format!("{base}: {short}")
    }
}

async fn send_raw<Req: Serialize>(
    profile: &ServerProfile,
    method: HttpMethod,
    path: &str,
    bearer: Option<&str>,
    body: Option<&Req>,
    scratch_dir: &Path,
) -> Result<(String, u64), String> {
    let (base, host, _) = validate_server_url(&profile.base_url)?;
    let url = format!("{base}{path}");
    let mut headers = vec![ResolvedHeader {
        name: "Accept".into(),
        value: "application/json".into(),
        generated: true,
    }];
    if let Some(tok) = bearer {
        headers.push(ResolvedHeader {
            name: "Authorization".into(),
            value: format!("Bearer {tok}"),
            generated: true,
        });
    }
    let body_json = if let Some(b) = body {
        headers.push(ResolvedHeader {
            name: "Content-Type".into(),
            value: "application/json".into(),
            generated: true,
        });
        Some(serde_json::to_string(b).map_err(|e| e.to_string())?)
    } else {
        None
    };

    let req = ResolvedRequest {
        method,
        url: url.clone(),
        masked_url: url,
        headers: headers.clone(),
        masked_headers: headers,
        body: match body_json {
            Some(text) => RequestBody::Json(text),
            None => RequestBody::None,
        },
        masked_body_preview: String::new(),
        auth_source: "ServerSession".into(),
        warnings: Vec::new(),
        blocking_errors: Vec::new(),
        requires_production_confirm: false,
        connection: RequestConnectionPolicy {
            timeout_ms: 15_000,
            follow_redirects: false,
            max_redirects: 0,
            send_auth_on_cross_host_redirect: false,
            tls_host_exceptions: if profile.allow_self_signed_tls {
                vec![host]
            } else {
                Vec::new()
            },
            ..Default::default()
        },
    };

    let max_attempts = if is_retry_safe_server_call(method, path) {
        3usize
    } else {
        1usize
    };
    let mut last_err = String::new();
    for attempt in 0..max_attempts {
        if attempt > 0 {
            let delay_ms = if attempt == 1 { 450 } else { 1100 };
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
        let resp = match execute_request(&req, scratch_dir, CancellationToken::new()).await {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("{}: {}", e.kind.as_str(), e.message);
                if attempt + 1 < max_attempts {
                    continue;
                }
                return Err(last_err);
            }
        };
        let latency = resp.duration_ms;
        let text = String::from_utf8_lossy(&resp.preview_bytes).into_owned();
        if (200..300).contains(&resp.status_code) {
            return Ok((text, latency));
        }
        if matches!(resp.status_code, 502..=504) && attempt + 1 < max_attempts {
            last_err = summarize_plain_http_error(resp.status_code, &text);
            continue;
        }
        if let Ok(env) = serde_json::from_str::<ErrorEnvelope>(&text) {
            return Err(format!(
                "HTTP {} [{}] {} (cid={})",
                resp.status_code, env.code, env.safe_message, env.correlation_id
            ));
        }
        return Err(summarize_plain_http_error(resp.status_code, &text));
    }
    Err(last_err)
}

pub async fn server_call<Req: Serialize, Resp: DeserializeOwned>(
    profile: &ServerProfile,
    method: HttpMethod,
    path: &str,
    bearer: Option<&str>,
    body: Option<&Req>,
    scratch_dir: &Path,
) -> Result<(Resp, u64), String> {
    let (text, latency) = send_raw(profile, method, path, bearer, body, scratch_dir).await?;
    let parsed: Resp =
        serde_json::from_str(&text).map_err(|e| format!("Invalid server response payload: {e}"))?;
    Ok((parsed, latency))
}

pub async fn server_call_empty<Req: Serialize>(
    profile: &ServerProfile,
    method: HttpMethod,
    path: &str,
    bearer: Option<&str>,
    body: Option<&Req>,
    scratch_dir: &Path,
) -> Result<(), String> {
    send_raw(profile, method, path, bearer, body, scratch_dir)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_policy_and_http_error_summary_work() {
        assert!(is_retry_safe_server_call(HttpMethod::Get, "/v1/workspaces"));
        assert!(is_retry_safe_server_call(
            HttpMethod::Post,
            "/v1/auth/firebase"
        ));
        assert!(!is_retry_safe_server_call(
            HttpMethod::Post,
            "/v1/workspaces/ws-1/entities"
        ));

        let msg = summarize_plain_http_error(503, "<html><body>Service Unavailable</body></html>");
        assert!(msg.contains("HTTP 503"));
        assert!(msg.contains("Service Unavailable"));
        assert!(!msg.contains("<html>"));
    }
}
