//! Clean, Modern Launch Hub (Home Screen):
//! - Effortless 2-column layout: Personal Workspaces (left) & Team Workspaces + Join Workspace (right)
//! - Clean top account bar, zero visual clutter
use crate::{
    app::{App, MainMode},
    worker::Command,
};
use eframe::egui;
use swaggen_domain::workspace::WorkspaceKind;
use swaggen_ui_kit::{Edge, IconKind, NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let active_prof = app.server_profiles.active().cloned();
    let signed_in = active_prof
        .as_ref()
        .is_some_and(|p| !p.session_id.is_empty());
    let prof_id = active_prof
        .as_ref()
        .map(|p| p.id.clone())
        .unwrap_or_else(|| "production".into());

    // Subtle bottom statusbar
    egui::Panel::bottom("welcome_statusbar")
        .frame(
            egui::Frame::new()
                .fill(Tokens::BG_SURFACE)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(24, 6)),
        )
        .show(ui, |ui| {
            swaggen_ui_kit::paint_edge_divider(ui, ui.max_rect(), Edge::Top);
            ui.horizontal(|ui| {
                let left = if app.firebase_auth_pending {
                    "Waiting for Google sign-in in your browser…".to_owned()
                } else if app.error && !app.status.is_empty() {
                    app.status.clone()
                } else {
                    format!("Swaggen v{} · Ready", crate::updater::APP_VERSION)
                };
                ui.label(egui::RichText::new(left).size(11.0).color(if app.error {
                    Tokens::TEXT_DANGER
                } else {
                    Tokens::TEXT_SECONDARY
                }));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("Native Rust HTTP Engine · Windows")
                            .size(11.0)
                            .color(Tokens::TEXT_SECONDARY),
                    );
                });
            });
        });

    let avail_w = ui.available_width();
    let pad_x = ((avail_w - 1040.0) * 0.5).clamp(24.0, 220.0) as i8;
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(swaggen_ui_kit::page_bg(ui))
                .inner_margin(egui::Margin::symmetric(pad_x, 28)),
        )
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Clean Top Header Bar (Logo left + Account controls right)
                ui.horizontal(|ui| {
                    swaggen_ui_kit::brand_mark(ui, 32.0);
                    ui.add_space(10.0);
                    ui.label(swaggen_ui_kit::bold_text(
                        "Swaggen",
                        20.0,
                        Tokens::TEXT_PRIMARY,
                    ));

                    if let Some(prof) = active_prof.as_ref().filter(|p| !p.session_id.is_empty())
                    {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if swaggen_ui_kit::danger_button(ui, !app.pending, "Sign out")
                                .clicked()
                            {
                                app.submit(Command::ServerLogout {
                                    profile_id: prof_id.clone(),
                                });
                            }
                            if swaggen_ui_kit::secondary_button(ui, !app.pending, "↻ Refresh")
                                .clicked()
                            {
                                app.submit(Command::RefreshTeamWorkspaces {
                                    profile_id: prof_id.clone(),
                                });
                                app.submit(Command::RefreshPendingInvitations {
                                    profile_id: prof_id.clone(),
                                });
                            }
                            swaggen_ui_kit::pill_badge(
                                ui,
                                Tokens::STATUS_SUCCESS,
                                &prof.email,
                            );
                        });
                    }
                });

                ui.add_space(18.0);
                ui.label(swaggen_ui_kit::bold_text(
                    "Choose or Join a Workspace",
                    25.0,
                    Tokens::TEXT_PRIMARY,
                ));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Open a private workspace stored on this PC, or collaborate in a shared Team Workspace.",
                    )
                    .size(12.5)
                    .color(Tokens::TEXT_SECONDARY),
                );
                ui.add_space(20.0);

                let cols_count = if ui.available_width() < 700.0 { 1 } else { 2 };
                ui.columns(cols_count, |cols| {
                    // ================= LEFT CARD: PERSONAL WORKSPACES =================
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(0xFA, 0xFE, 0xFC))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(0xC5, 0xE6, 0xD8),
                        ))
                        .corner_radius(12)
                        .inner_margin(egui::Margin::symmetric(22, 20))
                        .show(&mut cols[0], |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(swaggen_ui_kit::bold_text(
                                    "Personal Workspaces",
                                    16.5,
                                    Tokens::TEXT_PRIMARY,
                                ));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        swaggen_ui_kit::pill_badge(
                                            ui,
                                            Tokens::STATUS_SUCCESS,
                                            "Local PC",
                                        );
                                    },
                                );
                            });
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(
                                    "Stored strictly on this device with Windows DPAPI secret protection.",
                                )
                                .size(12.0)
                                .color(Tokens::TEXT_SECONDARY),
                            );
                            ui.add_space(14.0);

                            let mut switch_target = None;
                            let local_list: Vec<(swaggen_domain::LocalId, String)> =
                                if app.workspace_summaries.is_empty() {
                                    app.available_workspaces
                                        .iter()
                                        .filter(|w| {
                                            !w.as_str().starts_with("team-")
                                                && w.as_str() != "server-auth"
                                        })
                                        .map(|id| {
                                            let label = if id.as_str() == "personal" {
                                                "Personal".to_owned()
                                            } else {
                                                id.as_str().to_owned()
                                            };
                                            (id.clone(), label)
                                        })
                                        .collect()
                                } else {
                                    app.workspace_summaries
                                        .iter()
                                        .filter(|w| {
                                            w.kind == WorkspaceKind::Personal
                                                && !w.id.as_str().starts_with("team-")
                                                && w.id.as_str() != "server-auth"
                                        })
                                        .map(|w| {
                                            let label = if w.id.as_str() == "personal"
                                                && w.name == "personal"
                                            {
                                                "Personal".to_owned()
                                            } else {
                                                w.name.clone()
                                            };
                                            (w.id.clone(), label)
                                        })
                                        .collect()
                                };

                            for (ws_id, ws_label) in &local_list {
                                let is_loaded = *ws_id == app.workspace_id
                                    && app.selected_team_workspace_id.is_none();
                                let accent = swaggen_ui_kit::workspace_accent(ws_id.as_str(), false);
                                egui::Frame::new()
                                    .fill(if is_loaded {
                                        accent.bg_active
                                    } else {
                                        accent.bg_tint
                                    })
                                    .stroke(egui::Stroke::new(
                                        if is_loaded { 1.5 } else { 1.0 },
                                        accent.border,
                                    ))
                                    .corner_radius(8)
                                    .inner_margin(egui::Margin::symmetric(14, 10))
                                    .show(ui, |ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    let (dot_r, _) = ui.allocate_exact_size(
                                                        egui::vec2(10.0, 10.0),
                                                        egui::Sense::hover(),
                                                    );
                                                    ui.painter().circle_filled(
                                                        dot_r.center(),
                                                        4.5,
                                                        accent.dot,
                                                    );
                                                    ui.label(swaggen_ui_kit::bold_text(
                                                        ws_label,
                                                        13.5,
                                                        Tokens::TEXT_PRIMARY,
                                                    ));
                                                    if is_loaded {
                                                        swaggen_ui_kit::pill_badge(
                                                            ui,
                                                            accent.dot,
                                                            "Active",
                                                        );
                                                    }
                                                });
                                                let subtitle = if *ws_id == app.workspace_id {
                                                    format!(
                                                        "{} collection(s) · {} request(s)",
                                                        app.state.collections.len(),
                                                        app.state.requests.len()
                                                    )
                                                } else {
                                                    "Local workspace".to_owned()
                                                };
                                                ui.label(
                                                    egui::RichText::new(subtitle)
                                                        .size(11.0)
                                                        .color(Tokens::TEXT_SECONDARY),
                                                );
                                            });
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if swaggen_ui_kit::primary_button(
                                                        ui,
                                                        app.loaded,
                                                        "Open",
                                                    )
                                                    .clicked()
                                                    {
                                                        switch_target = Some((
                                                            ws_id.clone(),
                                                            *ws_id == app.workspace_id,
                                                        ));
                                                    }
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(6.0);
                            }

                            if let Some((id, already_loaded)) = switch_target {
                                if !already_loaded {
                                    app.submit(Command::SwitchWorkspace { id });
                                }
                                app.state.meta.kind = WorkspaceKind::Personal;
                                app.selected_team_workspace_id = None;
                                app.show_welcome = false;
                                app.navigate_to(MainMode::Workbench);
                            }

                            ui.add_space(12.0);
                            ui.separator();
                            ui.add_space(10.0);
                            ui.label(swaggen_ui_kit::bold_text(
                                "New personal workspace",
                                12.5,
                                Tokens::TEXT_PRIMARY,
                            ));
                            ui.add_space(5.0);
                            let input_w = (ui.available_width() - 120.0).max(140.0);
                            ui.horizontal(|ui| {
                                swaggen_ui_kit::text_input(
                                    ui,
                                    &mut app.new_workspace_name,
                                    "Workspace name (e.g. Payment API)",
                                    input_w,
                                    false,
                                );
                                let slug = App::slugify_workspace_id(&app.new_workspace_name);
                                if swaggen_ui_kit::secondary_button(
                                    ui,
                                    slug.is_some() && !app.new_workspace_name.trim().is_empty(),
                                    "+ Create",
                                )
                                .clicked()
                                    && let Some(id) = slug
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
                                    app.show_welcome = false;
                                    app.navigate_to(MainMode::Workbench);
                                }
                            });
                        });

                    if cols.len() == 1 {
                        cols[0].add_space(14.0);
                    }
                    let right_idx = 1 % cols.len();

                    // ================= RIGHT CARD: TEAM WORKSPACES & JOIN =================
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(0xFA, 0xFC, 0xFF))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgb(0xC7, 0xD7, 0xFA),
                        ))
                        .corner_radius(12)
                        .inner_margin(egui::Margin::symmetric(22, 20))
                        .show(&mut cols[right_idx], |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(swaggen_ui_kit::bold_text(
                                    "Team Workspaces",
                                    16.5,
                                    Tokens::TEXT_PRIMARY,
                                ));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        swaggen_ui_kit::pill_badge(
                                            ui,
                                            Tokens::STATUS_INFO,
                                            "Collaboration",
                                        );
                                    },
                                );
                            });
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(
                                    "Join a teammate's workspace via invite or link, and publish changes with Ctrl+S.",
                                )
                                .size(12.0)
                                .color(Tokens::TEXT_SECONDARY),
                            );
                            ui.add_space(14.0);

                            if !signed_in {
                                if app.firebase_auth_pending {
                                    swaggen_ui_kit::notice_banner(
                                        ui,
                                        NoticeKind::Info,
                                        "Continue in your browser",
                                        "Complete Google Sign-In in your browser window to connect.",
                                    );
                                    ui.add_space(8.0);
                                    if swaggen_ui_kit::secondary_button(
                                        ui,
                                        true,
                                        "Cancel sign-in",
                                    )
                                    .clicked()
                                    {
                                        app.cancel_firebase_browser_login();
                                    }
                                    ui.add_space(10.0);
                                }

                                if swaggen_ui_kit::google_button(
                                    ui,
                                    !app.pending && !app.firebase_auth_pending,
                                    "Continue with Google",
                                )
                                .clicked()
                                {
                                    let bt = app.auth_bootstrap_token.trim().to_owned();
                                    app.start_firebase_browser_login(
                                        prof_id.clone(),
                                        if bt.is_empty() { None } else { Some(bt) },
                                    );
                                }

                                if app.error && !app.status.is_empty() {
                                    ui.add_space(10.0);
                                    swaggen_ui_kit::notice_banner(
                                        ui,
                                        NoticeKind::Danger,
                                        "Sign-in failed",
                                        &app.status,
                                    );
                                }

                                ui.add_space(14.0);
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(16.0, 16.0),
                                        egui::Sense::hover(),
                                    );
                                    swaggen_ui_kit::paint_icon(
                                        ui.painter(),
                                        rect,
                                        IconKind::ShieldCheck,
                                        Tokens::STATUS_SUCCESS,
                                    );
                                    ui.label(
                                        egui::RichText::new(
                                            "Sign in with Google to join or create Team Workspaces.",
                                        )
                                        .size(11.5)
                                        .color(Tokens::TEXT_SECONDARY),
                                    );
                                });
                            } else {
                                // 1. Pending In-App Invitations right on Home!
                                if !app.pending_invitations.is_empty() {
                                    ui.label(swaggen_ui_kit::bold_text(
                                        &format!(
                                            "Pending Invitations ({})",
                                            app.pending_invitations.len()
                                        ),
                                        12.5,
                                        Tokens::STATUS_INFO,
                                    ));
                                    ui.add_space(5.0);
                                    let inv_list = app.pending_invitations.clone();
                                    for inv in inv_list {
                                        egui::Frame::new()
                                            .fill(swaggen_ui_kit::selected_bg(ui))
                                            .stroke(egui::Stroke::new(1.0, Tokens::STATUS_INFO))
                                            .corner_radius(8)
                                            .inner_margin(egui::Margin::symmetric(12, 8))
                                            .show(ui, |ui| {
                                                ui.horizontal_wrapped(|ui| {
                                                    ui.vertical(|ui| {
                                                        ui.label(swaggen_ui_kit::bold_text(
                                                            &inv.workspace_name,
                                                            13.0,
                                                            Tokens::TEXT_PRIMARY,
                                                        ));
                                                        ui.label(
                                                            egui::RichText::new(format!(
                                                                "Invited role: {}",
                                                                inv.role.as_str()
                                                            ))
                                                            .size(11.0)
                                                            .color(Tokens::TEXT_SECONDARY),
                                                        );
                                                    });
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            if swaggen_ui_kit::secondary_button(
                                                                ui,
                                                                !app.pending,
                                                                "Decline",
                                                            )
                                                            .clicked()
                                                            {
                                                                app.submit(
                                                                    Command::DeclineInAppInvitation {
                                                                        profile_id: prof_id.clone(),
                                                                        invitation_id: inv
                                                                            .invitation_id
                                                                            .clone(),
                                                                    },
                                                                );
                                                            }
                                                            if swaggen_ui_kit::primary_button(
                                                                ui,
                                                                !app.pending,
                                                                "Accept & Join",
                                                            )
                                                            .clicked()
                                                            {
                                                                app.submit(
                                                                    Command::AcceptInAppInvitation {
                                                                        profile_id: prof_id.clone(),
                                                                        invitation_id: inv
                                                                            .invitation_id
                                                                            .clone(),
                                                                    },
                                                                );
                                                            }
                                                        },
                                                    );
                                                });
                                            });
                                        ui.add_space(6.0);
                                    }
                                    ui.add_space(6.0);
                                }

                                // 2. Existing Team Workspaces
                                if app.team_workspaces.is_empty() {
                                    egui::Frame::new()
                                        .fill(swaggen_ui_kit::subtle_bg(ui))
                                        .corner_radius(8)
                                        .inner_margin(egui::Margin::symmetric(12, 10))
                                        .show(ui, |ui| {
                                            ui.set_min_width(ui.available_width());
                                            ui.label(
                                                egui::RichText::new(
                                                    "You haven't joined any Team Workspace yet. Paste an invite link or create one below.",
                                                )
                                                .size(11.5)
                                                .color(Tokens::TEXT_SECONDARY),
                                            );
                                        });
                                    ui.add_space(6.0);
                                } else {
                                    let mut open_team_ws = None;
                                    for ws in &app.team_workspaces {
                                        let is_cur_team = app
                                            .selected_team_workspace_id
                                            .as_deref()
                                            == Some(ws.workspace_id.as_str());
                                        let accent = swaggen_ui_kit::workspace_accent(
                                            &ws.workspace_id,
                                            true,
                                        );
                                        egui::Frame::new()
                                            .fill(if is_cur_team {
                                                accent.bg_active
                                            } else {
                                                accent.bg_tint
                                            })
                                            .stroke(egui::Stroke::new(
                                                if is_cur_team { 1.5 } else { 1.0 },
                                                accent.border,
                                            ))
                                            .corner_radius(8)
                                            .inner_margin(egui::Margin::symmetric(14, 10))
                                            .show(ui, |ui| {
                                                ui.set_min_width(ui.available_width());
                                                ui.horizontal(|ui| {
                                                    ui.vertical(|ui| {
                                                        ui.horizontal(|ui| {
                                                            let (dot_r, _) = ui.allocate_exact_size(
                                                                egui::vec2(10.0, 10.0),
                                                                egui::Sense::hover(),
                                                            );
                                                            ui.painter().circle_filled(
                                                                dot_r.center(),
                                                                4.5,
                                                                accent.dot,
                                                            );
                                                            ui.label(swaggen_ui_kit::bold_text(
                                                                &ws.name,
                                                                13.5,
                                                                Tokens::TEXT_PRIMARY,
                                                            ));
                                                            swaggen_ui_kit::pill_badge(
                                                                ui,
                                                                accent.dot,
                                                                ws.role.as_str(),
                                                            );
                                                            if is_cur_team {
                                                                swaggen_ui_kit::pill_badge(
                                                                    ui,
                                                                    Tokens::STATUS_SUCCESS,
                                                                    "Active",
                                                                );
                                                            }
                                                        });
                                                        ui.label(
                                                            egui::RichText::new(format!(
                                                                "{} member(s)",
                                                                ws.member_count
                                                            ))
                                                            .size(11.0)
                                                            .color(Tokens::TEXT_SECONDARY),
                                                        );
                                                    });
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            if swaggen_ui_kit::primary_button(
                                                                ui,
                                                                !app.pending,
                                                                "Open",
                                                            )
                                                            .clicked()
                                                            {
                                                                open_team_ws = Some((
                                                                    ws.workspace_id.clone(),
                                                                    ws.name.clone(),
                                                                ));
                                                            }
                                                        },
                                                    );
                                                });
                                            });
                                        ui.add_space(6.0);
                                    }
                                    if let Some((wid, wname)) = open_team_ws {
                                        app.open_team_workspace(&prof_id, &wid, &wname);
                                        app.show_welcome = false;
                                        app.navigate_to(MainMode::Workbench);
                                    }
                                }

                                // 3. Join Another Workspace via Invite Link (Strictly on Home!)
                                ui.add_space(10.0);
                                ui.separator();
                                ui.add_space(10.0);
                                ui.label(swaggen_ui_kit::bold_text(
                                    "Join another workspace",
                                    12.5,
                                    Tokens::TEXT_PRIMARY,
                                ));
                                ui.add_space(5.0);
                                let link_w = (ui.available_width() - 100.0).max(140.0);
                                ui.horizontal(|ui| {
                                    let link_resp = swaggen_ui_kit::text_input(
                                        ui,
                                        &mut app.invite_link_paste_input,
                                        "Paste swaggen://join/... invite link",
                                        link_w,
                                        false,
                                    );
                                    let trigger_join = swaggen_ui_kit::primary_button(
                                        ui,
                                        !app.pending
                                            && !app.invite_link_paste_input.trim().is_empty(),
                                        "Join",
                                    )
                                    .clicked()
                                        || (link_resp.lost_focus()
                                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                            && !app.invite_link_paste_input.trim().is_empty());
                                    if trigger_join {
                                        app.submit(Command::JoinByPublicInviteLink {
                                            profile_id: prof_id.clone(),
                                            link_or_code: app
                                                .invite_link_paste_input
                                                .trim()
                                                .to_owned(),
                                        });
                                    }
                                });

                                // 4. Create New Team Workspace
                                ui.add_space(10.0);
                                ui.label(swaggen_ui_kit::bold_text(
                                    "New team workspace",
                                    12.5,
                                    Tokens::TEXT_PRIMARY,
                                ));
                                ui.add_space(5.0);
                                let create_w = (ui.available_width() - 115.0).max(140.0);
                                ui.horizontal(|ui| {
                                    swaggen_ui_kit::text_input(
                                        ui,
                                        &mut app.team_ws_name,
                                        "Workspace name (e.g. Core API Team)",
                                        create_w,
                                        false,
                                    );
                                    let slug = App::slugify_workspace_id(&app.team_ws_name);
                                    if swaggen_ui_kit::secondary_button(
                                        ui,
                                        !app.pending
                                            && slug.is_some()
                                            && !app.team_ws_name.trim().is_empty(),
                                        "+ Create",
                                    )
                                    .clicked()
                                        && let Some(id) = slug
                                    {
                                        let name = app.team_ws_name.trim().to_owned();
                                        app.team_ws_slug = id.as_str().to_owned();
                                        app.submit(Command::CreateTeamWorkspace {
                                            profile_id: prof_id.clone(),
                                            slug: app.team_ws_slug.clone(),
                                            name,
                                        });
                                    }
                                });
                            }
                        });
                });
            });
        });
}
