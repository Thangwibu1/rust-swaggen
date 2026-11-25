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
            let mut select_id: Option<String> = None;
            let mut test_id: Option<String> = None;
            let mut remove_id: Option<String> = None;
            for prof in &app.server_profiles.profiles {
                let is_active = app.server_profiles.active_profile_id.as_deref() == Some(&prof.id);
                egui::Frame::new()
                    .fill(swaggen_ui_kit::subtle_bg(ui))
                    .stroke(egui::Stroke::new(
                        1.0,
                        if is_active {
                            Tokens::STATUS_INFO
                        } else {
                            swaggen_ui_kit::border_color(ui)
                        },
                    ))
                    .corner_radius(8)
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .selectable_label(
                                    is_active,
                                    swaggen_ui_kit::bold_text(
                                        &prof.name,
                                        13.5,
                                        Tokens::TEXT_PRIMARY,
                                    ),
                                )
                                .clicked()
                            {
                                select_id = Some(prof.id.clone());
                            }
                            if is_active {
                                swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_INFO, "Active");
                            }
                            ui.label(
                                egui::RichText::new(&prof.base_url)
                                    .size(12.0)
                                    .monospace()
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
                                        if prof.mongo_ready {
                                            "DB ready"
                                        } else {
                                            "DB degraded"
                                        }
                                    ),
                                );
                            }
                            if !prof.email.is_empty() {
                                swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_INFO, &prof.email);
                            }
                        });
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            if swaggen_ui_kit::primary_button(ui, true, "Connect server").clicked()
                            {
                                select_id = Some(prof.id.clone());
                                test_id = Some(prof.id.clone());
                            }
                            if !is_active
                                && swaggen_ui_kit::secondary_button(ui, true, "Set active")
                                    .clicked()
                            {
                                select_id = Some(prof.id.clone());
                            }
                            if swaggen_ui_kit::secondary_button(ui, true, "Remove").clicked() {
                                remove_id = Some(prof.id.clone());
                            }
                        });
                    });
                ui.add_space(6.0);
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
        swaggen_ui_kit::config_field_row(
            ui,
            "Server address",
            "HTTPS origin of Swaggen server",
            |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.server_url_input)
                        .hint_text("https://swaggen-api.ngocthang.io.vn")
                        .desired_width(360.0),
                );
                if let Err(reason) = validate_server_url(&app.server_url_input) {
                    ui.label(
                        egui::RichText::new(reason)
                            .size(11.5)
                            .color(Tokens::TEXT_WARNING),
                    );
                }
            },
        );
        swaggen_ui_kit::config_field_row(
            ui,
            "TLS verification",
            "Host-scoped exception only",
            |ui| {
                ui.checkbox(
                    &mut app.server_allow_self_signed,
                    "Trust self-signed TLS certificate strictly for this host only",
                );
            },
        );
        ui.add_space(4.0);
        if swaggen_ui_kit::primary_button(
            ui,
            !app.server_name_input.trim().is_empty()
                && validate_server_url(&app.server_url_input).is_ok(),
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
            let prof_id = if id.is_empty() {
                "team-server".into()
            } else {
                id
            };
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
            app.submit(Command::TestServerConnection {
                profile_id: prof_id,
            });
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

    let firebase_ready =
        active.firebase_project_id.is_some() && active.firebase_web_api_key.is_some();
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

    let auth_cols = if ui.available_width() < 680.0 { 1 } else { 2 };
    ui.columns(auth_cols, |cols| {
        swaggen_ui_kit::surface_card(&mut cols[0], |ui| {
            swaggen_ui_kit::card_header(ui, "Sign In with Google");
            ui.label(
                egui::RichText::new(
                    "Sign in or automatically create your Swaggen account via Google (Firebase Auth).",
                )
                .size(12.0)
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
                let bt = app.auth_bootstrap_token.trim().to_owned();
                app.start_firebase_browser_login(
                    active.id.clone(),
                    if bt.is_empty() { None } else { Some(bt) },
                );
            }
            ui.add_space(12.0);
            if swaggen_ui_kit::secondary_button(ui, true, "Open Welcome Sign-In").clicked() {
                app.show_welcome = true;
            }
        });

        if cols.len() == 1 {
            cols[0].add_space(12.0);
        }
        let right_idx = 1 % cols.len();
        swaggen_ui_kit::surface_card(&mut cols[right_idx], |ui| {
            swaggen_ui_kit::card_header(ui, "In-App Workspace Invitations");
            ui.label(
                egui::RichText::new(
                    "Invitations are delivered straight inside Swaggen (no email needed). Sign in above to see invitations sent to your account.",
                )
                .size(11.5)
                .color(swaggen_ui_kit::text_secondary(ui)),
            );
            ui.add_space(8.0);
            if !active.session_id.is_empty() {
                ui.horizontal(|ui| {
                    if swaggen_ui_kit::secondary_button(ui, !app.pending, "Check invitations")
                        .clicked()
                    {
                        app.submit(Command::RefreshPendingInvitations {
                            profile_id: active.id.clone(),
                        });
                    }
                    ui.label(
                        egui::RichText::new(format!("Account: {}", active.email))
                            .size(11.0)
                            .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                });
                ui.add_space(8.0);
                if app.pending_invitations.is_empty() {
                    ui.label(
                        egui::RichText::new("No pending workspace invitations.")
                            .size(12.0)
                            .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                } else {
                    let invs = app.pending_invitations.clone();
                    for inv in invs {
                        swaggen_ui_kit::table_row_frame(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.strong(&inv.workspace_name);
                                swaggen_ui_kit::pill_badge(
                                    ui,
                                    Tokens::STATUS_INFO,
                                    inv.role.as_str(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if swaggen_ui_kit::secondary_button(
                                            ui,
                                            !app.pending,
                                            "Decline",
                                        )
                                        .clicked()
                                        {
                                            app.submit(Command::DeclineInAppInvitation {
                                                profile_id: active.id.clone(),
                                                invitation_id: inv.invitation_id.clone(),
                                            });
                                        }
                                        if swaggen_ui_kit::primary_button(
                                            ui,
                                            !app.pending,
                                            "✓ Join",
                                        )
                                        .clicked()
                                        {
                                            app.submit(Command::AcceptInAppInvitation {
                                                profile_id: active.id.clone(),
                                                invitation_id: inv.invitation_id.clone(),
                                            });
                                        }
                                    },
                                );
                            });
                        });
                    }
                }
                ui.add_space(8.0);
            }

            if !active.session_id.is_empty() {
                swaggen_ui_kit::surface_card(ui, |ui| {
                    ui.label(swaggen_ui_kit::bold_text(
                        "Join workspace via public invite link",
                        13.0,
                        Tokens::TEXT_PRIMARY,
                    ));
                    ui.label(
                        egui::RichText::new(
                            "Paste a shared swaggen://join/... link or token shared by a workspace owner.",
                        )
                        .size(11.5)
                        .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                    ui.add_space(6.0);
                    let input_w = (ui.available_width() - 140.0).max(180.0);
                    ui.horizontal_wrapped(|ui| {
                        swaggen_ui_kit::text_input(
                            ui,
                            &mut app.invite_link_paste_input,
                            "swaggen://join/<workspace>/<token>",
                            input_w,
                            false,
                        );
                        if swaggen_ui_kit::primary_button(
                            ui,
                            !app.pending && !app.invite_link_paste_input.trim().is_empty(),
                            "Join via link",
                        )
                        .clicked()
                        {
                            app.submit(Command::JoinByPublicInviteLink {
                                profile_id: active.id.clone(),
                                link_or_code: app.invite_link_paste_input.trim().to_owned(),
                            });
                        }
                    });
                });
                ui.add_space(8.0);
            }

            egui::CollapsingHeader::new(
                egui::RichText::new("Join with manual invite code (optional fallback)")
                    .size(12.0)
                    .color(swaggen_ui_kit::text_secondary(ui)),
            )
            .default_open(false)
            .show(ui, |ui| {
                let field_w = ui.available_width();
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.invite_token_input,
                    "In-app invite code",
                    field_w,
                    true,
                );
                ui.add_space(6.0);
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.auth_email,
                    "Invited account email",
                    field_w,
                    false,
                );
                ui.add_space(6.0);
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.auth_display_name,
                    "Display name",
                    field_w,
                    false,
                );
                ui.add_space(6.0);
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.auth_password,
                    "Password (if creating internal account)",
                    field_w,
                    true,
                );
                ui.add_space(8.0);
                if swaggen_ui_kit::primary_button(
                    ui,
                    !app.pending && !app.invite_token_input.trim().is_empty(),
                    "Accept code & join workspace",
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
                if swaggen_ui_kit::secondary_button(ui, !app.pending, "Initialize Internal Owner")
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
            } else if swaggen_ui_kit::secondary_button(
                ui,
                !app.pending,
                "Internal Password Sign In",
            )
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
            "Sign in required",
            "Sign in with Google to access and manage team workspaces.",
        );
        return;
    }

    ui.horizontal_wrapped(|ui| {
        if swaggen_ui_kit::secondary_button(ui, !app.pending, "Refresh workspaces").clicked() {
            app.submit(Command::RefreshTeamWorkspaces {
                profile_id: active.id.clone(),
            });
            app.submit(Command::RefreshPendingInvitations {
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

    ui.add_space(6.0);
    swaggen_ui_kit::surface_card(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Join via public invite link:");
            let w = (ui.available_width() - 320.0).clamp(180.0, 380.0);
            swaggen_ui_kit::text_input(
                ui,
                &mut app.invite_link_paste_input,
                "Paste swaggen://join/<workspace>/<token>",
                w,
                false,
            );
            if swaggen_ui_kit::secondary_button(
                ui,
                !app.pending && !app.invite_link_paste_input.trim().is_empty(),
                "Join via link",
            )
            .clicked()
            {
                app.submit(Command::JoinByPublicInviteLink {
                    profile_id: active.id.clone(),
                    link_or_code: app.invite_link_paste_input.trim().to_owned(),
                });
            }
        });
    });

    if !app.pending_invitations.is_empty() {
        ui.add_space(6.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.strong(format!(
                "Your Pending In-App Invitations ({})",
                app.pending_invitations.len()
            ));
            let invs = app.pending_invitations.clone();
            for inv in invs {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(&inv.workspace_name);
                    swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_INFO, inv.role.as_str());
                    if swaggen_ui_kit::primary_button(ui, !app.pending, "✓ Accept & Join").clicked()
                    {
                        app.submit(Command::AcceptInAppInvitation {
                            profile_id: active.id.clone(),
                            invitation_id: inv.invitation_id.clone(),
                        });
                    }
                    if swaggen_ui_kit::secondary_button(ui, !app.pending, "Decline").clicked() {
                        app.submit(Command::DeclineInAppInvitation {
                            profile_id: active.id.clone(),
                            invitation_id: inv.invitation_id.clone(),
                        });
                    }
                });
            }
        });
    }
    ui.separator();

    let mut select_ws: Option<String> = None;
    for ws in &app.team_workspaces {
        let sel = app.selected_team_workspace_id.as_deref() == Some(&ws.workspace_id);
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(sel, format!("{} ({})", ws.name, ws.slug))
                .clicked()
            {
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
                ui.label(egui::RichText::new("Read-only workspace").color(Tokens::TEXT_WARNING));
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
            ui.strong("Invite teammate directly in-app (no email required)");
            ui.label(
                egui::RichText::new(
                    "Sent invitations appear immediately inside your teammate's Swaggen app when signed in.",
                )
                .size(11.5)
                .color(swaggen_ui_kit::text_secondary(ui)),
            );
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("User email:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.invite_email_input)
                        .hint_text("teammate@example.com")
                        .desired_width(200.0),
                );
                egui::ComboBox::from_id_salt("invite-role")
                    .selected_text(app.invite_role.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.invite_role, Role::Admin, "admin");
                        ui.selectable_value(&mut app.invite_role, Role::Editor, "editor");
                        ui.selectable_value(&mut app.invite_role, Role::Viewer, "viewer");
                    });
                if swaggen_ui_kit::primary_button(ui, !app.pending, "Send in-app invite").clicked()
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
                    ui.label(
                        egui::RichText::new("In-app invitation ready · Optional manual code:")
                            .size(11.0)
                            .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                    ui.monospace(&app.last_created_invite_token);
                    if ui.small_button("Copy").clicked() {
                        ui.ctx().copy_text(app.last_created_invite_token.clone());
                    }
                });
            }
        });

        ui.add_space(6.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("Method 2 · Public expiring invite links");
                if ui.small_button("↻ Refresh links").clicked() {
                    app.submit(Command::LoadPublicInviteLinks {
                        profile_id: active.id.clone(),
                        workspace_id: wid.clone(),
                    });
                }
            });
            ui.label(
                egui::RichText::new(
                    "Generate a time-limited swaggen://join/... link that any signed-in teammate can paste to join this workspace.",
                )
                .size(11.5)
                .color(swaggen_ui_kit::text_secondary(ui)),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Role:");
                egui::ComboBox::from_id_salt("pub-link-role")
                    .selected_text(app.invite_link_role.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.invite_link_role, Role::Admin, "admin");
                        ui.selectable_value(&mut app.invite_link_role, Role::Editor, "editor");
                        ui.selectable_value(&mut app.invite_link_role, Role::Viewer, "viewer");
                    });
                ui.label("Expires:");
                egui::ComboBox::from_id_salt("pub-link-expiry")
                    .selected_text(format!("{}h", app.invite_link_expiry_hours))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.invite_link_expiry_hours, 1, "1 hour");
                        ui.selectable_value(&mut app.invite_link_expiry_hours, 24, "24 hours");
                        ui.selectable_value(&mut app.invite_link_expiry_hours, 72, "3 days (72h)");
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
                if swaggen_ui_kit::primary_button(ui, !app.pending, "Create public invite link")
                    .clicked()
                {
                    let max_uses = app.invite_link_max_uses_input.trim().parse::<u32>().ok();
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
                        .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                swaggen_ui_kit::pill_badge(
                                    ui,
                                    Tokens::STATUS_INFO,
                                    lnk.role.as_str(),
                                );
                                let status_txt = if is_revoked { "Revoked" } else { "Active" };
                                swaggen_ui_kit::pill_badge(
                                    ui,
                                    if is_revoked {
                                        Tokens::TEXT_DANGER
                                    } else {
                                        Tokens::STATUS_SUCCESS
                                    },
                                    status_txt,
                                );
                                let uses_txt = match lnk.max_uses {
                                    Some(m) => format!("Uses: {}/{m}", lnk.uses_count),
                                    None => format!("Uses: {}/∞", lnk.uses_count),
                                };
                                ui.label(
                                    egui::RichText::new(uses_txt)
                                        .size(11.5)
                                        .color(swaggen_ui_kit::text_secondary(ui)),
                                );
                                ui.monospace(&lnk.share_link);
                                if swaggen_ui_kit::secondary_button(ui, true, "Copy link").clicked()
                                {
                                    ui.ctx().copy_text(lnk.share_link.clone());
                                    app.status = "Copied public invite link".into();
                                }
                                if !is_revoked
                                    && swaggen_ui_kit::danger_button(ui, !app.pending, "Revoke")
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

        ui.add_space(4.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.strong(format!(
                "Workspace members · mem_rev {}",
                details.membership_revision
            ));
            for m in &details.members {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{} <{}>", m.display_name, m.email));
                    swaggen_ui_kit::status_badge(ui, NoticeKind::Info, m.role.as_str());
                    if m.role != Role::Owner {
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

        ui.add_space(4.0);
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.strong("Workspace security audit log");
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
        ui.strong("Role Permission Matrix");
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
            ui.strong("Active device sessions");
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
        ui.strong("Change password");
        ui.horizontal(|ui| {
            ui.label("Current password:");
            ui.add(egui::TextEdit::singleline(&mut app.pw_current_input).password(true));
        });
        ui.horizontal(|ui| {
            ui.label("New password (min 10):");
            ui.add(egui::TextEdit::singleline(&mut app.pw_new_input).password(true));
        });
        ui.checkbox(
            &mut app.pw_revoke_others,
            "Revoke all other signed-in devices",
        );
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
