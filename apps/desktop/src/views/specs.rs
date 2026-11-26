//! Clean, Distraction-Free OpenAPI / Swagger / Postman Documentation Viewer:
//! - Only 2 top actions: Fetch Docs URL or Load / Drag-and-Drop File (.json / .yaml / .html)
//! - Crisp Swagger Docs Preview with tag filtering, endpoint schemas, and 1-click `Try in Workbench →`
use crate::{app::App, worker::Command};
use eframe::egui;
use std::collections::BTreeMap;
use swaggen_spec_engine::openapi::{
    SpecDocument, generate_linked_request, parse_api_documentation,
};
use swaggen_ui_kit::Tokens;

fn pick_spec_file_windows() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let script = "Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.OpenFileDialog; $f.Title = 'Select API Documentation File'; $f.Filter = 'API Docs (*.json;*.yaml;*.yml;*.html)|*.json;*.yaml;*.yml;*.html|All Files (*.*)|*.*'; if ($f.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Write-Output $f.FileName }";
        let out = std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-Sta",
                "-WindowStyle",
                "Hidden",
                "-Command",
                script,
            ])
            .output()
            .ok()?;
        let path_str = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if path_str.is_empty() {
            None
        } else {
            Some(std::path::PathBuf::from(path_str))
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

pub fn load_spec_into_app(app: &mut App, fname: String, raw: &str) {
    let doc = parse_api_documentation(
        swaggen_domain::workspace::new_uid(),
        fname.clone(),
        raw,
        &[],
    );
    if doc.operations.is_empty()
        && (raw
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("<!doctype html")
            || raw.trim_start().to_ascii_lowercase().starts_with("<html"))
    {
        app.error = true;
        app.status = format!(
            "'{fname}' is a plain HTML webpage without Swagger/OpenAPI endpoints. Provide an OpenAPI/Swagger JSON, YAML, or Swagger UI file."
        );
        return;
    }
    app.spec_editor_text = doc.raw_text.clone();
    // Auto-prune any leftover broken 0-endpoint HTML placeholders
    app.specs.retain(|s| !s.operations.is_empty());
    app.specs.push(doc);
    app.selected_spec_idx = app.specs.len() - 1;
    app.spec_view_mode = 0;
    app.error = false;
    app.submit(Command::SaveSpecs {
        specs: app.specs.clone(),
    });
    app.status = format!("Loaded API documentation '{fname}'");
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::page_header(
        ui,
        "API Documentation",
        "Fetch from a Swagger / OpenAPI URL, or load & drag-drop a .json / .yaml / .html specification file.",
        None,
        false,
    );

    // Clean 2-Option Import Card: (1) Fetch URL  |  (2) Load / Drag-and-Drop File
    let is_hovering_drop = !ui.ctx().input(|i| i.raw.hovered_files.is_empty());
    egui::Frame::new()
        .fill(if is_hovering_drop {
            swaggen_ui_kit::selected_bg(ui)
        } else {
            swaggen_ui_kit::surface_bg(ui)
        })
        .stroke(egui::Stroke::new(
            if is_hovering_drop { 1.5 } else { 1.0 },
            if is_hovering_drop {
                Tokens::STATUS_INFO
            } else {
                swaggen_ui_kit::border_color(ui)
            },
        ))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            let total_w = ui.available_width();
            let url_w = (total_w - 350.0).clamp(200.0, 520.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let url_resp = swaggen_ui_kit::text_input(
                    ui,
                    &mut app.spec_import_url,
                    "https://api.example.com/openapi.json or /swagger",
                    url_w,
                    false,
                );
                let trigger_fetch = swaggen_ui_kit::primary_button(
                    ui,
                    !app.pending && !app.spec_import_url.trim().is_empty(),
                    "Fetch URL",
                )
                .clicked()
                    || (url_resp.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && !app.spec_import_url.trim().is_empty());

                if trigger_fetch {
                    let guessed_name = app
                        .spec_import_url
                        .trim()
                        .rsplit('/')
                        .next()
                        .filter(|s| !s.is_empty())
                        .unwrap_or("API Spec")
                        .to_owned();
                    app.submit(Command::FetchSpecUrl {
                        name: guessed_name,
                        url: app.spec_import_url.trim().to_owned(),
                    });
                }

                ui.separator();

                if swaggen_ui_kit::secondary_button(ui, !app.pending, "Upload / Load File…")
                    .on_hover_text(
                        "Pick a local OpenAPI / Swagger (.json, .yaml) or Postman JSON file",
                    )
                    .clicked()
                    && let Some(path) = pick_spec_file_windows()
                {
                    match std::fs::read_to_string(&path) {
                        Ok(raw) => {
                            let fname = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Local Spec")
                                .to_owned();
                            app.spec_local_file_path = path.display().to_string();
                            load_spec_into_app(app, fname, &raw);
                        }
                        Err(e) => {
                            app.error = true;
                            app.status = format!("Failed to read file: {e}");
                        }
                    }
                }

                ui.label(
                    egui::RichText::new(if is_hovering_drop {
                        "Drop file now to open documentation…"
                    } else {
                        "or drag & drop .json / .yaml / .html file here"
                    })
                    .size(11.5)
                    .color(if is_hovering_drop {
                        Tokens::STATUS_INFO
                    } else {
                        swaggen_ui_kit::text_secondary(ui)
                    }),
                );
            });
        });

    ui.add_space(10.0);

    // Clean up any stale 0-endpoint broken entries automatically if valid docs also exist
    if app.specs.iter().any(|s| !s.operations.is_empty())
        && app.specs.iter().any(|s| s.operations.is_empty())
    {
        app.specs.retain(|s| !s.operations.is_empty());
        app.selected_spec_idx = app.selected_spec_idx.min(app.specs.len().saturating_sub(1));
    }

    if app.specs.is_empty() {
        egui::Frame::new()
            .fill(swaggen_ui_kit::subtle_bg(ui))
            .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(24, 36))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(swaggen_ui_kit::bold_text(
                        "No API documentation loaded yet",
                        15.0,
                        swaggen_ui_kit::text_primary(ui),
                    ));
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(
                            "Paste a Swagger / OpenAPI URL above, click 'Upload / Load File…', or drag and drop your .json / .yaml file straight onto this window.",
                        )
                        .size(12.5)
                        .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                });
            });
        return;
    }

    // Minimalist Spec Switcher Pills (only shown when >1 doc or so user can close with ×)
    let mut select_idx: Option<usize> = None;
    let mut delete_idx: Option<usize> = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (idx, doc) in app.specs.iter().enumerate() {
            let active = app.selected_spec_idx == idx;
            let title_lbl = if !doc.title.trim().is_empty() && doc.title != "Untitled" {
                &doc.title
            } else {
                &doc.name
            };
            egui::Frame::new()
                .fill(if active {
                    swaggen_ui_kit::selected_bg(ui)
                } else {
                    swaggen_ui_kit::surface_bg(ui)
                })
                .stroke(egui::Stroke::new(
                    1.0,
                    if active {
                        Tokens::STATUS_INFO
                    } else {
                        swaggen_ui_kit::border_color(ui)
                    },
                ))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let lbl = format!("{title_lbl} ({})", doc.operations.len());
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(lbl)
                                        .size(11.5)
                                        .color(if active {
                                            Tokens::STATUS_INFO
                                        } else {
                                            swaggen_ui_kit::text_primary(ui)
                                        })
                                        .strong(),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            select_idx = Some(idx);
                        }
                        if swaggen_ui_kit::mini_icon_button(
                            ui,
                            ("close_spec_tab", &doc.id),
                            swaggen_ui_kit::MiniIconKind::Close,
                            Tokens::TEXT_DANGER,
                            "Close documentation tab",
                        )
                        .clicked()
                        {
                            delete_idx = Some(idx);
                        }
                    });
                });
        }
    });
    if let Some(i) = select_idx {
        app.selected_spec_idx = i;
        app.spec_editor_text = app.specs[i].raw_text.clone();
    }
    if let Some(i) = delete_idx {
        app.specs.remove(i);
        app.selected_spec_idx = app.selected_spec_idx.min(app.specs.len().saturating_sub(1));
        app.spec_editor_text = app
            .specs
            .get(app.selected_spec_idx)
            .map(|s| s.raw_text.clone())
            .unwrap_or_default();
        app.submit(Command::SaveSpecs {
            specs: app.specs.clone(),
        });
        if app.specs.is_empty() {
            return;
        }
    }

    let idx = app.selected_spec_idx.min(app.specs.len() - 1);
    ui.add_space(8.0);
    let doc_clone = app.specs[idx].clone();
    show_swagger_docs_pane(app, ui, &doc_clone);
}

fn method_accent_colors(method: &str) -> (egui::Color32, egui::Color32) {
    match method {
        "GET" => (
            egui::Color32::from_rgb(37, 99, 235),
            egui::Color32::from_rgba_unmultiplied(37, 99, 235, 16),
        ),
        "POST" => (
            egui::Color32::from_rgb(16, 185, 129),
            egui::Color32::from_rgba_unmultiplied(16, 185, 129, 16),
        ),
        "PUT" => (
            egui::Color32::from_rgb(217, 119, 6),
            egui::Color32::from_rgba_unmultiplied(217, 119, 6, 16),
        ),
        "PATCH" => (
            egui::Color32::from_rgb(124, 58, 237),
            egui::Color32::from_rgba_unmultiplied(124, 58, 237, 16),
        ),
        "DELETE" => (
            egui::Color32::from_rgb(220, 38, 38),
            egui::Color32::from_rgba_unmultiplied(220, 38, 38, 16),
        ),
        _ => (
            Tokens::STATUS_INFO,
            egui::Color32::from_rgba_unmultiplied(71, 85, 105, 14),
        ),
    }
}

fn show_swagger_docs_pane(app: &mut App, ui: &mut egui::Ui, doc_clone: &SpecDocument) {
    let tag_filter_id = egui::Id::unique(("spec_tag_filter", &doc_clone.id));
    let mut selected_tag: Option<String> = ui.data_mut(|d| d.get_temp(tag_filter_id)).flatten();

    swaggen_ui_kit::surface_card(ui, |ui| {
        // API Hero Header
        let heading = if !doc_clone.title.trim().is_empty() && doc_clone.title != "Untitled" {
            &doc_clone.title
        } else {
            &doc_clone.name
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(swaggen_ui_kit::bold_text(
                heading,
                18.0,
                swaggen_ui_kit::text_primary(ui),
            ));
            if !doc_clone.api_version.is_empty() {
                swaggen_ui_kit::pill_badge(
                    ui,
                    Tokens::STATUS_INFO,
                    &format!("v{}", doc_clone.api_version),
                );
            }
            let spec_ver_lbl = match &doc_clone.version {
                Some(swaggen_spec_engine::openapi::OpenApiVersion::V3_1(_)) => "OpenAPI 3.1",
                Some(swaggen_spec_engine::openapi::OpenApiVersion::V3_0(_)) => "OpenAPI 3.0",
                _ => "API Docs",
            };
            swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_INFO, spec_ver_lbl);
            swaggen_ui_kit::pill_badge(
                ui,
                Tokens::STATUS_SUCCESS,
                &format!("{} endpoints", doc_clone.operations.len()),
            );
            if !doc_clone.schemas.is_empty() {
                swaggen_ui_kit::pill_badge(
                    ui,
                    Tokens::TEXT_SECONDARY,
                    &format!("{} schemas", doc_clone.schemas.len()),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.spec_doc_search,
                    "Filter path, summary, schema…",
                    220.0,
                    false,
                );
            });
        });

        // Base Servers & Security Schemes strip
        if !doc_clone.servers.is_empty() || !doc_clone.security_schemes.is_empty() {
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for srv in &doc_clone.servers {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(format!("🌐 {srv}"))
                                    .size(11.0)
                                    .monospace()
                                    .color(swaggen_ui_kit::text_primary(ui)),
                            )
                            .fill(swaggen_ui_kit::subtle_bg(ui))
                            .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                            .corner_radius(5),
                        )
                        .on_hover_text("Click to copy server URL")
                        .clicked()
                    {
                        ui.ctx().copy_text(srv.clone());
                        app.status = format!("Copied server URL: {srv}");
                    }
                }
                for (sname, stype) in &doc_clone.security_schemes {
                    swaggen_ui_kit::pill_badge(
                        ui,
                        Tokens::TEXT_WARNING,
                        &format!("🔐 {sname} ({stype})"),
                    );
                }
            });
        }

        // Tag Filter Bar
        let mut all_tags: Vec<String> = Vec::new();
        for op in &doc_clone.operations {
            let t = op.tags.first().cloned().unwrap_or_else(|| "Default".into());
            if !all_tags.contains(&t) {
                all_tags.push(t);
            }
        }
        if all_tags.len() > 1 {
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .selectable_label(selected_tag.is_none(), "All tags")
                    .clicked()
                {
                    selected_tag = None;
                }
                for t in &all_tags {
                    let active = selected_tag.as_deref() == Some(t.as_str());
                    if ui.selectable_label(active, t).clicked() {
                        selected_tag = if active { None } else { Some(t.clone()) };
                    }
                }
            });
        }
        ui.data_mut(|d| d.insert_temp(tag_filter_id, selected_tag.clone()));
        ui.add_space(8.0);

        let (ops, schemas) = doc_clone.search(&app.spec_doc_search);
        let mut by_tag: BTreeMap<String, Vec<_>> = BTreeMap::new();
        for op in ops {
            let tag = op.tags.first().cloned().unwrap_or_else(|| "Default".into());
            if selected_tag.as_deref().is_some_and(|f| f != tag) {
                continue;
            }
            by_tag.entry(tag).or_default().push(op);
        }

        let docs_h = ui.available_height().max(220.0);
        let mut try_op_uid: Option<String> = None;

        egui::ScrollArea::vertical()
            .id_salt("d01-live-docs")
            .max_height(docs_h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if by_tag.is_empty() && schemas.is_empty() {
                    ui.label(
                        egui::RichText::new("No matching endpoints or schemas in this specification.")
                            .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                }
                for (tag, group_ops) in by_tag {
                    egui::CollapsingHeader::new(swaggen_ui_kit::bold_text(
                        &format!("{tag} ({})", group_ops.len()),
                        14.0,
                        swaggen_ui_kit::text_primary(ui),
                    ))
                    .default_open(true)
                    .show(ui, |ui| {
                        let default_open = group_ops.len() <= 3;
                        for op in group_ops {
                            let (accent, tint) = method_accent_colors(op.method.as_str());
                            let card_id =
                                egui::Id::unique(("spec_op_open", &doc_clone.id, &op.operation_uid));
                            let mut is_open = ui
                                .data_mut(|d| d.get_temp::<bool>(card_id))
                                .unwrap_or(default_open);

                            egui::Frame::new()
                                .fill(swaggen_ui_kit::surface_bg(ui))
                                .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                                .corner_radius(7)
                                .inner_margin(egui::Margin::ZERO)
                                .show(ui, |ui| {
                                    // Colored Swagger Accordion Header Bar
                                    let hdr = egui::Frame::new()
                                        .fill(tint)
                                        .inner_margin(egui::Margin::symmetric(12, 8))
                                        .show(ui, |ui| {
                                            ui.horizontal_wrapped(|ui| {
                                                let arrow = if is_open { "▾" } else { "▸" };
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            egui::RichText::new(arrow)
                                                                .size(12.5)
                                                                .color(accent)
                                                                .strong(),
                                                        )
                                                        .frame(false),
                                                    )
                                                    .clicked()
                                                {
                                                    is_open = !is_open;
                                                }
                                                swaggen_ui_kit::method_badge(ui, op.method.as_str());
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            egui::RichText::new(&op.path)
                                                                .size(13.0)
                                                                .monospace()
                                                                .strong()
                                                                .color(swaggen_ui_kit::text_primary(ui)),
                                                        )
                                                        .frame(false),
                                                    )
                                                    .on_hover_text("Click to expand/collapse endpoint details")
                                                    .clicked()
                                                {
                                                    is_open = !is_open;
                                                }
                                                if !op.summary.is_empty() {
                                                    ui.label(
                                                        egui::RichText::new(&op.summary)
                                                            .size(12.0)
                                                            .color(swaggen_ui_kit::text_secondary(ui)),
                                                    );
                                                }
                                                if let Some(op_id) = &op.operation_id {
                                                    swaggen_ui_kit::pill_badge(
                                                        ui,
                                                        Tokens::TEXT_SECONDARY,
                                                        op_id,
                                                    );
                                                }
                                                if op.deprecated {
                                                    swaggen_ui_kit::pill_badge(
                                                        ui,
                                                        Tokens::TEXT_WARNING,
                                                        "Deprecated",
                                                    );
                                                }
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        if swaggen_ui_kit::primary_button(
                                                            ui,
                                                            true,
                                                            "Try in Workbench →",
                                                        )
                                                        .clicked()
                                                        {
                                                            try_op_uid =
                                                                Some(op.operation_uid.clone());
                                                        }
                                                    },
                                                );
                                            });
                                        });
                                    // Accent 3px left stripe on card header
                                    let r = hdr.response.rect;
                                    ui.painter().line_segment(
                                        [r.left_top(), r.left_bottom()],
                                        egui::Stroke::new(3.0, accent),
                                    );

                                    if is_open {
                                        egui::Frame::new()
                                            .fill(swaggen_ui_kit::surface_bg(ui))
                                            .inner_margin(egui::Margin::symmetric(14, 10))
                                            .show(ui, |ui| {
                                                if !op.description.is_empty() {
                                                    ui.label(
                                                        egui::RichText::new(&op.description)
                                                            .size(12.0)
                                                            .color(swaggen_ui_kit::text_secondary(ui)),
                                                    );
                                                    ui.add_space(6.0);
                                                }

                                                // Parameters Grid
                                                if !op.parameters.is_empty() {
                                                    ui.label(swaggen_ui_kit::bold_text(
                                                        "Parameters",
                                                        12.5,
                                                        swaggen_ui_kit::text_primary(ui),
                                                    ));
                                                    ui.add_space(4.0);
                                                    egui::Grid::new(egui::Id::unique((
                                                        "doc_params",
                                                        &doc_clone.id,
                                                        &op.operation_uid,
                                                    )))
                                                    .striped(true)
                                                    .min_col_width(72.0)
                                                    .spacing([12.0, 5.0])
                                                    .show(ui, |ui| {
                                                        for h in ["NAME", "IN", "TYPE", "DESCRIPTION"] {
                                                            ui.label(
                                                                egui::RichText::new(h)
                                                                    .size(10.0)
                                                                    .strong()
                                                                    .color(swaggen_ui_kit::text_secondary(ui)),
                                                            );
                                                        }
                                                        ui.end_row();
                                                        for p in &op.parameters {
                                                            ui.horizontal(|ui| {
                                                                ui.label(
                                                                    egui::RichText::new(&p.name)
                                                                        .monospace()
                                                                        .strong()
                                                                        .size(11.5),
                                                                );
                                                                if p.required {
                                                                    ui.label(
                                                                        egui::RichText::new("*")
                                                                            .color(Tokens::TEXT_DANGER)
                                                                            .strong(),
                                                                    );
                                                                }
                                                            });
                                                            swaggen_ui_kit::pill_badge(
                                                                ui,
                                                                Tokens::STATUS_INFO,
                                                                &p.location,
                                                            );
                                                            let mut t_desc = p.schema_type.clone();
                                                            if p.nullable {
                                                                t_desc.push_str(" | null");
                                                            }
                                                            if let Some(def) = &p.default_value {
                                                                t_desc.push_str(&format!(" = {def}"));
                                                            }
                                                            if !p.enum_values.is_empty() {
                                                                t_desc.push_str(&format!(
                                                                    " [{}]",
                                                                    p.enum_values.join(", ")
                                                                ));
                                                            }
                                                            ui.label(
                                                                egui::RichText::new(t_desc)
                                                                    .monospace()
                                                                    .size(11.0)
                                                                    .color(swaggen_ui_kit::text_secondary(ui)),
                                                            );
                                                            ui.label(
                                                                egui::RichText::new(if p.description.is_empty() {
                                                                    "—"
                                                                } else {
                                                                    &p.description
                                                                })
                                                                .size(11.5)
                                                                .color(swaggen_ui_kit::text_secondary(ui)),
                                                            );
                                                            ui.end_row();
                                                        }
                                                    });
                                                    ui.add_space(8.0);
                                                }

                                                // Request Body
                                                if let Some(rb) = &op.request_body {
                                                    ui.horizontal_wrapped(|ui| {
                                                        ui.label(swaggen_ui_kit::bold_text(
                                                            "Request Body",
                                                            12.5,
                                                            swaggen_ui_kit::text_primary(ui),
                                                        ));
                                                        if rb.required {
                                                            swaggen_ui_kit::pill_badge(
                                                                ui,
                                                                Tokens::TEXT_DANGER,
                                                                "required",
                                                            );
                                                        }
                                                        for ct in &rb.content_types {
                                                            swaggen_ui_kit::pill_badge(
                                                                ui,
                                                                Tokens::STATUS_INFO,
                                                                ct,
                                                            );
                                                        }
                                                        if let Some(ex) = &rb.example_json
                                                            && ui.small_button("Copy JSON").clicked()
                                                        {
                                                            ui.ctx().copy_text(ex.clone());
                                                            app.status = "Copied request body JSON".into();
                                                        }
                                                    });
                                                    if let Some(ex) = &rb.example_json {
                                                        ui.add_space(4.0);
                                                        egui::Frame::new()
                                                            .fill(swaggen_ui_kit::subtle_bg(ui))
                                                            .stroke(egui::Stroke::new(
                                                                1.0,
                                                                swaggen_ui_kit::border_color(ui),
                                                            ))
                                                            .corner_radius(5)
                                                            .inner_margin(egui::Margin::same(8))
                                                            .show(ui, |ui| {
                                                                let mut preview = ex.clone();
                                                                ui.add(
                                                                    egui::TextEdit::multiline(&mut preview)
                                                                        .desired_width(ui.available_width())
                                                                        .desired_rows(
                                                                            ex.lines().count().clamp(2, 10),
                                                                        )
                                                                        .code_editor()
                                                                        .interactive(false),
                                                                );
                                                            });
                                                    }
                                                    ui.add_space(8.0);
                                                }

                                                // Responses
                                                if !op.responses.is_empty() {
                                                    ui.label(swaggen_ui_kit::bold_text(
                                                        "Responses",
                                                        12.5,
                                                        swaggen_ui_kit::text_primary(ui),
                                                    ));
                                                    ui.add_space(4.0);
                                                    for resp in &op.responses {
                                                        let r_col = if resp.status.starts_with('2') {
                                                            Tokens::STATUS_SUCCESS
                                                        } else if resp.status.starts_with('4') {
                                                            Tokens::TEXT_WARNING
                                                        } else if resp.status.starts_with('5') {
                                                            Tokens::TEXT_DANGER
                                                        } else {
                                                            Tokens::STATUS_INFO
                                                        };
                                                        ui.horizontal_wrapped(|ui| {
                                                            swaggen_ui_kit::pill_badge(
                                                                ui,
                                                                r_col,
                                                                &resp.status,
                                                            );
                                                            if !resp.description.is_empty() {
                                                                ui.label(
                                                                    egui::RichText::new(&resp.description)
                                                                        .size(12.0)
                                                                        .color(swaggen_ui_kit::text_primary(ui)),
                                                                );
                                                            }
                                                            for ct in &resp.content_types {
                                                                swaggen_ui_kit::pill_badge(
                                                                    ui,
                                                                    Tokens::TEXT_SECONDARY,
                                                                    ct,
                                                                );
                                                            }
                                                            if let Some(ex) = &resp.example_json
                                                                && ui.small_button("Copy JSON").clicked()
                                                            {
                                                                ui.ctx().copy_text(ex.clone());
                                                                app.status = format!(
                                                                    "Copied {} response JSON",
                                                                    resp.status
                                                                );
                                                            }
                                                        });
                                                        if let Some(ex) = &resp.example_json {
                                                            ui.add_space(3.0);
                                                            egui::Frame::new()
                                                                .fill(swaggen_ui_kit::subtle_bg(ui))
                                                                .stroke(egui::Stroke::new(
                                                                    1.0,
                                                                    swaggen_ui_kit::border_color(ui),
                                                                ))
                                                                .corner_radius(5)
                                                                .inner_margin(egui::Margin::same(8))
                                                                .show(ui, |ui| {
                                                                    let mut preview = ex.clone();
                                                                    ui.add(
                                                                        egui::TextEdit::multiline(&mut preview)
                                                                            .desired_width(ui.available_width())
                                                                            .desired_rows(
                                                                                ex.lines().count().clamp(2, 10),
                                                                            )
                                                                            .code_editor()
                                                                            .interactive(false),
                                                                    );
                                                                });
                                                        }
                                                        ui.add_space(4.0);
                                                    }
                                                }
                                            });
                                    }
                                });
                            ui.data_mut(|d| d.insert_temp(card_id, is_open));
                            ui.add_space(6.0);
                        }
                    });
                    ui.add_space(4.0);
                }

                if !schemas.is_empty() {
                    ui.add_space(8.0);
                    egui::CollapsingHeader::new(swaggen_ui_kit::bold_text(
                        &format!("Schemas / Models ({})", schemas.len()),
                        13.5,
                        swaggen_ui_kit::text_primary(ui),
                    ))
                    .default_open(false)
                    .show(ui, |ui| {
                        for s in schemas {
                            egui::Frame::new()
                                .fill(swaggen_ui_kit::subtle_bg(ui))
                                .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                                .corner_radius(6)
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(swaggen_ui_kit::bold_text(
                                        &s.name,
                                        13.0,
                                        Tokens::STATUS_INFO,
                                    ));
                                    swaggen_ui_kit::pill_badge(
                                        ui,
                                        Tokens::TEXT_SECONDARY,
                                        &s.schema_type,
                                    );
                                    swaggen_ui_kit::pill_badge(
                                        ui,
                                        Tokens::TEXT_SECONDARY,
                                        &format!("{} properties", s.properties.len()),
                                    );
                                });
                                if !s.properties.is_empty() {
                                    ui.add_space(4.0);
                                    egui::Grid::new(egui::Id::unique((
                                        "doc_schema_props",
                                        &doc_clone.id,
                                        &s.name,
                                    )))
                                    .striped(true)
                                    .spacing([14.0, 4.0])
                                    .show(ui, |ui| {
                                        for p in &s.properties {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&p.name)
                                                        .monospace()
                                                        .strong()
                                                        .size(11.5),
                                                );
                                                if p.required {
                                                    ui.label(
                                                        egui::RichText::new("*")
                                                            .color(Tokens::TEXT_DANGER)
                                                            .strong(),
                                                    );
                                                }
                                            });
                                            ui.label(
                                                egui::RichText::new(if p.nullable {
                                                    format!("{} | null", p.schema_type)
                                                } else {
                                                    p.schema_type.clone()
                                                })
                                                .monospace()
                                                .size(11.0)
                                                .color(Tokens::STATUS_INFO),
                                            );
                                            ui.label(
                                                egui::RichText::new(&p.constraints)
                                                    .size(11.0)
                                                    .color(swaggen_ui_kit::text_secondary(ui)),
                                            );
                                            ui.end_row();
                                        }
                                    });
                                }
                            });
                            ui.add_space(4.0);
                        }
                    });
                }
            });

        if let Some(uid) = try_op_uid
            && let Some(op) = doc_clone.operations.iter().find(|o| o.operation_uid == uid)
            && let Some(col_id) = app.state.collections.first().map(|c| c.id.clone())
        {
            let req = generate_linked_request(doc_clone, op, &col_id);
            let rid = req.id.clone();
            app.state.requests.push(req);
            app.open_request_in_workbench(&rid);
            app.mark_dirty();
        }
    });
}
