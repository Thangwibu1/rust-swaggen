//! Figma A01 / Welcome to Swaggen & A08 / Workspace Mode Chooser:
//! Choose between Individual (Local, no collaboration) Workspaces and Team Workspaces (Google Sign-In + Cloud Sync).
use crate::{
    app::{App, MainMode},
    worker::Command,
};
use eframe::egui;
use swaggen_domain::workspace::WorkspaceKind;
use swaggen_ui_kit::{Edge, IconKind, NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let active_prof = app.server_profiles.active().cloned();
    let signed_in = active_prof.as_ref().is_some_and(|p| !p.session_id.is_empty());
    let server_host = active_prof
        .as_ref()
        .map(|p| {
            p.base_url
                .strip_prefix("https://")
                .or_else(|| p.base_url.strip_prefix("http://"))
                .unwrap_or(&p.base_url)
                .to_owned()
        })
        .unwrap_or_else(|| "swaggen-api.ngocthang.io.vn".into());

    // Bottom 28px Figma statusbar on Launch Hub.
    egui::Panel::bottom("welcome_statusbar")
        .frame(
            egui::Frame::new()
                .fill(swaggen_ui_kit::surface_bg(ui))
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(20, 6)),
        )
        .show(ui, |ui| {
            swaggen_ui_kit::paint_edge_divider(ui, ui.max_rect(), Edge::Top);
            ui.horizontal(|ui| {
                let left: String = if app.firebase_auth_pending {
                    "Waiting for Google sign-in in your browser…".into()
                } else if app.error {
                    app.status.clone()
                } else if let Some(p) = active_prof.as_ref().filter(|p| !p.session_id.is_empty())
                {
                    format!("Signed in as {} · Choose a workspace to start", p.email)
                } else {
                    "Choose Individual (local-only) or sign in with Google for Team Workspaces".into()
                };
                ui.label(
                    egui::RichText::new(left)
                        .size(11.0)
                        .color(if app.error {
                            Tokens::TEXT_DANGER
                        } else {
                            Tokens::TEXT_PRIMARY
                        }),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("Rust engine  /  UTF-8  /  Windows")
                            .size(11.0)
                            .color(Tokens::TEXT_SECONDARY),
                    );
                });
            });
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(swaggen_ui_kit::page_bg(ui))
                .inner_margin(egui::Margin::symmetric(48, 28)),
        )
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Brand hero header (clean transparent Figma Brand/Mark + wordmark)
                ui.horizontal(|ui| {
                    swaggen_ui_kit::brand_mark(ui, 34.0);
                    ui.add_space(10.0);
                    ui.label(swaggen_ui_kit::bold_text(
                        "swaggen",
                        22.0,
                        Tokens::TEXT_PRIMARY,
                    ));
                    ui.add_space(14.0);
                    swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_SUCCESS, "Local-First");
                    swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_INFO, "Google & Mongo Sync");
                });
                ui.add_space(12.0);
                ui.label(swaggen_ui_kit::bold_text(
                    "Choose your workspace",
                    28.0,
                    Tokens::TEXT_PRIMARY,
                ));
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Start in an Individual workspace strictly on this device (no collaboration), or connect a Team workspace to sync collections and API specs.",
                    )
                    .size(13.0)
                    .color(Tokens::TEXT_SECONDARY),
                );
                ui.add_space(18.0);

                ui.columns(2, |cols| {
                    // Column 1: Individual / Local Workspace (No Collaboration)
                    let left = &mut cols[0];
                    egui::Frame::new()
                        .fill(Tokens::BG_SURFACE)
                        .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
                        .corner_radius(14)
                        .inner_margin(egui::Margin::symmetric(26, 22))
                        .show(left, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(swaggen_ui_kit::bold_text(
                                    "Individual workspace",
                                    18.0,
                                    Tokens::TEXT_PRIMARY,
                                ));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        swaggen_ui_kit::pill_badge(
                                            ui,
                                            Tokens::STATUS_SUCCESS,
                                            "Personal · No sync",
                                        );
                                    },
                                );
                            });
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(
                                    "Private workspace stored locally with Windows DPAPI encryption. No team sync or uploads.",
                                )
                                .size(12.5)
                                .color(Tokens::TEXT_SECONDARY),
                            );
                            ui.add_space(14.0);

                            let mut switch_target = None;
                            for ws_id in &app.available_workspaces {
                                let is_loaded = *ws_id == app.workspace_id;
                                egui::Frame::new()
                                    .fill(Tokens::BG_SURFACE)
                                    .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
                                    .corner_radius(8)
                                    .inner_margin(egui::Margin::symmetric(14, 10))
                                    .show(ui, |ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.vertical(|ui| {
                                                ui.label(swaggen_ui_kit::bold_text(
                                                    ws_id.as_str(),
                                                    13.5,
                                                    Tokens::TEXT_PRIMARY,
                                                ));
                                                let subtitle = if is_loaded {
                                                    format!(
                                                        "{} collection(s) · {} request(s)",
                                                        app.state.collections.len(),
                                                        app.state.requests.len()
                                                    )
                                                } else {
                                                    "Local personal workspace".to_owned()
                                                };
                                                ui.label(
                                                    egui::RichText::new(subtitle)
                                                        .size(11.5)
                                                        .color(Tokens::TEXT_SECONDARY),
                                                );
                                            });
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if swaggen_ui_kit::primary_button(
                                                        ui,
                                                        app.loaded,
                                                        "Open individual",
                                                    )
                                                    .clicked()
                                                    {
                                                        switch_target = Some((
                                                            ws_id.clone(),
                                                            is_loaded,
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
                                app.selected_team_workspace_id = None;
                                app.show_welcome = false;
                                app.navigate_to(MainMode::Workbench);
                            }

                            ui.add_space(10.0);
                            ui.label(swaggen_ui_kit::bold_text(
                                "Create new individual workspace",
                                13.0,
                                Tokens::TEXT_PRIMARY,
                            ));
                            ui.add_space(6.0);
                            let input_w = (ui.available_width() - 150.0).max(160.0);
                            ui.horizontal(|ui| {
                                swaggen_ui_kit::text_input(
                                    ui,
                                    &mut app.new_workspace_name,
                                    "e.g. My Local API",
                                    input_w,
                                    false,
                                );
                                let slug = App::slugify_workspace_id(&app.new_workspace_name);
                                if swaggen_ui_kit::secondary_button(
                                    ui,
                                    slug.is_some() && !app.new_workspace_name.trim().is_empty(),
                                    "+ Create & open",
                                )
                                .clicked()
                                    && let Some(id) = slug
                                {
                                    let display_name =
                                        std::mem::take(&mut app.new_workspace_name)
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

                    // Column 2: Team Workspace (Collaborate via Google + Swaggen Server)
                    let right = &mut cols[1];
                    egui::Frame::new()
                        .fill(Tokens::BG_SURFACE)
                        .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
                        .corner_radius(14)
                        .inner_margin(egui::Margin::symmetric(26, 22))
                        .show(right, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(swaggen_ui_kit::bold_text(
                                    "Team workspace",
                                    18.0,
                                    Tokens::TEXT_PRIMARY,
                                ));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        swaggen_ui_kit::pill_badge(
                                            ui,
                                            Tokens::STATUS_INFO,
                                            "Collaborate & sync",
                                        );
                                    },
                                );
                            });
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(
                                    "Share collections and OpenAPI contracts with teammates, resolve 3-way sync conflicts, and track comments.",
                                )
                                .size(12.5)
                                .color(Tokens::TEXT_SECONDARY),
                            );
                            ui.add_space(14.0);

                            let prof_id = active_prof
                                .as_ref()
                                .map(|p| p.id.clone())
                                .unwrap_or_else(|| "production".into());

                            if !signed_in {
                                if app.firebase_auth_pending {
                                    swaggen_ui_kit::notice_banner(
                                        ui,
                                        NoticeKind::Info,
                                        "Continue in your browser",
                                        "Finish signing in with Google in your system browser. Swaggen will receive your session automatically.",
                                    );
                                    ui.add_space(8.0);
                                    if swaggen_ui_kit::secondary_button(
                                        ui,
                                        true,
                                        "Cancel Google sign-in",
                                    )
                                    .clicked()
                                    {
                                        app.cancel_firebase_browser_login();
                                    }
                                    ui.add_space(12.0);
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
                                        egui::vec2(18.0, 18.0),
                                        egui::Sense::hover(),
                                    );
                                    swaggen_ui_kit::paint_icon(
                                        ui.painter(),
                                        rect,
                                        IconKind::ShieldCheck,
                                        Tokens::STATUS_SUCCESS,
                                    );
                                    ui.label(
                                        egui::RichText::new("Your tokens stay protected on this device.")
                                            .size(12.0)
                                            .color(Tokens::TEXT_PRIMARY),
                                    );
                                });
                            } else if let Some(ref prof) = active_prof {
                                swaggen_ui_kit::notice_banner(
                                    ui,
                                    NoticeKind::Success,
                                    "Signed in",
                                    &format!("{} ({})", prof.display_name, prof.email),
                                );
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    if swaggen_ui_kit::secondary_button(
                                        ui,
                                        !app.pending,
                                        "Refresh team list",
                                    )
                                    .clicked()
                                    {
                                        app.submit(Command::RefreshTeamWorkspaces {
                                            profile_id: prof_id.clone(),
                                        });
                                    }
                                    if swaggen_ui_kit::danger_button(ui, !app.pending, "Sign out")
                                        .clicked()
                                    {
                                        app.submit(Command::ServerLogout {
                                            profile_id: prof_id.clone(),
                                        });
                                    }
                                });
                                ui.add_space(10.0);

                                if app.team_workspaces.is_empty() {
                                    ui.label(
                                        egui::RichText::new(
                                            "No team workspaces yet. Create one below to start collaborating.",
                                        )
                                        .size(12.0)
                                        .color(Tokens::TEXT_SECONDARY),
                                    );
                                } else {
                                    let mut open_team_ws: Option<String> = None;
                                    for ws in &app.team_workspaces {
                                        egui::Frame::new()
                                            .fill(Tokens::BG_SURFACE)
                                            .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
                                            .corner_radius(8)
                                            .inner_margin(egui::Margin::symmetric(14, 10))
                                            .show(ui, |ui| {
                                                ui.set_min_width(ui.available_width());
                                                ui.horizontal(|ui| {
                                                    ui.vertical(|ui| {
                                                        ui.label(swaggen_ui_kit::bold_text(
                                                            &ws.name,
                                                            13.5,
                                                            Tokens::TEXT_PRIMARY,
                                                        ));
                                                        ui.label(
                                                            egui::RichText::new(format!(
                                                                "{} · {} members · rev {}",
                                                                ws.role.as_str(),
                                                                ws.member_count,
                                                                ws.current_revision
                                                            ))
                                                            .size(11.5)
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
                                                                "Open team",
                                                            )
                                                            .clicked()
                                                            {
                                                                open_team_ws = Some(
                                                                    ws.workspace_id.clone(),
                                                                );
                                                            }
                                                        },
                                                    );
                                                });
                                            });
                                        ui.add_space(6.0);
                                    }
                                    if let Some(wid) = open_team_ws {
                                        app.selected_team_workspace_id = Some(wid.clone());
                                        app.submit(Command::SyncNow {
                                            profile_id: prof_id.clone(),
                                            workspace_id: wid.clone(),
                                        });
                                        app.submit(Command::LoadCollabData {
                                            profile_id: prof_id.clone(),
                                            workspace_id: wid,
                                        });
                                        app.show_welcome = false;
                                        app.navigate_to(MainMode::Workbench);
                                    }
                                }

                                ui.add_space(10.0);
                                ui.label(swaggen_ui_kit::bold_text(
                                    "Create new team workspace",
                                    13.0,
                                    Tokens::TEXT_PRIMARY,
                                ));
                                ui.add_space(6.0);
                                let input_w = (ui.available_width() - 168.0).max(150.0);
                                ui.horizontal(|ui| {
                                    swaggen_ui_kit::text_input(
                                        ui,
                                        &mut app.team_ws_name,
                                        "e.g. Backend Team",
                                        input_w,
                                        false,
                                    );
                                    let slug = App::slugify_workspace_id(&app.team_ws_name);
                                    if swaggen_ui_kit::secondary_button(
                                        ui,
                                        !app.pending
                                            && slug.is_some()
                                            && !app.team_ws_name.trim().is_empty(),
                                        "+ Create team",
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

                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Team server")
                                        .size(12.0)
                                        .color(Tokens::TEXT_SECONDARY),
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(&server_host)
                                        .size(12.0)
                                        .color(Tokens::TEXT_PRIMARY),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    egui::RichText::new("Configure")
                                                        .size(11.5)
                                                        .color(Tokens::STATUS_INFO),
                                                )
                                                .fill(egui::Color32::TRANSPARENT)
                                                .stroke(egui::Stroke::NONE),
                                            )
                                            .clicked()
                                        {
                                            app.show_welcome = false;
                                            app.server_subtab = 0;
                                            app.navigate_to(MainMode::Server);
                                        }
                                    },
                                );
                            });
                        });
                });
            });
        });
}
