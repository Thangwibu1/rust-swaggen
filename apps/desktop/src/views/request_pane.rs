//! High-density Request Composer:
//! - Compact open tabs + `Split: ↕ / ↔` layout switcher
//! - Streamlined Method + URL + Env + Send + `⋯` actions menu (no bulky Duplicate/Save buttons)
//! - Dedicated `Query String` & `Path Params` tabs; hides `Body` tab automatically for `GET` / `HEAD`
//! - Auto-detects query parameters both from `?key=val` in URL AND from JSON response fields
use crate::app::{App, RequestSubTab};
use eframe::egui;
use swaggen_domain::workspace::{
    ApiKeyPlacement, AssertionRule, AssertionTarget, AuthConfig, HttpMethod, KeyValueRow,
    MultipartField, MultipartValueKind, RequestBody, RequestItem, VariableContext, folder_chain,
    infer_query_param_suggestions, new_uid, remove_url_path_param_key, rename_url_path_param_key,
    resolve_effective_auth, resolve_request_preview, split_url_parts,
};
use swaggen_ui_kit::{IconKind, NoticeKind, Tokens};

pub fn show_top_bar(app: &mut App, ui: &mut egui::Ui) -> Option<usize> {
    // 1. Compact open request tab strip + right-aligned Split layout toggle
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let mut switch_tab: Option<String> = None;
        let mut close_tab: Option<String> = None;
        let mut toggle_pin: Option<String> = None;
        for tab in &app.state.open_tabs {
            let (method, name) = app
                .state
                .requests
                .iter()
                .find(|r| r.id == tab.request_id)
                .map(|r| (r.method.as_str(), r.name.clone()))
                .unwrap_or(("GET", "Missing".into()));
            let active = app.state.active_tab_id.as_deref() == Some(tab.id.as_str());
            let fill = if active {
                Tokens::BG_SELECTED
            } else {
                Tokens::BG_SURFACE
            };
            egui::Frame::new()
                .fill(fill)
                .stroke(egui::Stroke::new(
                    1.0,
                    if active {
                        Tokens::STATUS_INFO
                    } else {
                        Tokens::BORDER_DEFAULT
                    },
                ))
                .corner_radius(5)
                .inner_margin(egui::Margin::symmetric(7, 3))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(method)
                                .monospace()
                                .size(10.0)
                                .color(swaggen_ui_kit::method_color(method)),
                        );
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(&name)
                                        .size(11.5)
                                        .color(Tokens::TEXT_PRIMARY),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE)
                                .frame(false),
                            )
                            .clicked()
                        {
                            switch_tab = Some(tab.id.clone());
                        }
                        if ui
                            .small_button(if tab.pinned { "*" } else { "·" })
                            .on_hover_text(if tab.pinned { "Unpin tab" } else { "Pin tab" })
                            .clicked()
                        {
                            toggle_pin = Some(tab.id.clone());
                        }
                        if !tab.pinned && ui.small_button("×").clicked() {
                            close_tab = Some(tab.id.clone());
                        }
                    });
                });
        }
        let (add_rect, add_resp) =
            ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
        if add_resp.hovered() {
            ui.painter().rect_filled(add_rect, 4.0, Tokens::BG_SUBTLE);
        }
        swaggen_ui_kit::paint_icon(
            ui.painter(),
            add_rect.shrink(4.5),
            IconKind::Plus,
            Tokens::TEXT_PRIMARY,
        );
        if add_resp.on_hover_text("Create new request").clicked()
            && let Some(cid) = app.state.collections.first().map(|c| c.id.clone())
        {
            let req = RequestItem::new(cid, "New request", HttpMethod::Get, "{{base_url}}/");
            let rid = req.id.clone();
            app.state.requests.push(req);
            app.state.open_or_focus_tab(&rid);
            app.mark_dirty();
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let split_lbl = if app.state.layout.workbench_side_by_side {
                "Layout: Left / Right ↔"
            } else {
                "Layout: Top / Bottom ↕"
            };
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(split_lbl)
                            .size(11.0)
                            .color(Tokens::TEXT_SECONDARY),
                    )
                    .fill(Tokens::BG_SURFACE)
                    .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
                    .min_size(egui::vec2(120.0, 23.0))
                    .corner_radius(5),
                )
                .on_hover_text("Switch Request/Response split between Top-Bottom and Side-by-Side")
                .clicked()
            {
                app.state.layout.workbench_side_by_side = !app.state.layout.workbench_side_by_side;
                app.response_maximized = false;
                app.request_config_collapsed = false;
            }
            if app.state.open_tabs.len() > 1 && ui.small_button("Close unpinned").clicked() {
                app.state.close_unpinned_tabs();
                app.mark_dirty();
            }
        });

        if let Some(tid) = switch_tab {
            app.state.active_tab_id = Some(tid);
        }
        if let Some(tid) = toggle_pin
            && let Some(t) = app.state.open_tabs.iter_mut().find(|t| t.id == tid)
        {
            t.pinned = !t.pinned;
            app.mark_dirty();
        }
        if let Some(tid) = close_tab {
            app.state.open_tabs.retain(|t| t.id != tid);
            if app.state.active_tab_id.as_deref() == Some(tid.as_str()) {
                app.state.active_tab_id = app.state.open_tabs.first().map(|t| t.id.clone());
            }
            app.mark_dirty();
        }
    });
    ui.add_space(4.0);

    let Some(req_id) = app.active_request_id() else {
        swaggen_ui_kit::page_header_secondary(
            ui,
            "API Workbench",
            "Select a request from the sidebar or click + above to start.",
            None,
            false,
        );
        return None;
    };

    let Some(req_idx) = app.state.requests.iter().position(|r| r.id == req_id) else {
        ui.label("Selected request not found.");
        return None;
    };

    let mut changed = false;
    let mut do_duplicate = false;
    let mut do_delete = false;
    let mut do_save = false;
    let mut do_copy_curl = false;
    let mut move_to_col: Option<String> = None;

    let all_cols: Vec<(String, String)> = app
        .state
        .collections
        .iter()
        .map(|c| (c.id.clone(), c.name.clone()))
        .collect();
    let cur_col_id = app.state.requests[req_idx].collection_id.clone();

    // 2. Streamlined Method + URL + Environment + Send + `⋯` Request Actions Menu
    let is_inflight = app.inflight_request_id.as_deref() == Some(req_id.as_str());
    let active_env_is_prod = app
        .state
        .active_environment()
        .is_some_and(|e| e.is_production);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let mut method = app.state.requests[req_idx].method;
        egui::ComboBox::from_id_salt("req-method")
            .selected_text(
                egui::RichText::new(method.as_str())
                    .color(swaggen_ui_kit::method_color(method.as_str()))
                    .monospace()
                    .size(11.5)
                    .strong(),
            )
            .width(76.0)
            .show_ui(ui, |ui| {
                for m in HttpMethod::ALL {
                    if ui
                        .selectable_value(
                            &mut method,
                            m,
                            egui::RichText::new(m.as_str())
                                .monospace()
                                .color(swaggen_ui_kit::method_color(m.as_str())),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                }
            });
        app.state.requests[req_idx].method = method;
        if matches!(method, HttpMethod::Get | HttpMethod::Head)
            && app.request_subtab == RequestSubTab::Body
        {
            app.request_subtab = RequestSubTab::Params;
        }

        let url_w = (ui.available_width() - 238.0).max(140.0);
        if swaggen_ui_kit::text_input(
            ui,
            &mut app.state.requests[req_idx].url,
            "https://api.example.test/v1/items?page=1&limit=10",
            url_w,
            false,
        )
        .changed()
        {
            app.state.requests[req_idx].sync_params_from_url();
            changed = true;
        }

        let env_label = app
            .state
            .active_environment()
            .map(|e| {
                if e.is_production {
                    format!("PROD: {}", e.name)
                } else {
                    e.name.clone()
                }
            })
            .unwrap_or_else(|| "No env".into());
        let env_color = if active_env_is_prod {
            Tokens::TEXT_DANGER
        } else {
            Tokens::TEXT_PRIMARY
        };
        egui::ComboBox::from_id_salt("active-env")
            .selected_text(
                egui::RichText::new(env_label)
                    .size(11.5)
                    .color(env_color)
                    .strong(),
            )
            .width(104.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(app.state.active_environment_id.is_none(), "No environment")
                    .clicked()
                {
                    app.state.active_environment_id = None;
                    changed = true;
                }
                for env in &app.state.environments {
                    let selected =
                        app.state.active_environment_id.as_deref() == Some(env.id.as_str());
                    let text = if env.is_production {
                        egui::RichText::new(format!("PROD · {}", env.name))
                            .color(Tokens::TEXT_DANGER)
                    } else {
                        egui::RichText::new(&env.name)
                    };
                    if ui.selectable_label(selected, text).clicked() {
                        app.state.active_environment_id = Some(env.id.clone());
                        changed = true;
                    }
                }
            });

        if is_inflight {
            if swaggen_ui_kit::danger_button(ui, true, "Cancel").clicked() {
                app.cancel_inflight();
            }
        } else if swaggen_ui_kit::primary_button(ui, app.loaded, "Send")
            .on_hover_text("Send HTTP request directly from this PC (Ctrl+Enter)")
            .clicked()
        {
            app.send_active_request(false);
        }

        // `⋯` Request Options Menu (replaces bulky Duplicate & Save draft buttons!)
        let menu_lbl = if app.dirty() { "⋯*" } else { "⋯" };
        let dots_btn = ui.add(
            egui::Button::new(
                egui::RichText::new(menu_lbl)
                    .size(13.0)
                    .color(if app.dirty() {
                        Tokens::STATUS_INFO
                    } else {
                        Tokens::TEXT_PRIMARY
                    })
                    .strong(),
            )
            .fill(Tokens::BG_SURFACE)
            .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
            .min_size(egui::vec2(28.0, 28.0))
            .corner_radius(5),
        );
        let req_menu_id = ui.make_persistent_id(("top_req_menu", &req_id));
        let dots_btn = dots_btn.on_hover_text(
            "Request actions: Save draft, Duplicate, Rename, Move to collection, Delete",
        );
        if dots_btn.clicked() {
            egui::Popup::toggle_id(ui.ctx(), req_menu_id);
        }
        egui::Popup::new(req_menu_id, ui.ctx().clone(), &dots_btn, ui.layer_id()).show(|ui| {
            ui.set_min_width(210.0);
            ui.label(
                egui::RichText::new("Request name")
                    .size(10.5)
                    .color(Tokens::TEXT_SECONDARY),
            );
            if ui
                .text_edit_singleline(&mut app.state.requests[req_idx].name)
                .changed()
            {
                changed = true;
            }
            let mut tags_csv = app.state.requests[req_idx].tags.join(", ");
            ui.label(
                egui::RichText::new("Tags (comma separated)")
                    .size(10.5)
                    .color(Tokens::TEXT_SECONDARY),
            );
            if ui.text_edit_singleline(&mut tags_csv).changed() {
                app.state.requests[req_idx].tags = tags_csv
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
                changed = true;
            }
            ui.separator();
            if ui
                .button(if app.dirty() {
                    "Save draft* (Ctrl+S)"
                } else {
                    "Save draft (Ctrl+S)"
                })
                .clicked()
            {
                do_save = true;
                ui.close();
            }
            if ui.button("Duplicate request").clicked() {
                do_duplicate = true;
                ui.close();
            }
            ui.menu_button("Move to collection ▸", |ui| {
                ui.set_min_width(155.0);
                for (cid, cname) in &all_cols {
                    let is_cur = *cid == cur_col_id;
                    let lbl = if is_cur {
                        format!("✓ {cname}")
                    } else {
                        cname.clone()
                    };
                    if ui.button(lbl).clicked() {
                        if !is_cur {
                            move_to_col = Some(cid.clone());
                        }
                        ui.close();
                    }
                }
            });
            if ui.button("Copy as cURL").clicked() {
                do_copy_curl = true;
                ui.close();
            }
            ui.separator();
            if ui
                .button(egui::RichText::new("Delete request").color(Tokens::TEXT_DANGER))
                .clicked()
            {
                do_delete = true;
                ui.close();
            }
        });
    });
    ui.add_space(3.0);

    // Auto-hydrate query & path parameters if an older saved request has inline URL params.
    app.state.requests[req_idx].reconcile_missing_params_from_url();

    if do_copy_curl {
        let curl =
            swaggen_spec_engine::importers::to_curl_command(&app.state.requests[req_idx], true);
        ui.ctx().copy_text(curl);
        app.status = "Copied cURL command to clipboard".into();
    }
    if let Some(target_cid) = move_to_col
        && app.state.move_request(&req_id, &target_cid, None)
    {
        changed = true;
        app.status = "Moved request to collection".into();
    }
    if do_duplicate && let Some(new_id) = app.state.duplicate_request(&req_id) {
        app.state.open_or_focus_tab(&new_id);
        changed = true;
    }
    if changed {
        app.state.requests[req_idx].dirty_vs_shared = true;
        app.mark_dirty();
    }
    if do_save {
        app.save_workspace();
    }
    if do_delete {
        app.state.delete_request(&req_id);
        app.mark_dirty();
        return None;
    }

    app.state.requests.iter().position(|r| r.id == req_id)
}

pub fn show_config_panel(app: &mut App, ui: &mut egui::Ui, req_idx: usize, allocated_height: f32) {
    if req_idx >= app.state.requests.len() {
        return;
    }
    let mut changed = false;

    let masked_secrets = app.masked_secret_placeholders();
    let req_clone = app.state.requests[req_idx].clone();
    let col = app
        .state
        .collections
        .iter()
        .find(|c| c.id == req_clone.collection_id)
        .cloned();
    let fchain = folder_chain(&app.state.folders, req_clone.folder_id.as_deref());
    let col_vars = col.as_ref().map(|c| c.variables.as_slice()).unwrap_or(&[]);
    let ctx = VariableContext {
        workspace_vars: &app.state.workspace_vars,
        collection_vars: col_vars,
        environment: app.state.active_environment(),
        request_vars: &req_clone.local_vars,
        local_secrets: &masked_secrets,
    };
    let now_secs = time::OffsetDateTime::now_utc().unix_timestamp();
    let resolved = resolve_request_preview(
        &req_clone,
        col.as_ref(),
        &fchain,
        &ctx,
        &app.state.cookies,
        now_secs,
    );

    // 3. Dedicated Sub-tabs: `Query String` | `Path Params` | `Headers` | (`Body` only on non-GET/HEAD!) | `Auth` | `Tests` | `Settings` | `Code`
    let q_count = req_clone
        .query_params
        .iter()
        .filter(|q| !q.key.is_empty() || !q.value.is_empty())
        .count();
    let p_count = req_clone.path_params.len();
    let h_count = req_clone.headers.len();
    let a_count = req_clone.assertions.len();
    let query_lbl = if q_count > 0 {
        format!("Query String ({q_count})")
    } else {
        "Query String".into()
    };
    let path_lbl = if p_count > 0 {
        format!("Path Params ({p_count})")
    } else {
        "Path Params".into()
    };
    let headers_lbl = if h_count > 0 {
        format!("Headers ({h_count})")
    } else {
        "Headers".into()
    };
    let tests_lbl = if a_count > 0 {
        format!("Tests ({a_count})")
    } else {
        "Tests".into()
    };

    let allows_body = !matches!(req_clone.method, HttpMethod::Get | HttpMethod::Head);
    if !allows_body && app.request_subtab == RequestSubTab::Body {
        app.request_subtab = RequestSubTab::Params;
    }

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let mut tabs: Vec<(RequestSubTab, &str)> = vec![
            (RequestSubTab::Params, query_lbl.as_str()),
            (RequestSubTab::PathParams, path_lbl.as_str()),
            (RequestSubTab::Headers, headers_lbl.as_str()),
        ];
        if allows_body {
            tabs.push((RequestSubTab::Body, "Body"));
        }
        tabs.extend([
            (RequestSubTab::Auth, "Auth"),
            (RequestSubTab::Assertions, tests_lbl.as_str()),
            (RequestSubTab::Connection, "Settings"),
            (RequestSubTab::Resolved, "Code"),
        ]);

        for (tab, label) in tabs {
            if swaggen_ui_kit::underline_tab(ui, app.request_subtab == tab, label).clicked() {
                app.request_subtab = tab;
                app.request_config_collapsed = false;
                app.response_maximized = false;
            }
        }
        if !app.state.layout.workbench_side_by_side {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let collapse_lbl = if app.request_config_collapsed {
                    "Show panel ▾"
                } else {
                    "Minimize ▴"
                };
                if ui
                    .small_button(collapse_lbl)
                    .on_hover_text("Collapse or expand request parameters panel")
                    .clicked()
                {
                    app.request_config_collapsed = !app.request_config_collapsed;
                }
            });
        }
    });

    if app.request_config_collapsed && !app.state.layout.workbench_side_by_side {
        return;
    }
    ui.add_space(2.0);

    let mut pending_oauth_cmd: Option<crate::worker::Command> = None;
    let card_h = (allocated_height - 32.0).max(46.0);
    let inner_scroll_h = (card_h - 14.0).max(34.0);
    let mut kv_ratio = app.state.layout.kv_key_ratio.clamp(0.18, 0.68);

    let suggestions: Vec<(String, String)> = app
        .responses
        .get(&req_clone.id)
        .and_then(|r| r.as_ref().ok())
        .map(|resp| infer_query_param_suggestions(&resp.preview_bytes))
        .unwrap_or_default();

    egui::Frame::new()
        .fill(Tokens::BG_SURFACE)
        .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height(inner_scroll_h);
            ui.set_max_height(inner_scroll_h);
            egui::ScrollArea::vertical()
                .id_salt("req-editor-scroll")
                .min_scrolled_height(inner_scroll_h)
                .max_height(inner_scroll_h)
                .auto_shrink([false, false])
                .show(ui, |ui| match app.request_subtab {
                    RequestSubTab::Params => {
                        // Smart quick-filter pills inferred from response JSON (e.g. MockAPI fields + pagination)
                        if !suggestions.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);
                                ui.label(
                                    egui::RichText::new("Detected filters:")
                                        .size(10.5)
                                        .color(Tokens::TEXT_SECONDARY),
                                );
                                for (sk, sv) in &suggestions {
                                    let already = app.state.requests[req_idx]
                                        .query_params
                                        .iter()
                                        .any(|q| q.key == *sk && q.enabled);
                                    if already {
                                        continue;
                                    }
                                    let chip = if sv.is_empty() {
                                        format!("+ {sk}")
                                    } else {
                                        format!("+ {sk}={sv}")
                                    };
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new(chip)
                                                    .monospace()
                                                    .size(10.5)
                                                    .color(Tokens::STATUS_INFO),
                                            )
                                            .fill(Tokens::BG_SELECTED)
                                            .stroke(egui::Stroke::new(1.0, Tokens::BORDER_DEFAULT))
                                            .corner_radius(4),
                                        )
                                        .on_hover_text("Click to add query parameter and sync URL")
                                        .clicked()
                                    {
                                        app.state.requests[req_idx]
                                            .query_params
                                            .push(KeyValueRow::new(sk.clone(), sv.clone()));
                                        app.state.requests[req_idx].sync_url_from_query_params();
                                        changed = true;
                                    }
                                }
                            });
                            ui.add_space(3.0);
                        }

                        if edit_kv_table_with_ghost(
                            ui,
                            "query-params",
                            "query key (e.g. page, search)",
                            &mut app.state.requests[req_idx].query_params,
                            &mut kv_ratio,
                        ) {
                            app.state.requests[req_idx].sync_url_from_query_params();
                            changed = true;
                        }
                    }
                    RequestSubTab::PathParams => {
                        if app.state.requests[req_idx].path_params.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    egui::RichText::new(
                                        "Use :id or {id} in the URL path (e.g. /se193254/:id) to bind path variables:",
                                    )
                                    .size(11.0)
                                    .color(Tokens::TEXT_SECONDARY),
                                );
                                if swaggen_ui_kit::secondary_button(ui, true, "+ Append /:id")
                                    .clicked()
                                {
                                    let req = &mut app.state.requests[req_idx];
                                    let (base, query, frag) = split_url_parts(&req.url);
                                    let sep = if base.ends_with('/') { "" } else { "/" };
                                    let mut new_url = format!("{base}{sep}:id");
                                    if let Some(q) = query {
                                        new_url.push('?');
                                        new_url.push_str(q);
                                    }
                                    if let Some(f) = frag {
                                        new_url.push('#');
                                        new_url.push_str(f);
                                    }
                                    req.url = new_url;
                                    req.sync_params_from_url();
                                    changed = true;
                                }
                            });
                        } else {
                            let prev_keys: Vec<String> = app.state.requests[req_idx]
                                .path_params
                                .iter()
                                .map(|p| p.key.clone())
                                .collect();
                            let prev_len = prev_keys.len();
                            if edit_kv_table(
                                ui,
                                "path-params",
                                "path variable",
                                &mut app.state.requests[req_idx].path_params,
                                &mut kv_ratio,
                            ) {
                                let req = &mut app.state.requests[req_idx];
                                if req.path_params.len() == prev_len {
                                    for (old_k, row) in prev_keys.iter().zip(req.path_params.iter())
                                    {
                                        if old_k != &row.key && !row.key.trim().is_empty() {
                                            req.url = rename_url_path_param_key(
                                                &req.url,
                                                old_k,
                                                row.key.trim(),
                                            );
                                        }
                                    }
                                } else if req.path_params.len() > prev_len {
                                    if let Some(last) = req.path_params.last_mut()
                                        && last.key.is_empty()
                                    {
                                        let mut next_name = "id".to_owned();
                                        let mut n = 2usize;
                                        while prev_keys.iter().any(|k| k == &next_name) {
                                            next_name = format!("id_{n}");
                                            n += 1;
                                        }
                                        last.key = next_name.clone();
                                        let (base, query, frag) = split_url_parts(&req.url);
                                        let sep = if base.ends_with('/') { "" } else { "/" };
                                        let mut new_url = format!("{base}{sep}:{next_name}");
                                        if let Some(q) = query {
                                            new_url.push('?');
                                            new_url.push_str(q);
                                        }
                                        if let Some(f) = frag {
                                            new_url.push('#');
                                            new_url.push_str(f);
                                        }
                                        req.url = new_url;
                                    }
                                } else if req.path_params.len() < prev_len {
                                    for old_k in &prev_keys {
                                        if !req.path_params.iter().any(|p| &p.key == old_k) {
                                            req.url = remove_url_path_param_key(&req.url, old_k);
                                        }
                                    }
                                }
                                changed = true;
                            }
                        }
                        if resolved.masked_url != req_clone.url {
                            ui.add_space(4.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    egui::RichText::new("Resolved URL:")
                                        .size(11.0)
                                        .color(Tokens::TEXT_SECONDARY),
                                );
                                ui.monospace(
                                    egui::RichText::new(&resolved.masked_url)
                                        .size(11.0)
                                        .color(Tokens::STATUS_INFO),
                                );
                            });
                        }
                    }
                    RequestSubTab::Headers => {
                        if edit_kv_table_with_ghost(
                            ui,
                            "req-headers",
                            "header (e.g. Accept, X-Custom)",
                            &mut app.state.requests[req_idx].headers,
                            &mut kv_ratio,
                        ) {
                            changed = true;
                        }
                        let auto_headers: Vec<_> = resolved
                            .masked_headers
                            .iter()
                            .filter(|h| h.generated)
                            .collect();
                        if !auto_headers.is_empty() {
                            egui::CollapsingHeader::new(
                                egui::RichText::new(format!(
                                    "Auto headers ({})",
                                    auto_headers.len()
                                ))
                                .size(11.0)
                                .color(Tokens::TEXT_SECONDARY),
                            )
                            .default_open(false)
                            .show(ui, |ui| {
                                for h in auto_headers {
                                    ui.monospace(format!("{}: {}", h.name, h.value));
                                }
                            });
                        }
                    }
                    RequestSubTab::Body => {
                        let mut kind_idx = match &app.state.requests[req_idx].body {
                            RequestBody::None => 0,
                            RequestBody::Json(_) => 1,
                            RequestBody::Text(_) => 2,
                            RequestBody::Xml(_) => 3,
                            RequestBody::FormUrlEncoded(_) => 4,
                            RequestBody::Multipart(_) => 5,
                            RequestBody::Binary { .. } => 6,
                        };
                        ui.horizontal_wrapped(|ui| {
                            for (idx, lbl) in [
                                (0, "None"),
                                (1, "JSON"),
                                (2, "Text"),
                                (3, "XML"),
                                (4, "Form URL-encoded"),
                                (5, "Multipart"),
                                (6, "Binary"),
                            ] {
                                if ui.selectable_label(kind_idx == idx, lbl).clicked()
                                    && kind_idx != idx
                                {
                                    kind_idx = idx;
                                    app.state.requests[req_idx].body = match idx {
                                        0 => RequestBody::None,
                                        1 => RequestBody::Json("{\n  \n}".into()),
                                        2 => RequestBody::Text(String::new()),
                                        3 => RequestBody::Xml("<root/>".into()),
                                        4 => RequestBody::FormUrlEncoded(Vec::new()),
                                        5 => RequestBody::Multipart(Vec::new()),
                                        _ => RequestBody::Binary {
                                            local_path: None,
                                            content_type: "application/octet-stream".into(),
                                        },
                                    };
                                    changed = true;
                                }
                            }
                        });
                        match &mut app.state.requests[req_idx].body {
                            RequestBody::None => {
                                ui.label(
                                    egui::RichText::new("No body attached to this request.")
                                        .size(11.5)
                                        .color(Tokens::TEXT_SECONDARY),
                                );
                            }
                            RequestBody::Json(text) => {
                                ui.horizontal(|ui| {
                                    if ui.small_button("Format JSON").clicked()
                                        && let Ok(val) =
                                            serde_json::from_str::<serde_json::Value>(text)
                                        && let Ok(pretty) = serde_json::to_string_pretty(&val)
                                    {
                                        *text = pretty;
                                        changed = true;
                                    }
                                    if !text.trim().is_empty()
                                        && let Err(err) =
                                            serde_json::from_str::<serde_json::Value>(text)
                                    {
                                        ui.label(
                                            egui::RichText::new(format!("JSON syntax: {err}"))
                                                .size(11.0)
                                                .color(Tokens::TEXT_WARNING),
                                        );
                                    }
                                });
                                if ui
                                    .add(
                                        egui::TextEdit::multiline(text)
                                            .desired_width(ui.available_width())
                                            .desired_rows(4)
                                            .code_editor(),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            }
                            RequestBody::Text(text) | RequestBody::Xml(text) => {
                                if ui
                                    .add(
                                        egui::TextEdit::multiline(text)
                                            .desired_width(ui.available_width())
                                            .desired_rows(4)
                                            .code_editor(),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                            }
                            RequestBody::FormUrlEncoded(rows) => {
                                if edit_kv_table_with_ghost(
                                    ui,
                                    "form-body",
                                    "field",
                                    rows,
                                    &mut kv_ratio,
                                ) {
                                    changed = true;
                                }
                            }
                            RequestBody::Multipart(fields) => {
                                let mut del_idx = None;
                                for (i, f) in fields.iter_mut().enumerate() {
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.checkbox(&mut f.enabled, "").changed() {
                                            changed = true;
                                        }
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(&mut f.name)
                                                    .hint_text("field")
                                                    .desired_width(110.0),
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                        match &mut f.value {
                                            MultipartValueKind::Text(v) => {
                                                if ui
                                                    .add(
                                                        egui::TextEdit::singleline(v)
                                                            .hint_text("value")
                                                            .desired_width(210.0),
                                                    )
                                                    .changed()
                                                {
                                                    changed = true;
                                                }
                                            }
                                            MultipartValueKind::File {
                                                local_path,
                                                file_name,
                                                ..
                                            } => {
                                                if ui
                                                    .add(
                                                        egui::TextEdit::singleline(local_path)
                                                            .hint_text("C:\\path\\to\\file.bin")
                                                            .desired_width(210.0),
                                                    )
                                                    .changed()
                                                {
                                                    *file_name = std::path::Path::new(local_path)
                                                        .file_name()
                                                        .map(|n| n.to_string_lossy().into_owned())
                                                        .unwrap_or_else(|| "upload.bin".into());
                                                    changed = true;
                                                }
                                                if !std::path::Path::new(local_path).is_file() {
                                                    swaggen_ui_kit::status_badge(
                                                        ui,
                                                        NoticeKind::Danger,
                                                        "File missing",
                                                    );
                                                }
                                            }
                                        }
                                        if ui.small_button("×").clicked() {
                                            del_idx = Some(i);
                                        }
                                    });
                                }
                                if let Some(i) = del_idx {
                                    fields.remove(i);
                                    changed = true;
                                }
                                ui.horizontal(|ui| {
                                    if swaggen_ui_kit::secondary_button(ui, true, "+ Text field")
                                        .clicked()
                                    {
                                        fields.push(MultipartField {
                                            name: String::new(),
                                            value: MultipartValueKind::Text(String::new()),
                                            enabled: true,
                                        });
                                        changed = true;
                                    }
                                    if swaggen_ui_kit::secondary_button(ui, true, "+ File field")
                                        .clicked()
                                    {
                                        fields.push(MultipartField {
                                            name: "file".into(),
                                            value: MultipartValueKind::File {
                                                local_path: String::new(),
                                                file_name: "upload.bin".into(),
                                                content_type: "application/octet-stream".into(),
                                            },
                                            enabled: true,
                                        });
                                        changed = true;
                                    }
                                });
                            }
                            RequestBody::Binary {
                                local_path,
                                content_type,
                            } => {
                                let mut p = local_path.clone().unwrap_or_default();
                                ui.horizontal(|ui| {
                                    ui.label("Local file:");
                                    if ui
                                        .add(
                                            egui::TextEdit::singleline(&mut p).desired_width(250.0),
                                        )
                                        .changed()
                                    {
                                        *local_path = if p.trim().is_empty() {
                                            None
                                        } else {
                                            Some(p)
                                        };
                                        changed = true;
                                    }
                                    ui.label("Content-Type:");
                                    if ui
                                        .add(
                                            egui::TextEdit::singleline(content_type)
                                                .desired_width(170.0),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                });
                            }
                        }
                    }
                    RequestSubTab::Auth => {
                        let eff = resolve_effective_auth(col.as_ref(), &fchain, &req_clone);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(format!("Effective: {}", eff.source))
                                    .size(11.0)
                                    .color(Tokens::TEXT_SECONDARY),
                            );
                            let mut auth_idx = match &app.state.requests[req_idx].auth {
                                AuthConfig::Inherit => 0,
                                AuthConfig::NoAuth => 1,
                                AuthConfig::Bearer { .. } => 2,
                                AuthConfig::Basic { .. } => 3,
                                AuthConfig::ApiKey { .. } => 4,
                            };
                            for (idx, lbl) in [
                                (0, "Inherit"),
                                (1, "No Auth"),
                                (2, "Bearer"),
                                (3, "Basic"),
                                (4, "API Key"),
                            ] {
                                if ui.selectable_label(auth_idx == idx, lbl).clicked()
                                    && auth_idx != idx
                                {
                                    auth_idx = idx;
                                    app.state.requests[req_idx].auth = match idx {
                                        0 => AuthConfig::Inherit,
                                        1 => AuthConfig::NoAuth,
                                        2 => AuthConfig::Bearer {
                                            token: "{{api_token}}".into(),
                                        },
                                        3 => AuthConfig::Basic {
                                            username: String::new(),
                                            password: String::new(),
                                        },
                                        _ => AuthConfig::ApiKey {
                                            key: "X-Api-Key".into(),
                                            value: "{{api_key}}".into(),
                                            placement: ApiKeyPlacement::Header,
                                        },
                                    };
                                    changed = true;
                                }
                            }
                        });
                        match &mut app.state.requests[req_idx].auth {
                            AuthConfig::Inherit => {
                                ui.label("Inherits auth from parent folder or collection.");
                            }
                            AuthConfig::NoAuth => {
                                ui.label("No authorization header attached.");
                            }
                            AuthConfig::Bearer { token } => {
                                ui.horizontal(|ui| {
                                    ui.label("Bearer token:");
                                    if ui
                                        .add(
                                            egui::TextEdit::singleline(token).desired_width(280.0),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                });
                                ui.collapsing("OAuth 2.0 PKCE Helper", |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label("Auth URL:");
                                        ui.add(
                                            egui::TextEdit::singleline(&mut app.oauth_auth_url)
                                                .desired_width(190.0),
                                        );
                                        ui.label("Token URL:");
                                        ui.add(
                                            egui::TextEdit::singleline(&mut app.oauth_token_url)
                                                .desired_width(190.0),
                                        );
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Client ID:");
                                        ui.add(
                                            egui::TextEdit::singleline(&mut app.oauth_client_id)
                                                .desired_width(130.0),
                                        );
                                        ui.label("Scope:");
                                        ui.add(
                                            egui::TextEdit::singleline(&mut app.oauth_scope)
                                                .desired_width(110.0),
                                        );
                                        if swaggen_ui_kit::secondary_button(
                                            ui,
                                            !app.oauth_auth_url.trim().is_empty()
                                                && !app.oauth_token_url.trim().is_empty(),
                                            "Start PKCE",
                                        )
                                        .clicked()
                                        {
                                            pending_oauth_cmd =
                                                Some(crate::worker::Command::RunOAuth2Pkce {
                                                    auth_url: app.oauth_auth_url.trim().to_owned(),
                                                    token_url: app
                                                        .oauth_token_url
                                                        .trim()
                                                        .to_owned(),
                                                    client_id: app
                                                        .oauth_client_id
                                                        .trim()
                                                        .to_owned(),
                                                    scope: app.oauth_scope.trim().to_owned(),
                                                    downloads_dir: app.root.join("downloads"),
                                                });
                                        }
                                    });
                                });
                            }
                            AuthConfig::Basic { username, password } => {
                                ui.horizontal(|ui| {
                                    ui.label("Username:");
                                    if ui.text_edit_singleline(username).changed() {
                                        changed = true;
                                    }
                                    ui.label("Password:");
                                    if ui
                                        .add(egui::TextEdit::singleline(password).password(true))
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                });
                            }
                            AuthConfig::ApiKey {
                                key,
                                value,
                                placement,
                            } => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label("Key:");
                                    if ui.text_edit_singleline(key).changed() {
                                        changed = true;
                                    }
                                    ui.label("Value:");
                                    if ui.text_edit_singleline(value).changed() {
                                        changed = true;
                                    }
                                    if ui
                                        .selectable_label(
                                            *placement == ApiKeyPlacement::Header,
                                            "Header",
                                        )
                                        .clicked()
                                    {
                                        *placement = ApiKeyPlacement::Header;
                                        changed = true;
                                    }
                                    if ui
                                        .selectable_label(
                                            *placement == ApiKeyPlacement::Query,
                                            "Query",
                                        )
                                        .clicked()
                                    {
                                        *placement = ApiKeyPlacement::Query;
                                        changed = true;
                                    }
                                });
                            }
                        }
                    }
                    RequestSubTab::Assertions => {
                        let rules = &mut app.state.requests[req_idx].assertions;
                        let mut del_idx = None;
                        for (i, r) in rules.iter_mut().enumerate() {
                            ui.horizontal_wrapped(|ui| {
                                if ui.checkbox(&mut r.enabled, "").changed() {
                                    changed = true;
                                }
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut r.label)
                                            .hint_text("label")
                                            .desired_width(100.0),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                let tname = match &r.target {
                                    AssertionTarget::StatusEquals => "Status",
                                    AssertionTarget::MaxDurationMs => "Max ms",
                                    AssertionTarget::HeaderEquals { .. } => "Header",
                                    AssertionTarget::JsonPathEquals { .. } => "JSON path",
                                    AssertionTarget::BodyContains => "Contains",
                                };
                                egui::ComboBox::from_id_salt(("assert-kind", i))
                                    .selected_text(tname)
                                    .width(84.0)
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .selectable_label(
                                                matches!(r.target, AssertionTarget::StatusEquals),
                                                "Status",
                                            )
                                            .clicked()
                                        {
                                            r.target = AssertionTarget::StatusEquals;
                                            changed = true;
                                        }
                                        if ui
                                            .selectable_label(
                                                matches!(r.target, AssertionTarget::MaxDurationMs),
                                                "Max ms",
                                            )
                                            .clicked()
                                        {
                                            r.target = AssertionTarget::MaxDurationMs;
                                            changed = true;
                                        }
                                        if ui
                                            .selectable_label(
                                                matches!(
                                                    r.target,
                                                    AssertionTarget::JsonPathEquals { .. }
                                                ),
                                                "JSON path",
                                            )
                                            .clicked()
                                        {
                                            r.target = AssertionTarget::JsonPathEquals {
                                                path: "$.ok".into(),
                                            };
                                            changed = true;
                                        }
                                        if ui
                                            .selectable_label(
                                                matches!(
                                                    r.target,
                                                    AssertionTarget::HeaderEquals { .. }
                                                ),
                                                "Header",
                                            )
                                            .clicked()
                                        {
                                            r.target = AssertionTarget::HeaderEquals {
                                                name: "content-type".into(),
                                            };
                                            changed = true;
                                        }
                                        if ui
                                            .selectable_label(
                                                matches!(r.target, AssertionTarget::BodyContains),
                                                "Contains",
                                            )
                                            .clicked()
                                        {
                                            r.target = AssertionTarget::BodyContains;
                                            changed = true;
                                        }
                                    });
                                match &mut r.target {
                                    AssertionTarget::JsonPathEquals { path }
                                    | AssertionTarget::HeaderEquals { name: path } => {
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(path)
                                                    .desired_width(100.0),
                                            )
                                            .changed()
                                        {
                                            changed = true;
                                        }
                                    }
                                    _ => {}
                                }
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut r.expected)
                                            .hint_text("expected")
                                            .desired_width(110.0),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui.small_button("×").clicked() {
                                    del_idx = Some(i);
                                }
                            });
                        }
                        if let Some(i) = del_idx {
                            rules.remove(i);
                            changed = true;
                        }
                        ui.horizontal(|ui| {
                            if swaggen_ui_kit::secondary_button(ui, true, "+ Assertion").clicked()
                            {
                                app.state.requests[req_idx].assertions.push(AssertionRule {
                                    id: new_uid(),
                                    label: "Status 200".into(),
                                    target: AssertionTarget::StatusEquals,
                                    expected: "200".into(),
                                    enabled: true,
                                });
                                changed = true;
                            }
                            if swaggen_ui_kit::secondary_button(ui, true, "+ Extractor").clicked()
                            {
                                app.state.requests[req_idx].extractors.push(
                                    swaggen_domain::dynamic_vars::ResponseExtractorRule {
                                        id: new_uid(),
                                        target_var: "extracted_id".into(),
                                        source:
                                            swaggen_domain::dynamic_vars::ExtractorSource::BodyJsonPath,
                                        expression: "$.id".into(),
                                        mark_secret: false,
                                    },
                                );
                                changed = true;
                            }
                        });
                        let extractors = &mut app.state.requests[req_idx].extractors;
                        let mut rm_ext = None;
                        for (i, ex) in extractors.iter_mut().enumerate() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Save var:");
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut ex.target_var)
                                            .desired_width(96.0),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui
                                    .selectable_label(
                                        ex.source
                                            == swaggen_domain::dynamic_vars::ExtractorSource::BodyJsonPath,
                                        "JSONPath",
                                    )
                                    .clicked()
                                {
                                    ex.source =
                                        swaggen_domain::dynamic_vars::ExtractorSource::BodyJsonPath;
                                    changed = true;
                                }
                                if ui
                                    .selectable_label(
                                        ex.source
                                            == swaggen_domain::dynamic_vars::ExtractorSource::Header,
                                        "Header",
                                    )
                                    .clicked()
                                {
                                    ex.source =
                                        swaggen_domain::dynamic_vars::ExtractorSource::Header;
                                    changed = true;
                                }
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut ex.expression)
                                            .hint_text("$.token")
                                            .desired_width(130.0),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui.checkbox(&mut ex.mark_secret, "Secret").changed() {
                                    changed = true;
                                }
                                if ui.small_button("×").clicked() {
                                    rm_ext = Some(i);
                                }
                            });
                        }
                        if let Some(i) = rm_ext {
                            extractors.remove(i);
                            changed = true;
                        }
                    }
                    RequestSubTab::Connection => {
                        let conn = &mut app.state.requests[req_idx].connection;
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Timeout (ms):");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut conn.timeout_ms)
                                        .range(100..=300_000),
                                )
                                .changed()
                            {
                                changed = true;
                            }
                            if ui
                                .checkbox(&mut conn.follow_redirects, "Follow redirects")
                                .changed()
                            {
                                changed = true;
                            }
                            ui.label("Max:");
                            if ui
                                .add(egui::DragValue::new(&mut conn.max_redirects).range(0..=20))
                                .changed()
                            {
                                changed = true;
                            }
                            if ui
                                .checkbox(
                                    &mut conn.send_auth_on_cross_host_redirect,
                                    "Forward auth across host redirects",
                                )
                                .changed()
                            {
                                changed = true;
                            }
                        });
                        let mut exc_csv = conn.tls_host_exceptions.join(", ");
                        let mut hp = conn.proxy.http_proxy.clone().unwrap_or_default();
                        ui.horizontal_wrapped(|ui| {
                            ui.label("TLS exceptions:");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut exc_csv)
                                        .hint_text("localhost:8443")
                                        .desired_width(160.0),
                                )
                                .changed()
                            {
                                conn.tls_host_exceptions = exc_csv
                                    .split(',')
                                    .map(|s| s.trim().to_owned())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                changed = true;
                            }
                            ui.label("Local HTTP proxy:");
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut hp)
                                        .hint_text("Direct from PC (none)")
                                        .desired_width(170.0),
                                )
                                .changed()
                            {
                                conn.proxy.http_proxy =
                                    if hp.trim().is_empty() { None } else { Some(hp) };
                                changed = true;
                            }
                        });
                    }
                    RequestSubTab::Resolved => {
                        for err in &resolved.blocking_errors {
                            swaggen_ui_kit::notice_banner(ui, NoticeKind::Danger, "Blocked", err);
                        }
                        for w in &resolved.warnings {
                            swaggen_ui_kit::notice_banner(ui, NoticeKind::Warning, "Note", w);
                        }
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} {}",
                                    resolved.method.as_str(),
                                    resolved.masked_url
                                ))
                                .monospace()
                                .size(11.0)
                                .color(Tokens::STATUS_INFO),
                            );
                            for t in swaggen_runner::protocols_and_tools::SnippetTarget::ALL {
                                if ui
                                    .selectable_label(app.snippet_target == t, t.label())
                                    .clicked()
                                {
                                    app.snippet_target = t;
                                }
                            }
                            let snippet =
                                swaggen_runner::protocols_and_tools::generate_code_snippet(
                                    &resolved,
                                    app.snippet_target,
                                );
                            if swaggen_ui_kit::secondary_button(ui, true, "Copy code").clicked() {
                                ui.ctx().copy_text(snippet);
                                app.status = "Copied code snippet to clipboard".into();
                            }
                        });
                        let snippet = swaggen_runner::protocols_and_tools::generate_code_snippet(
                            &resolved,
                            app.snippet_target,
                        );
                        swaggen_ui_kit::code_surface_numbered(ui, &snippet);
                    }
                });
        });

    if (kv_ratio - app.state.layout.kv_key_ratio).abs() > 0.002 {
        app.state.layout.kv_key_ratio = kv_ratio;
    }
    if let Some(cmd) = pending_oauth_cmd {
        app.submit(cmd);
    }
    if changed {
        app.state.requests[req_idx].dirty_vs_shared = true;
        app.mark_dirty();
    }
}

/// Postman-style KV editor that ALWAYS provides an inline ghost starter row at the bottom
/// so users can immediately type query parameters or headers without clicking "+ Add row" first.
fn edit_kv_table_with_ghost(
    ui: &mut egui::Ui,
    id_salt: &str,
    placeholder_kind: &str,
    rows: &mut Vec<KeyValueRow>,
    key_ratio: &mut f32,
) -> bool {
    let mut changed = edit_kv_table_inner(ui, id_salt, placeholder_kind, rows, key_ratio, false);
    let has_trailing_blank = rows
        .last()
        .is_some_and(|r| r.key.is_empty() && r.value.is_empty());
    if !has_trailing_blank {
        let next_idx = rows.len();
        let mut ghost_k = String::new();
        let mut ghost_v = String::new();
        ui.push_id(id_salt, |ui| {
            let total_w = ui.available_width().max(300.0);
            let key_w = (total_w * *key_ratio).clamp(90.0, (total_w - 180.0).max(90.0));
            let val_w = (total_w - key_w - 135.0).clamp(90.0, 560.0);
            swaggen_ui_kit::table_row_frame(ui, |ui| {
                let mut dummy_on = true;
                ui.add_enabled(false, egui::Checkbox::new(&mut dummy_on, ""));
                let k_changed = ui
                    .add(
                        egui::TextEdit::singleline(&mut ghost_k)
                            .id(egui::Id::unique((id_salt, "row_key", next_idx)))
                            .hint_text(placeholder_kind)
                            .desired_width(key_w),
                    )
                    .changed();
                let v_changed = ui
                    .add(
                        egui::TextEdit::singleline(&mut ghost_v)
                            .id(egui::Id::unique((id_salt, "row_val", next_idx)))
                            .hint_text("value")
                            .desired_width(val_w),
                    )
                    .changed();
                if (k_changed || v_changed) && (!ghost_k.is_empty() || !ghost_v.is_empty()) {
                    rows.push(KeyValueRow::new(ghost_k, ghost_v));
                    changed = true;
                }
            });
        });
    }
    changed
}

fn edit_kv_table(
    ui: &mut egui::Ui,
    id_salt: &str,
    placeholder_kind: &str,
    rows: &mut Vec<KeyValueRow>,
    key_ratio: &mut f32,
) -> bool {
    edit_kv_table_inner(ui, id_salt, placeholder_kind, rows, key_ratio, true)
}

fn edit_kv_table_inner(
    ui: &mut egui::Ui,
    id_salt: &str,
    placeholder_kind: &str,
    rows: &mut Vec<KeyValueRow>,
    key_ratio: &mut f32,
    show_add_btn: bool,
) -> bool {
    let mut changed = false;
    let mut remove_idx = None;
    let mut swap: Option<(usize, usize)> = None;
    ui.push_id(id_salt, |ui| {
        let header_left = ui.available_rect_before_wrap().left();
        let total_w = ui.available_width().max(300.0);
        let key_w = (total_w * *key_ratio).clamp(90.0, (total_w - 180.0).max(90.0));
        let val_w = (total_w - key_w - 135.0).clamp(90.0, 560.0);

        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(
                egui::vec2(key_w + 18.0, 18.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.label(
                        egui::RichText::new("KEY")
                            .size(10.0)
                            .color(Tokens::TEXT_SECONDARY)
                            .strong(),
                    );
                },
            );
            let (_, col_rect) = ui.allocate_space(egui::vec2(8.0, 18.0));
            let col_id = egui::Id::unique(("kv_col_split", id_salt));
            let col_resp = ui.interact(col_rect, col_id, egui::Sense::click_and_drag());
            let col_active = col_resp.hovered() || col_resp.dragged();
            if col_active {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if col_resp.dragged()
                && let Some(pos) = ui.ctx().pointer_interact_pos()
            {
                *key_ratio = ((pos.x - header_left - 24.0) / total_w).clamp(0.18, 0.68);
                ui.ctx().request_repaint();
            }
            ui.painter().line_segment(
                [
                    egui::pos2(col_rect.center().x, col_rect.top() + 2.0),
                    egui::pos2(col_rect.center().x, col_rect.bottom() - 2.0),
                ],
                egui::Stroke::new(
                    if col_active { 1.5 } else { 1.0 },
                    if col_active {
                        Tokens::STATUS_INFO
                    } else {
                        Tokens::BORDER_DEFAULT
                    },
                ),
            );
            let _ = col_resp.on_hover_text("Drag left/right to resize Key vs Value columns");
            ui.allocate_ui_with_layout(
                egui::vec2(val_w, 18.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.label(
                        egui::RichText::new("VALUE")
                            .size(10.0)
                            .color(Tokens::TEXT_SECONDARY)
                            .strong(),
                    );
                },
            );
        });
        ui.add_space(2.0);

        for (i, row) in rows.iter_mut().enumerate() {
            swaggen_ui_kit::table_row_frame(ui, |ui| {
                if ui.checkbox(&mut row.enabled, "").changed() {
                    changed = true;
                }
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut row.key)
                            .id(egui::Id::unique((id_salt, "row_key", i)))
                            .hint_text(placeholder_kind)
                            .desired_width(key_w),
                    )
                    .changed()
                {
                    changed = true;
                }
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut row.value)
                            .id(egui::Id::unique((id_salt, "row_val", i)))
                            .hint_text("value")
                            .desired_width(val_w),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").on_hover_text("Remove row").clicked() {
                        remove_idx = Some(i);
                    }
                    if i > 0 && ui.small_button("↑").on_hover_text("Move up").clicked() {
                        swap = Some((i, i - 1));
                    }
                    swaggen_ui_kit::pill_badge(
                        ui,
                        if row.enabled {
                            Tokens::STATUS_SUCCESS
                        } else {
                            Tokens::TEXT_SECONDARY
                        },
                        if row.enabled { "On" } else { "Off" },
                    );
                });
            });
        }
        if let Some((a, b)) = swap {
            rows.swap(a, b);
            changed = true;
        }
        if let Some(i) = remove_idx {
            rows.remove(i);
            changed = true;
        }
        if show_add_btn && swaggen_ui_kit::secondary_button(ui, true, "+ Add row").clicked() {
            rows.push(KeyValueRow::new("", ""));
            changed = true;
        }
    });
    changed
}
