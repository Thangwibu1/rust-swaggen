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
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        ui.set_max_width(520.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.label(swaggen_ui_kit::bold_text(
                    "Confirm production request",
                    20.0,
                    swaggen_ui_kit::text_primary(ui),
                ));
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(
                        "You are sending a data-mutating request to an environment marked Production.",
                    )
                    .size(12.5)
                    .color(swaggen_ui_kit::text_secondary(ui)),
                );
                ui.add_space(12.0);
                if let Some(req) = app.active_request() {
                    swaggen_ui_kit::notice_banner(
                        ui,
                        NoticeKind::Danger,
                        req.method.as_str(),
                        &format!("Target URL: {}", req.url),
                    );
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if swaggen_ui_kit::danger_button(ui, true, "Send to Production").clicked() {
                        app.send_active_request(true);
                    }
                    if swaggen_ui_kit::secondary_button(ui, true, "Cancel").clicked() {
                        app.show_production_confirm = false;
                    }
                });
            });
        });
    });
}

fn show_env_group_tabs(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for (mode, label) in [
            (MainMode::Environments, "Environments"),
            (MainMode::Secrets, "Personal secrets"),
            (MainMode::Cookies, "Cookies"),
            (MainMode::History, "History"),
        ] {
            if swaggen_ui_kit::underline_tab(ui, app.mode == mode, label).clicked() {
                app.hide_secret();
                app.mode = mode;
            }
        }
    });
    ui.add_space(10.0);
}

fn show_settings_group_tabs(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for (mode, label) in [
            (MainMode::Storage, "Storage & retention"),
            (MainMode::Privacy, "Privacy & updates"),
            (MainMode::Sanitizer, "Secret sanitizer"),
            (MainMode::Shortcuts, "Keyboard shortcuts"),
        ] {
            if swaggen_ui_kit::underline_tab(ui, app.mode == mode, label).clicked() {
                app.hide_secret();
                app.mode = mode;
            }
        }
    });
    ui.add_space(10.0);
}

pub fn show_workspace_manager(app: &mut App, ui: &mut egui::Ui) {
    if swaggen_ui_kit::page_header(
        ui,
        "Your workspaces",
        &format!("{} / Workspace management & defaults", app.state.meta.name),
        Some("Close"),
        true,
    ) {
        app.show_workspace_modal = false;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        let workspaces = app.available_workspaces.clone();
        ui.columns(3.min(workspaces.len().max(1)), |cols| {
            for (idx, ws_id) in workspaces.iter().enumerate() {
                let col_idx = idx % cols.len();
                let active = *ws_id == app.workspace_id;
                swaggen_ui_kit::surface_card(&mut cols[col_idx], |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(ws_id.as_str())
                                .size(14.5)
                                .strong()
                                .color(Tokens::TEXT_PRIMARY),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if active {
                                swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_SUCCESS, "Active");
                            } else {
                                swaggen_ui_kit::pill_badge(ui, Tokens::TEXT_SECONDARY, "Personal");
                            }
                        });
                    });
                    ui.add_space(6.0);
                    if active {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} requests · {} collections",
                                app.state.requests.len(),
                                app.state.collections.len()
                            ))
                            .size(12.0)
                            .color(Tokens::TEXT_SECONDARY),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("Local personal workspace")
                                .size(12.0)
                                .color(Tokens::TEXT_SECONDARY),
                        );
                    }
                    ui.add_space(8.0);
                    if swaggen_ui_kit::secondary_button(ui, !active, if active { "Current workspace" } else { "Open workspace" })
                        .clicked()
                    {
                        app.submit(Command::SwitchWorkspace { id: ws_id.clone() });
                        app.show_workspace_modal = false;
                    }
                });
                cols[col_idx].add_space(8.0);
            }
        });

        ui.add_space(10.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            swaggen_ui_kit::card_header(ui, "Create a workspace");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.new_workspace_id)
                        .hint_text("workspace-id (lowercase)")
                        .desired_width(180.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut app.new_workspace_name)
                        .hint_text("Display Name")
                        .desired_width(240.0),
                );
                let parsed = LocalId::parse(app.new_workspace_id.trim());
                if swaggen_ui_kit::primary_button(
                    ui,
                    parsed.is_ok() && !app.new_workspace_name.trim().is_empty(),
                    "Create workspace",
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

        ui.add_space(10.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            swaggen_ui_kit::card_header(ui, "Workspace settings");
            let mut changed = false;
            swaggen_ui_kit::config_field_row(ui, "Workspace name", "Shown in sidebar and headers", |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::TextEdit::singleline(&mut app.state.meta.name).desired_width(260.0))
                        .changed()
                    {
                        changed = true;
                    }
                    if ui.checkbox(&mut app.state.meta.archived, "Archived").changed() {
                        changed = true;
                    }
                });
            });
            swaggen_ui_kit::config_field_row(ui, "Description", "Team or project summary", |ui| {
                if ui
                    .add(egui::TextEdit::singleline(&mut app.state.meta.description).desired_width(360.0))
                    .changed()
                {
                    changed = true;
                }
            });
            swaggen_ui_kit::config_field_row(ui, "Default API Base URL", "Workspace fallback {{base_url}}", |ui| {
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut app.state.meta.defaults.default_base_url)
                            .desired_width(360.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
            swaggen_ui_kit::config_field_row(ui, "Naming convention", "Endpoint & schema lint guideline", |ui| {
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut app.state.meta.defaults.naming_convention)
                            .desired_width(360.0),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
            swaggen_ui_kit::config_field_row(ui, "Onboarding guide", "Notes shown to teammates", |ui| {
                if ui
                    .add(
                        egui::TextEdit::multiline(&mut app.state.meta.defaults.onboarding_guide)
                            .desired_width(360.0)
                            .desired_rows(2),
                    )
                    .changed()
                {
                    changed = true;
                }
            });
            if changed {
                app.mark_dirty();
            }
        });
    });
}

pub fn show_environments(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::page_header(
        ui,
        "Environments",
        "4-level precedence: Request > Environment (Personal override > Shared) > Collection > Workspace.",
        None,
        false,
    );
    show_env_group_tabs(app, ui);
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
    swaggen_ui_kit::page_header(
        ui,
        "Request history",
        "Stored locally on this machine. Open any past run as a new draft without altering shared templates.",
        None,
        false,
    );
    show_env_group_tabs(app, ui);
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
    swaggen_ui_kit::page_header(
        ui,
        "Workspace cookies",
        "Cookies are stored locally per workspace and matched automatically by domain, path and HTTPS flag.",
        None,
        false,
    );
    show_env_group_tabs(app, ui);
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
    swaggen_ui_kit::page_header(
        ui,
        "Personal secrets",
        "Protected for your Windows account via DPAPI. Values stay strictly on this computer.",
        None,
        false,
    );
    show_env_group_tabs(app, ui);
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
    swaggen_ui_kit::page_header(
        ui,
        "Sanitizer & privacy-safe preview",
        "Preview automatic secret redaction before export and inspect untrusted markup safely.",
        None,
        false,
    );
    show_settings_group_tabs(app, ui);
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
    swaggen_ui_kit::page_header(
        ui,
        "Storage & retention",
        "Inspect disk usage, clean cache/history without deleting collections or secrets, and export sanitized JSON.",
        None,
        false,
    );
    show_settings_group_tabs(app, ui);
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
    swaggen_ui_kit::page_header(
        ui,
        "Updates, privacy & diagnostics",
        "Check for Swaggen updates from GitHub Releases, apply downloaded update packages in-place, and review local privacy diagnostics.",
        Some(&format!("v{}", crate::updater::APP_VERSION)),
        false,
    );
    show_settings_group_tabs(app, ui);
    egui::ScrollArea::vertical().show(ui, |ui| {
        // 1. In-App Auto-Updater (Option 1: GitHub Releases) & Local Package Updater (Option 2: .exe/.msi/.zip)
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(swaggen_ui_kit::bold_text(
                    "Software Updates",
                    15.0,
                    swaggen_ui_kit::text_primary(ui),
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    swaggen_ui_kit::pill_badge(
                        ui,
                        Tokens::STATUS_INFO,
                        &format!("Installed v{}", crate::updater::APP_VERSION),
                    );
                });
            });
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(
                    "Option 1: Auto-check and download verified updates directly from GitHub Releases (SHA-256 checked).",
                )
                .size(12.0)
                .color(swaggen_ui_kit::text_secondary(ui)),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let feed_w = (ui.available_width() - 170.0).max(200.0);
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.update_feed_url,
                    crate::updater::DEFAULT_UPDATE_FEED_URL,
                    feed_w,
                    false,
                );
                if swaggen_ui_kit::primary_button(ui, !app.pending, "Check for updates").clicked() {
                    app.submit(Command::CheckForUpdates {
                        feed_url: app.update_feed_url.trim().to_owned(),
                    });
                }
            });
            ui.add_space(6.0);
            ui.checkbox(
                &mut app.update_silent_install,
                "Silent in-place upgrade (auto-close Swaggen, upgrade binaries, and relaunch)",
            );

            if let Some(ref uc) = app.update_check.clone() {
                ui.add_space(8.0);
                if uc.update_available {
                    swaggen_ui_kit::notice_banner(
                        ui,
                        NoticeKind::Success,
                        &format!("New version v{} available", uc.latest.version),
                        &format!(
                            "{} · SHA-256: {}",
                            if uc.latest.notes.is_empty() {
                                "Ready to download and install in-place."
                            } else {
                                uc.latest.notes.as_str()
                            },
                            &uc.latest.installer_sha256[..16.min(uc.latest.installer_sha256.len())]
                        ),
                    );
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if swaggen_ui_kit::primary_button(
                            ui,
                            !app.pending && !uc.latest.installer_url.is_empty(),
                            &format!("Download & Install v{} (.exe)", uc.latest.version),
                        )
                        .clicked()
                        {
                            app.submit(Command::DownloadAndInstallUpdate {
                                download_url: uc.latest.installer_url.clone(),
                                expected_sha256: uc.latest.installer_sha256.clone(),
                                version: uc.latest.version.clone(),
                                silent: app.update_silent_install,
                            });
                        }
                        if !uc.latest.portable_url.is_empty()
                            && swaggen_ui_kit::secondary_button(
                                ui,
                                !app.pending,
                                "Download & Apply Portable ZIP",
                            )
                            .clicked()
                        {
                            app.submit(Command::DownloadAndInstallUpdate {
                                download_url: uc.latest.portable_url.clone(),
                                expected_sha256: uc.latest.portable_sha256.clone(),
                                version: uc.latest.version.clone(),
                                silent: app.update_silent_install,
                            });
                        }
                    });
                } else {
                    swaggen_ui_kit::notice_banner(
                        ui,
                        NoticeKind::Info,
                        &format!("Up to date (v{})", uc.current_version),
                        &format!(
                            "Latest published release on feed is v{}.",
                            uc.latest.version
                        ),
                    );
                }
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(swaggen_ui_kit::bold_text(
                "Option 2: Install downloaded update file straight into Swaggen (.exe / .msi / .zip)",
                13.0,
                swaggen_ui_kit::text_primary(ui),
            ));
            ui.add_space(6.0);
            let local_w = ui.available_width();
            swaggen_ui_kit::text_input(
                ui,
                &mut app.local_update_path,
                "C:\\Users\\...\\Downloads\\Swaggen-Setup-0.1.4-x64.exe (or .msi / .zip)",
                local_w,
                false,
            );
            ui.add_space(6.0);
            swaggen_ui_kit::text_input(
                ui,
                &mut app.local_update_sha256,
                "Optional SHA-256 checksum from SHA256SUMS.txt",
                local_w,
                false,
            );
            ui.add_space(8.0);
            if swaggen_ui_kit::secondary_button(
                ui,
                !app.pending && !app.local_update_path.trim().is_empty(),
                "Install local update package now",
            )
            .clicked()
            {
                app.submit(Command::InstallLocalUpdatePackage {
                    path: PathBuf::from(app.local_update_path.trim()),
                    expected_sha256: app.local_update_sha256.trim().to_owned(),
                    silent: app.update_silent_install,
                });
            }
        });

        ui.add_space(10.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            swaggen_ui_kit::card_header(ui, "Privacy & Local Security");
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
    ui.add_space(28.0);
    ui.vertical_centered(|ui| {
        ui.set_max_width(560.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.horizontal(|ui| {
                    ui.label(swaggen_ui_kit::bold_text(
                        swaggen_ui_kit::tr(app.locale, "Command palette", "Bảng lệnh nhanh"),
                        20.0,
                        swaggen_ui_kit::text_primary(ui),
                    ));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if swaggen_ui_kit::secondary_button(ui, true, "Esc").clicked() {
                            app.show_command_palette = false;
                        }
                    });
                });
                ui.add_space(10.0);
                let q_resp = swaggen_ui_kit::text_input(
                    ui,
                    &mut app.command_palette_query,
                    "Type a command, screen, or request…",
                    ui.available_width(),
                    false,
                );
                q_resp.request_focus();
                ui.add_space(10.0);
                let q = app.command_palette_query.trim().to_ascii_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(380.0)
                    .show(ui, |ui| {
                        if q.is_empty() || "sign in firebase google welcome".contains(&q) {
                            swaggen_ui_kit::table_row_frame(ui, |ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("Sign in with Google / Firebase (A01)")
                                                .size(13.0)
                                                .color(Tokens::STATUS_INFO),
                                        )
                                        .fill(egui::Color32::TRANSPARENT)
                                        .stroke(egui::Stroke::NONE),
                                    )
                                    .clicked()
                                {
                                    app.show_command_palette = false;
                                    app.show_welcome = true;
                                }
                            });
                        }
                        for (mode, label) in [
                            (MainMode::Workbench, "Open API Workbench (R01)"),
                            (MainMode::Specs, "Open API documentation & OpenAPI (D01)"),
                            (MainMode::Runner, "Run collection, Mocks & Protocols (T02 / P01)"),
                            (MainMode::TeamSync, "Open Sync queue, Conflicts & Discussion (C04–C06)"),
                            (MainMode::Server, "Connect Team Server & Accounts (A05)"),
                            (MainMode::Environments, "Manage Environments & Variables (E01)"),
                            (MainMode::Secrets, "Personal Secrets Vault (DPAPI)"),
                            (MainMode::ImportExport, "Import APIs, Postman & cURL (I01)"),
                            (MainMode::Storage, "Workspace Storage & Settings (S01)"),
                            (MainMode::Shortcuts, "Keyboard Shortcuts (S08)"),
                        ] {
                            if q.is_empty() || label.to_ascii_lowercase().contains(&q) {
                                swaggen_ui_kit::table_row_frame(ui, |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new(label)
                                                    .size(13.0)
                                                    .color(swaggen_ui_kit::text_primary(ui)),
                                            )
                                            .fill(egui::Color32::TRANSPARENT)
                                            .stroke(egui::Stroke::NONE),
                                        )
                                        .clicked()
                                    {
                                        app.mode = mode;
                                        app.show_command_palette = false;
                                    }
                                });
                            }
                        }
                        let reqs: Vec<(String, String, String)> = app
                            .state
                            .requests
                            .iter()
                            .filter(|r| {
                                q.is_empty()
                                    || r.name.to_ascii_lowercase().contains(&q)
                                    || r.url.to_ascii_lowercase().contains(&q)
                            })
                            .map(|r| (r.id.clone(), r.method.as_str().to_owned(), r.name.clone()))
                            .collect();
                        for (rid, m, name) in reqs {
                            if swaggen_ui_kit::endpoint_row(ui, &m, &name, false).clicked() {
                                app.state.open_or_focus_tab(&rid);
                                app.mode = MainMode::Workbench;
                                app.show_command_palette = false;
                            }
                        }
                    });
            });
        });
    });
}

pub fn show_shortcuts(ui: &mut egui::Ui) {
    swaggen_ui_kit::page_header(
        ui,
        "Keyboard shortcuts",
        "Shortcuts avoid interfering with normal editor typing.",
        None,
        false,
    );
    swaggen_ui_kit::surface_card(ui, |ui| {
        swaggen_ui_kit::table_header(ui, &[("SHORTCUT", 180.0), ("ACTION", 420.0)]);
        for (keys, action) in [
            ("Ctrl + K", "Toggle Command Palette"),
            ("Ctrl + Enter", "Send active request"),
            ("Ctrl + S", "Save local workspace"),
            ("Ctrl + P", "Focus quick search in sidebar"),
            ("Ctrl + W", "Close active unpinned tab"),
            ("Esc", "Close active modal or palette"),
            ("Tab / Shift+Tab", "Navigate focusable controls (AccessKit)"),
        ] {
            swaggen_ui_kit::table_row_frame(ui, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(170.0, 22.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_INFO, keys);
                    },
                );
                ui.label(
                    egui::RichText::new(action)
                        .size(13.0)
                        .color(swaggen_ui_kit::text_primary(ui)),
                );
            });
        }
    });
}
