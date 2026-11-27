//! Git-Fork style Publish History, Live-Workspace Comparison, Batch Cherry-Pick & Revert View.
use crate::{app::App, worker::Command};
use eframe::egui;
use std::collections::{BTreeSet, HashMap};
use swaggen_api_contract::ChangeEventRecord;
use swaggen_domain::workspace::{
    RequestCherryPickHunk, RequestFieldDiff, RequestItem, WorkspaceState, diff_request_snapshots,
};
use swaggen_ui_kit::{IconKind, NoticeKind, Tokens};

enum CommitEntry {
    Request {
        ev: ChangeEventRecord,
        hist_req: Box<RequestItem>,
        prev_req: Option<Box<RequestItem>>,
        cur_live_req: Option<Box<RequestItem>>,
        diffs_vs_current: Vec<RequestFieldDiff>,
        diffs_vs_prev: Vec<RequestFieldDiff>,
    },
    Workspace {
        ev: ChangeEventRecord,
        requests: Vec<(RequestItem, Option<RequestItem>, Vec<RequestFieldDiff>)>,
        differing_count: usize,
    },
    Other {
        ev: ChangeEventRecord,
    },
}

impl CommitEntry {
    fn event(&self) -> &ChangeEventRecord {
        match self {
            Self::Request { ev, .. } | Self::Workspace { ev, .. } | Self::Other { ev } => ev,
        }
    }

    fn differs_from_current(&self) -> bool {
        match self {
            Self::Request {
                cur_live_req,
                diffs_vs_current,
                ..
            } => cur_live_req.is_none() || !diffs_vs_current.is_empty(),
            Self::Workspace {
                differing_count, ..
            } => *differing_count > 0,
            Self::Other { .. } => false,
        }
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        swaggen_ui_kit::page_header(
            ui,
            "Publish History & Cherry-Pick",
            "Click any commit row to expand, compare directly against your current workspace, cherry-pick 1 or multiple requests, or revert commits.",
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
                "Connect with Google on Home to publish, cherry-pick, and revert shared API commits.",
            );
            ui.add_space(8.0);
            if swaggen_ui_kit::primary_button(ui, true, "Go to Home").clicked() {
                app.show_welcome = true;
            }
            return;
        };

        let Some((pid, wid)) = app.active_team_target() else {
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Info,
                "Select a Team Workspace",
                "Choose a Team Workspace below to inspect commit history, cherry-pick, or revert changes.",
            );
            ui.add_space(8.0);
            if app.team_workspaces.is_empty() {
                ui.horizontal(|ui| {
                    if swaggen_ui_kit::primary_button(ui, !app.pending, "Open Home Workspace Hub")
                        .clicked()
                    {
                        app.show_welcome = true;
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
                    let acc = swaggen_ui_kit::workspace_accent(&ws.workspace_id, true);
                    egui::Frame::new()
                        .fill(acc.bg_tint)
                        .stroke(egui::Stroke::new(1.0, acc.border))
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.strong(&ws.name);
                                swaggen_ui_kit::pill_badge(ui, acc.dot, ws.role.as_str());
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
        let ws_acc = swaggen_ui_kit::workspace_accent(&wid, true);
        let dirty_count = app
            .state
            .requests
            .iter()
            .filter(|r| r.dirty_vs_shared)
            .count();

        // Top Toolbar Card
        swaggen_ui_kit::surface_card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                swaggen_ui_kit::pill_badge(ui, ws_acc.dot, &ws_name);
                ui.label(
                    egui::RichText::new(format!("{} commits", app.publish_history.len()))
                        .size(11.5)
                        .color(Tokens::TEXT_SECONDARY),
                );
                if swaggen_ui_kit::secondary_button(ui, !app.pending, "Refresh").clicked() {
                    app.submit(Command::LoadPublishHistory {
                        profile_id: pid.clone(),
                        workspace_id: wid.clone(),
                    });
                }

                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("Compare:")
                        .size(11.5)
                        .color(Tokens::TEXT_SECONDARY),
                );
                if ui
                    .selectable_label(app.history_compare_current, "vs Current Workspace")
                    .on_hover_text(
                        "Compare any historical commit directly against your current live workspace",
                    )
                    .clicked()
                {
                    app.history_compare_current = true;
                }
                if ui
                    .selectable_label(!app.history_compare_current, "vs Previous Commit")
                    .on_hover_text("Inspect only what each commit changed relative to its predecessor")
                    .clicked()
                {
                    app.history_compare_current = false;
                }
            });

            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.history_filter_query,
                    "Filter by request, author, note, or #cursor...",
                    260.0,
                    false,
                );
                swaggen_ui_kit::text_input(
                    ui,
                    &mut app.publish_note_input,
                    "Commit note (default: Ctrl+S)",
                    200.0,
                    false,
                );
                if dirty_count > 0 {
                    if swaggen_ui_kit::primary_button(
                        ui,
                        !app.pending,
                        &format!("Publish modified ({dirty_count})"),
                    )
                    .on_hover_text("Publish all modified / restored requests to Team")
                    .clicked()
                    {
                        app.publish_all_dirty_requests();
                    }
                } else if swaggen_ui_kit::primary_button(
                    ui,
                    !app.pending,
                    "Publish active (Ctrl+S)",
                )
                .clicked()
                {
                    let note = std::mem::take(&mut app.publish_note_input);
                    app.save_workspace_local_only();
                    app.publish_current_on_save(Some(&note));
                }
                ui.checkbox(&mut app.history_auto_publish, "Auto-publish on apply")
                    .on_hover_text(
                        "When off, Cherry-pick & Revert apply cleanly to your current local workspace until you press Ctrl+S.",
                    );
            });
        });

        ui.add_space(10.0);

        if app.publish_history.is_empty() {
            swaggen_ui_kit::notice_banner(
                ui,
                NoticeKind::Info,
                "No commits recorded yet",
                "Edit any API in Workbench and press Ctrl+S to create the first sequential commit.",
            );
            return;
        }

        // Index current workspace requests by ID for live comparison
        let current_req_map: HashMap<String, RequestItem> = app
            .state
            .requests
            .iter()
            .map(|r| (r.id.clone(), r.clone()))
            .collect();

        let events = app.publish_history.clone();
        let mut entries: Vec<CommitEntry> = Vec::with_capacity(events.len());
        for (idx, ev) in events.iter().enumerate() {
            if ev.entity_kind == "request"
                && let Some(hist_req) = ev
                    .payload_json
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<RequestItem>(raw).ok())
            {
                let prev_req = events[idx + 1..]
                    .iter()
                    .find(|older| older.entity_id == ev.entity_id && older.entity_kind == "request")
                    .and_then(|older| older.payload_json.as_deref())
                    .and_then(|raw| serde_json::from_str::<RequestItem>(raw).ok());
                let cur_live_req = current_req_map.get(&hist_req.id).cloned();
                let diffs_vs_current = diff_request_snapshots(cur_live_req.as_ref(), &hist_req);
                let diffs_vs_prev = diff_request_snapshots(prev_req.as_ref(), &hist_req);
                entries.push(CommitEntry::Request {
                    ev: ev.clone(),
                    hist_req: Box::new(hist_req),
                    prev_req: prev_req.map(Box::new),
                    cur_live_req: cur_live_req.map(Box::new),
                    diffs_vs_current,
                    diffs_vs_prev,
                });
            } else if ev.entity_kind == "workspace"
                && let Some(ws_snap) = ev
                    .payload_json
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<WorkspaceState>(raw).ok())
            {
                let mut rows = Vec::with_capacity(ws_snap.requests.len());
                let mut diff_cnt = 0usize;
                for hreq in ws_snap.requests {
                    let cur_opt = current_req_map.get(&hreq.id).cloned();
                    let diffs = diff_request_snapshots(cur_opt.as_ref(), &hreq);
                    if cur_opt.is_none() || !diffs.is_empty() {
                        diff_cnt += 1;
                    }
                    rows.push((hreq, cur_opt, diffs));
                }
                entries.push(CommitEntry::Workspace {
                    ev: ev.clone(),
                    requests: rows,
                    differing_count: diff_cnt,
                });
            } else {
                entries.push(CommitEntry::Other { ev: ev.clone() });
            }
        }

        // Filter commits
        let fq = app.history_filter_query.trim().to_ascii_lowercase();
        let filtered_indices: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                if fq.is_empty() {
                    return true;
                }
                let ev = entry.event();
                if format!("#{}", ev.cursor).contains(&fq)
                    || ev.entity_name.to_ascii_lowercase().contains(&fq)
                    || ev.actor_email.to_ascii_lowercase().contains(&fq)
                    || ev.note.to_ascii_lowercase().contains(&fq)
                {
                    return true;
                }
                if let CommitEntry::Workspace { requests, .. } = entry {
                    return requests
                        .iter()
                        .any(|(r, _, _)| r.name.to_ascii_lowercase().contains(&fq));
                }
                false
            })
            .map(|(i, _)| i)
            .collect();

        // Compute multi-selection totals
        let mut selected_commit_reqs: Vec<(i64, RequestItem)> = Vec::new();
        let mut selected_revert_targets: Vec<(ChangeEventRecord, Option<RequestItem>)> = Vec::new();
        let mut seen_req_ids: BTreeSet<String> = BTreeSet::new();

        for &idx in &filtered_indices {
            let entry = &entries[idx];
            let cursor = entry.event().cursor;
            if app.selected_history_cursors.contains(&cursor) {
                match entry {
                    CommitEntry::Request {
                        ev,
                        hist_req,
                        prev_req,
                        ..
                    } => {
                        selected_revert_targets.push((ev.clone(), prev_req.as_deref().cloned()));
                        if seen_req_ids.insert(hist_req.id.clone()) {
                            selected_commit_reqs.push((cursor, hist_req.as_ref().clone()));
                        }
                    }
                    CommitEntry::Workspace { requests, .. } => {
                        for (hreq, cur_opt, diffs) in requests {
                            if (cur_opt.is_none() || !diffs.is_empty())
                                && seen_req_ids.insert(hreq.id.clone())
                            {
                                selected_commit_reqs.push((cursor, hreq.clone()));
                            }
                        }
                    }
                    CommitEntry::Other { .. } => {}
                }
            } else if let CommitEntry::Workspace { requests, .. } = entry {
                for (hreq, _, _) in requests {
                    if app
                        .selected_ws_snapshot_reqs
                        .contains(&(cursor, hreq.id.clone()))
                        && seen_req_ids.insert(hreq.id.clone())
                    {
                        selected_commit_reqs.push((cursor, hreq.clone()));
                    }
                }
            }
        }

        // Clean Batch Action Strip
        let sel_commit_count = app.selected_history_cursors.len();
        let sel_req_count = selected_commit_reqs.len();
        let has_sel = sel_commit_count > 0 || sel_req_count > 0;
        egui::Frame::new()
            .fill(if has_sel {
                swaggen_ui_kit::selected_bg(ui)
            } else {
                swaggen_ui_kit::subtle_bg(ui)
            })
            .stroke(egui::Stroke::new(
                1.0,
                if has_sel {
                    Tokens::STATUS_INFO
                } else {
                    swaggen_ui_kit::border_color(ui)
                },
            ))
            .corner_radius(7)
            .inner_margin(egui::Margin::symmetric(12, 7))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.label(
                        egui::RichText::new(if has_sel {
                            format!("Selected: {sel_req_count} request(s) in {sel_commit_count} commit(s)")
                        } else {
                            "Check 1 or more commits to Cherry-pick or Revert".into()
                        })
                        .size(11.5)
                        .color(if has_sel {
                            Tokens::TEXT_PRIMARY
                        } else {
                            Tokens::TEXT_SECONDARY
                        })
                        .strong(),
                    );
                    if ui.small_button("Select differing").clicked() {
                        app.selected_history_cursors.clear();
                        for &idx in &filtered_indices {
                            if entries[idx].differs_from_current() {
                                app.selected_history_cursors
                                    .insert(entries[idx].event().cursor);
                            }
                        }
                    }
                    if (sel_commit_count > 0 || !app.selected_ws_snapshot_reqs.is_empty())
                        && ui.small_button("Clear").clicked()
                    {
                        app.selected_history_cursors.clear();
                        app.selected_ws_snapshot_reqs.clear();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let rev_cnt = selected_revert_targets.len();
                        if swaggen_ui_kit::danger_button(
                            ui,
                            rev_cnt > 0 && !app.pending,
                            &format!("Revert selected ({rev_cnt})"),
                        )
                        .clicked()
                        {
                            app.revert_publish_events(selected_revert_targets.clone());
                        }
                        if swaggen_ui_kit::primary_button(
                            ui,
                            sel_req_count > 0 && !app.pending,
                            &format!("Cherry-pick selected ({sel_req_count})"),
                        )
                        .clicked()
                        {
                            app.cherry_pick_batch_requests(selected_commit_reqs.clone());
                        }
                    });
                });
            });

        ui.add_space(8.0);

        let mut single_cherry_pick_full: Option<ChangeEventRecord> = None;
        let mut single_cherry_pick_hunks: Option<(i64, RequestItem, Vec<RequestCherryPickHunk>)> =
            None;
        let mut single_revert: Option<(ChangeEventRecord, Option<RequestItem>)> = None;
        let mut batch_apply_ws_reqs: Option<Vec<(i64, RequestItem)>> = None;

        // Unified Clean Git-Fork Commit List Container
        egui::Frame::new()
            .fill(Tokens::BG_SURFACE)
            .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
            .corner_radius(8)
            .inner_margin(egui::Margin::ZERO)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                // Header bar
                egui::Frame::new()
                    .fill(swaggen_ui_kit::subtle_bg(ui))
                    .corner_radius(egui::CornerRadius {
                        nw: 8,
                        ne: 8,
                        sw: 0,
                        se: 0,
                    })
                    .inner_margin(egui::Margin::symmetric(12, 7))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("COMMIT")
                                    .size(10.0)
                                    .color(Tokens::TEXT_SECONDARY)
                                    .strong(),
                            );
                            ui.add_space(16.0);
                            ui.label(
                                egui::RichText::new("REQUEST / SNAPSHOT")
                                    .size(10.0)
                                    .color(Tokens::TEXT_SECONDARY)
                                    .strong(),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new("STATUS & AUTHOR")
                                        .size(10.0)
                                        .color(Tokens::TEXT_SECONDARY)
                                        .strong(),
                                );
                            });
                        });
                    });

                for (row_i, &idx) in filtered_indices.iter().enumerate() {
                    if row_i > 0 {
                        ui.separator();
                    }
                    let entry = &entries[idx];
                    let ev = entry.event();
                    let cursor = ev.cursor;
                    let is_expanded = app.expanded_history_cursors.contains(&cursor);
                    let is_checked = app.selected_history_cursors.contains(&cursor);
                    let is_preview_json = app.preview_history_cursor == Some(cursor);

                    let row_bg = if is_expanded || is_checked {
                        swaggen_ui_kit::selected_bg(ui)
                    } else if row_i % 2 == 1 {
                        egui::Color32::from_rgb(0xFC, 0xFD, 0xFE)
                    } else {
                        Tokens::BG_SURFACE
                    };

                    let row_frame = egui::Frame::new()
                        .fill(row_bg)
                        .inner_margin(egui::Margin::symmetric(12, 8));

                    row_frame.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());

                        // Single Clean Scannable Commit Row (click title area to expand/collapse)
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;

                            let mut chk = is_checked;
                            if ui.checkbox(&mut chk, "").changed() {
                                if chk {
                                    app.selected_history_cursors.insert(cursor);
                                } else {
                                    app.selected_history_cursors.remove(&cursor);
                                }
                            }

                            // Vector Chevron Icon (authentic Figma asset, no ugly unicode boxes)
                            let (chev_rect, chev_resp) =
                                ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
                            swaggen_ui_kit::paint_icon(
                                ui.painter(),
                                chev_rect,
                                if is_expanded {
                                    IconKind::ChevronDown
                                } else {
                                    IconKind::ChevronRight
                                },
                                Tokens::TEXT_SECONDARY,
                            );

                            let ts = format_unix_utc_compact(ev.created_at_unix);
                            let note_str = if ev.note.trim().is_empty() {
                                "Saved"
                            } else {
                                ev.note.trim()
                            };
                            let actor_short = ev
                                .actor_email
                                .split('@')
                                .next()
                                .unwrap_or(&ev.actor_email);

                            // Reserve clean space on right for status + optional Cherry-pick button + author/time
                            let right_reserve = if entry.differs_from_current() { 290.0 } else { 215.0 };
                            let left_w = (ui.available_width() - right_reserve).max(160.0);

                            let left_resp = ui
                                .allocate_ui_with_layout(
                                    egui::vec2(left_w, 22.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = 7.0;
                                        ui.label(
                                            egui::RichText::new(format!("#{cursor}"))
                                                .monospace()
                                                .size(11.5)
                                                .color(Tokens::TEXT_SECONDARY)
                                                .strong(),
                                        );
                                        match entry {
                                            CommitEntry::Request { hist_req, .. } => {
                                                let m_str = if hist_req.is_graphql() {
                                                    "GQL"
                                                } else {
                                                    hist_req.method.as_str()
                                                };
                                                ui.label(
                                                    egui::RichText::new(
                                                        swaggen_ui_kit::compact_method_badge(m_str),
                                                    )
                                                    .monospace()
                                                    .size(10.5)
                                                    .color(swaggen_ui_kit::method_color(m_str))
                                                    .strong(),
                                                );
                                                ui.label(swaggen_ui_kit::bold_text(
                                                    &hist_req.name,
                                                    12.5,
                                                    Tokens::TEXT_PRIMARY,
                                                ));
                                            }
                                            CommitEntry::Workspace { ev, .. } => {
                                                swaggen_ui_kit::pill_badge(
                                                    ui,
                                                    Tokens::METHOD_PATCH,
                                                    "SNAPSHOT",
                                                );
                                                ui.label(swaggen_ui_kit::bold_text(
                                                    &ev.entity_name,
                                                    12.5,
                                                    Tokens::TEXT_PRIMARY,
                                                ));
                                            }
                                            CommitEntry::Other { ev } => {
                                                ui.label(swaggen_ui_kit::bold_text(
                                                    &ev.entity_name,
                                                    12.5,
                                                    Tokens::TEXT_PRIMARY,
                                                ));
                                            }
                                        }
                                        ui.label(
                                            egui::RichText::new(format!("r{} · {note_str}", ev.revision))
                                                .size(11.0)
                                                .color(Tokens::TEXT_SECONDARY),
                                        );
                                    },
                                )
                                .response;

                            let click_area = ui
                                .interact(
                                    left_resp.rect,
                                    egui::Id::unique(("hist_row_toggle", cursor)),
                                    egui::Sense::click(),
                                )
                                .on_hover_text("Click to expand/collapse commit details");
                            if click_area.clicked() || chev_resp.clicked() {
                                if is_expanded {
                                    app.expanded_history_cursors.remove(&cursor);
                                } else {
                                    app.expanded_history_cursors.insert(cursor);
                                }
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;

                                // Show quick Cherry-pick ONLY when commit actually differs from current (reduces row clutter!)
                                if entry.differs_from_current()
                                    && ev.payload_json.is_some()
                                    && !ev.deleted
                                {
                                    let cp_lbl = match entry {
                                        CommitEntry::Workspace { .. } => "Inspect",
                                        _ => "Cherry-pick",
                                    };
                                    if swaggen_ui_kit::secondary_button(ui, !app.pending, cp_lbl)
                                        .clicked()
                                    {
                                        match entry {
                                            CommitEntry::Workspace { .. } => {
                                                app.expanded_history_cursors.insert(cursor);
                                            }
                                            _ => single_cherry_pick_full = Some(ev.clone()),
                                        }
                                    }
                                }

                                ui.label(
                                    egui::RichText::new(format!("{actor_short} · {ts}"))
                                        .size(11.0)
                                        .color(Tokens::TEXT_SECONDARY),
                                );

                                // Clean Status Indicator
                                match entry {
                                    CommitEntry::Request {
                                        prev_req,
                                        cur_live_req,
                                        diffs_vs_current,
                                        diffs_vs_prev,
                                        ..
                                    } => {
                                        if app.history_compare_current {
                                            if cur_live_req.is_none() {
                                                swaggen_ui_kit::pill_badge(
                                                    ui,
                                                    Tokens::TEXT_WARNING,
                                                    "Removed locally",
                                                );
                                            } else if diffs_vs_current.is_empty() {
                                                ui.label(
                                                    egui::RichText::new("In sync")
                                                        .size(11.0)
                                                        .color(Tokens::STATUS_SUCCESS),
                                                );
                                            } else {
                                                swaggen_ui_kit::pill_badge(
                                                    ui,
                                                    Tokens::TEXT_WARNING,
                                                    &format!("{} diff", diffs_vs_current.len()),
                                                );
                                            }
                                        } else if prev_req.is_none() {
                                            swaggen_ui_kit::pill_badge(
                                                ui,
                                                Tokens::STATUS_SUCCESS,
                                                "Created",
                                            );
                                        } else if diffs_vs_prev.is_empty() {
                                            ui.label(
                                                egui::RichText::new("No diff")
                                                    .size(11.0)
                                                    .color(Tokens::TEXT_SECONDARY),
                                            );
                                        } else {
                                            swaggen_ui_kit::pill_badge(
                                                ui,
                                                Tokens::TEXT_WARNING,
                                                &format!("{} changed", diffs_vs_prev.len()),
                                            );
                                        }
                                    }
                                    CommitEntry::Workspace {
                                        requests,
                                        differing_count,
                                        ..
                                    } => {
                                        if *differing_count > 0 {
                                            swaggen_ui_kit::pill_badge(
                                                ui,
                                                Tokens::TEXT_WARNING,
                                                &format!(
                                                    "{differing_count}/{} differ",
                                                    requests.len()
                                                ),
                                            );
                                        } else {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "All {} match",
                                                    requests.len()
                                                ))
                                                .size(11.0)
                                                .color(Tokens::STATUS_SUCCESS),
                                            );
                                        }
                                    }
                                    CommitEntry::Other { .. } => {}
                                }
                            });
                        });

                        // ================= EXPANDED GIT-FORK DETAIL DRAWER =================
                        if is_expanded {
                            ui.add_space(8.0);
                            egui::Frame::new()
                                .fill(Tokens::BG_SURFACE)
                                .stroke(egui::Stroke::new(
                                    1.0,
                                    swaggen_ui_kit::border_color(ui),
                                ))
                                .corner_radius(6)
                                .inner_margin(egui::Margin::symmetric(12, 10))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());

                                    // Detail Action Bar inside Expanded Drawer
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "Commit #{cursor} · {} · {}",
                                                ev.actor_email,
                                                format_unix_utc(ev.created_at_unix)
                                            ))
                                            .size(11.0)
                                            .color(Tokens::TEXT_SECONDARY),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ev.payload_json.is_some()
                                                    && swaggen_ui_kit::secondary_button(
                                                        ui,
                                                        true,
                                                        if is_preview_json {
                                                            "Hide JSON"
                                                        } else {
                                                            "View JSON"
                                                        },
                                                    )
                                                    .clicked()
                                                {
                                                    app.preview_history_cursor = if is_preview_json {
                                                        None
                                                    } else {
                                                        Some(cursor)
                                                    };
                                                }
                                                if let CommitEntry::Request { ev, prev_req, .. } =
                                                    entry
                                                    && swaggen_ui_kit::danger_button(
                                                        ui,
                                                        !app.pending,
                                                        "Revert commit",
                                                    )
                                                    .clicked()
                                                {
                                                    single_revert = Some((
                                                        ev.clone(),
                                                        prev_req.as_deref().cloned(),
                                                    ));
                                                }
                                                if ev.payload_json.is_some() && !ev.deleted {
                                                    let lbl = match entry {
                                                        CommitEntry::Workspace { .. } => {
                                                            "Restore entire snapshot"
                                                        }
                                                        _ => "Cherry-pick full request",
                                                    };
                                                    if swaggen_ui_kit::primary_button(
                                                        ui,
                                                        !app.pending,
                                                        lbl,
                                                    )
                                                    .clicked()
                                                    {
                                                        single_cherry_pick_full = Some(ev.clone());
                                                    }
                                                }
                                            },
                                        );
                                    });

                                    ui.add_space(6.0);
                                    ui.separator();
                                    ui.add_space(6.0);

                                    match entry {
                                        CommitEntry::Request {
                                            ev,
                                            hist_req,
                                            prev_req,
                                            cur_live_req,
                                            diffs_vs_current,
                                            diffs_vs_prev,
                                        } => {
                                            let active_diffs = if app.history_compare_current {
                                                diffs_vs_current
                                            } else {
                                                diffs_vs_prev
                                            };

                                            if active_diffs.is_empty() {
                                                ui.horizontal_wrapped(|ui| {
                                                    let msg = if app.history_compare_current {
                                                        "Current workspace is identical to this commit."
                                                    } else {
                                                        "No changes compared to the previous commit. Switch Compare mode to 'vs Current Workspace' to compare with your live state."
                                                    };
                                                    ui.label(
                                                        egui::RichText::new(msg)
                                                            .size(11.5)
                                                            .color(Tokens::TEXT_SECONDARY),
                                                    );
                                                    if cur_live_req.is_some()
                                                        && ui
                                                            .small_button("Open in Workbench")
                                                            .clicked()
                                                    {
                                                        app.open_request_in_workbench(&hist_req.id);
                                                    }
                                                });
                                            } else {
                                                let all_hunks: BTreeSet<RequestCherryPickHunk> =
                                                    active_diffs.iter().map(|d| d.hunk).collect();
                                                let sel_set = app
                                                    .cherry_pick_selections
                                                    .entry(cursor)
                                                    .or_insert_with(|| all_hunks.clone());

                                                ui.horizontal_wrapped(|ui| {
                                                    let dir_hint = if app.history_compare_current {
                                                        if cur_live_req.is_none() {
                                                            format!("Removed locally — all sections of #{} available to restore (+):", ev.cursor)
                                                        } else {
                                                            format!("Diff: Current Workspace (-) vs Commit #{} (+)", ev.cursor)
                                                        }
                                                    } else if prev_req.is_some() {
                                                        format!("Changes in Commit #{} vs previous commit:", ev.cursor)
                                                    } else {
                                                        format!("Initial snapshot in Commit #{}:", ev.cursor)
                                                    };
                                                    ui.label(
                                                        egui::RichText::new(dir_hint)
                                                            .size(11.5)
                                                            .color(Tokens::TEXT_PRIMARY)
                                                            .strong(),
                                                    );
                                                    if ui.small_button("All").clicked() {
                                                        *sel_set = all_hunks.clone();
                                                    }
                                                    if ui.small_button("None").clicked() {
                                                        sel_set.clear();
                                                    }
                                                });
                                                ui.add_space(6.0);

                                                for diff in active_diffs {
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
                                                                    Tokens::TEXT_PRIMARY,
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
                                                ui.add_space(4.0);
                                                ui.horizontal_wrapped(|ui| {
                                                    if swaggen_ui_kit::primary_button(
                                                        ui,
                                                        sel_count > 0 && !app.pending,
                                                        &format!("Apply selected sections ({sel_count})"),
                                                    )
                                                    .clicked()
                                                    {
                                                        single_cherry_pick_hunks = Some((
                                                            cursor,
                                                            hist_req.as_ref().clone(),
                                                            selected_list,
                                                        ));
                                                    }
                                                    if swaggen_ui_kit::secondary_button(
                                                        ui,
                                                        !app.pending,
                                                        "Open in Workbench",
                                                    )
                                                    .clicked()
                                                    {
                                                        app.open_request_in_workbench(&hist_req.id);
                                                    }
                                                });
                                            }
                                        }
                                        CommitEntry::Workspace {
                                            requests,
                                            differing_count,
                                            ..
                                        } => {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "Snapshot contains {} request(s) ({} differ from current).",
                                                        requests.len(),
                                                        differing_count
                                                    ))
                                                    .size(11.5)
                                                    .color(Tokens::TEXT_PRIMARY),
                                                );
                                                ui.checkbox(
                                                    &mut app.history_show_matching_ws_reqs,
                                                    "Show identical",
                                                );
                                                if ui.small_button("Check differing").clicked() {
                                                    for (hreq, cur_opt, diffs) in requests {
                                                        if cur_opt.is_none() || !diffs.is_empty() {
                                                            app.selected_ws_snapshot_reqs
                                                                .insert((cursor, hreq.id.clone()));
                                                        }
                                                    }
                                                }
                                            });
                                            ui.add_space(6.0);

                                            let mut checked_in_ws: Vec<(i64, RequestItem)> = Vec::new();
                                            for (hreq, cur_opt, diffs) in requests {
                                                let differs = cur_opt.is_none() || !diffs.is_empty();
                                                if !differs && !app.history_show_matching_ws_reqs {
                                                    continue;
                                                }
                                                let req_key = (cursor, hreq.id.clone());
                                                let mut req_chk = app
                                                    .selected_ws_snapshot_reqs
                                                    .contains(&req_key);
                                                let req_exp = app
                                                    .expanded_ws_snapshot_reqs
                                                    .contains(&req_key);
                                                if req_chk {
                                                    checked_in_ws.push((cursor, hreq.clone()));
                                                }

                                                egui::Frame::new()
                                                    .fill(swaggen_ui_kit::subtle_bg(ui))
                                                    .stroke(egui::Stroke::new(
                                                        1.0,
                                                        if req_chk || req_exp {
                                                            Tokens::STATUS_INFO
                                                        } else {
                                                            swaggen_ui_kit::border_color(ui)
                                                        },
                                                    ))
                                                    .corner_radius(5)
                                                    .inner_margin(egui::Margin::symmetric(8, 5))
                                                    .show(ui, |ui| {
                                                        ui.horizontal(|ui| {
                                                            if ui.checkbox(&mut req_chk, "").changed()
                                                            {
                                                                if req_chk {
                                                                    app.selected_ws_snapshot_reqs
                                                                        .insert(req_key.clone());
                                                                } else {
                                                                    app.selected_ws_snapshot_reqs
                                                                        .remove(&req_key);
                                                                }
                                                            }
                                                            let (cr_rect, cr_resp) =
                                                                ui.allocate_exact_size(
                                                                    egui::vec2(12.0, 12.0),
                                                                    egui::Sense::click(),
                                                                );
                                                            swaggen_ui_kit::paint_icon(
                                                                ui.painter(),
                                                                cr_rect,
                                                                if req_exp {
                                                                    IconKind::ChevronDown
                                                                } else {
                                                                    IconKind::ChevronRight
                                                                },
                                                                Tokens::TEXT_SECONDARY,
                                                            );
                                                            let rlbl_resp = ui.add(
                                                                egui::Button::new(
                                                                    egui::RichText::new(format!(
                                                                        "{}  {}",
                                                                        hreq.method.as_str(),
                                                                        hreq.name
                                                                    ))
                                                                    .size(12.0)
                                                                    .color(Tokens::TEXT_PRIMARY)
                                                                    .strong(),
                                                                )
                                                                .fill(egui::Color32::TRANSPARENT)
                                                                .stroke(egui::Stroke::NONE),
                                                            );
                                                            if rlbl_resp.clicked() || cr_resp.clicked()
                                                            {
                                                                if req_exp {
                                                                    app.expanded_ws_snapshot_reqs
                                                                        .remove(&req_key);
                                                                } else {
                                                                    app.expanded_ws_snapshot_reqs
                                                                        .insert(req_key.clone());
                                                                }
                                                            }
                                                            if cur_opt.is_none() {
                                                                swaggen_ui_kit::pill_badge(
                                                                    ui,
                                                                    Tokens::TEXT_WARNING,
                                                                    "Removed locally",
                                                                );
                                                            } else if diffs.is_empty() {
                                                                ui.label(
                                                                    egui::RichText::new("In sync")
                                                                        .size(11.0)
                                                                        .color(Tokens::STATUS_SUCCESS),
                                                                );
                                                            } else {
                                                                swaggen_ui_kit::pill_badge(
                                                                    ui,
                                                                    Tokens::TEXT_WARNING,
                                                                    &format!("{} diff", diffs.len()),
                                                                );
                                                            }
                                                            ui.with_layout(
                                                                egui::Layout::right_to_left(
                                                                    egui::Align::Center,
                                                                ),
                                                                |ui| {
                                                                    if swaggen_ui_kit::secondary_button(
                                                                        ui,
                                                                        !app.pending,
                                                                        "Cherry-pick",
                                                                    )
                                                                    .clicked()
                                                                    {
                                                                        batch_apply_ws_reqs =
                                                                            Some(vec![(
                                                                                cursor,
                                                                                hreq.clone(),
                                                                            )]);
                                                                    }
                                                                },
                                                            );
                                                        });

                                                        if req_exp && !diffs.is_empty() {
                                                            ui.add_space(4.0);
                                                            let all_h: BTreeSet<RequestCherryPickHunk> =
                                                                diffs.iter().map(|d| d.hunk).collect();
                                                            let hset = app
                                                                .cherry_pick_ws_req_selections
                                                                .entry(req_key.clone())
                                                                .or_insert_with(|| all_h.clone());
                                                            for d in diffs {
                                                                let mut hc = hset.contains(&d.hunk);
                                                                ui.horizontal(|ui| {
                                                                    if ui.checkbox(&mut hc, "").changed()
                                                                    {
                                                                        if hc {
                                                                            hset.insert(d.hunk);
                                                                        } else {
                                                                            hset.remove(&d.hunk);
                                                                        }
                                                                    }
                                                                    ui.strong(d.label);
                                                                });
                                                                for line in &d.before_lines {
                                                                    render_diff_line(ui, false, line);
                                                                }
                                                                for line in &d.after_lines {
                                                                    render_diff_line(ui, true, line);
                                                                }
                                                            }
                                                            let picked_h: Vec<RequestCherryPickHunk> =
                                                                hset.iter().copied().collect();
                                                            if !picked_h.is_empty()
                                                                && swaggen_ui_kit::primary_button(
                                                                    ui,
                                                                    !app.pending,
                                                                    &format!(
                                                                        "Apply sections ({})",
                                                                        picked_h.len()
                                                                    ),
                                                                )
                                                                .clicked()
                                                            {
                                                                single_cherry_pick_hunks = Some((
                                                                    cursor,
                                                                    hreq.clone(),
                                                                    picked_h,
                                                                ));
                                                            }
                                                        }
                                                    });
                                                ui.add_space(3.0);
                                            }

                                            if !checked_in_ws.is_empty()
                                                && swaggen_ui_kit::primary_button(
                                                    ui,
                                                    !app.pending,
                                                    &format!(
                                                        "Cherry-pick checked ({})",
                                                        checked_in_ws.len()
                                                    ),
                                                )
                                                .clicked()
                                            {
                                                batch_apply_ws_reqs = Some(checked_in_ws);
                                            }
                                        }
                                        CommitEntry::Other { ev } => {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "Event kind '{}' ({}) · revision r{}",
                                                    ev.entity_kind, ev.entity_name, ev.revision
                                                ))
                                                .size(11.5)
                                                .color(Tokens::TEXT_SECONDARY),
                                            );
                                        }
                                    }

                                    if is_preview_json && let Some(ref raw) = ev.payload_json {
                                        ui.add_space(6.0);
                                        let pretty =
                                            serde_json::from_str::<serde_json::Value>(raw)
                                                .ok()
                                                .and_then(|v| serde_json::to_string_pretty(&v).ok())
                                                .unwrap_or_else(|| raw.clone());
                                        swaggen_ui_kit::code_surface_numbered(ui, &pretty);
                                    }
                                });
                        }
                    });
                }
            });

        if let Some((cursor, source, hunks)) = single_cherry_pick_hunks {
            app.cherry_pick_ws_request_hunks(cursor, &source, &hunks);
        } else if let Some(items) = batch_apply_ws_reqs {
            app.cherry_pick_batch_requests(items);
        } else if let Some(target) = single_revert {
            app.revert_publish_events(vec![target]);
        } else if let Some(ev) = single_cherry_pick_full {
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

fn format_unix_utc_compact(unix: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(unix)
        .map(|dt| {
            format!(
                "{:02}-{:02} {:02}:{:02}",
                u8::from(dt.month()),
                dt.day(),
                dt.hour(),
                dt.minute()
            )
        })
        .unwrap_or_else(|_| unix.to_string())
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
