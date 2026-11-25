//! Native OAuth 2.0 PKCE + Loopback CSRF State Callback + Client Credentials + Token Refresh (AU08).
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};
use swaggen_domain::workspace::{
    HttpMethod, RequestBody, RequestConnectionPolicy, ResolvedHeader, ResolvedRequest,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OAuthTokenSet {
    pub access_token: String,
    pub token_type: String,
    pub refresh_token: Option<String>,
    pub expires_in_secs: Option<u64>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PkceChallenge {
    pub state: String,
    pub code_verifier: String,
    pub code_challenge: String,
}

impl PkceChallenge {
    pub fn generate() -> Self {
        let mut s_bytes = [0u8; 24];
        let mut v_bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut s_bytes);
        rand::rng().fill_bytes(&mut v_bytes);
        let state = URL_SAFE_NO_PAD.encode(s_bytes);
        let code_verifier = URL_SAFE_NO_PAD.encode(v_bytes);
        let challenge = Sha256::digest(code_verifier.as_bytes());
        let code_challenge = URL_SAFE_NO_PAD.encode(challenge);
        Self {
            state,
            code_verifier,
            code_challenge,
        }
    }

    pub fn build_authorize_url(
        &self,
        auth_url: &str,
        client_id: &str,
        redirect_uri: &str,
        scope: &str,
    ) -> String {
        let sep = if auth_url.contains('?') { '&' } else { '?' };
        let mut pairs = url::form_urlencoded::Serializer::new(String::new());
        pairs.append_pair("response_type", "code");
        pairs.append_pair("client_id", client_id);
        pairs.append_pair("redirect_uri", redirect_uri);
        pairs.append_pair("state", &self.state);
        pairs.append_pair("code_challenge", &self.code_challenge);
        pairs.append_pair("code_challenge_method", "S256");
        if !scope.trim().is_empty() {
            pairs.append_pair("scope", scope.trim());
        }
        format!("{auth_url}{sep}{}", pairs.finish())
    }
}

/// Awaits one HTTP callback on loopback `127.0.0.1` and strictly verifies OAuth2 CSRF `state` (AU08).
pub async fn await_loopback_callback(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<String, String> {
    let accept_fut = async {
        loop {
            let (mut sock, _) = listener.accept().await.map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let first_line = req.lines().next().unwrap_or("");
            let path = first_line.split_whitespace().nth(1).unwrap_or("/");
            let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");

            let mut code: Option<String> = None;
            let mut state: Option<String> = None;
            let mut err: Option<String> = None;
            for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
                match k.as_ref() {
                    "code" => code = Some(v.into_owned()),
                    "state" => state = Some(v.into_owned()),
                    "error" => err = Some(v.into_owned()),
                    _ => {}
                }
            }

            if let Some(e_msg) = err {
                let body = "Authorization denied by provider.";
                let _ = sock
                    .write_all(
                        format!(
                            "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await;
                return Err(format!("OAuth provider error: {e_msg}"));
            }
            if state.as_deref() != Some(expected_state) {
                let body = "OAuth state mismatch rejected.";
                let _ = sock
                    .write_all(
                        format!(
                            "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await;
                return Err("OAuth callback rejected: CSRF state mismatch".into());
            }
            if let Some(c) = code {
                let body = "Swaggen authorization received. You may close this window.";
                let _ = sock
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await;
                return Ok(c);
            }
        }
    };

    tokio::time::timeout(timeout, accept_fut)
        .await
        .map_err(|_| "Timed out waiting for OAuth loopback callback".to_owned())?
}

pub async fn exchange_token_form(
    token_url: &str,
    form_pairs: &[(&str, &str)],
    downloads_dir: &Path,
) -> Result<OAuthTokenSet, String> {
    let body = {
        let mut ser = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in form_pairs {
            ser.append_pair(k, v);
        }
        ser.finish()
    };
    let req = ResolvedRequest {
        method: HttpMethod::Post,
        url: token_url.to_owned(),
        masked_url: token_url.to_owned(),
        headers: vec![ResolvedHeader {
            name: "Content-Type".into(),
            value: "application/x-www-form-urlencoded".into(),
            generated: true,
        }],
        masked_headers: Vec::new(),
        body: RequestBody::Text(body),
        masked_body_preview: String::new(),
        auth_source: "OAuth2".into(),
        warnings: Vec::new(),
        blocking_errors: Vec::new(),
        requires_production_confirm: false,
        connection: RequestConnectionPolicy::default(),
    };
    let resp = crate::execute_request(&req, downloads_dir, CancellationToken::new())
        .await
        .map_err(|e| format!("{}: {}", e.kind.as_str(), e.message))?;
    let json_val: serde_json::Value = serde_json::from_slice(&resp.preview_bytes)
        .map_err(|e| format!("Invalid OAuth JSON token response: {e}"))?;
    if !(200..300).contains(&resp.status_code) {
        return Err(format!(
            "Token endpoint rejected request (HTTP {})",
            resp.status_code
        ));
    }
    Ok(OAuthTokenSet {
        access_token: json_val
            .get("access_token")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "Missing access_token in response".to_owned())?
            .to_owned(),
        token_type: json_val
            .get("token_type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Bearer")
            .to_owned(),
        refresh_token: json_val
            .get("refresh_token")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        expires_in_secs: json_val
            .get("expires_in")
            .and_then(serde_json::Value::as_u64),
        scope: json_val
            .get("scope")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pkce_loopback_verifies_state_rejects_mismatch_and_exchanges_token() {
        let pkce = PkceChallenge::generate();
        assert_eq!(pkce.code_verifier.len(), 43);

        // 1. State mismatch rejected cleanly
        let l1 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let a1 = l1.local_addr().unwrap();
        let state1 = pkce.state.clone();
        let srv1 = tokio::spawn(async move {
            await_loopback_callback(l1, &state1, Duration::from_secs(3)).await
        });
        let mut s = tokio::net::TcpStream::connect(a1).await.unwrap();
        s.write_all(b"GET /cb?code=abc&state=WRONG HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        assert!(srv1.await.unwrap().is_err());

        // 2. Valid state + token exchange + refresh against mock OAuth server
        let l2 = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let a2 = l2.local_addr().unwrap();
        let state2 = pkce.state.clone();
        let srv2 = tokio::spawn(async move {
            await_loopback_callback(l2, &state2, Duration::from_secs(3)).await
        });
        let mut s2 = tokio::net::TcpStream::connect(a2).await.unwrap();
        s2.write_all(
            format!(
                "GET /cb?code=authcode123&state={} HTTP/1.1\r\nHost: localhost\r\n\r\n",
                pkce.state
            )
            .as_bytes(),
        )
        .await
        .unwrap();
        assert_eq!(srv2.await.unwrap().unwrap(), "authcode123");

        let token_srv = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let taddr = token_srv.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut tsock, _)) = token_srv.accept().await {
                let mut buf = vec![0u8; 4096];
                let _ = tsock.read(&mut buf).await;
                let body = r#"{"access_token":"tok_1","token_type":"Bearer","refresh_token":"ref_1","expires_in":3600}"#;
                let _ = tsock
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await;
            }
        });
        let dl = std::env::temp_dir().join(format!("swaggen-oauth-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dl);
        let tok = exchange_token_form(
            &format!("http://{taddr}/token"),
            &[
                ("grant_type", "authorization_code"),
                ("code", "authcode123"),
                ("code_verifier", &pkce.code_verifier),
            ],
            &dl,
        )
        .await
        .unwrap();
        assert_eq!(tok.access_token, "tok_1");
        assert_eq!(tok.refresh_token.as_deref(), Some("ref_1"));
        let _ = std::fs::remove_dir_all(dl);
    }
}
