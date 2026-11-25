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
    let title = match app.runner_subtab {
        0 => "Run a collection",
        1 => "Mock servers",
        2 => "GraphQL & SSE",
        3 => "Developer utilities",
        _ => "Script sandbox & extensions",
    };
    swaggen_ui_kit::page_header(
        ui,
        title,
        "Execute collection runs with CSV/JSON datasets, local schema-seeded mocks, GraphQL/SSE clients, and dev codecs.",
        None,
        false,
    );

    ui.horizontal_wrapped(|ui| {
        for (idx, lbl) in [
            (0usize, "Collection Runner"),
            (1, "Mock Server"),
            (2, "GraphQL & SSE"),
            (3, "Dev Utilities"),
            (4, "Sandbox & Extensions"),
        ] {
            if swaggen_ui_kit::underline_tab(ui, app.runner_subtab == idx, lbl).clicked() {
                app.runner_subtab = idx;
            }
        }
    });
    ui.add_space(10.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        swaggen_ui_kit::surface_card(ui, |ui| match app.runner_subtab {
            0 => show_collection_runner(app, ui, ctx),
            1 => show_mock_server(app, ui),
            2 => show_graphql_sse(app, ui),
            3 => show_dev_utils(app, ui, ctx),
            _ => show_sandbox_and_extensions(app, ui),
        });
    });
}

fn show_collection_runner(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Iterations:");
        ui.add(egui::DragValue::new(&mut app.runner_iterations).range(1..=100));
        ui.label("Delay (ms):");
        ui.add(egui::DragValue::new(&mut app.runner_delay_ms).range(0..=10_000));
        ui.checkbox(&mut app.runner_stop_on_fail, "Stop on first failure");
        ui.checkbox(
            &mut app.runner_validate_contract,
            "Validate OpenAPI contract",
        );
    });
    ui.label("Optional CSV or JSON dataset:");
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
            if swaggen_ui_kit::secondary_button(ui, true, "Copy JUnit XML").clicked() {
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
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            swaggen_ui_kit::pill_badge(
                ui,
                swaggen_ui_kit::Tokens::STATUS_SUCCESS,
                &format!("Passed: {}", rep.passed),
            );
            swaggen_ui_kit::pill_badge(
                ui,
                if rep.failed_assertions + rep.failed_contract + rep.infra_errors > 0 {
                    swaggen_ui_kit::Tokens::TEXT_DANGER
                } else {
                    swaggen_ui_kit::text_secondary(ui)
                },
                &format!(
                    "Failed: {}",
                    rep.failed_assertions + rep.failed_contract + rep.infra_errors
                ),
            );
            swaggen_ui_kit::pill_badge(
                ui,
                swaggen_ui_kit::text_secondary(ui),
                &format!("Skipped: {}", rep.skipped),
            );
            swaggen_ui_kit::pill_badge(
                ui,
                swaggen_ui_kit::Tokens::STATUS_INFO,
                &format!("{} ms", rep.total_duration_ms),
            );
        });
        ui.add_space(6.0);
        swaggen_ui_kit::table_header(
            ui,
            &[("STATUS", 110.0), ("ENDPOINT", 260.0), ("RESULT", 260.0)],
        );
        for s in &rep.steps {
            let (tag, col) = match s.outcome {
                StepOutcome::Passed => ("PASS", swaggen_ui_kit::Tokens::STATUS_SUCCESS),
                StepOutcome::FailedAssertions => {
                    ("FAIL_ASSERT", swaggen_ui_kit::Tokens::TEXT_DANGER)
                }
                StepOutcome::FailedContract => {
                    ("FAIL_CONTRACT", swaggen_ui_kit::Tokens::TEXT_DANGER)
                }
                StepOutcome::Skipped => ("SKIPPED", swaggen_ui_kit::text_secondary(ui)),
                StepOutcome::InfraError => ("INFRA_ERR", swaggen_ui_kit::Tokens::TEXT_DANGER),
            };
            swaggen_ui_kit::table_row_frame(ui, |ui| {
                swaggen_ui_kit::pill_badge(ui, col, tag);
                swaggen_ui_kit::method_badge(ui, &s.method);
                ui.label(swaggen_ui_kit::bold_text(
                    &format!("{} (iter {})", s.request_name, s.iteration + 1),
                    12.5,
                    swaggen_ui_kit::text_primary(ui),
                ));
                ui.label(
                    egui::RichText::new(format!(
                        "{:?} · {} ms · {}",
                        s.status_code, s.duration_ms, s.detail
                    ))
                    .size(12.0)
                    .color(swaggen_ui_kit::text_secondary(ui)),
                );
            });
        }
    }
}

fn show_mock_server(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Deterministic seed:");
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
    swaggen_ui_kit::card_header(ui, "GraphQL Explorer");
    ui.horizontal(|ui| {
        let ep_w = (ui.available_width() - 130.0).max(200.0);
        swaggen_ui_kit::text_input(
            ui,
            &mut app.gql_endpoint,
            "https://api.example.test/graphql",
            ep_w,
            false,
        );
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
    ui.add_space(10.0);
    ui.columns(2, |cols| {
        swaggen_ui_kit::surface_card(&mut cols[0], |ui| {
            swaggen_ui_kit::card_header(ui, "Query & Variables");
            ui.add(
                egui::TextEdit::multiline(&mut app.gql_query)
                    .desired_width(ui.available_width())
                    .desired_rows(7)
                    .code_editor(),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Variables JSON")
                    .size(11.5)
                    .color(swaggen_ui_kit::text_secondary(ui)),
            );
            ui.add(
                egui::TextEdit::multiline(&mut app.gql_vars)
                    .desired_width(ui.available_width())
                    .desired_rows(3)
                    .code_editor(),
            );
        });
        swaggen_ui_kit::surface_card(&mut cols[1], |ui| {
            swaggen_ui_kit::card_header(ui, "GraphQL Response");
            if let Some(res) = &app.gql_result {
                ui.horizontal_wrapped(|ui| {
                    swaggen_ui_kit::pill_badge(
                        ui,
                        if (200..300).contains(&res.http_status) {
                            swaggen_ui_kit::Tokens::STATUS_SUCCESS
                        } else {
                            swaggen_ui_kit::Tokens::TEXT_DANGER
                        },
                        &format!("HTTP {}", res.http_status),
                    );
                    swaggen_ui_kit::pill_badge(
                        ui,
                        swaggen_ui_kit::text_secondary(ui),
                        &format!("{} ms", res.duration_ms),
                    );
                });
                ui.add_space(6.0);
                swaggen_ui_kit::code_surface_numbered(ui, &res.data_pretty);
            } else {
                ui.label(
                    egui::RichText::new("Execute a GraphQL query to inspect the response payload.")
                        .size(12.0)
                        .color(swaggen_ui_kit::text_secondary(ui)),
                );
            }
        });
    });

    ui.separator();
    ui.heading("SSE Stream Client");
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
    ui.heading("Offline Developer Utilities");
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
            app.dev_util_output = dev_utils::yaml_to_json(&app.dev_util_input)
                .unwrap_or_else(|e| format!("ERR: {e}"));
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
    ui.heading("Pre/Test Script Sandbox & Postman Analyzer");
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
    ui.heading("Capability-Gated Extensions (Off by Default)");
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
    ui.heading("Auth Scheme Matrix & HTTP Protocol Policy");
    for row in swaggen_runner::advanced_protocols::auth_scheme_matrix() {
        ui.monospace(format!(
            "[{}] {} — {}",
            if row.supported {
                "SUPPORTED"
            } else {
                "UNSUPPORTED"
            },
            row.scheme,
            row.note
        ));
    }
    ui.monospace(format!(
        "HTTP/2 policy: {}",
        swaggen_runner::advanced_protocols::HttpProtocolPreference::Auto.describe_policy(true)
    ));
}
