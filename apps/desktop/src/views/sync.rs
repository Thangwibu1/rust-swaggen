//! Sequential Publish History, Field-Level Commit Diffs & Git-Style Partial Cherry-Pick View.
use crate::{app::App, worker::Command};
use eframe::egui;
use std::collections::BTreeSet;
use swaggen_domain::workspace::{RequestCherryPickHunk, RequestItem, diff_request_snapshots};
use swaggen_ui_kit::{NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        swaggen_ui_kit::page_header(
            ui,
            "Publish History & Cherry-Pick",
            "Every Ctrl+S in a Team Workspace sends a sequential commit event to the server. Inspect exact changes in each commit and cherry-pick individual sections back anytime.",
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
            if swaggen_ui_kit::primary_button(ui, true, "Open Team Collaboration").clicked() {
                app.navigate_to(crate::app::MainMode::Server);
            }
            return;
        };

        let Some((pid, wid)) = app.active_team_target() else {
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Info,
                "Select a Team Workspace",
                "Choose a Team Workspace below to start publishing with Ctrl+S and cherry-picking changes.",
            );
            ui.add_space(8.0);
            if app.team_workspaces.is_empty() {
                ui.horizontal(|ui| {
                    if swaggen_ui_kit::primary_button(
                        ui,
                        !app.pending,
                        "Create / Join Team Workspace",
                    )
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
                                            chosen =
                                                Some((ws.workspace_id.clone(), ws.name.clone()));
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
                    &format!("{} commit(s)", app.publish_history.len()),
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
                    "Optional commit note (default: Ctrl+S)",
                    270.0,
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
                "No publish commits recorded yet",
                "Edit any API in Workbench and press Ctrl+S to publish the first sequential snapshot.",
            );
            return;
        }

        let events = app.publish_history.clone();
        let mut cherry_pick_full_ev = None;
        let mut cherry_pick_partial: Option<(
            swaggen_api_contract::ChangeEventRecord,
            Vec<RequestCherryPickHunk>,
        )> = None;

        for (idx, ev) in events.iter().enumerate() {
            let parsed_cur = if ev.entity_kind == "request" {
                ev.payload_json
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<RequestItem>(raw).ok())
            } else {
                None
            };
            let parsed_prev = if ev.entity_kind == "request" {
                events[idx + 1..]
                    .iter()
                    .find(|older| older.entity_id == ev.entity_id && older.entity_kind == "request")
                    .and_then(|older| older.payload_json.as_deref())
                    .and_then(|raw| serde_json::from_str::<RequestItem>(raw).ok())
            } else {
                None
            };
            let diffs = parsed_cur
                .as_ref()
                .map(|cur| diff_request_snapshots(parsed_prev.as_ref(), cur))
                .unwrap_or_default();

            let is_expanded = app.expanded_history_cursors.contains(&ev.cursor);
            let is_preview_json = app.preview_history_cursor == Some(ev.cursor);

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

                        if parsed_cur.is_some() {
                            if parsed_prev.is_none() {
                                swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_SUCCESS, "Created");
                            } else if diffs.is_empty() {
                                swaggen_ui_kit::pill_badge(
                                    ui,
                                    swaggen_ui_kit::text_secondary(ui),
                                    "Unchanged",
                                );
                            } else {
                                let summary = if diffs.len() <= 3 {
                                    diffs
                                        .iter()
                                        .map(|d| d.label)
                                        .collect::<Vec<_>>()
                                        .join(" · ")
                                } else {
                                    format!(
                                        "{} · +{} more",
                                        diffs
                                            .iter()
                                            .take(2)
                                            .map(|d| d.label)
                                            .collect::<Vec<_>>()
                                            .join(" · "),
                                        diffs.len() - 2
                                    )
                                };
                                swaggen_ui_kit::pill_badge(ui, Tokens::TEXT_WARNING, &summary);
                            }
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if swaggen_ui_kit::secondary_button(
                                ui,
                                ev.payload_json.is_some() && !ev.deleted,
                                "Restore all",
                            )
                            .on_hover_text("Restore the entire snapshot of this commit")
                            .clicked()
                            {
                                cherry_pick_full_ev = Some(ev.clone());
                            }
                            if ev.payload_json.is_some()
                                && swaggen_ui_kit::secondary_button(
                                    ui,
                                    true,
                                    if is_preview_json { "Hide JSON" } else { "JSON" },
                                )
                                .clicked()
                            {
                                app.preview_history_cursor = if is_preview_json {
                                    None
                                } else {
                                    Some(ev.cursor)
                                };
                            }
                            if parsed_cur.is_some() {
                                let diff_btn_lbl = if is_expanded {
                                    format!("Hide changes ({})", diffs.len())
                                } else {
                                    format!("Changes ({})", diffs.len())
                                };
                                if swaggen_ui_kit::primary_button(ui, true, &diff_btn_lbl)
                                    .on_hover_text("View field-by-field diff and cherry-pick individual sections")
                                    .clicked()
                                {
                                    if is_expanded {
                                        app.expanded_history_cursors.remove(&ev.cursor);
                                    } else {
                                        app.expanded_history_cursors.insert(ev.cursor);
                                    }
                                }
                            }
                        });
                    });

                    ui.add_space(3.0);
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

                    if is_expanded && parsed_cur.is_some() {
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(6.0);

                        if diffs.is_empty() {
                            ui.label(
                                egui::RichText::new(
                                    "No field changes compared to the previous commit for this request.",
                                )
                                .size(11.5)
                                .color(swaggen_ui_kit::text_secondary(ui)),
                            );
                        } else {
                            let all_hunks: BTreeSet<RequestCherryPickHunk> =
                                diffs.iter().map(|d| d.hunk).collect();
                            let sel_set = app
                                .cherry_pick_selections
                                .entry(ev.cursor)
                                .or_insert_with(|| all_hunks.clone());

                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    egui::RichText::new(if parsed_prev.is_some() {
                                        "Select changed parts to cherry-pick into Workbench:"
                                    } else {
                                        "Initial fields in this commit (select parts to cherry-pick):"
                                    })
                                    .size(11.5)
                                    .color(swaggen_ui_kit::text_secondary(ui)),
                                );
                                if ui.small_button("Select all").clicked() {
                                    *sel_set = all_hunks.clone();
                                }
                                if ui.small_button("None").clicked() {
                                    sel_set.clear();
                                }
                            });
                            ui.add_space(4.0);

                            for diff in &diffs {
                                let mut checked = sel_set.contains(&diff.hunk);
                                egui::Frame::new()
                                    .fill(swaggen_ui_kit::subtle_bg(ui))
                                    .stroke(egui::Stroke::new(
                                        1.0,
                                        swaggen_ui_kit::border_color(ui),
                                    ))
                                    .corner_radius(5)
                                    .inner_margin(egui::Margin::symmetric(10, 6))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            if ui.checkbox(&mut checked, "").changed() {
                                                if checked {
                                                    sel_set.insert(diff.hunk);
                                                } else {
                                                    sel_set.remove(&diff.hunk);
                                                }
                                            }
                                            ui.label(swaggen_ui_kit::bold_text(
                                                diff.label,
                                                12.0,
                                                swaggen_ui_kit::text_primary(ui),
                                            ));
                                        });
                                        for line in &diff.before_lines {
                                            render_diff_line(ui, false, line);
                                        }
                                        for line in &diff.after_lines {
                                            render_diff_line(ui, true, line);
                                        }
                                    });
                                ui.add_space(4.0);
                            }

                            let selected_list: Vec<RequestCherryPickHunk> =
                                sel_set.iter().copied().collect();
                            let sel_count = selected_list.len();
                            ui.add_space(2.0);
                            ui.horizontal(|ui| {
                                if swaggen_ui_kit::primary_button(
                                    ui,
                                    sel_count > 0 && !app.pending,
                                    &format!("Cherry-pick selected ({sel_count})"),
                                )
                                .on_hover_text(
                                    "Apply ONLY the checked sections above to Workbench & record a sequential cherry-pick commit",
                                )
                                .clicked()
                                {
                                    cherry_pick_partial = Some((ev.clone(), selected_list));
                                }
                            });
                        }
                    }

                    if is_preview_json && let Some(ref raw) = ev.payload_json {
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

        if let Some((ev, hunks)) = cherry_pick_partial {
            app.cherry_pick_request_hunks(&ev, &hunks);
        } else if let Some(ev) = cherry_pick_full_ev {
            app.cherry_pick_publish_event(&ev);
        }
    });
}

fn render_diff_line(ui: &mut egui::Ui, is_added: bool, line: &str) {
    let (bg, fg, sign) = if is_added {
        (
            egui::Color32::from_rgb(240, 253, 244),
            egui::Color32::from_rgb(21, 128, 61),
            "+",
        )
    } else {
        (
            egui::Color32::from_rgb(254, 242, 242),
            egui::Color32::from_rgb(185, 28, 28),
            "-",
        )
    };
    egui::Frame::new()
        .fill(bg)
        .corner_radius(3)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(format!("{sign} {line}"))
                    .size(11.0)
                    .color(fg),
            );
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
