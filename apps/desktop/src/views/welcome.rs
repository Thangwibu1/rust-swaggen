//! Figma A01 / Welcome to Swaggen & A02 / Continue in your browser (Firebase Google + Email Auth).
use crate::{
    app::{App, MainMode},
    worker::Command,
};
use eframe::egui;
use swaggen_ui_kit::{Edge, IconKind, NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let active_prof = app.server_profiles.active().cloned();
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

    // Bottom 28px Figma statusbar on Welcome screen.
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
                let left = if app.pending {
                    "Waiting for Firebase sign-in in your browser…"
                } else if app.error {
                    app.status.as_str()
                } else {
                    "Not signed in · Continue securely with Firebase / Google"
                };
                ui.label(
                    egui::RichText::new(left)
                        .size(11.0)
                        .color(if app.error {
                            Tokens::TEXT_DANGER
                        } else {
                            swaggen_ui_kit::text_primary(ui)
                        }),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("Rust engine  /  UTF-8  /  Windows")
                            .size(11.0)
                            .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                });
            });
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(swaggen_ui_kit::page_bg(ui))
                .inner_margin(egui::Margin::symmetric(56, 28)),
        )
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let avail_h = ui.available_height().max(540.0);
                ui.add_space(((avail_h - 580.0) * 0.35).max(8.0));
                ui.columns(2, |cols| {
                    // Left Hero column matching Figma A01 (6:144).
                    let left = &mut cols[0];
                    left.add_space(28.0);
                    swaggen_ui_kit::brand_mark(left, 38.0);
                    left.add_space(18.0);
                    left.label(swaggen_ui_kit::bold_text(
                        "swaggen",
                        24.0,
                        swaggen_ui_kit::text_primary(left),
                    ));
                    left.add_space(18.0);
                    left.label(swaggen_ui_kit::bold_text(
                        "Build better APIs.\nTogether.",
                        40.0,
                        swaggen_ui_kit::text_primary(left),
                    ));
                    left.add_space(18.0);
                    left.label(
                        egui::RichText::new(
                            "A shared home for your requests,\nAPI contracts and team conversations.",
                        )
                        .size(14.0)
                        .color(swaggen_ui_kit::text_secondary(left)),
                    );
                    left.add_space(22.0);
                    left.horizontal_wrapped(|ui| {
                        for tag in ["Design", "Test", "Collaborate"] {
                            swaggen_ui_kit::pill_badge(ui, swaggen_ui_kit::text_primary(ui), tag);
                        }
                    });
                    left.add_space(22.0);
                    left.label(
                        egui::RichText::new("YOUR SERVER. YOUR WORKFLOW.")
                            .size(11.0)
                            .color(swaggen_ui_kit::text_primary(left)),
                    );

                    // Right Sign-In Card matching Figma A01 (corner_radius: 16, width ~460px).
                    let right = &mut cols[1];
                    right.vertical_centered(|ui| {
                        ui.set_max_width(470.0);
                        egui::Frame::new()
                            .fill(swaggen_ui_kit::surface_bg(ui))
                            .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                            .corner_radius(16)
                            .inner_margin(egui::Margin::symmetric(32, 24))
                            .show(ui, |ui| {
                                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                                    ui.label(swaggen_ui_kit::bold_text(
                                        "Sign in to Swaggen",
                                        22.0,
                                        swaggen_ui_kit::text_primary(ui),
                                    ));
                                    ui.add_space(6.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "Continue with Google to sign in or automatically create your account.",
                                        )
                                        .size(12.5)
                                        .color(swaggen_ui_kit::text_secondary(ui)),
                                    );
                                    ui.add_space(16.0);

                                    let prof_id = active_prof
                                        .as_ref()
                                        .map(|p| p.id.clone())
                                        .unwrap_or_else(|| "production".into());

                                    if app.pending {
                                        swaggen_ui_kit::notice_banner(
                                            ui,
                                            NoticeKind::Info,
                                            "Continue in your browser",
                                            "Finish signing in with Firebase in your system browser. Swaggen will receive your session automatically.",
                                        );
                                        ui.add_space(12.0);
                                    }

                                    // Google / Firebase browser handoff button (A01/A02).
                                    if swaggen_ui_kit::google_button(
                                        ui,
                                        !app.pending,
                                        "Continue with Google",
                                    )
                                    .clicked()
                                    {
                                        let bt = app.auth_bootstrap_token.trim().to_owned();
                                        app.submit(Command::FirebaseBrowserLogin {
                                            profile_id: prof_id.clone(),
                                            bootstrap_token: if bt.is_empty() { None } else { Some(bt) },
                                        });
                                    }

                                    ui.add_space(14.0);
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("or sign in with Firebase Email & Password")
                                                .size(11.0)
                                                .color(swaggen_ui_kit::text_secondary(ui)),
                                        );
                                    });
                                    ui.add_space(8.0);

                                    let input_w = ui.available_width();
                                    swaggen_ui_kit::text_input(
                                        ui,
                                        &mut app.auth_email,
                                        "Email address",
                                        input_w,
                                        false,
                                    );
                                    ui.add_space(8.0);
                                    swaggen_ui_kit::text_input(
                                        ui,
                                        &mut app.auth_password,
                                        "Password",
                                        input_w,
                                        true,
                                    );
                                    if app.firebase_create_account {
                                        ui.add_space(8.0);
                                        swaggen_ui_kit::text_input(
                                            ui,
                                            &mut app.auth_display_name,
                                            "Display name",
                                            input_w,
                                            false,
                                        );
                                    }
                                    ui.add_space(8.0);
                                    ui.horizontal(|ui| {
                                        ui.checkbox(
                                            &mut app.firebase_create_account,
                                            "Create new Firebase account",
                                        );
                                    });
                                    ui.add_space(10.0);

                                    let can_submit = !app.pending
                                        && !app.auth_email.trim().is_empty()
                                        && !app.auth_password.is_empty();
                                    if ui
                                        .add_enabled(
                                            can_submit,
                                            egui::Button::new(
                                                egui::RichText::new(if app.firebase_create_account {
                                                    "Create account with Firebase"
                                                } else {
                                                    "Sign in with Firebase"
                                                })
                                                .size(13.0)
                                                .color(if can_submit {
                                                    Tokens::STATUS_INFO
                                                } else {
                                                    swaggen_ui_kit::text_secondary(ui)
                                                })
                                                .strong(),
                                            )
                                            .fill(swaggen_ui_kit::surface_bg(ui))
                                            .stroke(egui::Stroke::new(
                                                1.0,
                                                swaggen_ui_kit::border_color(ui),
                                            ))
                                            .min_size(egui::vec2(input_w, 40.0))
                                            .corner_radius(6),
                                        )
                                        .clicked()
                                    {
                                        let pw = std::mem::take(&mut app.auth_password);
                                        let bt = app.auth_bootstrap_token.trim().to_owned();
                                        app.submit(Command::FirebaseEmailAuth {
                                            profile_id: prof_id.clone(),
                                            email: app.auth_email.trim().to_owned(),
                                            password: pw,
                                            display_name: app.auth_display_name.trim().to_owned(),
                                            create_account: app.firebase_create_account,
                                            bootstrap_token: if bt.is_empty() { None } else { Some(bt) },
                                        });
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

                                    ui.add_space(18.0);
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
                                            egui::RichText::new("Your tokens stay on your device.")
                                                .size(12.0)
                                                .color(swaggen_ui_kit::text_primary(ui)),
                                        );
                                    });
                                    ui.add_space(10.0);
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("Team server")
                                                .size(12.0)
                                                .color(swaggen_ui_kit::text_secondary(ui)),
                                        );
                                        ui.add_space(8.0);
                                        ui.label(
                                            egui::RichText::new(&server_host)
                                                .size(12.0)
                                                .color(swaggen_ui_kit::text_primary(ui)),
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
                                                    app.mode = MainMode::Server;
                                                    app.server_subtab = 0;
                                                }
                                            },
                                        );
                                    });
                                    ui.add_space(14.0);
                                    ui.horizontal(|ui| {
                                        if swaggen_ui_kit::secondary_button(
                                            ui,
                                            true,
                                            "Continue offline to workspace →",
                                        )
                                        .clicked()
                                        {
                                            app.show_welcome = false;
                                        }
                                    });
                                });
                            });
                    });
                });
            });
        });
}
