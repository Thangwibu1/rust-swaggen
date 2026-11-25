//! Playwright-Style Test Runner Studio, Scenario Mock Server & Realtime Protocols (F7):
//! - 4 guided test workflows: `Unit Test`, `Functional (Data-Driven)`, `Integration`, `End-to-End (E2E)`
//! - Interactive Test Explorer (`[✓]` endpoint selection, bounded parallel workers `TS09`)
//! - Step Trace Inspector (`Request` | `Response` | `Tests & Console` | `Extracted & Timings`)
//! - Scenario Mock Server (`MK01`–`MK03`) + Realtime Protocols (`WebSocket PR03`, `SSE PR04`, `gRPC PR05`, `GraphQL SDL PR02`)
use crate::{app::App, worker::Command};
use eframe::egui;
use swaggen_runner::{
    advanced_protocols::{MockScenarioRule, validate_graphql_against_sdl},
    collection_runner::{RunnerConfig, StepOutcome, parse_dataset},
    local_mock::build_mock_routes,
    protocols_and_tools::dev_utils,
};
use swaggen_ui_kit::{NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    let title = match app.runner_subtab {
        0 => swaggen_ui_kit::tr(
            app.locale,
            "API Test Runner (Playwright Studio)",
            "Bộ Chạy Kiểm Thử API (Playwright Studio)",
        ),
        1 => swaggen_ui_kit::tr(app.locale, "Local Mock Server", "Máy Chủ Mock Cục Bộ"),
        2 => swaggen_ui_kit::tr(
            app.locale,
            "Realtime Protocols (WS · SSE · gRPC)",
            "Giao Thức Thời Gian Thực (WS · SSE · gRPC)",
        ),
        _ => swaggen_ui_kit::tr(
            app.locale,
            "Developer Utilities & Extensions",
            "Tiện Ích Dev & Extensions",
        ),
    };
    swaggen_ui_kit::page_header(
        ui,
        title,
        "Run Unit, Functional, Integration, and E2E API test suites with full step traces, local scenario mocks, and realtime protocols.",
        None,
        false,
    );

    ui.horizontal_wrapped(|ui| {
        for (idx, lbl) in [
            (0usize, "▶ Test Runner"),
            (1, "Mock Server"),
            (2, "WebSocket · SSE · gRPC"),
            (3, "Dev Utilities & Extensions"),
        ] {
            if swaggen_ui_kit::underline_tab(ui, app.runner_subtab == idx, lbl).clicked() {
                app.runner_subtab = idx;
            }
        }
    });
    ui.add_space(8.0);

    match app.runner_subtab {
        0 => show_playwright_test_runner(app, ui, ctx),
        1 => show_mock_server(app, ui),
        2 => show_realtime_protocols(app, ui),
        _ => show_dev_utils_and_extensions(app, ui, ctx),
    }
}

fn show_playwright_test_runner(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    // 1. 4 Guided Testing Workflow Presets (Unit / Functional / Integration / E2E)
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (k_idx, title, desc) in [
            (
                0usize,
                "1. Unit Test",
                "Fast isolated endpoint status/schema checks (supports parallel workers)",
            ),
            (
                1,
                "2. Functional (CSV/JSON)",
                "Data-driven parameter & payload matrix across CSV/JSON dataset rows",
            ),
            (
                2,
                "3. Integration",
                "Chained multi-request workflow passing variables + OpenAPI contract check",
            ),
            (
                3,
                "4. End-to-End (E2E)",
                "Full sequential user journey with fail-fast stop on first error",
            ),
        ] {
            let active = app.runner_test_kind == k_idx;
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(title)
                            .size(11.5)
                            .color(if active {
                                Tokens::STATUS_INFO
                            } else {
                                swaggen_ui_kit::text_primary(ui)
                            })
                            .strong(),
                    )
                    .fill(if active {
                        swaggen_ui_kit::selected_bg(ui)
                    } else {
                        swaggen_ui_kit::surface_bg(ui)
                    })
                    .stroke(egui::Stroke::new(
                        if active { 1.5 } else { 1.0 },
                        if active {
                            Tokens::STATUS_INFO
                        } else {
                            swaggen_ui_kit::border_color(ui)
                        },
                    ))
                    .corner_radius(6),
                )
                .on_hover_text(desc)
                .clicked()
            {
                app.runner_test_kind = k_idx;
                match k_idx {
                    0 => {
                        app.runner_stop_on_fail = false;
                        app.runner_validate_contract = false;
                    }
                    1 => {
                        app.runner_stop_on_fail = false;
                        if app.runner_dataset_text.trim().is_empty() {
                            app.runner_dataset_text = "id,expected_status\n1,200\n2,200\n".into();
                        }
                    }
                    2 => {
                        app.runner_concurrency = 1;
                        app.runner_validate_contract = true;
                        app.runner_stop_on_fail = false;
                    }
                    _ => {
                        app.runner_concurrency = 1;
                        app.runner_stop_on_fail = true;
                        app.runner_validate_contract = true;
                    }
                }
            }
        }
    });
    ui.add_space(8.0);

    let total_w = ui.available_width().max(280.0);
    let avail_h = ui.available_height().max(260.0);
    if total_w >= 680.0 {
        let left_w = (total_w * 0.34).clamp(240.0, 360.0);
        let right_w = (total_w - left_w - 12.0).max(300.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(left_w, avail_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    show_test_explorer_pane(app, ui, avail_h);
                },
            );
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(
                egui::vec2(right_w, avail_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    show_trace_inspector_pane(app, ui, ctx, avail_h);
                },
            );
        });
    } else {
        egui::ScrollArea::vertical()
            .id_salt("runner_narrow_stack")
            .show(ui, |ui| {
                show_test_explorer_pane(app, ui, 260.0);
                ui.add_space(8.0);
                show_trace_inspector_pane(app, ui, ctx, 380.0);
            });
    }
}

fn show_test_explorer_pane(app: &mut App, ui: &mut egui::Ui, max_h: f32) {
    swaggen_ui_kit::surface_card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(swaggen_ui_kit::bold_text(
                "Test Suite Explorer",
                13.5,
                swaggen_ui_kit::text_primary(ui),
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("None").clicked() {
                    app.runner_selected_requests.clear();
                }
                if ui.small_button("All").clicked() {
                    app.runner_selected_requests =
                        app.state.requests.iter().map(|r| r.id.clone()).collect();
                }
            });
        });
        ui.add_space(4.0);

        // Run controls right at top of Explorer (like Playwright UI)
        ui.horizontal_wrapped(|ui| {
            if app.runner_running {
                if swaggen_ui_kit::danger_button(ui, true, "■ Stop Run").clicked() {
                    app.submit(Command::CancelRunner);
                }
            } else {
                let sel_count = if app.runner_selected_requests.is_empty() {
                    app.state.requests.len()
                } else {
                    app.runner_selected_requests.len()
                };
                if ui
                    .add(
                        egui::Button::new(swaggen_ui_kit::bold_text(
                            &format!("▶ Run {sel_count} Test(s)"),
                            12.0,
                            egui::Color32::WHITE,
                        ))
                        .fill(Tokens::STATUS_SUCCESS)
                        .stroke(egui::Stroke::new(1.0, Tokens::STATUS_SUCCESS))
                        .min_size(egui::vec2(120.0, 28.0))
                        .corner_radius(5),
                    )
                    .clicked()
                {
                    let dataset_rows = if app.runner_dataset_text.trim().is_empty() {
                        Vec::new()
                    } else {
                        parse_dataset(&app.runner_dataset_text)
                            .map(|p| p.rows)
                            .unwrap_or_default()
                    };
                    let req_ids: Vec<String> = app
                        .state
                        .requests
                        .iter()
                        .filter(|r| {
                            app.runner_selected_requests.is_empty()
                                || app.runner_selected_requests.contains(&r.id)
                        })
                        .map(|r| r.id.clone())
                        .collect();
                    app.runner_running = true;
                    app.runner_selected_step_idx = 0;
                    app.submit(Command::RunCollection {
                        state: Box::new(app.state.clone()),
                        specs: app.specs.clone(),
                        config: RunnerConfig {
                            request_ids: req_ids,
                            iterations: app.runner_iterations.max(1),
                            delay_ms: app.runner_delay_ms,
                            stop_on_failure: app.runner_stop_on_fail,
                            validate_contract: app.runner_validate_contract,
                            dataset_rows,
                            redaction_policy: Default::default(),
                        },
                        concurrency: app.runner_concurrency.max(1),
                    });
                }
            }
        });

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Iterations:");
            ui.add(egui::DragValue::new(&mut app.runner_iterations).range(1..=100));
            ui.label("Delay ms:");
            ui.add(
                egui::DragValue::new(&mut app.runner_delay_ms)
                    .range(0..=10_000)
                    .speed(25),
            );
            if app.runner_test_kind == 0 {
                ui.label("Workers:");
                ui.add(egui::DragValue::new(&mut app.runner_concurrency).range(1..=16));
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut app.runner_stop_on_fail, "Fail-fast");
            ui.checkbox(&mut app.runner_validate_contract, "OpenAPI contract check");
        });

        if app.runner_test_kind == 1 || !app.runner_dataset_text.trim().is_empty() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Data-Driven CSV / JSON:")
                        .size(11.0)
                        .color(swaggen_ui_kit::text_secondary(ui))
                        .strong(),
                );
                if ui.small_button("Clear").clicked() {
                    app.runner_dataset_text.clear();
                }
            });
            ui.add(
                egui::TextEdit::multiline(&mut app.runner_dataset_text)
                    .hint_text("user_id,role\n1,admin\n2,member")
                    .desired_width(ui.available_width())
                    .desired_rows(3)
                    .code_editor(),
            );
            if !app.runner_dataset_text.trim().is_empty() {
                match parse_dataset(&app.runner_dataset_text) {
                    Ok(prev) => {
                        ui.label(
                            egui::RichText::new(format!(
                                "✓ {} dataset: {} row(s) · cols [{}]",
                                prev.format.to_uppercase(),
                                prev.rows.len(),
                                prev.columns.join(", ")
                            ))
                            .size(10.5)
                            .color(Tokens::STATUS_SUCCESS),
                        );
                    }
                    Err(e) => {
                        ui.label(
                            egui::RichText::new(format!("Dataset error: {e}"))
                                .size(10.5)
                                .color(Tokens::TEXT_DANGER),
                        );
                    }
                }
            }
        }

        ui.add_space(6.0);
        ui.separator();
        let list_h = (max_h - 195.0).max(80.0);
        egui::ScrollArea::vertical()
            .id_salt("runner_explorer_tree")
            .max_height(list_h)
            .show(ui, |ui| {
                let all_implicitly_selected = app.runner_selected_requests.is_empty();
                for col in &app.state.collections {
                    ui.label(swaggen_ui_kit::bold_text(
                        &format!("📁 {}", col.name),
                        11.5,
                        swaggen_ui_kit::text_primary(ui),
                    ));
                    for req in app
                        .state
                        .requests
                        .iter()
                        .filter(|r| r.collection_id == col.id)
                    {
                        let mut checked = if all_implicitly_selected {
                            true
                        } else {
                            app.runner_selected_requests.contains(&req.id)
                        };
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut checked, "").changed() {
                                if all_implicitly_selected {
                                    app.runner_selected_requests =
                                        app.state.requests.iter().map(|r| r.id.clone()).collect();
                                }
                                if checked {
                                    app.runner_selected_requests.insert(req.id.clone());
                                } else {
                                    app.runner_selected_requests.remove(&req.id);
                                }
                            }
                            let badge = if req.is_graphql() {
                                "GQL"
                            } else {
                                req.method.as_str()
                            };
                            ui.monospace(
                                egui::RichText::new(badge)
                                    .size(10.0)
                                    .color(swaggen_ui_kit::method_color(badge)),
                            );
                            ui.label(egui::RichText::new(&req.name).size(11.5));
                            let test_cnt = req.assertions.len()
                                + usize::from(!req.post_response_script.trim().is_empty());
                            if test_cnt > 0 {
                                ui.label(
                                    egui::RichText::new(format!("({test_cnt})"))
                                        .size(10.0)
                                        .color(Tokens::STATUS_INFO),
                                );
                            }
                        });
                    }
                    ui.add_space(4.0);
                }
            });
    });
}

fn show_trace_inspector_pane(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context, max_h: f32) {
    swaggen_ui_kit::surface_card(ui, |ui| {
        let Some(rep) = app.last_run_report.clone() else {
            ui.vertical_centered(|ui| {
                ui.add_space(28.0);
                ui.label(swaggen_ui_kit::bold_text(
                    "Ready to execute test suite",
                    15.0,
                    swaggen_ui_kit::text_primary(ui),
                ));
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "Pick a test workflow above (Unit, Functional, Integration, or E2E), select your endpoints on the left, and click ▶ Run Tests.\nEvery step captures full Request/Response payloads, JS assertions, console logs, and timing traces.",
                    )
                    .color(swaggen_ui_kit::text_secondary(ui)),
                );
                ui.add_space(28.0);
            });
            return;
        };

        // Top Summary Bar
        ui.horizontal_wrapped(|ui| {
            let total_fail = rep.failed_assertions + rep.failed_contract + rep.infra_errors;
            swaggen_ui_kit::status_badge(
                ui,
                if total_fail == 0 {
                    NoticeKind::Success
                } else {
                    NoticeKind::Danger
                },
                &format!("Passed {} / {}", rep.passed, rep.steps.len()),
            );
            if total_fail > 0 {
                swaggen_ui_kit::pill_badge(
                    ui,
                    Tokens::TEXT_DANGER,
                    &format!("Failed: {total_fail}"),
                );
            }
            if rep.skipped > 0 {
                swaggen_ui_kit::pill_badge(
                    ui,
                    swaggen_ui_kit::text_secondary(ui),
                    &format!("Skipped: {}", rep.skipped),
                );
            }
            swaggen_ui_kit::pill_badge(
                ui,
                Tokens::STATUS_INFO,
                &format!("Total {} ms", rep.total_duration_ms),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if swaggen_ui_kit::secondary_button(ui, true, "Export JSON & JUnit").clicked() {
                    let dir = app.root.join("runner-reports");
                    let _ = std::fs::create_dir_all(&dir);
                    if let Ok(js) = serde_json::to_string_pretty(&rep) {
                        let _ = std::fs::write(dir.join("run-report.json"), js);
                    }
                    let _ = std::fs::write(dir.join("junit.xml"), rep.to_junit_xml());
                    app.status = format!("Saved JUnit XML & JSON reports to {}", dir.display());
                }
                if swaggen_ui_kit::secondary_button(ui, true, "Copy JUnit XML").clicked() {
                    ctx.copy_text(rep.to_junit_xml());
                    app.status = "Copied JUnit XML report".into();
                }
            });
        });
        ui.add_space(6.0);

        // Timeline Step List (upper half)
        let list_h = ((max_h - 90.0) * 0.42).clamp(90.0, 200.0);
        egui::ScrollArea::vertical()
            .id_salt("runner_timeline_steps")
            .max_height(list_h)
            .show(ui, |ui| {
                for (s_idx, s) in rep.steps.iter().enumerate() {
                    let selected = app.runner_selected_step_idx == s_idx;
                    let (icon, col) = match s.outcome {
                        StepOutcome::Passed => ("✓ PASS", Tokens::STATUS_SUCCESS),
                        StepOutcome::FailedAssertions => ("✖ ASSERT", Tokens::TEXT_DANGER),
                        StepOutcome::FailedContract => ("✖ CONTRACT", Tokens::TEXT_DANGER),
                        StepOutcome::Skipped => ("○ SKIP", swaggen_ui_kit::text_secondary(ui)),
                        StepOutcome::InfraError => ("✖ ERROR", Tokens::TEXT_DANGER),
                    };
                    let resp = egui::Frame::new()
                        .fill(if selected {
                            swaggen_ui_kit::selected_bg(ui)
                        } else {
                            swaggen_ui_kit::surface_bg(ui)
                        })
                        .stroke(egui::Stroke::new(
                            1.0,
                            if selected {
                                Tokens::STATUS_INFO
                            } else {
                                swaggen_ui_kit::border_color(ui)
                            },
                        ))
                        .corner_radius(5)
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    egui::RichText::new(icon)
                                        .monospace()
                                        .size(10.5)
                                        .color(col)
                                        .strong(),
                                );
                                ui.monospace(
                                    egui::RichText::new(&s.method)
                                        .size(10.5)
                                        .color(swaggen_ui_kit::method_color(&s.method)),
                                );
                                ui.label(swaggen_ui_kit::bold_text(
                                    &s.request_name,
                                    12.0,
                                    swaggen_ui_kit::text_primary(ui),
                                ));
                                if let Some(code) = s.status_code {
                                    ui.monospace(format!("HTTP {code}"));
                                }
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} ms · {}",
                                        s.duration_ms, s.detail
                                    ))
                                    .size(11.0)
                                    .color(swaggen_ui_kit::text_secondary(ui)),
                                );
                            });
                        });
                    if resp.response.interact(egui::Sense::click()).clicked() {
                        app.runner_selected_step_idx = s_idx;
                    }
                    ui.add_space(2.0);
                }
            });

        ui.add_space(6.0);
        ui.separator();

        // Selected Step Trace Inspector (bottom half — Playwright Trace Viewer style!)
        if app.runner_selected_step_idx >= rep.steps.len() {
            app.runner_selected_step_idx = 0;
        }
        if let Some(step) = rep.steps.get(app.runner_selected_step_idx) {
            ui.horizontal_wrapped(|ui| {
                ui.label(swaggen_ui_kit::bold_text(
                    &format!("Step Trace: {} {}", step.method, step.request_name),
                    12.5,
                    Tokens::STATUS_INFO,
                ));
                for (t_idx, lbl) in [
                    (0usize, "Response"),
                    (1, "Request"),
                    (2, &format!("Tests & Console ({})", step.script_tests.len())),
                    (3, "Variables & Timings"),
                ] {
                    if swaggen_ui_kit::underline_tab(ui, app.runner_trace_tab == t_idx, lbl)
                        .clicked()
                    {
                        app.runner_trace_tab = t_idx;
                    }
                }
            });
            ui.add_space(4.0);
            let detail_h = (ui.available_height() - 10.0).max(90.0);
            egui::ScrollArea::vertical()
                .id_salt("runner_step_trace_detail")
                .max_height(detail_h)
                .show(ui, |ui| match app.runner_trace_tab {
                    0 => {
                        ui.monospace(format!(
                            "Status: {:?} · Duration: {} ms",
                            step.status_code, step.duration_ms
                        ));
                        if !step.response_body_preview.is_empty() {
                            swaggen_ui_kit::code_surface_numbered(ui, &step.response_body_preview);
                        } else {
                            ui.label("(empty response body)");
                        }
                    }
                    1 => {
                        ui.monospace(format!("{} {}", step.method, step.redacted_url));
                        for (hk, hv) in &step.request_headers_preview {
                            ui.monospace(format!("{hk}: {hv}"));
                        }
                        if !step.request_body_preview.is_empty() {
                            ui.add_space(4.0);
                            swaggen_ui_kit::code_surface_numbered(ui, &step.request_body_preview);
                        }
                    }
                    2 => {
                        ui.label(egui::RichText::new(&step.detail).strong());
                        for st in &step.script_tests {
                            ui.horizontal_wrapped(|ui| {
                                swaggen_ui_kit::status_badge(
                                    ui,
                                    if st.passed {
                                        NoticeKind::Success
                                    } else {
                                        NoticeKind::Danger
                                    },
                                    if st.passed { "PASS" } else { "FAIL" },
                                );
                                ui.label(&st.name);
                                if let Some(ref e) = st.error {
                                    ui.label(egui::RichText::new(e).color(Tokens::TEXT_DANGER));
                                }
                            });
                        }
                        for cm in &step.contract_mismatches {
                            swaggen_ui_kit::notice_banner(
                                ui,
                                NoticeKind::Danger,
                                "Contract mismatch",
                                cm,
                            );
                        }
                        if !step.console_logs.is_empty() {
                            ui.add_space(4.0);
                            swaggen_ui_kit::code_surface(ui, &step.console_logs.join("\n"));
                        }
                    }
                    _ => {
                        if !step.timings_summary.is_empty() {
                            ui.monospace(&step.timings_summary);
                        }
                        if step.extracted_keys.is_empty() {
                            ui.label("No variables mutated/extracted during this step.");
                        } else {
                            ui.label(format!(
                                "Updated variables: {}",
                                step.extracted_keys.join(", ")
                            ));
                        }
                    }
                });
        }
    });
}

fn show_mock_server(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .id_salt("mock_srv_scroll")
        .show(ui, |ui| {
            swaggen_ui_kit::surface_card(ui, |ui| {
                swaggen_ui_kit::card_header(
                    ui,
                    "Local Mock Server (Examples + OpenAPI Schema Seed + Scenario Rules)",
                );
                ui.horizontal_wrapped(|ui| {
                    ui.label("Port (0 = auto):");
                    swaggen_ui_kit::text_input(ui, &mut app.mock_port_input, "0", 72.0, false);
                    ui.label("Schema seed:");
                    ui.add(egui::DragValue::new(&mut app.mock_seed));
                    if app.local_mock_url.is_none() {
                        if swaggen_ui_kit::primary_button(ui, true, "▶ Start Local Mock Server")
                            .clicked()
                        {
                            let routes = build_mock_routes(&app.state, &app.specs, app.mock_seed);
                            let port = app.mock_port_input.trim().parse::<u16>().unwrap_or(0);
                            app.submit(Command::StartLocalMock {
                                routes,
                                scenarios: app.mock_scenarios.clone(),
                                port,
                            });
                        }
                    } else {
                        if swaggen_ui_kit::danger_button(ui, true, "■ Stop Mock Server").clicked()
                        {
                            app.submit(Command::StopLocalMock);
                        }
                        if swaggen_ui_kit::secondary_button(ui, true, "Refresh Traffic").clicked() {
                            app.submit(Command::PollMockTraffic);
                        }
                    }
                });
                if let Some(url) = app.local_mock_url.clone() {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        swaggen_ui_kit::status_badge(
                            ui,
                            NoticeKind::Success,
                            &format!("Listening at {url}"),
                        );
                        if swaggen_ui_kit::secondary_button(
                            ui,
                            true,
                            "Set as {{base_url}} in Active Environment",
                        )
                        .clicked()
                            && let Some(eid) = app.state.active_environment_id.clone()
                            && let Some(env) =
                                app.state.environments.iter_mut().find(|e| e.id == eid)
                        {
                            if let Some(v) = env.variables.iter_mut().find(|x| x.key == "base_url")
                            {
                                v.shared_value = url.clone();
                            } else {
                                env.variables.push(
                                    swaggen_domain::workspace::VariableEntry::public(
                                        "base_url",
                                        url.clone(),
                                    ),
                                );
                            }
                            app.mark_dirty();
                            app.status = format!("Updated {{{{base_url}}}} -> {url}");
                        }
                    });
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(swaggen_ui_kit::bold_text(
                        "Scenario Rules (Conditional Status & Latency — MK03)",
                        12.0,
                        swaggen_ui_kit::text_primary(ui),
                    ));
                    if ui.small_button("+ Add Scenario Rule").clicked() {
                        app.mock_scenarios.push(MockScenarioRule {
                            id: format!("rule-{}", app.mock_scenarios.len() + 1),
                            method: "GET".into(),
                            path: "/health".into(),
                            match_header: None,
                            match_query: None,
                            match_body_contains: None,
                            status: 200,
                            delay_ms: 0,
                            response_body: "{\"status\":\"mocked-ok\"}".into(),
                        });
                    }
                });
                let mut rm_sc = None;
                for (si, sc) in app.mock_scenarios.iter_mut().enumerate() {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut sc.method).desired_width(55.0));
                        ui.add(egui::TextEdit::singleline(&mut sc.path).desired_width(130.0));
                        ui.label("HTTP");
                        ui.add(egui::DragValue::new(&mut sc.status).range(100..=599));
                        ui.label("Delay ms:");
                        ui.add(egui::DragValue::new(&mut sc.delay_ms).range(0..=5000));
                        ui.add(
                            egui::TextEdit::singleline(&mut sc.response_body).desired_width(210.0),
                        );
                        if ui.small_button("×").clicked() {
                            rm_sc = Some(si);
                        }
                    });
                }
                if let Some(i) = rm_sc {
                    app.mock_scenarios.remove(i);
                }

                if !app.local_mock_logs.is_empty() {
                    ui.add_space(8.0);
                    swaggen_ui_kit::card_header(ui, "Incoming Mock Traffic (Sanitized)");
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
            });
        });
}

fn show_realtime_protocols(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for (p_idx, lbl) in [
            (0usize, "WebSocket (RFC 6455)"),
            (1, "Server-Sent Events (SSE)"),
            (2, "gRPC Unary (Protobuf)"),
            (3, "GraphQL SDL Schema Check"),
        ] {
            if ui
                .selectable_label(app.protocol_subtab == p_idx, lbl)
                .clicked()
            {
                app.protocol_subtab = p_idx;
            }
        }
    });
    ui.add_space(6.0);
    swaggen_ui_kit::surface_card(ui, |ui| match app.protocol_subtab {
        0 => {
            swaggen_ui_kit::card_header(ui, "WebSocket Client (PR03 — Direct from PC)");
            ui.horizontal_wrapped(|ui| {
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.ws_url_input,
                    "ws://127.0.0.1:8080/ws",
                    280.0,
                    false,
                );
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.ws_message_input,
                    "{\"ping\":1}",
                    220.0,
                    false,
                );
                if swaggen_ui_kit::primary_button(
                    ui,
                    !app.ws_url_input.trim().is_empty(),
                    "Connect & Send Frame",
                )
                .clicked()
                {
                    app.ws_logs.push(format!("→ {}", app.ws_message_input));
                    app.submit(Command::SendWebSocketMessage {
                        url: app.ws_url_input.trim().to_owned(),
                        message: app.ws_message_input.clone(),
                    });
                }
                if ui.small_button("Clear log").clicked() {
                    app.ws_logs.clear();
                }
            });
            if !app.ws_logs.is_empty() {
                ui.add_space(6.0);
                swaggen_ui_kit::code_surface(ui, &app.ws_logs.join("\n"));
            }
        }
        1 => {
            swaggen_ui_kit::card_header(ui, "Server-Sent Events Stream Listener (PR04)");
            ui.horizontal_wrapped(|ui| {
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.sse_url,
                    "http://127.0.0.1:8080/events",
                    280.0,
                    false,
                );
                ui.label("Max events:");
                ui.add(egui::DragValue::new(&mut app.sse_max_events).range(10..=5000));
                if app.sse_running {
                    if swaggen_ui_kit::danger_button(ui, true, "■ Stop SSE").clicked() {
                        app.submit(Command::StopSse);
                    }
                } else if swaggen_ui_kit::primary_button(
                    ui,
                    !app.sse_url.trim().is_empty(),
                    "▶ Listen SSE",
                )
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
        2 => {
            swaggen_ui_kit::card_header(
                ui,
                "gRPC Unary Caller (PR05 — 5-Byte Length-Prefixed Protobuf Wire Framing)",
            );
            ui.horizontal_wrapped(|ui| {
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.grpc_endpoint_input,
                    "http://127.0.0.1:50051",
                    210.0,
                    false,
                );
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.grpc_method_input,
                    "package.Service/Method",
                    210.0,
                    false,
                );
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.grpc_payload_input,
                    "Field #1 string payload",
                    180.0,
                    false,
                );
                if swaggen_ui_kit::primary_button(ui, true, "Invoke gRPC").clicked() {
                    app.submit(Command::CallGrpcUnary {
                        endpoint: app.grpc_endpoint_input.trim().to_owned(),
                        service_method: app.grpc_method_input.trim().to_owned(),
                        field_text: app.grpc_payload_input.clone(),
                    });
                }
            });
            if !app.grpc_output.is_empty() {
                ui.add_space(6.0);
                swaggen_ui_kit::code_surface(ui, &app.grpc_output);
            }
        }
        _ => {
            swaggen_ui_kit::card_header(ui, "GraphQL SDL Offline Validator & Playground (PR02)");
            ui.horizontal(|ui| {
                let ep_w = (ui.available_width() - 130.0).max(160.0);
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.gql_endpoint,
                    "https://api.example.test/graphql",
                    ep_w,
                    false,
                );
                if swaggen_ui_kit::primary_button(
                    ui,
                    !app.gql_endpoint.trim().is_empty(),
                    "Execute",
                )
                .clicked()
                {
                    app.submit(Command::ExecuteGraphQl {
                        endpoint: app.gql_endpoint.trim().to_owned(),
                        query: app.gql_query.clone(),
                        variables_json: app.gql_vars.clone(),
                    });
                }
            });
            ui.add_space(6.0);
            ui.add(
                egui::TextEdit::multiline(&mut app.gql_query)
                    .desired_width(ui.available_width())
                    .desired_rows(5)
                    .code_editor(),
            );
            let sample_sdl = "type Query {\n  health: String\n  workspaces: [Workspace]\n}";
            let sdl_issues = validate_graphql_against_sdl(sample_sdl, &app.gql_query);
            if sdl_issues.is_empty() {
                ui.label(
                    egui::RichText::new("✓ Valid against Query SDL")
                        .size(11.0)
                        .color(Tokens::STATUS_SUCCESS),
                );
            }
            if let Some(res) = &app.gql_result {
                swaggen_ui_kit::code_surface_numbered(ui, &res.data_pretty);
            }
        }
    });
}

fn show_dev_utils_and_extensions(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    egui::ScrollArea::vertical()
        .id_salt("dev_utils_scroll")
        .show(ui, |ui| {
            swaggen_ui_kit::surface_card(ui, |ui| {
                swaggen_ui_kit::card_header(ui, "Offline Codecs & Formatters (PR09)");
                ui.add(
                    egui::TextEdit::multiline(&mut app.dev_util_input)
                        .desired_width(ui.available_width())
                        .desired_rows(4)
                        .code_editor(),
                );
                ui.horizontal_wrapped(|ui| {
                    if swaggen_ui_kit::secondary_button(ui, true, "URL Encode").clicked() {
                        app.dev_util_output = dev_utils::url_encode(&app.dev_util_input);
                    }
                    if swaggen_ui_kit::secondary_button(ui, true, "URL Decode").clicked() {
                        app.dev_util_output = dev_utils::url_decode(&app.dev_util_input)
                            .unwrap_or_else(|e| format!("ERR: {e}"));
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
                    if swaggen_ui_kit::secondary_button(
                        ui,
                        !app.dev_util_output.is_empty(),
                        "Copy output",
                    )
                    .clicked()
                    {
                        ctx.copy_text(app.dev_util_output.clone());
                    }
                });
                if !app.dev_util_output.is_empty() {
                    swaggen_ui_kit::code_surface(ui, &app.dev_util_output);
                }
            });
            ui.add_space(8.0);
            swaggen_ui_kit::surface_card(ui, |ui| {
                swaggen_ui_kit::card_header(
                    ui,
                    "Capability-Gated Extensions (EX09 — Off by Default)",
                );
                for ext in &mut app.extensions {
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
                }
            });
        });
}
