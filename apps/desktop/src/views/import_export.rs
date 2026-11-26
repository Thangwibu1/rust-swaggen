//! Clean 2-Tab Import & Export Hub (F10 — fixes `image6.png` clutter):
//! - Tab 0 (`Import`): Segmented source picker (`cURL` | `Postman v2.1 & Env` | `OpenAPI` | `HAR & .http`),
//!   local file loader, Import Preview (`IM06`), and safe pure-Rust import without shell execution.
//! - Tab 1 (`Export & Git Sync`): Full-width clean cards for Masked cURL, Sanitized Workspace JSON,
//!   Git-Friendly Directory Tree, and Shared Attachment Bundler (no truncated paths or raw HTML dumps!).
use crate::{
    app::{App, MainMode},
    worker::Command,
};
use eframe::egui;
use std::path::PathBuf;
use swaggen_domain::security::RedactionPolicy;
use swaggen_spec_engine::advanced::{bundle_shared_attachments, export_git_project_tree};
use swaggen_spec_engine::importers::{
    export_open_project_bundle, import_postman_collection_v2_1, import_postman_environment,
    parse_curl_command, preview_request_import, to_curl_command,
};
use swaggen_spec_engine::openapi::parse_api_documentation;
use swaggen_ui_kit::{NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    swaggen_ui_kit::page_header(
        ui,
        swaggen_ui_kit::tr(
            app.locale,
            "Import & Export Hub",
            "Nhập & Xuất Dữ Liệu (Import / Export)",
        ),
        "Import cURL, Postman v2.1, OpenAPI, HAR, or .http safely offline — or export sanitized JSON & Git-friendly project trees.",
        None,
        false,
    );

    ui.horizontal_wrapped(|ui| {
        for (idx, lbl) in [
            (0usize, "Import APIs & Collections"),
            (1, "Export, Git Sync & Bundles"),
        ] {
            if swaggen_ui_kit::underline_tab(ui, app.import_hub_tab == idx, lbl).clicked() {
                app.import_hub_tab = idx;
            }
        }
    });
    ui.add_space(10.0);

    egui::ScrollArea::vertical()
        .id_salt("import_export_hub_scroll")
        .show(ui, |ui| match app.import_hub_tab {
            0 => show_import_tab(app, ui, ctx),
            _ => show_export_tab(app, ui, ctx),
        });
}

fn show_import_tab(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    // Drag & Drop `.json` / `.csv` Dropzone Card
    let is_hovering_file = ctx.input(|i| !i.raw.hovered_files.is_empty());
    egui::Frame::new()
        .fill(if is_hovering_file {
            swaggen_ui_kit::selected_bg(ui)
        } else {
            swaggen_ui_kit::subtle_bg(ui)
        })
        .stroke(egui::Stroke::new(
            if is_hovering_file { 2.0 } else { 1.0 },
            if is_hovering_file {
                Tokens::STATUS_INFO
            } else {
                swaggen_ui_kit::border_color(ui)
            },
        ))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_INFO, "Drag & Drop");
                ui.label(swaggen_ui_kit::bold_text(
                    "Drop any .json or .csv file into this window to import APIs for instant testing",
                    12.5,
                    swaggen_ui_kit::text_primary(ui),
                ));
            });
            ui.label(
                egui::RichText::new(
                    "Auto-detects CSV (name,method,url,headers,body), Postman Collection v2.1 / Environment JSON, OpenAPI 3.x / Swagger 2.0 JSON/YAML, HAR 1.2 JSON, and .http files.",
                )
                .size(11.0)
                .color(swaggen_ui_kit::text_secondary(ui)),
            );
        });
    ui.add_space(10.0);

    swaggen_ui_kit::surface_card(ui, |ui| {
        // 1. Format Picker Pills
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(swaggen_ui_kit::bold_text(
                "Source Format:",
                12.5,
                swaggen_ui_kit::text_primary(ui),
            ));
            for (k_idx, lbl) in [
                (0usize, "cURL Command"),
                (1, "Postman Collection v2.1 / Env"),
                (2, "OpenAPI / Swagger"),
                (3, "HAR 1.2 / .http"),
                (4, "CSV (.csv API table)"),
            ] {
                if ui
                    .selectable_label(app.import_source_kind == k_idx, lbl)
                    .clicked()
                {
                    app.import_source_kind = k_idx;
                }
            }
        });
        ui.add_space(8.0);

        // 2. Load from File Path helper (except cURL where direct paste is primary, though still handy!)
        if app.import_source_kind != 0 {
            ui.horizontal_wrapped(|ui| {
                let path_w = (ui.available_width() - 190.0).max(180.0);
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.import_file_path,
                    "C:\\Users\\...\\apis.json or apis.csv (or drag & drop)",
                    path_w,
                    false,
                );
                if swaggen_ui_kit::secondary_button(
                    ui,
                    !app.import_file_path.trim().is_empty(),
                    "Load Buffer",
                )
                .clicked()
                {
                    match std::fs::read_to_string(app.import_file_path.trim()) {
                        Ok(content) => {
                            app.postman_input = content;
                            app.status = "Loaded file into import buffer".into();
                        }
                        Err(e) => {
                            app.error = true;
                            app.status = format!("Cannot read file: {e}");
                        }
                    }
                }
                if swaggen_ui_kit::primary_button(
                    ui,
                    !app.import_file_path.trim().is_empty(),
                    "Auto-Import File",
                )
                .clicked()
                {
                    let p = std::path::Path::new(app.import_file_path.trim());
                    match std::fs::read_to_string(p) {
                        Ok(content) => {
                            let fname = p
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("imported-file")
                                .to_owned();
                            app.import_dropped_or_loaded_file(&fname, &content);
                        }
                        Err(e) => {
                            app.error = true;
                            app.status = format!("Cannot read file: {e}");
                        }
                    }
                }
            });
            ui.add_space(6.0);
        }

        // 3. Input Editor + Action Buttons
        match app.import_source_kind {
            0 => {
                ui.label(
                    egui::RichText::new(
                        "Paste a cURL command (supports Linux \\ and Windows ^/backtick continuations — never executes a shell):",
                    )
                    .size(11.5)
                    .color(swaggen_ui_kit::text_secondary(ui)),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::multiline(&mut app.curl_input)
                        .hint_text("curl -X POST https://api.example.test/v1/items -H \"Content-Type: application/json\" -d '{\"name\":\"demo\"}'")
                        .desired_width(ui.available_width())
                        .desired_rows(6)
                        .code_editor(),
                );
                ui.add_space(8.0);
                if swaggen_ui_kit::primary_button(
                    ui,
                    !app.curl_input.trim().is_empty(),
                    "Import cURL into Workbench",
                )
                .clicked()
                    && let Some(col_id) = app.state.collections.first().map(|c| c.id.clone())
                {
                    match parse_curl_command(&app.curl_input, &col_id) {
                        Ok(req) => {
                            let rid = req.id.clone();
                            app.state.requests.push(req);
                            app.open_request_in_workbench(&rid);
                            app.mark_dirty();
                            app.status = "Imported cURL command into Workbench".into();
                        }
                        Err(msg) => {
                            app.error = true;
                            app.status = msg;
                        }
                    }
                }
            }
            1 => {
                ui.add(
                    egui::TextEdit::multiline(&mut app.postman_input)
                        .hint_text(
                            "Paste Postman Collection v2.1 JSON or Postman Environment JSON here…",
                        )
                        .desired_width(ui.available_width())
                        .desired_rows(9)
                        .code_editor(),
                );
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if swaggen_ui_kit::secondary_button(
                        ui,
                        !app.postman_input.trim().is_empty(),
                        "Preview Import",
                    )
                    .clicked()
                    {
                        match import_postman_collection_v2_1(&app.postman_input) {
                            Ok((col, folders, reqs, report)) => {
                                let preview = preview_request_import(&app.state, &reqs, &report);
                                app.last_import_summary = format!(
                                    "Preview '{}': {} new request(s), {} duplicate(s), {} folder(s), {} script/auth note(s)",
                                    col.name,
                                    preview.new_requests,
                                    preview.duplicate_requests,
                                    folders.len(),
                                    preview.unsupported_items.len()
                                );
                            }
                            Err(msg) => {
                                app.last_import_summary = format!("Preview check: {msg}");
                            }
                        }
                    }
                    if swaggen_ui_kit::primary_button(
                        ui,
                        !app.postman_input.trim().is_empty(),
                        "Import Postman Collection v2.1",
                    )
                    .clicked()
                    {
                        match import_postman_collection_v2_1(&app.postman_input) {
                            Ok((col, folders, reqs, report)) => {
                                let preview = preview_request_import(&app.state, &reqs, &report);
                                app.last_import_summary = format!(
                                    "Imported '{}': {} request(s) ({} duplicates), {} folder(s), Pre/Post scripts attached!",
                                    col.name,
                                    reqs.len(),
                                    preview.duplicate_requests,
                                    folders.len()
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
                    if swaggen_ui_kit::secondary_button(
                        ui,
                        !app.postman_input.trim().is_empty(),
                        "Import as Postman Environment",
                    )
                    .clicked()
                    {
                        match import_postman_environment(&app.postman_input) {
                            Ok((env, lines)) => {
                                let eid = env.id.clone();
                                app.last_import_summary = format!(
                                    "Imported Environment '{}' ({} vars): {}",
                                    env.name,
                                    env.variables.len(),
                                    lines.join(" · ")
                                );
                                app.state.environments.push(env);
                                app.state.active_environment_id = Some(eid);
                                app.mark_dirty();
                            }
                            Err(msg) => {
                                app.error = true;
                                app.status = msg;
                            }
                        }
                    }
                });
            }
            2 => {
                ui.add(
                    egui::TextEdit::multiline(&mut app.postman_input)
                        .hint_text("Paste OpenAPI 3.0/3.1 YAML/JSON, Swagger 2.0 JSON/YAML, Swagger HTML, or Postman Collection JSON here…")
                        .desired_width(ui.available_width())
                        .desired_rows(9)
                        .code_editor(),
                );
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if swaggen_ui_kit::primary_button(
                        ui,
                        !app.postman_input.trim().is_empty(),
                        "Import as Workbench Collection + Docs",
                    )
                    .clicked()
                    {
                        let raw = app.postman_input.clone();
                        app.import_dropped_or_loaded_file("pasted-openapi.json", &raw);
                    }
                    if swaggen_ui_kit::secondary_button(
                        ui,
                        !app.postman_input.trim().is_empty(),
                        "Open in Docs Studio Only",
                    )
                    .clicked()
                    {
                        let doc = parse_api_documentation(
                            swaggen_domain::workspace::new_uid(),
                            format!("Imported API Docs {}", app.specs.len() + 1),
                            &app.postman_input,
                            &[],
                        );
                        app.spec_editor_text = doc.raw_text.clone();
                        app.specs.push(doc);
                        app.selected_spec_idx = app.specs.len() - 1;
                        app.spec_view_mode = 0;
                        app.submit(Command::SaveSpecs {
                            specs: app.specs.clone(),
                        });
                        app.navigate_to(MainMode::Specs);
                    }
                });
            }
            3 => {
                ui.add(
                    egui::TextEdit::multiline(&mut app.postman_input)
                        .hint_text("Paste HAR 1.2 JSON or VS Code REST Client .http text here…")
                        .desired_width(ui.available_width())
                        .desired_rows(9)
                        .code_editor(),
                );
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if swaggen_ui_kit::primary_button(
                        ui,
                        !app.postman_input.trim().is_empty(),
                        "Import HAR 1.2",
                    )
                    .clicked()
                        && let Some(cid) = app.state.collections.first().map(|c| c.id.clone())
                    {
                        match swaggen_spec_engine::har_http::import_har(&app.postman_input, &cid) {
                            Ok((reqs, rep)) => {
                                app.last_import_summary = format!(
                                    "Imported {} request(s) from HAR ({} note(s))",
                                    reqs.len(),
                                    rep.warnings.len()
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
                    if swaggen_ui_kit::secondary_button(
                        ui,
                        !app.postman_input.trim().is_empty(),
                        "Import .http File",
                    )
                    .clicked()
                        && let Some(cid) = app.state.collections.first().map(|c| c.id.clone())
                    {
                        match swaggen_spec_engine::har_http::import_dot_http(
                            &app.postman_input,
                            &cid,
                        ) {
                            Ok((reqs, vars, _)) => {
                                app.last_import_summary = format!(
                                    "Imported {} request(s) and {} variable(s) from .http",
                                    reqs.len(),
                                    vars.len()
                                );
                                app.state.requests.extend(reqs);
                                app.state
                                    .workspace_vars
                                    .extend(vars.into_iter().map(|(k, v)| {
                                        swaggen_domain::workspace::VariableEntry::public(k, v)
                                    }));
                                app.mark_dirty();
                            }
                            Err(e) => {
                                app.error = true;
                                app.status = e;
                            }
                        }
                    }
                });
            }
            _ => {
                ui.add(
                    egui::TextEdit::multiline(&mut app.postman_input)
                        .hint_text("name,method,url,headers,body,description\nList Users,GET,https://api.example.test/users?page=1,Accept: application/json,,Fetch users")
                        .desired_width(ui.available_width())
                        .desired_rows(9)
                        .code_editor(),
                );
                ui.add_space(8.0);
                if swaggen_ui_kit::primary_button(
                    ui,
                    !app.postman_input.trim().is_empty(),
                    "Import CSV into Workbench Collection",
                )
                .clicked()
                {
                    match swaggen_spec_engine::importers::import_requests_from_csv(
                        "Imported CSV APIs",
                        &app.postman_input,
                    ) {
                        Ok((col, reqs, _rep)) => {
                            let count = reqs.len();
                            let first_id = reqs.first().map(|r| r.id.clone());
                            app.state.collections.push(col);
                            app.state.requests.extend(reqs);
                            if let Some(rid) = first_id {
                                app.open_request_in_workbench(&rid);
                            }
                            app.mark_dirty();
                            app.last_import_summary =
                                format!("Imported {count} API request(s) from CSV into Workbench");
                        }
                        Err(e) => {
                            app.error = true;
                            app.status = e;
                        }
                    }
                }
            }
        }

        if !app.last_import_summary.is_empty() {
            ui.add_space(8.0);
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Info,
                "Import Summary",
                &app.last_import_summary,
            );
        }
    });
}

fn show_export_tab(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    // 1. Export Sanitized Workspace JSON
    swaggen_ui_kit::surface_card(ui, |ui| {
        swaggen_ui_kit::card_header(
            ui,
            "1. Sanitized Workspace JSON Export (Secrets & Cookies Stripped)",
        );
        ui.label(
            egui::RichText::new(
                "Exports all Collections, Folders, Requests, Scripts, and shared Environment keys while automatically stripping DPAPI secrets, personal overrides, and cookies.",
            )
            .size(11.5)
            .color(swaggen_ui_kit::text_secondary(ui)),
        );
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            let path_w = (ui.available_width() - 190.0).max(200.0);
            swaggen_ui_kit::text_input(
                ui,
                &mut app.export_workspace_path,
                "C:\\path\\to\\workspace-export.json",
                path_w,
                false,
            );
            if swaggen_ui_kit::primary_button(ui, true, "Export Sanitized JSON").clicked() {
                app.submit(Command::ExportSanitizedWorkspace {
                    destination: PathBuf::from(app.export_workspace_path.trim()),
                    state: Box::new(app.state.clone()),
                });
            }
        });
    });
    ui.add_space(8.0);

    // 2. Git-Friendly Project Tree Export
    swaggen_ui_kit::surface_card(ui, |ui| {
        swaggen_ui_kit::card_header(ui, "2. Git-Friendly Multi-File Project Tree (IM09)");
        ui.label(
            egui::RichText::new(
                "Writes deterministic per-collection/per-spec files with stable ordering so teammates can review diffs cleanly in Git Pull Requests.",
            )
            .size(11.5)
            .color(swaggen_ui_kit::text_secondary(ui)),
        );
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            let dir_w = (ui.available_width() - 210.0).max(200.0);
            swaggen_ui_kit::text_input(
                ui,
                &mut app.git_export_dir,
                "C:\\path\\to\\git-export",
                dir_w,
                false,
            );
            if swaggen_ui_kit::primary_button(ui, true, "Export Git Project Tree").clicked() {
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
            ui.add_space(6.0);
            swaggen_ui_kit::code_surface(ui, &app.p07_export_preview);
        }
    });
    ui.add_space(8.0);

    // 3. Active Request Masked cURL + Open Project Bundle Summary
    swaggen_ui_kit::surface_card(ui, |ui| {
        swaggen_ui_kit::card_header(ui, "3. Masked cURL & Open Project Bundle");
        ui.horizontal_wrapped(|ui| {
            if swaggen_ui_kit::secondary_button(
                ui,
                app.active_request().is_some(),
                "Copy Active Request as Masked cURL",
            )
            .clicked()
                && let Some(req) = app.active_request()
            {
                let cmd = to_curl_command(req, true);
                ctx.copy_text(cmd);
                app.status = "Copied masked cURL command to clipboard".into();
            }
            if let Ok(bundle) =
                export_open_project_bundle(&app.state, &app.specs, &RedactionPolicy::default())
            {
                swaggen_ui_kit::pill_badge(
                    ui,
                    Tokens::STATUS_SUCCESS,
                    &format!(
                        "{} · {} bundle file(s) ready",
                        bundle.manifest.schema_version,
                        bundle.manifest.files.len()
                    ),
                );
            }
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Bundle attachments (CSV):");
            swaggen_ui_kit::text_input(
                ui,
                &mut app.attach_files_csv,
                "C:\\data\\sample.json",
                200.0,
                false,
            );
            swaggen_ui_kit::text_input(
                ui,
                &mut app.attach_shared_dir,
                "Output folder",
                200.0,
                false,
            );
            if swaggen_ui_kit::secondary_button(
                ui,
                !app.attach_files_csv.trim().is_empty(),
                "Bundle Files",
            )
            .clicked()
            {
                let files: Vec<PathBuf> = app
                    .attach_files_csv
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
                    .collect();
                match bundle_shared_attachments(
                    &files,
                    std::path::Path::new(app.attach_shared_dir.trim()),
                    5 * 1024 * 1024,
                ) {
                    Ok(refs) => {
                        app.p07_export_preview =
                            serde_json::to_string_pretty(&refs).unwrap_or_default();
                        app.status = format!("Bundled {} attachment(s)", refs.len());
                    }
                    Err(e) => {
                        app.error = true;
                        app.status = e;
                    }
                }
            }
        });
    });
}
