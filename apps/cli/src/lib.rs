//! Headless Swaggen CLI library: runs exported collections/environments with redacted outputs
//! and explicit CI exit codes (0=pass, 1=test/contract failures, 2=infra/usage error) (MK05, MK06).
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use swaggen_domain::{
    LocalId,
    security::{RedactionPolicy, sanitize_text},
    workspace::{Environment, WorkspaceState, resolve_request_preview},
};
use swaggen_local_store::WorkspaceDirectory;
use swaggen_runner::{
    collection_runner::{CollectionRunReport, RunnerConfig, parse_dataset, run_collection},
    protocols_and_tools::{SnippetTarget, generate_code_snippet},
};
use swaggen_secret_store::SecretStore;
use swaggen_spec_engine::openapi::{SpecDocument, parse_and_validate};
use tokio_util::sync::CancellationToken;

pub const EXIT_OK: i32 = 0;
pub const EXIT_TEST_FAILURE: i32 = 1;
pub const EXIT_INFRA_OR_USAGE: i32 = 2;

#[derive(Debug, Clone)]
pub struct CliRunArgs {
    pub workspace_path: PathBuf,
    pub env_path: Option<PathBuf>,
    pub dataset_path: Option<PathBuf>,
    pub spec_paths: Vec<PathBuf>,
    pub report_json_path: Option<PathBuf>,
    pub junit_path: Option<PathBuf>,
    pub secrets_root: Option<PathBuf>,
    pub workspace_id: String,
    pub stop_on_failure: bool,
    pub validate_contract: bool,
    pub iterations: usize,
    pub delay_ms: u64,
    pub extra_redact_fields: Vec<String>,
}

/// Loads local secrets from `SWAGGEN_SECRET_<NAME>` env vars and optional DPAPI workspace root (MK05).
pub fn collect_local_secrets(
    secrets_root: Option<&Path>,
    workspace_id: &str,
) -> BTreeMap<LocalId, String> {
    let mut map = BTreeMap::new();
    if let Some(root) = secrets_root
        && let Ok(ws_id) = LocalId::parse(workspace_id)
        && let Ok(dir) = WorkspaceDirectory::open(root, ws_id.clone())
        && let Ok(store) = SecretStore::open(dir)
    {
        for name in store.list_names() {
            if let Ok(sec) = store.get(&swaggen_domain::SecretRef {
                workspace: ws_id.clone(),
                name: name.clone(),
            }) && let Ok(s) = std::str::from_utf8(sec.expose())
            {
                map.insert(name, s.to_owned());
            }
        }
    }
    for (k, v) in std::env::vars() {
        if let Some(suffix) = k.strip_prefix("SWAGGEN_SECRET_")
            && let Ok(id) = LocalId::parse(suffix.to_ascii_lowercase())
        {
            map.insert(id, v);
        }
    }
    map
}

pub async fn execute_cli_run(args: &CliRunArgs) -> Result<(CollectionRunReport, i32), String> {
    let raw_ws = fs::read_to_string(&args.workspace_path).map_err(|e| {
        format!(
            "Failed reading workspace '{}': {e}",
            args.workspace_path.display()
        )
    })?;
    let mut ws: WorkspaceState = serde_json::from_str(&raw_ws).map_err(|e| {
        format!(
            "Invalid WorkspaceState JSON '{}': {e}",
            args.workspace_path.display()
        )
    })?;

    if let Some(ep) = &args.env_path {
        let raw_env = fs::read_to_string(ep)
            .map_err(|e| format!("Failed reading environment '{}': {e}", ep.display()))?;
        let env: Environment = serde_json::from_str(&raw_env)
            .map_err(|e| format!("Invalid Environment JSON '{}': {e}", ep.display()))?;
        ws.active_environment_id = Some(env.id.clone());
        ws.environments.retain(|x| x.id != env.id);
        ws.environments.push(env);
    }

    let dataset_rows = if let Some(dp) = &args.dataset_path {
        let raw_ds = fs::read_to_string(dp)
            .map_err(|e| format!("Failed reading dataset '{}': {e}", dp.display()))?;
        parse_dataset(&raw_ds)?.rows
    } else {
        Vec::new()
    };

    let mut specs: Vec<SpecDocument> = Vec::new();
    for sp in &args.spec_paths {
        let raw_sp = fs::read_to_string(sp)
            .map_err(|e| format!("Failed reading spec '{}': {e}", sp.display()))?;
        let name = sp
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("spec")
            .to_owned();
        specs.push(parse_and_validate(&name, &name, &raw_sp, &[]));
    }

    let secrets = collect_local_secrets(args.secrets_root.as_deref(), &args.workspace_id);
    let mut redaction = RedactionPolicy::default();
    redaction
        .extra_fields
        .extend(args.extra_redact_fields.clone());
    for v in secrets.values() {
        if !v.trim().is_empty() {
            redaction.extra_literal_values.push(v.clone());
        }
    }

    let downloads_dir = std::env::temp_dir().join(format!("swaggen-cli-dl-{}", std::process::id()));
    let _ = fs::create_dir_all(&downloads_dir);
    let cfg = RunnerConfig {
        request_ids: Vec::new(),
        iterations: args.iterations,
        delay_ms: args.delay_ms,
        stop_on_failure: args.stop_on_failure,
        validate_contract: args.validate_contract,
        dataset_rows,
        redaction_policy: redaction.clone(),
    };
    let report = run_collection(
        &ws,
        &specs,
        &secrets,
        &downloads_dir,
        &cfg,
        CancellationToken::new(),
    )
    .await;
    let _ = fs::remove_dir_all(&downloads_dir);

    if let Some(jp) = &args.report_json_path {
        let json_txt = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        let safe_json = sanitize_text(&json_txt, &redaction).sanitized_text;
        fs::write(jp, safe_json).map_err(|e| format!("Failed writing JSON report: {e}"))?;
    }
    if let Some(xp) = &args.junit_path {
        let junit = sanitize_text(&report.to_junit_xml(), &redaction).sanitized_text;
        fs::write(xp, junit).map_err(|e| format!("Failed writing JUnit XML: {e}"))?;
    }

    let code = if report.infra_errors > 0 || report.canceled {
        EXIT_INFRA_OR_USAGE
    } else if report.failed_assertions > 0 || report.failed_contract > 0 {
        EXIT_TEST_FAILURE
    } else {
        EXIT_OK
    };
    Ok((report, code))
}

pub fn generate_cli_snippet(
    workspace_path: &Path,
    request_index: usize,
    target: &str,
) -> Result<String, String> {
    let raw_ws =
        fs::read_to_string(workspace_path).map_err(|e| format!("Failed reading workspace: {e}"))?;
    let ws: WorkspaceState =
        serde_json::from_str(&raw_ws).map_err(|e| format!("Invalid WorkspaceState JSON: {e}"))?;
    let req = ws
        .requests
        .get(request_index)
        .ok_or_else(|| "Request index out of range".to_owned())?;
    let empty = BTreeMap::new();
    let ctx = swaggen_domain::workspace::VariableContext {
        workspace_vars: &ws.workspace_vars,
        collection_vars: &[],
        environment: ws.active_environment(),
        request_vars: &req.local_vars,
        local_secrets: &empty,
    };
    let resolved = resolve_request_preview(req, None, &[], &ctx, &ws.cookies, 0);
    let tgt = match target.to_ascii_lowercase().as_str() {
        "curl" => SnippetTarget::Curl,
        "rust" | "reqwest" => SnippetTarget::RustReqwest,
        "js" | "fetch" => SnippetTarget::JsFetch,
        "dart" => SnippetTarget::DartHttp,
        "go" => SnippetTarget::GoHttp,
        other => return Err(format!("Unsupported snippet target '{other}'")),
    };
    Ok(generate_code_snippet(&resolved, tgt))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, routing::get};
    use swaggen_domain::workspace::{AssertionRule, AssertionTarget, HttpMethod, RequestItem};

    #[tokio::test]
    async fn cli_runner_writes_redacted_junit_and_returns_accurate_exit_codes() {
        let app = Router::new()
            .route(
                "/ok",
                get(|| async { Json(serde_json::json!({"ok": true})) }),
            )
            .route(
                "/bad",
                get(|| async { (axum::http::StatusCode::BAD_REQUEST, "nope") }),
            );
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(l, app).await;
        });

        let dir = std::env::temp_dir().join(format!("swaggen-cli-t-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let ws_path = dir.join("ws.json");
        let junit_path = dir.join("junit.xml");
        let json_path = dir.join("report.json");

        let mut ws = WorkspaceState::new_personal(LocalId::parse("cli").unwrap(), "CLI");
        let cid = ws.collections[0].id.clone();
        let mut r1 = RequestItem::new(
            &cid,
            "PassReq",
            HttpMethod::Get,
            format!("http://{addr}/ok?api_key={{{{cli_tok}}}}"),
        );
        r1.assertions.push(AssertionRule {
            id: "a1".into(),
            label: "Status 200".into(),
            target: AssertionTarget::StatusEquals,
            expected: "200".into(),
            enabled: true,
        });
        ws.requests = vec![r1];
        fs::write(&ws_path, serde_json::to_string(&ws).unwrap()).unwrap();

        let args = CliRunArgs {
            workspace_path: ws_path.clone(),
            env_path: None,
            dataset_path: None,
            spec_paths: Vec::new(),
            report_json_path: Some(json_path.clone()),
            junit_path: Some(junit_path.clone()),
            secrets_root: None,
            workspace_id: "cli".into(),
            stop_on_failure: false,
            validate_contract: false,
            iterations: 1,
            delay_ms: 0,
            extra_redact_fields: Vec::new(),
        };
        // Expect blocked variable unresolved infra error first (2), then with URL without missing var expect 0, then 1 on failing assertion.
        let (_, code_missing) = execute_cli_run(&args).await.unwrap();
        assert_eq!(code_missing, EXIT_INFRA_OR_USAGE);

        ws.requests[0].url = format!("http://{addr}/ok?api_key=TOPSECRET99");
        ws.requests[0].sync_params_from_url();
        fs::write(&ws_path, serde_json::to_string(&ws).unwrap()).unwrap();
        let (_, code_ok) = execute_cli_run(&args).await.unwrap();
        assert_eq!(code_ok, EXIT_OK);
        let junit_txt = fs::read_to_string(&junit_path).unwrap();
        assert!(!junit_txt.contains("TOPSECRET99"));

        ws.requests[0].url = format!("http://{addr}/bad");
        ws.requests[0].sync_params_from_url();
        fs::write(&ws_path, serde_json::to_string(&ws).unwrap()).unwrap();
        let (_, code_fail) = execute_cli_run(&args).await.unwrap();
        assert_eq!(code_fail, EXIT_TEST_FAILURE);
        let _ = fs::remove_dir_all(&dir);
    }
}
