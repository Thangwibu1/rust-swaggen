//! Redaction, untrusted-content sanitization, and desktop privacy/diagnostic policy.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Default sensitive header names matched case-insensitively.
const SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
    "x-api-key",
    "x-auth-token",
    "x-csrf-token",
    "x-xsrf-token",
    "www-authenticate",
    "proxy-authenticate",
];

/// Default sensitive query/JSON/KV field names matched case-insensitively.
const SENSITIVE_FIELDS: &[&str] = &[
    "password",
    "passwd",
    "pwd",
    "secret",
    "client_secret",
    "private_key",
    "token",
    "access_token",
    "refresh_token",
    "id_token",
    "api_key",
    "apikey",
    "auth",
    "authorization",
    "cookie",
    "session_id",
    "sig",
    "signature",
    "jwt",
];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionPolicy {
    pub extra_fields: Vec<String>,
    pub extra_literal_values: Vec<String>,
}

impl RedactionPolicy {
    pub fn is_sensitive_header(&self, name: &str) -> bool {
        let lower = name.trim().to_ascii_lowercase();
        SENSITIVE_HEADERS.contains(&lower.as_str())
            || self
                .extra_fields
                .iter()
                .any(|f| !f.trim().is_empty() && f.trim().eq_ignore_ascii_case(&lower))
    }

    pub fn is_sensitive_field(&self, name: &str) -> bool {
        let lower = name.trim().to_ascii_lowercase();
        SENSITIVE_FIELDS.contains(&lower.as_str())
            || self
                .extra_fields
                .iter()
                .any(|f| !f.trim().is_empty() && f.trim().eq_ignore_ascii_case(&lower))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionPreview {
    pub sanitized_text: String,
    pub redacted_count: usize,
    pub matched_rules: Vec<String>,
    pub manual_review_warning: &'static str,
}

pub fn sanitize_headers(
    headers: &[(String, String)],
    policy: &RedactionPolicy,
) -> (Vec<(String, String)>, usize) {
    let mut count = 0usize;
    let out = headers
        .iter()
        .map(|(k, v)| {
            if policy.is_sensitive_header(k) || contains_literal_secret(v, policy) {
                count += 1;
                (k.clone(), "[REDACTED]".to_owned())
            } else {
                (k.clone(), v.clone())
            }
        })
        .collect();
    (out, count)
}

pub fn sanitize_url_query(input: &str, policy: &RedactionPolicy) -> (String, usize) {
    let Some((base, query)) = input.split_once('?') else {
        return (input.to_owned(), 0);
    };
    let (query_part, fragment) = match query.split_once('#') {
        Some((q, f)) => (q, Some(f)),
        None => (query, None),
    };
    let mut count = 0usize;
    let mut rebuilt = Vec::new();
    for pair in query_part.split('&') {
        if pair.is_empty() {
            rebuilt.push(String::new());
            continue;
        }
        if let Some((key, val)) = pair.split_once('=') {
            if policy.is_sensitive_field(key) || contains_literal_secret(val, policy) {
                count += 1;
                rebuilt.push(format!("{key}=[REDACTED]"));
            } else {
                rebuilt.push(pair.to_owned());
            }
        } else {
            rebuilt.push(pair.to_owned());
        }
    }
    let mut out = format!("{base}?{}", rebuilt.join("&"));
    if let Some(frag) = fragment {
        out.push('#');
        out.push_str(frag);
    }
    (out, count)
}

pub fn sanitize_text(input: &str, policy: &RedactionPolicy) -> RedactionPreview {
    let mut rules = BTreeSet::new();
    let mut total_redactions = 0usize;
    let mut out_lines = Vec::new();

    for raw_line in input.lines() {
        let mut line = raw_line.to_owned();

        // 1. Explicit literal secret values supplied by caller/policy.
        for literal in &policy.extra_literal_values {
            let trimmed = literal.trim();
            if !trimmed.is_empty() && line.contains(trimmed) {
                let occurrences = line.matches(trimmed).count();
                line = line.replace(trimmed, "[REDACTED]");
                total_redactions += occurrences;
                rules.insert("explicit-secret-value".to_owned());
            }
        }

        // 2. HTTP header lines (`Header-Name: value`).
        if let Some((name, _value)) = line.split_once(':') {
            let clean_name = name.trim().to_owned();
            if !clean_name.contains('"')
                && !clean_name.contains(' ')
                && policy.is_sensitive_header(&clean_name)
            {
                line = format!("{clean_name}: [REDACTED]");
                total_redactions += 1;
                rules.insert(format!("header:{}", clean_name.to_ascii_lowercase()));
                out_lines.push(line);
                continue;
            }
        }

        // 3. Query strings inside URLs on the line.
        if line.contains('?') {
            let mut tokens = Vec::new();
            for token in line.split(' ') {
                if token.contains('?') {
                    let (clean_url, count) = sanitize_url_query(token, policy);
                    if count > 0 {
                        total_redactions += count;
                        rules.insert("query-parameter".to_owned());
                    }
                    tokens.push(clean_url);
                } else {
                    tokens.push(token.to_owned());
                }
            }
            line = tokens.join(" ");
        }

        // 4. JSON `"field": "value"` or `"field": value` patterns on the line.
        if let Some(redacted_json) = redact_json_field_line(&line, policy, &mut rules) {
            line = redacted_json;
            total_redactions += 1;
        }

        // 5. Key=value tokens (`password=...`, `token=...`).
        if let Some(redacted_kv) = redact_kv_line(&line, policy, &mut rules) {
            line = redacted_kv.0;
            total_redactions += redacted_kv.1;
        }

        // 6. Inline Bearer / Basic auth scheme values.
        for scheme in ["Bearer ", "bearer ", "Basic ", "basic "] {
            if let Some(idx) = line.find(scheme) {
                let prefix = &line[..idx + scheme.len()];
                let rest = &line[idx + scheme.len()..];
                if !rest.starts_with("[REDACTED]") && !rest.trim().is_empty() {
                    let end = rest
                        .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                        .unwrap_or(rest.len());
                    line = format!("{prefix}[REDACTED]{}", &rest[end..]);
                    total_redactions += 1;
                    rules.insert("auth-scheme".to_owned());
                }
            }
        }

        out_lines.push(line);
    }

    let mut sanitized_text = out_lines.join("\n");
    if input.ends_with('\n') && !sanitized_text.ends_with('\n') {
        sanitized_text.push('\n');
    }

    RedactionPreview {
        sanitized_text,
        redacted_count: total_redactions,
        matched_rules: rules.into_iter().collect(),
        manual_review_warning: "Automatic detection cannot find every secret. Review every line before sharing or exporting.",
    }
}

fn contains_literal_secret(value: &str, policy: &RedactionPolicy) -> bool {
    policy
        .extra_literal_values
        .iter()
        .any(|s| !s.trim().is_empty() && value.contains(s.trim()))
}

fn redact_json_field_line(
    line: &str,
    policy: &RedactionPolicy,
    rules: &mut BTreeSet<String>,
) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('"') {
        return None;
    }
    let rest = &trimmed[1..];
    let key_end = rest.find('"')?;
    let key = &rest[..key_end];
    let after_key = rest[key_end + 1..].trim_start();
    if !after_key.starts_with(':') || !policy.is_sensitive_field(key) {
        return None;
    }
    let val_part = after_key[1..].trim_start();
    if val_part.starts_with("\"[REDACTED]\"") {
        return None;
    }
    let indent_len = line.len() - trimmed.len();
    let indent = &line[..indent_len];
    let trailing_comma = if trimmed.trim_end().ends_with(',') {
        ","
    } else {
        ""
    };
    rules.insert(format!("field:{}", key.to_ascii_lowercase()));
    Some(format!("{indent}\"{key}\": \"[REDACTED]\"{trailing_comma}"))
}

fn redact_kv_line(
    line: &str,
    policy: &RedactionPolicy,
    rules: &mut BTreeSet<String>,
) -> Option<(String, usize)> {
    let mut changed = 0usize;
    let mut out = Vec::new();
    for token in line.split(' ') {
        if let Some((k, v)) = token.split_once('=') {
            let clean_k =
                k.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-');
            if !clean_k.is_empty()
                && policy.is_sensitive_field(clean_k)
                && !v.starts_with("[REDACTED]")
            {
                changed += 1;
                rules.insert(format!("kv:{}", clean_k.to_ascii_lowercase()));
                out.push(format!("{k}=[REDACTED]"));
                continue;
            }
        }
        out.push(token.to_owned());
    }
    if changed > 0 {
        Some((out.join(" "), changed))
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedMarkup {
    pub safe_literal_text: String,
    pub blocked_constructs: Vec<String>,
}

/// Neutralizes untrusted HTML/Markdown/example content so active scripts, inline
/// event handlers, executable URIs, and automatic remote fetches are never run.
pub fn sanitize_imported_markup(raw: &str) -> SanitizedMarkup {
    let mut blocked = BTreeSet::new();
    let mut safe_lines = Vec::new();
    let lower = raw.to_ascii_lowercase();

    for (pattern, label) in [
        ("<script", "script-tag"),
        ("<iframe", "iframe-tag"),
        ("<object", "object-tag"),
        ("<embed", "embed-tag"),
        ("<svg", "svg-tag"),
        ("javascript:", "javascript-uri"),
        ("vbscript:", "vbscript-uri"),
        ("data:text/html", "data-html-uri"),
        ("onerror=", "inline-event-handler"),
        ("onload=", "inline-event-handler"),
        ("onclick=", "inline-event-handler"),
        ("onmouseover=", "inline-event-handler"),
        ("http://", "external-http-reference"),
        ("https://", "external-https-reference"),
    ] {
        if lower.contains(pattern) {
            blocked.insert(label.to_owned());
        }
    }

    for line in raw.lines() {
        let mut clean = line.to_owned();
        // Neutralize angle brackets and executable URI schemes into literal display form.
        clean = clean
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace("javascript:", "[blocked-uri:javascript]:")
            .replace("JAVASCRIPT:", "[blocked-uri:javascript]:")
            .replace("vbscript:", "[blocked-uri:vbscript]:")
            .replace("data:text/html", "[blocked-uri:data-html]");
        safe_lines.push(clean);
    }

    SanitizedMarkup {
        safe_literal_text: safe_lines.join("\n"),
        blocked_constructs: blocked.into_iter().collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopPolicy {
    pub analytics_enabled: bool,
    pub crash_upload_enabled: bool,
    pub remote_html_execution: bool,
    pub external_ref_fetch: bool,
    pub extension_host_enabled: bool,
    pub script_sandbox_enabled: bool,
    pub typed_commands_only: bool,
}

impl Default for DesktopPolicy {
    fn default() -> Self {
        Self {
            analytics_enabled: false,
            crash_upload_enabled: false,
            remote_html_execution: false,
            external_ref_fetch: false,
            extension_host_enabled: false,
            script_sandbox_enabled: false,
            typed_commands_only: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_headers_query_json_kv_and_custom_values_with_warning() {
        let policy = RedactionPolicy {
            extra_fields: vec!["x-custom-secret".into(), "tenant_pin".into()],
            extra_literal_values: vec!["canary-literal-9981".into()],
        };
        let input = "\
GET https://api.example.test/v1/items?page=2&api_key=abc123&token=t999 HTTP/1.1\n\
Authorization: Bearer secret-bearer-val\n\
Cookie: session=xyz; theme=light\n\
X-Custom-Secret: my-header-secret\n\
X-Safe: public-value\n\
{\n\
  \"tenant_pin\": \"4455\",\n\
  \"name\": \"DisNote\",\n\
  \"note\": \"contains canary-literal-9981 inside text\"\n\
}\n";
        let preview = sanitize_text(input, &policy);
        assert!(!preview.sanitized_text.contains("abc123"));
        assert!(!preview.sanitized_text.contains("t999"));
        assert!(!preview.sanitized_text.contains("secret-bearer-val"));
        assert!(!preview.sanitized_text.contains("session=xyz"));
        assert!(!preview.sanitized_text.contains("my-header-secret"));
        assert!(!preview.sanitized_text.contains("4455"));
        assert!(!preview.sanitized_text.contains("canary-literal-9981"));
        assert!(preview.sanitized_text.contains("page=2"));
        assert!(preview.sanitized_text.contains("X-Safe: public-value"));
        assert!(preview.sanitized_text.contains("\"name\": \"DisNote\""));
        assert!(preview.redacted_count >= 7);
        assert!(
            preview
                .manual_review_warning
                .contains("cannot find every secret")
        );
    }

    #[test]
    fn untrusted_markup_neutralizes_scripts_events_and_external_links() {
        let raw = "<script>alert(1)</script>\n<img src=\"https://evil.example/a.png\" onerror=\"steal()\">\n[click](javascript:alert(2))";
        let sanitized = sanitize_imported_markup(raw);
        assert!(!sanitized.safe_literal_text.contains("<script>"));
        assert!(!sanitized.safe_literal_text.contains("javascript:"));
        assert!(sanitized.safe_literal_text.contains("&lt;script&gt;"));
        assert!(
            sanitized
                .blocked_constructs
                .contains(&"script-tag".to_owned())
        );
        assert!(
            sanitized
                .blocked_constructs
                .contains(&"inline-event-handler".to_owned())
        );
        assert!(
            sanitized
                .blocked_constructs
                .contains(&"javascript-uri".to_owned())
        );
        assert!(
            sanitized
                .blocked_constructs
                .contains(&"external-https-reference".to_owned())
        );
    }

    #[test]
    fn desktop_policy_defaults_fail_closed_and_disable_telemetry() {
        let policy = DesktopPolicy::default();
        assert!(!policy.analytics_enabled);
        assert!(!policy.crash_upload_enabled);
        assert!(!policy.remote_html_execution);
        assert!(!policy.external_ref_fetch);
        assert!(!policy.extension_host_enabled);
        assert!(!policy.script_sandbox_enabled);
        assert!(policy.typed_commands_only);
    }
}
