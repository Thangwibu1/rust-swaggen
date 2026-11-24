//! Response inspector: status, wire vs decoded size, collapsible JSON tree, raw/XML/binary,
//! body search, duplicate headers & cookies, redirect chain, large-response save,
//! and sanitized example sharing (RS01-RS08, Q26-Q35).
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

    let Some(result) = app.responses.get(&req_id).cloned() else {
        ui.label(
            egui::RichText::new("Response will appear here after sending.")
                .color(Tokens::TEXT_SECONDARY),
        );
        return;
    };

    match result {
        Err(err) => {
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
            ui.horizontal_wrapped(|ui| {
                swaggen_ui_kit::status_badge(
                    ui,
                    status_kind,
                    &format!("{} {}", resp.status_code, resp.reason),
                );
                ui.label(format!("{} ms", resp.duration_ms));
                ui.label(format!(
                    "Wire: {} B · Decoded: {} B · Timings(ms): DNS={:?} TCP={:?} TLS={:?} TTFB={:?} Total={}",
                    resp.wire_bytes,
                    resp.decoded_bytes,
                    resp.timings.dns_ms,
                    resp.timings.tcp_ms,
                    resp.timings.tls_ms,
                    resp.timings.ttfb_ms,
                    resp.timings.total_ms
                ));
                if resp.tls_exception_used {
                    swaggen_ui_kit::status_badge(
                        ui,
                        NoticeKind::Warning,
                        "Host-scoped TLS exception used",
                    );
                }
            });

            if resp.preview_truncated {
                ui.add_space(4.0);
                swaggen_ui_kit::notice_banner(
                    ui,
                    NoticeKind::Warning,
                    "Large response preview capped (RS06)",
                    &format!(
                        "Showing first {} bytes of {} bytes. Full body was streamed to local cache.",
                        resp.preview_bytes.len(),
                        resp.decoded_bytes
                    ),
                );
            }

            ui.horizontal_wrapped(|ui| {
                ui.label("Save raw file to:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.save_response_path).desired_width(260.0),
                );
                if swaggen_ui_kit::secondary_button(ui, true, "Save raw response").clicked() {
                    app.submit(Command::SaveResponseFile {
                        source: resp.raw_body_path.clone(),
                        destination: PathBuf::from(app.save_response_path.trim()),
                    });
                }
            });

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
                    (ResponseSubTab::Assertions, "Assertions (TS01)"),
                    (
                        ResponseSubTab::Redirects,
                        &format!("Redirects ({})", resp.redirect_chain.len()),
                    ),
                    (ResponseSubTab::Diff, "Diff (RS09)"),
                    (ResponseSubTab::SaveExample, "Save shared example (RS08)"),
                ] {
                    if ui
                        .selectable_label(app.response_subtab == tab, label)
                        .clicked()
                    {
                        app.response_subtab = tab;
                    }
                }
            });
            ui.separator();

            egui::ScrollArea::vertical()
                .id_salt("resp-scroll")
                .show(ui, |ui| match app.response_subtab {
                    ResponseSubTab::Body => {
                        ui.horizontal_wrapped(|ui| {
                            for (fmt, label) in [
                                (ResponseBodyFormat::PrettyJson, "Pretty JSON"),
                                (ResponseBodyFormat::Raw, "Raw"),
                                (ResponseBodyFormat::Xml, "XML"),
                                (ResponseBodyFormat::Binary, "Binary"),
                            ] {
                                if ui
                                    .selectable_label(app.response_format == fmt, label)
                                    .clicked()
                                {
                                    app.response_format = fmt;
                                }
                            }
                            ui.separator();
                            ui.label("Search:");
                            ui.add(
                                egui::TextEdit::singleline(&mut app.response_search)
                                    .desired_width(140.0),
                            );
                        });

                        let text = String::from_utf8_lossy(&resp.preview_bytes);
                        if !app.response_search.trim().is_empty() {
                            let q = app.response_search.trim().to_ascii_lowercase();
                            let count = text.to_ascii_lowercase().matches(&q).count();
                            ui.label(
                                egui::RichText::new(format!("{count} match(es) in preview"))
                                    .color(Tokens::STATUS_INFO),
                            );
                        }

                        match app.response_format {
                            ResponseBodyFormat::PrettyJson => {
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                    render_json_tree(
                                        ui,
                                        ctx,
                                        "$",
                                        &val,
                                        &mut app.collapsed_json_paths,
                                    );
                                } else {
                                    swaggen_ui_kit::code_surface(ui, &text);
                                }
                            }
                            ResponseBodyFormat::Raw | ResponseBodyFormat::Xml => {
                                swaggen_ui_kit::code_surface(ui, &text);
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
                        ui.label(
                            egui::RichText::new("Response headers (duplicates preserved)").strong(),
                        );
                        for (k, v) in &resp.headers {
                            ui.monospace(format!("{k}: {v}"));
                        }
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Captured cookies").strong());
                        if resp.captured_cookies.is_empty() {
                            ui.label("No Set-Cookie headers returned.");
                        } else {
                            for c in &resp.captured_cookies {
                                ui.monospace(format!(
                                    "{}={} (domain={}, path={}, secure={}, httpOnly={})",
                                    c.name, c.value, c.domain, c.path, c.secure, c.http_only
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
                            for line in diff.header_diffs.iter().chain(diff.body_diffs.iter()) {
                                ui.monospace(format!(
                                    "{}: {} -> {}",
                                    line.path, line.left, line.right
                                ));
                            }
                        } else {
                            ui.label("Send this request at least twice to diff the previous and latest responses.");
                        }
                    }
                    ResponseSubTab::SaveExample => {
                        ui.horizontal(|ui| {
                            ui.label("Example name:");
                            ui.text_edit_singleline(&mut app.example_name_input);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Extra fields to redact (CSV):");
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
                        let preview_text = String::from_utf8_lossy(&resp.preview_bytes);
                        let now_secs = time::OffsetDateTime::now_utc().unix_timestamp();
                        let ex = build_sanitized_example(
                            &req_id,
                            app.example_name_input.trim(),
                            resp.status_code,
                            &resp.headers,
                            &preview_text,
                            &policy,
                            now_secs,
                        );
                        swaggen_ui_kit::notice_banner(
                            ui,
                            NoticeKind::Warning,
                            &format!("Sanitized preview ({} redactions)", ex.sanitized_redactions),
                            "Review carefully before saving as a shared example (Q32).",
                        );
                        swaggen_ui_kit::code_surface(ui, &ex.body_preview);
                        if swaggen_ui_kit::primary_button(ui, true, "Save sanitized example")
                            .clicked()
                        {
                            app.state.examples.push(ex);
                            app.mark_dirty();
                            app.status = "Sanitized response example saved to workspace".into();
                        }
                        if swaggen_ui_kit::secondary_button(ui, true, "Copy Sanitized Share JSON (CO08)")
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
                            let pkg = swaggen_domain::governance::create_sanitized_share_package(
                                req_title,
                                req_method,
                                &resp.final_url,
                                &resp.headers,
                                &preview_text,
                                Some(resp.status_code),
                                Some(resp.duration_ms),
                                &policy,
                            );
                            if let Ok(js) = serde_json::to_string_pretty(&pkg) {
                                ctx.copy_text(js);
                                app.status = format!(
                                    "Copied sanitized share package ({} redactions)",
                                    pkg.total_redactions
                                );
                            }
                        }
                    }
                });
        }
    }
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
