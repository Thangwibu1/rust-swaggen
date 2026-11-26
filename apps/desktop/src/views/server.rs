//! Streamlined Workspace Collaboration View:
//! - Google Sign-In
//! - Pending In-App Workspace Invitations (`Accept & Join` / `Decline`)
//! - Public Expiring Invite Links (`swaggen://join/...` create, copy, revoke, paste-to-join)
//! - Owner-controlled Workspace Member Management (role changes & soft-remove members)
use crate::{app::App, worker::Command};
use eframe::egui;
use swaggen_api_contract::Role;
use swaggen_domain::server_profiles::{ServerProfile, validate_server_url};
use swaggen_ui_kit::{NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        swaggen_ui_kit::page_header(
            ui,
            "Workspace Collaboration",
            "Collaborate on shared workspaces via in-app invitations, expiring invite links, and Owner member controls.",
            None,
            false,
        );

        let Some(active) = app.server_profiles.active().cloned() else {
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Warning,
                "Collaboration unavailable",
                "Collaboration profile not initialized. Restart the app to restore defaults.",
            );
            return;
        };

        if active.session_id.is_empty() {
            show_sign_in_panel(app, ui, &active.id, &active.base_url);
            return;
        }

        // Signed-in Header Card
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_SUCCESS, "Signed in");
                let who = if !active.display_name.trim().is_empty() {
                    format!("{} <{}>", active.display_name, active.email)
                } else {
                    active.email.clone()
                };
                ui.label(swaggen_ui_kit::bold_text(
                    &who,
                    13.0,
                    swaggen_ui_kit::text_primary(ui),
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if swaggen_ui_kit::danger_button(ui, !app.pending, "Sign out").clicked() {
                        app.submit(Command::ServerLogout {
                            profile_id: active.id.clone(),
                        });
                    }
                    if swaggen_ui_kit::secondary_button(ui, !app.pending, "↻ Refresh").clicked() {
                        app.submit(Command::RefreshTeamWorkspaces {
                            profile_id: active.id.clone(),
                        });
                        app.submit(Command::RefreshPendingInvitations {
                            profile_id: active.id.clone(),
                        });
                        if let Some(ref wid) = app.selected_team_workspace_id {
                            app.submit(Command::LoadTeamWorkspaceDetails {
                                profile_id: active.id.clone(),
                                workspace_id: wid.clone(),
                            });
                        }
                    }
                });
            });
        });

        ui.add_space(8.0);

        // Team Workspaces List & Create (joining other workspaces lives cleanly on the Home screen)
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Your Team Workspaces");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let slug_opt = crate::app::App::slugify_workspace_id(&app.team_ws_name);
                    if swaggen_ui_kit::primary_button(
                        ui,
                        !app.pending && slug_opt.is_some() && !app.team_ws_name.trim().is_empty(),
                        "+ Create Workspace",
                    )
                    .clicked()
                        && let Some(id) = slug_opt
                    {
                        app.submit(Command::CreateTeamWorkspace {
                            profile_id: active.id.clone(),
                            slug: id.as_str().to_owned(),
                            name: app.team_ws_name.trim().to_owned(),
                        });
                    }
                    swaggen_ui_kit::text_input(
                        ui,
                        &mut app.team_ws_name,
                        "New workspace name…",
                        190.0,
                        false,
                    );
                });
            });
            ui.add_space(6.0);

            if app.team_workspaces.is_empty() {
                ui.label(
                    egui::RichText::new("No shared workspaces yet. Create one above or join via invite.")
                        .color(Tokens::TEXT_SECONDARY),
                );
            } else {
                let mut select_ws: Option<(String, String)> = None;
                for ws in &app.team_workspaces {
                    let selected =
                        app.selected_team_workspace_id.as_deref() == Some(ws.workspace_id.as_str());
                    egui::Frame::new()
                        .fill(if selected {
                            swaggen_ui_kit::selected_bg(ui)
                        } else {
                            swaggen_ui_kit::subtle_bg(ui)
                        })
                        .stroke(egui::Stroke::new(
                            1.0,
                            if selected {
                                Tokens::STATUS_INFO
                            } else {
                                swaggen_ui_kit::border_color(ui)
                            },
                        ))
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .selectable_label(
                                        selected,
                                        swaggen_ui_kit::bold_text(
                                            &ws.name,
                                            13.0,
                                            Tokens::TEXT_PRIMARY,
                                        ),
                                    )
                                    .clicked()
                                {
                                    select_ws = Some((ws.workspace_id.clone(), ws.name.clone()));
                                }
                                swaggen_ui_kit::status_badge(
                                    ui,
                                    if ws.role == Role::Owner {
                                        NoticeKind::Success
                                    } else if ws.role == Role::Viewer {
                                        NoticeKind::Warning
                                    } else {
                                        NoticeKind::Info
                                    },
                                    ws.role.as_str(),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} member(s) · rev {}",
                                        ws.member_count, ws.current_revision
                                    ))
                                    .size(11.5)
                                    .color(Tokens::TEXT_SECONDARY),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if swaggen_ui_kit::secondary_button(
                                            ui,
                                            true,
                                            if selected {
                                                "Manage"
                                            } else {
                                                "Select & Manage"
                                            },
                                        )
                                        .clicked()
                                        {
                                            select_ws =
                                                Some((ws.workspace_id.clone(), ws.name.clone()));
                                        }
                                    },
                                );
                            });
                        });
                    ui.add_space(4.0);
                }
                if let Some((wid, wname)) = select_ws {
                    app.open_team_workspace(&active.id, &wid, &wname);
                }
            }
        });

        // 4. Selected Workspace Collaboration Details (Invites + Public Links + Owner Member Control)
        if let Some(ref details) = app.team_member_details.clone() {
            ui.add_space(8.0);
            let wid = details.workspace_id.clone();
            let caller_is_owner = details
                .members
                .iter()
                .any(|m| m.email.eq_ignore_ascii_case(&active.email) && m.role == Role::Owner);
            let caller_can_invite = details.members.iter().any(|m| {
                m.email.eq_ignore_ascii_case(&active.email)
                    && matches!(m.role, Role::Owner | Role::Admin)
            });

            if caller_can_invite {
                swaggen_ui_kit::surface_card(ui, |ui| {
                    ui.strong("Invite Collaborators");
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Option 1 · Direct In-App Invitation")
                            .size(12.0)
                            .color(Tokens::TEXT_PRIMARY)
                            .strong(),
                    );
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Email:");
                        ui.add(
                            egui::TextEdit::singleline(&mut app.invite_email_input)
                                .hint_text("teammate@example.com")
                                .desired_width(220.0),
                        );
                        egui::ComboBox::from_id_salt("invite-role")
                            .selected_text(app.invite_role.as_str())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut app.invite_role, Role::Admin, "admin");
                                ui.selectable_value(&mut app.invite_role, Role::Editor, "editor");
                                ui.selectable_value(&mut app.invite_role, Role::Viewer, "viewer");
                            });
                        if swaggen_ui_kit::primary_button(
                            ui,
                            !app.pending && !app.invite_email_input.trim().is_empty(),
                            "Send in-app invite",
                        )
                        .clicked()
                        {
                            app.submit(Command::CreateServerInvitation {
                                profile_id: active.id.clone(),
                                workspace_id: wid.clone(),
                                email: app.invite_email_input.trim().to_owned(),
                                role: app.invite_role,
                                expires_in_hours: 24,
                            });
                        }
                    });
                    if !app.last_created_invite_token.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new("Optional manual token:")
                                    .size(11.0)
                                    .color(Tokens::TEXT_SECONDARY),
                            );
                            ui.monospace(&app.last_created_invite_token);
                            if ui.small_button("Copy").clicked() {
                                ui.ctx().copy_text(app.last_created_invite_token.clone());
                            }
                        });
                    }

                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new("Option 2 · Public Expiring Invite Link")
                                .size(12.0)
                                .color(Tokens::TEXT_PRIMARY)
                                .strong(),
                        );
                        if ui.small_button("↻ Refresh").clicked() {
                            app.submit(Command::LoadPublicInviteLinks {
                                profile_id: active.id.clone(),
                                workspace_id: wid.clone(),
                            });
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Role:");
                        egui::ComboBox::from_id_salt("pub-link-role")
                            .selected_text(app.invite_link_role.as_str())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut app.invite_link_role,
                                    Role::Admin,
                                    "admin",
                                );
                                ui.selectable_value(
                                    &mut app.invite_link_role,
                                    Role::Editor,
                                    "editor",
                                );
                                ui.selectable_value(
                                    &mut app.invite_link_role,
                                    Role::Viewer,
                                    "viewer",
                                );
                            });
                        ui.label("Expires:");
                        egui::ComboBox::from_id_salt("pub-link-expiry")
                            .selected_text(format!("{}h", app.invite_link_expiry_hours))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut app.invite_link_expiry_hours,
                                    1,
                                    "1 hour",
                                );
                                ui.selectable_value(
                                    &mut app.invite_link_expiry_hours,
                                    24,
                                    "24 hours",
                                );
                                ui.selectable_value(
                                    &mut app.invite_link_expiry_hours,
                                    72,
                                    "3 days (72h)",
                                );
                                ui.selectable_value(
                                    &mut app.invite_link_expiry_hours,
                                    168,
                                    "7 days (168h)",
                                );
                            });
                        ui.label("Max uses:");
                        ui.add(
                            egui::TextEdit::singleline(&mut app.invite_link_max_uses_input)
                                .hint_text("Unlimited")
                                .desired_width(84.0),
                        );
                        if swaggen_ui_kit::primary_button(
                            ui,
                            !app.pending,
                            "Create public invite link",
                        )
                        .clicked()
                        {
                            let max_uses =
                                app.invite_link_max_uses_input.trim().parse::<u32>().ok();
                            app.submit(Command::CreatePublicInviteLink {
                                profile_id: active.id.clone(),
                                workspace_id: wid.clone(),
                                role: app.invite_link_role,
                                expires_in_hours: app.invite_link_expiry_hours,
                                max_uses,
                            });
                        }
                    });

                    if !app.public_invite_links.is_empty() {
                        ui.add_space(6.0);
                        let links = app.public_invite_links.clone();
                        for lnk in links {
                            let is_revoked = lnk.revoked_at_unix.is_some();
                            egui::Frame::new()
                                .fill(swaggen_ui_kit::subtle_bg(ui))
                                .stroke(egui::Stroke::new(
                                    1.0,
                                    swaggen_ui_kit::border_color(ui),
                                ))
                                .corner_radius(6)
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        swaggen_ui_kit::pill_badge(
                                            ui,
                                            Tokens::STATUS_INFO,
                                            lnk.role.as_str(),
                                        );
                                        swaggen_ui_kit::pill_badge(
                                            ui,
                                            if is_revoked {
                                                Tokens::TEXT_DANGER
                                            } else {
                                                Tokens::STATUS_SUCCESS
                                            },
                                            if is_revoked { "Revoked" } else { "Active" },
                                        );
                                        let uses_txt = match lnk.max_uses {
                                            Some(m) => format!("Uses: {}/{m}", lnk.uses_count),
                                            None => format!("Uses: {}/∞", lnk.uses_count),
                                        };
                                        ui.label(
                                            egui::RichText::new(uses_txt)
                                                .size(11.5)
                                                .color(Tokens::TEXT_SECONDARY),
                                        );
                                        ui.monospace(&lnk.share_link);
                                        if swaggen_ui_kit::secondary_button(
                                            ui,
                                            true,
                                            "Copy link",
                                        )
                                        .clicked()
                                        {
                                            ui.ctx().copy_text(lnk.share_link.clone());
                                            app.status = "Copied public invite link".into();
                                        }
                                        if !is_revoked
                                            && swaggen_ui_kit::danger_button(
                                                ui,
                                                !app.pending,
                                                "Revoke",
                                            )
                                            .clicked()
                                        {
                                            app.submit(Command::RevokePublicInviteLink {
                                                profile_id: active.id.clone(),
                                                workspace_id: wid.clone(),
                                                link_id: lnk.link_id.clone(),
                                            });
                                        }
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    }
                });
                ui.add_space(8.0);
            }

            // Members Card — Owner controls adding/removing members & assigning roles
            swaggen_ui_kit::surface_card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(format!(
                        "Workspace Members ({}) · mem_rev {}",
                        details.members.len(),
                        details.membership_revision
                    ));
                    if caller_is_owner {
                        swaggen_ui_kit::pill_badge(
                            ui,
                            Tokens::STATUS_SUCCESS,
                            "Owner controls enabled",
                        );
                    }
                });
                ui.add_space(4.0);
                for m in &details.members {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{} <{}>", m.display_name, m.email));
                        swaggen_ui_kit::status_badge(ui, NoticeKind::Info, m.role.as_str());
                        if caller_is_owner && m.role != Role::Owner {
                            for new_role in [Role::Admin, Role::Editor, Role::Viewer] {
                                if m.role != new_role
                                    && ui
                                        .small_button(format!("Set {}", new_role.as_str()))
                                        .clicked()
                                {
                                    app.submit(Command::UpdateMemberRole {
                                        profile_id: active.id.clone(),
                                        workspace_id: wid.clone(),
                                        user_id: m.user_id.clone(),
                                        role: new_role,
                                        expected_membership_revision: details.membership_revision,
                                    });
                                }
                            }
                            if swaggen_ui_kit::danger_button(ui, !app.pending, "Remove member")
                                .clicked()
                            {
                                app.submit(Command::RemoveWorkspaceMember {
                                    profile_id: active.id.clone(),
                                    workspace_id: wid.clone(),
                                    user_id: m.user_id.clone(),
                                });
                            }
                        }
                    });
                }

                if !details.invitations.is_empty() && caller_can_invite {
                    ui.separator();
                    ui.strong("Sent In-App Invitations");
                    for inv in &details.invitations {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!("{} ({})", inv.email, inv.role.as_str()));
                            let state = if inv.revoked_at_unix.is_some() {
                                "revoked"
                            } else if inv.used_at_unix.is_some() {
                                "accepted"
                            } else {
                                "pending"
                            };
                            ui.label(state);
                            if state == "pending" && ui.small_button("Revoke").clicked() {
                                app.submit(Command::RevokeServerInvitation {
                                    profile_id: active.id.clone(),
                                    workspace_id: wid.clone(),
                                    invitation_id: inv.invitation_id.clone(),
                                });
                            }
                        });
                    }
                }
            });
        }
    });
}

fn show_sign_in_panel(app: &mut App, ui: &mut egui::Ui, profile_id: &str, current_url: &str) {
    swaggen_ui_kit::surface_card(ui, |ui| {
        swaggen_ui_kit::card_header(ui, "Sign In to Collaborate");
        ui.label(
            egui::RichText::new(
                "Sign in with Google to create Team Workspaces, receive in-app invitations, or join via shared invite link.",
            )
            .size(12.5)
            .color(swaggen_ui_kit::text_secondary(ui)),
        );
        ui.add_space(10.0);
        if app.firebase_auth_pending {
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Info,
                "Waiting for browser sign-in",
                "Complete Google sign-in in your browser, or cancel below.",
            );
            ui.add_space(6.0);
            if swaggen_ui_kit::secondary_button(ui, true, "Cancel Google sign-in").clicked() {
                app.cancel_firebase_browser_login();
            }
            ui.add_space(8.0);
        }
        if swaggen_ui_kit::google_button(
            ui,
            !app.pending && !app.firebase_auth_pending,
            "Continue with Google",
        )
        .clicked()
        {
            app.start_firebase_browser_login(profile_id.to_owned(), None);
        }
        ui.add_space(10.0);
        egui::CollapsingHeader::new(
            egui::RichText::new("Advanced connection endpoint")
                .size(11.0)
                .color(Tokens::TEXT_SECONDARY),
        )
        .default_open(false)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!("Current collaboration endpoint: {current_url}"))
                    .size(11.0)
                    .color(Tokens::TEXT_SECONDARY),
            );
            ui.horizontal_wrapped(|ui| {
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.server_url_input,
                    "https://swaggen-api.ngocthang.io.vn",
                    280.0,
                    false,
                );
                if swaggen_ui_kit::secondary_button(
                    ui,
                    validate_server_url(&app.server_url_input).is_ok(),
                    "Update endpoint",
                )
                .clicked()
                {
                    let mut prof = ServerProfile::new(
                        profile_id,
                        "Swaggen Collaboration",
                        app.server_url_input.trim().trim_end_matches('/'),
                        false,
                    );
                    if let Some(existing) = app.server_profiles.active() {
                        prof.allow_self_signed_tls = existing.allow_self_signed_tls;
                    }
                    app.server_profiles.upsert(prof);
                    app.server_profiles.active_profile_id = Some(profile_id.to_owned());
                    app.submit(Command::SaveServerProfiles {
                        store: app.server_profiles.clone(),
                    });
                }
            });
        });
    });
}
