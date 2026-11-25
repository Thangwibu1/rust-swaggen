//! Team Server Profiles, Internal Account Auth, Invitations, Workspace RBAC & Audit UI (P04).
use crate::{app::App, worker::Command};
use eframe::egui;
use swaggen_api_contract::{Role, permission_matrix};
use swaggen_domain::server_profiles::{ServerProfile, validate_server_url};
use swaggen_ui_kit::{NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        let header_title = match app.server_subtab {
            0 => "Connect your team server",
            1 => "Sign in & owner setup",
            2 => "Team workspaces & members",
            _ => "Sessions & security",
        };
        swaggen_ui_kit::page_header(
            ui,
            header_title,
            "Connect to your self-hosted Swaggen server over HTTPS, authenticate accounts, and administer workspace roles.",
            None,
            false,
        );

        ui.horizontal_wrapped(|ui| {
            for (idx, label) in [
                "Server Profiles",
                "Sign In & Setup",
                "Workspaces & Roles",
                "Sessions & Password",
            ]
            .iter()
            .enumerate()
            {
                if swaggen_ui_kit::underline_tab(ui, app.server_subtab == idx, label).clicked() {
                    app.server_subtab = idx;
                }
            }
        });
        ui.add_space(10.0);

        match app.server_subtab {
            0 => show_server_profiles(app, ui),
            1 => show_auth_and_bootstrap(app, ui),
            2 => show_team_workspaces(app, ui),
            _ => show_sessions_and_password(app, ui),
        }
    });
}

fn show_server_profiles(app: &mut App, ui: &mut egui::Ui) {
    swaggen_ui_kit::notice_banner(
        ui,
        NoticeKind::Info,
        "",
        "Your workspace lives on your team server. API requests run from your PC.",
    );
    ui.add_space(10.0);

    swaggen_ui_kit::surface_card(ui, |ui| {
        swaggen_ui_kit::card_header(ui, "Overview");
        if app.server_profiles.profiles.is_empty() {
            ui.label(
                egui::RichText::new("No server profiles saved yet. Add one below.")
                    .color(Tokens::TEXT_SECONDARY),
            );
        } else {
            swaggen_ui_kit::table_header(
                ui,
                &[
                    ("NAME / OPERATION", 200.0),
                    ("VALUE / DETAILS", 340.0),
                    ("STATUS", 260.0),
                ],
            );
            let mut select_id: Option<String> = None;
            let mut test_id: Option<String> = None;
            let mut remove_id: Option<String> = None;
            for prof in &app.server_profiles.profiles {
                let is_active = app.server_profiles.active_profile_id.as_deref() == Some(&prof.id);
                swaggen_ui_kit::table_row_frame(ui, |ui| {
                    if ui.selectable_label(is_active, &prof.name).clicked() {
                        select_id = Some(prof.id.clone());
                    }
                    ui.label(
                        egui::RichText::new(&prof.base_url)
                            .size(12.0)
                            .color(Tokens::TEXT_SECONDARY),
                    );
                    if prof.allow_self_signed_tls {
                        swaggen_ui_kit::pill_badge(ui, Tokens::TEXT_WARNING, "Host TLS");
                    }
                    if !prof.last_server_version.is_empty() {
                        swaggen_ui_kit::pill_badge(
                            ui,
                            if prof.mongo_ready {
                                Tokens::STATUS_SUCCESS
                            } else {
                                Tokens::TEXT_WARNING
                            },
                            &format!(
                                "v{} · {}ms · {}",
                                prof.last_server_version,
                                prof.last_latency_ms.unwrap_or(0),
                                if prof.mongo_ready { "DB ready" } else { "DB degraded" }
                            ),
                        );
                    }
                    if !prof.email.is_empty() {
                        swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_INFO, &prof.email);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if swaggen_ui_kit::secondary_button(ui, true, "Remove").clicked() {
                            remove_id = Some(prof.id.clone());
                        }
                        if swaggen_ui_kit::primary_button(ui, true, "Connect server").clicked() {
                            select_id = Some(prof.id.clone());
                            test_id = Some(prof.id.clone());
                        }
                    });
                });
                if !prof.last_error.is_empty() {
                    swaggen_ui_kit::notice_banner(
                        ui,
                        NoticeKind::Danger,
                        "Unable to reach server",
                        &prof.last_error,
                    );
                    ui.add_space(4.0);
                }
            }
            if let Some(id) = select_id {
                app.server_profiles.active_profile_id = Some(id);
                app.submit(Command::SaveServerProfiles {
                    store: app.server_profiles.clone(),
                });
            }
            if let Some(id) = test_id {
                app.submit(Command::TestServerConnection { profile_id: id });
            }
            if let Some(id) = remove_id {
                app.server_profiles.profiles.retain(|p| p.id != id);
                if app.server_profiles.active_profile_id.as_deref() == Some(&id) {
                    app.server_profiles.active_profile_id =
                        app.server_profiles.profiles.first().map(|p| p.id.clone());
                }
                app.submit(Command::SaveServerProfiles {
                    store: app.server_profiles.clone(),
                });
            }
        }
    });

    ui.add_space(10.0);
    swaggen_ui_kit::surface_card(ui, |ui| {
        swaggen_ui_kit::card_header(ui, "Configuration");
        swaggen_ui_kit::config_field_row(ui, "Profile name", "Workspace server alias", |ui| {
            ui.add(egui::TextEdit::singleline(&mut app.server_name_input).desired_width(360.0));
        });
        swaggen_ui_kit::config_field_row(ui, "Server address", "HTTPS origin of Swaggen server", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut app.server_url_input)
                    .hint_text("https://swaggen-api.ngocthang.io.vn")
                    .desired_width(360.0),
            );
            if let Err(reason) = validate_server_url(&app.server_url_input) {
                ui.label(egui::RichText::new(reason).size(11.5).color(Tokens::TEXT_WARNING));
            }
        });
        swaggen_ui_kit::config_field_row(ui, "TLS verification", "Host-scoped exception only", |ui| {
            ui.checkbox(
                &mut app.server_allow_self_signed,
                "Trust self-signed TLS certificate strictly for this host only",
            );
        });
        ui.add_space(4.0);
        if swaggen_ui_kit::primary_button(
            ui,
            !app.server_name_input.trim().is_empty() && validate_server_url(&app.server_url_input).is_ok(),
            "Save & connect server",
        )
        .clicked()
        {
            let id: String = app
                .server_name_input
                .trim()
                .to_ascii_lowercase()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
                .collect();
            let prof_id = if id.is_empty() { "team-server".into() } else { id };
            let prof = ServerProfile::new(
                prof_id.clone(),
                app.server_name_input.trim(),
                app.server_url_input.trim().trim_end_matches('/'),
                app.server_allow_self_signed,
            );
            app.server_profiles.upsert(prof);
            app.server_profiles.active_profile_id = Some(prof_id.clone());
            app.submit(Command::SaveServerProfiles {
                store: app.server_profiles.clone(),
            });
            app.submit(Command::TestServerConnection { profile_id: prof_id });
        }
    });
}

fn show_auth_and_bootstrap(app: &mut App, ui: &mut egui::Ui) {
    let Some(active) = app.server_profiles.active().cloned() else {
        swaggen_ui_kit::notice_banner(
            ui,
            NoticeKind::Info,
            "No server selected",
            "Create or select a server profile first in tab 1.",
        );
        return;
    };

    let firebase_ready = active.firebase_project_id.is_some() && active.firebase_web_api_key.is_some();
    if firebase_ready {
        swaggen_ui_kit::notice_banner(
            ui,
            NoticeKind::Success,
            "Firebase Authentication enabled",
            &format!(
                "Connected to Firebase project '{}' via {}. Google Sign-In and Email/Password tokens are verified cryptographically on the server.",
                active.firebase_project_id.as_deref().unwrap_or(""),
                active.base_url
            ),
        );
    } else {
        swaggen_ui_kit::notice_banner(
            ui,
            NoticeKind::Info,
            "Firebase Authentication",
            "Click 'Connect server' in Server Profiles to refresh Firebase capabilities, or sign in directly below (requires SWAGGEN_FIREBASE_PROJECT_ID & SWAGGEN_FIREBASE_WEB_API_KEY on the server).",
        );
    }
    ui.add_space(12.0);

    ui.columns(2, |cols| {
        swaggen_ui_kit::surface_card(&mut cols[0], |ui| {
            swaggen_ui_kit::card_header(ui, "Firebase Sign In (Google & Email)");
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
                "Continue with Google (Browser)",
            )
            .clicked()
            {
                let bt = app.auth_bootstrap_token.trim().to_owned();
                app.start_firebase_browser_login(
                    active.id.clone(),
                    if bt.is_empty() { None } else { Some(bt) },
                );
            }
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new("Or authenticate with Firebase Email & Password:")
                    .size(12.0)
                    .color(swaggen_ui_kit::text_secondary(ui)),
            );
            ui.add_space(8.0);
            let field_w = ui.available_width();
            swaggen_ui_kit::text_input(ui, &mut app.auth_email, "Email address", field_w, false);
            ui.add_space(8.0);
            swaggen_ui_kit::text_input(ui, &mut app.auth_password, "Password", field_w, true);
            if app.firebase_create_account {
                ui.add_space(8.0);
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.auth_display_name,
                    "Display name",
                    field_w,
                    false,
                );
            }
            if active.bootstrap_required {
                ui.add_space(8.0);
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.auth_bootstrap_token,
                    "Optional server bootstrap token",
                    field_w,
                    true,
                );
            }
            ui.add_space(8.0);
            ui.checkbox(
                &mut app.firebase_create_account,
                "Create new Firebase account",
            );
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                let can_fb = !app.pending
                    && !app.auth_email.trim().is_empty()
                    && !app.auth_password.is_empty();
                if swaggen_ui_kit::primary_button(
                    ui,
                    can_fb,
                    if app.firebase_create_account {
                        "Create Firebase account"
                    } else {
                        "Sign in with Firebase"
                    },
                )
                .clicked()
                {
                    let pw = std::mem::take(&mut app.auth_password);
                    let bt = app.auth_bootstrap_token.trim().to_owned();
                    app.submit(Command::FirebaseEmailAuth {
                        profile_id: active.id.clone(),
                        email: app.auth_email.trim().to_owned(),
                        password: pw,
                        display_name: app.auth_display_name.trim().to_owned(),
                        create_account: app.firebase_create_account,
                        bootstrap_token: if bt.is_empty() { None } else { Some(bt) },
                    });
                }
                if swaggen_ui_kit::secondary_button(ui, true, "Open A01 Welcome").clicked() {
                    app.show_welcome = true;
                }
            });
        });

        swaggen_ui_kit::surface_card(&mut cols[1], |ui| {
            swaggen_ui_kit::card_header(ui, "Accept Single-Use Team Invitation");
            let field_w = ui.available_width();
            swaggen_ui_kit::text_input(
                ui,
                &mut app.invite_token_input,
                "One-time invitation token",
                field_w,
                true,
            );
            ui.add_space(8.0);
            swaggen_ui_kit::text_input(ui, &mut app.auth_email, "Email address", field_w, false);
            ui.add_space(8.0);
            swaggen_ui_kit::text_input(
                ui,
                &mut app.auth_display_name,
                "Display name",
                field_w,
                false,
            );
            ui.add_space(8.0);
            swaggen_ui_kit::text_input(
                ui,
                &mut app.auth_password,
                "Password (if creating internal fallback account)",
                field_w,
                true,
            );
            ui.add_space(10.0);
            if swaggen_ui_kit::primary_button(
                ui,
                !app.pending && !app.invite_token_input.trim().is_empty(),
                "Accept invitation & join workspace",
            )
            .clicked()
            {
                let pw = std::mem::take(&mut app.auth_password);
                let tok = std::mem::take(&mut app.invite_token_input);
                app.submit(Command::AcceptServerInvitation {
                    profile_id: active.id.clone(),
                    invite_token: tok,
                    email: app.auth_email.clone(),
                    display_name: app.auth_display_name.clone(),
                    password: if pw.is_empty() { None } else { Some(pw) },
                });
            }
        });
    });

    ui.add_space(12.0);
    swaggen_ui_kit::surface_card(ui, |ui| {
        egui::CollapsingHeader::new(
            egui::RichText::new("Legacy Internal Account (Self-hosted fallback)")
                .size(13.0)
                .color(swaggen_ui_kit::text_secondary(ui)),
        )
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Email:");
                ui.add(egui::TextEdit::singleline(&mut app.auth_email).desired_width(200.0));
                ui.label("Password (min 10):");
                ui.add(
                    egui::TextEdit::singleline(&mut app.auth_password)
                        .password(true)
                        .desired_width(180.0),
                );
            });
            if active.bootstrap_required {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Display name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut app.auth_display_name).desired_width(180.0),
                    );
                    ui.label("Bootstrap token:");
                    ui.add(
                        egui::TextEdit::singleline(&mut app.auth_bootstrap_token)
                            .password(true)
                            .desired_width(180.0),
                    );
                });
                if swaggen_ui_kit::secondary_button(
                    ui,
                    !app.pending,
                    "Initialize Internal Owner",
                )
                .clicked()
                {
                    let pw = std::mem::take(&mut app.auth_password);
                    let bt = std::mem::take(&mut app.auth_bootstrap_token);
                    app.submit(Command::BootstrapServerOwner {
                        profile_id: active.id.clone(),
                        email: app.auth_email.clone(),
                        display_name: app.auth_display_name.clone(),
                        password: pw,
                        bootstrap_token: if bt.trim().is_empty() { None } else { Some(bt) },
                    });
                }
            } else if swaggen_ui_kit::secondary_button(ui, !app.pending, "Internal Password Sign In")
                .clicked()
            {
                let pw = std::mem::take(&mut app.auth_password);
                app.submit(Command::ServerLogin {
                    profile_id: active.id.clone(),
                    email: app.auth_email.clone(),
                    password: pw,
                });
            }
        });
    });
}

fn show_team_workspaces(app: &mut App, ui: &mut egui::Ui) {
    let Some(active) = app.server_profiles.active().cloned() else {
        ui.label("Select a server profile first.");
        return;
    };
    if active.session_id.is_empty() {
        swaggen_ui_kit::notice_banner(
            ui,
            NoticeKind::Warning,
            "Sign in required (A04)",
            "Sign in with your internal server account to access team workspaces.",
        );
        return;
    }

    ui.horizontal_wrapped(|ui| {
        if swaggen_ui_kit::secondary_button(ui, !app.pending, "Refresh workspaces").clicked() {
            app.submit(Command::RefreshTeamWorkspaces {
                profile_id: active.id.clone(),
            });
        }
        ui.label("Slug:");
        ui.add(egui::TextEdit::singleline(&mut app.team_ws_slug).desired_width(120.0));
        ui.label("Name:");
        ui.add(egui::TextEdit::singleline(&mut app.team_ws_name).desired_width(150.0));
        if swaggen_ui_kit::primary_button(ui, !app.pending, "+ Create team workspace").clicked() {
            app.submit(Command::CreateTeamWorkspace {
                profile_id: active.id.clone(),
                slug: app.team_ws_slug.clone(),
                name: app.team_ws_name.clone(),
            });
        }
    });
    ui.separator();

    let mut select_ws: Option<String> = None;
    for ws in &app.team_workspaces {
        let sel = app.selected_team_workspace_id.as_deref() == Some(&ws.workspace_id);
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(sel, format!("{} ({})", ws.name, ws.slug)).clicked() {
                select_ws = Some(ws.workspace_id.clone());
            }
            swaggen_ui_kit::status_badge(
                ui,
                if ws.role == Role::Viewer {
                    NoticeKind::Warning
                } else {
                    NoticeKind::Info
                },
                ws.role.as_str(),
            );
            if ws.role == Role::Viewer {
                ui.label(
                    egui::RichText::new("Read-only workspace (X03)")
                        .color(Tokens::TEXT_WARNING),
                );
            }
            ui.label(
                egui::RichText::new(format!(
                    "{} members · rev {} · mem_rev {}",
                    ws.member_count, ws.current_revision, ws.membership_revision
                ))
                .color(Tokens::TEXT_SECONDARY),
            );
        });
    }
    if let Some(wid) = select_ws {
        app.selected_team_workspace_id = Some(wid.clone());
        app.submit(Command::LoadTeamWorkspaceDetails {
            profile_id: active.id.clone(),
            workspace_id: wid,
        });
    }

    if let Some(ref details) = app.team_member_details.clone() {
        ui.add_space(6.0);
        let wid = details.workspace_id.clone();
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.strong("Invite teammates (C02 / W05)");
            ui.horizontal_wrapped(|ui| {
                ui.label("Email:");
                ui.add(egui::TextEdit::singleline(&mut app.invite_email_input).desired_width(200.0));
                egui::ComboBox::from_id_salt("invite-role")
                    .selected_text(app.invite_role.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.invite_role, Role::Admin, "admin");
                        ui.selectable_value(&mut app.invite_role, Role::Editor, "editor");
                        ui.selectable_value(&mut app.invite_role, Role::Viewer, "viewer");
                    });
                if swaggen_ui_kit::primary_button(ui, !app.pending, "Create 24h one-time invite")
                    .clicked()
                {
                    app.submit(Command::CreateServerInvitation {
                        profile_id: active.id.clone(),
                        workspace_id: wid.clone(),
                        email: app.invite_email_input.clone(),
                        role: app.invite_role,
                        expires_in_hours: 24,
                    });
                }
            });
            if !app.last_created_invite_token.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.strong("Created one-time token:");
                    ui.monospace(&app.last_created_invite_token);
                    if ui.small_button("Copy").clicked() {
                        ui.ctx().copy_text(app.last_created_invite_token.clone());
                    }
                });
            }
        });

        ui.add_space(4.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.strong(format!(
                "Workspace members (C01 / W06) · mem_rev {}",
                details.membership_revision
            ));
            for m in &details.members {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{} <{}>", m.display_name, m.email));
                    swaggen_ui_kit::status_badge(ui, NoticeKind::Info, m.role.as_str());
                    if m.role != Role::Owner {
                        for new_role in [Role::Admin, Role::Editor, Role::Viewer] {
                            if m.role != new_role && ui.small_button(format!("Set {}", new_role.as_str())).clicked() {
                                app.submit(Command::UpdateMemberRole {
                                    profile_id: active.id.clone(),
                                    workspace_id: wid.clone(),
                                    user_id: m.user_id.clone(),
                                    role: new_role,
                                    expected_membership_revision: details.membership_revision,
                                });
                            }
                        }
                    }
                });
            }

            if !details.invitations.is_empty() {
                ui.separator();
                ui.strong("Invitations");
                for inv in &details.invitations {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{} ({})", inv.email, inv.role.as_str()));
                        let state = if inv.revoked_at_unix.is_some() {
                            "revoked"
                        } else if inv.used_at_unix.is_some() {
                            "used"
                        } else {
                            "pending"
                        };
                        ui.label(state);
                        if state == "pending" && ui.small_button("Revoke (W21)").clicked() {
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

        ui.add_space(4.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.strong("Workspace security audit log (S07)");
            if app.team_audit_events.is_empty() {
                ui.label(
                    egui::RichText::new("No audit entries loaded (Admin/Owner required).")
                        .color(Tokens::TEXT_SECONDARY),
                );
            } else {
                for ev in app.team_audit_events.iter().take(25) {
                    ui.label(format!(
                        "[{}] {} · {} · {}",
                        ev.created_at_unix, ev.actor_email, ev.action, ev.detail_redacted
                    ));
                }
            }
        });
    }

    ui.add_space(4.0);
    swaggen_ui_kit::surface_card(ui, |ui| {
        ui.strong("Role Permission Matrix (WS05 / SC04)");
        for row in permission_matrix() {
            ui.label(format!(
                "{}: Owner={} · Admin={} · Editor={} · Viewer={}",
                row.action, row.owner, row.admin, row.editor, row.viewer
            ));
        }
    });
}

fn show_sessions_and_password(app: &mut App, ui: &mut egui::Ui) {
    let Some(active) = app.server_profiles.active().cloned() else {
        ui.label("Select a server profile first.");
        return;
    };
    swaggen_ui_kit::surface_card(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Active device sessions (WS03 / SC03)");
            if swaggen_ui_kit::secondary_button(ui, !app.pending, "Refresh sessions").clicked() {
                app.submit(Command::ListServerSessions {
                    profile_id: active.id.clone(),
                });
            }
            if swaggen_ui_kit::danger_button(ui, !app.pending, "Log out this device").clicked() {
                app.submit(Command::ServerLogout {
                    profile_id: active.id.clone(),
                });
            }
        });
        let mut revoke_sid: Option<String> = None;
        for s in &app.server_sessions {
            ui.horizontal_wrapped(|ui| {
                ui.label(&s.device_name);
                if s.is_current {
                    swaggen_ui_kit::status_badge(ui, NoticeKind::Success, "Current device");
                }
                if s.revoked_at_unix.is_some() {
                    swaggen_ui_kit::status_badge(ui, NoticeKind::Danger, "Revoked");
                } else if ui.small_button("Revoke session").clicked() {
                    revoke_sid = Some(s.session_id.clone());
                }
            });
        }
        if let Some(sid) = revoke_sid {
            app.submit(Command::RevokeServerSession {
                profile_id: active.id.clone(),
                session_id: sid,
            });
        }
    });

    ui.add_space(6.0);
    swaggen_ui_kit::surface_card(ui, |ui| {
        ui.strong("Change password (WS03)");
        ui.horizontal(|ui| {
            ui.label("Current password:");
            ui.add(egui::TextEdit::singleline(&mut app.pw_current_input).password(true));
        });
        ui.horizontal(|ui| {
            ui.label("New password (min 10):");
            ui.add(egui::TextEdit::singleline(&mut app.pw_new_input).password(true));
        });
        ui.checkbox(&mut app.pw_revoke_others, "Revoke all other signed-in devices");
        if swaggen_ui_kit::primary_button(ui, !app.pending, "Update password").clicked() {
            let cur = std::mem::take(&mut app.pw_current_input);
            let next = std::mem::take(&mut app.pw_new_input);
            app.submit(Command::ServerChangePassword {
                profile_id: active.id.clone(),
                current_password: cur,
                new_password: next,
                revoke_other_sessions: app.pw_revoke_others,
            });
        }
    });
}
