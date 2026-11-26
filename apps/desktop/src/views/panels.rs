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
    workspace::{CookieRecord, Environment, PersonalVarOverride, VariableEntry, WorkspaceKind},
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
            app.active_workspace_display_name()
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
            let workspaces: Vec<_> = if app.workspace_summaries.is_empty() {
                app.available_workspaces
                    .iter()
                    .filter(|w| !w.as_str().starts_with("team-"))
                    .map(|id| crate::worker::WorkspaceListEntry {
                        name: if id.as_str() == "personal" {
                            "Personal".into()
                        } else {
                            id.as_str().to_owned()
                        },
                        id: id.clone(),
                        kind: WorkspaceKind::Personal,
                    })
                    .collect()
            } else {
                app.workspace_summaries
                    .iter()
                    .filter(|w| {
                        w.kind == WorkspaceKind::Personal && !w.id.as_str().starts_with("team-")
                    })
                    .cloned()
                    .collect()
            };
            let ws_cols = if ui.available_width() < 640.0 {
                1
            } else {
                3.min(workspaces.len().max(1))
            };
            ui.columns(ws_cols, |cols| {
                for (idx, ws_entry) in workspaces.iter().enumerate() {
                    let ws_id = &ws_entry.id;
                    let ws_display =
                        if ws_entry.id.as_str() == "personal" && ws_entry.name == "personal" {
                            "Personal"
                        } else {
                            ws_entry.name.as_str()
                        };
                    let col_idx = idx % cols.len();
                    let active =
                        *ws_id == app.workspace_id && app.selected_team_workspace_id.is_none();
                    swaggen_ui_kit::surface_card(&mut cols[col_idx], |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(ws_display)
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
                        if ws_entry.id == app.workspace_id {
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
                            if ws_entry.id != app.workspace_id {
                                app.submit(Command::SwitchWorkspace {
                                    id: ws_entry.id.clone(),
                                });
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
    show_env_group_tabs(app, ui);

    let mut changed = false;
    if app.selected_env_idx >= app.state.environments.len() {
        app.selected_env_idx = 0;
    }
    if app.selected_collection_var_idx >= app.state.collections.len() {
        app.selected_collection_var_idx = 0;
    }

    // Scope mapping in `app.env_scope_tab`: 0 = Deployment Environment, 1 = Collection, 2 = Global
    let avail_w = ui.available_width();
    let use_two_pane = avail_w >= 640.0;

    if use_two_pane {
        let total_h = ui.available_height().max(240.0);
        let list_w = (avail_w * 0.24).clamp(178.0, 224.0);
        let border_col = swaggen_ui_kit::border_color(ui);

        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui_with_layout(
                egui::vec2(list_w, total_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_width(list_w);
                    ui.set_max_width(list_w);
                    egui::ScrollArea::vertical()
                        .id_salt("env_scope_sidebar")
                        .max_height(total_h)
                        .show(ui, |ui| {
                            show_env_scope_list(app, ui, &mut changed);
                        });
                },
            );

            // Single clean 1px vertical separator between scope list & variable grid
            let (_, div_r) = ui.allocate_space(egui::vec2(1.0, total_h));
            ui.painter().line_segment(
                [
                    egui::pos2(div_r.center().x, div_r.top()),
                    egui::pos2(div_r.center().x, div_r.bottom()),
                ],
                egui::Stroke::new(1.0, border_col),
            );

            ui.add_space(16.0);
            let editor_w = (avail_w - list_w - 17.0).max(260.0);
            ui.allocate_ui_with_layout(
                egui::vec2(editor_w, total_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_width(editor_w);
                    ui.set_max_width(editor_w);
                    egui::ScrollArea::vertical()
                        .id_salt("env_manager_scroll")
                        .max_height(total_h)
                        .show(ui, |ui| {
                            show_env_active_editor(app, ui, &mut changed);
                        });
                },
            );
        });
    } else {
        egui::ScrollArea::vertical()
            .id_salt("env_manager_scroll_narrow")
            .show(ui, |ui| {
                show_env_scope_list(app, ui, &mut changed);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);
                show_env_active_editor(app, ui, &mut changed);
            });
    }

    if changed {
        app.mark_dirty();
    }
}

fn show_env_scope_list(app: &mut App, ui: &mut egui::Ui, changed: &mut bool) {
    ui.add_space(4.0);
    // 1. Globals
    let g_sel = app.env_scope_tab == 2;
    if env_sidebar_row(
        ui,
        "Globals",
        Some(app.state.workspace_vars.len()),
        g_sel,
        None,
    )
    .clicked()
    {
        app.env_scope_tab = 2;
    }

    ui.add_space(12.0);
    // 2. Environments header + quiet `+` icon button
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("ENVIRONMENTS")
                .size(10.0)
                .color(swaggen_ui_kit::text_secondary(ui))
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if swaggen_ui_kit::mini_icon_button(
                ui,
                "env_create_btn",
                swaggen_ui_kit::MiniIconKind::Plus,
                Tokens::STATUS_INFO,
                "Create new environment",
            )
            .clicked()
            {
                let mut idx_n = app.state.environments.len() + 1;
                let mut name = format!("Environment {idx_n}");
                while app.state.environments.iter().any(|e| e.name == name) {
                    idx_n += 1;
                    name = format!("Environment {idx_n}");
                }
                let mut created = Environment::new(name, false);
                created
                    .variables
                    .push(VariableEntry::public("base_url", "http://127.0.0.1:8080"));
                let cid = created.id.clone();
                app.state.environments.push(created);
                app.env_scope_tab = 0;
                app.selected_env_idx = app.state.environments.len().saturating_sub(1);
                app.state.active_environment_id = Some(cid);
                *changed = true;
            }
        });
    });
    ui.add_space(4.0);

    let mut activate_id: Option<String> = None;
    for (idx, env) in app.state.environments.iter().enumerate() {
        let is_selected = app.env_scope_tab == 0 && app.selected_env_idx == idx;
        let is_active = app.state.active_environment_id.as_deref() == Some(env.id.as_str());
        let dot_color = if env.is_production {
            Some(Tokens::TEXT_DANGER)
        } else if is_active {
            Some(Tokens::STATUS_SUCCESS)
        } else {
            None
        };
        let suffix = if env.is_production {
            format!("{} [PROD]", env.name)
        } else {
            env.name.clone()
        };
        let resp = env_sidebar_row(
            ui,
            &suffix,
            Some(env.variables.len()),
            is_selected,
            dot_color,
        )
        .on_hover_text("Click to edit and activate this environment");
        if resp.clicked() {
            app.env_scope_tab = 0;
            app.selected_env_idx = idx;
            activate_id = Some(env.id.clone());
        }
    }
    if let Some(eid) = activate_id
        && app.state.active_environment_id.as_deref() != Some(eid.as_str())
    {
        app.state.active_environment_id = Some(eid);
        *changed = true;
    }

    if !app.state.collections.is_empty() {
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("COLLECTION VARIABLES")
                .size(10.0)
                .color(swaggen_ui_kit::text_secondary(ui))
                .strong(),
        );
        ui.add_space(4.0);
        for (cidx, col) in app.state.collections.iter().enumerate() {
            let is_sel = app.env_scope_tab == 1 && app.selected_collection_var_idx == cidx;
            if env_sidebar_row(ui, &col.name, Some(col.variables.len()), is_sel, None).clicked() {
                app.env_scope_tab = 1;
                app.selected_collection_var_idx = cidx;
            }
        }
    }
}

fn env_sidebar_row(
    ui: &mut egui::Ui,
    label: &str,
    count: Option<usize>,
    selected: bool,
    dot_color: Option<egui::Color32>,
) -> egui::Response {
    let w = (ui.available_width() - 8.0).max(110.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 28.0), egui::Sense::click());
    let bg = if selected {
        swaggen_ui_kit::selected_bg(ui)
    } else if resp.hovered() {
        swaggen_ui_kit::subtle_bg(ui)
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 5.0, bg);

    let mut text_x = rect.left() + 10.0;
    if let Some(dc) = dot_color {
        ui.painter()
            .circle_filled(egui::pos2(rect.left() + 14.0, rect.center().y), 3.5, dc);
        text_x = rect.left() + 24.0;
    }

    let font = if selected {
        egui::FontId::new(12.0, egui::FontFamily::Name("bold".into()))
    } else {
        egui::FontId::new(12.0, egui::FontFamily::Proportional)
    };
    let fg = if selected {
        Tokens::STATUS_INFO
    } else {
        swaggen_ui_kit::text_primary(ui)
    };
    let max_chars = ((w - (text_x - rect.left()) - 28.0) / 6.6).max(6.0) as usize;
    let clipped: String = if label.chars().count() > max_chars {
        let mut s: String = label.chars().take(max_chars.saturating_sub(1)).collect();
        s.push('…');
        s
    } else {
        label.to_owned()
    };
    ui.painter().text(
        egui::pos2(text_x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        clipped,
        font,
        fg,
    );
    if let Some(n) = count {
        ui.painter().text(
            egui::pos2(rect.right() - 8.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            n.to_string(),
            egui::FontId::new(10.5, egui::FontFamily::Monospace),
            swaggen_ui_kit::text_secondary(ui),
        );
    }
    resp
}

fn show_env_active_editor(app: &mut App, ui: &mut egui::Ui, changed: &mut bool) {
    match app.env_scope_tab {
        0 => {
            let can_delete_env = app.state.environments.len() > 1;
            let mut delete_selected_env = false;
            let mut toggle_active_env: Option<Option<String>> = None;

            if let Some(env) = app.state.environments.get_mut(app.selected_env_idx) {
                let is_active = app.state.active_environment_id.as_deref() == Some(env.id.as_str());
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut env.name)
                                .hint_text("Environment name")
                                .font(egui::FontId::new(
                                    17.0,
                                    egui::FontFamily::Name("bold".into()),
                                ))
                                .frame(egui::Frame::NONE)
                                .desired_width(210.0),
                        )
                        .changed()
                    {
                        *changed = true;
                    }

                    let mut tier = env.effective_tier();
                    egui::ComboBox::from_id_salt("env_tier_combo")
                        .selected_text(tier.label())
                        .width(118.0)
                        .show_ui(ui, |ui| {
                            for t in [
                                swaggen_domain::workspace::EnvTier::Local,
                                swaggen_domain::workspace::EnvTier::Development,
                                swaggen_domain::workspace::EnvTier::Staging,
                                swaggen_domain::workspace::EnvTier::Production,
                                swaggen_domain::workspace::EnvTier::Custom,
                            ] {
                                if ui.selectable_value(&mut tier, t, t.label()).changed() {
                                    env.tier = Some(t);
                                    env.is_production =
                                        t == swaggen_domain::workspace::EnvTier::Production;
                                    *changed = true;
                                }
                            }
                        });

                    if ui
                        .checkbox(&mut env.is_production, "Production guard")
                        .on_hover_text(
                            "Ask confirmation before sending mutating calls in this environment",
                        )
                        .changed()
                    {
                        *changed = true;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if can_delete_env
                            && swaggen_ui_kit::danger_button(ui, true, "Delete").clicked()
                        {
                            delete_selected_env = true;
                        }
                        if is_active {
                            if swaggen_ui_kit::secondary_button(ui, true, "✓ Active")
                                .on_hover_text("Click to deactivate")
                                .clicked()
                            {
                                toggle_active_env = Some(None);
                            }
                        } else if swaggen_ui_kit::primary_button(ui, true, "Set Active").clicked() {
                            toggle_active_env = Some(Some(env.id.clone()));
                        }
                    });
                });

                ui.add_space(8.0);
                if edit_flat_variable_table(
                    ui,
                    "env_vars_grid",
                    &mut env.variables,
                    Some(&mut env.personal_overrides),
                ) {
                    *changed = true;
                }
            }

            if let Some(next_act) = toggle_active_env {
                app.state.active_environment_id = next_act;
                *changed = true;
            }
            if delete_selected_env && app.state.environments.len() > 1 {
                let eid = app.state.environments[app.selected_env_idx].id.clone();
                if app.state.delete_environment(&eid) {
                    app.selected_env_idx = app
                        .selected_env_idx
                        .min(app.state.environments.len().saturating_sub(1));
                    app.status = "Moved environment to Trash (restore from Sidebar)".into();
                    *changed = true;
                }
            }
        }
        1 => {
            if let Some(col) = app
                .state
                .collections
                .get_mut(app.selected_collection_var_idx)
            {
                ui.label(swaggen_ui_kit::bold_text(
                    &format!("Collection · {}", col.name),
                    17.0,
                    swaggen_ui_kit::text_primary(ui),
                ));
                ui.label(
                    egui::RichText::new(
                        "Variables scoped to requests in this collection (overridden by Active Environment).",
                    )
                    .size(11.5)
                    .color(swaggen_ui_kit::text_secondary(ui)),
                );
                ui.add_space(8.0);
                if edit_flat_variable_table(ui, "col_vars_grid", &mut col.variables, None) {
                    *changed = true;
                }
            }
        }
        _ => {
            ui.label(swaggen_ui_kit::bold_text(
                "Global Variables",
                17.0,
                swaggen_ui_kit::text_primary(ui),
            ));
            ui.label(
                egui::RichText::new(
                    "Shared across the workspace whenever not overridden by Collection or Active Environment.",
                )
                .size(11.5)
                .color(swaggen_ui_kit::text_secondary(ui)),
            );
            ui.add_space(8.0);
            if edit_flat_variable_table(ui, "global_vars_grid", &mut app.state.workspace_vars, None)
            {
                *changed = true;
            }
        }
    }
}

fn env_fixed_cell(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    centered: bool,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let layout = if centered {
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    };
    ui.allocate_ui_with_layout(egui::vec2(width, height), layout, |ui| {
        ui.set_min_size(egui::vec2(width, height));
        ui.set_max_size(egui::vec2(width, height));
        let clip = ui
            .max_rect()
            .shrink2(egui::vec2(2.0, 0.0))
            .intersect(ui.clip_rect());
        ui.set_clip_rect(clip);
        if !centered {
            ui.add_space(10.0);
        }
        add_contents(ui);
    });
}

/// Pixel-aligned Flat Postman Variable Grid (`✓ | VARIABLE | INITIAL VALUE | CURRENT VALUE | SECRET | ×`).
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
    let has_override_col = personal_overrides.is_some() && total_w >= 540.0;
    let chk_w = 34.0_f32;
    let sec_w = 64.0_f32;
    let act_w = 32.0_f32;
    let rem_w = (total_w - chk_w - sec_w - act_w).max(150.0);
    let (key_w, val_w, ov_w) = if has_override_col {
        (rem_w * 0.28, rem_w * 0.36, rem_w * 0.36)
    } else {
        (rem_w * 0.36, rem_w * 0.64, 0.0)
    };
    let header_h = 28.0_f32;
    let row_h = 32.0_f32;

    ui.push_id(id_salt, |ui| {
        let table_resp = egui::Frame::new()
            .fill(swaggen_ui_kit::surface_bg(ui))
            .stroke(egui::Stroke::new(1.0, border_col))
            .corner_radius(4)
            .inner_margin(egui::Margin::ZERO)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                let left_x = ui.available_rect_before_wrap().left();
                let x_chk_end = left_x + chk_w;
                let x_key_end = x_chk_end + key_w;
                let x_val_end = x_key_end + val_w;
                let x_ov_end = x_val_end + ov_w;
                let x_sec_end = x_ov_end + sec_w;

                // Header
                let hdr = egui::Frame::new()
                    .fill(swaggen_ui_kit::subtle_bg(ui))
                    .inner_margin(egui::Margin::ZERO)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                            env_fixed_cell(ui, chk_w, header_h, true, |_| {});
                            env_fixed_cell(ui, key_w, header_h, false, |ui| {
                                ui.label(
                                    egui::RichText::new("VARIABLE")
                                        .size(10.5)
                                        .color(swaggen_ui_kit::text_secondary(ui))
                                        .strong(),
                                );
                            });
                            env_fixed_cell(ui, val_w, header_h, false, |ui| {
                                ui.label(
                                    egui::RichText::new(if has_override_col {
                                        "INITIAL VALUE"
                                    } else {
                                        "VALUE"
                                    })
                                    .size(10.5)
                                    .color(swaggen_ui_kit::text_secondary(ui))
                                    .strong(),
                                );
                            });
                            if has_override_col {
                                env_fixed_cell(ui, ov_w, header_h, false, |ui| {
                                    ui.label(
                                        egui::RichText::new("CURRENT (LOCAL)")
                                            .size(10.5)
                                            .color(swaggen_ui_kit::text_secondary(ui))
                                            .strong(),
                                    );
                                });
                            }
                            env_fixed_cell(ui, sec_w, header_h, true, |ui| {
                                ui.label(
                                    egui::RichText::new("SECRET")
                                        .size(10.5)
                                        .color(swaggen_ui_kit::text_secondary(ui))
                                        .strong(),
                                );
                            });
                            env_fixed_cell(ui, act_w, header_h, true, |_| {});
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
                        .inner_margin(egui::Margin::ZERO)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                                env_fixed_cell(ui, chk_w, row_h, true, |ui| {
                                    if ui.checkbox(&mut v.enabled, "").changed() {
                                        changed = true;
                                    }
                                });
                                env_fixed_cell(ui, key_w, row_h, false, |ui| {
                                    if ui
                                        .add(
                                            egui::TextEdit::singleline(&mut v.key)
                                                .id(egui::Id::unique((id_salt, "vk", i)))
                                                .hint_text("variable_name")
                                                .align(egui::Align2::LEFT_CENTER)
                                                .margin(egui::Margin::ZERO)
                                                .frame(egui::Frame::NONE)
                                                .desired_width((key_w - 20.0).max(32.0)),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                });
                                env_fixed_cell(ui, val_w, row_h, false, |ui| {
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
                                                    .hint_text("DPAPI secret key (e.g. api-token)")
                                                    .align(egui::Align2::LEFT_CENTER)
                                                    .margin(egui::Margin::ZERO)
                                                    .frame(egui::Frame::NONE)
                                                    .desired_width((val_w - 20.0).max(32.0)),
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
                                                .align(egui::Align2::LEFT_CENTER)
                                                .margin(egui::Margin::ZERO)
                                                .frame(egui::Frame::NONE)
                                                .desired_width((val_w - 20.0).max(32.0)),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                });

                                if has_override_col
                                    && let Some(ref mut ov_map) = personal_overrides
                                {
                                    env_fixed_cell(ui, ov_w, row_h, false, |ui| {
                                        let mut ov_val = ov_map
                                            .get(&v.key)
                                            .map(|o| o.value.clone())
                                            .unwrap_or_default();
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(&mut ov_val)
                                                    .id(egui::Id::unique((id_salt, "vov", i)))
                                                    .hint_text("(Same as initial)")
                                                    .align(egui::Align2::LEFT_CENTER)
                                                    .margin(egui::Margin::ZERO)
                                                    .frame(egui::Frame::NONE)
                                                    .desired_width((ov_w - 20.0).max(32.0)),
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
                                    });
                                }

                                env_fixed_cell(ui, sec_w, row_h, true, |ui| {
                                    let lbl = if v.is_secret { "🔒" } else { "○" };
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new(lbl).size(12.0).color(
                                                    if v.is_secret {
                                                        Tokens::STATUS_INFO
                                                    } else {
                                                        swaggen_ui_kit::text_secondary(ui)
                                                    },
                                                ),
                                            )
                                            .frame(false),
                                        )
                                        .on_hover_text(if v.is_secret {
                                            "Secret (Windows DPAPI) — click to switch to normal variable"
                                        } else {
                                            "Click to bind this variable to Windows DPAPI Secrets Vault"
                                        })
                                        .clicked()
                                    {
                                        v.is_secret = !v.is_secret;
                                        changed = true;
                                    }
                                });

                                env_fixed_cell(ui, act_w, row_h, true, |ui| {
                                    if swaggen_ui_kit::mini_icon_button(
                                        ui,
                                        (id_salt, "env_del_row", i),
                                        swaggen_ui_kit::MiniIconKind::Close,
                                        Tokens::TEXT_DANGER,
                                        "Delete variable row",
                                    )
                                    .clicked()
                                    {
                                        rm_idx = Some(i);
                                    }
                                });
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
                    let mut kc = false;
                    let mut vc = false;
                    egui::Frame::new()
                        .fill(swaggen_ui_kit::surface_bg(ui))
                        .inner_margin(egui::Margin::ZERO)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                                env_fixed_cell(ui, chk_w, row_h, true, |_| {});
                                env_fixed_cell(ui, key_w, row_h, false, |ui| {
                                    kc = ui
                                        .add(
                                            egui::TextEdit::singleline(&mut nk)
                                                .id(egui::Id::unique((id_salt, "vk", next_i)))
                                                .hint_text("Add new variable")
                                                .align(egui::Align2::LEFT_CENTER)
                                                .margin(egui::Margin::ZERO)
                                                .frame(egui::Frame::NONE)
                                                .desired_width((key_w - 20.0).max(32.0)),
                                        )
                                        .changed();
                                });
                                env_fixed_cell(ui, val_w, row_h, false, |ui| {
                                    vc = ui
                                        .add(
                                            egui::TextEdit::singleline(&mut nv)
                                                .id(egui::Id::unique((id_salt, "vv", next_i)))
                                                .hint_text("Value")
                                                .align(egui::Align2::LEFT_CENTER)
                                                .margin(egui::Margin::ZERO)
                                                .frame(egui::Frame::NONE)
                                                .desired_width((val_w - 20.0).max(32.0)),
                                        )
                                        .changed();
                                });
                                if has_override_col {
                                    env_fixed_cell(ui, ov_w, row_h, false, |_| {});
                                }
                                env_fixed_cell(ui, sec_w, row_h, true, |_| {});
                                env_fixed_cell(ui, act_w, row_h, true, |_| {});
                                if (kc || vc) && (!nk.is_empty() || !nv.is_empty()) {
                                    vars.push(VariableEntry::public(nk, nv));
                                    changed = true;
                                }
                            });
                        });
                }

                (x_chk_end, x_key_end, x_val_end, x_ov_end, x_sec_end)
            });

        let (x_chk_end, x_key_end, x_val_end, x_ov_end, x_sec_end) = table_resp.inner;
        let t_rect = table_resp.response.rect;
        let p = ui.painter();
        let mut xs = vec![x_chk_end, x_key_end, x_val_end];
        if has_override_col {
            xs.push(x_ov_end);
        }
        xs.push(x_sec_end);
        for x in xs {
            p.line_segment(
                [egui::pos2(x, t_rect.top()), egui::pos2(x, t_rect.bottom())],
                egui::Stroke::new(1.0, border_col),
            );
        }

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
        "Manage workspaces, software updates, and local privacy diagnostics.",
        Some(&format!("v{}", crate::updater::APP_VERSION)),
        false,
    );
    show_settings_group_tabs(app, ui);
    egui::ScrollArea::vertical().show(ui, |ui| {
        // 0. General Preferences (English + Pure White light surface only).
        swaggen_ui_kit::surface_card(ui, |ui| {
            swaggen_ui_kit::card_header(ui, "General Preferences");
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
                            "Switch or manage workspaces…",
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
            ("Ctrl + Enter", "Send active request (anywhere)"),
            ("Enter", "Send active request (when outside text inputs)"),
            ("Ctrl + S", "Save local workspace / OpenAPI spec"),
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
