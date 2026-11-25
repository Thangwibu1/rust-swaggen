//! Bidirectional import/export codecs: Open Project manifest, Postman Collection v2.1,
//! Postman Environment, pure-Rust cURL parser/generator (no shell), and import preview (IM01-IM08).
use crate::openapi::{SpecDocument, SpecFormat, format_spec_text};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use swaggen_domain::security::RedactionPolicy;
use swaggen_domain::workspace::{
    ApiKeyPlacement, AuthConfig, Collection, Environment, Folder, HttpMethod, KeyValueRow,
    RequestBody, RequestItem, VariableEntry, WorkspaceState, new_uid,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityReport {
    pub imported_collections: usize,
    pub imported_folders: usize,
    pub imported_requests: usize,
    pub imported_variables: usize,
    pub warnings: Vec<String>,
    pub unsupported_items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenProjectManifest {
    pub schema_version: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub files: Vec<String>,
    pub excludes_secrets_and_history: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenProjectBundle {
    pub manifest: OpenProjectManifest,
    pub sanitized_workspace_json: String,
    pub specs: Vec<(String, String)>,
}

pub fn export_open_project_bundle(
    state: &WorkspaceState,
    specs: &[SpecDocument],
    policy: &RedactionPolicy,
) -> Result<OpenProjectBundle, String> {
    let sanitized_workspace_json = state
        .export_sanitized_json(policy)
        .map_err(|e| e.to_string())?;
    let mut files = vec!["manifest.json".to_owned(), "workspace.json".to_owned()];
    let mut spec_files = Vec::new();
    for s in specs {
        let ext = match s.format {
            SpecFormat::Json => "json",
            SpecFormat::Yaml => "yaml",
        };
        let fname = format!("specs/{}.{ext}", s.id);
        files.push(fname.clone());
        spec_files.push((fname, s.raw_text.clone()));
    }
    Ok(OpenProjectBundle {
        manifest: OpenProjectManifest {
            schema_version: "swaggen_project_v1".into(),
            workspace_id: state.meta.id.as_str().to_owned(),
            workspace_name: state.meta.name.clone(),
            files,
            excludes_secrets_and_history: true,
        },
        sanitized_workspace_json,
        specs: spec_files,
    })
}

pub fn export_openapi_roundtrip(
    doc: &SpecDocument,
    target_format: SpecFormat,
) -> Result<(String, Vec<String>), String> {
    let text = format_spec_text(&doc.raw_text, target_format)?;
    let mut notes = Vec::new();
    if doc.format != target_format {
        notes.push("Converted format; comments in YAML source are not preserved in JSON.".into());
    }
    Ok((text, notes))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportConflictStrategy {
    CreateNewWorkspace,
    MergeSkipDuplicates,
    MergeOverwriteDuplicates,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPreview {
    pub new_requests: usize,
    pub duplicate_requests: usize,
    pub skipped_items: usize,
    pub unsupported_items: Vec<String>,
}

pub fn preview_request_import(
    existing: &WorkspaceState,
    incoming: &[RequestItem],
    report: &CompatibilityReport,
) -> ImportPreview {
    let mut new_count = 0usize;
    let mut dup_count = 0usize;
    for r in incoming {
        let is_dup = existing.requests.iter().any(|e| {
            e.name.eq_ignore_ascii_case(&r.name) && e.method == r.method && e.url == r.url
        });
        if is_dup {
            dup_count += 1;
        } else {
            new_count += 1;
        }
    }
    ImportPreview {
        new_requests: new_count,
        duplicate_requests: dup_count,
        skipped_items: 0,
        unsupported_items: report.unsupported_items.clone(),
    }
}

pub fn import_postman_collection_v2_1(
    raw_json: &str,
) -> Result<
    (
        Collection,
        Vec<Folder>,
        Vec<RequestItem>,
        CompatibilityReport,
    ),
    String,
> {
    let root: Value = serde_json::from_str(raw_json).map_err(|e| format!("Invalid JSON: {e}"))?;
    let info = root
        .get("info")
        .and_then(Value::as_object)
        .ok_or_else(|| "Missing Postman 'info' object".to_owned())?;
    let col_name = info
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Imported Collection")
        .to_owned();
    let col_desc = info
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let col_id = new_uid();
    let mut variables = Vec::new();
    if let Some(vars) = root.get("variable").and_then(Value::as_array) {
        for v in vars {
            if let Some(k) = v.get("key").and_then(Value::as_str) {
                let val = v.get("value").and_then(Value::as_str).unwrap_or_default();
                variables.push(VariableEntry::public(k, val));
            }
        }
    }
    let mut report = CompatibilityReport {
        imported_collections: 1,
        imported_folders: 0,
        imported_requests: 0,
        imported_variables: variables.len(),
        warnings: Vec::new(),
        unsupported_items: Vec::new(),
    };
    let col_auth = parse_postman_auth(root.get("auth"), &mut report);
    let mut folders = Vec::new();
    let mut requests = Vec::new();
    if let Some(items) = root.get("item").and_then(Value::as_array) {
        walk_postman_items(
            items,
            &col_id,
            None,
            &mut folders,
            &mut requests,
            &mut report,
        );
    }
    report.imported_folders = folders.len();
    report.imported_requests = requests.len();
    let collection = Collection {
        id: col_id,
        name: col_name,
        description: col_desc,
        auth: col_auth,
        variables,
        order_key: 0,
    };
    Ok((collection, folders, requests, report))
}

fn walk_postman_items(
    items: &[Value],
    col_id: &str,
    parent_folder_id: Option<&str>,
    folders: &mut Vec<Folder>,
    requests: &mut Vec<RequestItem>,
    report: &mut CompatibilityReport,
) {
    for item in items {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Unnamed")
            .to_owned();
        if let Some(events) = item.get("event").and_then(Value::as_array)
            && !events.is_empty()
        {
            report.unsupported_items.push(format!(
                "Postman scripts on '{name}' are not executed in Phase A (preserved as warning)"
            ));
        }
        if let Some(sub_items) = item.get("item").and_then(Value::as_array) {
            let folder_id = new_uid();
            folders.push(Folder {
                id: folder_id.clone(),
                collection_id: col_id.to_owned(),
                parent_folder_id: parent_folder_id.map(ToOwned::to_owned),
                name,
                description: item
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                auth: parse_postman_auth(item.get("auth"), report),
                order_key: folders.len() as u64,
            });
            walk_postman_items(
                sub_items,
                col_id,
                Some(&folder_id),
                folders,
                requests,
                report,
            );
        } else if let Some(req_obj) = item.get("request") {
            let method = req_obj
                .get("method")
                .and_then(Value::as_str)
                .and_then(HttpMethod::parse)
                .unwrap_or(HttpMethod::Get);
            let (url_str, query_params, path_params) = parse_postman_url(req_obj.get("url"));
            let mut req = RequestItem::new(col_id, name, method, url_str);
            req.folder_id = parent_folder_id.map(ToOwned::to_owned);
            if !query_params.is_empty() {
                req.query_params = query_params;
                req.sync_url_from_query_params();
            }
            if !path_params.is_empty() {
                for p in path_params {
                    if let Some(existing) = req.path_params.iter_mut().find(|e| e.key == p.key) {
                        existing.value = p.value;
                        existing.enabled = p.enabled;
                    } else {
                        req.path_params.push(p);
                    }
                }
            }
            if let Some(hdrs) = req_obj.get("header").and_then(Value::as_array) {
                for h in hdrs {
                    if let Some(k) = h.get("key").and_then(Value::as_str) {
                        let v = h.get("value").and_then(Value::as_str).unwrap_or_default();
                        let disabled = h.get("disabled").and_then(Value::as_bool).unwrap_or(false);
                        let mut row = KeyValueRow::new(k, v);
                        row.enabled = !disabled;
                        req.headers.push(row);
                    }
                }
            }
            req.body = parse_postman_body(req_obj.get("body"));
            req.auth = parse_postman_auth(req_obj.get("auth"), report);
            requests.push(req);
        }
    }
}

fn parse_postman_url(val: Option<&Value>) -> (String, Vec<KeyValueRow>, Vec<KeyValueRow>) {
    let Some(val) = val else {
        return (String::new(), Vec::new(), Vec::new());
    };
    if let Some(s) = val.as_str() {
        return (s.to_owned(), Vec::new(), Vec::new());
    }
    let raw = val
        .get("raw")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut query = Vec::new();
    if let Some(arr) = val.get("query").and_then(Value::as_array) {
        for q in arr {
            if let Some(k) = q.get("key").and_then(Value::as_str) {
                let v = q.get("value").and_then(Value::as_str).unwrap_or_default();
                let disabled = q.get("disabled").and_then(Value::as_bool).unwrap_or(false);
                let mut row = KeyValueRow::new(k, v);
                row.enabled = !disabled;
                query.push(row);
            }
        }
    }
    let mut path_vars = Vec::new();
    if let Some(arr) = val.get("variable").and_then(Value::as_array) {
        for pv in arr {
            if let Some(k) = pv.get("key").and_then(Value::as_str) {
                let v = pv.get("value").and_then(Value::as_str).unwrap_or_default();
                let disabled = pv.get("disabled").and_then(Value::as_bool).unwrap_or(false);
                let mut row = KeyValueRow::new(k, v);
                row.enabled = !disabled;
                path_vars.push(row);
            }
        }
    }
    (raw, query, path_vars)
}

fn parse_postman_body(val: Option<&Value>) -> RequestBody {
    let Some(b) = val else {
        return RequestBody::None;
    };
    match b.get("mode").and_then(Value::as_str) {
        Some("raw") => {
            let raw = b
                .get("raw")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let lang = b
                .get("options")
                .and_then(|o| o.get("raw"))
                .and_then(|r| r.get("language"))
                .and_then(Value::as_str);
            match lang {
                Some("xml") => RequestBody::Xml(raw),
                Some("text") => RequestBody::Text(raw),
                _ => RequestBody::Json(raw),
            }
        }
        Some("urlencoded") => {
            let mut rows = Vec::new();
            if let Some(arr) = b.get("urlencoded").and_then(Value::as_array) {
                for row in arr {
                    if let Some(k) = row.get("key").and_then(Value::as_str) {
                        let v = row.get("value").and_then(Value::as_str).unwrap_or_default();
                        rows.push(KeyValueRow::new(k, v));
                    }
                }
            }
            RequestBody::FormUrlEncoded(rows)
        }
        _ => RequestBody::None,
    }
}

fn parse_postman_auth(val: Option<&Value>, report: &mut CompatibilityReport) -> AuthConfig {
    let Some(a) = val else {
        return AuthConfig::Inherit;
    };
    let kind = a.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "noauth" => AuthConfig::NoAuth,
        "bearer" => {
            let tok = postman_auth_kv(a.get("bearer"), "token").unwrap_or_default();
            AuthConfig::Bearer { token: tok }
        }
        "basic" => {
            let u = postman_auth_kv(a.get("basic"), "username").unwrap_or_default();
            let p = postman_auth_kv(a.get("basic"), "password").unwrap_or_default();
            AuthConfig::Basic {
                username: u,
                password: p,
            }
        }
        "apikey" => {
            let k = postman_auth_kv(a.get("apikey"), "key").unwrap_or_else(|| "X-Api-Key".into());
            let v = postman_auth_kv(a.get("apikey"), "value").unwrap_or_default();
            let loc = postman_auth_kv(a.get("apikey"), "in");
            AuthConfig::ApiKey {
                key: k,
                value: v,
                placement: if loc.as_deref() == Some("query") {
                    ApiKeyPlacement::Query
                } else {
                    ApiKeyPlacement::Header
                },
            }
        }
        other if !other.is_empty() => {
            report
                .unsupported_items
                .push(format!("Postman auth type '{other}' imported as NoAuth"));
            AuthConfig::NoAuth
        }
        _ => AuthConfig::Inherit,
    }
}

fn postman_auth_kv(arr: Option<&Value>, key: &str) -> Option<String> {
    let items = arr?.as_array()?;
    items.iter().find_map(|entry| {
        if entry.get("key").and_then(Value::as_str) == Some(key) {
            entry
                .get("value")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        } else {
            None
        }
    })
}

pub fn export_postman_collection_v2_1(
    col: &Collection,
    requests: &[RequestItem],
) -> Result<String, String> {
    let items: Vec<Value> = requests
        .iter()
        .filter(|r| r.collection_id == col.id)
        .map(|r| {
            let mut req_obj = Map::new();
            req_obj.insert("method".into(), Value::String(r.method.as_str().into()));
            req_obj.insert("url".into(), Value::String(r.url.clone()));
            let hdrs: Vec<Value> = r
                .headers
                .iter()
                .map(|h| {
                    serde_json::json!({
                        "key": h.key,
                        "value": h.value,
                        "disabled": !h.enabled
                    })
                })
                .collect();
            req_obj.insert("header".into(), Value::Array(hdrs));
            serde_json::json!({
                "name": r.name,
                "request": Value::Object(req_obj)
            })
        })
        .collect();
    let doc = serde_json::json!({
        "info": {
            "name": col.name,
            "description": col.description,
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
        },
        "item": items
    });
    serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())
}

pub fn import_postman_environment(raw_json: &str) -> Result<(Environment, Vec<String>), String> {
    let root: Value = serde_json::from_str(raw_json).map_err(|e| format!("Invalid JSON: {e}"))?;
    let name = root
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Imported Environment")
        .to_owned();
    let mut env = Environment::new(name, false);
    let mut preview_lines = Vec::new();
    if let Some(vals) = root.get("values").and_then(Value::as_array) {
        for v in vals {
            let Some(k) = v.get("key").and_then(Value::as_str) else {
                continue;
            };
            let val = v.get("value").and_then(Value::as_str).unwrap_or_default();
            let vtype = v.get("type").and_then(Value::as_str).unwrap_or("default");
            let enabled = v.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            let is_secret = vtype.eq_ignore_ascii_case("secret");
            if is_secret {
                preview_lines.push(format!("{k} = [LOCAL SECRET OVERRIDE; NOT SYNCED TO TEAM]"));
                let mut entry = VariableEntry::public(k, "");
                entry.is_secret = true;
                entry.enabled = enabled;
                env.variables.push(entry);
            } else {
                preview_lines.push(format!("{k} = {val}"));
                let mut entry = VariableEntry::public(k, val);
                entry.enabled = enabled;
                env.variables.push(entry);
            }
        }
    }
    Ok((env, preview_lines))
}

/// Safe pure-Rust cURL command parser (never executes shell commands, supports Windows `^` and POSIX quotes · IM05).
pub fn parse_curl_command(input: &str, collection_id: &str) -> Result<RequestItem, String> {
    let tokens = tokenize_curl(input)?;
    if tokens.is_empty() {
        return Err("Empty cURL command".into());
    }
    let start =
        if tokens[0].eq_ignore_ascii_case("curl") || tokens[0].eq_ignore_ascii_case("curl.exe") {
            1
        } else {
            0
        };
    let mut method: Option<HttpMethod> = None;
    let mut url: Option<String> = None;
    let mut headers = Vec::new();
    let mut data_parts = Vec::new();
    let mut auth = AuthConfig::Inherit;

    let mut i = start;
    while i < tokens.len() {
        let tok = &tokens[i];
        match tok.as_str() {
            "-X" | "--request" => {
                i += 1;
                let m = tokens.get(i).ok_or("Missing method after -X")?;
                method =
                    Some(HttpMethod::parse(m).ok_or_else(|| format!("Unsupported method '{m}'"))?);
            }
            "-H" | "--header" => {
                i += 1;
                let hdr = tokens.get(i).ok_or("Missing header after -H")?;
                let (k, v) = hdr
                    .split_once(':')
                    .ok_or_else(|| format!("Invalid header '{hdr}'"))?;
                headers.push(KeyValueRow::new(k.trim(), v.trim()));
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-urlencode" => {
                i += 1;
                let d = tokens.get(i).ok_or("Missing payload after -d")?;
                data_parts.push(d.clone());
            }
            "-u" | "--user" => {
                i += 1;
                let u = tokens.get(i).ok_or("Missing user:pass after -u")?;
                let (user, pass) = u.split_once(':').unwrap_or((u, ""));
                auth = AuthConfig::Basic {
                    username: user.to_owned(),
                    password: pass.to_owned(),
                };
            }
            "--url" => {
                i += 1;
                url = Some(tokens.get(i).ok_or("Missing URL after --url")?.clone());
            }
            "-s" | "--silent" | "-S" | "-L" | "--location" | "-i" | "-v" => {}
            other if !other.starts_with('-') && url.is_none() => {
                url = Some(other.to_owned());
            }
            _ => {}
        }
        i += 1;
    }

    let url = url.ok_or("No URL found in cURL command")?;
    let inferred_method = method.unwrap_or(if data_parts.is_empty() {
        HttpMethod::Get
    } else {
        HttpMethod::Post
    });
    let mut req = RequestItem::new(collection_id, "Imported cURL", inferred_method, url);
    req.headers = headers;
    req.auth = auth;
    if !data_parts.is_empty() {
        let joined = data_parts.join("&");
        req.body = if joined.trim_start().starts_with('{') || joined.trim_start().starts_with('[') {
            RequestBody::Json(joined)
        } else {
            RequestBody::Text(joined)
        };
    }
    Ok(req)
}

pub fn to_curl_command(req: &RequestItem, mask_secrets: bool) -> String {
    let mut parts = vec![format!("curl -X {} \"{}\"", req.method.as_str(), req.url)];
    for h in req
        .headers
        .iter()
        .filter(|h| h.enabled && !h.key.is_empty())
    {
        let val = if mask_secrets && h.key.eq_ignore_ascii_case("authorization") {
            "Bearer {{TOKEN}}".into()
        } else {
            h.value.replace('"', "\\\"")
        };
        parts.push(format!("-H \"{}: {val}\"", h.key));
    }
    match &req.body {
        RequestBody::Json(s) | RequestBody::Text(s) | RequestBody::Xml(s) if !s.is_empty() => {
            parts.push(format!("-d '{}'", s.replace('\'', "'\\''")));
        }
        _ => {}
    }
    parts.join(" \\\n  ")
}

fn tokenize_curl(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut has_token = false;

    while let Some(c) = chars.next() {
        if in_single {
            if c == '\'' {
                in_single = false;
            } else {
                cur.push(c);
            }
            continue;
        }
        if in_double {
            if c == '"' {
                in_double = false;
            } else if c == '\\' {
                if let Some(next) = chars.next() {
                    cur.push(next);
                }
            } else {
                cur.push(c);
            }
            continue;
        }
        match c {
            '\'' => {
                in_single = true;
                has_token = true;
            }
            '"' => {
                in_double = true;
                has_token = true;
            }
            '\\' | '^' => {
                if let Some(&next) = chars.peek() {
                    if next == '\r' || next == '\n' {
                        chars.next();
                        if next == '\r' && chars.peek() == Some(&'\n') {
                            chars.next();
                        }
                        continue;
                    }
                    cur.push(chars.next().unwrap());
                    has_token = true;
                }
            }
            ch if ch.is_whitespace() => {
                if has_token {
                    tokens.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            ch => {
                cur.push(ch);
                has_token = true;
            }
        }
    }
    if in_single || in_double {
        return Err("Unclosed quote in cURL command (I11)".into());
    }
    if has_token {
        tokens.push(cur);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postman_v2_1_and_environment_import_work_with_report() {
        let col_json = r#"{
          "info": { "name": "Demo API", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" },
          "item": [
            {
              "name": "Users",
              "item": [
                {
                  "name": "Get User",
                  "event": [{ "listen": "test", "script": { "exec": ["pm.test('ok', () => {})"] } }],
                  "request": {
                    "method": "GET",
                    "url": { "raw": "https://api.example.test/users?role=admin", "query": [{ "key": "role", "value": "admin" }] },
                    "header": [{ "key": "X-App", "value": "1" }]
                  }
                }
              ]
            }
          ]
        }"#;
        let (col, folders, reqs, report) = import_postman_collection_v2_1(col_json).unwrap();
        assert_eq!(col.name, "Demo API");
        assert_eq!(folders.len(), 1);
        assert_eq!(reqs.len(), 1);
        assert!(!report.unsupported_items.is_empty());

        let env_json = r#"{"name":"Dev","values":[{"key":"secret_tok","value":"hidden","type":"secret","enabled":true}]}"#;
        let (env, preview) = import_postman_environment(env_json).unwrap();
        assert_eq!(env.variables.len(), 1);
        assert!(env.variables[0].is_secret);
        assert!(preview[0].contains("NOT SYNCED"));
    }

    #[test]
    fn curl_roundtrip_and_windows_line_continuations_work_without_shell() {
        let cmd = "curl -X POST ^\r\n  -H \"Content-Type: application/json\" ^\r\n  -d '{\"a\":1}' https://api.example.test/items";
        let req = parse_curl_command(cmd, "c1").unwrap();
        assert_eq!(req.method, HttpMethod::Post);
        assert_eq!(req.url, "https://api.example.test/items");
        assert!(matches!(req.body, RequestBody::Json(ref s) if s == "{\"a\":1}"));
        let generated = to_curl_command(&req, true);
        assert!(generated.contains("curl -X POST"));
        assert!(parse_curl_command("curl \"unclosed", "c1").is_err());
    }
}
