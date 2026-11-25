//! Figma R01-aligned Response Inspector:
//! always visible below Request Configuration, consumes remaining vertical space,
//! shows in-flight spinner/progress, clean metrics bar, Copy body & Save file actions,
//! Pretty/Raw/XML/Binary formatting, searchable body preview, and collapsible JSON tree.
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
    let min_card_h = ui.available_height().max(130.0);

    egui::Frame::new()
        .fill(Tokens::BG_SURFACE)
        .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(18, 12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height((min_card_h - 24.0).max(105.0));

            if is_inflight {
                ui.horizontal(|ui| {
                    ui.label(swaggen_ui_kit::bold_text(
                        "Response",
                        13.5,
                        Tokens::TEXT_PRIMARY,
                    ));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let max_lbl = if app.response_maximized {
                            "Restore split"
                        } else {
                            "Expand"
                        };
                        if swaggen_ui_kit::secondary_button(ui, true, max_lbl).clicked() {
                            app.response_maximized = !app.response_maximized;
                        }
                    });
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(swaggen_ui_kit::bold_text(
                        "Sending request…",
                        13.0,
                        Tokens::STATUS_INFO,
                    ));
                    if let Some((_, phase, done, total)) = app.transfer_progress {
                        let txt = match total {
                            Some(t) => format!("({phase}: {done}/{t} B)"),
                            None => format!("({phase}: {done} B)"),
                        };
                        ui.label(
                            egui::RichText::new(txt)
                                .size(12.0)
                                .color(Tokens::TEXT_SECONDARY),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if swaggen_ui_kit::danger_button(ui, true, "Cancel").clicked() {
                            app.cancel_inflight();
                        }
                    });
                });
                return;
            }

            let Some(result) = app.responses.get(&req_id).cloned() else {
                ui.horizontal(|ui| {
                    ui.label(swaggen_ui_kit::bold_text(
                        "Response",
                        13.5,
                        Tokens::TEXT_PRIMARY,
                    ));
                    if app.response_maximized {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if swaggen_ui_kit::secondary_button(ui, true, "Restore split")
                                .clicked()
                            {
                                app.response_maximized = false;
                            }
                        });
                    }
                });
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "Click 'Send request' (or press Ctrl+Enter) to view response body, headers, cookies, and test results here.",
                    )
                    .size(12.5)
                    .color(Tokens::TEXT_SECONDARY),
                );
                return;
            };

            match result {
                Err(err) => {
                    ui.horizontal(|ui| {
                        ui.label(swaggen_ui_kit::bold_text(
                            "Response",
                            13.5,
                            Tokens::TEXT_PRIMARY,
                        ));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let max_lbl = if app.response_maximized {
                                "Restore split"
                            } else {
                                "Expand"
                            };
                            if swaggen_ui_kit::secondary_button(ui, true, max_lbl).clicked() {
                                app.response_maximized = !app.response_maximized;
                            }
                        });
                    });
                    ui.add_space(6.0);
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
                    let formatted_text = if matches!(app.response_format, ResponseBodyFormat::PrettyJson)
                        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw_text)
                    {
                        serde_json::to_string_pretty(&val).unwrap_or_else(|_| raw_text.clone())
                    } else {
                        raw_text.clone()
                    };

                    // Top bar: Response title + status badge + timings + Copy/Save buttons
                    ui.horizontal_wrapped(|ui| {
                        ui.label(swaggen_ui_kit::bold_text(
                            "Response",
                            13.5,
                            Tokens::TEXT_PRIMARY,
                        ));
                        ui.add_space(8.0);
                        swaggen_ui_kit::status_badge(
                            ui,
                            status_kind,
                            &format!("{} {}", resp.status_code, resp.reason),
                        );
                        ui.label(
                            egui::RichText::new(format!("{} ms", resp.duration_ms))
                                .size(12.0)
                                .color(Tokens::TEXT_SECONDARY),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{:.2} KB decoded · {} B wire",
                                resp.decoded_bytes as f64 / 1024.0,
                                resp.wire_bytes
                            ))
                            .size(12.0)
                            .color(Tokens::TEXT_SECONDARY),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "TTFB {}ms",
                                resp.timings.ttfb_ms.unwrap_or(0),
                            ))
                            .size(11.5)
                            .color(Tokens::TEXT_SECONDARY),
                        );
                        if resp.tls_exception_used {
                            swaggen_ui_kit::status_badge(
                                ui,
                                NoticeKind::Warning,
                                "Host TLS exception",
                            );
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let max_lbl = if app.response_maximized {
                                "Restore split"
                            } else {
                                "Expand"
                            };
                            if swaggen_ui_kit::secondary_button(ui, true, max_lbl)
                                .on_hover_text(if app.response_maximized {
                                    "Restore Request configuration split view"
                                } else {
                                    "Expand Response to fill the full Workbench height"
                                })
                                .clicked()
                            {
                                app.response_maximized = !app.response_maximized;
                            }
                            if swaggen_ui_kit::secondary_button(ui, true, "Save raw").clicked() {
                                app.submit(Command::SaveResponseFile {
                                    source: resp.raw_body_path.clone(),
                                    destination: PathBuf::from(app.save_response_path.trim()),
                                });
                            }
                            if swaggen_ui_kit::secondary_button(ui, true, "Copy body").clicked() {
                                ctx.copy_text(formatted_text.clone());
                                app.status = "Copied response body to clipboard".into();
                            }
                        });
                    });

                    if resp.preview_truncated {
                        ui.add_space(6.0);
                        swaggen_ui_kit::notice_banner(
                            ui,
                            NoticeKind::Warning,
                            "Large response preview capped",
                            &format!(
                                "Showing first {} bytes of {} bytes. Full body was streamed to local cache.",
                                resp.preview_bytes.len(),
                                resp.decoded_bytes
                            ),
                        );
                    }

                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        for (tab, label) in [
                            (ResponseSubTab::Body, "Body"),
                            (
                                ResponseSubTab::HeadersCookies,
                                &format!(
                                    "Headers ({}) & Cookies ({})",
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
                            (ResponseSubTab::SaveExample, "Save example"),
                        ] {
                            if swaggen_ui_kit::underline_tab(ui, app.response_subtab == tab, label)
                                .clicked()
                            {
                                app.response_subtab = tab;
                            }
                        }
                    });
                    ui.add_space(6.0);

                    if app.response_subtab == ResponseSubTab::Body {
                        ui.horizontal_wrapped(|ui| {
                            for (fmt, label) in [
                                (ResponseBodyFormat::PrettyJson, "Pretty"),
                                (ResponseBodyFormat::Raw, "Raw"),
                                (ResponseBodyFormat::Xml, "XML"),
                                (ResponseBodyFormat::Binary, "Binary"),
                            ] {
                                if swaggen_ui_kit::underline_tab(
                                    ui,
                                    app.response_format == fmt,
                                    label,
                                )
                                .clicked()
                                {
                                    app.response_format = fmt;
                                }
                            }
                            ui.add_space(8.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut app.response_search)
                                    .hint_text("Search preview…")
                                    .desired_width(180.0),
                            );
                            if !app.response_search.trim().is_empty() {
                                let q = app.response_search.trim().to_ascii_lowercase();
                                let count = formatted_text
                                    .to_ascii_lowercase()
                                    .matches(&q)
                                    .count();
                                ui.label(
                                    egui::RichText::new(format!("{count} match(es)"))
                                        .size(11.5)
                                        .color(Tokens::STATUS_INFO),
                                );
                            }
                        });
                        ui.add_space(4.0);
                    }

                    // Dedicated non-nested 2D ScrollArea filling remaining card height cleanly!
                    let scroll_h = ui.available_height().max(90.0);
                    egui::ScrollArea::both()
                        .id_salt("resp-scroll")
                        .min_scrolled_height(scroll_h)
                        .max_height(scroll_h)
                        .auto_shrink([false, false])
                        .show(ui, |ui| match app.response_subtab {
                            ResponseSubTab::Body => {
                                match app.response_format {
                                    ResponseBodyFormat::PrettyJson => {
                                        let display_text = if !app.response_search.trim().is_empty()
                                        {
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
                                            ui.add_space(6.0);
                                            egui::CollapsingHeader::new(
                                                egui::RichText::new(
                                                    "Interactive JSON tree (copy JSONPath)",
                                                )
                                                .size(12.0)
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
                                }
                            }
                            ResponseSubTab::HeadersCookies => {
                                ui.label(swaggen_ui_kit::bold_text(
                                    "Response headers",
                                    12.0,
                                    Tokens::TEXT_PRIMARY,
                                ));
                                for (k, v) in &resp.headers {
                                    ui.monospace(format!("{k}: {v}"));
                                }
                                ui.add_space(6.0);
                                ui.label(swaggen_ui_kit::bold_text(
                                    "Captured cookies",
                                    12.0,
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
                                    ui.label("Ignore dynamic JSON fields (CSV):");
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
                                    ui.label("Example name:");
                                    ui.text_edit_singleline(&mut app.example_name_input);
                                    ui.label("Extra redact fields:");
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
                                    &format!("Sanitized preview ({} redactions)", ex.sanitized_redactions),
                                    "Secrets are automatically redacted before saving as a shared example.",
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
                                        app.status = "Saved sanitized example to workspace".into();
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
