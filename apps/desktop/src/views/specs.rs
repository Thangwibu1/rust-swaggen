//! Simple, Self-Explanatory OpenAPI 3.0/3.1 Studio & Swagger Docs Viewer (F6):
//! - Spec tabs with 1-click `×` Delete & inline Rename
//! - Friendly HTML webpage guard (`<!doctype html>`) when a user enters a UI URL instead of `/openapi.json`
//! - 3 clean modes: `Swagger Docs Preview` (default) | `Split Editor + Docs` | `Lint, Drift & SDK Tools`
//! - Responsive 1-column fallback on narrow / split-screen windows (`F5`)
use crate::{app::App, worker::Command};
use eframe::egui;
use std::collections::BTreeMap;
use swaggen_domain::workspace::HttpMethod;
use swaggen_spec_engine::advanced::{
    TeamLintRules, assess_openapi_version, convert_swagger2_to_openapi3, detect_contract_drift,
    export_docs_html, export_docs_markdown, inspect_complex_schema, lint_spec_conventions,
    propose_spec_patch_from_request, resolve_and_bundle_multifile,
    validate_response_against_endpoint,
};
use swaggen_spec_engine::openapi::{
    IssueSeverity, SpecDocument, SpecFormat, add_endpoint_to_spec, diff_and_sync_linked_requests,
    find_replace_spec, format_spec_text, generate_linked_request, parse_and_validate,
};
use swaggen_ui_kit::{NoticeKind, Tokens};

const SAMPLE_OPENAPI_YAML: &str = "openapi: 3.1.0\ninfo:\n  title: Sample Tasks & Users API\n  version: 1.0.0\nservers:\n  - url: https://api.example.test\npaths:\n  /users:\n    get:\n      operationId: listUsers\n      summary: List all users\n      tags:\n        - Users\n      parameters:\n        - name: page\n          in: query\n          required: false\n          schema:\n            type: integer\n      responses:\n        \"200\":\n          description: Paginated user list\n    post:\n      operationId: createUser\n      summary: Create a new user\n      tags:\n        - Users\n      responses:\n        \"201\":\n          description: Created user\n  /tasks/{id}:\n    get:\n      operationId: getTask\n      summary: Get task by ID\n      tags:\n        - Tasks\n      parameters:\n        - name: id\n          in: path\n          required: true\n          schema:\n            type: string\n      responses:\n        \"200\":\n          description: Single task details\n";

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    if swaggen_ui_kit::page_header(
        ui,
        swaggen_ui_kit::tr(
            app.locale,
            "OpenAPI & Swagger Documentation",
            "Tài liệu OpenAPI & Swagger",
        ),
        swaggen_ui_kit::tr(
            app.locale,
            "Browse interactive Swagger API docs, test endpoints in 1 click, or edit OpenAPI 3.0/3.1 YAML/JSON.",
            "Xem tài liệu Swagger trực quan, bấm Try it để gọi thử ngay trong Workbench, hoặc chỉnh sửa OpenAPI YAML/JSON.",
        ),
        Some("+ New Sample OpenAPI 3.1"),
        true,
    ) {
        add_sample_spec(app);
    }

    // 1. Clean Import Bar (URL OR Local `.yaml`/`.json` File)
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        swaggen_ui_kit::text_input(
            ui,
            &mut app.spec_import_url,
            "https://petstore3.swagger.io/api/v3/openapi.json",
            270.0,
            false,
        );
        if swaggen_ui_kit::secondary_button(
            ui,
            !app.spec_import_url.trim().is_empty(),
            "Fetch OpenAPI URL",
        )
        .clicked()
        {
            let guessed_name = app
                .spec_import_url
                .trim()
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or("Imported Spec")
                .to_owned();
            app.submit(Command::FetchSpecUrl {
                name: guessed_name,
                url: app.spec_import_url.trim().to_owned(),
            });
        }

        ui.separator();
        swaggen_ui_kit::text_input(
            ui,
            &mut app.spec_local_file_path,
            "C:\\path\\to\\openapi.yaml (or drag & drop file into window)",
            250.0,
            false,
        );
        if swaggen_ui_kit::secondary_button(
            ui,
            !app.spec_local_file_path.trim().is_empty(),
            "Load File",
        )
        .clicked()
        {
            let path = std::path::Path::new(app.spec_local_file_path.trim());
            match std::fs::read_to_string(path) {
                Ok(raw) => {
                    let fname = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Local Spec")
                        .to_owned();
                    let doc =
                        parse_and_validate(swaggen_domain::workspace::new_uid(), fname, &raw, &[]);
                    app.spec_editor_text = doc.raw_text.clone();
                    app.specs.push(doc);
                    app.selected_spec_idx = app.specs.len() - 1;
                    app.submit(Command::SaveSpecs {
                        specs: app.specs.clone(),
                    });
                    app.status = "Loaded local OpenAPI file".into();
                }
                Err(e) => {
                    app.error = true;
                    app.status = format!("Failed to read file: {e}");
                }
            }
        }
    });
    ui.add_space(6.0);

    if app.specs.is_empty() {
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(16.0);
                ui.label(swaggen_ui_kit::bold_text(
                    "No OpenAPI specification loaded yet",
                    15.0,
                    swaggen_ui_kit::text_primary(ui),
                ));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Import an OpenAPI 3.0 / 3.1 JSON or YAML URL above, drag & drop a file into Swaggen, or load a ready-made sample.",
                    )
                    .color(swaggen_ui_kit::text_secondary(ui)),
                );
                ui.add_space(10.0);
                if swaggen_ui_kit::primary_button(ui, true, "+ Create Sample OpenAPI 3.1").clicked()
                {
                    add_sample_spec(app);
                }
                ui.add_space(16.0);
            });
        });
        return;
    }

    // 2. Spec Tabs with 1-Click `×` Remove Button
    let mut select_idx: Option<usize> = None;
    let mut delete_idx: Option<usize> = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (idx, doc) in app.specs.iter().enumerate() {
            let active = app.selected_spec_idx == idx;
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
                .corner_radius(5)
                .inner_margin(egui::Margin::symmetric(8, 3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let lbl = format!("{} ({})", doc.name, doc.operations.len());
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
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("×")
                                        .size(12.5)
                                        .color(swaggen_ui_kit::text_secondary(ui)),
                                )
                                .frame(false),
                            )
                            .on_hover_text("Delete this specification")
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
        app.spec_diffs.clear();
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
        app.status = "Deleted OpenAPI specification".into();
        if app.specs.is_empty() {
            return;
        }
    }

    let idx = app.selected_spec_idx.min(app.specs.len() - 1);
    if app.spec_editor_text.is_empty() && !app.specs[idx].raw_text.is_empty() {
        app.spec_editor_text = app.specs[idx].raw_text.clone();
    }

    ui.add_space(6.0);

    // 3. Friendly HTML Webpage Guard (fixes `image3.png` when user pastes MockAPI homepage HTML!)
    let trimmed_lower = app
        .spec_editor_text
        .trim_start()
        .chars()
        .take(48)
        .collect::<String>()
        .to_ascii_lowercase();
    if trimmed_lower.starts_with("<!doctype html") || trimmed_lower.starts_with("<html") {
        swaggen_ui_kit::notice_banner(
            ui,
            NoticeKind::Warning,
            "HTML Webpage Detected (Not an OpenAPI JSON/YAML file)",
            "The fetched URL returned a website HTML page instead of an OpenAPI 3.x specification. Enter a direct /openapi.json or /swagger.yaml URL, or replace this spec with a sample.",
        );
        ui.add_space(4.0);
        let mut deleted_broken_spec = false;
        ui.horizontal_wrapped(|ui| {
            if swaggen_ui_kit::primary_button(ui, true, "Replace with Sample OpenAPI 3.1").clicked()
            {
                app.spec_editor_text = SAMPLE_OPENAPI_YAML.into();
                let updated = parse_and_validate(
                    app.specs[idx].id.clone(),
                    "Sample Tasks API",
                    &app.spec_editor_text,
                    &[],
                );
                app.specs[idx] = updated;
                app.submit(Command::SaveSpecs {
                    specs: app.specs.clone(),
                });
            }
            if swaggen_ui_kit::danger_button(ui, true, "Delete this broken spec").clicked() {
                app.specs.remove(idx);
                app.selected_spec_idx =
                    app.selected_spec_idx.min(app.specs.len().saturating_sub(1));
                app.spec_editor_text = app
                    .specs
                    .get(app.selected_spec_idx)
                    .map(|s| s.raw_text.clone())
                    .unwrap_or_default();
                app.submit(Command::SaveSpecs {
                    specs: app.specs.clone(),
                });
                deleted_broken_spec = true;
            }
        });
        if deleted_broken_spec {
            return;
        }
        ui.add_space(6.0);
    }

    // 4. Clean 3-Mode View Switcher + Spec Name Editor
    ui.horizontal_wrapped(|ui| {
        for (m_idx, lbl) in [
            (0usize, "Swagger Docs Preview"),
            (1, "Split YAML/JSON Editor + Docs"),
            (2, "Linter, Drift & SDK Generator"),
        ] {
            if swaggen_ui_kit::underline_tab(ui, app.spec_view_mode == m_idx, lbl).clicked() {
                app.spec_view_mode = m_idx;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if swaggen_ui_kit::primary_button(ui, true, "Save Spec").clicked() {
                let old_ops = app.specs[idx].operations.clone();
                let updated = parse_and_validate(
                    app.specs[idx].id.clone(),
                    app.specs[idx].name.clone(),
                    &app.spec_editor_text,
                    &old_ops,
                );
                app.spec_diffs =
                    diff_and_sync_linked_requests(&old_ops, &updated, &mut app.state.requests);
                app.specs[idx] = updated;
                app.submit(Command::SaveSpecs {
                    specs: app.specs.clone(),
                });
                app.mark_dirty();
            }
            let mut spec_title = app.specs[idx].name.clone();
            if swaggen_ui_kit::text_input(ui, &mut spec_title, "Spec title", 160.0, false).changed()
                && !spec_title.trim().is_empty()
            {
                app.specs[idx].name = spec_title;
            }
        });
    });
    ui.add_space(8.0);

    let doc_clone = app.specs[idx].clone();
    match app.spec_view_mode {
        0 => {
            show_swagger_docs_pane(app, ui, &doc_clone);
        }
        1 => {
            if ui.available_width() >= 680.0 {
                ui.columns(2, |cols| {
                    show_spec_editor_pane(app, &mut cols[0], idx, &doc_clone);
                    show_swagger_docs_pane(app, &mut cols[1], &doc_clone);
                });
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("spec_stacked_narrow")
                    .show(ui, |ui| {
                        show_spec_editor_pane(app, ui, idx, &doc_clone);
                        ui.add_space(10.0);
                        show_swagger_docs_pane(app, ui, &doc_clone);
                    });
            }
        }
        _ => {
            show_advanced_spec_tools(app, ui, idx, &doc_clone);
        }
    }
}

fn add_sample_spec(app: &mut App) {
    let doc = parse_and_validate(
        swaggen_domain::workspace::new_uid(),
        format!("Sample API {}", app.specs.len() + 1),
        SAMPLE_OPENAPI_YAML,
        &[],
    );
    app.spec_editor_text = doc.raw_text.clone();
    app.specs.push(doc);
    app.selected_spec_idx = app.specs.len() - 1;
    app.spec_view_mode = 0;
    app.submit(Command::SaveSpecs {
        specs: app.specs.clone(),
    });
}

fn show_spec_editor_pane(app: &mut App, ui: &mut egui::Ui, _idx: usize, doc_clone: &SpecDocument) {
    swaggen_ui_kit::surface_card(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(swaggen_ui_kit::bold_text(
                if matches!(doc_clone.format, SpecFormat::Json) {
                    "openapi.json"
                } else {
                    "openapi.yaml"
                },
                13.5,
                swaggen_ui_kit::text_primary(ui),
            ));
            if ui.small_button("Format YAML").clicked()
                && let Ok(f) = format_spec_text(&app.spec_editor_text, SpecFormat::Yaml)
            {
                app.spec_editor_text = f;
            }
            if ui.small_button("Format JSON").clicked()
                && let Ok(f) = format_spec_text(&app.spec_editor_text, SpecFormat::Json)
            {
                app.spec_editor_text = f;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if doc_clone.is_strictly_valid() {
                    swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_SUCCESS, "Valid OpenAPI");
                } else {
                    swaggen_ui_kit::pill_badge(
                        ui,
                        Tokens::TEXT_WARNING,
                        &format!("{} issue(s)", doc_clone.issues.len()),
                    );
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.spec_find)
                    .hint_text("Find")
                    .desired_width(90.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.spec_replace)
                    .hint_text("Replace")
                    .desired_width(90.0),
            );
            if ui.small_button("Replace all").clicked() && !app.spec_find.is_empty() {
                let (out, count) =
                    find_replace_spec(&app.spec_editor_text, &app.spec_find, &app.spec_replace);
                app.spec_editor_text = out;
                app.status = format!("Replaced {count} occurrence(s)");
            }
        });
        ui.add_space(6.0);
        let editor_h = (ui.available_height() - 24.0).max(220.0);
        egui::ScrollArea::vertical()
            .id_salt("d01-spec-editor")
            .max_height(editor_h)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut app.spec_editor_text)
                        .desired_width(ui.available_width())
                        .desired_rows(20)
                        .code_editor(),
                );
                for issue in &doc_clone.issues {
                    let kind = match issue.severity {
                        IssueSeverity::Error => NoticeKind::Danger,
                        IssueSeverity::Warning | IssueSeverity::UnsupportedKeyword => {
                            NoticeKind::Warning
                        }
                    };
                    swaggen_ui_kit::notice_banner(ui, kind, &issue.pointer, &issue.message);
                }
            });
    });
}

fn show_swagger_docs_pane(app: &mut App, ui: &mut egui::Ui, doc_clone: &SpecDocument) {
    swaggen_ui_kit::surface_card(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(swaggen_ui_kit::bold_text(
                &doc_clone.name,
                16.0,
                swaggen_ui_kit::text_primary(ui),
            ));
            swaggen_ui_kit::pill_badge(
                ui,
                Tokens::STATUS_SUCCESS,
                &format!("{} endpoints", doc_clone.operations.len()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.spec_doc_search,
                    "Filter endpoints or schemas…",
                    210.0,
                    false,
                );
            });
        });
        ui.add_space(8.0);

        let (ops, schemas) = doc_clone.search(&app.spec_doc_search);
        let mut by_tag: BTreeMap<String, Vec<_>> = BTreeMap::new();
        for op in ops {
            let tag = op.tags.first().cloned().unwrap_or_else(|| "Default".into());
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
                        egui::RichText::new("No matching endpoints in this specification.")
                            .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                }
                for (tag, group_ops) in by_tag {
                    egui::CollapsingHeader::new(swaggen_ui_kit::bold_text(
                        &format!("{tag} ({})", group_ops.len()),
                        13.5,
                        swaggen_ui_kit::text_primary(ui),
                    ))
                    .default_open(true)
                    .show(ui, |ui| {
                        for op in group_ops {
                            egui::Frame::new()
                                .fill(swaggen_ui_kit::subtle_bg(ui))
                                .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                                .corner_radius(6)
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        swaggen_ui_kit::method_badge(ui, op.method.as_str());
                                        ui.label(swaggen_ui_kit::bold_text(
                                            &op.path,
                                            12.5,
                                            swaggen_ui_kit::text_primary(ui),
                                        ));
                                        if !op.summary.is_empty() {
                                            ui.label(
                                                egui::RichText::new(&op.summary)
                                                    .size(11.5)
                                                    .color(swaggen_ui_kit::text_secondary(ui)),
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
                                                    try_op_uid = Some(op.operation_uid.clone());
                                                }
                                            },
                                        );
                                    });
                                    if !op.parameters.is_empty() {
                                        ui.add_space(4.0);
                                        for p in &op.parameters {
                                            ui.monospace(
                                                egui::RichText::new(format!(
                                                    "  • {} ({}) : {}{}",
                                                    p.name,
                                                    p.location,
                                                    p.schema_type,
                                                    if p.required { " *required" } else { "" }
                                                ))
                                                .size(11.0),
                                            );
                                        }
                                    }
                                });
                            ui.add_space(6.0);
                        }
                    });
                    ui.add_space(4.0);
                }

                if !schemas.is_empty() {
                    ui.add_space(8.0);
                    egui::CollapsingHeader::new(swaggen_ui_kit::bold_text(
                        &format!("Data Models / Schemas ({})", schemas.len()),
                        13.0,
                        swaggen_ui_kit::text_primary(ui),
                    ))
                    .default_open(false)
                    .show(ui, |ui| {
                        for s in schemas {
                            ui.label(swaggen_ui_kit::bold_text(
                                &format!("{} ({})", s.name, s.schema_type),
                                12.0,
                                Tokens::STATUS_INFO,
                            ));
                            for p in &s.properties {
                                ui.monospace(format!(
                                    "  {}: {}{}",
                                    p.name,
                                    p.schema_type,
                                    if p.required { " [required]" } else { "" }
                                ));
                            }
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

fn show_advanced_spec_tools(
    app: &mut App,
    ui: &mut egui::Ui,
    idx: usize,
    doc_clone: &SpecDocument,
) {
    egui::ScrollArea::vertical()
        .id_salt("spec_adv_tools_scroll")
        .show(ui, |ui| {
            swaggen_ui_kit::surface_card(ui, |ui| {
                swaggen_ui_kit::card_header(ui, "Quick Add Endpoint to Spec");
                ui.horizontal_wrapped(|ui| {
                    let mut method = app.spec_new_method;
                    egui::ComboBox::from_id_salt("spec-new-method")
                        .selected_text(method.as_str())
                        .width(85.0)
                        .show_ui(ui, |ui| {
                            for m in HttpMethod::ALL {
                                ui.selectable_value(&mut method, m, m.as_str());
                            }
                        });
                    app.spec_new_method = method;
                    swaggen_ui_kit::text_input(
                        ui,
                        &mut app.spec_new_path,
                        "/items/{id}",
                        160.0,
                        false,
                    );
                    swaggen_ui_kit::text_input(
                        ui,
                        &mut app.spec_new_op_id,
                        "operationId",
                        140.0,
                        false,
                    );
                    if swaggen_ui_kit::primary_button(
                        ui,
                        !app.spec_new_path.trim().is_empty(),
                        "+ Add Endpoint",
                    )
                    .clicked()
                    {
                        match add_endpoint_to_spec(
                            &app.spec_editor_text,
                            app.specs[idx].format,
                            app.spec_new_path.trim(),
                            app.spec_new_method,
                            app.spec_new_op_id.trim(),
                            app.spec_new_op_id.trim(),
                        ) {
                            Ok(next_raw) => {
                                app.spec_editor_text = next_raw;
                                app.spec_view_mode = 1;
                                app.status = "Added endpoint to spec draft".into();
                            }
                            Err(msg) => {
                                app.error = true;
                                app.status = msg;
                            }
                        }
                    }
                });
            });
            ui.add_space(8.0);
            swaggen_ui_kit::surface_card(ui, |ui| {
                swaggen_ui_kit::card_header(
                    ui,
                    "Governance, Linter, Contract Drift & Client SDK Generator",
                );
                ui.horizontal_wrapped(|ui| {
                    if swaggen_ui_kit::secondary_button(ui, true, "Lint Conventions").clicked() {
                        app.spec_lint_issues =
                            lint_spec_conventions(doc_clone, &TeamLintRules::default());
                        app.status = format!(
                            "Linted spec: {} issue(s) · {}",
                            app.spec_lint_issues.len(),
                            assess_openapi_version(doc_clone)
                        );
                    }
                    if swaggen_ui_kit::secondary_button(ui, true, "Detect Contract Drift").clicked()
                    {
                        app.spec_drift_items =
                            detect_contract_drift(doc_clone, &app.state.requests);
                        if let Some(ep) = doc_clone.operations.first()
                            && let Some(req) = app.active_request()
                            && let Some(Ok(resp)) = app.responses.get(&req.id)
                        {
                            let ct = resp
                                .headers
                                .iter()
                                .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                                .map(|(_, v)| v.as_str())
                                .unwrap_or("application/json");
                            app.spec_contract_report = Some(validate_response_against_endpoint(
                                doc_clone,
                                ep,
                                resp.status_code,
                                ct,
                                &resp.preview_bytes,
                            ));
                        }
                        app.status = format!(
                            "Detected {} contract drift item(s)",
                            app.spec_drift_items.len()
                        );
                    }
                    if swaggen_ui_kit::secondary_button(ui, true, "Convert Swagger 2.0 -> 3.0.3")
                        .clicked()
                    {
                        match convert_swagger2_to_openapi3(&app.spec_editor_text) {
                            Ok((converted, rep)) => {
                                app.spec_editor_text = converted.raw_text.clone();
                                app.specs[idx] = converted;
                                app.status = format!(
                                    "Converted Swagger 2.0 -> OpenAPI 3.0.3 ({} notes)",
                                    rep.warnings.len()
                                );
                            }
                            Err(e) => {
                                app.error = true;
                                app.status = e;
                            }
                        }
                    }
                    if swaggen_ui_kit::secondary_button(
                        ui,
                        app.active_request().is_some(),
                        "Propose Patch from Active Request",
                    )
                    .clicked()
                        && let Some(req) = app.active_request()
                    {
                        let patch = propose_spec_patch_from_request(req);
                        app.status = format!(
                            "Patch {} {} — {}",
                            patch.method.as_str(),
                            patch.target_path,
                            patch.review_notice
                        );
                    }
                    if swaggen_ui_kit::secondary_button(ui, true, "Export HTML & Markdown Docs")
                        .clicked()
                    {
                        let md = export_docs_markdown(doc_clone, "v1");
                        let html = export_docs_html(doc_clone, "v1");
                        let docs_dir = app.root.join("docs-export");
                        let _ = std::fs::create_dir_all(&docs_dir);
                        let _ = std::fs::write(docs_dir.join("api.md"), md);
                        let _ = std::fs::write(docs_dir.join("api.html"), html);
                        app.status = format!("Exported sanitized docs to {}", docs_dir.display());
                    }
                    if swaggen_ui_kit::primary_button(ui, true, "Generate Rust & TypeScript SDK")
                        .clicked()
                    {
                        let rust_sdk = swaggen_runner::advanced_protocols::generate_sdk_stub(
                            doc_clone,
                            swaggen_runner::advanced_protocols::SdkLanguage::RustReqwest,
                        );
                        let ts_sdk = swaggen_runner::advanced_protocols::generate_sdk_stub(
                            doc_clone,
                            swaggen_runner::advanced_protocols::SdkLanguage::TypeScriptFetch,
                        );
                        let sdk_dir = app.root.join("sdk-stubs");
                        let _ = std::fs::create_dir_all(&sdk_dir);
                        let _ = std::fs::write(sdk_dir.join("client.rs"), rust_sdk);
                        let _ = std::fs::write(sdk_dir.join("client.ts"), ts_sdk);
                        app.status =
                            format!("Generated Rust & TS SDK stubs in {}", sdk_dir.display());
                    }
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Multi-file $ref folder:");
                    swaggen_ui_kit::text_input(
                        ui,
                        &mut app.spec_bundle_dir,
                        "C:\\path\\to\\openapi-folder",
                        240.0,
                        false,
                    );
                    if swaggen_ui_kit::secondary_button(ui, true, "Bundle $ref").clicked() {
                        let dir = std::path::PathBuf::from(app.spec_bundle_dir.trim());
                        let entry = dir.join("openapi.json");
                        match resolve_and_bundle_multifile(&entry, &dir, false) {
                            Ok((val, issues)) => {
                                app.spec_editor_text =
                                    serde_json::to_string_pretty(&val).unwrap_or_default();
                                app.spec_lint_issues = issues;
                                app.status = "Bundled multi-file OpenAPI $ref tree".into();
                            }
                            Err(e) => {
                                app.error = true;
                                app.status = e;
                            }
                        }
                    }
                    if swaggen_ui_kit::secondary_button(ui, true, "Inspect Schema Tree").clicked()
                        && let Ok(val) =
                            serde_json::from_str::<serde_json::Value>(&app.spec_editor_text)
                        && let Some((_, first_schema)) = val
                            .pointer("/components/schemas")
                            .and_then(|s| s.as_object())
                            .and_then(|m| m.iter().next())
                    {
                        app.spec_schema_tree =
                            Some(inspect_complex_schema(&val, first_schema, "RootSchema", 8));
                    }
                });

                for issue in &app.spec_lint_issues {
                    ui.monospace(format!(
                        "[{:?}] {} — {}",
                        issue.severity, issue.pointer, issue.message
                    ));
                }
                for item in &app.spec_drift_items {
                    ui.monospace(format!("Drift · {}: {}", item.request_name, item.issue));
                }
                if let Some(tree) = &app.spec_schema_tree {
                    ui.monospace(format!(
                        "Schema '{}': kind={}, children={}, cyclic={}",
                        tree.label,
                        tree.kind,
                        tree.children.len(),
                        tree.recursive_boundary
                    ));
                }
            });
        });
}
