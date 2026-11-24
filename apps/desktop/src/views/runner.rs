//! Version B Runner, Mock Server, GraphQL/SSE Clients & Offline Dev Utilities UI (TS02-06, TS10, MK01-02, PR01, PR04, PR09).
use crate::{app::App, worker::Command};
use eframe::egui;
use swaggen_runner::{
    collection_runner::{RunnerConfig, StepOutcome, parse_dataset},
    local_mock::build_mock_routes,
    protocols_and_tools::dev_utils,
};
use swaggen_ui_kit::NoticeKind;

pub fn show(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    swaggen_ui_kit::section(
        ui,
        "Runner, Mock Server, Protocols & Dev Tools (P08)",
        "Execute collection runs with CSV/JSON datasets & JUnit reports, spin up local schema-seeded mocks, query GraphQL, listen to bounded SSE, and run offline codecs.",
    );

    ui.horizontal_wrapped(|ui| {
        for (idx, lbl) in [
            (0usize, "Collection Runner (TS02–06 / TS10)"),
            (1, "Local Mock Server (MK01–02)"),
            (2, "GraphQL & SSE (PR01 / PR04)"),
            (3, "Dev Utilities (PR09)"),
            (4, "Script Sandbox & Extensions (TS07 / TS08 / PR10)"),
        ] {
            if ui.selectable_label(app.runner_subtab == idx, lbl).clicked() {
                app.runner_subtab = idx;
            }
        }
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| match app.runner_subtab {
        0 => show_collection_runner(app, ui, ctx),
        1 => show_mock_server(app, ui),
        2 => show_graphql_sse(app, ui),
        3 => show_dev_utils(app, ui, ctx),
        _ => show_sandbox_and_extensions(app, ui),
    });
}

fn show_collection_runner(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Iterations:");
        ui.add(egui::DragValue::new(&mut app.runner_iterations).range(1..=100));
        ui.label("Delay (ms):");
        ui.add(egui::DragValue::new(&mut app.runner_delay_ms).range(0..=10_000));
        ui.checkbox(&mut app.runner_stop_on_fail, "Stop on fail-fast (TS03)");
        ui.checkbox(
            &mut app.runner_validate_contract,
            "Validate OpenAPI contract (TS06)",
        );
    });
    ui.label("Optional CSV or JSON dataset (TS04):");
    ui.add(
        egui::TextEdit::multiline(&mut app.runner_dataset_text)
            .desired_width(ui.available_width())
            .desired_rows(3)
            .code_editor(),
    );
    if !app.runner_dataset_text.trim().is_empty() {
        match parse_dataset(&app.runner_dataset_text) {
            Ok(prev) => {
                ui.label(format!(
                    "Dataset ({}) · columns: {} · rows: {}",
                    prev.format,
                    prev.columns.join(", "),
                    prev.rows.len()
                ));
            }
            Err(e) => {
                swaggen_ui_kit::notice_banner(ui, NoticeKind::Danger, "Dataset parse error", &e);
            }
        }
    }
    ui.horizontal_wrapped(|ui| {
        if app.runner_running {
            if swaggen_ui_kit::danger_button(ui, true, "Cancel run").clicked() {
                app.submit(Command::CancelRunner);
            }
        } else if swaggen_ui_kit::primary_button(ui, app.loaded, "Run workspace collection")
            .clicked()
        {
            let dataset_rows = if app.runner_dataset_text.trim().is_empty() {
                Vec::new()
            } else {
                parse_dataset(&app.runner_dataset_text)
                    .map(|p| p.rows)
                    .unwrap_or_default()
            };
            app.runner_running = true;
            app.submit(Command::RunCollection {
                state: Box::new(app.state.clone()),
                specs: app.specs.clone(),
                config: RunnerConfig {
                    request_ids: Vec::new(),
                    iterations: app.runner_iterations,
                    delay_ms: app.runner_delay_ms,
                    stop_on_failure: app.runner_stop_on_fail,
                    validate_contract: app.runner_validate_contract,
                    dataset_rows,
                    redaction_policy: Default::default(),
                },
            });
        }
        if let Some(rep) = &app.last_run_report {
            if swaggen_ui_kit::secondary_button(ui, true, "Copy JUnit XML (TS05/MK06)").clicked() {
                ctx.copy_text(rep.to_junit_xml());
                app.status = "Copied sanitized JUnit XML report".into();
            }
            if swaggen_ui_kit::secondary_button(ui, true, "Export JSON & JUnit files").clicked() {
                let dir = app.root.join("runner-reports");
                let _ = std::fs::create_dir_all(&dir);
                if let Ok(js) = serde_json::to_string_pretty(rep) {
                    let _ = std::fs::write(dir.join("run-report.json"), js);
                }
                let _ = std::fs::write(dir.join("junit.xml"), rep.to_junit_xml());
                app.status = format!("Saved run reports to {}", dir.display());
            }
        }
    });
    if let Some(rep) = &app.last_run_report {
        ui.add_space(6.0);
        ui.monospace(format!(
            "Manifest SHA-256 (TS10): {} · Passed={} FailedAssert={} FailedContract={} Skipped={} Infra={} ({} ms)",
            rep.manifest.content_sha256,
            rep.passed,
            rep.failed_assertions,
            rep.failed_contract,
            rep.skipped,
            rep.infra_errors,
            rep.total_duration_ms
        ));
        for s in &rep.steps {
            let tag = match s.outcome {
                StepOutcome::Passed => "PASS",
                StepOutcome::FailedAssertions => "FAIL_ASSERT",
                StepOutcome::FailedContract => "FAIL_CONTRACT",
                StepOutcome::Skipped => "SKIPPED",
                StepOutcome::InfraError => "INFRA_ERR",
            };
            ui.monospace(format!(
                "[iter {}] [{}] {} {} -> {:?} ({} ms): {}",
                s.iteration + 1,
                tag,
                s.method,
                s.request_name,
                s.status_code,
                s.duration_ms,
                s.detail
            ));
        }
    }
}

fn show_mock_server(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Deterministic seed (MK02):");
        ui.add(egui::DragValue::new(&mut app.mock_seed));
        if app.local_mock_url.is_none() {
            if swaggen_ui_kit::primary_button(ui, true, "Start Local Mock Server").clicked() {
                let routes = build_mock_routes(&app.state, &app.specs, app.mock_seed);
                app.submit(Command::StartLocalMock { routes });
            }
        } else {
            if swaggen_ui_kit::danger_button(ui, true, "Stop Mock Server").clicked() {
                app.submit(Command::StopLocalMock);
            }
            if swaggen_ui_kit::secondary_button(ui, true, "Refresh traffic log").clicked() {
                app.submit(Command::PollMockTraffic);
            }
        }
    });
    if let Some(url) = &app.local_mock_url {
        swaggen_ui_kit::notice_banner(
            ui,
            NoticeKind::Success,
            "Local mock listening",
            &format!("Base URL: {url}"),
        );
    }
    for entry in &app.local_mock_logs {
        ui.monospace(format!(
            "{} {} -> HTTP {} [{}] {}",
            entry.method,
            entry.path,
            entry.status,
            entry.matched_source,
            entry.body_preview_redacted
        ));
    }
}

fn show_graphql_sse(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("GraphQL Composer (PR01)");
    ui.horizontal(|ui| {
        ui.label("Endpoint:");
        ui.add(egui::TextEdit::singleline(&mut app.gql_endpoint).desired_width(360.0));
        if swaggen_ui_kit::primary_button(ui, !app.gql_endpoint.trim().is_empty(), "Run GraphQL")
            .clicked()
        {
            app.submit(Command::ExecuteGraphQl {
                endpoint: app.gql_endpoint.trim().to_owned(),
                query: app.gql_query.clone(),
                variables_json: app.gql_vars.clone(),
            });
        }
    });
    ui.label("Query:");
    ui.add(
        egui::TextEdit::multiline(&mut app.gql_query)
            .desired_width(ui.available_width())
            .desired_rows(4)
            .code_editor(),
    );
    ui.label("Variables JSON:");
    ui.add(
        egui::TextEdit::multiline(&mut app.gql_vars)
            .desired_width(ui.available_width())
            .desired_rows(2)
            .code_editor(),
    );
    if let Some(res) = &app.gql_result {
        ui.monospace(format!(
            "HTTP {} ({} ms) · errors={:?}",
            res.http_status, res.duration_ms, res.errors
        ));
        swaggen_ui_kit::code_surface(ui, &res.data_pretty);
    }

    ui.separator();
    ui.heading("Bounded SSE Client (PR04)");
    ui.horizontal_wrapped(|ui| {
        ui.label("Stream URL:");
        ui.add(egui::TextEdit::singleline(&mut app.sse_url).desired_width(280.0));
        ui.label("Max retained events:");
        ui.add(egui::DragValue::new(&mut app.sse_max_events).range(10..=5000));
        if app.sse_running {
            if swaggen_ui_kit::danger_button(ui, true, "Stop SSE").clicked() {
                app.submit(Command::StopSse);
            }
        } else if swaggen_ui_kit::secondary_button(ui, !app.sse_url.trim().is_empty(), "Listen SSE")
            .clicked()
        {
            app.sse_events.clear();
            app.sse_running = true;
            app.submit(Command::StartSse {
                url: app.sse_url.trim().to_owned(),
                max_events: app.sse_max_events,
            });
        }
    });
    for ev in &app.sse_events {
        ui.monospace(format!("[{} id={:?}] {}", ev.event, ev.id, ev.data));
    }
}

fn show_dev_utils(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.heading("Offline Dev Utilities (PR09)");
    ui.add(
        egui::TextEdit::multiline(&mut app.dev_util_input)
            .desired_width(ui.available_width())
            .desired_rows(5)
            .code_editor(),
    );
    ui.horizontal_wrapped(|ui| {
        if swaggen_ui_kit::secondary_button(ui, true, "URL Encode").clicked() {
            app.dev_util_output = dev_utils::url_encode(&app.dev_util_input);
        }
        if swaggen_ui_kit::secondary_button(ui, true, "URL Decode").clicked() {
            app.dev_util_output =
                dev_utils::url_decode(&app.dev_util_input).unwrap_or_else(|e| format!("ERR: {e}"));
        }
        if swaggen_ui_kit::secondary_button(ui, true, "Base64 Encode").clicked() {
            app.dev_util_output = dev_utils::base64_encode(&app.dev_util_input);
        }
        if swaggen_ui_kit::secondary_button(ui, true, "Base64 Decode").clicked() {
            app.dev_util_output = dev_utils::base64_decode(&app.dev_util_input)
                .unwrap_or_else(|e| format!("ERR: {e}"));
        }
        if swaggen_ui_kit::secondary_button(ui, true, "JSON Pretty").clicked() {
            app.dev_util_output = dev_utils::format_json(&app.dev_util_input, true)
                .unwrap_or_else(|e| format!("ERR: {e}"));
        }
        if swaggen_ui_kit::secondary_button(ui, true, "JSON Minify").clicked() {
            app.dev_util_output = dev_utils::format_json(&app.dev_util_input, false)
                .unwrap_or_else(|e| format!("ERR: {e}"));
        }
        if swaggen_ui_kit::secondary_button(ui, true, "YAML -> JSON").clicked() {
            app.dev_util_output =
                dev_utils::yaml_to_json(&app.dev_util_input).unwrap_or_else(|e| format!("ERR: {e}"));
        }
        if swaggen_ui_kit::secondary_button(ui, !app.dev_util_output.is_empty(), "Copy output")
            .clicked()
        {
            ctx.copy_text(app.dev_util_output.clone());
        }
    });
    if !app.dev_util_output.is_empty() {
        swaggen_ui_kit::code_surface(ui, &app.dev_util_output);
    }
}

fn show_sandbox_and_extensions(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("Isolated Pre/Test Script Sandbox & Postman Analyzer (TS07 / TS08)");
    ui.label("Runs purely in-memory with strict instruction step (20k), duration (250ms), and memory (512KiB) caps; zero filesystem/shell/network access.");
    ui.add(
        egui::TextEdit::multiline(&mut app.sandbox_script_text)
            .desired_width(ui.available_width())
            .desired_rows(5)
            .code_editor(),
    );
    if swaggen_ui_kit::primary_button(ui, true, "Run Sandboxed Script").clicked() {
        let (st, dur, body, hdrs) = app
            .active_request()
            .and_then(|r| app.responses.get(&r.id))
            .and_then(|res| res.as_ref().ok())
            .map(|r| {
                (
                    r.status_code,
                    r.duration_ms,
                    String::from_utf8_lossy(&r.preview_bytes).into_owned(),
                    r.headers.clone(),
                )
            })
            .unwrap_or((200, 10, r#"{"ok":true}"#.into(), Vec::new()));
        let ctx = swaggen_runner::sandbox::ScriptContextSnapshot {
            env_vars: Default::default(),
            response_status: st,
            response_time_ms: dur,
            response_body: body,
            response_headers: hdrs,
        };
        app.sandbox_report = Some(swaggen_runner::sandbox::execute_sandboxed_script(
            &app.sandbox_script_text,
            &ctx,
            Default::default(),
        ));
    }
    if let Some(rep) = &app.sandbox_report {
        ui.monospace(format!(
            "Steps={} · {}ms · error={:?} · tests={:?}",
            rep.steps_executed, rep.duration_ms, rep.error, rep.test_results
        ));
        for u in &rep.compatibility.unsupported_apis {
            swaggen_ui_kit::notice_banner(ui, NoticeKind::Warning, "Unsupported Postman API", u);
        }
    }

    ui.separator();
    ui.heading("Capability-Gated Extensions (PR10 · Off by Default)");
    for ext in &mut app.extensions {
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("{} v{}", ext.name, ext.version));
                ui.checkbox(&mut ext.enabled, "Enabled");
                for cap in ext.declared_capabilities.clone() {
                    let mut granted = ext.granted_capabilities.contains(&cap);
                    if ui.checkbox(&mut granted, format!("{cap:?}")).changed() {
                        if granted {
                            let _ = ext.grant_capability(cap);
                        } else {
                            ext.revoke_capability(cap);
                        }
                    }
                }
                if ui.small_button("Revoke all").clicked() {
                    ext.revoke_all();
                }
            });
        });
    }

    ui.separator();
    ui.heading("Auth Scheme Matrix (AU10) & HTTP Protocol Policy (PR06)");
    for row in swaggen_runner::advanced_protocols::auth_scheme_matrix() {
        ui.monospace(format!(
            "[{}] {} — {}",
            if row.supported { "SUPPORTED" } else { "UNSUPPORTED" },
            row.scheme,
            row.note
        ));
    }
    ui.monospace(format!(
        "HTTP/2 policy: {}",
        swaggen_runner::advanced_protocols::HttpProtocolPreference::Auto.describe_policy(true)
    ));
}
