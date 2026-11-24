//! Secondary workspace views and modals: Workspace manager, Environments & Variable
//! Inspector, History, Cookie jar, Personal secrets, Sanitizer, Storage & Migration,
//! Privacy/Diagnostics, Shortcuts, and Production Request Confirmation.
use crate::{
    app::{App, MainMode},
    diagnostics,
    worker::Command,
};
use eframe::egui;
use std::path::PathBuf;
use swaggen_domain::{
    LocalId, Secret,
    security::{RedactionPolicy, sanitize_imported_markup, sanitize_text},
    workspace::{
        CookieRecord, Environment, PersonalVarOverride, VariableContext, VariableEntry,
        WorkspaceKind,
    },
};
use swaggen_ui_kit::{NoticeKind, Tokens};

pub fn show_production_confirm(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::section(
        ui,
        "Confirm production request (R16 / EV07)",
        "You are sending a data-mutating request to an environment marked Production.",
    );
    if let Some(req) = app.active_request() {
        swaggen_ui_kit::notice_banner(
            ui,
            NoticeKind::Danger,
            req.method.as_str(),
            &format!("Target URL: {}", req.url),
        );
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if swaggen_ui_kit::danger_button(ui, true, "Send to Production").clicked() {
            app.send_active_request(true);
        }
        if swaggen_ui_kit::secondary_button(ui, true, "Cancel").clicked() {
            app.show_production_confirm = false;
        }
    });
}

pub fn show_workspace_manager(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::section(
        ui,
        "Workspaces & Project Defaults (WS01 / WS04 / WS08)",
        "Switch between offline personal workspaces or configure project defaults.",
    );
    ui.horizontal(|ui| {
        if swaggen_ui_kit::secondary_button(ui, true, "Close").clicked() {
            app.show_workspace_modal = false;
        }
    });
    ui.add_space(6.0);
    egui::ScrollArea::vertical().show(ui, |ui| {
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.heading("Available local workspaces");
            ui.horizontal_wrapped(|ui| {
                for ws_id in app.available_workspaces.clone() {
                    let active = ws_id == app.workspace_id;
                    if ui.selectable_label(active, ws_id.as_str()).clicked() && !active {
                        app.submit(Command::SwitchWorkspace { id: ws_id });
                        app.show_workspace_modal = false;
                    }
                }
            });
            ui.separator();
            ui.label("Create new personal workspace (lowercase id):");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.new_workspace_id)
                        .hint_text("workspace-id")
                        .desired_width(140.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut app.new_workspace_name)
                        .hint_text("Display Name")
                        .desired_width(180.0),
                );
                let parsed = LocalId::parse(app.new_workspace_id.trim());
                if swaggen_ui_kit::primary_button(
                    ui,
                    parsed.is_ok() && !app.new_workspace_name.trim().is_empty(),
                    "Create",
                )
                .clicked()
                    && let Ok(id) = parsed
                {
                    app.submit(Command::CreateWorkspace {
                        id,
                        name: app.new_workspace_name.trim().to_owned(),
                        kind: WorkspaceKind::Personal,
                    });
                    app.new_workspace_id.clear();
                    app.new_workspace_name.clear();
                    app.show_workspace_modal = false;
                }
            });
        });

        ui.add_space(8.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.heading("Current workspace settings & defaults (WS04 / WS08)");
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("Name:");
                if ui.text_edit_singleline(&mut app.state.meta.name).changed() {
                    changed = true;
                }
                if ui
                    .checkbox(&mut app.state.meta.archived, "Archived (W07)")
                    .changed()
                {
                    changed = true;
                }
            });
            ui.label("Description:");
            if ui
                .text_edit_multiline(&mut app.state.meta.description)
                .changed()
            {
                changed = true;
            }
            ui.label("Default API Base URL:");
            if ui
                .text_edit_singleline(&mut app.state.meta.defaults.default_base_url)
                .changed()
            {
                changed = true;
            }
            ui.label("Naming convention:");
            if ui
                .text_edit_singleline(&mut app.state.meta.defaults.naming_convention)
                .changed()
            {
                changed = true;
            }
            ui.label("Onboarding guide:");
            if ui
                .text_edit_multiline(&mut app.state.meta.defaults.onboarding_guide)
                .changed()
            {
                changed = true;
            }
            if changed {
                app.mark_dirty();
            }
        });
    });
}

pub fn show_environments(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::section(
        ui,
        "Environments & Variable Inspector (EV01–EV07 / E01 / V01 / V02)",
        "4-level precedence: Request > Environment (Personal override > Shared) > Collection > Workspace.",
    );
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for (idx, env) in app.state.environments.iter().enumerate() {
            let label = if env.is_production {
                format!("PROD: {}", env.name)
            } else {
                env.name.clone()
            };
            if ui
                .selectable_label(app.selected_env_idx == idx, label)
                .clicked()
            {
                app.selected_env_idx = idx;
            }
        }
        ui.separator();
        ui.add(
            egui::TextEdit::singleline(&mut app.new_env_name)
                .hint_text("New env name")
                .desired_width(120.0),
        );
        ui.checkbox(&mut app.new_env_prod, "Production");
        if swaggen_ui_kit::secondary_button(ui, !app.new_env_name.trim().is_empty(), "+ Env")
            .clicked()
        {
            app.state
                .environments
                .push(Environment::new(app.new_env_name.trim(), app.new_env_prod));
            app.new_env_name.clear();
            app.new_env_prod = false;
            app.selected_env_idx = app.state.environments.len().saturating_sub(1);
            changed = true;
        }
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(env) = app.state.environments.get_mut(app.selected_env_idx) {
            ui.horizontal(|ui| {
                ui.label("Environment name:");
                if ui.text_edit_singleline(&mut env.name).changed() {
                    changed = true;
                }
                if ui
                    .checkbox(
                        &mut env.is_production,
                        "Production (requires confirmation on write)",
                    )
                    .changed()
                {
                    changed = true;
                }
            });
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Shared environment variables").strong());
            let mut rm_idx = None;
            for (i, v) in env.variables.iter_mut().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    if ui.checkbox(&mut v.enabled, "").changed() {
                        changed = true;
                    }
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut v.key)
                                .hint_text("KEY")
                                .desired_width(120.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if v.is_secret {
                        let mut rname = v
                            .secret_ref
                            .as_ref()
                            .map(|id| id.as_str().to_owned())
                            .unwrap_or_default();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut rname)
                                    .hint_text("secret-ref-id")
                                    .desired_width(140.0),
                            )
                            .changed()
                        {
                            v.secret_ref = LocalId::parse(rname.trim()).ok();
                            changed = true;
                        }
                        swaggen_ui_kit::status_badge(ui, NoticeKind::Info, "DPAPI ref");
                    } else if ui
                        .add(
                            egui::TextEdit::singleline(&mut v.shared_value)
                                .hint_text("shared value")
                                .desired_width(200.0),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if ui.checkbox(&mut v.is_secret, "Secret").changed() {
                        changed = true;
                    }
                    if ui.small_button("x").clicked() {
                        rm_idx = Some(i);
                    }
                });
            }
            if let Some(i) = rm_idx {
                env.variables.remove(i);
                changed = true;
            }
            if swaggen_ui_kit::secondary_button(ui, true, "+ Variable").clicked() {
                env.variables.push(VariableEntry::public("", ""));
                changed = true;
            }

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Personal local overrides (V02 · never published)").strong(),
            );
            let keys: Vec<String> = env
                .variables
                .iter()
                .map(|v| v.key.clone())
                .filter(|k| !k.is_empty())
                .collect();
            for key in keys {
                let mut ov_val = env
                    .personal_overrides
                    .get(&key)
                    .map(|o| o.value.clone())
                    .unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label(format!("{key} override:"));
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut ov_val)
                                .hint_text("leave empty for shared")
                                .desired_width(200.0),
                        )
                        .changed()
                    {
                        if ov_val.is_empty() {
                            env.personal_overrides.remove(&key);
                        } else {
                            env.personal_overrides.insert(
                                key,
                                PersonalVarOverride {
                                    value: ov_val,
                                    is_secret: false,
                                    secret_ref: None,
                                },
                            );
                        }
                        changed = true;
                    }
                });
            }
        }

        ui.add_space(10.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.heading("Variable inspector (V01)");
            ui.horizontal(|ui| {
                ui.label("Variable name:");
                ui.text_edit_singleline(&mut app.inspect_var_key);
            });
            let secrets = app.masked_secret_placeholders();
            let ctx = VariableContext {
                workspace_vars: &app.state.workspace_vars,
                collection_vars: &[],
                environment: app.state.environments.get(app.selected_env_idx),
                request_vars: app
                    .active_request()
                    .map(|r| r.local_vars.as_slice())
                    .unwrap_or(&[]),
                local_secrets: &secrets,
            };
            match ctx.inspect(app.inspect_var_key.trim()) {
                Some(eff) => {
                    ui.label(format!(
                        "Value: {} · Source: {}",
                        if eff.is_secret {
                            "••••••••"
                        } else {
                            &eff.value
                        },
                        eff.source
                    ));
                }
                None => {
                    ui.label("Not defined in active scopes.");
                }
            }
        });
    });

    if changed {
        app.mark_dirty();
    }
}

pub fn show_history(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::section(
        ui,
        "Local Request History (RS07 / R14 / Q30)",
        "Stored locally on this machine. Open a past run as a new draft without altering shared templates.",
    );
    ui.horizontal_wrapped(|ui| {
        ui.label("Filter:");
        ui.text_edit_singleline(&mut app.history_filter);
        if swaggen_ui_kit::danger_button(ui, !app.history.is_empty(), "Clear all history").clicked()
        {
            app.submit(Command::ClearHistory);
        }
    });
    ui.separator();
    let q = app.history_filter.trim().to_ascii_lowercase();
    let mut open_snap = None;
    let mut del_req_hist = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        for entry in &app.history {
            if !q.is_empty()
                && !entry.request_name.to_ascii_lowercase().contains(&q)
                && !entry.resolved_url.to_ascii_lowercase().contains(&q)
                && !entry.method.as_str().to_ascii_lowercase().contains(&q)
            {
                continue;
            }
            swaggen_ui_kit::surface_card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    swaggen_ui_kit::method_badge(ui, entry.method.as_str());
                    ui.label(egui::RichText::new(&entry.request_name).strong());
                    if let Some(code) = entry.status_code {
                        swaggen_ui_kit::status_badge(ui, NoticeKind::Info, &format!("HTTP {code}"));
                    } else if let Some(ref ek) = entry.error_kind {
                        swaggen_ui_kit::status_badge(ui, NoticeKind::Danger, ek);
                    }
                    ui.label(format!(
                        "{} ms · {} B",
                        entry.duration_ms, entry.decoded_bytes
                    ));
                });
                ui.monospace(&entry.resolved_url);
                ui.horizontal(|ui| {
                    if swaggen_ui_kit::secondary_button(ui, true, "Open as new copy (RQ09)")
                        .clicked()
                    {
                        open_snap = Some(entry.snapshot.clone());
                    }
                    if ui.small_button("Delete request history").clicked() {
                        del_req_hist = Some(entry.request_id.clone());
                    }
                });
            });
            ui.add_space(4.0);
        }
    });
    if let Some(mut snap) = open_snap {
        snap.id = swaggen_domain::workspace::new_uid();
        snap.name = format!("{} (from history)", snap.name);
        snap.shared_revision = 0;
        let rid = snap.id.clone();
        app.state.requests.push(snap);
        app.state.open_or_focus_tab(&rid);
        app.mode = MainMode::Workbench;
        app.mark_dirty();
    }
    if let Some(rid) = del_req_hist {
        app.submit(Command::DeleteRequestHistory { request_id: rid });
    }
}

pub fn show_cookies(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::section(
        ui,
        "Workspace Cookie Jar (AU04 / R12 / H11)",
        "Cookies are stored locally per workspace and automatically matched by domain, path and HTTPS flag.",
    );
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.cookie_domain)
                .hint_text("domain")
                .desired_width(120.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut app.cookie_path)
                .hint_text("/")
                .desired_width(70.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut app.cookie_name)
                .hint_text("name")
                .desired_width(110.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut app.cookie_value)
                .hint_text("value")
                .desired_width(140.0),
        );
        if swaggen_ui_kit::primary_button(
            ui,
            !app.cookie_domain.trim().is_empty() && !app.cookie_name.trim().is_empty(),
            "Add cookie",
        )
        .clicked()
        {
            app.state.cookies.push(CookieRecord {
                id: swaggen_domain::workspace::new_uid(),
                domain: app.cookie_domain.trim().to_owned(),
                path: if app.cookie_path.trim().is_empty() {
                    "/".into()
                } else {
                    app.cookie_path.trim().to_owned()
                },
                name: app.cookie_name.trim().to_owned(),
                value: app.cookie_value.clone(),
                expires_epoch_secs: None,
                secure: false,
                http_only: false,
            });
            app.cookie_name.clear();
            app.cookie_value.clear();
            app.mark_dirty();
        }
    });
    ui.separator();
    let mut remove_id = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        for c in &app.state.cookies {
            ui.horizontal(|ui| {
                ui.monospace(format!("{}{} -> {}={}", c.domain, c.path, c.name, c.value));
                if ui.small_button("Delete").clicked() {
                    remove_id = Some(c.id.clone());
                }
            });
        }
    });
    if let Some(cid) = remove_id {
        app.state.cookies.retain(|c| c.id != cid);
        app.mark_dirty();
    }
}

pub fn show_secrets(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::section(
        ui,
        "Personal secrets (E03 / V09)",
        "Protected for your Windows account via DPAPI. Values stay on this computer.",
    );
    swaggen_ui_kit::surface_card(ui, |ui| {
        ui.label(
            egui::RichText::new(format!(
                "Showing {} saved secret name(s)",
                app.secret_names.len()
            ))
            .color(Tokens::TEXT_SECONDARY),
        );
        let mut picked = None;
        ui.horizontal_wrapped(|ui| {
            for item in &app.secret_names {
                if swaggen_ui_kit::secondary_button(ui, app.loaded && !app.pending, item.as_str())
                    .clicked()
                {
                    picked = Some(item.as_str().to_owned());
                }
            }
        });
        if let Some(name) = picked {
            app.hide_secret();
            app.secret_name = name;
        }
    });
    ui.add_space(6.0);
    ui.label("Secret name");
    if ui
        .add_enabled(
            app.loaded && !app.pending && !app.recovery_needed,
            egui::TextEdit::singleline(&mut app.secret_name)
                .hint_text("api-token")
                .char_limit(64),
        )
        .changed()
    {
        app.hide_secret();
    }
    ui.label("New secret value");
    ui.add_enabled(
        app.loaded && !app.pending,
        egui::TextEdit::singleline(&mut app.secret_input)
            .password(true)
            .char_limit(4096),
    );
    let name = LocalId::parse(app.secret_name.clone());
    ui.horizontal_wrapped(|ui| {
        if swaggen_ui_kit::primary_button(
            ui,
            app.loaded
                && !app.pending
                && !app.recovery_needed
                && name.is_ok()
                && !app.secret_input.is_empty(),
            "Save secret locally",
        )
        .clicked()
        {
            app.hide_secret();
            let value = Secret::new(std::mem::take(&mut app.secret_input).into_bytes());
            if let (Ok(name), Ok(value)) = (name.clone(), value) {
                app.submit(Command::PutSecret { name, value });
            }
        }
        if swaggen_ui_kit::secondary_button(
            ui,
            app.loaded && !app.pending && !app.recovery_needed && name.is_ok(),
            "Reveal saved secret",
        )
        .clicked()
        {
            app.hide_secret();
            app.submit(Command::GetSecret {
                name: name.clone().unwrap(),
            });
        }
        if swaggen_ui_kit::secondary_button(ui, app.revealed.is_some(), "Hide value").clicked() {
            app.hide_secret();
        }
        if swaggen_ui_kit::danger_button(
            ui,
            app.loaded && !app.pending && !app.recovery_needed && name.is_ok(),
            "Delete secret",
        )
        .clicked()
        {
            app.hide_secret();
            app.submit(Command::DeleteSecret {
                name: name.unwrap(),
            });
        }
    });
    if let Some(secret) = &app.revealed {
        ui.add_space(6.0);
        swaggen_ui_kit::notice_banner(
            ui,
            NoticeKind::Warning,
            "Visible on this device only",
            "Not copied to clipboard.",
        );
        match std::str::from_utf8(secret.expose()) {
            Ok(text) => swaggen_ui_kit::code_surface(ui, text),
            Err(_) => {
                ui.label("Saved secret is binary");
            }
        }
    }
}

pub fn show_sanitizer(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::section(
        ui,
        "Sanitizer & privacy-safe content (SC02 / SC05 / Z15)",
        "Preview automatic secret redaction before export and inspect untrusted markup safely.",
    );
    egui::ScrollArea::vertical().show(ui, |ui| {
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.heading("1. Secret redaction preview (SC02)");
            ui.label("Extra sensitive field names (comma-separated)");
            ui.text_edit_singleline(&mut app.sanitizer_extra_fields);
            ui.add(
                egui::TextEdit::multiline(&mut app.sanitizer_input)
                    .desired_width(ui.available_width())
                    .desired_rows(4),
            );
            let policy = RedactionPolicy {
                extra_fields: app
                    .sanitizer_extra_fields
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect(),
                extra_literal_values: Vec::new(),
            };
            let preview = sanitize_text(&app.sanitizer_input, &policy);
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Warning,
                &format!("Redacted {} item(s)", preview.redacted_count),
                preview.manual_review_warning,
            );
            swaggen_ui_kit::code_surface(ui, &preview.sanitized_text);
        });
        ui.add_space(8.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.heading("2. Untrusted imported content preview (SC05 / Z15)");
            ui.add(
                egui::TextEdit::multiline(&mut app.untrusted_input)
                    .desired_width(ui.available_width())
                    .desired_rows(4),
            );
            let clean = sanitize_imported_markup(&app.untrusted_input);
            let summary = if clean.blocked_constructs.is_empty() {
                "No active constructs detected".to_owned()
            } else {
                format!("Neutralized: {}", clean.blocked_constructs.join(", "))
            };
            swaggen_ui_kit::notice_banner(ui, NoticeKind::Info, "Safe literal viewer", &summary);
            swaggen_ui_kit::code_surface(ui, &clean.safe_literal_text);
        });
    });
}

pub fn show_storage(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::section(
        ui,
        "Local storage, retention & folder migration (UX10 / S05 / Z08 / Z20)",
        "Inspect disk usage, clean cache/history without deleting collections or secrets, and export sanitized JSON.",
    );
    swaggen_ui_kit::surface_card(ui, |ui| {
        ui.label(format!(
            "Workspace journal: {} bytes",
            app.storage_stats.workspace_bytes
        ));
        ui.label(format!(
            "History journal: {} bytes ({} entries)",
            app.storage_stats.history_bytes,
            app.history.len()
        ));
        ui.label(format!(
            "Downloads/response cache: {} bytes",
            app.storage_stats.downloads_bytes
        ));
        ui.label(format!(
            "Secrets journal: {} bytes",
            app.storage_stats.secrets_bytes
        ));
        ui.horizontal(|ui| {
            ui.label("History retention cap:");
            if ui
                .add(egui::DragValue::new(&mut app.state.history_retention).range(10..=2000))
                .changed()
            {
                app.mark_dirty();
            }
            if swaggen_ui_kit::danger_button(
                ui,
                true,
                "Clean cache & history (preserve projects/secrets)",
            )
            .clicked()
            {
                app.submit(Command::CleanCache);
            }
        });
    });
    ui.add_space(8.0);
    swaggen_ui_kit::surface_card(ui, |ui| {
        ui.heading("Sanitized workspace export (EV06)");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut app.export_workspace_path).desired_width(300.0));
            if swaggen_ui_kit::primary_button(ui, true, "Export sanitized JSON").clicked() {
                app.submit(Command::ExportSanitizedWorkspace {
                    destination: PathBuf::from(app.export_workspace_path.trim()),
                    state: Box::new(app.state.clone()),
                });
            }
        });
        ui.separator();
        ui.heading("Migrate local data folder (Z08)");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.migrate_root_path)
                    .hint_text("C:\\Users\\...\\SwaggenData")
                    .desired_width(300.0),
            );
            if swaggen_ui_kit::secondary_button(
                ui,
                !app.migrate_root_path.trim().is_empty(),
                "Migrate data folder",
            )
            .clicked()
            {
                app.submit(Command::MigrateDataRoot {
                    target_root: PathBuf::from(app.migrate_root_path.trim()),
                });
            }
        });
    });
}

pub fn show_privacy(app: &mut App, ui: &mut egui::Ui, ctx: &egui::Context) {
    swaggen_ui_kit::section(
        ui,
        "Privacy & permissions (S06 / Z07)",
        "Analytics and automatic crash reporting are disabled. Only typed local commands are permitted.",
    );
    egui::ScrollArea::vertical().show(ui, |ui| {
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.label(format!(
                "Analytics: {}",
                if app.policy.analytics_enabled {
                    "on"
                } else {
                    "off"
                }
            ));
            ui.label(format!(
                "Automatic crash reports: {}",
                if app.policy.crash_upload_enabled {
                    "on"
                } else {
                    "off"
                }
            ));
            ui.label("Remote HTML/script execution: blocked");
            ui.label("External reference auto-fetch: blocked");
        });
        ui.add_space(8.0);
        ui.heading("Diagnostic report preview (Z07)");
        let diag = app.diagnostic_preview();
        swaggen_ui_kit::code_surface(ui, &diag.rendered_text);
        ui.horizontal(|ui| {
            if swaggen_ui_kit::primary_button(ui, true, "Copy reviewed diagnostics").clicked() {
                ctx.copy_text(diag.rendered_text.clone());
                app.status = "Reviewed diagnostics copied; nothing uploaded".into();
            }
            if swaggen_ui_kit::secondary_button(ui, true, "Export reviewed diagnostics file")
                .clicked()
            {
                let export_dir = app.root.join("diagnostics");
                match diagnostics::export_reviewed_bundle(&export_dir, &diag) {
                    Ok(path) => {
                        app.last_diag_export =
                            path.file_name().map(|n| n.to_string_lossy().into_owned());
                        app.status =
                            "Reviewed diagnostic file saved locally; nothing uploaded".into();
                    }
                    Err(err) => {
                        app.error = true;
                        app.status = err.to_string();
                    }
                }
            }
        });
        if let Some(name) = &app.last_diag_export {
            ui.label(
                egui::RichText::new(format!("Last exported file: {name}"))
                    .color(Tokens::STATUS_SUCCESS),
            );
        }
        ui.add_space(8.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.heading("Signed Update Verifier (OP08) & Deep Links (CO09)");
            ui.horizontal(|ui| {
                ui.label("Deep link:");
                ui.add(egui::TextEdit::singleline(&mut app.deep_link_input).desired_width(280.0));
                if swaggen_ui_kit::secondary_button(ui, true, "Open link").clicked() {
                    match swaggen_domain::governance::parse_deep_link(&app.deep_link_input) {
                        Ok(dl) if dl.workspace_id == app.workspace_id.as_str() => {
                            if dl.kind == "request"
                                && app.state.requests.iter().any(|r| r.id == dl.entity_id)
                            {
                                app.state.open_or_focus_tab(&dl.entity_id);
                                app.mode = MainMode::Workbench;
                                app.status = format!("Opened request {}", dl.entity_id);
                            } else {
                                app.mode = MainMode::Runner;
                                app.status = format!("Navigated to {}/{}", dl.kind, dl.entity_id);
                            }
                        }
                        Ok(dl) => {
                            app.error = true;
                            app.status = format!(
                                "Access denied: link workspace '{}' != active '{}'",
                                dl.workspace_id,
                                app.workspace_id.as_str()
                            );
                        }
                        Err(e) => {
                            app.error = true;
                            app.status = e;
                        }
                    }
                }
            });
            ui.label("Signed Update Manifest JSON:");
            ui.add(
                egui::TextEdit::multiline(&mut app.update_manifest_json)
                    .desired_width(ui.available_width())
                    .desired_rows(3)
                    .code_editor(),
            );
            ui.horizontal(|ui| {
                ui.label("HMAC key:");
                ui.add(egui::TextEdit::singleline(&mut app.update_signing_key).password(true));
                ui.label("Payload:");
                ui.text_edit_singleline(&mut app.update_payload_text);
                if swaggen_ui_kit::secondary_button(ui, true, "Verify signed update").clicked() {
                    match serde_json::from_str::<swaggen_domain::governance::SignedUpdateManifest>(
                        &app.update_manifest_json,
                    ) {
                        Ok(man) => match swaggen_domain::governance::verify_signed_update(
                            &man,
                            app.update_payload_text.as_bytes(),
                            app.update_signing_key.as_bytes(),
                        ) {
                            Ok(()) => {
                                app.error = false;
                                app.status = format!("Verified update {} cleanly", man.version);
                            }
                            Err(e) => {
                                app.error = true;
                                app.status = e;
                            }
                        },
                        Err(e) => {
                            app.error = true;
                            app.status = format!("Invalid manifest JSON: {e}");
                        }
                    }
                }
            });
        });
    });
}

pub fn show_command_palette(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::section(
        ui,
        swaggen_ui_kit::tr(app.locale, "Command Palette (Ctrl+K)", "Bảng lệnh (Ctrl+K)"),
        "Jump directly to any workspace request, OpenAPI spec, tool view, or action. Escape closes.",
    );
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.command_palette_query)
                .hint_text("Filter requests, specs, modes…")
                .desired_width(320.0),
        );
        if swaggen_ui_kit::secondary_button(ui, true, "Close (Esc)").clicked() {
            app.show_command_palette = false;
        }
    });
    ui.separator();
    let q = app.command_palette_query.trim().to_ascii_lowercase();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for (mode, label) in [
            (MainMode::Workbench, "Open Workbench"),
            (MainMode::Runner, "Open Collection Runner & Mock Server"),
            (MainMode::Server, "Open Team Server & Auth"),
            (MainMode::TeamSync, "Open Team Sync & Collaboration"),
            (MainMode::Specs, "Open OpenAPI & Docs"),
            (MainMode::ImportExport, "Open Import / Export"),
            (MainMode::Privacy, "Open Privacy, Diagnostics, Updates & Links"),
        ] {
            if (q.is_empty() || label.to_ascii_lowercase().contains(&q))
                && swaggen_ui_kit::secondary_button(ui, true, label).clicked()
            {
                app.mode = mode;
                app.show_command_palette = false;
            }
        }
        ui.separator();
        let reqs: Vec<(String, String, String)> = app
            .state
            .requests
            .iter()
            .filter(|r| q.is_empty() || r.name.to_ascii_lowercase().contains(&q) || r.url.to_ascii_lowercase().contains(&q))
            .map(|r| (r.id.clone(), r.method.as_str().to_owned(), r.name.clone()))
            .collect();
        for (rid, m, name) in reqs {
            if ui.selectable_label(false, format!("{m} {name}")).clicked() {
                app.state.open_or_focus_tab(&rid);
                app.mode = MainMode::Workbench;
                app.show_command_palette = false;
            }
        }
    });
}

pub fn show_shortcuts(ui: &mut egui::Ui) {
    swaggen_ui_kit::section(
        ui,
        "Keyboard shortcuts (UX03 / UX04 / UX07 / S04)",
        "Shortcuts avoid interfering with normal editor typing.",
    );
    for (keys, action) in [
        ("Ctrl + K", "Toggle Command Palette"),
        ("Ctrl + Enter", "Send active request"),
        ("Ctrl + S", "Save local workspace"),
        ("Ctrl + P", "Focus quick search in sidebar"),
        ("Ctrl + W", "Close active unpinned tab"),
        ("Esc", "Close active modal or palette"),
        ("Tab / Shift+Tab", "Navigate focusable controls (AccessKit)"),
    ] {
        ui.horizontal(|ui| {
            swaggen_ui_kit::status_badge(ui, NoticeKind::Info, keys);
            ui.label(action);
        });
    }
}
