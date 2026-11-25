//! Version C HAR 1.2 & `.http` / `.rest` importers with secret stripping & compatibility reports (IM10).
use crate::importers::CompatibilityReport;
use serde_json::Value;
use std::collections::BTreeMap;
use swaggen_domain::workspace::{HttpMethod, KeyValueRow, RequestBody, RequestItem};

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "authorization" | "cookie" | "set-cookie" | "x-api-key" | "proxy-authorization"
    )
}

fn empty_report() -> CompatibilityReport {
    CompatibilityReport {
        imported_collections: 0,
        imported_folders: 0,
        imported_requests: 0,
        imported_variables: 0,
        warnings: Vec::new(),
        unsupported_items: Vec::new(),
    }
}

/// Imports HTTP Archive (HAR 1.2) entries into workspace requests, stripping credential headers (IM10).
pub fn import_har(
    raw_json: &str,
    collection_id: &str,
) -> Result<(Vec<RequestItem>, CompatibilityReport), String> {
    let val: Value =
        serde_json::from_str(raw_json).map_err(|e| format!("Invalid HAR JSON: {e}"))?;
    let entries = val
        .pointer("/log/entries")
        .and_then(Value::as_array)
        .ok_or_else(|| "HAR archive missing /log/entries array".to_owned())?;
    let mut out = Vec::new();
    let mut rep = empty_report();
    for (idx, ent) in entries.iter().enumerate() {
        let Some(req_obj) = ent.get("request") else {
            continue;
        };
        let method_s = req_obj
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("GET");
        let method = HttpMethod::parse(method_s).unwrap_or(HttpMethod::Get);
        let url_s = req_obj.get("url").and_then(Value::as_str).unwrap_or("");
        if url_s.is_empty() {
            continue;
        }
        let mut item = RequestItem::new(
            collection_id,
            format!("HAR #{} {method_s}", idx + 1),
            method,
            url_s,
        );
        if let Some(hdrs) = req_obj.get("headers").and_then(Value::as_array) {
            for h in hdrs {
                let name = h.get("name").and_then(Value::as_str).unwrap_or("");
                let value = h.get("value").and_then(Value::as_str).unwrap_or("");
                if name.is_empty() || name.starts_with(':') {
                    continue;
                }
                if is_sensitive_header(name) {
                    rep.warnings.push(format!(
                        "Stripped sensitive HAR header '{name}' on request #{}",
                        idx + 1
                    ));
                    item.headers
                        .push(KeyValueRow::new(name, "{{redacted_secret}}"));
                } else {
                    item.headers.push(KeyValueRow::new(name, value));
                }
            }
        }
        if let Some(body_txt) = req_obj.pointer("/postData/text").and_then(Value::as_str)
            && !body_txt.is_empty()
        {
            let trimmed = body_txt.trim_start();
            item.body = if trimmed.starts_with('{') || trimmed.starts_with('[') {
                RequestBody::Json(body_txt.to_owned())
            } else {
                RequestBody::Text(body_txt.to_owned())
            };
        }
        rep.imported_requests += 1;
        out.push(item);
    }
    Ok((out, rep))
}

pub type DotHttpImportResult = (
    Vec<RequestItem>,
    BTreeMap<String, String>,
    CompatibilityReport,
);

/// Imports VS Code / IntelliJ `.http` / `.rest` files (`###` delimited + `@var = val`) (IM10).
pub fn import_dot_http(raw_text: &str, collection_id: &str) -> Result<DotHttpImportResult, String> {
    let mut vars = BTreeMap::new();
    let mut reqs = Vec::new();
    let mut rep = empty_report();

    for block_raw in raw_text.split("###") {
        let mut method_url: Option<(HttpMethod, String)> = None;
        let mut name = String::new();
        let mut headers = Vec::new();
        let mut body_lines = Vec::new();
        let mut in_body = false;

        for line in block_raw.lines().map(str::trim_end) {
            let t = line.trim();
            if in_body {
                body_lines.push(line);
                continue;
            }
            if t.is_empty() {
                if method_url.is_some() {
                    in_body = true;
                }
                continue;
            }
            if let Some(after_at) = t.strip_prefix('@')
                && let Some((k, v)) = after_at.split_once('=')
            {
                vars.insert(k.trim().to_owned(), v.trim().to_owned());
                continue;
            }
            if t.starts_with('#') || t.starts_with("//") {
                if name.is_empty() {
                    name = t.trim_start_matches(&['#', '/', ' '][..]).to_owned();
                }
                continue;
            }
            if method_url.is_none() {
                let mut parts = t.split_whitespace();
                let first = parts.next().unwrap_or("");
                if HttpMethod::parse(first).is_none()
                    && !t.contains("://")
                    && !t.starts_with("{{")
                    && name.is_empty()
                {
                    name = t.to_owned();
                    continue;
                }
                let m = HttpMethod::parse(first).unwrap_or(HttpMethod::Get);
                let url_tok = if HttpMethod::parse(first).is_some() {
                    parts.next().unwrap_or("")
                } else {
                    first
                };
                if !url_tok.is_empty() {
                    method_url = Some((m, url_tok.to_owned()));
                }
                continue;
            }
            if let Some((hk, hv)) = t.split_once(':') {
                if is_sensitive_header(hk) {
                    rep.warnings
                        .push(format!("Masked sensitive .http header '{}'", hk.trim()));
                    headers.push(KeyValueRow::new(hk.trim(), "{{redacted_secret}}"));
                } else {
                    headers.push(KeyValueRow::new(hk.trim(), hv.trim()));
                }
            }
        }
        if let Some((m, url_s)) = method_url {
            let mut item = RequestItem::new(
                collection_id,
                if name.is_empty() {
                    format!("{} {}", m.as_str(), url_s)
                } else {
                    name
                },
                m,
                url_s,
            );
            item.headers = headers;
            let b = body_lines.join("\n").trim().to_owned();
            if !b.is_empty() {
                item.body = if b.starts_with('{') || b.starts_with('[') {
                    RequestBody::Json(b)
                } else {
                    RequestBody::Text(b)
                };
            }
            rep.imported_requests += 1;
            reqs.push(item);
        }
    }
    if reqs.is_empty() {
        return Err("No valid HTTP requests found in .http input".into());
    }
    rep.imported_variables = vars.len();
    Ok((reqs, vars, rep))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn har_and_dot_http_import_requests_and_strip_credentials() {
        let har = r#"{"log":{"entries":[{"request":{"method":"POST","url":"https://api.test/orders","headers":[{"name":"Authorization","value":"Bearer LEAK"},{"name":"Content-Type","value":"application/json"}],"postData":{"text":"{\"qty\":2}"}}}]}}"#;
        let (items, rep) = import_har(har, "c1").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].headers[0].value, "{{redacted_secret}}");
        assert!(!rep.warnings.is_empty());

        let dot_http = "@host = https://api.test\n### Create Order\nPOST {{host}}/orders\nAuthorization: Bearer RAW_TOK\nContent-Type: application/json\n\n{\"qty\":3}\n";
        let (hitems, vars, hrep) = import_dot_http(dot_http, "c1").unwrap();
        assert_eq!(hitems.len(), 1);
        assert_eq!(
            vars.get("host").map(String::as_str),
            Some("https://api.test")
        );
        assert_eq!(hitems[0].name, "Create Order");
        assert!(!hrep.warnings.is_empty());
    }
}
