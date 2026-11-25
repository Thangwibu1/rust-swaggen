//! Local Mock Server generated from sanitized ResponseExamples & schema-seeded OpenAPI specs (MK01, MK02).
use crate::advanced_protocols::{MockScenarioRule, match_scenario_rule};
use axum::{
    Router,
    body::Bytes,
    extract::{OriginalUri, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use swaggen_domain::{
    security::{RedactionPolicy, sanitize_headers, sanitize_text},
    workspace::WorkspaceState,
};
use swaggen_spec_engine::openapi::SpecDocument;
use tokio::sync::oneshot;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MockRoute {
    pub method: String,
    pub path: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub source_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MockTrafficLog {
    pub method: String,
    pub path: String,
    pub status: u16,
    pub matched_source: String,
    pub body_preview_redacted: String,
}

fn synth_schema_value(schema: &Value, root: &Value, seed: u64, depth: usize) -> Value {
    if depth >= 5 {
        return json!({});
    }
    if let Some(r) = schema.get("$ref").and_then(Value::as_str)
        && let Some(ptr) = r.strip_prefix('#')
        && let Some(target) = root.pointer(ptr)
    {
        return synth_schema_value(target, root, seed, depth + 1);
    }
    if let Some(ex) = schema.get("example") {
        return ex.clone();
    }
    if let Some(en) = schema.get("enum").and_then(Value::as_array)
        && let Some(first) = en.first()
    {
        return first.clone();
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("integer") => json!((seed % 900) + 1),
        Some("number") => json!(((seed % 900) as f64) / 10.0),
        Some("boolean") => json!(seed.is_multiple_of(2)),
        Some("array") => {
            let item = schema
                .get("items")
                .map(|i| synth_schema_value(i, root, seed + 1, depth + 1))
                .unwrap_or_else(|| json!("item"));
            json!([item])
        }
        Some("object") => {
            let mut map = serde_json::Map::new();
            if let Some(props) = schema.get("properties").and_then(Value::as_object) {
                for (idx, (k, ps)) in props.iter().enumerate() {
                    map.insert(
                        k.clone(),
                        synth_schema_value(ps, root, seed.wrapping_add(idx as u64 + 3), depth + 1),
                    );
                }
            }
            Value::Object(map)
        }
        _ => json!(format!("mock-value-{seed}")),
    }
}

/// Builds deterministic mock routes from saved ResponseExamples (MK01) + OpenAPI specs (MK02).
pub fn build_mock_routes(
    state: &WorkspaceState,
    specs: &[SpecDocument],
    seed: u64,
) -> Vec<MockRoute> {
    let mut routes = Vec::new();
    for ex in &state.examples {
        if let Some(req) = state.requests.iter().find(|r| r.id == ex.request_id)
            && let Ok(parsed) = url::Url::parse(&req.url)
        {
            routes.push(MockRoute {
                method: req.method.as_str().into(),
                path: parsed.path().to_owned(),
                status: ex.status_code,
                headers: ex.headers.clone(),
                body: ex.body_preview.clone(),
                source_note: format!("example:{}", ex.name),
            });
        }
    }
    for sp in specs {
        let root_json = serde_json::from_str::<Value>(&sp.raw_text).unwrap_or(Value::Null);
        for (idx, op) in sp.operations.iter().enumerate() {
            let already = routes.iter().any(|r| {
                r.method.eq_ignore_ascii_case(op.method.as_str()) && path_matches(&r.path, &op.path)
            });
            if already {
                continue;
            }
            let body_val = root_json
                .pointer(&format!(
                    "/paths/{}/{}/responses/200/content/application~1json/schema",
                    op.path.replace('~', "~0").replace('/', "~1"),
                    op.method.as_str().to_ascii_lowercase()
                ))
                .map(|s| synth_schema_value(s, &root_json, seed + idx as u64, 0))
                .unwrap_or_else(|| json!({"operationId": op.operation_id, "mocked": true}));
            routes.push(MockRoute {
                method: op.method.as_str().into(),
                path: op.path.clone(),
                status: 200,
                headers: vec![("content-type".into(), "application/json".into())],
                body: serde_json::to_string_pretty(&body_val).unwrap_or_default(),
                source_note: format!("spec:{}", sp.name),
            });
        }
    }
    routes
}

fn path_matches(pattern: &str, actual: &str) -> bool {
    let p_segs: Vec<&str> = pattern.trim_matches('/').split('/').collect();
    let a_segs: Vec<&str> = actual.trim_matches('/').split('/').collect();
    if p_segs.len() != a_segs.len() {
        return false;
    }
    p_segs
        .iter()
        .zip(a_segs.iter())
        .all(|(p, a)| (p.starts_with('{') && p.ends_with('}')) || p.starts_with(':') || p == a)
}

pub struct LocalMockHandle {
    pub base_url: String,
    pub traffic_log: Arc<Mutex<Vec<MockTrafficLog>>>,
    shutdown: Option<oneshot::Sender<()>>,
}

impl LocalMockHandle {
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for LocalMockHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Clone)]
struct MockAppState {
    routes: Arc<Vec<MockRoute>>,
    scenarios: Arc<Vec<MockScenarioRule>>,
    logs: Arc<Mutex<Vec<MockTrafficLog>>>,
}

async fn mock_handler(
    State(st): State<MockAppState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path = uri.path();
    let query = uri.query().unwrap_or("");
    let req_headers: Vec<(String, String)> = headers
        .iter()
        .map(|(k, v)| (k.as_str().to_owned(), v.to_str().unwrap_or("").to_owned()))
        .collect();
    let _sanitized_hdrs = sanitize_headers(&req_headers, &RedactionPolicy::default());
    let body_txt = String::from_utf8_lossy(&body);
    let body_red = sanitize_text(&body_txt, &RedactionPolicy::default()).sanitized_text;

    if let Some((rule, _)) = match_scenario_rule(
        &st.scenarios,
        method.as_str(),
        path,
        &req_headers,
        query,
        &body_txt,
    ) {
        if rule.delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(rule.delay_ms.min(10_000))).await;
        }
        if let Ok(mut logs) = st.logs.lock() {
            if logs.len() >= 200 {
                logs.remove(0);
            }
            logs.push(MockTrafficLog {
                method: method.as_str().into(),
                path: path.to_owned(),
                status: rule.status,
                matched_source: format!("scenario:{}", rule.id),
                body_preview_redacted: body_red.chars().take(512).collect(),
            });
        }
        let status = StatusCode::from_u16(rule.status).unwrap_or(StatusCode::OK);
        let mut resp = (status, rule.response_body.clone()).into_response();
        resp.headers_mut().insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );
        return resp;
    }

    if let Some(matched) = st
        .routes
        .iter()
        .find(|r| r.method.eq_ignore_ascii_case(method.as_str()) && path_matches(&r.path, path))
    {
        if let Ok(mut logs) = st.logs.lock() {
            if logs.len() >= 200 {
                logs.remove(0);
            }
            logs.push(MockTrafficLog {
                method: method.as_str().into(),
                path: path.to_owned(),
                status: matched.status,
                matched_source: matched.source_note.clone(),
                body_preview_redacted: body_red.chars().take(512).collect(),
            });
        }
        let status = StatusCode::from_u16(matched.status).unwrap_or(StatusCode::OK);
        let mut resp = (status, matched.body.clone()).into_response();
        for (k, v) in &matched.headers {
            if let Ok(hn) = HeaderName::from_bytes(k.as_bytes())
                && let Ok(hv) = HeaderValue::from_str(v)
            {
                resp.headers_mut().insert(hn, hv);
            }
        }
        return resp;
    }
    (
        StatusCode::NOT_FOUND,
        serde_json::to_string(
            &json!({"error": "No mock route matched", "method": method.as_str(), "path": path}),
        )
        .unwrap_or_default(),
    )
        .into_response()
}

pub async fn start_local_mock(routes: Vec<MockRoute>) -> Result<LocalMockHandle, String> {
    start_local_mock_with_options(routes, Vec::new(), 0).await
}

pub async fn start_local_mock_with_options(
    routes: Vec<MockRoute>,
    scenarios: Vec<MockScenarioRule>,
    port: u16,
) -> Result<LocalMockHandle, String> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| format!("Failed to bind local mock server on 127.0.0.1:{port}: {e}"))?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    let logs = Arc::new(Mutex::new(Vec::new()));
    let app_state = MockAppState {
        routes: Arc::new(routes),
        scenarios: Arc::new(scenarios),
        logs: logs.clone(),
    };
    let app = Router::new().fallback(mock_handler).with_state(app_state);
    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
            })
            .await;
    });
    Ok(LocalMockHandle {
        base_url: format!("http://{addr}"),
        traffic_log: logs,
        shutdown: Some(tx),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use swaggen_domain::workspace::build_sanitized_example;
    use swaggen_spec_engine::openapi::parse_and_validate;

    #[tokio::test]
    async fn local_mock_serves_example_and_deterministic_schema_seed_and_redacts_logs() {
        let mut ws =
            WorkspaceState::new_personal(swaggen_domain::LocalId::parse("mk").unwrap(), "MK");
        let cid = ws.collections[0].id.clone();
        let req = swaggen_domain::workspace::RequestItem::new(
            &cid,
            "Health",
            swaggen_domain::workspace::HttpMethod::Get,
            "https://api.example.test/health",
        );
        let ex = build_sanitized_example(
            &req.id,
            "OK",
            200,
            &[("content-type".into(), "application/json".into())],
            r#"{"status":"healthy"}"#,
            &RedactionPolicy::default(),
            1,
        );
        ws.requests.push(req);
        ws.examples.push(ex);

        let spec_json = r#"{
          "openapi":"3.1.0",
          "info":{"title":"M","version":"1"},
          "paths":{"/users/{id}":{"get":{"operationId":"getUser","responses":{"200":{"description":"ok","content":{"application/json":{"schema":{"type":"object","properties":{"id":{"type":"integer"},"role":{"type":"string","enum":["admin","member"]}}}}}}}}}}
        }"#;
        let sp = parse_and_validate("s1", "M", spec_json, &[]);
        let r1 = build_mock_routes(&ws, std::slice::from_ref(&sp), 77);
        let r2 = build_mock_routes(&ws, &[sp], 77);
        assert_eq!(r1, r2);
        let mut handle = start_local_mock(r1).await.unwrap();

        let dl = std::env::temp_dir().join(format!("swaggen-mk-{}", std::process::id()));
        let req_item = swaggen_domain::workspace::RequestItem::new(
            &cid,
            "CallMock",
            swaggen_domain::workspace::HttpMethod::Get,
            format!("{}/users/99", handle.base_url),
        );
        let empty = std::collections::BTreeMap::new();
        let ctx = swaggen_domain::workspace::VariableContext {
            workspace_vars: &[],
            collection_vars: &[],
            environment: None,
            request_vars: &[],
            local_secrets: &empty,
        };
        let resolved =
            swaggen_domain::workspace::resolve_request_preview(&req_item, None, &[], &ctx, &[], 1);
        let res = swaggen_http_engine::execute_request(
            &resolved,
            &dl,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(res.status_code, 200);
        let txt = String::from_utf8_lossy(&res.preview_bytes);
        assert!(txt.contains("admin"));
        assert!(!handle.traffic_log.lock().unwrap().is_empty());
        handle.shutdown();
        let _ = std::fs::remove_dir_all(dl);
    }
}
