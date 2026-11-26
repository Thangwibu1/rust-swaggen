//! Firebase Authentication ID Token RS256 cryptographic verifier (Google JWKS + ring)
//! and desktop browser handoff page for Google & Email/Password Firebase sign-in.
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use http_body_util::{BodyExt, Empty};
use hyper::{Request, Uri, body::Bytes};
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use ring::signature::{RSA_PKCS1_2048_8192_SHA256, UnparsedPublicKey};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

const FIREBASE_JWKS_URL: &str =
    "https://www.googleapis.com/service_accounts/v1/jwk/securetoken@system.gserviceaccount.com";

#[derive(Debug, Clone)]
pub struct FirebaseAuthConfig {
    pub project_id: String,
    pub web_api_key: Option<String>,
    pub auth_domain: String,
}

impl FirebaseAuthConfig {
    pub fn from_env() -> Option<Self> {
        let project_id = std::env::var("SWAGGEN_FIREBASE_PROJECT_ID")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())?;
        let web_api_key = std::env::var("SWAGGEN_FIREBASE_WEB_API_KEY")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        let auth_domain = std::env::var("SWAGGEN_FIREBASE_AUTH_DOMAIN")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("{project_id}.firebaseapp.com"));
        Some(Self {
            project_id,
            web_api_key,
            auth_domain,
        })
    }
}

#[derive(Debug, Clone)]
pub struct VerifiedFirebaseClaims {
    pub uid: String,
    pub email: String,
    pub display_name: String,
    pub email_verified: bool,
}

#[derive(Debug, Deserialize)]
struct JwtHeader {
    alg: String,
    #[serde(default)]
    kid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FirebaseJwtPayload {
    aud: String,
    iss: String,
    sub: String,
    exp: i64,
    iat: i64,
    #[serde(default)]
    auth_time: Option<i64>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    email_verified: Option<bool>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwkSet {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwkKey {
    pub kid: String,
    pub kty: String,
    #[serde(default)]
    pub alg: Option<String>,
    pub n: String,
    pub e: String,
}

struct CachedJwks {
    keys: HashMap<String, Vec<u8>>,
    expires_at: Instant,
}

pub struct FirebaseVerifier {
    config: FirebaseAuthConfig,
    cache: RwLock<CachedJwks>,
}

impl FirebaseVerifier {
    pub fn new(config: FirebaseAuthConfig) -> Self {
        Self {
            config,
            cache: RwLock::new(CachedJwks {
                keys: HashMap::new(),
                expires_at: Instant::now(),
            }),
        }
    }

    pub fn config(&self) -> &FirebaseAuthConfig {
        &self.config
    }

    /// Cryptographically verifies a Firebase ID Token (RS256) against Google's Firebase JWKS
    /// and validates all Firebase security claims (`aud`, `iss`, `sub`, `exp`, `iat`, `auth_time`).
    pub async fn verify_id_token(
        &self,
        id_token: &str,
        now_unix: i64,
    ) -> Result<VerifiedFirebaseClaims, String> {
        let parts: Vec<&str> = id_token.trim().split('.').collect();
        if parts.len() != 3 {
            return Err("Malformed Firebase ID token".into());
        }
        let header_bytes = URL_SAFE_NO_PAD
            .decode(parts[0])
            .map_err(|_| "Invalid Firebase JWT header encoding".to_owned())?;
        let header: JwtHeader = serde_json::from_slice(&header_bytes)
            .map_err(|_| "Invalid Firebase JWT header JSON".to_owned())?;
        if header.alg != "RS256" {
            return Err("Firebase ID token must use RS256".into());
        }
        let kid = header
            .kid
            .filter(|k| !k.is_empty())
            .ok_or_else(|| "Firebase JWT header missing kid".to_owned())?;

        let key_der = self.resolve_key_der(&kid).await?;
        verify_jwt_with_rsa_der(id_token, &key_der, &self.config.project_id, now_unix)
    }

    async fn resolve_key_der(&self, kid: &str) -> Result<Vec<u8>, String> {
        {
            let guard = self
                .cache
                .read()
                .map_err(|_| "JWKS cache lock poisoned".to_owned())?;
            if Instant::now() < guard.expires_at
                && let Some(der) = guard.keys.get(kid)
            {
                return Ok(der.clone());
            }
        }
        let (fresh_keys, ttl) = fetch_google_firebase_jwks().await?;
        let mut guard = self
            .cache
            .write()
            .map_err(|_| "JWKS cache lock poisoned".to_owned())?;
        guard.keys = fresh_keys;
        guard.expires_at = Instant::now() + ttl;
        guard
            .keys
            .get(kid)
            .cloned()
            .ok_or_else(|| "Unknown Firebase signing key (kid)".to_owned())
    }
}

/// Pure function used by production verifier and offline unit tests.
pub fn verify_jwt_with_rsa_der(
    id_token: &str,
    rsa_pkcs1_der: &[u8],
    expected_project_id: &str,
    now_unix: i64,
) -> Result<VerifiedFirebaseClaims, String> {
    let parts: Vec<&str> = id_token.trim().split('.').collect();
    if parts.len() != 3 {
        return Err("Malformed Firebase ID token".into());
    }
    let signing_input = format!("{}.{}", parts[0], parts[1]);
    let sig_bytes = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|_| "Invalid Firebase JWT signature encoding".to_owned())?;

    let public_key = UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, rsa_pkcs1_der);
    public_key
        .verify(signing_input.as_bytes(), &sig_bytes)
        .map_err(|_| "Firebase ID token signature verification failed".to_owned())?;

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| "Invalid Firebase JWT payload encoding".to_owned())?;
    let payload: FirebaseJwtPayload = serde_json::from_slice(&payload_bytes)
        .map_err(|_| "Invalid Firebase JWT payload JSON".to_owned())?;

    let expected_iss = format!("https://securetoken.google.com/{expected_project_id}");
    if payload.aud != expected_project_id {
        return Err("Firebase token audience does not match project ID".into());
    }
    if payload.iss != expected_iss {
        return Err("Firebase token issuer does not match project ID".into());
    }
    if payload.sub.trim().is_empty() || payload.sub.len() > 128 {
        return Err("Firebase token subject (uid) is empty or invalid".into());
    }
    if payload.exp <= now_unix {
        return Err("Firebase ID token has expired".into());
    }
    if payload.iat > now_unix + 300 {
        return Err("Firebase ID token issued in the future".into());
    }
    if let Some(auth_time) = payload.auth_time
        && auth_time > now_unix + 300
    {
        return Err("Firebase ID token auth_time is in the future".into());
    }

    let email = payload
        .email
        .filter(|e| !e.trim().is_empty())
        .ok_or_else(|| "Firebase account does not expose an email address".to_owned())?;
    let display_name = payload
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| email.split('@').next().unwrap_or("User").to_owned());

    Ok(VerifiedFirebaseClaims {
        uid: payload.sub,
        email,
        display_name,
        email_verified: payload.email_verified.unwrap_or(false),
    })
}

/// Encodes RSA JWK modulus `n` and exponent `e` (base64url) into ASN.1 DER PKCS#1 RSAPublicKey.
pub fn encode_rsa_jwk_to_pkcs1_der(n_b64: &str, e_b64: &str) -> Result<Vec<u8>, String> {
    let n = URL_SAFE_NO_PAD
        .decode(n_b64)
        .map_err(|_| "Invalid JWK modulus base64url".to_owned())?;
    let e = URL_SAFE_NO_PAD
        .decode(e_b64)
        .map_err(|_| "Invalid JWK exponent base64url".to_owned())?;
    if n.is_empty() || e.is_empty() {
        return Err("Empty RSA JWK component".into());
    }
    let mut seq_body = Vec::with_capacity(n.len() + e.len() + 16);
    encode_der_integer(&n, &mut seq_body);
    encode_der_integer(&e, &mut seq_body);

    let mut out = Vec::with_capacity(seq_body.len() + 6);
    out.push(0x30); // ASN.1 SEQUENCE
    encode_der_length(seq_body.len(), &mut out);
    out.extend_from_slice(&seq_body);
    Ok(out)
}

fn encode_der_integer(unsigned_be: &[u8], out: &mut Vec<u8>) {
    let trimmed = match unsigned_be.iter().position(|&b| b != 0) {
        Some(idx) => &unsigned_be[idx..],
        None => &[0u8],
    };
    let needs_pad = trimmed[0] & 0x80 != 0;
    out.push(0x02); // ASN.1 INTEGER
    encode_der_length(trimmed.len() + usize::from(needs_pad), out);
    if needs_pad {
        out.push(0x00);
    }
    out.extend_from_slice(trimmed);
}

fn encode_der_length(len: usize, out: &mut Vec<u8>) {
    if len < 0x80 {
        out.push(len as u8);
    } else if len <= 0xFF {
        out.push(0x81);
        out.push(len as u8);
    } else {
        out.push(0x82);
        out.push(((len >> 8) & 0xFF) as u8);
        out.push((len & 0xFF) as u8);
    }
}

async fn fetch_google_firebase_jwks() -> Result<(HashMap<String, Vec<u8>>, Duration), String> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls_cfg = Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    );

    let uri: Uri = FIREBASE_JWKS_URL
        .parse()
        .map_err(|_| "Invalid Firebase JWKS URI".to_owned())?;
    let host = uri
        .host()
        .ok_or_else(|| "Missing JWKS host".to_owned())?
        .to_owned();

    let tcp = tokio::net::TcpStream::connect((host.as_str(), 443))
        .await
        .map_err(|_| "Failed to connect to Google Firebase JWKS endpoint".to_owned())?;
    let connector = tokio_rustls::TlsConnector::from(tls_cfg);
    let server_name = rustls_pki_types::ServerName::try_from(host)
        .map_err(|_| "Invalid JWKS TLS server name".to_owned())?;
    let tls_stream = connector
        .connect(server_name, tcp)
        .await
        .map_err(|_| "TLS handshake with Google Firebase JWKS failed".to_owned())?;

    let io = hyper_util::rt::TokioIo::new(tls_stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .map_err(|_| "HTTP/1 handshake with Google JWKS failed".to_owned())?;
    tokio::spawn(async move {
        let _ = conn.await;
    });

    let req = Request::builder()
        .method("GET")
        .uri(uri.path_and_query().map(|p| p.as_str()).unwrap_or("/"))
        .header("Host", "www.googleapis.com")
        .header("Accept", "application/json")
        .body(Empty::<Bytes>::new())
        .map_err(|_| "Failed to build JWKS request".to_owned())?;

    let resp = sender
        .send_request(req)
        .await
        .map_err(|_| "Failed to fetch Google Firebase JWKS".to_owned())?;
    if !resp.status().is_success() {
        return Err(format!(
            "Google Firebase JWKS returned HTTP {}",
            resp.status()
        ));
    }
    let ttl = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_max_age_secs)
        .unwrap_or(3600)
        .clamp(300, 21_600);
    let body = resp
        .into_body()
        .collect()
        .await
        .map_err(|_| "Failed to read Google Firebase JWKS body".to_owned())?
        .to_bytes();
    let jwks: JwkSet = serde_json::from_slice(&body)
        .map_err(|_| "Invalid Google Firebase JWKS JSON".to_owned())?;

    let mut out = HashMap::new();
    for key in jwks.keys {
        if key.kty == "RSA"
            && let Ok(der) = encode_rsa_jwk_to_pkcs1_der(&key.n, &key.e)
        {
            out.insert(key.kid, der);
        }
    }
    if out.is_empty() {
        return Err("Google Firebase JWKS returned no usable RSA keys".into());
    }
    let _ = Client::builder(TokioExecutor::new());
    Ok((out, Duration::from_secs(ttl)))
}

fn parse_max_age_secs(cache_control: &str) -> Option<u64> {
    for part in cache_control.split(',') {
        let p = part.trim().to_ascii_lowercase();
        if let Some(rest) = p.strip_prefix("max-age=")
            && let Ok(secs) = rest.trim().parse::<u64>()
        {
            return Some(secs);
        }
    }
    None
}

/// Renders the Figma-aligned white-surface Firebase browser sign-in page for the Windows desktop
/// loopback handoff (`A02 / Continue in your browser`). Strictly validates `port` and `state`.
pub fn render_browser_signin_html(
    cfg: &FirebaseAuthConfig,
    port: u16,
    state: &str,
) -> Result<String, String> {
    if port < 1024 {
        return Err("Invalid loopback callback port".into());
    }
    if state.len() < 8
        || state.len() > 128
        || !state
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Invalid CSRF state parameter".into());
    }
    let api_key = cfg.web_api_key.as_deref().ok_or_else(|| {
        "SWAGGEN_FIREBASE_WEB_API_KEY is not configured on this server".to_owned()
    })?;
    let fb_cfg_json = serde_json::json!({
        "apiKey": api_key,
        "authDomain": cfg.auth_domain,
        "projectId": cfg.project_id,
    })
    .to_string();
    let state_json = serde_json::to_string(state).map_err(|_| "State serialization failed")?;

    Ok(format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width,initial-scale=1" />
<title>Sign in to Swaggen</title>
<link rel="icon" type="image/png" href="/v1/assets/favicon.png" />
<link rel="shortcut icon" href="/favicon.ico" />
<style>
  body {{ margin: 0; background: #ffffff; color: #202124; font-family: "JetBrains Mono", ui-monospace, SFMono-Regular, Consolas, monospace; display: flex; min-height: 100vh; align-items: center; justify-content: center; }}
  .card {{ width: 440px; border: 1px solid #e5e7eb; border-radius: 16px; padding: 32px 36px; box-sizing: border-box; }}
  .brand {{ display: flex; align-items: center; gap: 10px; margin-bottom: 18px; font-weight: 700; font-size: 18px; color: #202124; }}
  h1 {{ font-size: 21px; margin: 0 0 8px 0; }}
  p {{ font-size: 12px; color: #65686f; line-height: 1.6; margin: 0 0 20px 0; }}
  .btn {{ width: 100%; height: 44px; border: 1px solid #e5e7eb; border-radius: 8px; background: #ffffff; color: #202124; font-family: inherit; font-size: 13px; font-weight: 600; cursor: pointer; margin-bottom: 12px; display: flex; align-items: center; justify-content: center; gap: 10px; }}
  .btn:hover {{ border-color: #202124; background: #f8fafc; }}
  .status {{ font-size: 12px; margin-top: 12px; color: #15803d; line-height: 1.5; }}
  .err {{ color: #b91c1c; }}
</style>
<script src="https://www.gstatic.com/firebasejs/10.14.1/firebase-app-compat.js"></script>
<script src="https://www.gstatic.com/firebasejs/10.14.1/firebase-auth-compat.js"></script>
</head>
<body>
  <div class="card">
    <div class="brand">
      <img src="/v1/assets/brand-mark.png" alt="Swaggen" width="28" height="28" />
      <span>swaggen</span>
    </div>
    <h1>Sign in to Swaggen</h1>
    <p>Continue with Google to sign in or automatically create your Swaggen account.</p>
    <button id="googleBtn" class="btn">
      <img src="/v1/assets/google-g.png" alt="Google" width="18" height="18" />
      <span>Continue with Google</span>
    </button>
    <div id="msg" class="status"></div>
  </div>
<script>
  const cfg = {fb_cfg_json};
  const state = {state_json};
  const callbackUrl = "http://127.0.0.1:{port}/callback";
  firebase.initializeApp(cfg);
  const auth = firebase.auth();
  const msg = document.getElementById("msg");

  async function finish(user) {{
    msg.className = "status";
    msg.textContent = "Sending verified Firebase token to Swaggen desktop…";
    const idToken = await user.getIdToken(true);
    const res = await fetch(callbackUrl, {{
      method: "POST",
      headers: {{ "Content-Type": "application/json" }},
      body: JSON.stringify({{ id_token: idToken, state: state, display_name: user.displayName || "" }})
    }});
    if (!res.ok) throw new Error("Desktop callback rejected: HTTP " + res.status);
    msg.textContent = "Signed in as " + (user.email || "user") + "! You can close this tab and return to Swaggen.";
  }}

  document.getElementById("googleBtn").onclick = async () => {{
    try {{
      try {{ await auth.signOut(); }} catch (_) {{}}
      const provider = new firebase.auth.GoogleAuthProvider();
      provider.setCustomParameters({{ prompt: "select_account" }});
      const res = await auth.signInWithPopup(provider);
      await finish(res.user);
    }} catch (e) {{
      msg.className = "status err";
      if (e && e.code === "auth/unauthorized-domain") {{
        msg.textContent = "Domain '" + window.location.hostname + "' is not in Firebase Authorized domains. Add it in Firebase Console -> Authentication -> Settings -> Authorized domains.";
      }} else {{
        msg.textContent = (e && e.message) ? e.message : String(e);
      }}
    }}
  }};
</script>
</body>
</html>
"##
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn der_encoder_and_claim_validation_reject_forged_tokens() {
        let der = encode_rsa_jwk_to_pkcs1_der(
            &URL_SAFE_NO_PAD.encode(vec![0xAA; 256]),
            &URL_SAFE_NO_PAD.encode([0x01, 0x00, 0x01]),
        )
        .expect("valid RSA DER");
        assert_eq!(der[0], 0x30);

        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","kid":"k1"}"#);
        let payload = URL_SAFE_NO_PAD.encode(
            r#"{"aud":"demo-proj","iss":"https://securetoken.google.com/demo-proj","sub":"uid1","exp":9999999999,"iat":1000,"email":"a@b.test"}"#,
        );
        let fake_sig = URL_SAFE_NO_PAD.encode(vec![0x11; 256]);
        let forged = format!("{header}.{payload}.{fake_sig}");
        let err = verify_jwt_with_rsa_der(&forged, &der, "demo-proj", 2000).unwrap_err();
        assert!(err.contains("signature verification failed"));
    }

    #[test]
    fn browser_signin_html_validates_port_and_state() {
        let cfg = FirebaseAuthConfig {
            project_id: "demo-proj".into(),
            web_api_key: Some("AIzaDemoKey".into()),
            auth_domain: "demo-proj.firebaseapp.com".into(),
        };
        assert!(render_browser_signin_html(&cfg, 80, "validstate123").is_err());
        assert!(render_browser_signin_html(&cfg, 45678, "<script>").is_err());
        let html = render_browser_signin_html(&cfg, 45678, "abc12345_state").unwrap();
        assert!(html.contains("demo-proj.firebaseapp.com"));
        assert!(html.contains("http://127.0.0.1:45678/callback"));
    }
}
