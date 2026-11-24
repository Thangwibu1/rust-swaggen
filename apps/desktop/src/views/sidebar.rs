//! Resizable sidebar with instant search, method/tag filters, collection/folder tree,
//! and quick links to Environments, History, Cookies, Secrets, Sanitizer, Storage & Privacy.
use crate::app::{App, MainMode};
use eframe::egui;
use swaggen_domain::workspace::{Collection, Folder, HttpMethod, RequestItem, new_uid};
use swaggen_ui_kit::Tokens;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        if ui
            .selectable_label(app.mode == MainMode::Workbench, "API Workbench")
            .clicked()
        {
            app.hide_secret();
            app.mode = MainMode::Workbench;
        }
    });
    ui.separator();

    // Instant search & filters (WS07 / Ctrl+P).
    ui.label(egui::RichText::new("Search requests (Ctrl+P)").color(Tokens::TEXT_SECONDARY));
    let search_resp = ui.add(
        egui::TextEdit::singleline(&mut app.search_query)
            .id(egui::Id::unique("sidebar-search"))
            .hint_text("Name, method, URL, tag…")
            .desired_width(ui.available_width()),
    );
    if app.focus_search {
        search_resp.request_focus();
        app.focus_search = false;
    }

    ui.horizontal(|ui| {
        let method_label = app
            .filter_method
            .map(|m| m.as_str())
            .unwrap_or("All methods");
        egui::ComboBox::from_id_salt("filter-method")
            .selected_text(method_label)
            .width(105.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.filter_method, None, "All methods");
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
            .width(95.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.filter_tag, String::new(), "All tags");
                for t in tags {
                    ui.selectable_value(&mut app.filter_tag, t.clone(), t);
                }
            });
    });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.new_collection_name)
                .hint_text("New collection")
                .desired_width(130.0),
        );
        if swaggen_ui_kit::secondary_button(ui, !app.new_collection_name.trim().is_empty(), "+ Col")
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
            app.mark_dirty();
        }
    });
    ui.separator();

    let filtering = !app.search_query.trim().is_empty()
        || app.filter_method.is_some()
        || !app.filter_tag.is_empty();

    let mut open_request_id: Option<String> = None;
    let mut add_req_target: Option<(String, Option<String>)> = None;
    let mut add_folder_target: Option<(String, Option<String>)> = None;
    let mut dup_req_id: Option<String> = None;
    let mut fork_col_id: Option<String> = None;

    egui::ScrollArea::vertical()
        .max_height((ui.available_height() - 220.0).max(120.0))
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
                        egui::RichText::new("No matching requests (X02)")
                            .color(Tokens::TEXT_SECONDARY),
                    );
                } else {
                    for req in &matches {
                        ui.horizontal(|ui| {
                            swaggen_ui_kit::method_badge(ui, req.method.as_str());
                            if ui.selectable_label(false, &req.name).clicked() {
                                open_request_id = Some(req.id.clone());
                            }
                        });
                    }
                }
            } else {
                let active_req_id = app.active_request_id();
                for col in &app.state.collections {
                    egui::CollapsingHeader::new(egui::RichText::new(&col.name).strong())
                        .id_salt(&col.id)
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui.small_button("+ Req").clicked() {
                                    add_req_target = Some((col.id.clone(), None));
                                }
                                if ui.small_button("+ Dir").clicked() {
                                    add_folder_target = Some((col.id.clone(), None));
                                }
                                if ui.small_button("Fork").clicked() {
                                    fork_col_id = Some(col.id.clone());
                                }
                            });
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

    ui.separator();
    ui.label(egui::RichText::new("Workspace tools").color(Tokens::TEXT_SECONDARY));
    for (mode, label) in [
        (MainMode::Runner, "Runner, Mock & Tools (TS02/MK01/PR01)"),
        (MainMode::Server, "Team Server & Auth (WS02/WS05)"),
        (MainMode::TeamSync, "Team Sync & Collab (CO02/SY02)"),
        (MainMode::Specs, "OpenAPI & Docs (D01/D06)"),
        (MainMode::ImportExport, "Import / Export (I01)"),
        (MainMode::Environments, "Environments (E01)"),
        (MainMode::History, "History (R14)"),
        (MainMode::Cookies, "Cookies (R12)"),
        (MainMode::Secrets, "Personal secrets (E03)"),
        (MainMode::Sanitizer, "Sanitizer & preview (Z15)"),
        (MainMode::Storage, "Storage & retention (S05)"),
        (MainMode::Privacy, "Privacy & diagnostics (S06)"),
        (MainMode::Shortcuts, "Keyboard shortcuts (S04)"),
    ] {
        if ui.selectable_label(app.mode == mode, label).clicked() {
            app.hide_secret();
            app.mode = mode;
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
    open_request_id: &mut Option<String>,
    add_req_target: &mut Option<(String, Option<String>)>,
    add_folder_target: &mut Option<(String, Option<String>)>,
    dup_req_id: &mut Option<String>,
) {
    for folder in folders.iter().filter(|f| {
        f.collection_id == collection_id && f.parent_folder_id.as_deref() == parent_folder_id
    }) {
        egui::CollapsingHeader::new(&folder.name)
            .id_salt(&folder.id)
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.small_button("+ Req").clicked() {
                        *add_req_target = Some((collection_id.to_owned(), Some(folder.id.clone())));
                    }
                    if ui.small_button("+ Sub").clicked() {
                        *add_folder_target =
                            Some((collection_id.to_owned(), Some(folder.id.clone())));
                    }
                });
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
    }

    for req in requests
        .iter()
        .filter(|r| r.collection_id == collection_id && r.folder_id.as_deref() == parent_folder_id)
    {
        let selected = active_req_id == Some(req.id.as_str());
        ui.horizontal(|ui| {
            swaggen_ui_kit::method_badge(ui, req.method.as_str());
            if ui.selectable_label(selected, &req.name).clicked() {
                *open_request_id = Some(req.id.clone());
            }
            if ui.small_button("Dup").clicked() {
                *dup_req_id = Some(req.id.clone());
            }
        });
    }
}
