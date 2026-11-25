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
    dup_req_id: Option<String>,
    del_req_id: Option<String>,
    move_req_to: Option<(String, String, Option<String>)>,
    rename_req: Option<(String, String)>,
    save_draft_now: bool,
    fork_col_id: Option<String>,
    del_col_id: Option<String>,
    rename_col: Option<(String, String)>,
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let sidebar_full_rect = ui.max_rect();

    // 1. Compact Brand title + workspace switcher
    ui.horizontal(|ui| {
        ui.label(swaggen_ui_kit::bold_text(
            "SWAGGEN",
            14.5,
            Tokens::TEXT_PRIMARY,
        ));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let kind_label = if app.selected_team_workspace_id.is_some() {
                "Team"
            } else {
                "Personal"
            };
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{} · {kind_label} ▾", app.state.meta.name))
                            .size(11.0)
                            .color(Tokens::TEXT_SECONDARY),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE)
                    .frame(false),
                )
                .on_hover_text("Switch Individual / Team workspace")
                .clicked()
            {
                app.hide_secret();
                app.show_command_palette = false;
                app.show_production_confirm = false;
                app.show_workspace_modal = !app.show_workspace_modal;
            }
        });
    });
    ui.add_space(4.0);

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
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(rect, 4.0, Tokens::BG_SUBTLE);
            }
            swaggen_ui_kit::paint_icon(
                ui.painter(),
                rect.shrink(3.5),
                IconKind::Plus,
                if resp.hovered() {
                    Tokens::STATUS_INFO
                } else {
                    Tokens::TEXT_SECONDARY
                },
            );
            if resp.on_hover_text("Create new collection").clicked() {
                app.show_new_collection_input = !app.show_new_collection_input;
            }
        });
    });

    if app.show_new_collection_input {
        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut app.new_collection_name)
                    .hint_text("Collection name…")
                    .desired_width((ui.available_width() - 42.0).max(70.0)),
            );
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.small_button("Add").clicked() || enter)
                && !app.new_collection_name.trim().is_empty()
            {
                let col = Collection {
                    id: new_uid(),
                    name: app.new_collection_name.trim().to_owned(),
                    description: String::new(),
                    auth: Default::default(),
                    variables: Vec::new(),
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

    // Exact height calculation with `.auto_shrink([false, false])` so `PERSONAL` stays anchored at bottom!
    let footer_reserved = if app.state.layout.sidebar_personal_collapsed {
        44.0
    } else {
        app.state.layout.sidebar_footer_height.clamp(112.0, 220.0)
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

                    let chev_rect = egui::Rect::from_center_size(
                        egui::pos2(row_rect.left() + 7.0, row_rect.center().y),
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
                        egui::pos2(row_rect.left() + 21.0, row_rect.center().y),
                        egui::vec2(14.0, 14.0),
                    );
                    swaggen_ui_kit::paint_icon(
                        ui.painter(),
                        folder_rect,
                        IconKind::Folder,
                        Tokens::METHOD_POST,
                    );
                    let max_col_chars = ((row_w - 80.0) / 6.8).max(6.0) as usize;
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
                        egui::pos2(row_rect.left() + 33.0, row_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        col_lbl,
                        egui::FontId::new(12.0, egui::FontFamily::Name("bold".into())),
                        Tokens::TEXT_PRIMARY,
                    );

                    // `+` New request button
                    let add_rect = egui::Rect::from_center_size(
                        egui::pos2(row_rect.right() - 31.0, row_rect.center().y),
                        egui::vec2(19.0, 19.0),
                    );
                    let add_resp = ui.interact(
                        add_rect,
                        ui.scope_id().with(("col_add_req", &col.id)),
                        egui::Sense::click(),
                    );
                    if add_resp.hovered() {
                        ui.painter().rect_filled(add_rect, 4.0, Tokens::BG_SELECTED);
                    }
                    swaggen_ui_kit::paint_icon(
                        ui.painter(),
                        add_rect.shrink(3.5),
                        IconKind::Plus,
                        if add_resp.hovered() {
                            Tokens::STATUS_INFO
                        } else {
                            Tokens::TEXT_SECONDARY
                        },
                    );
                    if add_resp.on_hover_text("Add request").clicked() {
                        acts.add_req_target = Some((col.id.clone(), None));
                    }

                    // `⋯` Collection actions button
                    let dots_rect = egui::Rect::from_center_size(
                        egui::pos2(row_rect.right() - 11.0, row_rect.center().y),
                        egui::vec2(19.0, 19.0),
                    );
                    let dots_id = ui.scope_id().with(("col_dots", &col.id));
                    let dots_resp = ui.interact(dots_rect, dots_id, egui::Sense::click());
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
                    let dots_resp = dots_resp.on_hover_text("Collection actions");
                    let mut col_name_buf = col.name.clone();
                    let col_id_clone = col.id.clone();
                    let show_col_menu =
                        |ui: &mut egui::Ui,
                         acts: &mut SidebarActions,
                         col_name_buf: &mut String| {
                            ui.set_min_width(172.0);
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
                                        egui::RichText::new("Delete collection")
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
    if let Some(rid) = acts.del_req_id
        && app.state.delete_request(&rid)
    {
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
    if let Some(cid) = acts.fork_col_id
        && app.state.fork_collection(&cid).is_some()
    {
        app.mark_dirty();
    }
    if let Some(cid) = acts.del_col_id
        && app.state.delete_collection(&cid)
    {
        app.mark_dirty();
    }

    // 5. Pointer-locked Splitter + PERSONAL section anchored cleanly at sidebar bottom
    if !app.state.layout.sidebar_personal_collapsed {
        let (split_resp, _) = swaggen_ui_kit::horizontal_splitter(ui, "sidebar_personal_hsplit");
        if split_resp.dragged()
            && let Some(pos) = ui.ctx().pointer_interact_pos()
        {
            app.state.layout.sidebar_footer_height =
                (sidebar_full_rect.bottom() - pos.y).clamp(112.0, 220.0);
        }
        if split_resp
            .on_hover_text("Drag up/down to resize Collections vs Personal · Double-click to reset")
            .double_clicked()
        {
            app.state.layout.sidebar_footer_height = 136.0;
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
        hdr.header_response.context_menu(|ui| {
            if ui.button("+ New request").clicked() {
                acts.add_req_target = Some((collection_id.to_owned(), Some(folder.id.clone())));
                ui.close();
            }
            if ui.button("+ New GraphQL request").clicked() {
                acts.add_gql_target = Some((collection_id.to_owned(), Some(folder.id.clone())));
                ui.close();
            }
            if ui.button("+ New subfolder").clicked() {
                acts.add_folder_target = Some((collection_id.to_owned(), Some(folder.id.clone())));
                ui.close();
            }
        });
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

    let m = if req.is_graphql() {
        "GQL"
    } else {
        req.method.as_str()
    };
    ui.painter().text(
        rect.min + egui::vec2(7.0, rect.height() * 0.5),
        egui::Align2::LEFT_CENTER,
        m,
        egui::FontId::new(9.5, egui::FontFamily::Monospace),
        swaggen_ui_kit::method_color(m),
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
        rect.min + egui::vec2(42.0, rect.height() * 0.5),
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
