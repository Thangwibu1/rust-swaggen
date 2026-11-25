//! Compact, High-Density Response Inspector:
//! - Single merged header + single merged Tab / Format / Search bar
//! - Explicit `Local PC` badge confirming requests run directly from the user's machine
//! - `Expand` / `Restore` toggle + full-height 2D scrollable JSON/code viewer
use crate::app::{App, ResponseBodyFormat, ResponseSubTab};
use crate::worker::Command;
use eframe::egui;
use std::path::PathBuf;
use swaggen_domain::{security::RedactionPolicy, workspace::build_sanitized_example};
use swaggen_spec_engine::assertions::evaluate_assertions;
use swaggen_ui_kit::{NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    let Some(req_id) = app.active_request_id() else {
        return;
    };
    let is_inflight = app.inflight_request_id.as_deref() == Some(req_id.as_str());
    let card_h = ui.available_height().max(96.0);

    egui::Frame::new()
        .fill(Tokens::BG_SURFACE)
        .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height((card_h - 16.0).max(80.0));

            if is_inflight {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(swaggen_ui_kit::bold_text(
                        "Sending directly from this PC…",
                        12.0,
                        Tokens::STATUS_INFO,
                    ));
                    if let Some((_, phase, done, total)) = app.transfer_progress {
                        let txt = match total {
                            Some(t) => format!("({phase}: {done}/{t} B)"),
                            None => format!("({phase}: {done} B)"),
                        };
                        ui.label(
                            egui::RichText::new(txt)
                                .size(11.0)
                                .color(Tokens::TEXT_SECONDARY),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if swaggen_ui_kit::danger_button(ui, true, "Cancel").clicked() {
                            app.cancel_inflight();
                        }
                        let max_lbl = if app.response_maximized {
                            "Restore"
                        } else {
                            "Expand"
                        };
                        if swaggen_ui_kit::secondary_button(ui, true, max_lbl).clicked() {
                            app.response_maximized = !app.response_maximized;
                        }
                    });
                });
                return;
            }

            let Some(result) = app.responses.get(&req_id).cloned() else {
                ui.horizontal(|ui| {
                    ui.label(swaggen_ui_kit::bold_text(
                        "Response",
                        12.5,
                        Tokens::TEXT_PRIMARY,
                    ));
                    swaggen_ui_kit::pill_badge(ui, Tokens::TEXT_SECONDARY, "Local PC engine");
                    if app.response_maximized {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if swaggen_ui_kit::secondary_button(ui, true, "Restore").clicked() {
                                app.response_maximized = false;
                            }
                        });
                    }
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Click 'Send' (or Ctrl+Enter) to execute directly from your computer and inspect the response here.",
                    )
                    .size(11.5)
                    .color(Tokens::TEXT_SECONDARY),
                );
                return;
            };

            match result {
                Err(err) => {
                    ui.horizontal(|ui| {
                        ui.label(swaggen_ui_kit::bold_text(
                            "Response",
                            12.5,
                            Tokens::TEXT_PRIMARY,
                        ));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let max_lbl = if app.response_maximized {
                                "Restore"
                            } else {
                                "Expand"
                            };
                            if swaggen_ui_kit::secondary_button(ui, true, max_lbl).clicked() {
                                app.response_maximized = !app.response_maximized;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    swaggen_ui_kit::notice_banner(
                        ui,
                        NoticeKind::Danger,
                        &format!("{} ({} ms)", err.kind.as_str(), err.duration_ms),
                        &format!("{} — {}", err.message, err.kind.guidance()),
                    );
                }
                Ok(resp) => {
                    let status_kind = match resp.status_code {
                        200..=299 => NoticeKind::Success,
                        300..=399 => NoticeKind::Warning,
                        _ => NoticeKind::Danger,
                    };
                    let raw_text = String::from_utf8_lossy(&resp.preview_bytes).into_owned();
                    let formatted_text =
                        if matches!(app.response_format, ResponseBodyFormat::PrettyJson)
                            && let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw_text)
                        {
                            serde_json::to_string_pretty(&val).unwrap_or_else(|_| raw_text.clone())
                        } else {
                            raw_text.clone()
                        };

                    // Compact Row 1: Status + Metrics + Local PC indicator + Copy/Save/Expand
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        ui.label(swaggen_ui_kit::bold_text(
                            "Response",
                            12.5,
                            Tokens::TEXT_PRIMARY,
                        ));
                        swaggen_ui_kit::status_badge(
                            ui,
                            status_kind,
                            &format!("{} {}", resp.status_code, resp.reason),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{} ms · {:.2} KB · TTFB {}ms · Local PC",
                                resp.duration_ms,
                                resp.decoded_bytes as f64 / 1024.0,
                                resp.timings.ttfb_ms.unwrap_or(0),
                            ))
                            .size(11.0)
                            .color(Tokens::TEXT_SECONDARY),
                        );
                        if resp.tls_exception_used {
                            swaggen_ui_kit::status_badge(ui, NoticeKind::Warning, "TLS exception");
                        }
                        if resp.headers.iter().any(|(k, v)| {
                            k.eq_ignore_ascii_case("x-idempotent-replayed")
                                && v.trim().eq_ignore_ascii_case("true")
                        }) {
                            swaggen_ui_kit::status_badge(
                                ui,
                                NoticeKind::Success,
                                "Idempotent Replay",
                            );
                        }
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw_text) {
                            if let Some(errs) = val.get("errors").and_then(|e| e.as_array())
                                && !errs.is_empty()
                            {
                                swaggen_ui_kit::status_badge(
                                    ui,
                                    NoticeKind::Danger,
                                    &format!("GQL Errors ({})", errs.len()),
                                );
                            }
                            if let Some(db_q) = val
                                .get("extensions")
                                .and_then(|e| e.get("performance"))
                                .and_then(|p| p.get("dbQueries"))
                                .and_then(|q| q.as_u64())
                            {
                                swaggen_ui_kit::pill_badge(
                                    ui,
                                    Tokens::STATUS_SUCCESS,
                                    &format!("DB Queries: {db_q} (Batched)"),
                                );
                            }
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let max_lbl = if app.response_maximized {
                                "Restore"
                            } else {
                                "Expand"
                            };
                            if swaggen_ui_kit::secondary_button(ui, true, max_lbl)
                                .on_hover_text(if app.response_maximized {
                                    "Restore Request/Response split view"
                                } else {
                                    "Expand Response to full Workbench height"
                                })
                                .clicked()
                            {
                                app.response_maximized = !app.response_maximized;
                            }
                            if swaggen_ui_kit::secondary_button(ui, true, "Save").clicked() {
                                app.submit(Command::SaveResponseFile {
                                    source: resp.raw_body_path.clone(),
                                    destination: PathBuf::from(app.save_response_path.trim()),
                                });
                            }
                            if swaggen_ui_kit::secondary_button(ui, true, "Copy").clicked() {
                                ctx.copy_text(formatted_text.clone());
                                app.status = "Copied response body".into();
                            }
                        });
                    });

                    // Compact Row 2: Merged Sub-tabs (left) + Format & Search (right) on ONE single bar!
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for (tab, label) in [
                            (ResponseSubTab::Body, "Body"),
                            (
                                ResponseSubTab::HeadersCookies,
                                &format!(
                                    "Headers ({}) · Cookies ({})",
                                    resp.headers.len(),
                                    resp.captured_cookies.len()
                                ),
                            ),
                            (ResponseSubTab::Assertions, "Tests"),
                            (
                                ResponseSubTab::Redirects,
                                &format!("Redirects ({})", resp.redirect_chain.len()),
                            ),
                            (ResponseSubTab::Diff, "Diff"),
                            (ResponseSubTab::SaveExample, "Example"),
                        ] {
                            if swaggen_ui_kit::underline_tab(ui, app.response_subtab == tab, label)
                                .clicked()
                            {
                                app.response_subtab = tab;
                            }
                        }

                        if app.response_subtab == ResponseSubTab::Body {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut app.response_search)
                                            .hint_text("Filter JSON…")
                                            .desired_width(128.0),
                                    );
                                    if !app.response_search.trim().is_empty() {
                                        let q = app.response_search.trim().to_ascii_lowercase();
                                        let count = formatted_text
                                            .to_ascii_lowercase()
                                            .matches(&q)
                                            .count();
                                        ui.label(
                                            egui::RichText::new(format!("{count}"))
                                                .size(10.5)
                                                .color(Tokens::STATUS_INFO),
                                        );
                                    }
                                    for (fmt, label) in [
                                        (ResponseBodyFormat::Binary, "Hex"),
                                        (ResponseBodyFormat::Xml, "XML"),
                                        (ResponseBodyFormat::Raw, "Raw"),
                                        (ResponseBodyFormat::PrettyJson, "Pretty"),
                                    ] {
                                        if ui
                                            .selectable_label(app.response_format == fmt, label)
                                            .clicked()
                                        {
                                            app.response_format = fmt;
                                        }
                                    }
                                },
                            );
                        }
                    });
                    ui.add_space(2.0);

                    if resp.preview_truncated {
                        swaggen_ui_kit::notice_banner(
                            ui,
                            NoticeKind::Warning,
                            "Preview capped",
                            &format!(
                                "Showing first {} B of {} B.",
                                resp.preview_bytes.len(),
                                resp.decoded_bytes
                            ),
                        );
                    }
                    if app.response_subtab == ResponseSubTab::Body
                        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw_text)
                        && let Some(errs) = val.get("errors").and_then(|e| e.as_array())
                        && !errs.is_empty()
                    {
                        let first_msg = errs[0]
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("GraphQL execution error");
                        swaggen_ui_kit::notice_banner(
                            ui,
                            NoticeKind::Danger,
                            &format!("GraphQL returned {} error(s)", errs.len()),
                            first_msg,
                        );
                        ui.add_space(2.0);
                    }

                    // Full remaining height dedicated to non-nested 2D ScrollArea
                    let scroll_h = ui.available_height().max(64.0);
                    egui::ScrollArea::both()
                        .id_salt("resp-scroll")
                        .min_scrolled_height(scroll_h)
                        .max_height(scroll_h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| match app.response_subtab {
                            ResponseSubTab::Body => match app.response_format {
                                ResponseBodyFormat::PrettyJson => {
                                    let display_text = if !app.response_search.trim().is_empty() {
                                        let q = app.response_search.trim().to_ascii_lowercase();
                                        let filtered: Vec<&str> = formatted_text
                                            .lines()
                                            .filter(|l| l.to_ascii_lowercase().contains(&q))
                                            .collect();
                                        if filtered.is_empty() {
                                            "(no matching lines)".to_owned()
                                        } else {
                                            filtered.join("\n")
                                        }
                                    } else {
                                        formatted_text.clone()
                                    };
                                    swaggen_ui_kit::code_surface_numbered(ui, &display_text);
                                    if let Ok(val) =
                                        serde_json::from_str::<serde_json::Value>(&raw_text)
                                    {
                                        ui.add_space(4.0);
                                        egui::CollapsingHeader::new(
                                            egui::RichText::new(
                                                "Interactive JSON tree (copy JSONPath)",
                                            )
                                            .size(11.0)
                                            .color(Tokens::TEXT_SECONDARY),
                                        )
                                        .default_open(false)
                                        .show(ui, |ui| {
                                            render_json_tree(
                                                ui,
                                                ctx,
                                                "$",
                                                &val,
                                                &mut app.collapsed_json_paths,
                                            );
                                        });
                                    }
                                }
                                ResponseBodyFormat::Raw | ResponseBodyFormat::Xml => {
                                    swaggen_ui_kit::code_surface_numbered(ui, &raw_text);
                                }
                                ResponseBodyFormat::Binary => {
                                    let preview_hex: String = resp
                                        .preview_bytes
                                        .iter()
                                        .take(256)
                                        .map(|b| format!("{b:02X}"))
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    ui.label(format!(
                                        "Binary payload ({} bytes). First 256 bytes (hex):",
                                        resp.decoded_bytes
                                    ));
                                    swaggen_ui_kit::code_surface(ui, &preview_hex);
                                }
                            },
                            ResponseSubTab::HeadersCookies => {
                                ui.label(swaggen_ui_kit::bold_text(
                                    "Response headers",
                                    11.5,
                                    Tokens::TEXT_PRIMARY,
                                ));
                                for (k, v) in &resp.headers {
                                    ui.monospace(format!("{k}: {v}"));
                                }
                                ui.add_space(4.0);
                                ui.label(swaggen_ui_kit::bold_text(
                                    "Captured cookies",
                                    11.5,
                                    Tokens::TEXT_PRIMARY,
                                ));
                                if resp.captured_cookies.is_empty() {
                                    ui.label("No Set-Cookie headers returned.");
                                } else {
                                    for c in &resp.captured_cookies {
                                        ui.monospace(format!(
                                            "{}={} (domain={}, path={}, secure={}, httpOnly={})",
                                            c.name,
                                            c.value,
                                            c.domain,
                                            c.path,
                                            c.secure,
                                            c.http_only
                                        ));
                                    }
                                }
                            }
                            ResponseSubTab::Assertions => {
                                let rules = app
                                    .active_request()
                                    .map(|r| r.assertions.as_slice())
                                    .unwrap_or(&[]);
                                if rules.is_empty() {
                                    ui.label("No assertions defined on this request.");
                                } else {
                                    let results = evaluate_assertions(
                                        rules,
                                        resp.status_code,
                                        resp.duration_ms,
                                        &resp.headers,
                                        &resp.preview_bytes,
                                    );
                                    for res in results {
                                        ui.horizontal(|ui| {
                                            swaggen_ui_kit::status_badge(
                                                ui,
                                                if res.passed {
                                                    NoticeKind::Success
                                                } else {
                                                    NoticeKind::Danger
                                                },
                                                if res.passed { "PASS" } else { "FAIL" },
                                            );
                                            ui.label(egui::RichText::new(&res.label).strong());
                                            ui.label(&res.detail);
                                        });
                                    }
                                }
                            }
                            ResponseSubTab::Redirects => {
                                if resp.redirect_chain.is_empty() {
                                    ui.label("No redirects occurred.");
                                } else {
                                    for (i, hop) in resp.redirect_chain.iter().enumerate() {
                                        ui.monospace(format!(
                                            "#{}: HTTP {} {} -> {} (auth_stripped={})",
                                            i + 1,
                                            hop.status,
                                            hop.from_url,
                                            hop.to_url,
                                            hop.stripped_auth
                                        ));
                                    }
                                }
                            }
                            ResponseSubTab::Diff => {
                                ui.horizontal(|ui| {
                                    ui.label("Ignore JSON fields (CSV):");
                                    ui.text_edit_singleline(&mut app.diff_ignore_paths);
                                });
                                if let Some(prev) = app.previous_responses.get(&req_id) {
                                    let ignore: Vec<String> = app
                                        .diff_ignore_paths
                                        .split(',')
                                        .map(|s| s.trim().to_owned())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    let prev_txt = String::from_utf8_lossy(&prev.preview_bytes);
                                    let curr_txt = String::from_utf8_lossy(&resp.preview_bytes);
                                    let diff = swaggen_domain::dynamic_vars::compare_responses(
                                        prev.status_code,
                                        &prev.headers,
                                        &prev_txt,
                                        resp.status_code,
                                        &resp.headers,
                                        &curr_txt,
                                        &ignore,
                                        &["date".into()],
                                    );
                                    swaggen_ui_kit::status_badge(
                                        ui,
                                        if diff.status_changed.is_some()
                                            || !diff.header_diffs.is_empty()
                                            || !diff.body_diffs.is_empty()
                                        {
                                            NoticeKind::Warning
                                        } else {
                                            NoticeKind::Success
                                        },
                                        &format!(
                                            "header_diffs={} body_diffs={}",
                                            diff.header_diffs.len(),
                                            diff.body_diffs.len()
                                        ),
                                    );
                                    for line in
                                        diff.header_diffs.iter().chain(diff.body_diffs.iter())
                                    {
                                        ui.monospace(format!(
                                            "{}: {} -> {}",
                                            line.path, line.left, line.right
                                        ));
                                    }
                                } else {
                                    ui.label(
                                        "Send this request at least twice to diff responses.",
                                    );
                                }
                            }
                            ResponseSubTab::SaveExample => {
                                ui.horizontal(|ui| {
                                    ui.label("Name:");
                                    ui.text_edit_singleline(&mut app.example_name_input);
                                    ui.label("Extra redact:");
                                    ui.text_edit_singleline(&mut app.example_extra_fields);
                                });
                                let policy = RedactionPolicy {
                                    extra_fields: app
                                        .example_extra_fields
                                        .split(',')
                                        .map(|s| s.trim().to_owned())
                                        .filter(|s| !s.is_empty())
                                        .collect(),
                                    extra_literal_values: Vec::new(),
                                };
                                let now_secs = time::OffsetDateTime::now_utc().unix_timestamp();
                                let ex = build_sanitized_example(
                                    &req_id,
                                    app.example_name_input.trim(),
                                    resp.status_code,
                                    &resp.headers,
                                    &raw_text,
                                    &policy,
                                    now_secs,
                                );
                                swaggen_ui_kit::notice_banner(
                                    ui,
                                    NoticeKind::Warning,
                                    &format!("Sanitized ({} redactions)", ex.sanitized_redactions),
                                    "Secrets are automatically redacted before saving.",
                                );
                                swaggen_ui_kit::code_surface(ui, &ex.body_preview);
                                ui.horizontal(|ui| {
                                    if swaggen_ui_kit::primary_button(
                                        ui,
                                        true,
                                        "Save sanitized example",
                                    )
                                    .clicked()
                                    {
                                        app.state.examples.push(ex);
                                        app.mark_dirty();
                                        app.status = "Saved sanitized example".into();
                                    }
                                    if swaggen_ui_kit::secondary_button(
                                        ui,
                                        true,
                                        "Copy share JSON",
                                    )
                                    .clicked()
                                    {
                                        let req_title = app
                                            .active_request()
                                            .map(|r| r.name.as_str())
                                            .unwrap_or("Shared Run");
                                        let req_method = app
                                            .active_request()
                                            .map(|r| r.method.as_str())
                                            .unwrap_or("GET");
                                        let pkg =
                                            swaggen_domain::governance::create_sanitized_share_package(
                                                req_title,
                                                req_method,
                                                &resp.final_url,
                                                &resp.headers,
                                                &raw_text,
                                                Some(resp.status_code),
                                                Some(resp.duration_ms),
                                                &policy,
                                            );
                                        if let Ok(js) = serde_json::to_string_pretty(&pkg) {
                                            ctx.copy_text(js);
                                            app.status = format!(
                                                "Copied share JSON ({} redactions)",
                                                pkg.total_redactions
                                            );
                                        }
                                    }
                                });
                            }
                        });
                }
            }
        });
}

fn render_json_tree(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    path: &str,
    value: &serde_json::Value,
    collapsed: &mut std::collections::BTreeSet<String>,
) {
    match value {
        serde_json::Value::Object(map) => {
            let is_collapsed = collapsed.contains(path);
            ui.horizontal(|ui| {
                if ui
                    .small_button(if is_collapsed { "+" } else { "-" })
                    .clicked()
                {
                    if is_collapsed {
                        collapsed.remove(path);
                    } else {
                        collapsed.insert(path.to_owned());
                    }
                }
                ui.monospace(format!("{path} {{{} keys}}", map.len()));
                if ui.small_button("Copy path").clicked() {
                    ctx.copy_text(path.to_owned());
                }
            });
            if !is_collapsed {
                ui.indent(path, |ui| {
                    for (k, v) in map {
                        render_json_tree(ui, ctx, &format!("{path}.{k}"), v, collapsed);
                    }
                });
            }
        }
        serde_json::Value::Array(items) => {
            let is_collapsed = collapsed.contains(path);
            ui.horizontal(|ui| {
                if ui
                    .small_button(if is_collapsed { "+" } else { "-" })
                    .clicked()
                {
                    if is_collapsed {
                        collapsed.remove(path);
                    } else {
                        collapsed.insert(path.to_owned());
                    }
                }
                ui.monospace(format!("{path} [{} items]", items.len()));
            });
            if !is_collapsed {
                ui.indent(path, |ui| {
                    for (idx, item) in items.iter().take(200).enumerate() {
                        render_json_tree(ui, ctx, &format!("{path}[{idx}]"), item, collapsed);
                    }
                    if items.len() > 200 {
                        ui.label(format!("… {} more items omitted", items.len() - 200));
                    }
                });
            }
        }
        scalar => {
            ui.horizontal(|ui| {
                ui.monospace(format!("{path}: {scalar}"));
                if ui.small_button("Copy path").clicked() {
                    ctx.copy_text(path.to_owned());
                }
                if ui.small_button("Copy value").clicked() {
                    let s = match scalar {
                        serde_json::Value::String(v) => v.clone(),
                        other => other.to_string(),
                    };
                    ctx.copy_text(s);
                }
            });
        }
    }
}
