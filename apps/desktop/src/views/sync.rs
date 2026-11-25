//! Desktop P05 Team Collaboration, Sync Queue (SY02), Publish (C05),
//! 3-Way Conflict Resolver (C06/Y04), Comments (C04/C27), Version History (C10/C11),
//! Activity & Notifications (C03/C28) UI.
use crate::{app::App, worker::Command};
use eframe::egui;
use swaggen_sync::{ConflictChoice, OutboxStatus, compute_three_way_diff};
use swaggen_ui_kit::{AvatarTone, NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        let title = match app.sync_subtab {
            0 => "Publish your changes",
            1 => "Resolve a sync conflict",
            2 => "Version history",
            3 => "Discuss & comments",
            _ => "Workspace activity",
        };
        swaggen_ui_kit::page_header(
            ui,
            title,
            "Durable outbox queue, idempotent team publish, 3-way conflict resolver, revision history, and workspace notifications.",
            None,
            false,
        );

        let Some(active_prof) = app.server_profiles.active().cloned() else {
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Info,
                "Working offline",
                "Connect a Team Server in Server Profiles when ready to collaborate.",
            );
            return;
        };
        let Some(ref wid) = app.selected_team_workspace_id.clone() else {
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Info,
                "No team workspace selected",
                "Sign in and select a team workspace under Team Server -> Workspaces & Roles.",
            );
            return;
        };

        ui.horizontal_wrapped(|ui| {
            swaggen_ui_kit::pill_badge(
                ui,
                Tokens::STATUS_INFO,
                &format!(
                    "Cursor {} · {} base · {} drafts · {} outbox",
                    app.sync_state.last_cursor,
                    app.sync_state.base_entities.len(),
                    app.sync_state.drafts.len(),
                    app.sync_state.outbox.len(),
                ),
            );
            if swaggen_ui_kit::primary_button(ui, !app.pending, "Sync now").clicked() {
                app.submit(Command::SyncNow {
                    profile_id: active_prof.id.clone(),
                    workspace_id: wid.clone(),
                });
            }
            if swaggen_ui_kit::secondary_button(ui, !app.pending, "Refresh activity")
                .clicked()
            {
                app.submit(Command::LoadCollabData {
                    profile_id: active_prof.id.clone(),
                    workspace_id: wid.clone(),
                });
            }
        });
        ui.add_space(6.0);

        ui.horizontal_wrapped(|ui| {
            for (idx, label) in [
                "Outbox & Publish",
                "3-Way Conflicts",
                "Version History",
                "Comments",
                "Activity & Notifications",
            ]
            .iter()
            .enumerate()
            {
                if swaggen_ui_kit::underline_tab(ui, app.sync_subtab == idx, label).clicked() {
                    app.sync_subtab = idx;
                }
            }
        });
        ui.add_space(10.0);

        swaggen_ui_kit::surface_card(ui, |ui| match app.sync_subtab {
            0 => show_outbox_and_publish(app, ui, &active_prof.id, wid),
            1 => show_conflicts(app, ui, &active_prof.id, wid),
            2 => show_history(app, ui, &active_prof.id, wid),
            3 => show_comments(app, ui, &active_prof.id, wid),
            _ => show_notifications(app, ui, &active_prof.id, wid),
        });
    });
}

fn show_outbox_and_publish(app: &mut App, ui: &mut egui::Ui, profile_id: &str, wid: &str) {
    swaggen_ui_kit::surface_card(ui, |ui| {
        ui.strong("Stage active request / edit for Team Publish (CO02 / C05)");
        ui.horizontal_wrapped(|ui| {
            ui.label("Publish note:");
            ui.add(
                egui::TextEdit::singleline(&mut app.publish_note_input)
                    .hint_text("Describe your change…")
                    .desired_width(280.0),
            );
            if swaggen_ui_kit::primary_button(
                ui,
                app.active_request().is_some() && !app.pending,
                "Queue active request into durable outbox & publish",
            )
            .clicked()
                && let Some(req) = app.active_request().cloned()
            {
                let payload_json = serde_json::to_string(&req).unwrap_or_else(|_| "{}".into());
                app.submit(Command::StageAndPublishEntity {
                    profile_id: profile_id.to_owned(),
                    workspace_id: wid.to_owned(),
                    entity_id: req.id.clone(),
                    name: req.name.clone(),
                    kind: "request".into(),
                    payload_json,
                    delete: false,
                    note: app.publish_note_input.clone(),
                });
            }
        });
    });

    ui.add_space(6.0);
    swaggen_ui_kit::surface_card(ui, |ui| {
        ui.strong("Durable Outbox Queue (SY01 / SY02 / C07)");
        if app.sync_state.outbox.is_empty() {
            ui.label(
                egui::RichText::new("Outbox is empty.")
                    .color(Tokens::TEXT_SECONDARY),
            );
        } else {
            for entry in app.sync_state.outbox.iter().rev() {
                let kind = match &entry.status {
                    OutboxStatus::Acked => NoticeKind::Success,
                    OutboxStatus::Pending | OutboxStatus::Sending => NoticeKind::Info,
                    OutboxStatus::RetryWait { .. } => NoticeKind::Warning,
                    _ => NoticeKind::Danger,
                };
                ui.horizontal_wrapped(|ui| {
                    swaggen_ui_kit::status_badge(ui, kind, entry.status.label());
                    ui.strong(&entry.name);
                    ui.label(format!(
                        "base rev {} · op {}",
                        entry.base_revision,
                        &entry.operation_id[..8.min(entry.operation_id.len())]
                    ));
                    if !entry.note.is_empty() {
                        ui.label(
                            egui::RichText::new(format!("({})", entry.note))
                                .color(Tokens::TEXT_SECONDARY),
                        );
                    }
                });
                match &entry.status {
                    OutboxStatus::RetryWait {
                        attempts,
                        last_error,
                        ..
                    } => {
                        ui.label(format!("Retry #{attempts}: {last_error}"));
                    }
                    OutboxStatus::BlockedPermission { reason } => {
                        swaggen_ui_kit::notice_banner(
                            ui,
                            NoticeKind::Danger,
                            "Permission revoked (Y05 / SY09)",
                            &format!("{reason}. Your local draft stays preserved for personal export."),
                        );
                    }
                    OutboxStatus::FailedActionRequired { reason } => {
                        ui.label(
                            egui::RichText::new(reason).color(Tokens::TEXT_DANGER),
                        );
                    }
                    _ => {}
                }
            }
        }
    });
}

fn show_conflicts(app: &mut App, ui: &mut egui::Ui, profile_id: &str, wid: &str) {
    let conflicts: Vec<_> = app
        .sync_state
        .outbox
        .iter()
        .filter(|e| {
            matches!(
                e.status,
                OutboxStatus::Conflicted { .. } | OutboxStatus::DeletedRemoteConflict { .. }
            )
        })
        .cloned()
        .collect();

    if conflicts.is_empty() {
        swaggen_ui_kit::notice_banner(
            ui,
            NoticeKind::Success,
            "No sync conflicts",
            "All outbox operations match remote revisions.",
        );
        return;
    }

    for entry in conflicts {
        swaggen_ui_kit::surface_card(ui, |ui| {
            match &entry.status {
                OutboxStatus::Conflicted {
                    server_revision,
                    server_payload_json,
                    reason,
                } => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(swaggen_ui_kit::bold_text(
                            &entry.name,
                            15.0,
                            swaggen_ui_kit::text_primary(ui),
                        ));
                        swaggen_ui_kit::pill_badge(
                            ui,
                            Tokens::TEXT_WARNING,
                            &format!("Base r{} vs Server r{}", entry.base_revision, server_revision),
                        );
                    });
                    ui.label(
                        egui::RichText::new(reason)
                            .size(12.0)
                            .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                    ui.add_space(8.0);
                    let diffs = compute_three_way_diff(
                        &entry.base_payload_json,
                        &entry.local_payload_json,
                        server_payload_json,
                    );
                    ui.columns(3, |cols| {
                        swaggen_ui_kit::surface_card(&mut cols[0], |ui| {
                            swaggen_ui_kit::card_header(ui, "Base (Common Ancestor)");
                            for d in &diffs {
                                ui.monospace(format!("{}: {}", d.path, d.base_val));
                            }
                        });
                        swaggen_ui_kit::surface_card(&mut cols[1], |ui| {
                            swaggen_ui_kit::card_header(ui, "Your Local Version");
                            for d in &diffs {
                                ui.label(
                                    egui::RichText::new(format!("{}: {}", d.path, d.local_val))
                                        .monospace()
                                        .color(Tokens::STATUS_INFO),
                                );
                            }
                        });
                        swaggen_ui_kit::surface_card(&mut cols[2], |ui| {
                            swaggen_ui_kit::card_header(ui, "Server Version");
                            for d in &diffs {
                                ui.label(
                                    egui::RichText::new(format!("{}: {}", d.path, d.server_val))
                                        .monospace()
                                        .color(Tokens::TEXT_WARNING),
                                );
                            }
                        });
                    });
                    ui.add_space(10.0);
                    ui.horizontal_wrapped(|ui| {
                        if swaggen_ui_kit::primary_button(
                            ui,
                            !app.pending,
                            "Rebase & publish my version",
                        )
                        .clicked()
                        {
                            app.submit(Command::ResolveSyncConflict {
                                profile_id: profile_id.to_owned(),
                                workspace_id: wid.to_owned(),
                                operation_id: entry.operation_id.clone(),
                                choice: ConflictChoice::RebaseWithPayload {
                                    merged_payload_json: entry.local_payload_json.clone(),
                                    note: format!(
                                        "Resolved conflict cleanly over r{server_revision}"
                                    ),
                                },
                            });
                        }
                        if swaggen_ui_kit::secondary_button(
                            ui,
                            !app.pending,
                            "Take server & keep my draft",
                        )
                        .clicked()
                        {
                            app.submit(Command::ResolveSyncConflict {
                                profile_id: profile_id.to_owned(),
                                workspace_id: wid.to_owned(),
                                operation_id: entry.operation_id.clone(),
                                choice: ConflictChoice::TakeServerKeepMyDraft,
                            });
                        }
                    });
                }
                OutboxStatus::DeletedRemoteConflict {
                    server_revision,
                    reason,
                } => {
                    swaggen_ui_kit::notice_banner(
                        ui,
                        NoticeKind::Danger,
                        "Deleted while offline (Y04 / SY07)",
                        &format!("{reason} (rev {server_revision}). Tombstone prevents silent resurrection."),
                    );
                    if swaggen_ui_kit::secondary_button(
                        ui,
                        !app.pending,
                        "Accept deletion & preserve my draft locally",
                    )
                    .clicked()
                    {
                        app.submit(Command::ResolveSyncConflict {
                            profile_id: profile_id.to_owned(),
                            workspace_id: wid.to_owned(),
                            operation_id: entry.operation_id.clone(),
                            choice: ConflictChoice::TakeServerKeepMyDraft,
                        });
                    }
                }
                _ => {}
            }
        });
    }
}

fn show_history(app: &mut App, ui: &mut egui::Ui, profile_id: &str, wid: &str) {
    ui.horizontal(|ui| {
        ui.label("Entity ID:");
        ui.add(egui::TextEdit::singleline(&mut app.history_entity_id_input).desired_width(220.0));
        if swaggen_ui_kit::secondary_button(ui, !app.pending, "Load revisions (C10)").clicked() {
            app.submit(Command::LoadEntityRevisions {
                profile_id: profile_id.to_owned(),
                workspace_id: wid.to_owned(),
                entity_id: app.history_entity_id_input.clone(),
            });
        }
    });
    ui.separator();
    let cur_rev = app
        .team_revisions
        .first()
        .map(|r| r.revision)
        .unwrap_or(1);
    let mut restore_target: Option<(String, i64)> = None;
    for r in &app.team_revisions {
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!("r{}", r.revision));
            ui.label(format!("{} · {} · {}", r.actor_email, r.name, r.note));
            if ui.small_button("Restore as new version (C11)").clicked() {
                restore_target = Some((r.entity_id.clone(), r.revision));
            }
        });
    }
    if let Some((entity_id, target_revision)) = restore_target {
        app.submit(Command::RestoreEntityRevision {
            profile_id: profile_id.to_owned(),
            workspace_id: wid.to_owned(),
            entity_id,
            target_revision,
            expected_current_revision: cur_rev,
        });
    }
}

fn show_comments(app: &mut App, ui: &mut egui::Ui, profile_id: &str, wid: &str) {
    // Figma C04 / Discuss 2-column layout: selected request preview left + Discussion feed right.
    let active_req = app.active_request().cloned();
    if app.history_entity_id_input.trim().is_empty()
        && let Some(ref req) = active_req
    {
        app.history_entity_id_input = req.id.clone();
    }

    let mut resolve_cid: Option<String> = None;
    ui.columns(2, |cols| {
        // Left column: Selected endpoint preview
        swaggen_ui_kit::surface_card(&mut cols[0], |ui| {
            swaggen_ui_kit::card_header(ui, "Referenced Endpoint");
            if let Some(ref req) = active_req {
                ui.horizontal_wrapped(|ui| {
                    swaggen_ui_kit::method_badge(ui, req.method.as_str());
                    ui.label(swaggen_ui_kit::bold_text(
                        &req.name,
                        15.0,
                        swaggen_ui_kit::text_primary(ui),
                    ));
                });
                ui.add_space(6.0);
                swaggen_ui_kit::code_surface(ui, &req.url);
                ui.add_space(10.0);
                if !req.query_params.is_empty() {
                    swaggen_ui_kit::card_header(ui, "Query parameters");
                    for row in &req.query_params {
                        ui.monospace(format!("{} = {}", row.key, row.value));
                    }
                }
            } else {
                ui.label(
                    egui::RichText::new("Select an endpoint in the sidebar to anchor comments.")
                        .color(swaggen_ui_kit::text_secondary(ui)),
                );
            }
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new("Entity ID")
                    .size(11.5)
                    .color(swaggen_ui_kit::text_secondary(ui)),
            );
            swaggen_ui_kit::text_input(
                ui,
                &mut app.history_entity_id_input,
                "request entity id",
                ui.available_width(),
                false,
            );
        });

        // Right column: Figma C04 Discussion thread + reply box
        swaggen_ui_kit::surface_card(&mut cols[1], |ui| {
            ui.horizontal(|ui| {
                ui.label(swaggen_ui_kit::bold_text(
                    "Discussion",
                    15.0,
                    swaggen_ui_kit::text_primary(ui),
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    swaggen_ui_kit::pill_badge(
                        ui,
                        swaggen_ui_kit::text_secondary(ui),
                        &format!("{} thread(s)", app.team_comments.len()),
                    );
                });
            });
            ui.add_space(10.0);

            if app.team_comments.is_empty() {
                ui.label(
                    egui::RichText::new("No discussion threads yet. Start a review note below.")
                        .size(12.0)
                        .color(swaggen_ui_kit::text_secondary(ui)),
                );
            } else {
                for (i, c) in app.team_comments.iter().enumerate() {
                    let initials: String = c
                        .author_email
                        .split(|ch: char| !ch.is_alphanumeric())
                        .filter(|s| !s.is_empty())
                        .filter_map(|s| s.chars().next())
                        .take(2)
                        .collect::<String>()
                        .to_ascii_uppercase();
                    let tone = match i % 3 {
                        0 => AvatarTone::Teal,
                        1 => AvatarTone::Salmon,
                        _ => AvatarTone::Cream,
                    };
                    swaggen_ui_kit::surface_card(ui, |ui| {
                        ui.horizontal(|ui| {
                            swaggen_ui_kit::avatar_circle(
                                ui,
                                if initials.is_empty() { "TM" } else { &initials },
                                tone,
                            );
                            ui.vertical(|ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(swaggen_ui_kit::bold_text(
                                        &c.author_email,
                                        12.5,
                                        swaggen_ui_kit::text_primary(ui),
                                    ));
                                    ui.label(
                                        egui::RichText::new(format!("r{}", c.anchor_revision))
                                            .size(11.0)
                                            .color(swaggen_ui_kit::text_secondary(ui)),
                                    );
                                    if c.resolved {
                                        swaggen_ui_kit::pill_badge(
                                            ui,
                                            Tokens::STATUS_SUCCESS,
                                            "Resolved",
                                        );
                                    } else if ui.small_button("Resolve").clicked() {
                                        resolve_cid = Some(c.comment_id.clone());
                                    }
                                });
                                ui.label(
                                    egui::RichText::new(&c.body)
                                        .size(12.5)
                                        .color(swaggen_ui_kit::text_primary(ui)),
                                );
                            });
                        });
                        for rep in &c.replies {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(format!("  ↳ {}: {}", rep.author_email, rep.body))
                                    .size(12.0)
                                    .color(swaggen_ui_kit::text_secondary(ui)),
                            );
                        }
                    });
                    ui.add_space(8.0);
                }
            }

            ui.add_space(8.0);
            swaggen_ui_kit::text_input(
                ui,
                &mut app.comment_body_input,
                "Write a reply…",
                ui.available_width(),
                false,
            );
            ui.add_space(8.0);
            if swaggen_ui_kit::primary_button(
                ui,
                !app.pending && !app.comment_body_input.trim().is_empty(),
                "Post comment",
            )
            .clicked()
            {
                let rev = app
                    .sync_state
                    .base_entities
                    .get(&app.history_entity_id_input)
                    .map(|e| e.revision)
                    .unwrap_or(1);
                let body = std::mem::take(&mut app.comment_body_input);
                let eid = app.history_entity_id_input.clone();
                app.submit(Command::CreateTeamComment {
                    profile_id: profile_id.to_owned(),
                    workspace_id: wid.to_owned(),
                    entity_id: eid,
                    anchor_revision: rev,
                    body,
                });
            }
        });
    });

    if let Some(cid) = resolve_cid {
        app.submit(Command::ResolveTeamComment {
            profile_id: profile_id.to_owned(),
            workspace_id: wid.to_owned(),
            comment_id: cid,
            resolved: true,
        });
    }
}

fn show_notifications(app: &mut App, ui: &mut egui::Ui, profile_id: &str, wid: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!(
            "Notifications inbox ({} unread · C28)",
            app.team_unread_notifications
        ));
        if swaggen_ui_kit::secondary_button(ui, !app.pending, "Mark all read").clicked() {
            app.submit(Command::MarkTeamNotificationsRead {
                profile_id: profile_id.to_owned(),
                workspace_id: wid.to_owned(),
            });
        }
    });
    for n in &app.team_notifications {
        ui.horizontal_wrapped(|ui| {
            swaggen_ui_kit::status_badge(
                ui,
                if n.read { NoticeKind::Info } else { NoticeKind::Warning },
                if n.read { "read" } else { "new" },
            );
            ui.label(&n.title);
        });
    }
}
