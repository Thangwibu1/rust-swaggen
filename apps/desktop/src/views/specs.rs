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
    swaggen_ui_kit::section(
        ui,
        "OpenAPI 3.0 / 3.1 Specifications & Docs (OA01–OA06 / DC01–DC06)",
        "Edit JSON/YAML, validate without silent passes on unsupported keywords, browse native docs, generate linked requests, and sync spec diffs.",
    );

    ui.horizontal_wrapped(|ui| {
        for (idx, doc) in app.specs.iter().enumerate() {
            let label = format!("{} ({})", doc.name, doc.operations.len());
            if ui
                .selectable_label(app.selected_spec_idx == idx, label)
                .clicked()
            {
                app.selected_spec_idx = idx;
                app.spec_editor_text = doc.raw_text.clone();
                app.spec_diffs.clear();
            }
        }
        if swaggen_ui_kit::secondary_button(ui, true, "+ New OpenAPI 3.1").clicked() {
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
    });

    ui.horizontal_wrapped(|ui| {
        ui.label("Import from URL (OA01):");
        ui.add(
            egui::TextEdit::singleline(&mut app.spec_import_url)
                .hint_text("https://api.example.test/openapi.json")
                .desired_width(260.0),
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
    ui.separator();

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

    // Quick Add Endpoint (OA04).
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
        if swaggen_ui_kit::secondary_button(ui, !app.spec_new_path.trim().is_empty(), "+ Endpoint")
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

    // Version B Spec tools (OA07-OA10, DC07-DC09).
    ui.horizontal_wrapped(|ui| {
        if swaggen_ui_kit::secondary_button(ui, true, "Convert Swagger 2.0 -> 3.0.3 (OA09)")
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
        if swaggen_ui_kit::secondary_button(ui, true, "Lint Conventions (OA10)").clicked() {
            app.spec_lint_issues = lint_spec_conventions(&app.specs[idx], &TeamLintRules::default());
            app.status = format!(
                "Linted spec: {} issue(s) · {}",
                app.spec_lint_issues.len(),
                assess_openapi_version(&app.specs[idx])
            );
        }
        if swaggen_ui_kit::secondary_button(ui, true, "Detect Contract Drift (DC07)").clicked() {
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
            app.status = format!("Detected {} contract drift item(s)", app.spec_drift_items.len());
        }
        if swaggen_ui_kit::secondary_button(ui, app.active_request().is_some(), "Propose Patch (DC09)")
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
        if swaggen_ui_kit::secondary_button(ui, true, "Export HTML & MD Docs (DC08)").clicked() {
            let md = export_docs_markdown(&app.specs[idx], "v1");
            let html = export_docs_html(&app.specs[idx], "v1");
            let docs_dir = app.root.join("docs-export");
            let _ = std::fs::create_dir_all(&docs_dir);
            let _ = std::fs::write(docs_dir.join("api.md"), md);
            let _ = std::fs::write(docs_dir.join("api.html"), html);
            app.status = format!("Exported sanitized docs to {}", docs_dir.display());
        }
        if swaggen_ui_kit::secondary_button(ui, true, "Generate Rust & TS SDK Stubs (PR08)").clicked() {
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
            app.status = format!("Generated pinned Rust & TS SDK stubs in {}", sdk_dir.display());
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Multi-file $ref entry dir (OA08):");
        ui.add(egui::TextEdit::singleline(&mut app.spec_bundle_dir).desired_width(240.0));
        if swaggen_ui_kit::secondary_button(ui, true, "Bundle folder $ref").clicked() {
            let dir = std::path::PathBuf::from(app.spec_bundle_dir.trim());
            let entry = dir.join("openapi.json");
            match resolve_and_bundle_multifile(&entry, &dir, false) {
                Ok((val, issues)) => {
                    app.spec_editor_text = serde_json::to_string_pretty(&val).unwrap_or_default();
                    app.spec_lint_issues = issues;
                    app.status = "Bundled multi-file OpenAPI $ref tree".into();
                }
                Err(e) => {
                    app.error = true;
                    app.status = e;
                }
            }
        }
        if swaggen_ui_kit::secondary_button(ui, true, "Inspect Schema Tree (OA07)").clicked()
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

    let doc = &app.specs[idx];
    if doc.is_strictly_valid() {
        swaggen_ui_kit::status_badge(ui, NoticeKind::Success, "Valid OpenAPI specification");
    } else {
        swaggen_ui_kit::status_badge(
            ui,
            NoticeKind::Warning,
            &format!("{} validation / unsupported item(s)", doc.issues.len()),
        );
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add(
            egui::TextEdit::multiline(&mut app.spec_editor_text)
                .desired_width(ui.available_width())
                .desired_rows(10)
                .code_editor(),
        );
        for issue in &app.specs[idx].issues {
            let kind = match issue.severity {
                IssueSeverity::Error => NoticeKind::Danger,
                IssueSeverity::Warning | IssueSeverity::UnsupportedKeyword => NoticeKind::Warning,
            };
            swaggen_ui_kit::notice_banner(ui, kind, &issue.pointer, &issue.message);
        }

        if !app.spec_diffs.is_empty() {
            ui.add_space(6.0);
            ui.heading("Contract diff & linked request sync (DC06)");
            for d in &app.spec_diffs {
                ui.monospace(format!("[{}] {} — {}", d.kind, d.summary, d.detail));
            }
        }
        if !app.spec_lint_issues.is_empty() {
            ui.add_space(6.0);
            ui.heading("OpenAPI lint & bundle issues (OA08 / OA10)");
            for issue in &app.spec_lint_issues {
                ui.monospace(format!("[{:?}] {} — {}", issue.severity, issue.pointer, issue.message));
            }
        }
        if !app.spec_drift_items.is_empty() {
            ui.add_space(6.0);
            ui.heading("Contract drift items (DC07)");
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
                "Complex Schema '{}': kind={}, children={}, cyclic={}",
                tree.label,
                tree.kind,
                tree.children.len(),
                tree.recursive_boundary
            ));
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading("Native API Documentation (DC01–DC03)");
            ui.add(
                egui::TextEdit::singleline(&mut app.spec_doc_search)
                    .hint_text("Search endpoint or schema…")
                    .desired_width(180.0),
            );
        });
        let doc_clone = app.specs[idx].clone();
        let (ops, schemas) = doc_clone.search(&app.spec_doc_search);
        for op in ops {
            swaggen_ui_kit::surface_card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    swaggen_ui_kit::method_badge(ui, op.method.as_str());
                    ui.label(egui::RichText::new(&op.path).monospace().strong());
                    ui.label(&op.summary);
                    if op.deprecated {
                        swaggen_ui_kit::status_badge(ui, NoticeKind::Warning, "Deprecated");
                    }
                    if swaggen_ui_kit::primary_button(ui, true, "Try / Generate linked request")
                        .clicked()
                    {
                        let col_id = app
                            .state
                            .collections
                            .first()
                            .map(|c| c.id.clone())
                            .unwrap_or_default();
                        if !col_id.is_empty() {
                            let req = generate_linked_request(&doc_clone, op, &col_id);
                            let rid = req.id.clone();
                            app.state.requests.push(req);
                            app.state.open_or_focus_tab(&rid);
                            app.mode = MainMode::Workbench;
                            app.mark_dirty();
                        }
                    }
                });
                for p in &op.parameters {
                    ui.monospace(format!(
                        "  param {} ({}) : {}{}{}",
                        p.name,
                        p.location,
                        p.schema_type,
                        if p.required { " required" } else { "" },
                        if p.nullable { " nullable" } else { "" }
                    ));
                }
            });
            ui.add_space(4.0);
        }
        for s in schemas {
            swaggen_ui_kit::surface_card(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!("Schema: {} ({})", s.name, s.schema_type)).strong(),
                );
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
            ui.add_space(4.0);
        }
    });
}
