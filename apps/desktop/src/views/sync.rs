//! Sequential Publish & Cherry-Pick History View:
//! Every `Ctrl+S` (or explicit Publish) in a Team Workspace sends a sequential commit event
//! to the server; teammates see the timeline and can cherry-pick any saved version back anytime.
use crate::{app::App, worker::Command};
use eframe::egui;
use swaggen_ui_kit::{NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        swaggen_ui_kit::page_header(
            ui,
            "Publish History & Cherry-Pick",
            "Press Ctrl+S inside a Team Workspace to publish your change sequentially to the server. Browse history below and cherry-pick any version back into Workbench.",
            None,
            false,
        );

        let Some(active_prof) = app
            .server_profiles
            .active()
            .filter(|p| !p.session_id.is_empty())
            .cloned()
        else {
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Info,
                "Sign in to view Team Publish History",
                "Connect with Google in Team Collaboration to publish and cherry-pick shared API versions.",
            );
            ui.add_space(8.0);
            if swaggen_ui_kit::primary_button(
                ui,
                true,
                "Open Team Collaboration",
            )
            .clicked()
            {
                app.navigate_to(crate::app::MainMode::Server);
            }
            return;
        };

        let Some((pid, wid)) = app.active_team_target() else {
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Info,
                "Select a Team Workspace",
                "Choose a Team Workspace below to start publishing with Ctrl+S and cherry-picking revisions.",
            );
            ui.add_space(8.0);
            if app.team_workspaces.is_empty() {
                ui.horizontal(|ui| {
                    if swaggen_ui_kit::primary_button(ui, !app.pending, "Create / Join Team Workspace")
                        .clicked()
                    {
                        app.navigate_to(crate::app::MainMode::Server);
                    }
                    if swaggen_ui_kit::secondary_button(ui, !app.pending, "Refresh workspaces")
                        .clicked()
                    {
                        app.submit(Command::RefreshTeamWorkspaces {
                            profile_id: active_prof.id.clone(),
                        });
                    }
                });
            } else {
                let mut chosen: Option<(String, String)> = None;
                for ws in &app.team_workspaces {
                    egui::Frame::new()
                        .fill(swaggen_ui_kit::subtle_bg(ui))
                        .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.strong(&ws.name);
                                swaggen_ui_kit::pill_badge(
                                    ui,
                                    Tokens::STATUS_INFO,
                                    ws.role.as_str(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if swaggen_ui_kit::primary_button(
                                            ui,
                                            !app.pending,
                                            "Open Team Workspace",
                                        )
                                        .clicked()
                                        {
                                            chosen = Some((
                                                ws.workspace_id.clone(),
                                                ws.name.clone(),
                                            ));
                                        }
                                    },
                                );
                            });
                        });
                    ui.add_space(4.0);
                }
                if let Some((chosen_id, chosen_name)) = chosen {
                    app.open_team_workspace(&active_prof.id, &chosen_id, &chosen_name);
                }
            }
            return;
        };

        let ws_name = app
            .team_workspaces
            .iter()
            .find(|w| w.workspace_id == wid)
            .map(|w| w.name.clone())
            .unwrap_or_else(|| app.state.meta.name.clone());

        // Top Action & Quick Publish Bar
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                swaggen_ui_kit::pill_badge(
                    ui,
                    Tokens::STATUS_SUCCESS,
                    &format!("Team: {ws_name}"),
                );
                swaggen_ui_kit::pill_badge(
                    ui,
                    Tokens::STATUS_INFO,
                    &format!("{} sequential event(s)", app.publish_history.len()),
                );
                if swaggen_ui_kit::secondary_button(ui, !app.pending, "Refresh history").clicked() {
                    app.submit(Command::LoadPublishHistory {
                        profile_id: pid.clone(),
                        workspace_id: wid.clone(),
                    });
                }
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                let target_label = app
                    .active_request()
                    .map(|r| format!("{} {}", r.method.as_str(), r.name))
                    .unwrap_or_else(|| format!("Workspace: {ws_name}"));
                ui.label(
                    egui::RichText::new(format!("Target: {target_label}"))
                        .size(11.5)
                        .color(swaggen_ui_kit::text_secondary(ui)),
                );
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.publish_note_input,
                    "Optional note (default: Ctrl+S)",
                    260.0,
                    false,
                );
                if swaggen_ui_kit::primary_button(ui, !app.pending, "Publish change (Ctrl+S)")
                    .clicked()
                {
                    let note = std::mem::take(&mut app.publish_note_input);
                    app.save_workspace_local_only();
                    app.publish_current_on_save(Some(&note));
                }
            });
        });

        ui.add_space(10.0);

        if app.publish_history.is_empty() {
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Info,
                "No publish events recorded yet",
                "Edit any API in Workbench and press Ctrl+S to publish the first sequential snapshot.",
            );
            return;
        }

        let mut cherry_pick_ev = None;
        let events = app.publish_history.clone();
        for ev in &events {
            let is_preview = app.preview_history_cursor == Some(ev.cursor);
            egui::Frame::new()
                .fill(swaggen_ui_kit::surface_bg(ui))
                .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(12, 9))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        swaggen_ui_kit::pill_badge(
                            ui,
                            Tokens::STATUS_INFO,
                            &format!("#{}", ev.cursor),
                        );
                        swaggen_ui_kit::pill_badge(
                            ui,
                            Tokens::METHOD_POST,
                            &ev.entity_kind.to_ascii_uppercase(),
                        );
                        ui.label(swaggen_ui_kit::bold_text(
                            &ev.entity_name,
                            13.0,
                            swaggen_ui_kit::text_primary(ui),
                        ));
                        ui.label(
                            egui::RichText::new(format!("r{}", ev.revision))
                                .size(11.0)
                                .color(swaggen_ui_kit::text_secondary(ui)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if swaggen_ui_kit::primary_button(
                                ui,
                                ev.payload_json.is_some() && !ev.deleted,
                                "Cherry-pick & Restore",
                            )
                            .on_hover_text(
                                "Restore this exact saved version into Workbench and publish a new sequential commit",
                            )
                            .clicked()
                            {
                                cherry_pick_ev = Some(ev.clone());
                            }
                            if ev.payload_json.is_some()
                                && swaggen_ui_kit::secondary_button(
                                    ui,
                                    true,
                                    if is_preview { "Hide JSON" } else { "Preview" },
                                )
                                .clicked()
                            {
                                app.preview_history_cursor = if is_preview {
                                    None
                                } else {
                                    Some(ev.cursor)
                                };
                            }
                        });
                    });
                    ui.add_space(3.0);
                    ui.horizontal_wrapped(|ui| {
                        let ts = format_unix_utc(ev.created_at_unix);
                        let note_str = if ev.note.trim().is_empty() {
                            "Saved"
                        } else {
                            ev.note.trim()
                        };
                        ui.label(
                            egui::RichText::new(format!(
                                "{} · {} · Note: {}",
                                ev.actor_email, ts, note_str
                            ))
                            .size(11.0)
                            .color(swaggen_ui_kit::text_secondary(ui)),
                        );
                    });

                    if is_preview && let Some(ref raw) = ev.payload_json {
                        ui.add_space(6.0);
                        let pretty = serde_json::from_str::<serde_json::Value>(raw)
                            .ok()
                            .and_then(|v| serde_json::to_string_pretty(&v).ok())
                            .unwrap_or_else(|| raw.clone());
                        swaggen_ui_kit::code_surface_numbered(ui, &pretty);
                    }
                });
            ui.add_space(6.0);
        }

        if let Some(ev) = cherry_pick_ev {
            app.cherry_pick_publish_event(&ev);
        }
    });
}

fn format_unix_utc(unix: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(unix)
        .map(|dt| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
                dt.year(),
                u8::from(dt.month()),
                dt.day(),
                dt.hour(),
                dt.minute(),
                dt.second()
            )
        })
        .unwrap_or_else(|_| unix.to_string())
}
