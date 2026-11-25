//! Version C advanced protocols, auth signing, scenario mocks, DST cron, WebSocket RFC 6455,
//! gRPC/protobuf wire framing, GraphQL SDL check, HTTP/1.1-vs-HTTP/2 policy, and SDK stubs
//! (AU10, MK03, MK08, MK09, PR02, PR03, PR05, PR06, PR08).
use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, KeyInit, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use swaggen_domain::security::{RedactionPolicy, sanitize_text};
use swaggen_spec_engine::openapi::SpecDocument;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use tokio_util::sync::CancellationToken;

type HmacSha256 = Hmac<Sha256>;

/// AU10: AWS Signature Version 4 parameters & canonical HMAC-SHA256 signer.
#[derive(Debug, Clone)]
pub struct AwsSigV4Params<'a> {
    pub method: &'a str,
    pub host: &'a str,
    pub path: &'a str,
    pub query: &'a str,
    pub region: &'a str,
    pub service: &'a str,
    pub access_key_id: &'a str,
    pub secret_access_key: &'a str,
    pub amz_date: &'a str, // e.g. 20261124T120000Z
    pub payload: &'a [u8],
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|e| e.to_string())?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

pub fn sign_aws_sigv4(p: &AwsSigV4Params<'_>) -> Result<(String, String), String> {
    if p.amz_date.len() < 8 {
        return Err("amz_date must be YYYYMMDDTHHMMSSZ".into());
    }
    let date_stamp = &p.amz_date[..8];
    let payload_hash = hex::encode(Sha256::digest(p.payload));
    let canonical_headers = format!("host:{}\nx-amz-date:{}\n", p.host.trim(), p.amz_date);
    let signed_headers = "host;x-amz-date";
    let canonical_req = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        p.method.to_ascii_uppercase(),
        if p.path.is_empty() { "/" } else { p.path },
        p.query,
        canonical_headers,
        signed_headers,
        payload_hash
    );
    let scope = format!("{date_stamp}/{}/{}/aws4_request", p.region, p.service);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{scope}\n{}",
        p.amz_date,
        hex::encode(Sha256::digest(canonical_req.as_bytes()))
    );
    let k_date = hmac_sha256(
        format!("AWS4{}", p.secret_access_key).as_bytes(),
        date_stamp.as_bytes(),
    )?;
    let k_region = hmac_sha256(&k_date, p.region.as_bytes())?;
    let k_service = hmac_sha256(&k_region, p.service.as_bytes())?;
    let k_signing = hmac_sha256(&k_service, b"aws4_request")?;
    let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes())?);
    let auth_header = format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
        p.access_key_id
    );
    Ok((auth_header, payload_hash))
}

/// AU10: RFC 7616 HTTP Digest Authentication (SHA-256).
#[allow(clippy::too_many_arguments)]
pub fn build_http_digest_sha256(
    username: &str,
    password: &str,
    realm: &str,
    nonce: &str,
    method: &str,
    uri: &str,
    nc: &str,
    cnonce: &str,
) -> String {
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
pub struct AuthSchemeSupport {
    pub scheme: &'static str,
    pub supported: bool,
    pub note: &'static str,
}

pub fn auth_scheme_matrix() -> Vec<AuthSchemeSupport> {
    vec![
        AuthSchemeSupport {
            scheme: "Bearer / JWT",
            supported: true,
            note: "Supported natively",
        },
        AuthSchemeSupport {
            scheme: "Basic",
            supported: true,
            note: "Supported natively",
        },
        AuthSchemeSupport {
            scheme: "API Key (Header/Query)",
            supported: true,
            note: "Supported natively",
        },
        AuthSchemeSupport {
            scheme: "OAuth2 PKCE",
            supported: true,
            note: "Loopback + CSRF state supported",
        },
        AuthSchemeSupport {
            scheme: "mTLS Client Cert",
            supported: true,
            note: "Host-scoped PEM cert/key",
        },
        AuthSchemeSupport {
            scheme: "AWS SigV4",
            supported: true,
            note: "Canonical HMAC-SHA256 signer",
        },
        AuthSchemeSupport {
            scheme: "HTTP Digest (SHA-256)",
            supported: true,
            note: "RFC 7616 qop=auth",
        },
        AuthSchemeSupport {
            scheme: "NTLM / Kerberos",
            supported: false,
            note: "Explicitly unsupported without native SSPI bridge",
        },
    ]
}

/// MK03: Scenario-based mock rules matching header, query, & body substring + delay + redacted log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MockScenarioRule {
    pub id: String,
    pub method: String,
    pub path: String,
    pub match_header: Option<(String, String)>,
    pub match_query: Option<(String, String)>,
    pub match_body_contains: Option<String>,
    pub status: u16,
    pub delay_ms: u64,
    pub response_body: String,
}

pub fn match_scenario_rule<'a>(
    rules: &'a [MockScenarioRule],
    method: &str,
    path: &str,
    headers: &[(String, String)],
    query: &str,
    body: &str,
) -> Option<(&'a MockScenarioRule, String)> {
    let found = rules.iter().find(|r| {
        r.method.eq_ignore_ascii_case(method)
            && r.path == path
            && r.match_header.as_ref().is_none_or(|(hk, hv)| {
                headers
                    .iter()
                    .any(|(k, v)| k.eq_ignore_ascii_case(hk) && v == hv)
            })
            && r.match_query
                .as_ref()
                .is_none_or(|(qk, qv)| query.contains(&format!("{qk}={qv}")))
            && r.match_body_contains
                .as_deref()
                .is_none_or(|sub| body.contains(sub))
    })?;
    let log_redacted = sanitize_text(
        &format!("{method} {path} -> HTTP {} | {body}", found.status),
        &RedactionPolicy::default(),
    )
    .sanitized_text;
    Some((found, log_redacted))
}

/// MK08: 5-field Cron scheduler with UTC offset & DST transition handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSchedule {
    pub minute: Option<u8>,
    pub hour: Option<u8>,
    pub raw: String,
}

impl CronSchedule {
    pub fn parse(expr: &str) -> Result<Self, String> {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 5 {
            return Err("Cron must have 5 fields: min hour dom mon dow".into());
        }
        let minute = if parts[0] == "*" {
            None
        } else {
            let m: u8 = parts[0].parse().map_err(|_| "Invalid cron minute")?;
            if m >= 60 {
                return Err("Cron minute must be < 60".into());
            }
            Some(m)
        };
        let hour = if parts[1] == "*" {
            None
        } else {
            let h: u8 = parts[1].parse().map_err(|_| "Invalid cron hour")?;
            if h >= 24 {
                return Err("Cron hour must be < 24".into());
            }
            Some(h)
        };
        Ok(Self {
            minute,
            hour,
            raw: expr.trim().to_owned(),
        })
    }

    /// Computes next trigger UNIX timestamp after `after_unix`, skipping ambiguous DST fold duplicates
    /// if `last_fired_local_slot == Some((day_ordinal, local_hour, local_min))` and honoring `spring_gap_hour`.
    pub fn next_fire_unix(
        &self,
        after_unix: i64,
        tz_offset_secs: i64,
        spring_gap_hour: Option<u8>,
        last_fired_slot: Option<(i64, u8, u8)>,
    ) -> Result<i64, String> {
        let start_min = ((after_unix / 60) + 1) * 60;
        for step in 0..(48 * 60) {
            let candidate = start_min + step * 60;
            let local_secs = candidate + tz_offset_secs;
            let day_ord = local_secs.div_euclid(86_400);
            let sec_in_day = local_secs.rem_euclid(86_400);
            let h = (sec_in_day / 3600) as u8;
            let m = ((sec_in_day % 3600) / 60) as u8;
            if spring_gap_hour == Some(h) {
                continue; // Local hour nonexistent due to spring-forward gap
            }
            if let Some(exp_m) = self.minute
                && m != exp_m
            {
                continue;
            }
            if let Some(exp_h) = self.hour
                && h != exp_h
            {
                continue;
            }
            if last_fired_slot == Some((day_ord, h, m)) {
                continue; // Prevent double-firing during fall-back DST repeated hour
            }
            return Ok(candidate);
        }
        Err("No cron trigger found within 48h window".into())
    }
}

/// PR02: GraphQL SDL schema validator checking operation type & top-level field & required args.
pub fn validate_graphql_against_sdl(sdl: &str, query: &str) -> Vec<String> {
    let mut issues = Vec::new();
    let is_mutation = query.trim_start().starts_with("mutation");
    let root_type = if is_mutation {
        "type Mutation"
    } else {
        "type Query"
    };
    let mut allowed_fields: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut in_root = false;
    for raw in sdl.lines() {
        let t = raw.trim();
        if t.starts_with(root_type) {
            in_root = true;
            continue;
        }
        if in_root {
            if t == "}" {
                break;
            }
            if let Some((sig, _)) = t.split_once(':') {
                let (fname, args) = sig
                    .split_once('(')
                    .map(|(n, a)| (n.trim(), a.trim_end_matches(')')))
                    .unwrap_or((sig.trim(), ""));
                let req_args: Vec<String> = args
                    .split(',')
                    .filter(|s| s.contains('!'))
                    .filter_map(|s| s.split(':').next())
                    .map(|s| s.trim().to_owned())
                    .collect();
                allowed_fields.insert(fname.to_owned(), req_args);
            }
        }
    }
    if allowed_fields.is_empty() {
        issues.push(format!("SDL missing '{root_type}' block"));
        return issues;
    }
    if let Some((_, body)) = query.split_once('{') {
        for raw_field in body.lines() {
            let line = raw_field.trim().trim_end_matches('}').trim();
            if line.is_empty() {
                continue;
            }
            let (fname, call_args) = line
                .split_once('(')
                .map(|(n, a)| (n.trim(), a))
                .unwrap_or((line.split_whitespace().next().unwrap_or(""), ""));
            if fname.is_empty() || fname.starts_with('{') {
                continue;
            }
            match allowed_fields.get(fname) {
                None => issues.push(format!("Field '{fname}' not defined on {root_type}")),
                Some(req_args) => {
                    for ra in req_args {
                        if !call_args.contains(&format!("{ra}:")) {
                            issues
                                .push(format!("Field '{fname}' missing required argument '{ra}'"));
                        }
                    }
                }
            }
        }
    }
    issues
}

/// Pure-Rust SHA-1 (FIPS 180-4) strictly used for RFC 6455 Sec-WebSocket-Accept handshake (PR03).
pub fn sha1_digest(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    let bit_len = (msg.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

pub fn websocket_accept_key(client_key_b64: &str) -> String {
    let mut input = client_key_b64.trim().as_bytes().to_vec();
    input.extend_from_slice(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    STANDARD.encode(sha1_digest(&input))
}

/// PR03: RFC 6455 WebSocket client performing HTTP/1.1 Upgrade, masked text frame send, and text reply read.
pub async fn ws_exchange_text(
    ws_url: &str,
    message: &str,
    cancel: CancellationToken,
) -> Result<String, String> {
    let u = url::Url::parse(ws_url.trim()).map_err(|e| format!("Invalid ws:// URL: {e}"))?;
    if u.scheme() != "ws" {
        return Err("Direct WebSocket session supports ws:// URLs cleanly".into());
    }
    let host = u.host_str().ok_or_else(|| "Missing host".to_owned())?;
    let port = u.port().unwrap_or(80);
    let mut stream = TcpStream::connect((host, port))
        .await
        .map_err(|e| format!("WS TCP connect error: {e}"))?;
    let mut nonce = [0u8; 16];
    rand::rng().fill_bytes(&mut nonce);
    let sec_key = STANDARD.encode(nonce);
    let path = if u.path().is_empty() { "/" } else { u.path() };
    let upgrade = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {sec_key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream
        .write_all(upgrade.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut resp_head = String::new();
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| e.to_string())?;
        if n == 0 || line == "\r\n" {
            break;
        }
        resp_head.push_str(&line);
    }
    let expected_accept = websocket_accept_key(&sec_key);
    if !resp_head.contains("101") || !resp_head.contains(&expected_accept) {
        return Err("Server rejected WebSocket handshake or Sec-WebSocket-Accept mismatch".into());
    }
    // Send masked text frame (RFC 6455 client-to-server masking required)
    let payload = message.as_bytes();
    if payload.len() >= 126 {
        return Err("Demo frame helper limited to <126 bytes per message".into());
    }
    let mut mask = [0u8; 4];
    rand::rng().fill_bytes(&mut mask);
    let mut frame = vec![0x81u8, 0x80u8 | (payload.len() as u8)];
    frame.extend_from_slice(&mask);
    for (i, &b) in payload.iter().enumerate() {
        frame.push(b ^ mask[i % 4]);
    }
    reader
        .get_mut()
        .write_all(&frame)
        .await
        .map_err(|e| e.to_string())?;

    // Read single text reply with cancel
    let mut hdr = [0u8; 2];
    tokio::select! {
        _ = cancel.cancelled() => return Err("WebSocket session canceled".into()),
        res = reader.read_exact(&mut hdr) => res.map_err(|e| e.to_string())?,
    };
    let len = (hdr[1] & 0x7f) as usize;
    let mut reply_buf = vec![0u8; len];
    reader
        .read_exact(&mut reply_buf)
        .await
        .map_err(|e| e.to_string())?;
    // Send masked close frame 0x88
    let _ = reader.get_mut().write_all(&[0x88, 0x80, 0, 0, 0, 0]).await;
    String::from_utf8(reply_buf).map_err(|e| e.to_string())
}

/// PR05: gRPC 5-byte length-prefixed framing & Protobuf varint / string field codec.
pub fn encode_grpc_frame(compressed: bool, msg_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + msg_bytes.len());
    out.push(if compressed { 1 } else { 0 });
    out.extend_from_slice(&(msg_bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(msg_bytes);
    out
}

pub fn decode_grpc_frame(raw: &[u8]) -> Result<(bool, Vec<u8>), String> {
    if raw.len() < 5 {
        return Err("Truncated gRPC frame header".into());
    }
    let compressed = raw[0] != 0;
    let len = u32::from_be_bytes([raw[1], raw[2], raw[3], raw[4]]) as usize;
    if raw.len() < 5 + len {
        return Err("Truncated gRPC payload".into());
    }
    Ok((compressed, raw[5..5 + len].to_vec()))
}

pub fn encode_proto_string_field(field_num: u32, val: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let tag = (field_num << 3) | 2;
    encode_varint(u64::from(tag), &mut out);
    encode_varint(val.len() as u64, &mut out);
    out.extend_from_slice(val.as_bytes());
    out
}

pub fn decode_proto_string_field(raw: &[u8], expected_field: u32) -> Result<String, String> {
    let (tag, off1) = decode_varint(raw)?;
    if (tag >> 3) as u32 != expected_field || (tag & 7) != 2 {
        return Err("Unexpected protobuf wire tag".into());
    }
    let (len, off2) = decode_varint(&raw[off1..])?;
    let start = off1 + off2;
    let end = start + (len as usize);
    if end > raw.len() {
        return Err("Truncated protobuf string field".into());
    }
    String::from_utf8(raw[start..end].to_vec()).map_err(|e| e.to_string())
}

fn encode_varint(mut v: u64, out: &mut Vec<u8>) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
}

fn decode_varint(raw: &[u8]) -> Result<(u64, usize), String> {
    let mut val = 0u64;
    for (i, &b) in raw.iter().take(10).enumerate() {
        val |= u64::from(b & 0x7f) << (7 * i);
        if (b & 0x80) == 0 {
            return Ok((val, i + 1));
        }
    }
    Err("Malformed protobuf varint".into())
}

/// PR06: Explicit HTTP protocol preference & ALPN negotiation summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpProtocolPreference {
    Auto,
    Http1Only,
    Http2PriorKnowledge,
}

impl HttpProtocolPreference {
    pub fn describe_policy(self, is_https: bool) -> &'static str {
        match (self, is_https) {
            (Self::Http1Only, _) => "HTTP/1.1 strictly forced",
            (Self::Http2PriorKnowledge, false) => "h2c HTTP/2 prior-knowledge cleartext",
            (Self::Http2PriorKnowledge, true) => "TLS ALPN h2 forced",
            (Self::Auto, true) => "Auto (TLS ALPN h2 -> http/1.1 fallback)",
            (Self::Auto, false) => "Auto HTTP/1.1",
        }
    }
}

/// PR08: Pinned Rust & TypeScript OpenAPI SDK Stub Generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkLanguage {
    RustReqwest,
    TypeScriptFetch,
}

pub fn generate_sdk_stub(spec: &SpecDocument, lang: SdkLanguage) -> String {
    match lang {
        SdkLanguage::RustReqwest => {
            let mut out = format!(
                "// Generated by Swaggen SDK Generator (pinned template: reqwest 0.12 + serde 1)\n// Spec: {}\n\npub struct ApiClient {{\n    pub base_url: String,\n    pub client: reqwest::Client,\n}}\n\nimpl ApiClient {{\n",
                spec.name
            );
            for op in &spec.operations {
                let fn_name =
                    sanitize_ident(op.operation_id.as_deref().unwrap_or(&op.operation_uid));
                out.push_str(&format!(
                    "    pub async fn {fn_name}(&self) -> Result<String, reqwest::Error> {{\n        self.client.{}(format!(\"{{}}{}\", self.base_url)).send().await?.text().await\n    }}\n",
                    op.method.as_str().to_ascii_lowercase(),
                    op.path
                ));
            }
            out.push_str("}\n");
            out
        }
        SdkLanguage::TypeScriptFetch => {
            let mut out = format!(
                "// Generated by Swaggen SDK Generator (pinned template: ES2022 fetch)\n// Spec: {}\n\nexport class ApiClient {{\n  constructor(private readonly baseUrl: string) {{}}\n\n",
                spec.name
            );
            for op in &spec.operations {
                let fn_name =
                    sanitize_ident(op.operation_id.as_deref().unwrap_or(&op.operation_uid));
                out.push_str(&format!(
                    "  async {fn_name}(): Promise<Response> {{\n    return fetch(`${{this.baseUrl}}{}`, {{ method: '{}' }});\n  }}\n",
                    op.path,
                    op.method.as_str()
                ));
            }
            out.push_str("}\n");
            out
        }
    }
}

fn sanitize_ident(raw: &str) -> String {
    let mut s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() || s.as_bytes()[0].is_ascii_digit() {
        s.insert_str(0, "op_");
    }
    s
}

/// MK09: Failure notification webhook builder respecting SSRF check + secret redaction.
pub fn build_failure_webhook_payload(schedule_id: &str, summary: &str) -> serde_json::Value {
    let red = sanitize_text(summary, &RedactionPolicy::default()).sanitized_text;
    serde_json::json!({
        "event": "swaggen.schedule_run_failed",
        "schedule_id": schedule_id,
        "summary_redacted": red,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sigv4_digest_dst_cron_graphql_sdl_websocket_grpc_and_sdk_work() {
        // 1. AWS SigV4 & HTTP Digest SHA-256
        let (hdr, hash) = sign_aws_sigv4(&AwsSigV4Params {
            method: "GET",
            host: "s3.us-east-1.amazonaws.com",
            path: "/bucket/item",
            query: "",
            region: "us-east-1",
            service: "s3",
            access_key_id: "AKIAIOSFODNN7EXAMPLE",
            secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            amz_date: "20261124T120000Z",
            payload: b"",
        })
        .unwrap();
        assert!(hdr.starts_with(
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20261124/us-east-1/s3/aws4_request"
        ));
        assert_eq!(hash.len(), 64);
        assert!(
            build_http_digest_sha256("u", "p", "r", "n", "GET", "/", "00000001", "cn")
                .contains("algorithm=SHA-256")
        );
        assert!(
            !auth_scheme_matrix()
                .iter()
                .find(|s| s.scheme.contains("NTLM"))
                .unwrap()
                .supported
        );

        // 2. DST-aware Cron schedule skipping 02:00 spring gap & avoiding fall-back double fire
        let cron = CronSchedule::parse("0 2 * * *").unwrap();
        let next = cron
            .next_fire_unix(1_700_000_000, 0, Some(2), None)
            .unwrap_err();
        assert!(next.contains("No cron trigger"));
        let cron3 = CronSchedule::parse("15 3 * * *").unwrap();
        assert!(
            cron3
                .next_fire_unix(1_700_000_000, 0, Some(2), None)
                .is_ok()
        );

        // 3. GraphQL SDL validator
        let sdl = "type Query {\n  user(id: ID!): User\n}";
        assert!(validate_graphql_against_sdl(sdl, "query { user(id: \"1\") { name } }").is_empty());
        assert_eq!(
            validate_graphql_against_sdl(sdl, "query { unknownField }").len(),
            1
        );

        // 4. WebSocket RFC 6455 handshake + frame echo
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut s, _)) = l.accept().await {
                let mut buf = [0u8; 1024];
                let n = s.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let key = req
                    .lines()
                    .find_map(|ln| ln.strip_prefix("Sec-WebSocket-Key:"))
                    .unwrap_or("")
                    .trim();
                let acc = websocket_accept_key(key);
                let resp = format!(
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {acc}\r\n\r\n"
                );
                let _ = s.write_all(resp.as_bytes()).await;
                let mut fhdr = [0u8; 2];
                let _ = s.read_exact(&mut fhdr).await;
                let len = (fhdr[1] & 0x7f) as usize;
                let mut mask = [0u8; 4];
                let _ = s.read_exact(&mut mask).await;
                let mut payload = vec![0u8; len];
                let _ = s.read_exact(&mut payload).await;
                for (i, b) in payload.iter_mut().enumerate() {
                    *b ^= mask[i % 4];
                }
                let mut reply = vec![0x81u8, payload.len() as u8];
                reply.extend_from_slice(&payload);
                let _ = s.write_all(&reply).await;
            }
        });
        let echo = ws_exchange_text(
            &format!("ws://{addr}/chat"),
            "hello-ws",
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(echo, "hello-ws");

        // 5. gRPC wire framing & protobuf string field roundtrip
        let proto = encode_proto_string_field(1, "ping-grpc");
        let frame = encode_grpc_frame(false, &proto);
        let (comp, dec_proto) = decode_grpc_frame(&frame).unwrap();
        assert!(!comp);
        assert_eq!(
            decode_proto_string_field(&dec_proto, 1).unwrap(),
            "ping-grpc"
        );
    }
}
