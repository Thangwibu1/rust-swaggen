//! Compact, clean Figma Project Sidebar:
//! - Aligned Search + Filter bar (zero border overlap)
//! - Drag-and-drop requests across Collections & Folders
//! - `⋯` action menu on every Request & Collection (Save draft, Duplicate, Rename, Move to collection, Delete)
//! - Pointer-locked vertical splitter between `COLLECTIONS` and `PERSONAL` (zero empty bottom gap)
use crate::app::{App, MainMode};
use eframe::egui;
use swaggen_domain::workspace::{Collection, Folder, HttpMethod, RequestItem, new_uid};
use swaggen_ui_kit::{IconKind, Tokens};

#[derive(Default)]
struct SidebarActions {
    open_request_id: Option<String>,
    add_req_target: Option<(String, Option<String>)>,
    add_gql_target: Option<(String, Option<String>)>,
    add_folder_target: Option<(String, Option<String>)>,
    rename_folder: Option<(String, String)>,
    del_folder_id: Option<String>,
    dup_req_id: Option<String>,
    del_req_id: Option<String>,
    reorder_req: Option<(String, bool)>,
    move_req_to: Option<(String, String, Option<String>)>,
    rename_req: Option<(String, String)>,
    save_draft_now: bool,
    fork_col_id: Option<String>,
    del_col_id: Option<String>,
    rename_col: Option<(String, String)>,
    recolor_col: Option<(String, Option<String>)>,
}

const COLLECTION_COLOR_SWATCHES: &[(&str, Option<&str>, egui::Color32)] = &[
    ("Default", None, Tokens::METHOD_POST),
    (
        "Blue",
        Some("#2563EB"),
        egui::Color32::from_rgb(0x25, 0x63, 0xEB),
    ),
    (
        "Teal",
        Some("#0F766E"),
        egui::Color32::from_rgb(0x0F, 0x76, 0x6E),
    ),
    (
        "Emerald",
        Some("#15803D"),
        egui::Color32::from_rgb(0x15, 0x80, 0x3D),
    ),
    (
        "Amber",
        Some("#D97706"),
        egui::Color32::from_rgb(0xD9, 0x77, 0x06),
    ),
    (
        "Rose",
        Some("#DC2626"),
        egui::Color32::from_rgb(0xDC, 0x26, 0x26),
    ),
    (
        "Purple",
        Some("#7C3AED"),
        egui::Color32::from_rgb(0x7C, 0x3A, 0xED),
    ),
    (
        "Slate",
        Some("#475569"),
        egui::Color32::from_rgb(0x47, 0x55, 0x69),
    ),
];

fn resolve_collection_color(hex: Option<&str>) -> egui::Color32 {
    if let Some(h) = hex {
        for (_, code, col) in COLLECTION_COLOR_SWATCHES {
            if *code == Some(h) {
                return *col;
            }
        }
    }
    Tokens::METHOD_POST
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let sidebar_full_rect = ui.max_rect();

    // 1. Clean Full-Width Workspace Switcher Pill (no redundant "SWAGGEN" text)
    ui.horizontal(|ui| {
        let is_team = app.selected_team_workspace_id.is_some();
        let kind_badge = if is_team { "Team" } else { "Local" };
        let kind_icon = if is_team { "👥" } else { "📁" };
        let ws_btn_w = (ui.available_width() - 2.0).max(120.0);
        let max_ws_chars = ((ws_btn_w - 86.0) / 7.0).max(6.0) as usize;
        let ws_display = app.active_workspace_display_name();
        let ws_short: String = if ws_display.chars().count() > max_ws_chars {
            let mut s: String = ws_display
                .chars()
                .take(max_ws_chars.saturating_sub(1))
                .collect();
            s.push('…');
            s
        } else {
            ws_display
        };
        let ws_btn = ui
            .add_sized(
                [ws_btn_w, 30.0],
                egui::Button::new(
                    egui::RichText::new(format!("{kind_icon}  {ws_short}  ·  {kind_badge}  ▾"))
                        .size(11.5)
                        .color(swaggen_ui_kit::text_primary(ui))
                        .strong(),
                )
                .fill(swaggen_ui_kit::subtle_bg(ui))
                .stroke(egui::Stroke::new(
                    1.0,
                    if is_team {
                        Tokens::STATUS_INFO
                    } else {
                        swaggen_ui_kit::border_color(ui)
                    },
                ))
                .corner_radius(7),
            )
            .on_hover_text("Switch Individual or Team Workspace");

        let mut switch_local_ws: Option<swaggen_domain::LocalId> = None;
        let mut switch_team_ws: Option<(String, String)> = None;
        let mut open_ws_manager = false;
        let local_ws_list: Vec<(swaggen_domain::LocalId, String)> =
            if app.workspace_summaries.is_empty() {
                app.available_workspaces
                    .iter()
                    .filter(|w| !w.as_str().starts_with("team-") && w.as_str() != "server-auth")
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
                        w.kind == swaggen_domain::workspace::WorkspaceKind::Personal
                            && !w.id.as_str().starts_with("team-")
                            && w.id.as_str() != "server-auth"
                    })
                    .map(|w| {
                        let label = if w.id.as_str() == "personal" && w.name == "personal" {
                            "Personal".to_owned()
                        } else {
                            w.name.clone()
                        };
                        (w.id.clone(), label)
                    })
                    .collect()
            };
        let cur_ws_id = app.workspace_id.clone();
        let cur_team_id = app.selected_team_workspace_id.clone();
        let team_ws_list: Vec<(String, String, String)> = app
            .team_workspaces
            .iter()
            .map(|tw| {
                (
                    tw.workspace_id.clone(),
                    tw.name.clone(),
                    format!("{:?}", tw.role),
                )
            })
            .collect();

        egui::Popup::menu(&ws_btn)
            .id(egui::Id::unique("sidebar_ws_switcher_popup"))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(215.0);
                ui.label(
                    egui::RichText::new("INDIVIDUAL WORKSPACES (LOCAL)")
                        .size(10.0)
                        .color(swaggen_ui_kit::text_secondary(ui))
                        .strong(),
                );
                for (wid, wname) in &local_ws_list {
                    let is_active = cur_team_id.is_none() && *wid == cur_ws_id;
                    if ui
                        .selectable_label(is_active, format!("📁 {wname}"))
                        .clicked()
                    {
                        switch_local_ws = Some(wid.clone());
                        ui.close();
                    }
                }
                if !team_ws_list.is_empty() {
                    ui.separator();
                    ui.label(
                        egui::RichText::new("TEAM WORKSPACES")
                            .size(10.0)
                            .color(swaggen_ui_kit::text_secondary(ui))
                            .strong(),
                    );
                    for (tid, tname, role_str) in &team_ws_list {
                        let is_active = cur_team_id.as_deref() == Some(tid.as_str());
                        if ui
                            .selectable_label(is_active, format!("👥 {tname} ({role_str})"))
                            .clicked()
                        {
                            switch_team_ws = Some((tid.clone(), tname.clone()));
                            ui.close();
                        }
                    }
                }
                ui.separator();
                if ui.button("+ Create or Manage Workspaces…").clicked() {
                    open_ws_manager = true;
                    ui.close();
                }
            });

        if let Some(target_id) = switch_local_ws {
            app.selected_team_workspace_id = None;
            if target_id != app.workspace_id {
                app.submit(crate::worker::Command::SwitchWorkspace { id: target_id });
            }
        }
        if let Some((tid, tname)) = switch_team_ws {
            app.selected_team_workspace_id = Some(tid.clone());
            app.state.meta.name = tname.clone();
            app.status = format!("Active team workspace: {tname}");
            if let Some(pid) = app.server_profiles.active().map(|p| p.id.clone()) {
                app.submit(crate::worker::Command::LoadTeamWorkspaceDetails {
                    profile_id: pid.clone(),
                    workspace_id: tid.clone(),
                });
                app.submit(crate::worker::Command::SyncNow {
                    profile_id: pid,
                    workspace_id: tid,
                });
            }
        }
        if open_ws_manager {
            app.hide_secret();
            app.show_command_palette = false;
            app.show_production_confirm = false;
            app.show_workspace_modal = true;
        }
    });
    ui.add_space(5.0);

    // 2. Compact Search APIs + aligned Filter icon button (fits 100% inside sidebar without clipping)
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let search_w = (ui.available_width() - 42.0).max(60.0);
        let search_resp = swaggen_ui_kit::text_input(
            ui,
            &mut app.search_query,
            "Search APIs (Ctrl+K)",
            search_w,
            false,
        );
        if app.focus_search {
            search_resp.request_focus();
            app.focus_search = false;
        }
        let f_active =
            app.show_sidebar_filters || app.filter_method.is_some() || !app.filter_tag.is_empty();
        if swaggen_ui_kit::filter_icon_button(ui, f_active).clicked() {
            app.show_sidebar_filters = !app.show_sidebar_filters;
        }
    });

    if app.show_sidebar_filters {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let method_label = app.filter_method.map(|m| m.as_str()).unwrap_or("All");
            egui::ComboBox::from_id_salt("filter-method")
                .selected_text(method_label)
                .width(76.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.filter_method, None, "All");
                    for m in HttpMethod::ALL {
                        ui.selectable_value(&mut app.filter_method, Some(m), m.as_str());
                    }
                });
            let tags = app.state.all_tags();
            let tag_label = if app.filter_tag.is_empty() {
                "All tags".to_owned()
            } else {
                app.filter_tag.clone()
            };
            egui::ComboBox::from_id_salt("filter-tag")
                .selected_text(tag_label)
                .width(82.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.filter_tag, String::new(), "All tags");
                    for t in tags {
                        ui.selectable_value(&mut app.filter_tag, t.clone(), t);
                    }
                });
        });
    }

    ui.add_space(4.0);
    if swaggen_ui_kit::nav_item(
        ui,
        IconKind::Folder,
        "Collections",
        Some(app.state.requests.len()),
        app.mode == MainMode::Workbench && !app.show_workspace_modal,
    )
    .clicked()
    {
        app.navigate_to(MainMode::Workbench);
    }
    if swaggen_ui_kit::nav_item(
        ui,
        IconKind::FileCode,
        "API definitions",
        Some(app.specs.len()),
        app.mode == MainMode::Specs && !app.show_workspace_modal,
    )
    .clicked()
    {
        app.navigate_to(MainMode::Specs);
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("COLLECTIONS")
                .size(10.5)
                .color(Tokens::TEXT_SECONDARY)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if swaggen_ui_kit::mini_icon_button(
                ui,
                "sidebar_new_collection_btn",
                swaggen_ui_kit::MiniIconKind::Plus,
                Tokens::STATUS_INFO,
                "Create new collection",
            )
            .clicked()
            {
                app.show_new_collection_input = !app.show_new_collection_input;
            }
        });
    });

    if app.show_new_collection_input {
        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut app.new_collection_name)
                    .hint_text("Collection name…")
                    .desired_width((ui.available_width() - 48.0).max(70.0)),
            );
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (swaggen_ui_kit::mini_icon_button(
                ui,
                "sidebar_confirm_new_col",
                swaggen_ui_kit::MiniIconKind::Plus,
                Tokens::STATUS_SUCCESS,
                "Confirm create collection",
            )
            .clicked()
                || enter)
                && !app.new_collection_name.trim().is_empty()
            {
                let col = Collection {
                    id: new_uid(),
                    name: app.new_collection_name.trim().to_owned(),
                    description: String::new(),
                    color: None,
                    auth: Default::default(),
                    variables: Vec::new(),
                    pre_request_script: String::new(),
                    post_response_script: String::new(),
                    order_key: app.state.collections.len() as u64,
                };
                app.state.collections.push(col);
                app.new_collection_name.clear();
                app.show_new_collection_input = false;
                app.mark_dirty();
            }
        });
    }

    let filtering = !app.search_query.trim().is_empty()
        || app.filter_method.is_some()
        || !app.filter_tag.is_empty();

    let mut acts = SidebarActions::default();
    let all_collections_meta: Vec<(String, String)> = app
        .state
        .collections
        .iter()
        .map(|c| (c.id.clone(), c.name.clone()))
        .collect();
    let can_delete_col = all_collections_meta.len() > 1;
    let is_dirty = app.dirty();
    let pointer_pos = ui
        .ctx()
        .pointer_hover_pos()
        .or_else(|| ui.ctx().pointer_interact_pos());
    let pointer_released = !ui.ctx().input(|i| i.pointer.any_down());

    // Exact height calculation with `.auto_shrink([false, false])` so `PERSONAL` stays anchored without clipping!
    let footer_reserved = if app.state.layout.sidebar_personal_collapsed {
        44.0
    } else {
        app.state.layout.sidebar_footer_height.clamp(126.0, 230.0)
    };
    let tree_height = (ui.available_height() - footer_reserved - 10.0).max(68.0);
    let show_endpoint_selection = app.mode == MainMode::Workbench && !app.show_workspace_modal;

    egui::ScrollArea::vertical()
        .id_salt("sidebar-tree-scroll")
        .min_scrolled_height(tree_height)
        .max_height(tree_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let active_req_id = if show_endpoint_selection {
                app.active_request_id()
            } else {
                None
            };
            if filtering {
                let matches: Vec<RequestItem> = app
                    .state
                    .search_requests(&app.search_query, app.filter_method, Some(&app.filter_tag))
                    .into_iter()
                    .cloned()
                    .collect();
                if matches.is_empty() {
                    ui.label(
                        egui::RichText::new("No matching requests")
                            .size(11.5)
                            .color(Tokens::TEXT_SECONDARY),
                    );
                } else {
                    for req in &matches {
                        render_request_row(
                            ui,
                            req,
                            active_req_id.as_deref() == Some(req.id.as_str()),
                            &all_collections_meta,
                            is_dirty,
                            &mut app.dragging_request_id,
                            &mut acts,
                        );
                    }
                }
            } else {
                for col in &app.state.collections {
                    let col_state_id = ui.make_persistent_id(("col_open", &col.id));
                    let mut is_open = ui
                        .ctx()
                        .data_mut(|d| *d.get_persisted_mut_or(col_state_id, true));

                    let row_w = ui.available_width().max(100.0);
                    let (row_rect, row_resp) =
                        ui.allocate_exact_size(egui::vec2(row_w, 26.0), egui::Sense::hover());

                    // Highlight collection when dragging a request over it!
                    let is_drop_hovered = app.dragging_request_id.is_some()
                        && pointer_pos.is_some_and(|p| row_rect.contains(p));
                    if is_drop_hovered {
                        ui.painter().rect(
                            row_rect,
                            5.0,
                            Tokens::BG_SELECTED,
                            egui::Stroke::new(1.5, Tokens::STATUS_INFO),
                            egui::StrokeKind::Middle,
                        );
                        if pointer_released && let Some(drag_id) = app.dragging_request_id.clone() {
                            acts.move_req_to = Some((drag_id, col.id.clone(), None));
                            is_open = true;
                            ui.ctx()
                                .data_mut(|d| d.insert_persisted(col_state_id, true));
                        }
                    } else if row_resp.hovered() {
                        ui.painter().rect_filled(row_rect, 5.0, Tokens::BG_SUBTLE);
                    }

                    // Expand/collapse click zone (leaving 44px on right for `+` and `⋯`)
                    let toggle_rect = egui::Rect::from_min_max(
                        row_rect.min,
                        egui::pos2(
                            (row_rect.right() - 44.0).max(row_rect.left() + 30.0),
                            row_rect.bottom(),
                        ),
                    );
                    let toggle_resp = ui.interact(
                        toggle_rect,
                        ui.scope_id().with(("col_toggle", &col.id)),
                        egui::Sense::click(),
                    );
                    if toggle_resp.clicked() {
                        is_open = !is_open;
                        ui.ctx()
                            .data_mut(|d| d.insert_persisted(col_state_id, is_open));
                    }

                    let col_tint = resolve_collection_color(col.color.as_deref());
                    // Accent left color stripe on Collection row
                    if col.color.is_some() {
                        ui.painter().line_segment(
                            [
                                egui::pos2(row_rect.left() + 1.5, row_rect.top() + 4.0),
                                egui::pos2(row_rect.left() + 1.5, row_rect.bottom() - 4.0),
                            ],
                            egui::Stroke::new(2.5, col_tint),
                        );
                    }
                    let chev_rect = egui::Rect::from_center_size(
                        egui::pos2(row_rect.left() + 9.0, row_rect.center().y),
                        egui::vec2(11.0, 11.0),
                    );
                    swaggen_ui_kit::paint_icon(
                        ui.painter(),
                        chev_rect,
                        if is_open {
                            IconKind::ChevronDown
                        } else {
                            IconKind::ChevronRight
                        },
                        Tokens::TEXT_SECONDARY,
                    );
                    let folder_rect = egui::Rect::from_center_size(
                        egui::pos2(row_rect.left() + 23.0, row_rect.center().y),
                        egui::vec2(14.0, 14.0),
                    );
                    swaggen_ui_kit::paint_icon(
                        ui.painter(),
                        folder_rect,
                        IconKind::Folder,
                        col_tint,
                    );
                    let max_col_chars = ((row_w - 86.0) / 6.8).max(6.0) as usize;
                    let col_lbl: String = if col.name.chars().count() > max_col_chars {
                        let mut s: String = col
                            .name
                            .chars()
                            .take(max_col_chars.saturating_sub(1))
                            .collect();
                        s.push('…');
                        s
                    } else {
                        col.name.clone()
                    };
                    ui.painter().text(
                        egui::pos2(row_rect.left() + 35.0, row_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        col_lbl,
                        egui::FontId::new(12.0, egui::FontFamily::Name("bold".into())),
                        Tokens::TEXT_PRIMARY,
                    );

                    // Crisp `+` New request icon button
                    let add_rect = egui::Rect::from_center_size(
                        egui::pos2(row_rect.right() - 34.0, row_rect.center().y),
                        egui::vec2(20.0, 20.0),
                    );
                    let add_resp = ui.interact(
                        add_rect,
                        ui.scope_id().with(("col_add_req", &col.id)),
                        egui::Sense::click(),
                    );
                    ui.painter().rect(
                        add_rect,
                        5.0,
                        if add_resp.hovered() {
                            Tokens::BG_SELECTED
                        } else {
                            Tokens::BG_SUBTLE
                        },
                        egui::Stroke::new(
                            1.0,
                            if add_resp.hovered() {
                                Tokens::STATUS_INFO
                            } else {
                                Tokens::BORDER_DEFAULT
                            },
                        ),
                        egui::StrokeKind::Middle,
                    );
                    swaggen_ui_kit::paint_mini_icon(
                        ui.painter(),
                        add_rect,
                        swaggen_ui_kit::MiniIconKind::Plus,
                        if add_resp.hovered() {
                            Tokens::STATUS_INFO
                        } else {
                            Tokens::TEXT_SECONDARY
                        },
                    );
                    if add_resp
                        .on_hover_text("Add REST request (open ⋯ for GraphQL / Folder)")
                        .clicked()
                    {
                        acts.add_req_target = Some((col.id.clone(), None));
                    }

                    // Crisp `⋯` Collection actions icon button
                    let dots_rect = egui::Rect::from_center_size(
                        egui::pos2(row_rect.right() - 11.0, row_rect.center().y),
                        egui::vec2(20.0, 20.0),
                    );
                    let dots_id = ui.scope_id().with(("col_dots", &col.id));
                    let dots_resp = ui.interact(dots_rect, dots_id, egui::Sense::click());
                    ui.painter().rect(
                        dots_rect,
                        5.0,
                        if dots_resp.hovered() {
                            Tokens::BG_SELECTED
                        } else {
                            egui::Color32::TRANSPARENT
                        },
                        if dots_resp.hovered() {
                            egui::Stroke::new(1.0, Tokens::STATUS_INFO)
                        } else {
                            egui::Stroke::NONE
                        },
                        egui::StrokeKind::Middle,
                    );
                    swaggen_ui_kit::paint_mini_icon(
                        ui.painter(),
                        dots_rect,
                        swaggen_ui_kit::MiniIconKind::More,
                        if dots_resp.hovered() {
                            Tokens::STATUS_INFO
                        } else {
                            Tokens::TEXT_SECONDARY
                        },
                    );
                    let dots_resp = dots_resp.on_hover_text("Collection color, rename & actions");
                    let mut col_name_buf = col.name.clone();
                    let col_id_clone = col.id.clone();
                    let cur_color = col.color.clone();
                    let show_col_menu =
                        |ui: &mut egui::Ui,
                         acts: &mut SidebarActions,
                         col_name_buf: &mut String| {
                            ui.set_min_width(196.0);
                            ui.label(
                                egui::RichText::new("Collection name")
                                    .size(10.5)
                                    .color(Tokens::TEXT_SECONDARY),
                            );
                            let name_resp = ui.text_edit_singleline(col_name_buf);
                            if name_resp.changed() && !col_name_buf.trim().is_empty() {
                                acts.rename_col =
                                    Some((col_id_clone.clone(), col_name_buf.trim().to_owned()));
                            }
                            if name_resp.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                ui.close();
                            }
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("Collection color")
                                    .size(10.5)
                                    .color(Tokens::TEXT_SECONDARY),
                            );
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(5.0, 4.0);
                                for (cname, cval, c32) in COLLECTION_COLOR_SWATCHES {
                                    let selected = cur_color.as_deref() == *cval;
                                    let (srect, sresp) = ui.allocate_exact_size(
                                        egui::vec2(17.0, 17.0),
                                        egui::Sense::click(),
                                    );
                                    ui.painter().circle_filled(srect.center(), 6.5, *c32);
                                    if selected || sresp.hovered() {
                                        ui.painter().circle_stroke(
                                            srect.center(),
                                            8.5,
                                            egui::Stroke::new(
                                                if selected { 1.8 } else { 1.0 },
                                                if selected {
                                                    Tokens::TEXT_PRIMARY
                                                } else {
                                                    Tokens::STATUS_INFO
                                                },
                                            ),
                                        );
                                    }
                                    if sresp.on_hover_text(*cname).clicked() {
                                        acts.recolor_col =
                                            Some((col_id_clone.clone(), cval.map(str::to_owned)));
                                    }
                                }
                            });
                            ui.separator();
                            if ui.button("+ New request").clicked() {
                                acts.add_req_target = Some((col_id_clone.clone(), None));
                                ui.close();
                            }
                            if ui.button("+ New GraphQL request").clicked() {
                                acts.add_gql_target = Some((col_id_clone.clone(), None));
                                ui.close();
                            }
                            if ui.button("+ New folder").clicked() {
                                acts.add_folder_target = Some((col_id_clone.clone(), None));
                                ui.close();
                            }
                            if ui.button("Fork collection").clicked() {
                                acts.fork_col_id = Some(col_id_clone.clone());
                                ui.close();
                            }
                            if can_delete_col {
                                ui.separator();
                                if ui
                                    .button(
                                        egui::RichText::new("Move collection to Trash")
                                            .color(Tokens::TEXT_DANGER),
                                    )
                                    .clicked()
                                {
                                    acts.del_col_id = Some(col_id_clone.clone());
                                    ui.close();
                                }
                            }
                        };
                    egui::Popup::menu(&dots_resp)
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                        .show(|ui| show_col_menu(ui, &mut acts, &mut col_name_buf));
                    toggle_resp.context_menu(|ui| show_col_menu(ui, &mut acts, &mut col_name_buf));

                    if is_open {
                        ui.indent(("col_indent", &col.id), |ui| {
                            show_folder_children(
                                ui,
                                &app.state.folders,
                                &app.state.requests,
                                &col.id,
                                None,
                                active_req_id.as_deref(),
                                &all_collections_meta,
                                is_dirty,
                                pointer_pos,
                                pointer_released,
                                &mut app.dragging_request_id,
                                &mut acts,
                            );
                        });
                    }
                }
            }
        });

    // Floating Drag-and-Drop ghost pill & drop cleanup
    if let Some(ref drag_id) = app.dragging_request_id.clone() {
        if let Some(pos) = pointer_pos
            && let Some(r) = app.state.requests.iter().find(|r| &r.id == drag_id)
        {
            egui::Area::new(egui::Id::unique("req_dnd_ghost"))
                .order(egui::Order::Tooltip)
                .fixed_pos(pos + egui::vec2(12.0, 10.0))
                .interactable(false)
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .fill(Tokens::BG_SELECTED)
                        .stroke(egui::Stroke::new(1.5, Tokens::STATUS_INFO))
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let badge = if r.is_graphql() {
                                    "GQL"
                                } else {
                                    r.method.as_str()
                                };
                                ui.label(
                                    egui::RichText::new(badge)
                                        .monospace()
                                        .size(10.5)
                                        .color(swaggen_ui_kit::method_color(badge)),
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}  → Drop onto a collection",
                                        r.name
                                    ))
                                    .size(11.0)
                                    .color(Tokens::TEXT_PRIMARY),
                                );
                            });
                        });
                });
        }
        if pointer_released {
            app.dragging_request_id = None;
        }
    }

    // Apply deferred mutations cleanly
    if acts.save_draft_now {
        app.save_workspace();
    }
    if let Some(rid) = acts.open_request_id {
        app.open_request_in_workbench(&rid);
    }
    if let Some((cid, folder_id)) = acts.add_req_target {
        let mut req = RequestItem::new(cid, "New request", HttpMethod::Get, "{{base_url}}/");
        req.folder_id = folder_id;
        let rid = req.id.clone();
        app.state.requests.push(req);
        app.open_request_in_workbench(&rid);
        app.mark_dirty();
    }
    if let Some((cid, folder_id)) = acts.add_gql_target {
        let mut req = RequestItem::new_graphql(cid, "GraphQL query", "{{base_url}}/graphql");
        req.folder_id = folder_id;
        let rid = req.id.clone();
        app.state.requests.push(req);
        app.open_request_in_workbench(&rid);
        app.mark_dirty();
    }
    if let Some((cid, parent)) = acts.add_folder_target {
        let folder = Folder {
            id: new_uid(),
            collection_id: cid,
            parent_folder_id: parent,
            name: format!("Folder {}", app.state.folders.len() + 1),
            description: String::new(),
            auth: Default::default(),
            pre_request_script: String::new(),
            post_response_script: String::new(),
            order_key: app.state.folders.len() as u64,
        };
        app.state.folders.push(folder);
        app.mark_dirty();
    }
    if let Some(rid) = acts.dup_req_id
        && let Some(new_id) = app.state.duplicate_request(&rid)
    {
        app.open_request_in_workbench(&new_id);
        app.mark_dirty();
    }
    if let Some((rid, move_up)) = acts.reorder_req
        && app.state.reorder_request(&rid, move_up)
    {
        app.mark_dirty();
    }
    if let Some(rid) = acts.del_req_id
        && app.state.delete_request(&rid)
    {
        app.status = "Moved request to Trash (restore anytime from Sidebar Trash)".into();
        app.mark_dirty();
    }
    if let Some((rid, target_cid, target_fid)) = acts.move_req_to
        && app.state.move_request(&rid, &target_cid, target_fid)
    {
        app.mark_dirty();
        app.status = "Moved request to collection".into();
    }
    if let Some((rid, new_name)) = acts.rename_req
        && let Some(r) = app.state.requests.iter_mut().find(|r| r.id == rid)
    {
        r.name = new_name;
        app.mark_dirty();
    }
    if let Some((cid, new_name)) = acts.rename_col
        && let Some(c) = app.state.collections.iter_mut().find(|c| c.id == cid)
    {
        c.name = new_name;
        app.mark_dirty();
    }
    if let Some((cid, new_color)) = acts.recolor_col
        && let Some(c) = app.state.collections.iter_mut().find(|c| c.id == cid)
    {
        c.color = new_color;
        app.mark_dirty();
        app.status = "Updated collection color".into();
    }
    if let Some((fid, new_name)) = acts.rename_folder
        && let Some(f) = app.state.folders.iter_mut().find(|f| f.id == fid)
    {
        f.name = new_name;
        app.mark_dirty();
    }
    if let Some(fid) = acts.del_folder_id
        && app.state.delete_folder(&fid)
    {
        app.mark_dirty();
        app.status = "Moved folder to Trash (restore anytime from Sidebar Trash)".into();
    }
    if let Some(cid) = acts.fork_col_id
        && app.state.fork_collection(&cid).is_some()
    {
        app.mark_dirty();
    }
    if let Some(cid) = acts.del_col_id
        && app.state.delete_collection(&cid)
    {
        app.mark_dirty();
        app.status = "Moved collection to Trash (restore anytime from Sidebar Trash)".into();
    }

    // 5. Pointer-locked Splitter + PERSONAL section anchored cleanly at sidebar bottom
    if !app.state.layout.sidebar_personal_collapsed {
        let (split_resp, _) = swaggen_ui_kit::horizontal_splitter(ui, "sidebar_personal_hsplit");
        if split_resp.dragged()
            && let Some(pos) = ui.ctx().pointer_interact_pos()
        {
            app.state.layout.sidebar_footer_height =
                (sidebar_full_rect.bottom() - pos.y).clamp(126.0, 230.0);
        }
        if split_resp
            .on_hover_text("Drag up/down to resize Collections vs Personal · Double-click to reset")
            .double_clicked()
        {
            app.state.layout.sidebar_footer_height = 138.0;
        }
    } else {
        ui.add_space(4.0);
    }

    let personal_hdr = if app.state.layout.sidebar_personal_collapsed {
        "PERSONAL ▸"
    } else {
        "PERSONAL ▾"
    };
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new(personal_hdr)
                    .size(10.5)
                    .color(Tokens::TEXT_SECONDARY)
                    .strong(),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE)
            .frame(false),
        )
        .clicked()
    {
        app.state.layout.sidebar_personal_collapsed = !app.state.layout.sidebar_personal_collapsed;
    }

    if !app.state.layout.sidebar_personal_collapsed {
        if swaggen_ui_kit::nav_item(
            ui,
            IconKind::History,
            "History",
            Some(app.history.len()),
            app.mode == MainMode::History && !app.show_workspace_modal,
        )
        .clicked()
        {
            app.navigate_to(MainMode::History);
        }
        if swaggen_ui_kit::nav_item(
            ui,
            IconKind::Globe,
            "Environments",
            Some(app.state.environments.len()),
            app.mode == MainMode::Environments && !app.show_workspace_modal,
        )
        .clicked()
        {
            app.navigate_to(MainMode::Environments);
        }
        if swaggen_ui_kit::nav_item(
            ui,
            IconKind::KeyRound,
            "Secrets & Cookies",
            Some(app.secret_names.len() + app.state.cookies.len()),
            matches!(app.mode, MainMode::Secrets | MainMode::Cookies) && !app.show_workspace_modal,
        )
        .clicked()
        {
            app.navigate_to(MainMode::Secrets);
        }
        let total_trashed = app.state.trashed_requests.len()
            + app.state.trashed_collections.len()
            + app.state.trashed_folders.len()
            + app.state.trashed_environments.len();
        if total_trashed > 0 {
            let trash_btn = swaggen_ui_kit::nav_item(
                ui,
                IconKind::Trash,
                "Trash (Restore)",
                Some(total_trashed),
                app.show_sidebar_trash,
            );
            let mut restore_req_id: Option<String> = None;
            let mut restore_col_id: Option<String> = None;
            let mut restore_fld_id: Option<String> = None;
            let mut restore_env_id: Option<String> = None;
            let mut empty_all = false;
            let trashed_cols: Vec<(String, String)> = app
                .state
                .trashed_collections
                .iter()
                .rev()
                .map(|c| (c.id.clone(), c.name.clone()))
                .collect();
            let trashed_flds: Vec<(String, String)> = app
                .state
                .trashed_folders
                .iter()
                .rev()
                .map(|f| (f.id.clone(), f.name.clone()))
                .collect();
            let trashed_envs: Vec<(String, String)> = app
                .state
                .trashed_environments
                .iter()
                .rev()
                .map(|e| (e.id.clone(), e.name.clone()))
                .collect();
            let trashed_snapshot: Vec<(String, String, String)> = app
                .state
                .trashed_requests
                .iter()
                .rev()
                .map(|r| (r.id.clone(), r.method.as_str().to_owned(), r.name.clone()))
                .collect();
            egui::Popup::menu(&trash_btn)
                .id(egui::Id::unique("sidebar_trash_popup"))
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.set_min_width(245.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("SOFT-DELETED ITEMS")
                                .size(10.0)
                                .color(swaggen_ui_kit::text_secondary(ui))
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button(
                                    egui::RichText::new("Empty").color(Tokens::TEXT_DANGER),
                                )
                                .clicked()
                            {
                                empty_all = true;
                                ui.close();
                            }
                        });
                    });
                    ui.separator();
                    for (cid, cname) in &trashed_cols {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("COL")
                                    .size(10.0)
                                    .color(Tokens::METHOD_POST),
                            );
                            ui.label(egui::RichText::new(cname).size(11.5));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("Restore").clicked() {
                                        restore_col_id = Some(cid.clone());
                                        ui.close();
                                    }
                                },
                            );
                        });
                    }
                    for (fid, fname) in &trashed_flds {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("DIR")
                                    .size(10.0)
                                    .color(Tokens::TEXT_SECONDARY),
                            );
                            ui.label(egui::RichText::new(fname).size(11.5));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("Restore").clicked() {
                                        restore_fld_id = Some(fid.clone());
                                        ui.close();
                                    }
                                },
                            );
                        });
                    }
                    for (eid, ename) in &trashed_envs {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("ENV")
                                    .size(10.0)
                                    .color(Tokens::STATUS_SUCCESS),
                            );
                            ui.label(egui::RichText::new(ename).size(11.5));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("Restore").clicked() {
                                        restore_env_id = Some(eid.clone());
                                        ui.close();
                                    }
                                },
                            );
                        });
                    }
                    for (tid, tm, tname) in &trashed_snapshot {
                        ui.horizontal(|ui| {
                            ui.monospace(
                                egui::RichText::new(tm)
                                    .size(10.0)
                                    .color(swaggen_ui_kit::method_color(tm)),
                            );
                            ui.label(egui::RichText::new(tname).size(11.5));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("Restore").clicked() {
                                        restore_req_id = Some(tid.clone());
                                        ui.close();
                                    }
                                },
                            );
                        });
                    }
                });
            if let Some(cid) = restore_col_id
                && app.state.restore_trashed_collection(&cid)
            {
                app.mark_dirty();
                app.status = "Restored collection from Trash".into();
            }
            if let Some(fid) = restore_fld_id
                && app.state.restore_trashed_folder(&fid)
            {
                app.mark_dirty();
                app.status = "Restored folder from Trash".into();
            }
            if let Some(eid) = restore_env_id
                && app.state.restore_trashed_environment(&eid)
            {
                app.mark_dirty();
                app.status = "Restored environment from Trash".into();
            }
            if let Some(rid) = restore_req_id
                && let Some(restored_id) = app.state.restore_trashed_request(&rid)
            {
                app.open_request_in_workbench(&restored_id);
                app.mark_dirty();
                app.status = "Restored request from Trash".into();
            }
            if empty_all {
                app.state.empty_trash();
                app.mark_dirty();
                app.status = "Emptied workspace Trash".into();
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn show_folder_children(
    ui: &mut egui::Ui,
    folders: &[Folder],
    requests: &[RequestItem],
    collection_id: &str,
    parent_folder_id: Option<&str>,
    active_req_id: Option<&str>,
    all_collections: &[(String, String)],
    is_dirty: bool,
    pointer_pos: Option<egui::Pos2>,
    pointer_released: bool,
    dragging_req_id: &mut Option<String>,
    acts: &mut SidebarActions,
) {
    for folder in folders.iter().filter(|f| {
        f.collection_id == collection_id && f.parent_folder_id.as_deref() == parent_folder_id
    }) {
        let hdr = egui::CollapsingHeader::new(
            egui::RichText::new(&folder.name)
                .size(11.5)
                .color(Tokens::TEXT_PRIMARY),
        )
        .id_salt(&folder.id)
        .default_open(true)
        .show(ui, |ui| {
            show_folder_children(
                ui,
                folders,
                requests,
                collection_id,
                Some(&folder.id),
                active_req_id,
                all_collections,
                is_dirty,
                pointer_pos,
                pointer_released,
                dragging_req_id,
                acts,
            );
        });
        if dragging_req_id.is_some()
            && pointer_pos.is_some_and(|p| hdr.header_response.rect.contains(p))
        {
            ui.painter().rect_stroke(
                hdr.header_response.rect,
                4.0,
                egui::Stroke::new(1.5, Tokens::STATUS_INFO),
                egui::StrokeKind::Middle,
            );
            if pointer_released && let Some(drag_id) = dragging_req_id.clone() {
                acts.move_req_to =
                    Some((drag_id, collection_id.to_owned(), Some(folder.id.clone())));
            }
        }
        let mut folder_name_buf = folder.name.clone();
        let folder_id_clone = folder.id.clone();
        let col_id_clone = collection_id.to_owned();
        let render_folder_menu = |ui: &mut egui::Ui,
                                  acts: &mut SidebarActions,
                                  folder_name_buf: &mut String| {
            ui.set_min_width(172.0);
            ui.label(
                egui::RichText::new("Folder name")
                    .size(10.5)
                    .color(Tokens::TEXT_SECONDARY),
            );
            let name_resp = ui.text_edit_singleline(folder_name_buf);
            if name_resp.changed() && !folder_name_buf.trim().is_empty() {
                acts.rename_folder =
                    Some((folder_id_clone.clone(), folder_name_buf.trim().to_owned()));
            }
            if name_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                ui.close();
            }
            ui.separator();
            if ui.button("+ New request").clicked() {
                acts.add_req_target = Some((col_id_clone.clone(), Some(folder_id_clone.clone())));
                ui.close();
            }
            if ui.button("+ New GraphQL request").clicked() {
                acts.add_gql_target = Some((col_id_clone.clone(), Some(folder_id_clone.clone())));
                ui.close();
            }
            if ui.button("+ New subfolder").clicked() {
                acts.add_folder_target =
                    Some((col_id_clone.clone(), Some(folder_id_clone.clone())));
                ui.close();
            }
            ui.separator();
            if ui
                .button(egui::RichText::new("Delete folder").color(Tokens::TEXT_DANGER))
                .clicked()
            {
                acts.del_folder_id = Some(folder_id_clone.clone());
                ui.close();
            }
        };
        hdr.header_response
            .context_menu(|ui| render_folder_menu(ui, acts, &mut folder_name_buf));
    }

    for req in requests
        .iter()
        .filter(|r| r.collection_id == collection_id && r.folder_id.as_deref() == parent_folder_id)
    {
        render_request_row(
            ui,
            req,
            active_req_id == Some(req.id.as_str()),
            all_collections,
            is_dirty,
            dragging_req_id,
            acts,
        );
    }
}

fn render_request_row(
    ui: &mut egui::Ui,
    req: &RequestItem,
    selected: bool,
    all_collections: &[(String, String)],
    is_dirty: bool,
    dragging_req_id: &mut Option<String>,
    acts: &mut SidebarActions,
) {
    let w = ui.available_width().max(90.0);
    let (_, rect) = ui.allocate_space(egui::vec2(w, 25.0));

    let main_rect = egui::Rect::from_min_max(
        rect.min,
        egui::pos2((rect.right() - 22.0).max(rect.left() + 40.0), rect.bottom()),
    );
    let dots_rect = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 10.0, rect.center().y),
        egui::vec2(18.0, 18.0),
    );

    let main_id = egui::Id::unique(("req_row_main", &req.id));
    let dots_id = egui::Id::unique(("req_row_dots", &req.id));
    let main_resp = ui.interact(main_rect, main_id, egui::Sense::click_and_drag());
    let dots_resp = ui.interact(dots_rect, dots_id, egui::Sense::click());

    let row_hovered = main_resp.hovered() || dots_resp.hovered();
    let bg = if selected {
        Tokens::BG_SELECTED
    } else if row_hovered {
        Tokens::BG_SUBTLE
    } else {
        Tokens::BG_SURFACE
    };
    let stroke = if selected {
        egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT)
    } else {
        egui::Stroke::NONE
    };
    ui.painter()
        .rect(rect, 5.0, bg, stroke, egui::StrokeKind::Middle);

    let raw_m = if req.is_graphql() {
        "GQL"
    } else {
        req.method.as_str()
    };
    let short_m = swaggen_ui_kit::compact_method_badge(raw_m);
    ui.painter().text(
        rect.min + egui::vec2(7.0, rect.height() * 0.5),
        egui::Align2::LEFT_CENTER,
        short_m,
        egui::FontId::new(9.5, egui::FontFamily::Monospace),
        swaggen_ui_kit::method_color(raw_m),
    );
    let max_chars = ((w - 64.0) / 6.5).max(6.0) as usize;
    let clipped: String = if req.name.chars().count() > max_chars {
        let mut s: String = req.name.chars().take(max_chars.saturating_sub(1)).collect();
        s.push('…');
        s
    } else {
        req.name.clone()
    };
    ui.painter().text(
        rect.min + egui::vec2(41.0, rect.height() * 0.5),
        egui::Align2::LEFT_CENTER,
        clipped,
        egui::FontId::new(11.5, egui::FontFamily::Proportional),
        Tokens::TEXT_PRIMARY,
    );

    // Paint `⋯` menu button on hover or when selected
    if row_hovered || selected {
        if dots_resp.hovered() {
            ui.painter()
                .rect_filled(dots_rect, 4.0, Tokens::BG_SELECTED);
        }
        ui.painter().text(
            dots_rect.center(),
            egui::Align2::CENTER_CENTER,
            "⋯",
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
            if dots_resp.hovered() {
                Tokens::STATUS_INFO
            } else {
                Tokens::TEXT_SECONDARY
            },
        );
    }

    let main_resp = main_resp.on_hover_text(format!("{raw_m} {}\n{}", req.name, req.url));
    if main_resp.clicked() {
        acts.open_request_id = Some(req.id.clone());
    }
    if main_resp.drag_started() || (main_resp.dragged() && main_resp.drag_delta().length_sq() > 4.0)
    {
        *dragging_req_id = Some(req.id.clone());
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }

    let dots_resp = dots_resp.on_hover_text("Request actions (Save, Duplicate, Move, Delete)");

    let mut name_buf = req.name.clone();
    let req_id_clone = req.id.clone();
    let cur_col_id = req.collection_id.clone();
    let render_menu = |ui: &mut egui::Ui, acts: &mut SidebarActions, name_buf: &mut String| {
        ui.set_min_width(185.0);
        ui.label(
            egui::RichText::new("Request name")
                .size(10.5)
                .color(Tokens::TEXT_SECONDARY),
        );
        let name_resp = ui.text_edit_singleline(name_buf);
        if name_resp.changed() && !name_buf.trim().is_empty() {
            acts.rename_req = Some((req_id_clone.clone(), name_buf.trim().to_owned()));
        }
        if name_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            ui.close();
        }
        ui.separator();
        if ui
            .button(if is_dirty {
                "Save draft* (Ctrl+S)"
            } else {
                "Save draft (Ctrl+S)"
            })
            .clicked()
        {
            acts.save_draft_now = true;
            ui.close();
        }
        if ui.button("Duplicate request").clicked() {
            acts.dup_req_id = Some(req_id_clone.clone());
            ui.close();
        }
        ui.horizontal(|ui| {
            if ui.small_button("↑ Move up").clicked() {
                acts.reorder_req = Some((req_id_clone.clone(), true));
                ui.close();
            }
            if ui.small_button("↓ Move down").clicked() {
                acts.reorder_req = Some((req_id_clone.clone(), false));
                ui.close();
            }
        });
        ui.menu_button("Move to collection ▸", |ui| {
            ui.set_min_width(150.0);
            for (cid, cname) in all_collections {
                let is_cur = *cid == cur_col_id;
                let lbl = if is_cur {
                    format!("✓ {cname}")
                } else {
                    cname.clone()
                };
                if ui.button(lbl).clicked() {
                    if !is_cur {
                        acts.move_req_to = Some((req_id_clone.clone(), cid.clone(), None));
                    }
                    ui.close();
                }
            }
        });
        ui.separator();
        if ui
            .button(egui::RichText::new("Delete request").color(Tokens::TEXT_DANGER))
            .clicked()
        {
            acts.del_req_id = Some(req_id_clone.clone());
            ui.close();
        }
    };

    egui::Popup::menu(&dots_resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| render_menu(ui, acts, &mut name_buf));
    main_resp.context_menu(|ui| render_menu(ui, acts, &mut name_buf));
}
