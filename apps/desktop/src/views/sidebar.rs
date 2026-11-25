//! Figma-aligned Shell/Project sidebar (248px baseline):
//! SWAGGEN header, clickable workspace mode switcher, Search APIs Ctrl+K,
//! Collections / API definitions nav items, collection/endpoint tree with amber folders
//! and direct "+ New request" buttons, and fixed bottom Personal tools footer.
use crate::app::{App, MainMode};
use eframe::egui;
use swaggen_domain::workspace::{Collection, Folder, HttpMethod, RequestItem, new_uid};
use swaggen_ui_kit::{IconKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    // 1. Brand title + workspace mode switcher (Individual / Personal vs Team).
    ui.label(swaggen_ui_kit::bold_text(
        "SWAGGEN",
        18.0,
        Tokens::TEXT_PRIMARY,
    ));
    let kind_label = if app.selected_team_workspace_id.is_some() {
        "Team"
    } else {
        "Personal"
    };
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new(format!("{} / {kind_label} ▾", app.state.meta.name))
                    .size(12.0)
                    .color(Tokens::TEXT_SECONDARY),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE)
            .frame(false),
        )
        .on_hover_text("Switch between Individual & Team workspaces")
        .clicked()
    {
        app.hide_secret();
        app.show_command_palette = false;
        app.show_production_confirm = false;
        app.show_workspace_modal = !app.show_workspace_modal;
    }
    ui.add_space(10.0);

    // 2. Search APIs Ctrl+K + collapsible Filter toggle.
    let search_w = (ui.available_width() - 40.0).max(110.0);
    ui.horizontal(|ui| {
        let search_resp = swaggen_ui_kit::text_input(
            ui,
            &mut app.search_query,
            "Search APIs  Ctrl K",
            search_w,
            false,
        );
        if app.focus_search {
            search_resp.request_focus();
            app.focus_search = false;
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("F")
                        .size(11.5)
                        .color(if app.show_sidebar_filters {
                            Tokens::STATUS_INFO
                        } else {
                            Tokens::TEXT_SECONDARY
                        }),
                )
                .fill(if app.show_sidebar_filters {
                    Tokens::BG_SELECTED
                } else {
                    Tokens::BG_SURFACE
                })
                .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
                .min_size(egui::vec2(34.0, 38.0))
                .corner_radius(6),
            )
            .on_hover_text("Filter by HTTP method or tag")
            .clicked()
        {
            app.show_sidebar_filters = !app.show_sidebar_filters;
        }
    });

    if app.show_sidebar_filters {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let method_label = app.filter_method.map(|m| m.as_str()).unwrap_or("All");
            egui::ComboBox::from_id_salt("filter-method")
                .selected_text(method_label)
                .width(96.0)
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
                .width(96.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.filter_tag, String::new(), "All tags");
                    for t in tags {
                        ui.selectable_value(&mut app.filter_tag, t.clone(), t);
                    }
                });
        });
    }

    ui.add_space(4.0);
    // 3. Top Navigation items matching Figma (Collections, API definitions).
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

    ui.add_space(8.0);
    // 4. COLLECTIONS section header + toggle "+ Collection" inline bar.
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("COLLECTIONS")
                .size(11.0)
                .color(Tokens::TEXT_SECONDARY),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (btn_rect, btn_resp) =
                ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
            if btn_resp.hovered() {
                ui.painter()
                    .rect_filled(btn_rect, 4.0, Tokens::BG_SUBTLE);
            }
            swaggen_ui_kit::paint_icon(
                ui.painter(),
                btn_rect.shrink(4.0),
                IconKind::Plus,
                Tokens::TEXT_SECONDARY,
            );
            if btn_resp.on_hover_text("New collection").clicked() {
                app.show_new_collection_input = !app.show_new_collection_input;
            }
        });
    });

    if app.show_new_collection_input {
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 62.0).max(90.0);
            ui.add(
                egui::TextEdit::singleline(&mut app.new_collection_name)
                    .hint_text("Collection name")
                    .desired_width(w),
            );
            if swaggen_ui_kit::secondary_button(
                ui,
                !app.new_collection_name.trim().is_empty(),
                "Add",
            )
            .clicked()
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

    let mut open_request_id: Option<String> = None;
    let mut add_req_target: Option<(String, Option<String>)> = None;
    let mut add_folder_target: Option<(String, Option<String>)> = None;
    let mut dup_req_id: Option<String> = None;
    let mut fork_col_id: Option<String> = None;

    // Reserve exact height for PERSONAL nav section + footer so ScrollArea NEVER overlaps clicks.
    let footer_reserved = 166.0;
    let tree_height = (ui.available_height() - footer_reserved).max(90.0);
    let show_endpoint_selection = app.mode == MainMode::Workbench && !app.show_workspace_modal;

    egui::ScrollArea::vertical()
        .id_salt("sidebar-tree-scroll")
        .min_scrolled_height(tree_height)
        .max_height(tree_height)
        .show(ui, |ui| {
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
                            .size(12.0)
                            .color(Tokens::TEXT_SECONDARY),
                    );
                } else {
                    for req in &matches {
                        let sel = show_endpoint_selection
                            && app.active_request_id().as_deref() == Some(req.id.as_str());
                        if swaggen_ui_kit::endpoint_row(ui, req.method.as_str(), &req.name, sel)
                            .clicked()
                        {
                            open_request_id = Some(req.id.clone());
                        }
                    }
                }
            } else {
                let active_req_id = if show_endpoint_selection {
                    app.active_request_id()
                } else {
                    None
                };
                for col in &app.state.collections {
                    let col_state_id = ui.make_persistent_id(("col_open", &col.id));
                    let mut is_open = ui
                        .ctx()
                        .data_mut(|d| *d.get_persisted_mut_or(col_state_id, true));

                    let row_w = ui.available_width().max(120.0);
                    let (row_rect, row_resp) =
                        ui.allocate_exact_size(egui::vec2(row_w, 30.0), egui::Sense::hover());
                    if row_resp.hovered() {
                        ui.painter().rect_filled(row_rect, 6.0, Tokens::BG_SUBTLE);
                    }

                    // Expand/collapse toggle area
                    let toggle_rect = egui::Rect::from_min_max(
                        row_rect.min,
                        egui::pos2(row_rect.right() - 26.0, row_rect.bottom()),
                    );
                    let toggle_resp = ui.interact(
                        toggle_rect,
                        ui.scope_id().with(("col_toggle", &col.id)),
                        egui::Sense::click(),
                    );
                    if toggle_resp.clicked() {
                        is_open = !is_open;
                        ui.ctx().data_mut(|d| d.insert_persisted(col_state_id, is_open));
                    }

                    // + New request button inside collection row
                    let add_rect = egui::Rect::from_center_size(
                        egui::pos2(row_rect.right() - 13.0, row_rect.center().y),
                        egui::vec2(22.0, 22.0),
                    );
                    let add_resp = ui.interact(
                        add_rect,
                        ui.scope_id().with(("col_add_req", &col.id)),
                        egui::Sense::click(),
                    );
                    if add_resp.hovered() {
                        ui.painter().rect_filled(add_rect, 4.0, Tokens::BG_SELECTED);
                    }

                    let chev_rect = egui::Rect::from_center_size(
                        egui::pos2(row_rect.left() + 8.0, row_rect.center().y),
                        egui::vec2(12.0, 12.0),
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
                        egui::pos2(row_rect.left() + 24.0, row_rect.center().y),
                        egui::vec2(16.0, 16.0),
                    );
                    swaggen_ui_kit::paint_icon(
                        ui.painter(),
                        folder_rect,
                        IconKind::Folder,
                        Tokens::METHOD_POST,
                    );
                    ui.painter().text(
                        egui::pos2(row_rect.left() + 38.0, row_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &col.name,
                        egui::FontId::new(13.0, egui::FontFamily::Name("bold".into())),
                        Tokens::TEXT_PRIMARY,
                    );
                    swaggen_ui_kit::paint_icon(
                        ui.painter(),
                        add_rect.shrink(4.0),
                        IconKind::Plus,
                        if add_resp.hovered() {
                            Tokens::STATUS_INFO
                        } else {
                            Tokens::TEXT_SECONDARY
                        },
                    );
                    if add_resp.on_hover_text("Add request to collection").clicked() {
                        add_req_target = Some((col.id.clone(), None));
                    }

                    toggle_resp.context_menu(|ui| {
                        if ui.button("New request").clicked() {
                            add_req_target = Some((col.id.clone(), None));
                            ui.close();
                        }
                        if ui.button("New folder").clicked() {
                            add_folder_target = Some((col.id.clone(), None));
                            ui.close();
                        }
                        if ui.button("Fork collection").clicked() {
                            fork_col_id = Some(col.id.clone());
                            ui.close();
                        }
                    });

                    if is_open {
                        ui.indent(("col_body", &col.id), |ui| {
                            show_folder_children(
                                ui,
                                &app.state.folders,
                                &app.state.requests,
                                &col.id,
                                None,
                                active_req_id.as_deref(),
                                &mut open_request_id,
                                &mut add_req_target,
                                &mut add_folder_target,
                                &mut dup_req_id,
                            );
                        });
                    }
                }
            }
        });

    if let Some(rid) = open_request_id {
        app.open_request_in_workbench(&rid);
    }
    if let Some((cid, fid)) = add_req_target {
        let mut req = RequestItem::new(cid, "New request", HttpMethod::Get, "{{base_url}}/");
        req.folder_id = fid;
        let rid = req.id.clone();
        app.state.requests.push(req);
        app.open_request_in_workbench(&rid);
        app.mark_dirty();
    }
    if let Some((cid, parent)) = add_folder_target {
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
    if let Some(rid) = dup_req_id
        && let Some(new_id) = app.state.duplicate_request(&rid)
    {
        app.open_request_in_workbench(&new_id);
        app.mark_dirty();
    }
    if let Some(cid) = fork_col_id
        && app.state.fork_collection(&cid).is_some()
    {
        app.mark_dirty();
    }

    // 5. PERSONAL & TOOLS navigation footer (sequential layout — no overlap!).
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("PERSONAL")
            .size(11.0)
            .color(Tokens::TEXT_SECONDARY),
    );
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
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(format!(
            "{} · {} collections",
            if app.selected_team_workspace_id.is_some() {
                "Team workspace"
            } else {
                "Individual workspace"
            },
            app.state.collections.len()
        ))
        .size(11.0)
        .color(Tokens::TEXT_SECONDARY),
    );
}

#[allow(clippy::too_many_arguments)]
fn show_folder_children(
    ui: &mut egui::Ui,
    folders: &[Folder],
    requests: &[RequestItem],
    collection_id: &str,
    parent_folder_id: Option<&str>,
    active_req_id: Option<&str>,
    open_request_id: &mut Option<String>,
    add_req_target: &mut Option<(String, Option<String>)>,
    add_folder_target: &mut Option<(String, Option<String>)>,
    dup_req_id: &mut Option<String>,
) {
    for folder in folders.iter().filter(|f| {
        f.collection_id == collection_id && f.parent_folder_id.as_deref() == parent_folder_id
    }) {
        let hdr = egui::CollapsingHeader::new(
            egui::RichText::new(&folder.name)
                .size(12.5)
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
                open_request_id,
                add_req_target,
                add_folder_target,
                dup_req_id,
            );
        });
        hdr.header_response.context_menu(|ui| {
            if ui.button("New request").clicked() {
                *add_req_target = Some((collection_id.to_owned(), Some(folder.id.clone())));
                ui.close();
            }
            if ui.button("New subfolder").clicked() {
                *add_folder_target = Some((collection_id.to_owned(), Some(folder.id.clone())));
                ui.close();
            }
        });
    }

    for req in requests
        .iter()
        .filter(|r| r.collection_id == collection_id && r.folder_id.as_deref() == parent_folder_id)
    {
        let selected = active_req_id == Some(req.id.as_str());
        let row = swaggen_ui_kit::endpoint_row(ui, req.method.as_str(), &req.name, selected);
        if row.clicked() {
            *open_request_id = Some(req.id.clone());
        }
        row.context_menu(|ui| {
            if ui.button("Duplicate request").clicked() {
                *dup_req_id = Some(req.id.clone());
                ui.close();
            }
            if ui.button("New request in folder").clicked() {
                *add_req_target =
                    Some((collection_id.to_owned(), parent_folder_id.map(str::to_owned)));
                ui.close();
            }
        });
    }
}
