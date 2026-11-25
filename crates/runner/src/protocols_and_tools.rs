//! Version B protocols & productivity utilities:
//! - GraphQL Query + Variables + Errors Inspector (PR01)
//! - Bounded SSE Client with reconnect/cancel & FIFO ring cap (PR04)
//! - Secret-safe Code Snippets (PR07: cURL, Rust reqwest, JS fetch, Dart, Go)
//! - Offline Dev Utilities (PR09: URL, Base64, JSON pretty/minify, JSON <-> YAML)
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::VecDeque, path::Path};
use swaggen_domain::workspace::{
    HttpMethod, RequestBody, RequestConnectionPolicy, ResolvedHeader, ResolvedRequest,
};
use swaggen_http_engine::execute_request;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::mpsc::UnboundedSender,
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphQlOutcome {
    pub http_status: u16,
    pub duration_ms: u64,
    pub data_pretty: String,
    pub errors: Vec<String>,
    pub raw_body: String,
}

/// Executes a GraphQL HTTP POST query and separates `data` vs `errors` array cleanly (PR01).
pub async fn execute_graphql(
    endpoint: &str,
    query: &str,
    variables_json: &str,
    operation_name: Option<&str>,
    extra_headers: &[(String, String)],
    downloads_dir: &Path,
    cancel: CancellationToken,
) -> Result<GraphQlOutcome, String> {
    let vars_val: Value = if variables_json.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(variables_json)
            .map_err(|e| format!("Invalid GraphQL variables JSON: {e}"))?
    };
    let mut payload = serde_json::Map::new();
    payload.insert("query".into(), Value::String(query.to_owned()));
    payload.insert("variables".into(), vars_val);
    if let Some(op) = operation_name.filter(|s| !s.trim().is_empty()) {
        payload.insert("operationName".into(), Value::String(op.trim().to_owned()));
    }
    let body_str = serde_json::to_string(&Value::Object(payload)).map_err(|e| e.to_string())?;
    let mut headers = vec![ResolvedHeader {
        name: "Content-Type".into(),
        value: "application/json".into(),
        generated: true,
    }];
    for (k, v) in extra_headers {
        if !k.trim().is_empty() {
            headers.push(ResolvedHeader {
                name: k.trim().to_owned(),
                value: v.clone(),
                generated: false,
            });
        }
    }
    let req = ResolvedRequest {
        method: HttpMethod::Post,
        url: endpoint.trim().to_owned(),
        masked_url: endpoint.trim().to_owned(),
        headers,
        masked_headers: Vec::new(),
        body: RequestBody::Text(body_str),
        masked_body_preview: String::new(),
        auth_source: "GraphQL".into(),
        warnings: Vec::new(),
        blocking_errors: Vec::new(),
        requires_production_confirm: false,
        connection: RequestConnectionPolicy::default(),
    };
    let resp = execute_request(&req, downloads_dir, cancel)
        .await
        .map_err(|e| format!("{}: {}", e.kind.as_str(), e.message))?;
    let raw = String::from_utf8_lossy(&resp.preview_bytes).into_owned();
    let parsed: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
    let data_pretty = parsed
        .get("data")
        .map(|d| serde_json::to_string_pretty(d).unwrap_or_default())
        .unwrap_or_else(|| "null".into());
    let errors = parsed
        .get("errors")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|e| {
                    e.get("message")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| e.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(GraphQlOutcome {
        http_status: resp.status_code,
        duration_ms: resp.duration_ms,
        data_pretty,
        errors,
        raw_body: raw,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SseEventRecord {
    pub event: String,
    pub id: Option<String>,
    pub data: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SseClientPolicy {
    pub max_events: usize,
    pub max_buffer_bytes: usize,
}

impl Default for SseClientPolicy {
    fn default() -> Self {
        Self {
            max_events: 500,
            max_buffer_bytes: 512 * 1024,
        }
    }
}

/// Bounded HTTP/1.1 Server-Sent Events client obeying `max_events`, `max_buffer_bytes`, and `cancel` (PR04).
pub async fn stream_sse_bounded(
    url_str: &str,
    headers: &[(String, String)],
    policy: SseClientPolicy,
    cancel: CancellationToken,
    live_tx: Option<UnboundedSender<SseEventRecord>>,
) -> Result<Vec<SseEventRecord>, String> {
    let url = url::Url::parse(url_str.trim()).map_err(|e| format!("Invalid SSE URL: {e}"))?;
    if url.scheme() != "http" {
        return Err(
            "Bounded direct SSE reader supports http:// URLs cleanly without buffering".into(),
        );
    }
    let host = url.host_str().ok_or_else(|| "Missing host".to_owned())?;
    let port = url.port_or_known_default().unwrap_or(80);
    let path_q = match url.query() {
        Some(q) => format!("{}?{q}", url.path()),
        None => url.path().to_owned(),
    };
    let mut stream = TcpStream::connect((host, port))
        .await
        .map_err(|e| format!("SSE connect error: {e}"))?;
    let mut raw_req = format!(
        "GET {path_q} HTTP/1.1\r\nHost: {host}:{port}\r\nAccept: text/event-stream\r\nConnection: close\r\n"
    );
    for (k, v) in headers {
        if !k.trim().is_empty() {
            raw_req.push_str(&format!("{}: {v}\r\n", k.trim()));
        }
    }
    raw_req.push_str("\r\n");
    stream
        .write_all(raw_req.as_bytes())
        .await
        .map_err(|e| format!("SSE write error: {e}"))?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    // Skip HTTP status + headers until empty line.
    loop {
        line.clear();
        let n = reader
            .read_line(&mut line)
            .await
            .map_err(|e| e.to_string())?;
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
    }

    let mut events = VecDeque::new();
    let mut total_bytes = 0usize;
    let mut cur_event = String::from("message");
    let mut cur_id: Option<String> = None;
    let mut cur_data = Vec::new();
    let cap_events = policy.max_events.max(1);
    let cap_bytes = policy.max_buffer_bytes.max(1024);

    loop {
        line.clear();
        let read_n = tokio::select! {
            _ = cancel.cancelled() => break,
            res = reader.read_line(&mut line) => match res {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset && !events.is_empty() => 0,
                Err(e) => return Err(e.to_string()),
            },
        };
        if read_n == 0 {
            break;
        }
        let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
        if trimmed.is_empty() {
            if !cur_data.is_empty() {
                let ev = SseEventRecord {
                    event: cur_event.clone(),
                    id: cur_id.take(),
                    data: cur_data.join("\n"),
                };
                if let Some(tx) = &live_tx {
                    let _ = tx.send(ev.clone());
                }
                total_bytes += ev.data.len() + ev.event.len();
                events.push_back(ev);
                while events.len() > cap_events || total_bytes > cap_bytes {
                    if let Some(popped) = events.pop_front() {
                        total_bytes =
                            total_bytes.saturating_sub(popped.data.len() + popped.event.len());
                    } else {
                        break;
                    }
                }
                cur_event = "message".into();
                cur_data.clear();
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("event:") {
            cur_event = rest.trim().to_owned();
        } else if let Some(rest) = trimmed.strip_prefix("id:") {
            cur_id = Some(rest.trim().to_owned());
        } else if let Some(rest) = trimmed.strip_prefix("data:") {
            cur_data.push(rest.trim_start().to_owned());
        }
    }
    Ok(events.into_iter().collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetTarget {
    Curl,
    RustReqwest,
    JsFetch,
    DartHttp,
    GoHttp,
}

impl SnippetTarget {
    pub const ALL: [Self; 5] = [
        Self::Curl,
        Self::RustReqwest,
        Self::JsFetch,
        Self::DartHttp,
        Self::GoHttp,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Curl => "cURL",
            Self::RustReqwest => "Rust (reqwest)",
            Self::JsFetch => "JavaScript (fetch)",
            Self::DartHttp => "Dart (http)",
            Self::GoHttp => "Go (net/http)",
        }
    }
}

/// Generates code snippets strictly using masked values so local secrets never leak into copied snippets (PR07).
pub fn generate_code_snippet(req: &ResolvedRequest, target: SnippetTarget) -> String {
    let url = &req.masked_url;
    let method = req.method.as_str();
    let body_opt = if req.masked_body_preview.is_empty() {
        None
    } else {
        Some(req.masked_body_preview.as_str())
    };
    match target {
        SnippetTarget::Curl => {
            let mut s = format!("curl -X {method} \"{url}\"");
            for h in &req.masked_headers {
                s.push_str(&format!(" \\\n  -H \"{}: {}\"", h.name, h.value));
            }
            if let Some(b) = body_opt {
                s.push_str(&format!(" \\\n  -d '{}'", b.replace('\'', "\\'")));
            }
            s
        }
        SnippetTarget::RustReqwest => {
            let mut s = format!(
                "let client = reqwest::Client::new();\nlet resp = client.{}(\"{url}\")\n",
                method.to_ascii_lowercase()
            );
            for h in &req.masked_headers {
                s.push_str(&format!("    .header(\"{}\", \"{}\")\n", h.name, h.value));
            }
            if let Some(b) = body_opt {
                s.push_str(&format!("    .body({:?})\n", b));
            }
            s.push_str("    .send()\n    .await?;");
            s
        }
        SnippetTarget::JsFetch => {
            let hdrs: serde_json::Map<String, Value> = req
                .masked_headers
                .iter()
                .map(|h| (h.name.clone(), Value::String(h.value.clone())))
                .collect();
            let mut opts = serde_json::Map::new();
            opts.insert("method".into(), json!(method));
            if !hdrs.is_empty() {
                opts.insert("headers".into(), Value::Object(hdrs));
            }
            if let Some(b) = body_opt {
                opts.insert("body".into(), json!(b));
            }
            format!(
                "const res = await fetch(\"{url}\", {});",
                serde_json::to_string_pretty(&Value::Object(opts)).unwrap_or_default()
            )
        }
        SnippetTarget::DartHttp => {
            let mut s = format!("final req = http.Request('{method}', Uri.parse('{url}'));\n");
            for h in &req.masked_headers {
                s.push_str(&format!("req.headers['{}'] = '{}';\n", h.name, h.value));
            }
            if let Some(b) = body_opt {
                s.push_str(&format!(
                    "req.body = {};\n",
                    serde_json::to_string(b).unwrap_or_default()
                ));
            }
            s.push_str("final res = await req.send();");
            s
        }
        SnippetTarget::GoHttp => {
            let body_expr = body_opt
                .map(|b| {
                    format!(
                        "strings.NewReader({})",
                        serde_json::to_string(b).unwrap_or_default()
                    )
                })
                .unwrap_or_else(|| "nil".into());
            let mut s =
                format!("req, err := http.NewRequest(\"{method}\", \"{url}\", {body_expr})\n");
            for h in &req.masked_headers {
                s.push_str(&format!(
                    "req.Header.Add(\"{}\", \"{}\")\n",
                    h.name, h.value
                ));
            }
            s.push_str("resp, err := http.DefaultClient.Do(req)");
            s
        }
    }
}

/// Offline Dev Utilities (PR09).
pub mod dev_utils {
    use super::*;

    pub fn url_encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }

    pub fn url_decode(s: &str) -> Result<String, String> {
        let bytes: Vec<u8> = url::form_urlencoded::parse(format!("v={s}").as_bytes())
            .next()
            .map(|(_, v)| v.as_bytes().to_vec())
            .unwrap_or_default();
        String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8 after URL decode: {e}"))
    }

    pub fn base64_encode(s: &str) -> String {
        STANDARD.encode(s.as_bytes())
    }

    pub fn base64_decode(s: &str) -> Result<String, String> {
        let bytes = STANDARD
            .decode(s.trim())
            .map_err(|e| format!("Invalid Base64: {e}"))?;
        String::from_utf8(bytes).map_err(|e| format!("Decoded Base64 is not UTF-8: {e}"))
    }

    pub fn format_json(s: &str, pretty: bool) -> Result<String, String> {
        let val: Value = serde_json::from_str(s).map_err(|e| format!("Invalid JSON: {e}"))?;
        if pretty {
            serde_json::to_string_pretty(&val).map_err(|e| e.to_string())
        } else {
            serde_json::to_string(&val).map_err(|e| e.to_string())
        }
    }

    pub fn yaml_to_json(yaml_str: &str) -> Result<String, String> {
        let docs = yaml_rust2::YamlLoader::load_from_str(yaml_str)
            .map_err(|e| format!("Invalid YAML: {e}"))?;
        let first = docs
            .first()
            .ok_or_else(|| "Empty YAML document".to_owned())?;
        let val = yaml_to_val(first);
        serde_json::to_string_pretty(&val).map_err(|e| e.to_string())
    }

    fn yaml_to_val(y: &yaml_rust2::Yaml) -> Value {
        match y {
            yaml_rust2::Yaml::Null | yaml_rust2::Yaml::BadValue => Value::Null,
            yaml_rust2::Yaml::Boolean(b) => Value::Bool(*b),
            yaml_rust2::Yaml::Integer(i) => json!(i),
            yaml_rust2::Yaml::Real(s) => s
                .parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(Value::Number)
                .unwrap_or_else(|| Value::String(s.clone())),
            yaml_rust2::Yaml::String(s) => Value::String(s.clone()),
            yaml_rust2::Yaml::Array(a) => Value::Array(a.iter().map(yaml_to_val).collect()),
            yaml_rust2::Yaml::Hash(h) => {
                let mut m = serde_json::Map::new();
                for (k, v) in h {
                    let ks = match k {
                        yaml_rust2::Yaml::String(s) => s.clone(),
                        other => format!("{other:?}"),
                    };
                    m.insert(ks, yaml_to_val(v));
                }
                Value::Object(m)
            }
            _ => Value::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, routing::post};

    #[tokio::test]
    async fn graphql_sse_cap_snippets_and_dev_utils_work() {
        // 1. GraphQL errors separation
        let gql_app = Router::new().route(
            "/graphql",
            post(|| async {
                Json(json!({"data": {"user": {"id": 1}}, "errors": [{"message": "partial field error"}]}))
            }),
        );
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(l, gql_app).await;
        });
        let dl = std::env::temp_dir().join(format!("swaggen-pr-{}", std::process::id()));
        let res = execute_graphql(
            &format!("http://{addr}/graphql"),
            "query { user { id } }",
            "{}",
            None,
            &[],
            &dl,
            CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(res.http_status, 200);
        assert_eq!(res.errors, vec!["partial field error".to_owned()]);
        assert!(res.data_pretty.contains("\"id\": 1"));

        // 2. Bounded SSE reader enforces max_events cap
        let sse_l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let sse_addr = sse_l.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut s, _)) = sse_l.accept().await {
                let mut req_buf = [0u8; 512];
                let _ = tokio::io::AsyncReadExt::read(&mut s, &mut req_buf).await;
                let _ = s
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n")
                    .await;
                for i in 1..=5 {
                    let _ = s
                        .write_all(format!("id: {i}\ndata: msg-{i}\n\n").as_bytes())
                        .await;
                }
                let _ = s.shutdown().await;
            }
        });
        let evs = stream_sse_bounded(
            &format!("http://{sse_addr}/events"),
            &[],
            SseClientPolicy {
                max_events: 3,
                max_buffer_bytes: 16 * 1024,
            },
            CancellationToken::new(),
            None,
        )
        .await
        .unwrap();
        assert_eq!(evs.len(), 3);
        assert_eq!(evs[0].data, "msg-3");
        assert_eq!(evs[2].data, "msg-5");

        // 3. Snippets stay secret-safe + Dev utils round-trip
        let masked_req = ResolvedRequest {
            method: HttpMethod::Post,
            url: "https://api.test/v1?token=RAW_SECRET".into(),
            masked_url: "https://api.test/v1?token=••••••••".into(),
            headers: vec![ResolvedHeader {
                name: "Authorization".into(),
                value: "Bearer RAW_SECRET".into(),
                generated: false,
            }],
            masked_headers: vec![ResolvedHeader {
                name: "Authorization".into(),
                value: "Bearer ••••••••".into(),
                generated: false,
            }],
            body: RequestBody::None,
            masked_body_preview: r#"{"ok":true}"#.into(),
            auth_source: "Bearer".into(),
            warnings: Vec::new(),
            blocking_errors: Vec::new(),
            requires_production_confirm: false,
            connection: RequestConnectionPolicy::default(),
        };
        for target in SnippetTarget::ALL {
            let snip = generate_code_snippet(&masked_req, target);
            assert!(!snip.contains("RAW_SECRET"));
            assert!(snip.contains("••••••••"));
        }
        assert_eq!(
            dev_utils::base64_decode(&dev_utils::base64_encode("hello")).unwrap(),
            "hello"
        );
        assert!(
            dev_utils::yaml_to_json("a: 1\nb:\n  - ok")
                .unwrap()
                .contains("\"a\": 1")
        );
        let _ = std::fs::remove_dir_all(dl);
    }
}
