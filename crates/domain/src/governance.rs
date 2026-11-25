//! Version B governance, security, branching, deep links, TOTP MFA, tamper-evident audit,
//! and signed update verification (CO05, CO08, CO09, SY10, SC09, SC10, OP08).
use crate::security::{RedactionPolicy, sanitize_headers, sanitize_text};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

type HmacSha256 = Hmac<Sha256>;

/// CO05: Extract normalized `@mention` handles/emails from comment text.
pub fn parse_mentions(text: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for tok in text.split_whitespace() {
        let Some(after) = tok.strip_prefix('@') else {
            continue;
        };
        let clean = after.trim_end_matches(&[',', '.', ';', ':', '!', '?', ')', ']'][..]);
        if clean.len() >= 2
            && clean
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'@'))
        {
            let norm = clean.to_ascii_lowercase();
            if seen.insert(norm.clone()) {
                out.push(norm);
            }
        }
    }
    out
}

/// CO09: Internal deep-link parser (`swaggen://workspace/<ws>/<kind>/<entity>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwaggenDeepLink {
    pub workspace_id: String,
    pub kind: String,
    pub entity_id: String,
}

pub fn parse_deep_link(raw: &str) -> Result<SwaggenDeepLink, String> {
    let trimmed = raw.trim();
    let rest = trimmed
        .strip_prefix("swaggen://workspace/")
        .ok_or_else(|| {
            "Unsupported deep link scheme (expected swaggen://workspace/...)".to_owned()
        })?;
    let parts: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() != 3 {
        return Err("Deep link must have form swaggen://workspace/<ws>/<kind>/<entity>".into());
    }
    for p in &parts {
        if *p == "."
            || *p == ".."
            || p.contains("..")
            || !p
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        {
            return Err("Deep link segment contains unsafe characters".into());
        }
    }
    Ok(SwaggenDeepLink {
        workspace_id: parts[0].to_owned(),
        kind: parts[1].to_owned(),
        entity_id: parts[2].to_owned(),
    })
}

/// CO08: Sanitized share package stripping tokens, cookies, local file paths, and extra keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanitizedSharePackage {
    pub title: String,
    pub method: String,
    pub redacted_url: String,
    pub redacted_headers: Vec<(String, String)>,
    pub redacted_body: String,
    pub status_code: Option<u16>,
    pub duration_ms: Option<u64>,
    pub total_redactions: usize,
    pub includes_secrets_or_cookies: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn create_sanitized_share_package(
    title: &str,
    method: &str,
    raw_url: &str,
    headers: &[(String, String)],
    body_preview: &str,
    status_code: Option<u16>,
    duration_ms: Option<u64>,
    policy: &RedactionPolicy,
) -> SanitizedSharePackage {
    let url_san = sanitize_text(raw_url, policy);
    let (red_hdrs, hdr_red_count) = sanitize_headers(headers, policy);
    let body_san = sanitize_text(body_preview, policy);
    SanitizedSharePackage {
        title: title.trim().to_owned(),
        method: method.to_owned(),
        redacted_url: url_san.sanitized_text,
        redacted_headers: red_hdrs
            .into_iter()
            .filter(|(k, _)| {
                !k.eq_ignore_ascii_case("cookie") && !k.eq_ignore_ascii_case("set-cookie")
            })
            .collect(),
        redacted_body: body_san.sanitized_text,
        status_code,
        duration_ms,
        total_redactions: url_san.redacted_count + hdr_red_count + body_san.redacted_count,
        includes_secrets_or_cookies: false,
    }
}

/// SY10: Lightweight project branch & explicit non-destructive merge evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchChange {
    pub entity_id: String,
    pub base_hash: String,
    pub target_hash: String,
    pub branch_hash: String,
    pub branch_payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchMergeEvaluation {
    pub clean_apply: Vec<BranchChange>,
    pub conflicts: Vec<BranchChange>,
    pub approved_by: Vec<String>,
}

pub fn evaluate_branch_merge(
    changes: &[BranchChange],
    approved_by: Vec<String>,
) -> BranchMergeEvaluation {
    let mut clean_apply = Vec::new();
    let mut conflicts = Vec::new();
    for ch in changes {
        if ch.target_hash == ch.base_hash || ch.target_hash == ch.branch_hash {
            clean_apply.push(ch.clone());
        } else {
            conflicts.push(ch.clone());
        }
    }
    BranchMergeEvaluation {
        clean_apply,
        conflicts,
        approved_by,
    }
}

/// SC09: RFC 6238 TOTP (HMAC-SHA256) + SHA-256 hashed single-use recovery codes.
pub fn encode_base32(data: &[u8]) -> String {
    const ALPH: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::new();
    let mut buf = 0u32;
    let mut bits = 0usize;
    for &b in data {
        buf = (buf << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPH[((buf >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPH[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

pub fn decode_base32(input: &str) -> Result<Vec<u8>, String> {
    let mut buf = 0u32;
    let mut bits = 0usize;
    let mut out = Vec::new();
    for c in input.trim().bytes() {
        if c == b'=' || c == b' ' {
            continue;
        }
        let v = match c.to_ascii_uppercase() {
            b'A'..=b'Z' => c.to_ascii_uppercase() - b'A',
            b'2'..=b'7' => 26 + (c - b'2'),
            _ => return Err("Invalid Base32 TOTP secret character".into()),
        };
        buf = (buf << 5) | u32::from(v);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

pub fn generate_totp_sha256(secret_b32: &str, unix_secs: i64) -> Result<String, String> {
    let key = decode_base32(secret_b32)?;
    let step = (unix_secs.max(0) as u64) / 30;
    let mut mac = HmacSha256::new_from_slice(&key).map_err(|e| e.to_string())?;
    mac.update(&step.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let bin = ((u32::from(digest[offset]) & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    Ok(format!("{:06}", bin % 1_000_000))
}

pub fn verify_totp_sha256(
    secret_b32: &str,
    candidate: &str,
    unix_secs: i64,
    skew_steps: i64,
) -> bool {
    let clean = candidate.trim();
    if clean.len() != 6 || !clean.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    for delta in -skew_steps..=skew_steps {
        let t = unix_secs + delta * 30;
        if let Ok(expected) = generate_totp_sha256(secret_b32, t)
            && constant_time_eq(expected.as_bytes(), clean.as_bytes())
        {
            return true;
        }
    }
    false
}

pub fn hash_recovery_code(code: &str) -> String {
    hex::encode(Sha256::digest(code.trim().as_bytes()))
}

pub fn verify_and_consume_recovery_code(hashed_codes: &mut Vec<String>, candidate: &str) -> bool {
    let target = hash_recovery_code(candidate);
    if let Some(pos) = hashed_codes
        .iter()
        .position(|h| constant_time_eq(h.as_bytes(), target.as_bytes()))
    {
        hashed_codes.remove(pos);
        true
    } else {
        false
    }
}

pub fn build_otpauth_uri(issuer: &str, email: &str, secret_b32: &str) -> String {
    format!(
        "otpauth://totp/{issuer}:{email}?secret={secret_b32}&issuer={issuer}&algorithm=SHA256&digits=6&period=30"
    )
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// SC10: Hash-chained redacted audit export + honest WORM boundary notice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditExportRow {
    pub event_id: String,
    pub actor_email: String,
    pub action: String,
    pub target_id: String,
    pub detail_redacted: String,
    pub created_at_unix: i64,
    pub chain_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditExportBundle {
    pub retention_days: u32,
    pub tamper_limit_notice: String,
    pub rows: Vec<AuditExportRow>,
}

pub fn build_hash_chained_audit_export(
    events: &[(String, String, String, String, String, i64)],
    retention_days: u32,
    cutoff_unix: i64,
) -> AuditExportBundle {
    let mut prev = String::from("0000000000000000000000000000000000000000000000000000000000000000");
    let mut rows = Vec::new();
    for (eid, email, action, target, detail, ts) in events {
        if *ts < cutoff_unix {
            continue;
        }
        let mut h = Sha256::new();
        h.update(prev.as_bytes());
        h.update(eid.as_bytes());
        h.update(action.as_bytes());
        h.update(target.as_bytes());
        h.update(ts.to_le_bytes());
        let chain = hex::encode(h.finalize());
        prev = chain.clone();
        rows.push(AuditExportRow {
            event_id: eid.clone(),
            actor_email: email.clone(),
            action: action.clone(),
            target_id: target.clone(),
            detail_redacted: sanitize_text(detail, &RedactionPolicy::default()).sanitized_text,
            created_at_unix: *ts,
            chain_sha256: chain,
        });
    }
    AuditExportBundle {
        retention_days,
        tamper_limit_notice: "Hash-chain detects intra-export tampering; full immutability against root DB modification requires forwarding to external WORM/SIEM storage.".into(),
        rows,
    }
}

/// OP08: Signed self-hosted update manifest verifier rejecting modified metadata or artifact payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedUpdateManifest {
    pub version: String,
    pub channel: String,
    pub artifact_name: String,
    pub artifact_sha256: String,
    pub published_at_unix: i64,
    pub manifest_hmac_sha256: String,
}

impl SignedUpdateManifest {
    fn canonical_message(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.version,
            self.channel,
            self.artifact_name,
            self.artifact_sha256,
            self.published_at_unix
        )
    }

    pub fn sign_with_key(&mut self, signing_key: &[u8]) -> Result<(), String> {
        let mut mac = HmacSha256::new_from_slice(signing_key).map_err(|e| e.to_string())?;
        mac.update(self.canonical_message().as_bytes());
        self.manifest_hmac_sha256 = hex::encode(mac.finalize().into_bytes());
        Ok(())
    }
}

pub fn verify_signed_update(
    manifest: &SignedUpdateManifest,
    artifact_bytes: &[u8],
    signing_key: &[u8],
) -> Result<(), String> {
    let mut mac = HmacSha256::new_from_slice(signing_key).map_err(|e| e.to_string())?;
    mac.update(manifest.canonical_message().as_bytes());
    let expected_sig = hex::encode(mac.finalize().into_bytes());
    if !constant_time_eq(
        expected_sig.as_bytes(),
        manifest
            .manifest_hmac_sha256
            .trim()
            .to_ascii_lowercase()
            .as_bytes(),
    ) {
        return Err("Update manifest signature verification failed; update aborted".into());
    }
    let actual_hash = hex::encode(Sha256::digest(artifact_bytes));
    if !constant_time_eq(
        actual_hash.as_bytes(),
        manifest
            .artifact_sha256
            .trim()
            .to_ascii_lowercase()
            .as_bytes(),
    ) {
        return Err("Update artifact SHA-256 mismatch; tampered binary rejected".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions_deep_links_totp_recovery_and_signed_update_work() {
        assert_eq!(
            parse_mentions("Hey @Alice@example.com, please review with @bob-dev!"),
            vec!["alice@example.com", "bob-dev"]
        );
        let dl = parse_deep_link("swaggen://workspace/team-api/request/req-1").unwrap();
        assert_eq!(dl.workspace_id, "team-api");
        assert!(parse_deep_link("swaggen://workspace/bad/../x").is_err());

        let pkg = create_sanitized_share_package(
            "Share",
            "GET",
            "https://api.test/v1?api_key=RAW123",
            &[
                ("Authorization".into(), "Bearer RAW_TOK".into()),
                ("Cookie".into(), "sid=99".into()),
            ],
            r#"{"secret":"abc"}"#,
            Some(200),
            Some(12),
            &RedactionPolicy::default(),
        );
        assert!(!pkg.redacted_url.contains("RAW123"));
        assert_eq!(pkg.redacted_headers.len(), 1);

        let sec_b32 = encode_base32(b"12345678901234567890");
        let code = generate_totp_sha256(&sec_b32, 1_700_000_000).unwrap();
        assert!(verify_totp_sha256(&sec_b32, &code, 1_700_000_000, 1));
        assert!(!verify_totp_sha256(&sec_b32, "000000", 1_700_000_000, 0));

        let mut codes = vec![hash_recovery_code("REC-1111")];
        assert!(verify_and_consume_recovery_code(&mut codes, "REC-1111"));
        assert!(!verify_and_consume_recovery_code(&mut codes, "REC-1111"));

        let payload = b"swaggen-binary-payload-v0.2.0";
        let mut man = SignedUpdateManifest {
            version: "0.2.0".into(),
            channel: "stable".into(),
            artifact_name: "swaggen-desktop.zip".into(),
            artifact_sha256: hex::encode(Sha256::digest(payload)),
            published_at_unix: 1000,
            manifest_hmac_sha256: String::new(),
        };
        man.sign_with_key(b"release-signing-key").unwrap();
        assert!(verify_signed_update(&man, payload, b"release-signing-key").is_ok());
        assert!(verify_signed_update(&man, b"tampered", b"release-signing-key").is_err());
        man.version = "0.2.1".into();
        assert!(verify_signed_update(&man, payload, b"release-signing-key").is_err());

        let eval = evaluate_branch_merge(
            &[
                BranchChange {
                    entity_id: "e1".into(),
                    base_hash: "h1".into(),
                    target_hash: "h1".into(),
                    branch_hash: "h2".into(),
                    branch_payload: "{}".into(),
                },
                BranchChange {
                    entity_id: "e2".into(),
                    base_hash: "h1".into(),
                    target_hash: "h3".into(),
                    branch_hash: "h2".into(),
                    branch_payload: "{}".into(),
                },
            ],
            vec!["alice".into()],
        );
        assert_eq!(eval.clean_apply.len(), 1);
        assert_eq!(eval.conflicts.len(), 1);
    }
}
