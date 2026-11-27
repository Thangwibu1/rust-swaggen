//! Local Mock Server, Realtime Protocols (WS · SSE · gRPC · GraphQL SDL), & Dev Utilities:
//! - Scenario Mock Server (`MK01`–`MK03`)
//! - Realtime Protocols (`WebSocket PR03`, `SSE PR04`, `gRPC PR05`, `GraphQL SDL PR02`)
//! - Offline Codecs, Formatters & Capability-Gated Extensions
use crate::{app::App, worker::Command};
use eframe::egui;
use swaggen_runner::{
    advanced_protocols::{MockScenarioRule, validate_graphql_against_sdl},
    local_mock::build_mock_routes,
    protocols_and_tools::dev_utils,
};
use swaggen_ui_kit::{NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    let title = match app.runner_subtab {
        0 => swaggen_ui_kit::tr(app.locale, "Local Mock Server", "Máy Chủ Mock Cục Bộ"),
        1 => swaggen_ui_kit::tr(
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
        "Run local scenario mocks, inspect WebSocket/SSE/gRPC traffic, and use offline codecs.",
        None,
        false,
    );

    ui.horizontal_wrapped(|ui| {
        for (idx, lbl) in [
            (0usize, "Mock Server"),
            (1, "WebSocket · SSE · gRPC"),
            (2, "Dev Utilities & Extensions"),
        ] {
            if swaggen_ui_kit::underline_tab(ui, app.runner_subtab == idx, lbl).clicked() {
                app.runner_subtab = idx;
            }
        }
    });
    ui.add_space(8.0);

    match app.runner_subtab {
        0 => show_mock_server(app, ui),
        1 => show_realtime_protocols(app, ui),
        _ => show_dev_utils_and_extensions(app, ui, ctx),
    }
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
