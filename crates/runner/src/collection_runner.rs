//! Collection Runner (TS02-TS06, TS10): iteration/delay/cancel, extractor chaining,
//! CSV/JSON data-driven runs, pass/fail/skipped/infra classification, contract validation,
//! deterministic run manifests, and JUnit XML / redacted JSON reports.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, time::Duration};
use swaggen_domain::{
    dynamic_vars::apply_response_extractors,
    security::{RedactionPolicy, sanitize_text},
    workspace::{
        Environment, RequestItem, VariableContext, VariableEntry, WorkspaceState, folder_chain,
        resolve_request_preview,
    },
};
use swaggen_http_engine::execute_request;
use swaggen_spec_engine::{
    advanced::validate_response_against_endpoint,
    assertions::evaluate_assertions,
    openapi::SpecDocument,
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetRow {
    pub index: usize,
    pub values: BTreeMap<String, String>,
    pub inferred_types: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetPreview {
    pub format: String,
    pub columns: Vec<String>,
    pub rows: Vec<DatasetRow>,
}

fn infer_type(raw: &str) -> &'static str {
    let t = raw.trim();
    if t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("false") {
        "bool"
    } else if t.parse::<i64>().is_ok() {
        "int"
    } else if t.parse::<f64>().is_ok() {
        "float"
    } else {
        "string"
    }
}

fn split_csv_line(line: &str) -> Result<Vec<String>, String> {
    let mut cols = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            in_quotes = true;
        } else if c == ',' {
            cols.push(cur.trim().to_owned());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    if in_quotes {
        return Err("Unterminated quoted field in CSV dataset".into());
    }
    cols.push(cur.trim().to_owned());
    Ok(cols)
}

/// Parses CSV or JSON array datasets for data-driven runs with column & type preview (TS04).
pub fn parse_dataset(raw: &str) -> Result<DatasetPreview, String> {
    let clean = raw.trim().trim_start_matches('\u{feff}');
    if clean.is_empty() {
        return Err("Dataset input is empty".into());
    }
    if clean.starts_with('[') {
        let arr: Vec<serde_json::Map<String, serde_json::Value>> =
            serde_json::from_str(clean).map_err(|e| format!("Invalid JSON array dataset: {e}"))?;
        let mut columns = Vec::new();
        let mut rows = Vec::new();
        for (idx, obj) in arr.into_iter().enumerate() {
            let mut values = BTreeMap::new();
            let mut inferred = BTreeMap::new();
            for (k, v) in obj {
                if !columns.contains(&k) {
                    columns.push(k.clone());
                }
                let (s, ty) = match v {
                    serde_json::Value::String(s) => (s, "string".to_owned()),
                    serde_json::Value::Number(n) => {
                        let s = n.to_string();
                        let ty = if n.is_i64() || n.is_u64() { "int" } else { "float" };
                        (s, ty.to_owned())
                    }
                    serde_json::Value::Bool(b) => (b.to_string(), "bool".to_owned()),
                    serde_json::Value::Null => (String::new(), "null".to_owned()),
                    other => (other.to_string(), "json".to_owned()),
                };
                values.insert(k.clone(), s);
                inferred.insert(k, ty);
            }
            rows.push(DatasetRow {
                index: idx,
                values,
                inferred_types: inferred,
            });
        }
        return Ok(DatasetPreview {
            format: "json".into(),
            columns,
            rows,
        });
    }

    let mut lines = clean.lines().filter(|l| !l.trim().is_empty());
    let header_line = lines.next().ok_or_else(|| "CSV missing header row".to_owned())?;
    let columns = split_csv_line(header_line)?;
    if columns.is_empty() || columns.iter().any(|c| c.is_empty()) {
        return Err("CSV dataset header must contain non-empty column names".into());
    }
    let mut rows = Vec::new();
    for (idx, line) in lines.enumerate() {
        let cells = split_csv_line(line)?;
        if cells.len() != columns.len() {
            return Err(format!(
                "CSV row {} has {} columns, expected {}",
                idx + 1,
                cells.len(),
                columns.len()
            ));
        }
        let mut values = BTreeMap::new();
        let mut inferred = BTreeMap::new();
        for (col, val) in columns.iter().zip(cells) {
            inferred.insert(col.clone(), infer_type(&val).to_owned());
            values.insert(col.clone(), val);
        }
        rows.push(DatasetRow {
            index: idx,
            values,
            inferred_types: inferred,
        });
    }
    Ok(DatasetPreview {
        format: "csv".into(),
        columns,
        rows,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepOutcome {
    Passed,
    FailedAssertions,
    FailedContract,
    Skipped,
    InfraError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStepResult {
    pub iteration: usize,
    pub dataset_row: Option<usize>,
    pub request_id: String,
    pub request_name: String,
    pub method: String,
    pub redacted_url: String,
    pub status_code: Option<u16>,
    pub duration_ms: u64,
    pub outcome: StepOutcome,
    pub assertions_passed: usize,
    pub assertions_failed: usize,
    pub contract_mismatches: Vec<String>,
    pub extracted_keys: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunManifest {
    pub schema_version: String,
    pub runner_version: String,
    pub workspace_id: String,
    pub environment_name: Option<String>,
    pub iterations: usize,
    pub dataset_rows: usize,
    pub request_ids: Vec<String>,
    pub content_sha256: String,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionRunReport {
    pub manifest: RunManifest,
    pub started_at_unix: i64,
    pub finished_at_unix: i64,
    pub total_duration_ms: u64,
    pub canceled: bool,
    pub passed: usize,
    pub failed_assertions: usize,
    pub failed_contract: usize,
    pub skipped: usize,
    pub infra_errors: usize,
    pub steps: Vec<RunStepResult>,
}

impl CollectionRunReport {
    pub fn has_failures(&self) -> bool {
        self.failed_assertions > 0
            || self.failed_contract > 0
            || self.infra_errors > 0
            || self.canceled
    }

    pub fn to_junit_xml(&self) -> String {
        let total = self.steps.len();
        let failures = self.failed_assertions + self.failed_contract;
        let errors = self.infra_errors;
        let skipped = self.skipped;
        let mut out = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites tests=\"{total}\" failures=\"{failures}\" errors=\"{errors}\" skipped=\"{skipped}\">\n  <testsuite name=\"swaggen-collection-run\" tests=\"{total}\" failures=\"{failures}\" errors=\"{errors}\" skipped=\"{skipped}\">\n"
        );
        for step in &self.steps {
            let name = xml_escape(&format!(
                "iter-{} row-{} {} {}",
                step.iteration + 1,
                step.dataset_row.map(|r| r + 1).unwrap_or(0),
                step.method,
                step.request_name
            ));
            let time_s = format!("{:.3}", step.duration_ms as f64 / 1000.0);
            out.push_str(&format!(
                "    <testcase classname=\"swaggen.runner\" name=\"{name}\" time=\"{time_s}\">\n"
            ));
            match step.outcome {
                StepOutcome::Passed => {}
                StepOutcome::Skipped => out.push_str("      <skipped message=\"Skipped after prior fail-fast\"/>\n"),
                StepOutcome::FailedAssertions | StepOutcome::FailedContract => {
                    let msg = xml_escape(&step.detail);
                    out.push_str(&format!("      <failure message=\"{msg}\">{msg}</failure>\n"));
                }
                StepOutcome::InfraError => {
                    let msg = xml_escape(&step.detail);
                    out.push_str(&format!("      <error message=\"{msg}\">{msg}</error>\n"));
                }
            }
            out.push_str("    </testcase>\n");
        }
        out.push_str("  </testsuite>\n</testsuites>\n");
        out
    }
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[derive(Debug, Clone)]
pub struct RunnerConfig {
    pub request_ids: Vec<String>,
    pub iterations: usize,
    pub delay_ms: u64,
    pub stop_on_failure: bool,
    pub validate_contract: bool,
    pub dataset_rows: Vec<DatasetRow>,
    pub redaction_policy: RedactionPolicy,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            request_ids: Vec::new(),
            iterations: 1,
            delay_ms: 0,
            stop_on_failure: false,
            validate_contract: false,
            dataset_rows: Vec::new(),
            redaction_policy: RedactionPolicy::default(),
        }
    }
}

fn select_requests<'a>(state: &'a WorkspaceState, ids: &[String]) -> Vec<&'a RequestItem> {
    if ids.is_empty() {
        return state.requests.iter().collect();
    }
    ids.iter()
        .filter_map(|id| state.requests.iter().find(|r| r.id == *id))
        .collect()
}

pub fn build_run_manifest(state: &WorkspaceState, cfg: &RunnerConfig, now_unix: i64) -> RunManifest {
    let selected = select_requests(state, &cfg.request_ids);
    let ids: Vec<String> = selected.iter().map(|r| r.id.clone()).collect();
    let env_name = state.active_environment().map(|e| e.name.clone());
    let mut h = Sha256::new();
    h.update(state.meta.id.as_str().as_bytes());
    h.update(env_name.as_deref().unwrap_or("").as_bytes());
    for r in &selected {
        h.update(r.id.as_bytes());
        h.update(r.method.as_str().as_bytes());
        h.update(r.url.as_bytes());
    }
    h.update(cfg.iterations.to_le_bytes());
    h.update(cfg.dataset_rows.len().to_le_bytes());
    RunManifest {
        schema_version: "swaggen.run-manifest.v1".into(),
        runner_version: env!("CARGO_PKG_VERSION").into(),
        workspace_id: state.meta.id.as_str().to_owned(),
        environment_name: env_name,
        iterations: cfg.iterations.max(1),
        dataset_rows: cfg.dataset_rows.len(),
        request_ids: ids,
        content_sha256: hex::encode(h.finalize()),
        created_at_unix: now_unix,
    }
}

/// Runs requests across iterations and optional dataset rows with extractor chaining,
/// fail-fast skipping, OpenAPI contract validation, redaction, and cancellation (TS02-TS06, TS10).
#[allow(clippy::too_many_arguments)]
pub async fn run_collection(
    state: &WorkspaceState,
    specs: &[SpecDocument],
    local_secrets: &BTreeMap<swaggen_domain::LocalId, String>,
    downloads_dir: &Path,
    cfg: &RunnerConfig,
    cancel: CancellationToken,
) -> CollectionRunReport {
    let started_at_unix = time::OffsetDateTime::now_utc().unix_timestamp();
    let t0 = std::time::Instant::now();
    let manifest = build_run_manifest(state, cfg, started_at_unix);
    let selected: Vec<RequestItem> = select_requests(state, &cfg.request_ids)
        .into_iter()
        .cloned()
        .collect();
    let iterations = cfg.iterations.max(1);
    let row_slots: Vec<Option<&DatasetRow>> = if cfg.dataset_rows.is_empty() {
        vec![None]
    } else {
        cfg.dataset_rows.iter().map(Some).collect()
    };

    let mut working_env: Environment = state
        .active_environment()
        .cloned()
        .unwrap_or_else(|| Environment::new("Runner Ephemeral", false));
    let mut working_cookies = state.cookies.clone();
    let mut redaction = cfg.redaction_policy.clone();
    for v in local_secrets.values() {
        if !v.trim().is_empty() {
            redaction.extra_literal_values.push(v.clone());
        }
    }

    let mut steps = Vec::new();
    let mut passed = 0usize;
    let mut failed_assertions = 0usize;
    let mut failed_contract = 0usize;
    let mut skipped = 0usize;
    let mut infra_errors = 0usize;
    let mut canceled = false;

    for iter_idx in 0..iterations {
        for row_opt in &row_slots {
            let mut fail_fast_triggered = false;
            for req in &selected {
                if cancel.is_cancelled() {
                    canceled = true;
                    break;
                }
                if fail_fast_triggered {
                    skipped += 1;
                    steps.push(RunStepResult {
                        iteration: iter_idx,
                        dataset_row: row_opt.map(|r| r.index),
                        request_id: req.id.clone(),
                        request_name: req.name.clone(),
                        method: req.method.as_str().into(),
                        redacted_url: sanitize_text(&req.url, &redaction).sanitized_text,
                        status_code: None,
                        duration_ms: 0,
                        outcome: StepOutcome::Skipped,
                        assertions_passed: 0,
                        assertions_failed: 0,
                        contract_mismatches: Vec::new(),
                        extracted_keys: Vec::new(),
                        detail: "Skipped after fail-fast stop condition".into(),
                    });
                    continue;
                }

                // Inject current dataset row variables into ephemeral env copy.
                let mut iter_env = working_env.clone();
                if let Some(row) = row_opt {
                    for (k, v) in &row.values {
                        if let Some(ex) = iter_env.variables.iter_mut().find(|x| x.key == *k) {
                            ex.shared_value = v.clone();
                        } else {
                            iter_env.variables.push(VariableEntry::public(k, v.clone()));
                        }
                    }
                }

                let col = state.collections.iter().find(|c| c.id == req.collection_id);
                let fchain = folder_chain(&state.folders, req.folder_id.as_deref());
                let col_vars = col.map(|c| c.variables.as_slice()).unwrap_or(&[]);
                let ctx = VariableContext {
                    workspace_vars: &state.workspace_vars,
                    collection_vars: col_vars,
                    environment: Some(&iter_env),
                    request_vars: &req.local_vars,
                    local_secrets,
                };
                let now_secs = time::OffsetDateTime::now_utc().unix_timestamp();
                let resolved = resolve_request_preview(req, col, &fchain, &ctx, &working_cookies, now_secs);

                if !resolved.blocking_errors.is_empty() {
                    infra_errors += 1;
                    let detail = sanitize_text(&resolved.blocking_errors.join("; "), &redaction).sanitized_text;
                    steps.push(RunStepResult {
                        iteration: iter_idx,
                        dataset_row: row_opt.map(|r| r.index),
                        request_id: req.id.clone(),
                        request_name: req.name.clone(),
                        method: req.method.as_str().into(),
                        redacted_url: sanitize_text(&resolved.masked_url, &redaction).sanitized_text,
                        status_code: None,
                        duration_ms: 0,
                        outcome: StepOutcome::InfraError,
                        assertions_passed: 0,
                        assertions_failed: 0,
                        contract_mismatches: Vec::new(),
                        extracted_keys: Vec::new(),
                        detail,
                    });
                    if cfg.stop_on_failure {
                        fail_fast_triggered = true;
                    }
                    continue;
                }

                match execute_request(&resolved, downloads_dir, cancel.clone()).await {
                    Ok(resp) => {
                        working_cookies.extend(resp.captured_cookies.clone());
                        let extracted = apply_response_extractors(
                            &req.extractors,
                            &resp.headers,
                            &resp.preview_bytes,
                        );
                        let mut extracted_keys = Vec::new();
                        for (k, v, _is_secret) in &extracted {
                            extracted_keys.push(k.clone());
                            if let Some(ex) = working_env.variables.iter_mut().find(|x| x.key == *k) {
                                ex.shared_value = v.clone();
                            } else {
                                working_env.variables.push(VariableEntry::public(k, v.clone()));
                            }
                        }

                        let a_res = evaluate_assertions(
                            &req.assertions,
                            resp.status_code,
                            resp.duration_ms,
                            &resp.headers,
                            &resp.preview_bytes,
                        );
                        let assertions_passed = a_res.iter().filter(|a| a.passed).count();
                        let assertions_failed = a_res.len().saturating_sub(assertions_passed);

                        let mut contract_mismatches = Vec::new();
                        if cfg.validate_contract
                            && let Some(op_uid) = &req.linked_operation_uid
                        {
                            for sp in specs {
                                if let Some(ep) = sp.operations.iter().find(|o| o.operation_uid == *op_uid) {
                                    let ct = resp
                                        .headers
                                        .iter()
                                        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                                        .map(|(_, v)| v.as_str())
                                        .unwrap_or("application/json");
                                    let cv = validate_response_against_endpoint(
                                        sp,
                                        ep,
                                        resp.status_code,
                                        ct,
                                        &resp.preview_bytes,
                                    );
                                    contract_mismatches.extend(cv.mismatches);
                                    break;
                                }
                            }
                        }

                        let (outcome, detail) = if assertions_failed > 0 {
                            failed_assertions += 1;
                            if cfg.stop_on_failure {
                                fail_fast_triggered = true;
                            }
                            let msgs: Vec<String> = a_res
                                .into_iter()
                                .filter(|a| !a.passed)
                                .map(|a| format!("{}: {}", a.label, a.detail))
                                .collect();
                            (StepOutcome::FailedAssertions, sanitize_text(&msgs.join("; "), &redaction).sanitized_text)
                        } else if !contract_mismatches.is_empty() {
                            failed_contract += 1;
                            if cfg.stop_on_failure {
                                fail_fast_triggered = true;
                            }
                            (
                                StepOutcome::FailedContract,
                                sanitize_text(&contract_mismatches.join("; "), &redaction).sanitized_text,
                            )
                        } else {
                            passed += 1;
                            (
                                StepOutcome::Passed,
                                format!("HTTP {} · {} assertion(s) passed", resp.status_code, assertions_passed),
                            )
                        };

                        steps.push(RunStepResult {
                            iteration: iter_idx,
                            dataset_row: row_opt.map(|r| r.index),
                            request_id: req.id.clone(),
                            request_name: req.name.clone(),
                            method: req.method.as_str().into(),
                            redacted_url: sanitize_text(&resolved.masked_url, &redaction).sanitized_text,
                            status_code: Some(resp.status_code),
                            duration_ms: resp.duration_ms,
                            outcome,
                            assertions_passed,
                            assertions_failed,
                            contract_mismatches,
                            extracted_keys,
                            detail,
                        });
                    }
                    Err(err) => {
                        if err.kind == swaggen_http_engine::HttpErrorKind::Canceled {
                            canceled = true;
                        }
                        infra_errors += 1;
                        if cfg.stop_on_failure {
                            fail_fast_triggered = true;
                        }
                        steps.push(RunStepResult {
                            iteration: iter_idx,
                            dataset_row: row_opt.map(|r| r.index),
                            request_id: req.id.clone(),
                            request_name: req.name.clone(),
                            method: req.method.as_str().into(),
                            redacted_url: sanitize_text(&resolved.masked_url, &redaction).sanitized_text,
                            status_code: None,
                            duration_ms: err.duration_ms,
                            outcome: StepOutcome::InfraError,
                            assertions_passed: 0,
                            assertions_failed: 0,
                            contract_mismatches: Vec::new(),
                            extracted_keys: Vec::new(),
                            detail: sanitize_text(&format!("{}: {}", err.kind.as_str(), err.message), &redaction).sanitized_text,
                        });
                        if canceled {
                            break;
                        }
                    }
                }

                if cfg.delay_ms > 0 {
                    tokio::select! {
                        _ = cancel.cancelled() => { canceled = true; break; }
                        _ = tokio::time::sleep(Duration::from_millis(cfg.delay_ms)) => {}
                    }
                }
            }
            if canceled {
                break;
            }
        }
        if canceled {
            break;
        }
    }

    CollectionRunReport {
        manifest,
        started_at_unix,
        finished_at_unix: time::OffsetDateTime::now_utc().unix_timestamp(),
        total_duration_ms: t0.elapsed().as_millis() as u64,
        canceled,
        passed,
        failed_assertions,
        failed_contract,
        skipped,
        infra_errors,
        steps,
    }
}

/// TS09: Bounded parallel runner executing independent dataset rows / iterations under a semaphore
/// without shared mutable environment races, preserving input order.
pub async fn run_collection_parallel(
    state: &WorkspaceState,
    specs: &[SpecDocument],
    local_secrets: &BTreeMap<swaggen_domain::LocalId, String>,
    downloads_dir: &Path,
    cfg: &RunnerConfig,
    concurrency_limit: usize,
    cancel: CancellationToken,
) -> Vec<CollectionRunReport> {
    let cap = concurrency_limit.clamp(1, 32);
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(cap));
    let mut handles = Vec::new();
    let shards: Vec<Vec<DatasetRow>> = if cfg.dataset_rows.is_empty() {
        (0..cfg.iterations.max(1)).map(|_| Vec::new()).collect()
    } else {
        cfg.dataset_rows.iter().cloned().map(|r| vec![r]).collect()
    };

    for (shard_idx, rows) in shards.into_iter().enumerate() {
        let permit = sem.clone().acquire_owned().await;
        let Ok(permit) = permit else { break };
        let st = state.clone();
        let sp = specs.to_vec();
        let sec = local_secrets.clone();
        let dl = downloads_dir.to_path_buf();
        let tok = cancel.clone();
        let mut sub_cfg = cfg.clone();
        sub_cfg.iterations = 1;
        sub_cfg.dataset_rows = rows;
        handles.push((
            shard_idx,
            tokio::spawn(async move {
                let _p = permit;
                run_collection(&st, &sp, &sec, &dl, &sub_cfg, tok).await
            }),
        ));
    }

    let mut ordered = vec![None; handles.len()];
    for (idx, h) in handles {
        if let Ok(rep) = h.await {
            ordered[idx] = Some(rep);
        }
    }
    ordered.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, routing::{get, post}, Json};
    use swaggen_domain::dynamic_vars::{ExtractorSource, ResponseExtractorRule};
    use swaggen_domain::workspace::{AssertionRule, AssertionTarget, HttpMethod};

    #[tokio::test]
    async fn collection_runner_chains_extractors_dataset_fail_fast_and_junit_cleanly() {
        let app = Router::new()
            .route("/login", post(|| async { Json(serde_json::json!({"token": "tok-abc"})) }))
            .route("/items/{id}", get(|axum::extract::Path(id): axum::extract::Path<String>| async move {
                Json(serde_json::json!({"id": id, "ok": true}))
            }))
            .route("/fail", get(|| async { (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "boom") }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let mut ws = WorkspaceState::new_personal(swaggen_domain::LocalId::parse("run").unwrap(), "Run");
        let cid = ws.collections[0].id.clone();
        let mut r1 = RequestItem::new(&cid, "Login", HttpMethod::Post, format!("http://{addr}/login"));
        r1.extractors.push(ResponseExtractorRule {
            id: "ex1".into(),
            target_var: "auth_tok".into(),
            source: ExtractorSource::BodyJsonPath,
            expression: "$.token".into(),
            mark_secret: false,
        });
        let mut r2 = RequestItem::new(&cid, "GetItem", HttpMethod::Get, format!("http://{addr}/items/{{{{item_id}}}}"));
        r2.headers.push(swaggen_domain::workspace::KeyValueRow::new("X-Auth", "{{auth_tok}}"));
        r2.assertions.push(AssertionRule {
            id: "a1".into(),
            label: "200".into(),
            target: AssertionTarget::StatusEquals,
            expected: "200".into(),
            enabled: true,
        });
        let mut r3 = RequestItem::new(&cid, "ShouldFail", HttpMethod::Get, format!("http://{addr}/fail"));
        r3.assertions.push(AssertionRule {
            id: "a2".into(),
            label: "Expect 200".into(),
            target: AssertionTarget::StatusEquals,
            expected: "200".into(),
            enabled: true,
        });
        let r4 = RequestItem::new(&cid, "AfterFail", HttpMethod::Get, format!("http://{addr}/login"));
        ws.requests = vec![r1.clone(), r2.clone(), r3.clone(), r4.clone()];

        let ds = parse_dataset("item_id,flag\n42,true\n").unwrap();
        assert_eq!(ds.rows[0].inferred_types["item_id"], "int");
        assert_eq!(ds.rows[0].inferred_types["flag"], "bool");

        let dl = std::env::temp_dir().join(format!("swaggen-run-dl-{}", std::process::id()));
        let mut sec = BTreeMap::new();
        sec.insert(swaggen_domain::LocalId::parse("my_secret").unwrap(), "SECRET_VAL_XYZ".into());
        let cfg = RunnerConfig {
            request_ids: vec![r1.id.clone(), r2.id, r3.id, r4.id],
            iterations: 1,
            delay_ms: 0,
            stop_on_failure: true,
            validate_contract: false,
            dataset_rows: ds.rows,
            redaction_policy: RedactionPolicy::default(),
        };
        let rep = run_collection(&ws, &[], &sec, &dl, &cfg, CancellationToken::new()).await;
        assert_eq!(rep.passed, 2);
        assert_eq!(rep.failed_assertions, 1);
        assert_eq!(rep.skipped, 1);
        assert_eq!(rep.infra_errors, 0);
        let par_cfg = RunnerConfig {
            request_ids: vec![r1.id.clone()],
            iterations: 3,
            ..RunnerConfig::default()
        };
        let par_reps = run_collection_parallel(&ws, &[], &sec, &dl, &par_cfg, 2, CancellationToken::new()).await;
        assert_eq!(par_reps.len(), 3);
        assert!(par_reps.iter().all(|r| r.passed == 1));
        let xml = rep.to_junit_xml();
        assert!(xml.contains("failures=\"1\""));
        assert!(xml.contains("<skipped"));
        let _ = std::fs::remove_dir_all(dl);
    }
}
