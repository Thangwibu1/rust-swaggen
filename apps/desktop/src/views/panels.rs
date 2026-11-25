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
    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.label(swaggen_ui_kit::bold_text(
            "Confirm production request",
            18.0,
            swaggen_ui_kit::text_primary(ui),
        ));
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "You are sending a data-mutating request to an environment marked Production.",
            )
            .size(12.5)
            .color(swaggen_ui_kit::text_secondary(ui)),
        );
        ui.add_space(10.0);
        if let Some(req) = app.active_request() {
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Danger,
                req.method.as_str(),
                &format!("Target URL: {}", req.url),
            );
        }
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if swaggen_ui_kit::danger_button(ui, true, "Send to Production").clicked() {
                app.send_active_request(true);
            }
            if swaggen_ui_kit::secondary_button(ui, true, "Cancel").clicked() {
                app.show_production_confirm = false;
            }
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
            (MainMode::Privacy, "General & updates"),
            (MainMode::Storage, "Storage & retention"),
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
    if swaggen_ui_kit::page_header_secondary(
        ui,
        "Your workspaces",
        &format!(
            "{} · Switch between Individual (local-only) & Team workspaces",
            app.state.meta.name
        ),
        Some("Close"),
        true,
    ) {
        app.show_workspace_modal = false;
    }
    let scroll_h = ui.available_height().clamp(220.0, 560.0);
    egui::ScrollArea::vertical()
        .id_salt("ws_manager_modal_scroll")
        .max_height(scroll_h)
        .show(ui, |ui| {
            ui.label(swaggen_ui_kit::bold_text(
                "Individual Workspaces (Personal · Local only)",
                14.0,
                Tokens::TEXT_PRIMARY,
            ));
            ui.add_space(6.0);
            let workspaces = app.available_workspaces.clone();
            let ws_cols = if ui.available_width() < 640.0 {
                1
            } else {
                3.min(workspaces.len().max(1))
            };
            ui.columns(ws_cols, |cols| {
                for (idx, ws_id) in workspaces.iter().enumerate() {
                    let col_idx = idx % cols.len();
                    let active =
                        *ws_id == app.workspace_id && app.selected_team_workspace_id.is_none();
                    swaggen_ui_kit::surface_card(&mut cols[col_idx], |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(ws_id.as_str())
                                    .size(14.5)
                                    .strong()
                                    .color(Tokens::TEXT_PRIMARY),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if active {
                                        swaggen_ui_kit::pill_badge(
                                            ui,
                                            Tokens::STATUS_SUCCESS,
                                            "Active",
                                        );
                                    } else {
                                        swaggen_ui_kit::pill_badge(
                                            ui,
                                            Tokens::STATUS_SUCCESS,
                                            "Personal",
                                        );
                                    }
                                },
                            );
                        });
                        ui.add_space(6.0);
                        if *ws_id == app.workspace_id {
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
                        if swaggen_ui_kit::secondary_button(
                            ui,
                            !active,
                            if active {
                                "Current workspace"
                            } else {
                                "Open individual"
                            },
                        )
                        .clicked()
                        {
                            if *ws_id != app.workspace_id {
                                app.submit(Command::SwitchWorkspace { id: ws_id.clone() });
                            }
                            app.selected_team_workspace_id = None;
                            app.show_workspace_modal = false;
                            app.navigate_to(MainMode::Workbench);
                        }
                    });
                    cols[col_idx].add_space(8.0);
                }
            });

            ui.add_space(10.0);
            swaggen_ui_kit::surface_card(ui, |ui| {
                swaggen_ui_kit::card_header(ui, "Create individual workspace");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut app.new_workspace_name)
                            .hint_text("Workspace name (e.g. My Local API)")
                            .desired_width(280.0),
                    );
                    let parsed = App::slugify_workspace_id(&app.new_workspace_name);
                    if swaggen_ui_kit::primary_button(
                        ui,
                        parsed.is_some() && !app.new_workspace_name.trim().is_empty(),
                        "Create & open",
                    )
                    .clicked()
                        && let Some(id) = parsed
                    {
                        let display_name = std::mem::take(&mut app.new_workspace_name)
                            .trim()
                            .to_owned();
                        app.new_workspace_id.clear();
                        app.submit(Command::CreateWorkspace {
                            id,
                            name: display_name,
                            kind: WorkspaceKind::Personal,
                        });
                        app.selected_team_workspace_id = None;
                        app.show_workspace_modal = false;
                        app.navigate_to(MainMode::Workbench);
                    }
                });
            });

            if !app.team_workspaces.is_empty() {
                ui.add_space(12.0);
                ui.label(swaggen_ui_kit::bold_text(
                    "Team Workspaces (Collaborate & Sync)",
                    14.0,
                    Tokens::TEXT_PRIMARY,
                ));
                ui.add_space(6.0);
                let prof_id = app
                    .server_profiles
                    .active()
                    .map(|p| p.id.clone())
                    .unwrap_or_else(|| "production".into());
                let mut open_team: Option<String> = None;
                let teams = app.team_workspaces.clone();
                ui.columns(3.min(teams.len().max(1)), |cols| {
                    for (idx, tw) in teams.iter().enumerate() {
                        let col_idx = idx % cols.len();
                        let active = app.selected_team_workspace_id.as_deref()
                            == Some(tw.workspace_id.as_str());
                        swaggen_ui_kit::surface_card(&mut cols[col_idx], |ui| {
                            ui.horizontal(|ui| {
                                ui.label(swaggen_ui_kit::bold_text(
                                    &tw.name,
                                    14.0,
                                    Tokens::TEXT_PRIMARY,
                                ));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_INFO, "Team");
                                    },
                                );
                            });
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} · {} members",
                                    tw.role.as_str(),
                                    tw.member_count
                                ))
                                .size(12.0)
                                .color(Tokens::TEXT_SECONDARY),
                            );
                            ui.add_space(8.0);
                            if swaggen_ui_kit::primary_button(
                                ui,
                                !active,
                                if active { "Current team" } else { "Open team" },
                            )
                            .clicked()
                            {
                                open_team = Some(tw.workspace_id.clone());
                            }
                        });
                    }
                });
                if let Some(wid) = open_team {
                    app.selected_team_workspace_id = Some(wid.clone());
                    app.submit(Command::SyncNow {
                        profile_id: prof_id.clone(),
                        workspace_id: wid.clone(),
                    });
                    app.submit(Command::LoadCollabData {
                        profile_id: prof_id,
                        workspace_id: wid,
                    });
                    app.show_workspace_modal = false;
                    app.navigate_to(MainMode::Workbench);
                }
            }

            ui.add_space(10.0);
            swaggen_ui_kit::surface_card(ui, |ui| {
                swaggen_ui_kit::card_header(ui, "Workspace settings");
                let mut changed = false;
                swaggen_ui_kit::config_field_row(
                    ui,
                    "Workspace name",
                    "Shown in sidebar and headers",
                    |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut app.state.meta.name)
                                        .desired_width(260.0),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                            if ui
                                .checkbox(&mut app.state.meta.archived, "Archived")
                                .changed()
                            {
                                changed = true;
                            }
                        });
                    },
                );
                swaggen_ui_kit::config_field_row(
                    ui,
                    "Description",
                    "Team or project summary",
                    |ui| {
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut app.state.meta.description)
                                    .desired_width(360.0),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    },
                );
                swaggen_ui_kit::config_field_row(
                    ui,
                    "Default API Base URL",
                    "Workspace fallback {{base_url}}",
                    |ui| {
                        if ui
                            .add(
                                egui::TextEdit::singleline(
                                    &mut app.state.meta.defaults.default_base_url,
                                )
                                .desired_width(360.0),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    },
                );
                swaggen_ui_kit::config_field_row(
                    ui,
                    "Naming convention",
                    "Endpoint & schema lint guideline",
                    |ui| {
                        if ui
                            .add(
                                egui::TextEdit::singleline(
                                    &mut app.state.meta.defaults.naming_convention,
                                )
                                .desired_width(360.0),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    },
                );
                swaggen_ui_kit::config_field_row(
                    ui,
                    "Onboarding guide",
                    "Notes shown to teammates",
                    |ui| {
                        if ui
                            .add(
                                egui::TextEdit::multiline(
                                    &mut app.state.meta.defaults.onboarding_guide,
                                )
                                .desired_width(360.0)
                                .desired_rows(2),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    },
                );
                if changed {
                    app.mark_dirty();
                }
            });
        });
}

pub fn show_environments(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::page_header(
        ui,
        swaggen_ui_kit::tr(
            app.locale,
            "Environments & Variables",
            "Môi trường & Biến (Variables)",
        ),
        swaggen_ui_kit::tr(
            app.locale,
            "Clear variable scopes: Request > Active Environment (My Local Override > Shared) > Collection > Global Workspace.",
            "Thứ tự ưu tiên biến rõ ràng: Request > Môi trường đang bật (Cá nhân > Chung) > Collection > Toàn Workspace.",
        ),
        None,
        false,
    );
    show_env_group_tabs(app, ui);

    let mut changed = false;

    // Visual Scope Hierarchy Selector (F8)
    let active_env_summary = app
        .state
        .active_environment()
        .map(|e| format!("● Active: {}", e.name))
        .unwrap_or_else(|| "○ No Active Environment".into());

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (idx, lbl) in [
            (
                0usize,
                format!(
                    "1. Deployment Environments ({})",
                    app.state.environments.len()
                ),
            ),
            (
                1,
                format!("2. Collection Variables ({})", app.state.collections.len()),
            ),
            (
                2,
                format!(
                    "3. Global Workspace Variables ({})",
                    app.state.workspace_vars.len()
                ),
            ),
        ] {
            if ui
                .selectable_label(
                    app.env_scope_tab == idx,
                    egui::RichText::new(lbl).size(12.0).strong(),
                )
                .clicked()
            {
                app.env_scope_tab = idx;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            swaggen_ui_kit::pill_badge(
                ui,
                if app.state.active_environment_id.is_some() {
                    Tokens::STATUS_SUCCESS
                } else {
                    swaggen_ui_kit::text_secondary(ui)
                },
                &active_env_summary,
            );
        });
    });
    ui.add_space(6.0);

    egui::ScrollArea::vertical()
        .id_salt("env_manager_scroll")
        .show(ui, |ui| {
            match app.env_scope_tab {
                0 => {
                    // Deployment Environments: Local / Development / Staging / Production
                    swaggen_ui_kit::surface_card(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let mut activate_id: Option<String> = None;
                            for (idx, env) in app.state.environments.iter().enumerate() {
                                let is_selected = app.selected_env_idx == idx;
                                let is_active = app.state.active_environment_id.as_deref()
                                    == Some(env.id.as_str());
                                let dot = if is_active { "●" } else { "○" };
                                let tag = if env.is_production { " [PROD]" } else { "" };
                                let col = if env.is_production {
                                    Tokens::TEXT_DANGER
                                } else if is_active {
                                    Tokens::STATUS_SUCCESS
                                } else {
                                    swaggen_ui_kit::text_primary(ui)
                                };
                                let btn = ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{dot} {}{tag} ({})",
                                                env.name,
                                                env.variables.len()
                                            ))
                                            .size(12.0)
                                            .color(col)
                                            .strong(),
                                        )
                                        .fill(if is_selected {
                                            swaggen_ui_kit::selected_bg(ui)
                                        } else {
                                            swaggen_ui_kit::surface_bg(ui)
                                        })
                                        .stroke(egui::Stroke::new(
                                            if is_selected { 1.5 } else { 1.0 },
                                            if is_selected {
                                                Tokens::STATUS_INFO
                                            } else {
                                                swaggen_ui_kit::border_color(ui)
                                            },
                                        ))
                                        .corner_radius(6),
                                    )
                                    .on_hover_text(
                                        "Click to edit & activate this environment for {{variables}}",
                                    );
                                if btn.clicked() {
                                    app.selected_env_idx = idx;
                                    activate_id = Some(env.id.clone());
                                }
                            }
                            if let Some(eid) = activate_id
                                && app.state.active_environment_id.as_deref()
                                    != Some(eid.as_str())
                            {
                                app.state.active_environment_id = Some(eid);
                                changed = true;
                            }

                            ui.separator();
                            swaggen_ui_kit::text_input(
                                ui,
                                &mut app.new_env_name,
                                "New env (e.g. Local, QA)",
                                155.0,
                                false,
                            );
                            ui.checkbox(&mut app.new_env_prod, "Prod");
                            if swaggen_ui_kit::secondary_button(
                                ui,
                                !app.new_env_name.trim().is_empty(),
                                "+ Add Env",
                            )
                            .clicked()
                            {
                                let mut created =
                                    Environment::new(app.new_env_name.trim(), app.new_env_prod);
                                created.variables.push(VariableEntry::public(
                                    "base_url",
                                    "http://127.0.0.1:8080",
                                ));
                                let cid = created.id.clone();
                                app.state.environments.push(created);
                                app.new_env_name.clear();
                                app.new_env_prod = false;
                                app.selected_env_idx =
                                    app.state.environments.len().saturating_sub(1);
                                app.state.active_environment_id = Some(cid);
                                changed = true;
                            }
                        });

                        ui.add_space(10.0);
                        let can_delete_env = app.state.environments.len() > 1;
                        let mut delete_selected_env = false;
                        let mut toggle_active_env: Option<Option<String>> = None;

                        if app.selected_env_idx >= app.state.environments.len() {
                            app.selected_env_idx = 0;
                        }
                        if let Some(env) = app.state.environments.get_mut(app.selected_env_idx) {
                            let is_active = app.state.active_environment_id.as_deref()
                                == Some(env.id.as_str());
                            ui.horizontal_wrapped(|ui| {
                                ui.label(swaggen_ui_kit::bold_text(
                                    "Environment Name:",
                                    12.0,
                                    swaggen_ui_kit::text_primary(ui),
                                ));
                                if swaggen_ui_kit::text_input(
                                    ui,
                                    &mut env.name,
                                    "Environment name",
                                    170.0,
                                    false,
                                )
                                .changed()
                                {
                                    changed = true;
                                }
                                let mut tier = env.effective_tier();
                                egui::ComboBox::from_id_salt("env_tier_combo")
                                    .selected_text(format!("Tier: {}", tier.label()))
                                    .width(145.0)
                                    .show_ui(ui, |ui| {
                                        for t in [
                                            swaggen_domain::workspace::EnvTier::Local,
                                            swaggen_domain::workspace::EnvTier::Development,
                                            swaggen_domain::workspace::EnvTier::Staging,
                                            swaggen_domain::workspace::EnvTier::Production,
                                            swaggen_domain::workspace::EnvTier::Custom,
                                        ] {
                                            if ui
                                                .selectable_value(&mut tier, t, t.label())
                                                .changed()
                                            {
                                                env.tier = Some(t);
                                                env.is_production = t
                                                    == swaggen_domain::workspace::EnvTier::Production;
                                                changed = true;
                                            }
                                        }
                                    });
                                if ui
                                    .checkbox(&mut env.is_production, "Production guard")
                                    .on_hover_text(
                                        "Prompts before sending POST/PUT/PATCH/DELETE or GraphQL Mutations",
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if is_active {
                                    swaggen_ui_kit::status_badge(
                                        ui,
                                        NoticeKind::Success,
                                        "✓ Active Environment",
                                    );
                                    if ui.small_button("Deactivate").clicked() {
                                        toggle_active_env = Some(None);
                                    }
                                } else if swaggen_ui_kit::primary_button(
                                    ui,
                                    true,
                                    "Set as Active Environment",
                                )
                                .clicked()
                                {
                                    toggle_active_env = Some(Some(env.id.clone()));
                                }
                                if can_delete_env {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if swaggen_ui_kit::danger_button(
                                                ui,
                                                true,
                                                "Delete Env",
                                            )
                                            .clicked()
                                            {
                                                delete_selected_env = true;
                                            }
                                        },
                                    );
                                }
                            });

                            ui.add_space(8.0);
                            if edit_flat_variable_table(
                                ui,
                                "env_vars_grid",
                                &mut env.variables,
                                Some(&mut env.personal_overrides),
                            ) {
                                changed = true;
                            }
                        }

                        if let Some(next_act) = toggle_active_env {
                            app.state.active_environment_id = next_act;
                            changed = true;
                        }
                        if delete_selected_env && app.state.environments.len() > 1 {
                            let removed = app.state.environments.remove(app.selected_env_idx);
                            if app.state.active_environment_id.as_deref()
                                == Some(removed.id.as_str())
                            {
                                app.state.active_environment_id =
                                    app.state.environments.first().map(|e| e.id.clone());
                            }
                            app.selected_env_idx = app
                                .selected_env_idx
                                .min(app.state.environments.len().saturating_sub(1));
                            changed = true;
                        }
                    });
                }
                1 => {
                    // Collection-Scoped Variables ("cho riêng collection nào đó")
                    swaggen_ui_kit::surface_card(ui, |ui| {
                        swaggen_ui_kit::card_header(
                            ui,
                            "Collection Variables (Scoped to a single Collection)",
                        );
                        ui.label(
                            egui::RichText::new(
                                "Variables defined here apply automatically to all requests inside the selected Collection (overridden by Active Environment if same name).",
                            )
                            .size(11.5)
                            .color(swaggen_ui_kit::text_secondary(ui)),
                        );
                        ui.add_space(6.0);
                        if app.selected_collection_var_idx >= app.state.collections.len() {
                            app.selected_collection_var_idx = 0;
                        }
                        ui.horizontal_wrapped(|ui| {
                            ui.label(swaggen_ui_kit::bold_text(
                                "Select Collection:",
                                12.0,
                                swaggen_ui_kit::text_primary(ui),
                            ));
                            for (cidx, col) in app.state.collections.iter().enumerate() {
                                if ui
                                    .selectable_label(
                                        app.selected_collection_var_idx == cidx,
                                        format!("📁 {} ({})", col.name, col.variables.len()),
                                    )
                                    .clicked()
                                {
                                    app.selected_collection_var_idx = cidx;
                                }
                            }
                        });
                        ui.add_space(8.0);
                        if let Some(col) = app
                            .state
                            .collections
                            .get_mut(app.selected_collection_var_idx)
                            && edit_flat_variable_table(
                                ui,
                                "col_vars_grid",
                                &mut col.variables,
                                None,
                            )
                        {
                            changed = true;
                        }
                    });
                }
                _ => {
                    // Global Workspace Variables
                    swaggen_ui_kit::surface_card(ui, |ui| {
                        swaggen_ui_kit::card_header(
                            ui,
                            "Global Workspace Variables (Shared fallback across all Collections & Environments)",
                        );
                        ui.label(
                            egui::RichText::new(
                                "Global variables resolve everywhere in this workspace whenever a variable is not overridden by the Collection or Active Environment.",
                            )
                            .size(11.5)
                            .color(swaggen_ui_kit::text_secondary(ui)),
                        );
                        ui.add_space(8.0);
                        if edit_flat_variable_table(
                            ui,
                            "global_vars_grid",
                            &mut app.state.workspace_vars,
                            None,
                        ) {
                            changed = true;
                        }
                    });
                }
            }

            ui.add_space(10.0);
            swaggen_ui_kit::surface_card(ui, |ui| {
                swaggen_ui_kit::card_header(ui, "Live {{variable}} Precedence Inspector");
                ui.horizontal_wrapped(|ui| {
                    ui.label("Test variable name:");
                    swaggen_ui_kit::text_input(
                        ui,
                        &mut app.inspect_var_key,
                        "base_url",
                        190.0,
                        false,
                    );
                    let secrets = app.masked_secret_placeholders();
                    let col_vars = app
                        .active_request()
                        .and_then(|r| {
                            app.state
                                .collections
                                .iter()
                                .find(|c| c.id == r.collection_id)
                        })
                        .or_else(|| app.state.collections.get(app.selected_collection_var_idx))
                        .map(|c| c.variables.as_slice())
                        .unwrap_or(&[]);
                    let ctx = VariableContext {
                        workspace_vars: &app.state.workspace_vars,
                        collection_vars: col_vars,
                        environment: app.state.active_environment(),
                        request_vars: app
                            .active_request()
                            .map(|r| r.local_vars.as_slice())
                            .unwrap_or(&[]),
                        local_secrets: &secrets,
                    };
                    match ctx.inspect(app.inspect_var_key.trim()) {
                        Some(eff) => {
                            swaggen_ui_kit::status_badge(
                                ui,
                                NoticeKind::Success,
                                &format!("Resolved from {}", eff.source),
                            );
                            ui.monospace(if eff.is_secret {
                                "••••••••"
                            } else {
                                eff.value.as_str()
                            });
                        }
                        None => {
                            swaggen_ui_kit::status_badge(
                                ui,
                                NoticeKind::Warning,
                                "Undefined in active scopes",
                            );
                        }
                    }
                });
            });
        });

    if changed {
        app.mark_dirty();
    }
}

/// Flat Postman-style Variable Grid (`✓ | VARIABLE | TYPE | SHARED VALUE | MY LOCAL OVERRIDE | ×`).
fn edit_flat_variable_table(
    ui: &mut egui::Ui,
    id_salt: &str,
    vars: &mut Vec<VariableEntry>,
    mut personal_overrides: Option<&mut std::collections::BTreeMap<String, PersonalVarOverride>>,
) -> bool {
    let mut changed = false;
    let mut rm_idx = None;
    let border_col = swaggen_ui_kit::border_color(ui);
    let total_w = ui.available_width().max(260.0);
    let has_override_col = personal_overrides.is_some() && total_w >= 560.0;
    let chk_w = 28.0;
    let type_w = 88.0;
    let act_w = 28.0;
    let rem_w = (total_w - chk_w - type_w - act_w - 16.0).max(150.0);
    let (key_w, val_w, ov_w) = if has_override_col {
        (rem_w * 0.30, rem_w * 0.36, rem_w * 0.34)
    } else {
        (rem_w * 0.38, rem_w * 0.62, 0.0)
    };

    ui.push_id(id_salt, |ui| {
        egui::Frame::new()
            .fill(swaggen_ui_kit::surface_bg(ui))
            .stroke(egui::Stroke::new(1.0, border_col))
            .corner_radius(4)
            .inner_margin(egui::Margin::ZERO)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                // Header
                let hdr = egui::Frame::new()
                    .fill(swaggen_ui_kit::subtle_bg(ui))
                    .inner_margin(egui::Margin::symmetric(0, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                            ui.allocate_exact_size(egui::vec2(chk_w, 18.0), egui::Sense::hover());
                            ui.add_space(8.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(key_w, 18.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new("VARIABLE")
                                            .size(10.0)
                                            .color(swaggen_ui_kit::text_secondary(ui))
                                            .strong(),
                                    );
                                },
                            );
                            ui.allocate_ui_with_layout(
                                egui::vec2(type_w, 18.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new("TYPE")
                                            .size(10.0)
                                            .color(swaggen_ui_kit::text_secondary(ui))
                                            .strong(),
                                    );
                                },
                            );
                            ui.allocate_ui_with_layout(
                                egui::vec2(val_w, 18.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new("INITIAL / SHARED VALUE")
                                            .size(10.0)
                                            .color(swaggen_ui_kit::text_secondary(ui))
                                            .strong(),
                                    );
                                },
                            );
                            if has_override_col {
                                ui.allocate_ui_with_layout(
                                    egui::vec2(ov_w, 18.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new("CURRENT / MY LOCAL OVERRIDE")
                                                .size(10.0)
                                                .color(swaggen_ui_kit::text_secondary(ui))
                                                .strong(),
                                        );
                                    },
                                );
                            }
                        });
                    });
                ui.painter().line_segment(
                    [
                        hdr.response.rect.left_bottom(),
                        hdr.response.rect.right_bottom(),
                    ],
                    egui::Stroke::new(1.0, border_col),
                );

                for (i, v) in vars.iter_mut().enumerate() {
                    let r = egui::Frame::new()
                        .fill(swaggen_ui_kit::surface_bg(ui))
                        .inner_margin(egui::Margin::symmetric(0, 3))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                                ui.allocate_ui_with_layout(
                                    egui::vec2(chk_w, 22.0),
                                    egui::Layout::centered_and_justified(
                                        egui::Direction::LeftToRight,
                                    ),
                                    |ui| {
                                        if ui.checkbox(&mut v.enabled, "").changed() {
                                            changed = true;
                                        }
                                    },
                                );
                                ui.add_space(8.0);
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut v.key)
                                            .id(egui::Id::unique((id_salt, "vk", i)))
                                            .hint_text("variable_name")
                                            .frame(egui::Frame::NONE)
                                            .desired_width((key_w - 6.0).max(40.0)),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.allocate_ui_with_layout(
                                    egui::vec2(type_w, 22.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        let lbl = if v.is_secret {
                                            "Secret 🔒"
                                        } else {
                                            "Default"
                                        };
                                        if ui.small_button(lbl).clicked() {
                                            v.is_secret = !v.is_secret;
                                            changed = true;
                                        }
                                    },
                                );
                                if v.is_secret {
                                    let mut rname = v
                                        .secret_ref
                                        .as_ref()
                                        .map(|id| id.as_str().to_owned())
                                        .unwrap_or_default();
                                    if ui
                                        .add(
                                            egui::TextEdit::singleline(&mut rname)
                                                .id(egui::Id::unique((id_salt, "vsec", i)))
                                                .hint_text("DPAPI secret name (e.g. api-token)")
                                                .frame(egui::Frame::NONE)
                                                .desired_width((val_w - 6.0).max(40.0)),
                                        )
                                        .changed()
                                    {
                                        v.secret_ref = LocalId::parse(rname.trim()).ok();
                                        changed = true;
                                    }
                                } else if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut v.shared_value)
                                            .id(egui::Id::unique((id_salt, "vv", i)))
                                            .hint_text("https://api.example.test")
                                            .frame(egui::Frame::NONE)
                                            .desired_width((val_w - 6.0).max(40.0)),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }

                                if has_override_col && let Some(ref mut ov_map) = personal_overrides
                                {
                                    let mut ov_val = ov_map
                                        .get(&v.key)
                                        .map(|o| o.value.clone())
                                        .unwrap_or_default();
                                    if ui
                                        .add(
                                            egui::TextEdit::singleline(&mut ov_val)
                                                .id(egui::Id::unique((id_salt, "vov", i)))
                                                .hint_text("(Same as shared)")
                                                .frame(egui::Frame::NONE)
                                                .desired_width((ov_w - 6.0).max(40.0)),
                                        )
                                        .changed()
                                        && !v.key.trim().is_empty()
                                    {
                                        if ov_val.is_empty() {
                                            ov_map.remove(&v.key);
                                        } else {
                                            ov_map.insert(
                                                v.key.clone(),
                                                PersonalVarOverride {
                                                    value: ov_val,
                                                    is_secret: false,
                                                    secret_ref: None,
                                                },
                                            );
                                        }
                                        changed = true;
                                    }
                                }

                                ui.allocate_ui_with_layout(
                                    egui::vec2(act_w, 22.0),
                                    egui::Layout::centered_and_justified(
                                        egui::Direction::LeftToRight,
                                    ),
                                    |ui| {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    egui::RichText::new("×")
                                                        .size(13.0)
                                                        .color(swaggen_ui_kit::text_secondary(ui)),
                                                )
                                                .frame(false),
                                            )
                                            .clicked()
                                        {
                                            rm_idx = Some(i);
                                        }
                                    },
                                );
                            });
                        });
                    ui.painter().line_segment(
                        [
                            r.response.rect.left_bottom(),
                            r.response.rect.right_bottom(),
                        ],
                        egui::Stroke::new(1.0, border_col),
                    );
                }

                // Inline Ghost Row to add a new variable effortlessly!
                let has_blank = vars
                    .last()
                    .is_some_and(|v| v.key.is_empty() && v.shared_value.is_empty());
                if !has_blank {
                    let next_i = vars.len();
                    let mut nk = String::new();
                    let mut nv = String::new();
                    egui::Frame::new()
                        .fill(swaggen_ui_kit::surface_bg(ui))
                        .inner_margin(egui::Margin::symmetric(0, 3))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                                ui.allocate_exact_size(
                                    egui::vec2(chk_w, 22.0),
                                    egui::Sense::hover(),
                                );
                                ui.add_space(8.0);
                                let kc = ui
                                    .add(
                                        egui::TextEdit::singleline(&mut nk)
                                            .id(egui::Id::unique((id_salt, "vk", next_i)))
                                            .hint_text("Add new variable (e.g. base_url)")
                                            .frame(egui::Frame::NONE)
                                            .desired_width((key_w - 6.0).max(40.0)),
                                    )
                                    .changed();
                                ui.allocate_exact_size(
                                    egui::vec2(type_w, 22.0),
                                    egui::Sense::hover(),
                                );
                                let vc = ui
                                    .add(
                                        egui::TextEdit::singleline(&mut nv)
                                            .id(egui::Id::unique((id_salt, "vv", next_i)))
                                            .hint_text("Value")
                                            .frame(egui::Frame::NONE)
                                            .desired_width((val_w - 6.0).max(40.0)),
                                    )
                                    .changed();
                                if (kc || vc) && (!nk.is_empty() || !nv.is_empty()) {
                                    vars.push(VariableEntry::public(nk, nv));
                                    changed = true;
                                }
                            });
                        });
                }
            });
        if let Some(i) = rm_idx {
            vars.remove(i);
            changed = true;
        }
    });
    changed
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
                    if swaggen_ui_kit::secondary_button(ui, true, "Open as new copy").clicked() {
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
            ui.heading("Secret redaction preview");
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
            ui.heading("Untrusted imported content preview");
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
        ui.heading("Sanitized workspace export");
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
        ui.heading("Migrate local data folder");
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
        "Settings & updates",
        "Manage language, workspaces, software updates, and local privacy diagnostics.",
        Some(&format!("v{}", crate::updater::APP_VERSION)),
        false,
    );
    show_settings_group_tabs(app, ui);
    egui::ScrollArea::vertical().show(ui, |ui| {
        // 0. General Preferences (Language & Workspaces) — light white surface only.
        swaggen_ui_kit::surface_card(ui, |ui| {
            swaggen_ui_kit::card_header(ui, "General Preferences");
            swaggen_ui_kit::config_field_row(
                ui,
                "Theme",
                "Pure White Figma surface (default) or Dark Slate theme (UX06)",
                |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(!app.dark_mode, "Light (Pure White)")
                            .clicked()
                            && app.dark_mode
                        {
                            app.dark_mode = false;
                            swaggen_ui_kit::apply_theme(ctx, false);
                        }
                        if ui
                            .selectable_label(app.dark_mode, "Dark (Slate)")
                            .clicked()
                            && !app.dark_mode
                        {
                            app.dark_mode = true;
                            swaggen_ui_kit::apply_theme(ctx, true);
                        }
                    });
                },
            );
            swaggen_ui_kit::config_field_row(
                ui,
                "Language",
                "Choose interface display language",
                |ui| {
                    ui.horizontal(|ui| {
                        let is_en = app.locale == swaggen_ui_kit::UiLocale::En;
                        if ui.selectable_label(is_en, "English").clicked() {
                            app.locale = swaggen_ui_kit::UiLocale::En;
                        }
                        if ui.selectable_label(!is_en, "Tiếng Việt").clicked() {
                            app.locale = swaggen_ui_kit::UiLocale::Vi;
                        }
                    });
                },
            );
            swaggen_ui_kit::config_field_row(
                ui,
                "Workspaces",
                &format!(
                    "Active: {} ({})",
                    app.state.meta.name,
                    if app.selected_team_workspace_id.is_some() {
                        "Team sync"
                    } else {
                        "Individual local-only"
                    }
                ),
                |ui| {
                    ui.horizontal(|ui| {
                        if swaggen_ui_kit::secondary_button(
                            ui,
                            true,
                            swaggen_ui_kit::tr(
                                app.locale,
                                "Switch or manage workspaces…",
                                "Chuyển hoặc quản lý không gian…",
                            ),
                        )
                        .clicked()
                        {
                            app.show_workspace_modal = true;
                        }
                        if swaggen_ui_kit::secondary_button(ui, true, "Open Launch Hub")
                            .clicked()
                        {
                            app.show_welcome = true;
                        }
                    });
                },
            );
        });
        ui.add_space(10.0);

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
        ui.heading("Diagnostic report preview");
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
            ui.heading("Signed Update Verifier & Deep Links");
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
    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.horizontal(|ui| {
            ui.label(swaggen_ui_kit::bold_text(
                swaggen_ui_kit::tr(app.locale, "Command palette", "Bảng lệnh nhanh"),
                17.5,
                swaggen_ui_kit::text_primary(ui),
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if swaggen_ui_kit::secondary_button(ui, true, "Esc").clicked() {
                    app.show_command_palette = false;
                }
            });
        });
        ui.add_space(8.0);
        let q_resp = swaggen_ui_kit::text_input(
            ui,
            &mut app.command_palette_query,
            "Type a command, screen, or request…",
            ui.available_width(),
            false,
        );
        q_resp.request_focus();
        ui.add_space(8.0);
        let q = app.command_palette_query.trim().to_ascii_lowercase();
        egui::ScrollArea::vertical()
            .id_salt("cmd_palette_scroll")
            .max_height(360.0)
            .show(ui, |ui| {
                if q.is_empty() || "sign in firebase google welcome".contains(&q) {
                    swaggen_ui_kit::table_row_frame(ui, |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Sign in with Google / Firebase")
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
                    (MainMode::Workbench, "Open API Workbench"),
                    (MainMode::Specs, "Open API documentation & OpenAPI"),
                    (MainMode::Runner, "Run collection, Mocks & Protocols"),
                    (
                        MainMode::TeamSync,
                        "Open Sync queue, Conflicts & Discussion",
                    ),
                    (MainMode::Server, "Connect Team Server & Accounts"),
                    (MainMode::Environments, "Manage Environments & Variables"),
                    (MainMode::Secrets, "Personal Secrets Vault (DPAPI)"),
                    (MainMode::ImportExport, "Import APIs, Postman & cURL"),
                    (MainMode::Privacy, "Settings, Updates & Privacy"),
                    (MainMode::Shortcuts, "Keyboard Shortcuts"),
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
                                app.navigate_to(mode);
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
                    .map(|r| {
                        let m = if r.is_graphql() {
                            "GQL".to_owned()
                        } else {
                            r.method.as_str().to_owned()
                        };
                        (r.id.clone(), m, r.name.clone())
                    })
                    .collect();
                for (rid, m, name) in reqs {
                    if swaggen_ui_kit::endpoint_row(ui, &m, &name, false).clicked() {
                        app.open_request_in_workbench(&rid);
                    }
                }
            });
    });
}

pub fn show_shortcuts(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::page_header(
        ui,
        "Keyboard shortcuts",
        "Shortcuts avoid interfering with normal editor typing.",
        None,
        false,
    );
    show_settings_group_tabs(app, ui);
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
