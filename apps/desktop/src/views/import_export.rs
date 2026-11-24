//! Import/Export Hub: cURL bidirectional, Postman Collection v2.1 & Environment,
//! Open Project bundle, and sanitized preview (IM01-IM06, IM08).
use crate::app::{App, MainMode};
use eframe::egui;
use swaggen_domain::security::RedactionPolicy;
use swaggen_spec_engine::advanced::{bundle_shared_attachments, export_git_project_tree};
use swaggen_spec_engine::importers::{
    export_open_project_bundle, export_openapi_roundtrip, import_postman_collection_v2_1,
    import_postman_environment, parse_curl_command, preview_request_import, to_curl_command,
};
use swaggen_spec_engine::openapi::SpecFormat;
use swaggen_ui_kit::NoticeKind;

pub fn show(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    swaggen_ui_kit::page_header(
        ui,
        "Import APIs & collections",
        "Bidirectional cURL (without shell execution), Postman v2.1, Postman Environment, and sanitized Open Project bundles.",
        None,
        false,
    );

    egui::ScrollArea::vertical().show(ui, |ui| {
        // 1. cURL bidirectional (IM05).
        swaggen_ui_kit::surface_card(ui, |ui| {
            swaggen_ui_kit::card_header(ui, "cURL bidirectional (pure-Rust parser)");
            ui.add(
                egui::TextEdit::multiline(&mut app.curl_input)
                    .desired_width(ui.available_width())
                    .desired_rows(3)
                    .code_editor(),
            );
            ui.horizontal_wrapped(|ui| {
                if swaggen_ui_kit::primary_button(ui, true, "Import cURL as request").clicked() {
                    let col_id = app
                        .state
                        .collections
                        .first()
                        .map(|c| c.id.clone())
                        .unwrap_or_default();
                    if !col_id.is_empty() {
                        match parse_curl_command(&app.curl_input, &col_id) {
                            Ok(req) => {
                                let rid = req.id.clone();
                                app.state.requests.push(req);
                                app.state.open_or_focus_tab(&rid);
                                app.mode = MainMode::Workbench;
                                app.mark_dirty();
                                app.status = "Imported cURL command safely".into();
                            }
                            Err(msg) => {
                                app.error = true;
                                app.status = msg;
                            }
                        }
                    }
                }
                if swaggen_ui_kit::secondary_button(
                    ui,
                    app.active_request().is_some(),
                    "Copy active request as masked cURL",
                )
                .clicked()
                    && let Some(req) = app.active_request()
                {
                    let cmd = to_curl_command(req, true);
                    app.curl_input = cmd.clone();
                    ctx.copy_text(cmd);
                    app.status = "Generated masked cURL command".into();
                }
            });
        });
        ui.add_space(8.0);

        // 2. Postman Collection v2.1 & Environment (IM02, IM03, IM06).
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.heading("2. Postman Collection v2.1 & Environment (IM02 / IM03 / IM06)");
            ui.add(
                egui::TextEdit::multiline(&mut app.postman_input)
                    .desired_width(ui.available_width())
                    .desired_rows(5)
                    .code_editor(),
            );
            ui.horizontal_wrapped(|ui| {
                if swaggen_ui_kit::primary_button(ui, true, "Import Postman Collection v2.1")
                    .clicked()
                {
                    match import_postman_collection_v2_1(&app.postman_input) {
                        Ok((col, folders, reqs, report)) => {
                            let preview = preview_request_import(&app.state, &reqs, &report);
                            app.last_import_summary = format!(
                                "Collection '{}': {} new, {} duplicate(s), {} folder(s), {} unsupported note(s)",
                                col.name,
                                preview.new_requests,
                                preview.duplicate_requests,
                                folders.len(),
                                preview.unsupported_items.len()
                            );
                            app.state.collections.push(col);
                            app.state.folders.extend(folders);
                            app.state.requests.extend(reqs);
                            app.mark_dirty();
                        }
                        Err(msg) => {
                            app.error = true;
                            app.status = msg;
                        }
                    }
                }
                if swaggen_ui_kit::secondary_button(ui, true, "Import Postman Environment")
                    .clicked()
                {
                    match import_postman_environment(&app.postman_input) {
                        Ok((env, lines)) => {
                            app.last_import_summary = format!(
                                "Imported Environment '{}' ({} vars):\n{}",
                                env.name,
                                env.variables.len(),
                                lines.join("\n")
                            );
                            app.state.environments.push(env);
                            app.mark_dirty();
                        }
                        Err(msg) => {
                            app.error = true;
                            app.status = msg;
                        }
                    }
                }
                if swaggen_ui_kit::secondary_button(ui, true, "Import HAR 1.2 (IM10)").clicked() {
                    let cid = app
                        .state
                        .collections
                        .first()
                        .map(|c| c.id.clone())
                        .unwrap_or_default();
                    match swaggen_spec_engine::har_http::import_har(&app.postman_input, &cid) {
                        Ok((reqs, rep)) => {
                            app.last_import_summary = format!(
                                "HAR imported {} reqs; warnings={:?}",
                                reqs.len(),
                                rep.warnings
                            );
                            app.state.requests.extend(reqs);
                            app.mark_dirty();
                        }
                        Err(e) => {
                            app.error = true;
                            app.status = e;
                        }
                    }
                }
                if swaggen_ui_kit::secondary_button(ui, true, "Import .http file (IM10)").clicked() {
                    let cid = app
                        .state
                        .collections
                        .first()
                        .map(|c| c.id.clone())
                        .unwrap_or_default();
                    match swaggen_spec_engine::har_http::import_dot_http(&app.postman_input, &cid) {
                        Ok((reqs, vars, rep)) => {
                            app.last_import_summary = format!(
                                ".http imported {} reqs, {} vars; warnings={:?}",
                                reqs.len(),
                                vars.len(),
                                rep.warnings
                            );
                            app.state.requests.extend(reqs);
                            app.mark_dirty();
                        }
                        Err(e) => {
                            app.error = true;
                            app.status = e;
                        }
                    }
                }
            });
            if !app.last_import_summary.is_empty() {
                swaggen_ui_kit::notice_banner(
                    ui,
                    NoticeKind::Info,
                    "Import compatibility report",
                    &app.last_import_summary,
                );
            }
        });
        ui.add_space(8.0);

        // 3. Sanitized Open Project bundle preview (IM01, IM04, IM08).
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.heading("3. Sanitized Open Project & OpenAPI Export Preview (IM01 / IM04 / IM08)");
            if let Ok(bundle) =
                export_open_project_bundle(&app.state, &app.specs, &RedactionPolicy::default())
            {
                ui.label(format!(
                    "Schema: {} · Files: {} · Excludes secrets & history: {}",
                    bundle.manifest.schema_version,
                    bundle.manifest.files.join(", "),
                    bundle.manifest.excludes_secrets_and_history
                ));
                if let Some(first_spec) = app.specs.first()
                    && let Ok((yaml, notes)) = export_openapi_roundtrip(first_spec, SpecFormat::Yaml)
                {
                    if !notes.is_empty() {
                        ui.label(notes.join("; "));
                    }
                    swaggen_ui_kit::code_surface(
                        ui,
                        &yaml.chars().take(600).collect::<String>(),
                    );
                }
            }
        });
        ui.add_space(8.0);

        // 4. Shared attachment bundler & deterministic Git export (IM07, IM09).
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.heading("4. Shared Attachments (IM07) & Git Project Export (IM09)");
            ui.horizontal(|ui| {
                ui.label("Files (CSV):");
                ui.add(
                    egui::TextEdit::singleline(&mut app.attach_files_csv)
                        .hint_text("C:\\data\\payload.json")
                        .desired_width(220.0),
                );
                ui.label("Bundle dir:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.attach_shared_dir).desired_width(200.0),
                );
                if swaggen_ui_kit::secondary_button(
                    ui,
                    !app.attach_files_csv.trim().is_empty(),
                    "Bundle attachments",
                )
                .clicked()
                {
                    let files: Vec<std::path::PathBuf> = app
                        .attach_files_csv
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(std::path::PathBuf::from)
                        .collect();
                    match bundle_shared_attachments(
                        &files,
                        std::path::Path::new(app.attach_shared_dir.trim()),
                        5 * 1024 * 1024,
                    ) {
                        Ok(refs) => {
                            app.p07_export_preview = serde_json::to_string_pretty(&refs).unwrap_or_default();
                            app.status = format!("Bundled {} shared attachment(s)", refs.len());
                        }
                        Err(e) => {
                            app.error = true;
                            app.status = e;
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Git tree export dir:");
                ui.add(egui::TextEdit::singleline(&mut app.git_export_dir).desired_width(280.0));
                if swaggen_ui_kit::primary_button(ui, true, "Export Git-friendly project tree")
                    .clicked()
                {
                    match export_git_project_tree(
                        &app.state,
                        &app.specs,
                        std::path::Path::new(app.git_export_dir.trim()),
                    ) {
                        Ok(files) => {
                            app.p07_export_preview = files
                                .iter()
                                .map(|p| p.display().to_string())
                                .collect::<Vec<_>>()
                                .join("\n");
                            app.status = format!(
                                "Exported {} deterministic files for Git review",
                                files.len()
                            );
                        }
                        Err(e) => {
                            app.error = true;
                            app.status = e;
                        }
                    }
                }
            });
            if !app.p07_export_preview.is_empty() {
                swaggen_ui_kit::code_surface(ui, &app.p07_export_preview);
            }
        });
    });
}
