//! Native OpenAPI 3.0/3.1 editor, structural validator, API documentation viewer,
//! Try-it / linked-request generator, and spec diff sync (OA01-OA06, DC01-DC06).
use crate::{
    app::{App, MainMode},
    worker::Command,
};
use eframe::egui;
use swaggen_domain::workspace::HttpMethod;
use swaggen_spec_engine::advanced::{
    TeamLintRules, assess_openapi_version, convert_swagger2_to_openapi3, detect_contract_drift,
    export_docs_html, export_docs_markdown, inspect_complex_schema, lint_spec_conventions,
    propose_spec_patch_from_request, resolve_and_bundle_multifile,
    validate_response_against_endpoint,
};
use swaggen_spec_engine::openapi::{
    IssueSeverity, SpecFormat, add_endpoint_to_spec, diff_and_sync_linked_requests,
    find_replace_spec, format_spec_text, generate_linked_request, parse_and_validate,
};
use swaggen_ui_kit::NoticeKind;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    if swaggen_ui_kit::page_header(
        ui,
        "API definitions & OpenAPI",
        "Edit OpenAPI 3.0/3.1 JSON or YAML, browse live documentation, generate linked requests, and review spec diffs.",
        Some("+ New OpenAPI 3.1"),
        true,
    ) {
        let sample = "openapi: 3.1.0\ninfo:\n  title: DisNote API\n  version: 1.0.0\nservers:\n  - url: https://api.example.test\npaths:\n  /tasks:\n    get:\n      operationId: listTasks\n      summary: List tasks\n      tags:\n        - Tasks\n      responses:\n        \"200\":\n          description: Task list\n";
        let doc = parse_and_validate(
            swaggen_domain::workspace::new_uid(),
            format!("Spec {}", app.specs.len() + 1),
            sample,
            &[],
        );
        app.spec_editor_text = doc.raw_text.clone();
        app.specs.push(doc);
        app.selected_spec_idx = app.specs.len() - 1;
        app.submit(Command::SaveSpecs {
            specs: app.specs.clone(),
        });
    }

    ui.horizontal_wrapped(|ui| {
        for (idx, doc) in app.specs.iter().enumerate() {
            let label = format!("{} ({})", doc.name, doc.operations.len());
            if swaggen_ui_kit::underline_tab(ui, app.selected_spec_idx == idx, &label).clicked() {
                app.selected_spec_idx = idx;
                app.spec_editor_text = doc.raw_text.clone();
                app.spec_diffs.clear();
            }
        }
    });
    ui.add_space(6.0);

    ui.horizontal_wrapped(|ui| {
        ui.label("Import from URL");
        ui.add(
            egui::TextEdit::singleline(&mut app.spec_import_url)
                .hint_text("https://api.example.test/openapi.json")
                .desired_width(300.0),
        );
        if swaggen_ui_kit::secondary_button(ui, !app.spec_import_url.trim().is_empty(), "Fetch URL")
            .clicked()
        {
            app.submit(Command::FetchSpecUrl {
                name: "Imported URL Spec".into(),
                url: app.spec_import_url.trim().to_owned(),
            });
        }
    });
    ui.add_space(8.0);

    if app.specs.is_empty() {
        ui.label("No OpenAPI specifications yet. Click '+ New OpenAPI 3.1' or fetch from a URL.");
        return;
    }

    let idx = app.selected_spec_idx.min(app.specs.len() - 1);
    if app.spec_editor_text.is_empty() && !app.specs[idx].raw_text.is_empty() {
        app.spec_editor_text = app.specs[idx].raw_text.clone();
    }

    ui.horizontal_wrapped(|ui| {
        if swaggen_ui_kit::primary_button(ui, true, "Validate & Save Spec").clicked() {
            let old_ops = app.specs[idx].operations.clone();
            let updated = parse_and_validate(
                app.specs[idx].id.clone(),
                app.specs[idx].name.clone(),
                &app.spec_editor_text,
                &old_ops,
            );
            let diffs = diff_and_sync_linked_requests(&old_ops, &updated, &mut app.state.requests);
            app.spec_diffs = diffs;
            app.specs[idx] = updated;
            app.submit(Command::SaveSpecs {
                specs: app.specs.clone(),
            });
            app.mark_dirty();
        }
        if swaggen_ui_kit::secondary_button(ui, true, "Format YAML").clicked()
            && let Ok(formatted) = format_spec_text(&app.spec_editor_text, SpecFormat::Yaml)
        {
            app.spec_editor_text = formatted;
        }
        if swaggen_ui_kit::secondary_button(ui, true, "Format JSON").clicked()
            && let Ok(formatted) = format_spec_text(&app.spec_editor_text, SpecFormat::Json)
        {
            app.spec_editor_text = formatted;
        }
        ui.separator();
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
        if swaggen_ui_kit::secondary_button(ui, !app.spec_find.is_empty(), "Replace all").clicked()
        {
            let (out, count) =
                find_replace_spec(&app.spec_editor_text, &app.spec_find, &app.spec_replace);
            app.spec_editor_text = out;
            app.status = format!("Replaced {count} occurrence(s) in spec draft");
        }
    });

    // Quick Add & Advanced spec tools tucked into a clean collapsible toolbar so D01 stays airy.
    egui::CollapsingHeader::new(
        egui::RichText::new("Endpoint generator, OpenAPI linter, drift check & SDK export")
            .size(12.0)
            .color(swaggen_ui_kit::text_secondary(ui)),
    )
    .default_open(false)
    .show(ui, |ui| {
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
            ui.add(
                egui::TextEdit::singleline(&mut app.spec_new_path)
                    .hint_text("/items/{id}")
                    .desired_width(140.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.spec_new_op_id)
                    .hint_text("operationId")
                    .desired_width(120.0),
            );
            if swaggen_ui_kit::secondary_button(
                ui,
                !app.spec_new_path.trim().is_empty(),
                "+ Endpoint",
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
                        app.status = "Added endpoint to spec draft".into();
                    }
                    Err(msg) => {
                        app.error = true;
                        app.status = msg;
                    }
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            if swaggen_ui_kit::secondary_button(ui, true, "Convert Swagger 2.0 -> 3.0.3").clicked()
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
            if swaggen_ui_kit::secondary_button(ui, true, "Lint Conventions").clicked() {
                app.spec_lint_issues =
                    lint_spec_conventions(&app.specs[idx], &TeamLintRules::default());
                app.status = format!(
                    "Linted spec: {} issue(s) · {}",
                    app.spec_lint_issues.len(),
                    assess_openapi_version(&app.specs[idx])
                );
            }
            if swaggen_ui_kit::secondary_button(ui, true, "Detect Drift").clicked() {
                app.spec_drift_items = detect_contract_drift(&app.specs[idx], &app.state.requests);
                if let Some(ep) = app.specs[idx].operations.first()
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
                        &app.specs[idx],
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
            if swaggen_ui_kit::secondary_button(ui, app.active_request().is_some(), "Propose Patch")
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
            if swaggen_ui_kit::secondary_button(ui, true, "Export HTML & MD Docs").clicked() {
                let md = export_docs_markdown(&app.specs[idx], "v1");
                let html = export_docs_html(&app.specs[idx], "v1");
                let docs_dir = app.root.join("docs-export");
                let _ = std::fs::create_dir_all(&docs_dir);
                let _ = std::fs::write(docs_dir.join("api.md"), md);
                let _ = std::fs::write(docs_dir.join("api.html"), html);
                app.status = format!("Exported sanitized docs to {}", docs_dir.display());
            }
            if swaggen_ui_kit::secondary_button(ui, true, "Generate Rust & TS SDK").clicked() {
                let rust_sdk = swaggen_runner::advanced_protocols::generate_sdk_stub(
                    &app.specs[idx],
                    swaggen_runner::advanced_protocols::SdkLanguage::RustReqwest,
                );
                let ts_sdk = swaggen_runner::advanced_protocols::generate_sdk_stub(
                    &app.specs[idx],
                    swaggen_runner::advanced_protocols::SdkLanguage::TypeScriptFetch,
                );
                let sdk_dir = app.root.join("sdk-stubs");
                let _ = std::fs::create_dir_all(&sdk_dir);
                let _ = std::fs::write(sdk_dir.join("client.rs"), rust_sdk);
                let _ = std::fs::write(sdk_dir.join("client.ts"), ts_sdk);
                app.status = format!(
                    "Generated pinned Rust & TS SDK stubs in {}",
                    sdk_dir.display()
                );
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Multi-file $ref entry dir:");
            ui.add(egui::TextEdit::singleline(&mut app.spec_bundle_dir).desired_width(240.0));
            if swaggen_ui_kit::secondary_button(ui, true, "Bundle folder $ref").clicked() {
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
                && let Ok(val) = serde_json::from_str::<serde_json::Value>(&app.spec_editor_text)
                && let Some((_, first_schema)) = val
                    .pointer("/components/schemas")
                    .and_then(|s| s.as_object())
                    .and_then(|m| m.iter().next())
            {
                app.spec_schema_tree =
                    Some(inspect_complex_schema(&val, first_schema, "RootSchema", 8));
            }
        });
    });

    ui.add_space(10.0);

    // Figma D01 / DisNote OpenAPI 2-column split: OpenAPI YAML/JSON editor on left, Live documentation on right.
    let doc_clone = app.specs[idx].clone();
    let is_valid = doc_clone.is_strictly_valid();
    let issue_count = doc_clone.issues.len();

    ui.columns(2, |cols| {
        let col_h = cols[0].available_height().max(260.0);
        // Left Column: openapi.yaml editor card
        swaggen_ui_kit::surface_card(&mut cols[0], |ui| {
            ui.horizontal(|ui| {
                ui.label(swaggen_ui_kit::bold_text(
                    if matches!(doc_clone.format, SpecFormat::Json) {
                        "openapi.json"
                    } else {
                        "openapi.yaml"
                    },
                    14.0,
                    swaggen_ui_kit::text_primary(ui),
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if is_valid {
                        swaggen_ui_kit::pill_badge(
                            ui,
                            swaggen_ui_kit::Tokens::STATUS_SUCCESS,
                            "Valid OpenAPI",
                        );
                    } else {
                        swaggen_ui_kit::pill_badge(
                            ui,
                            swaggen_ui_kit::Tokens::TEXT_WARNING,
                            &format!("{issue_count} issue(s)"),
                        );
                    }
                });
            });
            ui.add_space(8.0);
            let editor_h = (col_h - 68.0).max(200.0);
            egui::ScrollArea::vertical()
                .id_salt("d01-spec-editor")
                .max_height(editor_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut app.spec_editor_text)
                            .desired_width(ui.available_width())
                            .desired_rows(22)
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
                    if !app.spec_diffs.is_empty() {
                        ui.add_space(6.0);
                        swaggen_ui_kit::card_header(ui, "Contract diff & linked request sync");
                        for d in &app.spec_diffs {
                            ui.monospace(format!("[{}] {} — {}", d.kind, d.summary, d.detail));
                        }
                    }
                    if !app.spec_lint_issues.is_empty() {
                        ui.add_space(6.0);
                        swaggen_ui_kit::card_header(ui, "OpenAPI lint issues");
                        for issue in &app.spec_lint_issues {
                            ui.monospace(format!(
                                "[{:?}] {} — {}",
                                issue.severity, issue.pointer, issue.message
                            ));
                        }
                    }
                    if !app.spec_drift_items.is_empty() {
                        ui.add_space(6.0);
                        swaggen_ui_kit::card_header(ui, "Contract drift");
                        for item in &app.spec_drift_items {
                            ui.monospace(format!("{}: {}", item.request_name, item.issue));
                        }
                    }
                    if let Some(rep) = &app.spec_contract_report {
                        ui.monospace(format!(
                            "Contract validation matched={}: {:?}",
                            rep.matched, rep.mismatches
                        ));
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

        // Right Column: Figma D01 Live documentation card
        swaggen_ui_kit::surface_card(&mut cols[1], |ui| {
            ui.horizontal(|ui| {
                ui.label(swaggen_ui_kit::bold_text(
                    "Live documentation",
                    14.0,
                    swaggen_ui_kit::text_primary(ui),
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    swaggen_ui_kit::pill_badge(
                        ui,
                        swaggen_ui_kit::Tokens::STATUS_SUCCESS,
                        "OpenAPI 3.1",
                    );
                });
            });
            ui.add_space(6.0);
            ui.label(swaggen_ui_kit::bold_text(
                &doc_clone.name,
                18.0,
                swaggen_ui_kit::text_primary(ui),
            ));
            ui.add_space(8.0);
            swaggen_ui_kit::text_input(
                ui,
                &mut app.spec_doc_search,
                "Search endpoints or schemas…",
                ui.available_width(),
                false,
            );
            ui.add_space(10.0);

            let (ops, schemas) = doc_clone.search(&app.spec_doc_search);
            let docs_h = ui.available_height().max(200.0);
            egui::ScrollArea::vertical()
                .id_salt("d01-live-docs")
                .max_height(docs_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for op in ops {
                        swaggen_ui_kit::surface_card(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                swaggen_ui_kit::method_badge(ui, op.method.as_str());
                                ui.label(swaggen_ui_kit::bold_text(
                                    &op.path,
                                    13.0,
                                    swaggen_ui_kit::text_primary(ui),
                                ));
                                if op.deprecated {
                                    swaggen_ui_kit::pill_badge(
                                        ui,
                                        swaggen_ui_kit::Tokens::TEXT_WARNING,
                                        "Deprecated",
                                    );
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if swaggen_ui_kit::primary_button(ui, true, "Try it")
                                            .clicked()
                                        {
                                            let col_id = app
                                                .state
                                                .collections
                                                .first()
                                                .map(|c| c.id.clone())
                                                .unwrap_or_default();
                                            if !col_id.is_empty() {
                                                let req = generate_linked_request(
                                                    &doc_clone, op, &col_id,
                                                );
                                                let rid = req.id.clone();
                                                app.state.requests.push(req);
                                                app.state.open_or_focus_tab(&rid);
                                                app.mode = MainMode::Workbench;
                                                app.mark_dirty();
                                            }
                                        }
                                    },
                                );
                            });
                            if !op.summary.is_empty() {
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(&op.summary)
                                        .size(12.0)
                                        .color(swaggen_ui_kit::text_secondary(ui)),
                                );
                            }
                            if !op.parameters.is_empty() {
                                ui.add_space(6.0);
                                for p in &op.parameters {
                                    ui.monospace(format!(
                                        "  {} ({}) : {}{}{}",
                                        p.name,
                                        p.location,
                                        p.schema_type,
                                        if p.required { " required" } else { "" },
                                        if p.nullable { " nullable" } else { "" }
                                    ));
                                }
                            }
                        });
                        ui.add_space(8.0);
                    }
                    for s in schemas {
                        swaggen_ui_kit::surface_card(ui, |ui| {
                            ui.label(swaggen_ui_kit::bold_text(
                                &format!("Schema: {} ({})", s.name, s.schema_type),
                                13.0,
                                swaggen_ui_kit::text_primary(ui),
                            ));
                            for p in &s.properties {
                                ui.monospace(format!(
                                    "  {}: {}{}{}",
                                    p.name,
                                    p.schema_type,
                                    if p.required { " [required]" } else { "" },
                                    if p.nullable { " [nullable]" } else { "" }
                                ));
                            }
                        });
                        ui.add_space(8.0);
                    }
                });
        });
    });
}
