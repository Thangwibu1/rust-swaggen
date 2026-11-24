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

    let resp = execute_request(&req, scratch_dir, CancellationToken::new())
        .await
        .map_err(|e| format!("{}: {}", e.kind.as_str(), e.message))?;
    let latency = resp.duration_ms;
    let text = String::from_utf8_lossy(&resp.preview_bytes).into_owned();
    if !(200..300).contains(&resp.status_code) {
        if let Ok(env) = serde_json::from_str::<ErrorEnvelope>(&text) {
            return Err(format!(
                "HTTP {} [{}] {} (cid={})",
                resp.status_code, env.code, env.safe_message, env.correlation_id
            ));
        }
        return Err(format!("Server returned HTTP {}", resp.status_code));
    }
    Ok((text, latency))
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
    let parsed: Resp = serde_json::from_str(&text)
        .map_err(|e| format!("Invalid server response payload: {e}"))?;
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
