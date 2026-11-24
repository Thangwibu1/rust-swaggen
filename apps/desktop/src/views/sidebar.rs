//! Figma-aligned Shell/Project sidebar (248px baseline):
//! SWAGGEN header, Search APIs Ctrl+K, Collections / API definitions nav items,
//! clean collection/endpoint tree, and Personal tool navigation.
use crate::app::{App, MainMode};
use eframe::egui;
use swaggen_domain::workspace::{Collection, Folder, HttpMethod, RequestItem, new_uid};
use swaggen_ui_kit::IconKind;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    // 1. Brand title + workspace subtitle (matching Figma I6:1948).
    ui.label(swaggen_ui_kit::bold_text(
        "SWAGGEN",
        18.0,
        swaggen_ui_kit::text_primary(ui),
    ));
    let kind_label = if app.server_profiles.active().is_some_and(|p| p.mongo_ready) {
        "Team"
    } else {
        "Local"
    };
    ui.label(
        egui::RichText::new(format!("{} / {kind_label}", app.state.meta.name))
            .size(12.0)
            .color(swaggen_ui_kit::text_secondary(ui)),
    );
    ui.add_space(10.0);

    // 2. Search APIs Ctrl+K (40px Figma Input) + collapsible Filter toggle.
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
                        .color(swaggen_ui_kit::text_secondary(ui)),
                )
                .fill(swaggen_ui_kit::surface_bg(ui))
                .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                .min_size(egui::vec2(34.0, 38.0))
                .corner_radius(6),
            )
            .on_hover_text("Toggle method/tag filters")
            .clicked()
        {
            app.show_sidebar_filters = !app.show_sidebar_filters;
        }
    });

    if app.show_sidebar_filters {
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
        app.mode == MainMode::Workbench,
    )
    .clicked()
    {
        app.hide_secret();
        app.mode = MainMode::Workbench;
    }
    if swaggen_ui_kit::nav_item(
        ui,
        IconKind::FileCode,
        "API definitions",
        Some(app.specs.len()),
        app.mode == MainMode::Specs,
    )
    .clicked()
    {
        app.hide_secret();
        app.mode = MainMode::Specs;
    }

    ui.add_space(6.0);
    // 4. COLLECTIONS section header + toggle "+ Collection" inline bar.
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("COLLECTIONS")
                .size(11.0)
                .color(swaggen_ui_kit::text_secondary(ui)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("+")
                .on_hover_text("Create new collection")
                .clicked()
            {
                app.show_new_collection_input = !app.show_new_collection_input;
            }
        });
    });

    if app.show_new_collection_input {
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 56.0).max(90.0);
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

    egui::ScrollArea::vertical()
        .id_salt("sidebar-tree-scroll")
        .max_height((ui.available_height() - 190.0).max(130.0))
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
                            .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                } else {
                    for req in &matches {
                        let sel = app.active_request_id().as_deref() == Some(req.id.as_str());
                        if swaggen_ui_kit::endpoint_row(ui, req.method.as_str(), &req.name, sel)
                            .clicked()
                        {
                            open_request_id = Some(req.id.clone());
                        }
                    }
                }
            } else {
                let active_req_id = app.active_request_id();
                for col in &app.state.collections {
                    let header = egui::CollapsingHeader::new(swaggen_ui_kit::bold_text(
                        &col.name,
                        13.0,
                        swaggen_ui_kit::text_primary(ui),
                    ))
                    .id_salt(&col.id)
                    .default_open(true)
                    .show(ui, |ui| {
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
                    header.header_response.context_menu(|ui| {
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
                }
            }
        });

    if let Some(rid) = open_request_id {
        app.state.open_or_focus_tab(&rid);
        app.mode = MainMode::Workbench;
        app.mark_dirty();
    }
    if let Some((cid, fid)) = add_req_target {
        let mut req = RequestItem::new(cid, "New request", HttpMethod::Get, "{{base_url}}/");
        req.folder_id = fid;
        let rid = req.id.clone();
        app.state.requests.push(req);
        app.state.open_or_focus_tab(&rid);
        app.mode = MainMode::Workbench;
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
        app.state.open_or_focus_tab(&new_id);
        app.mark_dirty();
    }
    if let Some(cid) = fork_col_id
        && app.state.fork_collection(&cid).is_some()
    {
        app.mark_dirty();
    }

    // 5. PERSONAL & TOOLS navigation footer (matching Figma Shell/Project sidebar).
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("PERSONAL")
            .size(11.0)
            .color(swaggen_ui_kit::text_secondary(ui)),
    );
    if swaggen_ui_kit::nav_item(
        ui,
        IconKind::History,
        "History",
        Some(app.history.len()),
        app.mode == MainMode::History,
    )
    .clicked()
    {
        app.hide_secret();
        app.mode = MainMode::History;
    }
    if swaggen_ui_kit::nav_item(
        ui,
        IconKind::Globe,
        "Environments",
        Some(app.state.environments.len()),
        app.mode == MainMode::Environments,
    )
    .clicked()
    {
        app.hide_secret();
        app.mode = MainMode::Environments;
    }
    if swaggen_ui_kit::nav_item(
        ui,
        IconKind::KeyRound,
        "Secrets & Cookies",
        Some(app.secret_names.len() + app.state.cookies.len()),
        matches!(app.mode, MainMode::Secrets | MainMode::Cookies),
    )
    .clicked()
    {
        app.hide_secret();
        app.mode = MainMode::Secrets;
    }

    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        ui.label(
            egui::RichText::new(format!(
                "{} · {} collections",
                app.state.meta.name,
                app.state.collections.len()
            ))
            .size(11.0)
            .color(swaggen_ui_kit::text_secondary(ui)),
        );
    });
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
                .color(swaggen_ui_kit::text_primary(ui)),
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
                *add_req_target = Some((collection_id.to_owned(), parent_folder_id.map(str::to_owned)));
                ui.close();
            }
        });
    }
}
