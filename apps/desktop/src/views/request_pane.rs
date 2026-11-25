//! Clean Postman-style Request Composer (v0.1.16):
//! - Flat Postman tab strip (top `#1D4ED8` active bar, zero heavy nested boxes)
//! - Unified URL bar (`[ Method ▾ | URL ]  [ Send ]  [ ⋯ ]` with `GRAPHQL` inside Method dropdown)
//! - Live interactive `{{variable}}` status pills + 1-click inline variable quick-setter
//! - Flat border-collapsed Postman Key-Value grid (zero box-inside-box clutter)
//! - First-class `Scripts` (`Pre-request` | `Post-response`) JS tab with Postman `pm.*` & Playwright `expect()`
//! - Clean 2-column Request `Settings` & full `Auth` (`Bearer`, `Basic`, `API Key`, `OAuth 2.0`, `AWS SigV4`, `Digest`)
use crate::app::{App, MainMode, RequestSubTab};
use eframe::egui;
use swaggen_domain::{
    graphql::{
        GraphQlOperationKind, analyze_graphql_document, build_operation_template,
        generate_variables_skeleton,
    },
    workspace::{
        ApiKeyPlacement, AssertionRule, AssertionTarget, AuthConfig, ClientCertMapping, HttpMethod,
        HttpVersionPolicy, IdempotencyMode, KeyValueRow, MultipartField, MultipartValueKind,
        OAuthGrantType, RequestBody, RequestItem, VariableContext, VariableEntry, folder_chain,
        infer_query_param_suggestions, inspect_request_variables, new_uid,
        remove_url_path_param_key, rename_url_path_param_key, resolve_effective_auth,
        resolve_request_preview, split_url_parts,
    },
};
use swaggen_ui_kit::{Edge, IconKind, NoticeKind, Tokens};

pub fn show_top_bar(app: &mut App, ui: &mut egui::Ui) -> Option<usize> {
    // 1. Clean Flat Postman-Style Open Tab Strip
    let tab_bar_resp = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let mut switch_tab: Option<String> = None;
        let mut close_tab: Option<String> = None;
        let mut toggle_pin: Option<String> = None;

        let close_unpinned_w = if app.state.open_tabs.len() > 1 {
            88.0
        } else {
            0.0
        };
        let fixed_right_w = 24.0 + 44.0 + 28.0 + close_unpinned_w + 18.0;
        let tabs_scroll_w = (ui.available_width() - fixed_right_w).max(80.0);

        ui.allocate_ui_with_layout(
            egui::vec2(tabs_scroll_w, 28.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("open_tabs_scroll")
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            for tab in &app.state.open_tabs {
                                let (raw_method, name, req_url) = app
                                    .state
                                    .requests
                                    .iter()
                                    .find(|r| r.id == tab.request_id)
                                    .map(|r| {
                                        let badge = if r.is_graphql() {
                                            "GQL"
                                        } else {
                                            r.method.as_str()
                                        };
                                        (badge, r.name.clone(), r.url.clone())
                                    })
                                    .unwrap_or(("GET", "Missing".into(), String::new()));
                                let method = swaggen_ui_kit::compact_method_badge(raw_method);
                                let short_name: String = if name.chars().count() > 20 {
                                    let mut s: String = name.chars().take(19).collect();
                                    s.push('…');
                                    s
                                } else {
                                    name.clone()
                                };
                                let active =
                                    app.state.active_tab_id.as_deref() == Some(tab.id.as_str());
                                let tab_fill = if active {
                                    swaggen_ui_kit::surface_bg(ui)
                                } else {
                                    swaggen_ui_kit::subtle_bg(ui)
                                };
                                let inner = egui::Frame::new()
                                    .fill(tab_fill)
                                    .stroke(egui::Stroke::NONE)
                                    .inner_margin(egui::Margin::symmetric(9, 4))
                                    .show(ui, |ui| {
                                        ui.spacing_mut().item_spacing.x = 5.0;
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new(method)
                                                    .monospace()
                                                    .size(10.0)
                                                    .color(swaggen_ui_kit::method_color(raw_method))
                                                    .strong(),
                                            );
                                            let title_resp = ui
                                                .add(
                                                    egui::Button::new(if active {
                                                        swaggen_ui_kit::bold_text(
                                                            &short_name,
                                                            11.5,
                                                            swaggen_ui_kit::text_primary(ui),
                                                        )
                                                    } else {
                                                        egui::RichText::new(&short_name)
                                                            .size(11.5)
                                                            .color(
                                                                swaggen_ui_kit::text_secondary(ui),
                                                            )
                                                    })
                                                    .fill(egui::Color32::TRANSPARENT)
                                                    .stroke(egui::Stroke::NONE)
                                                    .frame(false),
                                                )
                                                .on_hover_text(format!(
                                                    "{raw_method} {name}\n{req_url}\n(Middle-click to close · Right-click to pin)"
                                                ));
                                            if title_resp.clicked() {
                                                switch_tab = Some(tab.id.clone());
                                            }
                                            if title_resp.secondary_clicked() {
                                                toggle_pin = Some(tab.id.clone());
                                            }
                                            if !tab.pinned && title_resp.middle_clicked() {
                                                close_tab = Some(tab.id.clone());
                                            }
                                            if tab.pinned {
                                                if ui
                                                    .add(
                                                        egui::Button::new(
                                                            egui::RichText::new("📌").size(9.5),
                                                        )
                                                        .frame(false),
                                                    )
                                                    .on_hover_text("Unpin tab")
                                                    .clicked()
                                                {
                                                    toggle_pin = Some(tab.id.clone());
                                                }
                                            } else if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new("×")
                                                            .size(12.0)
                                                            .color(
                                                                swaggen_ui_kit::text_secondary(ui),
                                                            ),
                                                    )
                                                    .frame(false),
                                                )
                                                .on_hover_text("Close tab (Ctrl+W)")
                                                .clicked()
                                            {
                                                close_tab = Some(tab.id.clone());
                                            }
                                        });
                                    });
                                let r = inner.response.rect;
                                ui.painter().line_segment(
                                    [r.right_top(), r.right_bottom()],
                                    egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)),
                                );
                                if active {
                                    ui.painter().line_segment(
                                        [
                                            egui::pos2(r.left(), r.top() + 1.0),
                                            egui::pos2(r.right(), r.top() + 1.0),
                                        ],
                                        egui::Stroke::new(2.0, Tokens::STATUS_INFO),
                                    );
                                }
                            }
                        });
                    });
            },
        );

        let (add_rect, add_resp) =
            ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
        if add_resp.hovered() {
            ui.painter()
                .rect_filled(add_rect, 4.0, swaggen_ui_kit::subtle_bg(ui));
        }
        swaggen_ui_kit::paint_icon(
            ui.painter(),
            add_rect.shrink(4.5),
            IconKind::Plus,
            swaggen_ui_kit::text_primary(ui),
        );
        if add_resp.on_hover_text("New HTTP request").clicked()
            && let Some(cid) = app.state.collections.first().map(|c| c.id.clone())
        {
            let req = RequestItem::new(cid, "New request", HttpMethod::Get, "{{base_url}}/");
            let rid = req.id.clone();
            app.state.requests.push(req);
            app.open_request_in_workbench(&rid);
            app.mark_dirty();
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("+ GQL")
                        .monospace()
                        .size(10.0)
                        .color(swaggen_ui_kit::method_color("GQL"))
                        .strong(),
                )
                .fill(swaggen_ui_kit::surface_bg(ui))
                .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                .min_size(egui::vec2(40.0, 22.0))
                .corner_radius(4),
            )
            .on_hover_text("New GraphQL request")
            .clicked()
            && let Some(cid) = app.state.collections.first().map(|c| c.id.clone())
        {
            let req = RequestItem::new_graphql(cid, "GraphQL query", "{{base_url}}/graphql");
            let rid = req.id.clone();
            app.state.requests.push(req);
            app.open_request_in_workbench(&rid);
            app.mark_dirty();
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if swaggen_ui_kit::split_layout_icon_button(ui, app.state.layout.workbench_side_by_side)
                .clicked()
            {
                app.state.layout.workbench_side_by_side = !app.state.layout.workbench_side_by_side;
                app.response_maximized = false;
                app.request_config_collapsed = false;
            }
            if app.state.open_tabs.len() > 1 && ui.small_button("Close all").clicked() {
                app.state.close_unpinned_tabs();
                app.mark_dirty();
            }
        });

        if let Some(tid) = switch_tab {
            app.state.active_tab_id = Some(tid);
            if let Some(req) = app.active_request() {
                if req.is_graphql() {
                    app.request_subtab = RequestSubTab::GraphQl;
                } else if app.request_subtab == RequestSubTab::GraphQl {
                    app.request_subtab = RequestSubTab::Params;
                }
            }
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
    swaggen_ui_kit::paint_edge_divider(ui, tab_bar_resp.response.rect, Edge::Bottom);
    ui.add_space(6.0);

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
    let is_inflight = app.inflight_request_id.as_deref() == Some(req_id.as_str());

    // 2. Unified Postman URL Bar: `[ Method ▾ | URL input ]  [ Send ]  [ ⋯ ]` (F3)
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let bar_w = (ui.available_width() - 118.0).max(160.0);
        egui::Frame::new()
            .fill(swaggen_ui_kit::surface_bg(ui))
            .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(6, 3))
            .show(ui, |ui| {
                ui.set_min_width((bar_w - 12.0).max(120.0));
                ui.set_max_width((bar_w - 12.0).max(120.0));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let is_gql = app.state.requests[req_idx].is_graphql();
                    let cur_method = app.state.requests[req_idx].method;
                    let method_display = if is_gql {
                        "GRAPHQL"
                    } else {
                        cur_method.as_str()
                    };
                    let method_col = swaggen_ui_kit::method_color(method_display);

                    let method_btn = ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(format!("{method_display} ▾"))
                                    .monospace()
                                    .size(11.5)
                                    .color(method_col)
                                    .strong(),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                            .min_size(egui::vec2(74.0, 22.0)),
                        )
                        .on_hover_text("Select HTTP Method or switch to GraphQL");

                    egui::Popup::menu(&method_btn)
                        .id(egui::Id::unique(("req_method_popup", &req_id)))
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                        .show(|ui| {
                            ui.set_min_width(120.0);
                            for m in HttpMethod::ALL {
                                let sel = !is_gql && cur_method == m;
                                if ui
                                    .selectable_label(
                                        sel,
                                        egui::RichText::new(m.as_str())
                                            .monospace()
                                            .color(swaggen_ui_kit::method_color(m.as_str()))
                                            .strong(),
                                    )
                                    .clicked()
                                {
                                    app.state.requests[req_idx].method = m;
                                    if is_gql {
                                        app.state.requests[req_idx].body = if matches!(
                                            m,
                                            HttpMethod::Get | HttpMethod::Head
                                        ) {
                                            RequestBody::None
                                        } else {
                                            RequestBody::Json("{\n  \n}".into())
                                        };
                                        app.request_subtab = RequestSubTab::Params;
                                    } else if matches!(m, HttpMethod::Get | HttpMethod::Head)
                                        && app.request_subtab == RequestSubTab::Body
                                    {
                                        app.request_subtab = RequestSubTab::Params;
                                    }
                                    changed = true;
                                    ui.close();
                                }
                            }
                            ui.separator();
                            if ui
                                .selectable_label(
                                    is_gql,
                                    egui::RichText::new("GRAPHQL")
                                        .monospace()
                                        .color(swaggen_ui_kit::method_color("GQL"))
                                        .strong(),
                                )
                                .clicked()
                            {
                                if !is_gql {
                                    app.state.requests[req_idx].method = HttpMethod::Post;
                                    app.state.requests[req_idx].body = RequestBody::GraphQl {
                                        query: "query ExampleQuery {\n  __typename\n}".into(),
                                        variables: "{\n  \n}".into(),
                                        operation_name: None,
                                    };
                                }
                                app.request_subtab = RequestSubTab::GraphQl;
                                changed = true;
                                ui.close();
                            }
                        });

                    let sep_r = method_btn.rect;
                    ui.painter().line_segment(
                        [
                            egui::pos2(sep_r.right() + 2.0, sep_r.top() + 2.0),
                            egui::pos2(sep_r.right() + 2.0, sep_r.bottom() - 2.0),
                        ],
                        egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)),
                    );

                    let url_input_w = ui.available_width().max(60.0);
                    let url_hint = if is_gql {
                        "Enter URL or {{base_url}}/graphql"
                    } else {
                        "Enter URL or {{base_url}}/endpoint"
                    };
                    let url_resp = ui
                        .add(
                            egui::TextEdit::singleline(&mut app.state.requests[req_idx].url)
                                .hint_text(url_hint)
                                .frame(egui::Frame::NONE)
                                .desired_width(url_input_w),
                        )
                        .on_hover_text(
                            "Enter URL — supports {{variables}}, :pathParams, and ?query=values (Enter / Ctrl+Enter to Send)",
                        );
                    if url_resp.changed() {
                        app.state.requests[req_idx].sync_params_from_url();
                        changed = true;
                    }
                    if url_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        app.send_active_request(false);
                    }
                });
            });

        if is_inflight {
            if swaggen_ui_kit::danger_button(ui, true, "Cancel").clicked() {
                app.cancel_inflight();
            }
        } else if ui
            .add(
                egui::Button::new(swaggen_ui_kit::bold_text(
                    "Send",
                    12.5,
                    egui::Color32::WHITE,
                ))
                .fill(Tokens::STATUS_INFO)
                .stroke(egui::Stroke::new(1.0, Tokens::STATUS_INFO))
                .min_size(egui::vec2(72.0, 30.0))
                .corner_radius(6),
            )
            .on_hover_text("Send request directly from this PC (Ctrl+Enter)")
            .clicked()
        {
            app.send_active_request(false);
        }

        ui.push_id(("top_req_menu_btn", &req_id), |ui| {
            let menu_lbl = if app.dirty() { "⋯*" } else { "⋯" };
            let dots_btn = ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(menu_lbl)
                            .size(13.0)
                            .color(if app.dirty() {
                                Tokens::STATUS_INFO
                            } else {
                                swaggen_ui_kit::text_primary(ui)
                            })
                            .strong(),
                    )
                    .fill(swaggen_ui_kit::surface_bg(ui))
                    .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                    .min_size(egui::vec2(30.0, 30.0))
                    .corner_radius(6),
                )
                .on_hover_text("Request actions: Rename, Save, Duplicate, Move, Copy cURL, Delete");
            egui::Popup::menu(&dots_btn)
                .align(egui::RectAlign::BOTTOM_END)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    ui.set_min_width(210.0);
                    ui.label(
                        egui::RichText::new("Request name")
                            .size(10.5)
                            .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                    let name_resp = ui.text_edit_singleline(&mut app.state.requests[req_idx].name);
                    if name_resp.changed() {
                        changed = true;
                    }
                    if name_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        ui.close();
                    }
                    let mut tags_csv = app.state.requests[req_idx].tags.join(", ");
                    ui.label(
                        egui::RichText::new("Tags (comma separated)")
                            .size(10.5)
                            .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                    let tags_resp = ui.text_edit_singleline(&mut tags_csv);
                    if tags_resp.changed() {
                        app.state.requests[req_idx].tags = tags_csv
                            .split(',')
                            .map(|s| s.trim().to_owned())
                            .filter(|s| !s.is_empty())
                            .collect();
                        changed = true;
                    }
                    if tags_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        ui.close();
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
                        .button(
                            egui::RichText::new("Move to Trash").color(Tokens::TEXT_DANGER),
                        )
                        .clicked()
                    {
                        do_delete = true;
                        ui.close();
                    }
                });
        });
    });

    // 2b. Live Interactive `{{variable}}` Resolution Pills + 1-Click Quick-Set Popover (F8)
    let masked_secrets = app.masked_secret_placeholders();
    let var_statuses = {
        let req_ref = &app.state.requests[req_idx];
        let col_ref = app
            .state
            .collections
            .iter()
            .find(|c| c.id == req_ref.collection_id);
        let fchain = folder_chain(&app.state.folders, req_ref.folder_id.as_deref());
        let col_vars = col_ref.map(|c| c.variables.as_slice()).unwrap_or(&[]);
        let vctx = VariableContext {
            workspace_vars: &app.state.workspace_vars,
            collection_vars: col_vars,
            environment: app.state.active_environment(),
            request_vars: &req_ref.local_vars,
            local_secrets: &masked_secrets,
        };
        inspect_request_variables(req_ref, col_ref, &fchain, &vctx)
    };
    if !var_statuses.is_empty() {
        ui.add_space(3.0);
        let active_env_name = app
            .state
            .active_environment()
            .map(|e| e.name.clone())
            .unwrap_or_else(|| "Global Workspace".into());
        let mut set_var_action: Option<(String, String, bool)> = None; // (key, val, into_collection)
        let mut open_env_screen = false;

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(5.0, 3.0);
            ui.label(
                egui::RichText::new("Variables:")
                    .size(10.5)
                    .color(swaggen_ui_kit::text_secondary(ui)),
            );
            for vs in &var_statuses {
                let is_ok = vs.error.is_none();
                let fg = if is_ok {
                    Tokens::STATUS_SUCCESS
                } else {
                    Tokens::TEXT_DANGER
                };
                let preview = if let Some(ref mv) = vs.masked_value {
                    let short: String = if mv.chars().count() > 28 {
                        let mut s: String = mv.chars().take(27).collect();
                        s.push('…');
                        s
                    } else {
                        mv.clone()
                    };
                    format!("{{{{{}}}}} = {short}", vs.name)
                } else {
                    format!("⚠ {{{{{}}}}} · Click to set value", vs.name)
                };
                let pill_btn = ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(preview)
                                .monospace()
                                .size(10.5)
                                .color(fg),
                        )
                        .fill(swaggen_ui_kit::subtle_bg(ui))
                        .stroke(egui::Stroke::new(
                            1.0,
                            if is_ok {
                                swaggen_ui_kit::border_color(ui)
                            } else {
                                Tokens::TEXT_DANGER
                            },
                        ))
                        .corner_radius(4),
                    )
                    .on_hover_text(match (&vs.resolved_value, &vs.source, &vs.error) {
                        (Some(_), Some(src), None) => {
                            format!("Resolved from {src} — click to edit value")
                        }
                        (_, _, Some(err)) => {
                            format!("{err} — click to assign a value immediately!")
                        }
                        _ => "Click to edit variable".into(),
                    });

                if pill_btn.clicked() && app.quick_var_name != vs.name {
                    app.quick_var_name = vs.name.clone();
                    app.quick_var_value = vs.resolved_value.clone().unwrap_or_default();
                }

                let vname_clone = vs.name.clone();
                egui::Popup::menu(&pill_btn)
                    .id(egui::Id::unique(("quick_var_set_popup", &vname_clone)))
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        ui.set_min_width(250.0);
                        if app.quick_var_name != vname_clone {
                            app.quick_var_name = vname_clone.clone();
                            app.quick_var_value = vs.resolved_value.clone().unwrap_or_default();
                        }
                        ui.label(swaggen_ui_kit::bold_text(
                            &format!("{{{{{vname_clone}}}}}"),
                            12.0,
                            Tokens::STATUS_INFO,
                        ));
                        if let Some(ref src) = vs.source {
                            ui.label(
                                egui::RichText::new(format!("Current scope: {src}"))
                                    .size(10.5)
                                    .color(swaggen_ui_kit::text_secondary(ui)),
                            );
                        }
                        ui.add_space(3.0);
                        let inp = ui.add(
                            egui::TextEdit::singleline(&mut app.quick_var_value)
                                .hint_text("https://api.example.test")
                                .desired_width(236.0),
                        );
                        let enter =
                            inp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if swaggen_ui_kit::primary_button(
                                ui,
                                true,
                                &format!("Save to {active_env_name}"),
                            )
                            .clicked()
                                || enter
                            {
                                set_var_action =
                                    Some((vname_clone.clone(), app.quick_var_value.clone(), false));
                                ui.close();
                            }
                            if ui.small_button("Save to Collection").clicked() {
                                set_var_action =
                                    Some((vname_clone.clone(), app.quick_var_value.clone(), true));
                                ui.close();
                            }
                        });
                        if ui.small_button("Open Environments Manager…").clicked() {
                            open_env_screen = true;
                            ui.close();
                        }
                    });
            }
        });

        if let Some((vkey, vval, to_collection)) = set_var_action {
            if to_collection {
                if let Some(col) = app
                    .state
                    .collections
                    .iter_mut()
                    .find(|c| c.id == cur_col_id)
                {
                    if let Some(ex) = col.variables.iter_mut().find(|x| x.key == vkey) {
                        ex.shared_value = vval;
                        ex.enabled = true;
                    } else {
                        col.variables.push(VariableEntry::public(&vkey, vval));
                    }
                    app.status = format!("Saved {{{{{vkey}}}}} to Collection variables");
                }
            } else if let Some(env_id) = app.state.active_environment_id.clone()
                && let Some(env) = app.state.environments.iter_mut().find(|e| e.id == env_id)
            {
                if let Some(ex) = env.variables.iter_mut().find(|x| x.key == vkey) {
                    ex.shared_value = vval;
                    ex.enabled = true;
                } else {
                    env.variables.push(VariableEntry::public(&vkey, vval));
                }
                app.status = format!("Saved {{{{{vkey}}}}} in environment '{}'", env.name);
            } else {
                if let Some(ex) = app.state.workspace_vars.iter_mut().find(|x| x.key == vkey) {
                    ex.shared_value = vval;
                    ex.enabled = true;
                } else {
                    app.state
                        .workspace_vars
                        .push(VariableEntry::public(&vkey, vval));
                }
                app.status = format!("Saved {{{{{vkey}}}}} in Global Workspace variables");
            }
            changed = true;
        }
        if open_env_screen {
            app.navigate_to(MainMode::Environments);
            return None;
        }
    }

    ui.add_space(3.0);
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
        app.status = "Moved request to Trash".into();
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
    let (resolved, eff_auth_source) = {
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
        let eff_source = resolve_effective_auth(col.as_ref(), &fchain, &req_clone).source;
        (resolved, eff_source)
    };

    // 3. Clean Postman Sub-tabs: `Params` | `Path Variables` | `Headers` | `Body` | `Auth` | `Scripts` | `Tests` | `Settings` | `Code`
    let q_count = req_clone
        .query_params
        .iter()
        .filter(|q| !q.key.is_empty() || !q.value.is_empty())
        .count();
    let p_count = req_clone.path_params.len();
    let h_count = req_clone.headers.len();
    let a_count = req_clone.assertions.len() + req_clone.extractors.len();
    let has_scripts = !req_clone.pre_request_script.trim().is_empty()
        || !req_clone.post_response_script.trim().is_empty();

    let query_lbl = if q_count > 0 {
        format!("Params ({q_count})")
    } else {
        "Params".into()
    };
    let path_lbl = if p_count > 0 {
        format!("Path Vars ({p_count})")
    } else {
        "Path Vars".into()
    };
    let headers_lbl = if h_count > 0 {
        format!("Headers ({h_count})")
    } else {
        "Headers".into()
    };
    let scripts_lbl = if has_scripts {
        "Scripts ●"
    } else {
        "Scripts"
    };
    let tests_lbl = if a_count > 0 {
        format!("Assertions ({a_count})")
    } else {
        "Assertions".into()
    };

    let is_gql = req_clone.is_graphql();
    let allows_body = !is_gql && !matches!(req_clone.method, HttpMethod::Get | HttpMethod::Head);
    if is_gql && app.request_subtab == RequestSubTab::Body {
        app.request_subtab = RequestSubTab::GraphQl;
    } else if (!is_gql && app.request_subtab == RequestSubTab::GraphQl)
        || (!allows_body && app.request_subtab == RequestSubTab::Body)
    {
        app.request_subtab = RequestSubTab::Params;
    }

    let subtab_bar = ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let mut tabs: Vec<(RequestSubTab, &str)> = Vec::with_capacity(10);
        if is_gql {
            tabs.push((RequestSubTab::GraphQl, "GraphQL"));
        }
        tabs.push((RequestSubTab::Params, query_lbl.as_str()));
        if p_count > 0 || app.request_subtab == RequestSubTab::PathParams {
            tabs.push((RequestSubTab::PathParams, path_lbl.as_str()));
        }
        tabs.push((RequestSubTab::Headers, headers_lbl.as_str()));
        if allows_body {
            tabs.push((RequestSubTab::Body, "Body"));
        }
        tabs.extend([
            (RequestSubTab::Auth, "Auth"),
            (RequestSubTab::Scripts, scripts_lbl),
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
                    "▾"
                } else {
                    "▴"
                };
                if ui
                    .small_button(collapse_lbl)
                    .on_hover_text("Minimize or expand request composer panel")
                    .clicked()
                {
                    app.request_config_collapsed = !app.request_config_collapsed;
                }
            });
        }
    });
    swaggen_ui_kit::paint_edge_divider(ui, subtab_bar.response.rect, Edge::Bottom);

    if app.request_config_collapsed && !app.state.layout.workbench_side_by_side {
        return;
    }
    ui.add_space(6.0);

    let mut pending_worker_cmd: Option<crate::worker::Command> = None;
    let inner_scroll_h = (allocated_height - 38.0).max(36.0);
    let mut kv_ratio = app.state.layout.kv_key_ratio.clamp(0.18, 0.68);

    let suggestions: Vec<(String, String)> = app
        .responses
        .get(&req_clone.id)
        .and_then(|r| r.as_ref().ok())
        .map(|resp| infer_query_param_suggestions(&resp.preview_bytes))
        .unwrap_or_default();

    // Borderless content region so ONLY the single 1px splitter separates Request from Response!
    egui::ScrollArea::vertical()
        .id_salt("req-editor-scroll")
        .min_scrolled_height(inner_scroll_h)
        .max_height(inner_scroll_h)
        .auto_shrink([false, false])
        .show(ui, |ui| match app.request_subtab {
            RequestSubTab::GraphQl => {
                show_graphql_studio(
                    app,
                    ui,
                    req_idx,
                    &req_clone,
                    &resolved,
                    inner_scroll_h,
                    &mut changed,
                    &mut pending_worker_cmd,
                );
            }
            RequestSubTab::Params => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("Query Parameters")
                            .size(11.5)
                            .color(swaggen_ui_kit::text_secondary(ui))
                            .strong(),
                    );
                    if p_count == 0 && ui.small_button("+ Add :id Path Param").clicked() {
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
                        app.request_subtab = RequestSubTab::PathParams;
                        changed = true;
                    }
                    for (sk, sv) in suggestions.iter().take(6) {
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
                            .small_button(
                                egui::RichText::new(chip)
                                    .monospace()
                                    .size(10.0)
                                    .color(Tokens::STATUS_INFO),
                            )
                            .on_hover_text("Add detected query filter")
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
                ui.add_space(4.0);
                if edit_postman_kv_grid(
                    ui,
                    "query-params",
                    "Key",
                    "Value",
                    &mut app.state.requests[req_idx].query_params,
                    &mut kv_ratio,
                    true,
                ) {
                    app.state.requests[req_idx].sync_url_from_query_params();
                    changed = true;
                }
            }
            RequestSubTab::PathParams => {
                ui.label(
                    egui::RichText::new("Path Variables (:id or {id} in URL)")
                        .size(11.5)
                        .color(swaggen_ui_kit::text_secondary(ui))
                        .strong(),
                );
                ui.add_space(4.0);
                let prev_keys: Vec<String> = app.state.requests[req_idx]
                    .path_params
                    .iter()
                    .map(|p| p.key.clone())
                    .collect();
                let prev_len = prev_keys.len();
                if edit_postman_kv_grid(
                    ui,
                    "path-params",
                    "Path variable",
                    "Value",
                    &mut app.state.requests[req_idx].path_params,
                    &mut kv_ratio,
                    false,
                ) {
                    let req = &mut app.state.requests[req_idx];
                    if req.path_params.len() == prev_len {
                        for (old_k, row) in prev_keys.iter().zip(req.path_params.iter()) {
                            if old_k != &row.key && !row.key.trim().is_empty() {
                                req.url =
                                    rename_url_path_param_key(&req.url, old_k, row.key.trim());
                            }
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
                ui.add_space(4.0);
                if swaggen_ui_kit::secondary_button(ui, true, "+ Add :param segment").clicked() {
                    let req = &mut app.state.requests[req_idx];
                    let mut next_name = "id".to_owned();
                    let mut n = 2usize;
                    while req.path_params.iter().any(|p| p.key == next_name) {
                        next_name = format!("id_{n}");
                        n += 1;
                    }
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
                    req.sync_params_from_url();
                    changed = true;
                }
            }
            RequestSubTab::Headers => {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Request Headers")
                            .size(11.5)
                            .color(swaggen_ui_kit::text_secondary(ui))
                            .strong(),
                    );
                    let auto_count = resolved
                        .masked_headers
                        .iter()
                        .filter(|h| h.generated)
                        .count();
                    if auto_count > 0 {
                        ui.label(
                            egui::RichText::new(format!("· {auto_count} auto-generated"))
                                .size(11.0)
                                .color(Tokens::STATUS_INFO),
                        );
                    }
                });
                ui.add_space(4.0);
                if edit_postman_kv_grid(
                    ui,
                    "req-headers",
                    "Header (e.g. Accept, X-Api-Version)",
                    "Value",
                    &mut app.state.requests[req_idx].headers,
                    &mut kv_ratio,
                    true,
                ) {
                    changed = true;
                }
                let auto_headers: Vec<_> = resolved
                    .masked_headers
                    .iter()
                    .filter(|h| h.generated)
                    .collect();
                if !auto_headers.is_empty() {
                    ui.add_space(4.0);
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!(
                            "Auto-generated headers ({})",
                            auto_headers.len()
                        ))
                        .size(11.0)
                        .color(swaggen_ui_kit::text_secondary(ui)),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        for h in auto_headers {
                            ui.monospace(
                                egui::RichText::new(format!("{}: {}", h.name, h.value)).size(11.0),
                            );
                        }
                    });
                }
            }
            RequestSubTab::Body => {
                show_body_editor(
                    app,
                    ui,
                    req_idx,
                    inner_scroll_h,
                    &mut kv_ratio,
                    &mut changed,
                );
            }
            RequestSubTab::Auth => {
                show_auth_editor(
                    app,
                    ui,
                    req_idx,
                    &eff_auth_source,
                    &mut changed,
                    &mut pending_worker_cmd,
                );
            }
            RequestSubTab::Scripts => {
                show_scripts_editor(app, ui, req_idx, inner_scroll_h, &mut changed);
            }
            RequestSubTab::Assertions => {
                show_assertions_editor(app, ui, req_idx, &mut changed);
            }
            RequestSubTab::Connection => {
                show_clean_settings_editor(app, ui, req_idx, &resolved, &mut changed);
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
                    let snippet = swaggen_runner::protocols_and_tools::generate_code_snippet(
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

    if (kv_ratio - app.state.layout.kv_key_ratio).abs() > 0.002 {
        app.state.layout.kv_key_ratio = kv_ratio;
    }
    if let Some(cmd) = pending_worker_cmd {
        app.submit(cmd);
    }
    if changed {
        app.state.requests[req_idx].dirty_vs_shared = true;
        app.mark_dirty();
    }
}

#[allow(clippy::too_many_arguments)]
fn show_graphql_studio(
    app: &mut App,
    ui: &mut egui::Ui,
    req_idx: usize,
    req_clone: &RequestItem,
    resolved: &swaggen_domain::workspace::ResolvedRequest,
    inner_scroll_h: f32,
    changed: &mut bool,
    pending_cmd: &mut Option<crate::worker::Command>,
) {
    let schema_opt = app.graphql_schemas.get(&req_clone.id).cloned();
    let is_introspecting = app.graphql_introspecting_id.as_deref() == Some(req_clone.id.as_str());
    let active_idem_key = resolved
        .masked_headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("idempotency-key"))
        .map(|h| h.value.clone());
    let mut trigger_introspect = false;

    if let RequestBody::GraphQl {
        query,
        variables,
        operation_name,
    } = &mut app.state.requests[req_idx].body
    {
        let analysis = analyze_graphql_document(query, operation_name.as_deref(), 8, 120);
        let selected_op_kind = analysis.active_kind;

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            let op_label = operation_name
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("(Auto operation)");
            egui::ComboBox::from_id_salt("gql-op-select")
                .selected_text(
                    egui::RichText::new(format!("Op: {op_label}"))
                        .size(11.0)
                        .monospace(),
                )
                .width(155.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(operation_name.is_none(), "(Auto — single/first op)")
                        .clicked()
                    {
                        *operation_name = None;
                        *changed = true;
                    }
                    for op in &analysis.operations {
                        if let Some(ref name) = op.name {
                            let selected = operation_name.as_deref() == Some(name.as_str());
                            if ui
                                .selectable_label(
                                    selected,
                                    format!("{} · {}", op.kind.as_str(), name),
                                )
                                .clicked()
                            {
                                *operation_name = Some(name.clone());
                                *changed = true;
                            }
                        }
                    }
                });

            let fetch_lbl = if is_introspecting {
                "Fetching Schema…"
            } else if schema_opt.is_some() {
                "Refresh Schema"
            } else {
                "Fetch Schema"
            };
            if swaggen_ui_kit::secondary_button(ui, !is_introspecting, fetch_lbl).clicked() {
                trigger_introspect = true;
            }
            if swaggen_ui_kit::secondary_button(ui, true, "Sync $Vars").clicked() {
                let active_vars: Vec<(String, String)> = analysis
                    .operations
                    .iter()
                    .find(|op| match operation_name.as_deref() {
                        Some(want) if !want.trim().is_empty() => {
                            op.name.as_deref() == Some(want.trim())
                        }
                        _ => true,
                    })
                    .map(|op| op.variables.clone())
                    .unwrap_or_default();
                *variables = generate_variables_skeleton(&active_vars, variables);
                *changed = true;
            }
            if ui.small_button("Format JSON").clicked()
                && let Ok(val) = serde_json::from_str::<serde_json::Value>(variables)
                && let Ok(pretty) = serde_json::to_string_pretty(&val)
            {
                *variables = pretty;
                *changed = true;
            }
            swaggen_ui_kit::pill_badge(
                ui,
                swaggen_ui_kit::method_color("GQL"),
                &format!(
                    "{} · Depth {} · {} fields",
                    selected_op_kind.as_str().to_ascii_uppercase(),
                    analysis.max_depth,
                    analysis.total_fields
                ),
            );
            if !analysis.potential_n_plus_1_paths.is_empty() {
                swaggen_ui_kit::status_badge(
                    ui,
                    NoticeKind::Warning,
                    &format!("N+1 Check ({})", analysis.potential_n_plus_1_paths.len()),
                );
            }
            if active_idem_key.is_some() {
                swaggen_ui_kit::status_badge(ui, NoticeKind::Success, "Idempotent");
            }
        });
        ui.add_space(3.0);

        for err in &resolved.blocking_errors {
            swaggen_ui_kit::notice_banner(ui, NoticeKind::Danger, "Blocked", err);
            ui.add_space(2.0);
        }

        let avail_rect = ui.available_rect_before_wrap();
        let total_w = avail_rect.width().max(260.0);
        let pane_h = (inner_scroll_h - 38.0).max(96.0);
        let mut split_ratio = app.gql_split_ratio.clamp(0.32, 0.76);
        let left_w = ((total_w - 10.0) * split_ratio).clamp(130.0, (total_w - 130.0).max(130.0));
        let right_w = (total_w - left_w - 10.0).max(110.0);
        let code_rows = ((pane_h - 26.0) / 17.5).clamp(4.0, 26.0) as usize;

        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.allocate_ui_with_layout(
                egui::vec2(left_w, pane_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.label(
                        egui::RichText::new("QUERY / MUTATION")
                            .size(10.0)
                            .color(swaggen_ui_kit::text_secondary(ui))
                            .strong(),
                    );
                    if ui
                        .add(
                            egui::TextEdit::multiline(query)
                                .id_salt("gql_query_editor")
                                .hint_text("query Example($id: ID!) {\n  ...\n}")
                                .desired_width(left_w)
                                .desired_rows(code_rows)
                                .code_editor(),
                        )
                        .changed()
                    {
                        *changed = true;
                    }
                },
            );

            let (split_resp, _) =
                swaggen_ui_kit::vertical_splitter(ui, "gql_studio_vsplit", pane_h);
            if split_resp.dragged()
                && let Some(pos) = ui.ctx().pointer_interact_pos()
            {
                split_ratio = ((pos.x - avail_rect.left()) / total_w).clamp(0.32, 0.76);
                app.gql_split_ratio = split_ratio;
            }
            if split_resp.double_clicked() {
                app.gql_split_ratio = 0.60;
            }

            ui.allocate_ui_with_layout(
                egui::vec2(right_w, pane_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.label(
                        egui::RichText::new("VARIABLES (JSON)")
                            .size(10.0)
                            .color(swaggen_ui_kit::text_secondary(ui))
                            .strong(),
                    );
                    let var_rows = if schema_opt.is_some() {
                        (code_rows / 2).max(3)
                    } else {
                        code_rows
                    };
                    if ui
                        .add(
                            egui::TextEdit::multiline(variables)
                                .id_salt("gql_vars_editor")
                                .hint_text("{\n  \"id\": \"1\"\n}")
                                .desired_width(right_w)
                                .desired_rows(var_rows)
                                .code_editor(),
                        )
                        .changed()
                    {
                        *changed = true;
                    }
                    if let Some(ref schema) = schema_opt {
                        ui.add_space(4.0);
                        egui::CollapsingHeader::new(
                            egui::RichText::new(format!(
                                "Schema ({} queries · {} mutations)",
                                schema.query_fields.len(),
                                schema.mutation_fields.len()
                            ))
                            .size(10.5)
                            .color(Tokens::STATUS_INFO),
                        )
                        .default_open(true)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("gql_schema_explorer_scroll")
                                .max_height(110.0)
                                .show(ui, |ui| {
                                    for (kind, list) in [
                                        (GraphQlOperationKind::Query, &schema.query_fields),
                                        (GraphQlOperationKind::Mutation, &schema.mutation_fields),
                                    ] {
                                        for f in list {
                                            ui.horizontal_wrapped(|ui| {
                                                if ui.small_button("Use").clicked() {
                                                    let (tpl_q, tpl_v) =
                                                        build_operation_template(kind, f, schema);
                                                    *query = tpl_q;
                                                    *variables = tpl_v;
                                                    *operation_name = None;
                                                    *changed = true;
                                                }
                                                ui.monospace(format!(
                                                    "{} {}: {}",
                                                    kind.as_str(),
                                                    f.name,
                                                    f.return_type
                                                ));
                                            });
                                        }
                                    }
                                });
                        });
                    }
                },
            );
        });
    }

    if trigger_introspect {
        app.graphql_introspecting_id = Some(req_clone.id.clone());
        *pending_cmd = Some(crate::worker::Command::IntrospectGraphQl {
            request_id: req_clone.id.clone(),
            request: Box::new(app.state.requests[req_idx].clone()),
            state: Box::new(app.state.clone()),
        });
    }
}

fn show_body_editor(
    app: &mut App,
    ui: &mut egui::Ui,
    req_idx: usize,
    inner_scroll_h: f32,
    kv_ratio: &mut f32,
    changed: &mut bool,
) {
    let mut kind_idx = match &app.state.requests[req_idx].body {
        RequestBody::None => 0,
        RequestBody::Json(_) => 1,
        RequestBody::GraphQl { .. } => 2,
        RequestBody::Text(_) => 3,
        RequestBody::Xml(_) => 4,
        RequestBody::FormUrlEncoded(_) => 5,
        RequestBody::Multipart(_) => 6,
        RequestBody::Binary { .. } => 7,
    };
    ui.horizontal_wrapped(|ui| {
        for (idx, lbl) in [
            (0, "none"),
            (1, "JSON"),
            (5, "x-www-form-urlencoded"),
            (6, "form-data"),
            (3, "raw text"),
            (4, "XML"),
            (7, "binary"),
            (2, "GraphQL"),
        ] {
            if ui.selectable_label(kind_idx == idx, lbl).clicked() && kind_idx != idx {
                kind_idx = idx;
                app.state.requests[req_idx].body = match idx {
                    0 => RequestBody::None,
                    1 => RequestBody::Json("{\n  \n}".into()),
                    2 => RequestBody::GraphQl {
                        query: "query ExampleQuery {\n  __typename\n}".into(),
                        variables: "{\n  \n}".into(),
                        operation_name: None,
                    },
                    3 => RequestBody::Text(String::new()),
                    4 => RequestBody::Xml("<root/>".into()),
                    5 => RequestBody::FormUrlEncoded(Vec::new()),
                    6 => RequestBody::Multipart(Vec::new()),
                    _ => RequestBody::Binary {
                        local_path: None,
                        content_type: "application/octet-stream".into(),
                    },
                };
                if idx == 2 {
                    app.request_subtab = RequestSubTab::GraphQl;
                }
                *changed = true;
            }
        }
    });
    ui.add_space(4.0);
    match &mut app.state.requests[req_idx].body {
        RequestBody::None => {
            ui.label(
                egui::RichText::new("This request does not have a body.")
                    .size(11.5)
                    .color(swaggen_ui_kit::text_secondary(ui)),
            );
        }
        RequestBody::GraphQl { .. } => {
            ui.label("Edit GraphQL query & variables in the GraphQL tab.");
        }
        RequestBody::Json(text) => {
            let body_rows = ((inner_scroll_h - 56.0) / 17.5).clamp(4.0, 26.0) as usize;
            ui.horizontal(|ui| {
                if ui.small_button("Beautify JSON").clicked()
                    && let Ok(val) = serde_json::from_str::<serde_json::Value>(text)
                    && let Ok(pretty) = serde_json::to_string_pretty(&val)
                {
                    *text = pretty;
                    *changed = true;
                }
                if !text.trim().is_empty()
                    && !text.contains("{{")
                    && let Err(err) = serde_json::from_str::<serde_json::Value>(text)
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
                        .desired_rows(body_rows)
                        .code_editor(),
                )
                .changed()
            {
                *changed = true;
            }
        }
        RequestBody::Text(text) | RequestBody::Xml(text) => {
            let body_rows = ((inner_scroll_h - 40.0) / 17.5).clamp(4.0, 26.0) as usize;
            if ui
                .add(
                    egui::TextEdit::multiline(text)
                        .desired_width(ui.available_width())
                        .desired_rows(body_rows)
                        .code_editor(),
                )
                .changed()
            {
                *changed = true;
            }
        }
        RequestBody::FormUrlEncoded(rows) => {
            if edit_postman_kv_grid(ui, "form-body", "Field", "Value", rows, kv_ratio, true) {
                *changed = true;
            }
        }
        RequestBody::Multipart(fields) => {
            let mut del_idx = None;
            for (i, f) in fields.iter_mut().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    if ui.checkbox(&mut f.enabled, "").changed() {
                        *changed = true;
                    }
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut f.name)
                                .hint_text("field")
                                .desired_width(120.0),
                        )
                        .changed()
                    {
                        *changed = true;
                    }
                    match &mut f.value {
                        MultipartValueKind::Text(v) => {
                            if ui
                                .add(
                                    egui::TextEdit::singleline(v)
                                        .hint_text("value")
                                        .desired_width(220.0),
                                )
                                .changed()
                            {
                                *changed = true;
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
                                        .desired_width(220.0),
                                )
                                .changed()
                            {
                                *file_name = std::path::Path::new(local_path)
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| "upload.bin".into());
                                *changed = true;
                            }
                            if !local_path.is_empty() && !std::path::Path::new(local_path).is_file()
                            {
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
                *changed = true;
            }
            ui.horizontal(|ui| {
                if swaggen_ui_kit::secondary_button(ui, true, "+ Text field").clicked() {
                    fields.push(MultipartField {
                        name: String::new(),
                        value: MultipartValueKind::Text(String::new()),
                        enabled: true,
                    });
                    *changed = true;
                }
                if swaggen_ui_kit::secondary_button(ui, true, "+ File field").clicked() {
                    fields.push(MultipartField {
                        name: "file".into(),
                        value: MultipartValueKind::File {
                            local_path: String::new(),
                            file_name: "upload.bin".into(),
                            content_type: "application/octet-stream".into(),
                        },
                        enabled: true,
                    });
                    *changed = true;
                }
            });
        }
        RequestBody::Binary {
            local_path,
            content_type,
        } => {
            let mut p = local_path.clone().unwrap_or_default();
            ui.horizontal_wrapped(|ui| {
                ui.label("Local file:");
                if ui
                    .add(egui::TextEdit::singleline(&mut p).desired_width(260.0))
                    .changed()
                {
                    *local_path = if p.trim().is_empty() { None } else { Some(p) };
                    *changed = true;
                }
                ui.label("Content-Type:");
                if ui
                    .add(egui::TextEdit::singleline(content_type).desired_width(170.0))
                    .changed()
                {
                    *changed = true;
                }
            });
        }
    }
}

fn show_auth_editor(
    app: &mut App,
    ui: &mut egui::Ui,
    req_idx: usize,
    eff_auth_source: &str,
    changed: &mut bool,
    pending_cmd: &mut Option<crate::worker::Command>,
) {
    let mut auth_idx = match &app.state.requests[req_idx].auth {
        AuthConfig::Inherit => 0,
        AuthConfig::NoAuth => 1,
        AuthConfig::Bearer { .. } => 2,
        AuthConfig::Basic { .. } => 3,
        AuthConfig::ApiKey { .. } => 4,
        AuthConfig::OAuth2 { .. } => 5,
        AuthConfig::AwsSigV4 { .. } => 6,
        AuthConfig::Digest { .. } => 7,
    };
    let auth_names = [
        "Inherit auth from parent",
        "No Auth",
        "Bearer Token",
        "Basic Auth",
        "API Key",
        "OAuth 2.0",
        "AWS Signature v4",
        "Digest Auth (SHA-256)",
    ];
    ui.horizontal_wrapped(|ui| {
        ui.label(swaggen_ui_kit::bold_text(
            "Auth Type:",
            12.0,
            swaggen_ui_kit::text_primary(ui),
        ));
        egui::ComboBox::from_id_salt("req_auth_type_combo")
            .selected_text(auth_names[auth_idx])
            .width(205.0)
            .show_ui(ui, |ui| {
                for (idx, name) in auth_names.iter().enumerate() {
                    if ui.selectable_value(&mut auth_idx, idx, *name).changed() {
                        app.state.requests[req_idx].auth = match idx {
                            0 => AuthConfig::Inherit,
                            1 => AuthConfig::NoAuth,
                            2 => AuthConfig::Bearer {
                                token: String::new(),
                            },
                            3 => AuthConfig::Basic {
                                username: String::new(),
                                password: String::new(),
                            },
                            4 => AuthConfig::ApiKey {
                                key: "X-Api-Key".into(),
                                value: String::new(),
                                placement: ApiKeyPlacement::Header,
                            },
                            5 => AuthConfig::OAuth2 {
                                grant_type: OAuthGrantType::AuthorizationCodePkce,
                                auth_url: app.oauth_auth_url.clone(),
                                token_url: app.oauth_token_url.clone(),
                                client_id: app.oauth_client_id.clone(),
                                client_secret: String::new(),
                                scope: app.oauth_scope.clone(),
                                access_token: String::new(),
                            },
                            6 => AuthConfig::AwsSigV4 {
                                access_key: String::new(),
                                secret_key: String::new(),
                                region: "us-east-1".into(),
                                service: "execute-api".into(),
                                session_token: String::new(),
                            },
                            _ => AuthConfig::Digest {
                                username: String::new(),
                                password: String::new(),
                                realm: "api".into(),
                                nonce: "nonce01".into(),
                            },
                        };
                        *changed = true;
                    }
                }
            });
        ui.label(
            egui::RichText::new(format!("Effective: {eff_auth_source}"))
                .size(11.0)
                .color(swaggen_ui_kit::text_secondary(ui)),
        );
    });
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);

    match &mut app.state.requests[req_idx].auth {
        AuthConfig::Inherit => {
            ui.label(
                egui::RichText::new(
                    "This request inherits authorization from its parent Folder or Collection.",
                )
                .color(swaggen_ui_kit::text_secondary(ui)),
            );
        }
        AuthConfig::NoAuth => {
            ui.label(
                egui::RichText::new("This request does not send an Authorization header.")
                    .color(swaggen_ui_kit::text_secondary(ui)),
            );
        }
        AuthConfig::Bearer { token } => {
            setting_row(
                ui,
                "Token",
                "Paste a raw JWT/token or reference an environment variable like {{api_token}}",
                |ui| {
                    if swaggen_ui_kit::text_input(
                        ui,
                        token,
                        "Token or {{api_token}}",
                        (ui.available_width() - 10.0).clamp(180.0, 380.0),
                        false,
                    )
                    .changed()
                    {
                        *changed = true;
                    }
                },
            );
        }
        AuthConfig::Basic { username, password } => {
            setting_row(ui, "Username", "HTTP Basic username or {{var}}", |ui| {
                if swaggen_ui_kit::text_input(ui, username, "username", 260.0, false).changed() {
                    *changed = true;
                }
            });
            setting_row(ui, "Password", "HTTP Basic password or {{var}}", |ui| {
                if swaggen_ui_kit::text_input(ui, password, "password", 260.0, true).changed() {
                    *changed = true;
                }
            });
        }
        AuthConfig::ApiKey {
            key,
            value,
            placement,
        } => {
            setting_row(
                ui,
                "Header / Param Key",
                "e.g. X-Api-Key or api_key",
                |ui| {
                    if swaggen_ui_kit::text_input(ui, key, "X-Api-Key", 240.0, false).changed() {
                        *changed = true;
                    }
                },
            );
            setting_row(ui, "Value", "Key value or {{api_key}}", |ui| {
                if swaggen_ui_kit::text_input(ui, value, "Value or {{api_key}}", 260.0, false)
                    .changed()
                {
                    *changed = true;
                }
            });
            setting_row(
                ui,
                "Add to",
                "Send in HTTP Header or URL Query String",
                |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(*placement == ApiKeyPlacement::Header, "Header")
                            .clicked()
                        {
                            *placement = ApiKeyPlacement::Header;
                            *changed = true;
                        }
                        if ui
                            .selectable_label(*placement == ApiKeyPlacement::Query, "Query Params")
                            .clicked()
                        {
                            *placement = ApiKeyPlacement::Query;
                            *changed = true;
                        }
                    });
                },
            );
        }
        AuthConfig::OAuth2 {
            grant_type,
            auth_url,
            token_url,
            client_id,
            client_secret,
            scope,
            access_token,
        } => {
            setting_row(
                ui,
                "Grant Type",
                "Authorization Code with PKCE (browser) or Client Credentials",
                |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(
                                *grant_type == OAuthGrantType::AuthorizationCodePkce,
                                "Auth Code (PKCE)",
                            )
                            .clicked()
                        {
                            *grant_type = OAuthGrantType::AuthorizationCodePkce;
                            *changed = true;
                        }
                        if ui
                            .selectable_label(
                                *grant_type == OAuthGrantType::ClientCredentials,
                                "Client Credentials",
                            )
                            .clicked()
                        {
                            *grant_type = OAuthGrantType::ClientCredentials;
                            *changed = true;
                        }
                    });
                },
            );
            if *grant_type == OAuthGrantType::AuthorizationCodePkce {
                setting_row(ui, "Auth URL", "Authorization endpoint", |ui| {
                    if swaggen_ui_kit::text_input(
                        ui,
                        auth_url,
                        "https://.../authorize",
                        300.0,
                        false,
                    )
                    .changed()
                    {
                        *changed = true;
                    }
                });
            }
            setting_row(ui, "Access Token URL", "Token exchange endpoint", |ui| {
                if swaggen_ui_kit::text_input(
                    ui,
                    token_url,
                    "https://.../oauth/token",
                    300.0,
                    false,
                )
                .changed()
                {
                    *changed = true;
                }
            });
            setting_row(ui, "Client ID", "OAuth application Client ID", |ui| {
                if swaggen_ui_kit::text_input(ui, client_id, "client_id", 240.0, false).changed() {
                    *changed = true;
                }
            });
            if *grant_type == OAuthGrantType::ClientCredentials {
                setting_row(
                    ui,
                    "Client Secret",
                    "OAuth Client Secret or {{var}}",
                    |ui| {
                        if swaggen_ui_kit::text_input(
                            ui,
                            client_secret,
                            "client_secret",
                            240.0,
                            true,
                        )
                        .changed()
                        {
                            *changed = true;
                        }
                    },
                );
            }
            setting_row(ui, "Scope", "Space-separated scopes", |ui| {
                if swaggen_ui_kit::text_input(ui, scope, "openid profile", 240.0, false).changed() {
                    *changed = true;
                }
            });
            let a_url = auth_url.clone();
            let t_url = token_url.clone();
            let cid = client_id.clone();
            let csec = client_secret.clone();
            let scp = scope.clone();
            let gt = *grant_type;
            setting_row(
                ui,
                "Current Access Token",
                "Attached as Authorization: Bearer <token>",
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if swaggen_ui_kit::text_input(
                            ui,
                            access_token,
                            "Bearer access token",
                            230.0,
                            false,
                        )
                        .changed()
                        {
                            *changed = true;
                        }
                        if swaggen_ui_kit::primary_button(
                            ui,
                            !t_url.trim().is_empty(),
                            "Get New Access Token",
                        )
                        .clicked()
                        {
                            *pending_cmd = Some(match gt {
                                OAuthGrantType::AuthorizationCodePkce => {
                                    crate::worker::Command::RunOAuth2Pkce {
                                        auth_url: a_url,
                                        token_url: t_url,
                                        client_id: cid,
                                        scope: scp,
                                        downloads_dir: app.root.join("downloads"),
                                    }
                                }
                                OAuthGrantType::ClientCredentials => {
                                    crate::worker::Command::RunOAuth2ClientCredentials {
                                        token_url: t_url,
                                        client_id: cid,
                                        client_secret: csec,
                                        scope: scp,
                                        downloads_dir: app.root.join("downloads"),
                                    }
                                }
                            });
                        }
                    });
                },
            );
        }
        AuthConfig::AwsSigV4 {
            access_key,
            secret_key,
            region,
            service,
            session_token,
        } => {
            setting_row(
                ui,
                "AccessKey",
                "AWS Access Key ID or {{aws_access_key}}",
                |ui| {
                    if swaggen_ui_kit::text_input(ui, access_key, "AKIA...", 250.0, false).changed()
                    {
                        *changed = true;
                    }
                },
            );
            setting_row(
                ui,
                "SecretKey",
                "AWS Secret Access Key or {{aws_secret_key}}",
                |ui| {
                    if swaggen_ui_kit::text_input(ui, secret_key, "Secret key", 250.0, true)
                        .changed()
                    {
                        *changed = true;
                    }
                },
            );
            setting_row(
                ui,
                "AWS Region & Service",
                "e.g. us-east-1 / s3 or execute-api",
                |ui| {
                    ui.horizontal(|ui| {
                        if swaggen_ui_kit::text_input(ui, region, "us-east-1", 120.0, false)
                            .changed()
                        {
                            *changed = true;
                        }
                        if swaggen_ui_kit::text_input(ui, service, "execute-api", 120.0, false)
                            .changed()
                        {
                            *changed = true;
                        }
                    });
                },
            );
            setting_row(
                ui,
                "Session Token",
                "Optional STS X-Amz-Security-Token",
                |ui| {
                    if swaggen_ui_kit::text_input(ui, session_token, "(optional)", 250.0, false)
                        .changed()
                    {
                        *changed = true;
                    }
                },
            );
        }
        AuthConfig::Digest {
            username,
            password,
            realm,
            nonce,
        } => {
            setting_row(
                ui,
                "Username & Password",
                "RFC 7616 SHA-256 Digest credentials",
                |ui| {
                    ui.horizontal(|ui| {
                        if swaggen_ui_kit::text_input(ui, username, "username", 125.0, false)
                            .changed()
                        {
                            *changed = true;
                        }
                        if swaggen_ui_kit::text_input(ui, password, "password", 125.0, true)
                            .changed()
                        {
                            *changed = true;
                        }
                    });
                },
            );
            setting_row(
                ui,
                "Realm & Nonce",
                "Server WWW-Authenticate realm and nonce",
                |ui| {
                    ui.horizontal(|ui| {
                        if swaggen_ui_kit::text_input(ui, realm, "realm", 125.0, false).changed() {
                            *changed = true;
                        }
                        if swaggen_ui_kit::text_input(ui, nonce, "nonce", 125.0, false).changed() {
                            *changed = true;
                        }
                    });
                },
            );
        }
    }
}

/// First-class Pre-request & Post-response JS Automation Editor (F9 — Postman `pm.*` + Playwright `expect()`).
fn show_scripts_editor(
    app: &mut App,
    ui: &mut egui::Ui,
    req_idx: usize,
    inner_scroll_h: f32,
    changed: &mut bool,
) {
    let req_id = app.state.requests[req_idx].id.clone();
    let pre_has = !app.state.requests[req_idx]
        .pre_request_script
        .trim()
        .is_empty();
    let post_has = !app.state.requests[req_idx]
        .post_response_script
        .trim()
        .is_empty();

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let pre_lbl = if pre_has {
            "Pre-request ●"
        } else {
            "Pre-request"
        };
        let post_lbl = if post_has {
            "Post-response (Tests) ●"
        } else {
            "Post-response (Tests)"
        };
        if ui
            .selectable_label(!app.script_subtab_post, pre_lbl)
            .on_hover_text(
                "Runs BEFORE {{variables}} resolve — generate nonces, timestamps, headers, or env vars",
            )
            .clicked()
        {
            app.script_subtab_post = false;
        }
        if ui
            .selectable_label(app.script_subtab_post, post_lbl)
            .on_hover_text(
                "Runs AFTER HTTP response arrives — assert status/JSON with pm.test() or Playwright expect(), and chain tokens into env!",
            )
            .clicked()
        {
            app.script_subtab_post = true;
        }

        let active_code = if app.script_subtab_post {
            app.state.requests[req_idx].post_response_script.clone()
        } else {
            app.state.requests[req_idx].pre_request_script.clone()
        };
        if !active_code.trim().is_empty() {
            let compat = swaggen_runner::sandbox::analyze_postman_script(&active_code);
            if compat.unsupported_apis.is_empty() {
                swaggen_ui_kit::pill_badge(ui, Tokens::STATUS_SUCCESS, "Sandbox Ready");
            } else {
                swaggen_ui_kit::pill_badge(
                    ui,
                    Tokens::TEXT_WARNING,
                    &format!("{} migration note(s)", compat.unsupported_apis.len()),
                );
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if swaggen_ui_kit::secondary_button(ui, !active_code.trim().is_empty(), "▶ Dry-Run Script")
                .on_hover_text("Test this script locally against the latest response & active environment")
                .clicked()
            {
                let resp_opt = app
                    .responses
                    .get(&req_id)
                    .and_then(|r| r.as_ref().ok());
                let env_map = app
                    .state
                    .active_environment()
                    .map(|e| {
                        e.variables
                            .iter()
                            .filter(|v| v.enabled)
                            .map(|v| (v.key.clone(), v.shared_value.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                let ctx = swaggen_runner::sandbox::ScriptContextSnapshot {
                    env_vars: env_map,
                    request_method: app.state.requests[req_idx].method.as_str().into(),
                    request_url: app.state.requests[req_idx].url.clone(),
                    response_status: resp_opt.map(|r| r.status_code).unwrap_or(200),
                    response_time_ms: resp_opt.map(|r| r.duration_ms).unwrap_or(12),
                    response_body: resp_opt
                        .map(|r| String::from_utf8_lossy(&r.preview_bytes).into_owned())
                        .unwrap_or_else(|| "{\"ok\":true}".into()),
                    response_headers: resp_opt.map(|r| r.headers.clone()).unwrap_or_default(),
                    ..swaggen_runner::sandbox::ScriptContextSnapshot::default()
                };
                let rep = swaggen_runner::sandbox::execute_sandboxed_script(
                    &active_code,
                    &ctx,
                    swaggen_runner::sandbox::SandboxLimits::default(),
                );
                let col_id = app.state.requests[req_idx].collection_id.clone();
                crate::worker::apply_merged_mutations(
                    &mut app.state,
                    &col_id,
                    &rep.mutated_env,
                    &rep.mutated_collection_vars,
                    &rep.mutated_globals,
                );
                let entry = app
                    .script_reports
                    .entry(req_id.clone())
                    .or_insert((None, None));
                if app.script_subtab_post {
                    entry.1 = Some(rep.clone());
                } else {
                    entry.0 = Some(rep.clone());
                }
                app.status = match rep.error {
                    Some(ref e) => format!("Script error: {e}"),
                    None => format!(
                        "Script dry-run OK ({} test(s), {} log(s))",
                        rep.test_results.len(),
                        rep.logs.len()
                    ),
                };
                *changed = true;
            }
        });
    });
    ui.add_space(4.0);

    let total_w = ui.available_width().max(260.0);
    let wide = total_w >= 620.0;
    let code_rows = ((inner_scroll_h - 46.0) / 17.5).clamp(5.0, 22.0) as usize;

    let snippets: &[(&str, &str)] = if app.script_subtab_post {
        &[
            (
                "+ Playwright: status 200 & fast",
                "test(\"Status is 200 and fast\", () => {\n  expect(response.status()).toBe(200);\n  expect(response.responseTime).toBeLessThan(1000);\n});\n",
            ),
            (
                "+ Postman: status 200",
                "pm.test(\"Status code is 200\", () => {\n  pm.response.to.have.status(200);\n});\n",
            ),
            (
                "+ Save JSON token -> Env",
                "const data = pm.response.json();\nif (data.token) {\n  pm.environment.set(\"api_token\", data.token);\n}\n",
            ),
            (
                "+ Save first item id -> Collection",
                "const items = pm.response.json();\nif (items.length > 0) {\n  pm.collectionVariables.set(\"item_id\", items[0].id);\n}\n",
            ),
            (
                "+ Assert JSON property",
                "const body = response.json();\ntest(\"Has expected field\", () => {\n  expect(body).toHaveProperty(\"id\");\n});\n",
            ),
        ]
    } else {
        &[
            (
                "+ Set Env Variable",
                "pm.environment.set(\"request_timestamp\", Date.now());\n",
            ),
            (
                "+ Inject X-Request-Id UUID",
                "const id = crypto.randomUUID();\npm.request.headers.upsert(\"X-Request-Id\", id);\npm.variables.set(\"trace_id\", id);\n",
            ),
            (
                "+ Set Collection Variable",
                "pm.collectionVariables.set(\"api_version\", \"v1\");\n",
            ),
            (
                "+ Basic Auth Header (btoa)",
                "const token = btoa(\"admin:secret\");\npm.request.headers.upsert(\"Authorization\", `Basic ${token}`);\n",
            ),
        ]
    };

    if wide {
        let snippet_w = 215.0;
        let editor_w = (total_w - snippet_w - 14.0).max(200.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(editor_w, inner_scroll_h - 34.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let target = if app.script_subtab_post {
                        &mut app.state.requests[req_idx].post_response_script
                    } else {
                        &mut app.state.requests[req_idx].pre_request_script
                    };
                    if ui
                        .add(
                            egui::TextEdit::multiline(target)
                                .id_salt(("req_js_editor", app.script_subtab_post))
                                .hint_text(if app.script_subtab_post {
                                    "// JavaScript (Postman pm.* & Playwright test/expect)\ntest(\"Status 200\", () => {\n  expect(response.status()).toBe(200);\n});"
                                } else {
                                    "// Pre-request JavaScript\npm.environment.set(\"timestamp\", Date.now());"
                                })
                                .desired_width(editor_w)
                                .desired_rows(code_rows)
                                .code_editor(),
                        )
                        .changed()
                    {
                        *changed = true;
                    }
                },
            );
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(
                egui::vec2(snippet_w, inner_scroll_h - 34.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.label(
                        egui::RichText::new("SNIPPETS (1-CLICK)")
                            .size(10.0)
                            .color(swaggen_ui_kit::text_secondary(ui))
                            .strong(),
                    );
                    ui.add_space(3.0);
                    for (label, code) in snippets {
                        if ui
                            .add_sized(
                                [snippet_w, 24.0],
                                egui::Button::new(
                                    egui::RichText::new(*label)
                                        .size(11.0)
                                        .color(Tokens::STATUS_INFO),
                                )
                                .fill(swaggen_ui_kit::subtle_bg(ui))
                                .stroke(egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)))
                                .corner_radius(4),
                            )
                            .clicked()
                        {
                            let target = if app.script_subtab_post {
                                &mut app.state.requests[req_idx].post_response_script
                            } else {
                                &mut app.state.requests[req_idx].pre_request_script
                            };
                            if !target.is_empty() && !target.ends_with('\n') {
                                target.push('\n');
                            }
                            target.push_str(code);
                            *changed = true;
                        }
                    }
                },
            );
        });
    } else {
        ui.horizontal_wrapped(|ui| {
            for (label, code) in snippets {
                if ui.small_button(*label).clicked() {
                    let target = if app.script_subtab_post {
                        &mut app.state.requests[req_idx].post_response_script
                    } else {
                        &mut app.state.requests[req_idx].pre_request_script
                    };
                    if !target.is_empty() && !target.ends_with('\n') {
                        target.push('\n');
                    }
                    target.push_str(code);
                    *changed = true;
                }
            }
        });
        let target = if app.script_subtab_post {
            &mut app.state.requests[req_idx].post_response_script
        } else {
            &mut app.state.requests[req_idx].pre_request_script
        };
        if ui
            .add(
                egui::TextEdit::multiline(target)
                    .id_salt(("req_js_editor_narrow", app.script_subtab_post))
                    .desired_width(ui.available_width())
                    .desired_rows(code_rows)
                    .code_editor(),
            )
            .changed()
        {
            *changed = true;
        }
    }
}

fn show_assertions_editor(app: &mut App, ui: &mut egui::Ui, req_idx: usize, changed: &mut bool) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("Declarative Assertions & Variable Extractors")
                .size(11.5)
                .color(swaggen_ui_kit::text_secondary(ui))
                .strong(),
        );
        if ui.small_button("Open JS Scripts tab →").clicked() {
            app.request_subtab = RequestSubTab::Scripts;
            app.script_subtab_post = true;
        }
    });
    ui.add_space(4.0);
    let rules = &mut app.state.requests[req_idx].assertions;
    let mut del_idx = None;
    for (i, r) in rules.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            if ui.checkbox(&mut r.enabled, "").changed() {
                *changed = true;
            }
            if ui
                .add(
                    egui::TextEdit::singleline(&mut r.label)
                        .hint_text("label")
                        .desired_width(110.0),
                )
                .changed()
            {
                *changed = true;
            }
            let tname = match &r.target {
                AssertionTarget::StatusEquals => "Status ==",
                AssertionTarget::MaxDurationMs => "Latency <=",
                AssertionTarget::HeaderEquals { .. } => "Header ==",
                AssertionTarget::JsonPathEquals { .. } => "JSONPath ==",
                AssertionTarget::BodyContains => "Body contains",
            };
            egui::ComboBox::from_id_salt(("assert-kind", i))
                .selected_text(tname)
                .width(105.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(
                            matches!(r.target, AssertionTarget::StatusEquals),
                            "Status ==",
                        )
                        .clicked()
                    {
                        r.target = AssertionTarget::StatusEquals;
                        *changed = true;
                    }
                    if ui
                        .selectable_label(
                            matches!(r.target, AssertionTarget::MaxDurationMs),
                            "Latency <=",
                        )
                        .clicked()
                    {
                        r.target = AssertionTarget::MaxDurationMs;
                        *changed = true;
                    }
                    if ui
                        .selectable_label(
                            matches!(r.target, AssertionTarget::JsonPathEquals { .. }),
                            "JSONPath ==",
                        )
                        .clicked()
                    {
                        r.target = AssertionTarget::JsonPathEquals {
                            path: "$.ok".into(),
                        };
                        *changed = true;
                    }
                    if ui
                        .selectable_label(
                            matches!(r.target, AssertionTarget::HeaderEquals { .. }),
                            "Header ==",
                        )
                        .clicked()
                    {
                        r.target = AssertionTarget::HeaderEquals {
                            name: "content-type".into(),
                        };
                        *changed = true;
                    }
                    if ui
                        .selectable_label(
                            matches!(r.target, AssertionTarget::BodyContains),
                            "Body contains",
                        )
                        .clicked()
                    {
                        r.target = AssertionTarget::BodyContains;
                        *changed = true;
                    }
                });
            match &mut r.target {
                AssertionTarget::JsonPathEquals { path }
                | AssertionTarget::HeaderEquals { name: path } => {
                    if ui
                        .add(egui::TextEdit::singleline(path).desired_width(110.0))
                        .changed()
                    {
                        *changed = true;
                    }
                }
                _ => {}
            }
            if ui
                .add(
                    egui::TextEdit::singleline(&mut r.expected)
                        .hint_text("expected")
                        .desired_width(120.0),
                )
                .changed()
            {
                *changed = true;
            }
            if ui.small_button("×").clicked() {
                del_idx = Some(i);
            }
        });
    }
    if let Some(i) = del_idx {
        rules.remove(i);
        *changed = true;
    }
    ui.horizontal(|ui| {
        if swaggen_ui_kit::secondary_button(ui, true, "+ Add Assertion").clicked() {
            app.state.requests[req_idx].assertions.push(AssertionRule {
                id: new_uid(),
                label: "Status 200".into(),
                target: AssertionTarget::StatusEquals,
                expected: "200".into(),
                enabled: true,
            });
            *changed = true;
        }
        if swaggen_ui_kit::secondary_button(ui, true, "+ Add Variable Extractor").clicked() {
            app.state.requests[req_idx].extractors.push(
                swaggen_domain::dynamic_vars::ResponseExtractorRule {
                    id: new_uid(),
                    target_var: "extracted_id".into(),
                    source: swaggen_domain::dynamic_vars::ExtractorSource::BodyJsonPath,
                    expression: "$.id".into(),
                    mark_secret: false,
                },
            );
            *changed = true;
        }
    });
    let extractors = &mut app.state.requests[req_idx].extractors;
    let mut rm_ext = None;
    for (i, ex) in extractors.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            ui.label("Save to env:");
            if ui
                .add(egui::TextEdit::singleline(&mut ex.target_var).desired_width(96.0))
                .changed()
            {
                *changed = true;
            }
            if ui
                .selectable_label(
                    ex.source == swaggen_domain::dynamic_vars::ExtractorSource::BodyJsonPath,
                    "JSONPath",
                )
                .clicked()
            {
                ex.source = swaggen_domain::dynamic_vars::ExtractorSource::BodyJsonPath;
                *changed = true;
            }
            if ui
                .selectable_label(
                    ex.source == swaggen_domain::dynamic_vars::ExtractorSource::Header,
                    "Header",
                )
                .clicked()
            {
                ex.source = swaggen_domain::dynamic_vars::ExtractorSource::Header;
                *changed = true;
            }
            if ui
                .add(
                    egui::TextEdit::singleline(&mut ex.expression)
                        .hint_text("$.token")
                        .desired_width(130.0),
                )
                .changed()
            {
                *changed = true;
            }
            if ui.checkbox(&mut ex.mark_secret, "DPAPI Secret").changed() {
                *changed = true;
            }
            if ui.small_button("×").clicked() {
                rm_ext = Some(i);
            }
        });
    }
    if let Some(i) = rm_ext {
        extractors.remove(i);
        *changed = true;
    }
}

/// Clean Postman-style 2-column Request `Settings` panel (F4 — replaces crammed horizontal row).
fn show_clean_settings_editor(
    app: &mut App,
    ui: &mut egui::Ui,
    req_idx: usize,
    resolved: &swaggen_domain::workspace::ResolvedRequest,
    changed: &mut bool,
) {
    let req = &mut app.state.requests[req_idx];

    setting_row(
        ui,
        "HTTP Version",
        "Select protocol version preference for transport negotiation (PR06)",
        |ui| {
            egui::ComboBox::from_id_salt("req_http_ver_combo")
                .selected_text(req.connection.http_version.label())
                .width(215.0)
                .show_ui(ui, |ui| {
                    for v in [
                        HttpVersionPolicy::Auto,
                        HttpVersionPolicy::Http1Only,
                        HttpVersionPolicy::Http2PriorKnowledge,
                    ] {
                        if ui
                            .selectable_value(&mut req.connection.http_version, v, v.label())
                            .changed()
                        {
                            *changed = true;
                        }
                    }
                });
        },
    );

    setting_row(
        ui,
        "Request Timeout (ms)",
        "Maximum duration to wait before aborting the request (0 = 100ms min)",
        |ui| {
            if ui
                .add(
                    egui::DragValue::new(&mut req.connection.timeout_ms)
                        .range(100..=300_000)
                        .speed(100),
                )
                .changed()
            {
                *changed = true;
            }
        },
    );

    setting_row(
        ui,
        "Automatically Follow Redirects",
        "Follow HTTP 3xx Location responses up to the maximum redirect hop count",
        |ui| {
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut req.connection.follow_redirects, "Enabled")
                    .changed()
                {
                    *changed = true;
                }
                if req.connection.follow_redirects {
                    ui.label("Max hops:");
                    if ui
                        .add(egui::DragValue::new(&mut req.connection.max_redirects).range(1..=20))
                        .changed()
                    {
                        *changed = true;
                    }
                }
            });
        },
    );

    setting_row(
        ui,
        "Retain Authorization on Redirect",
        "Forward Authorization & Cookie headers when redirecting to a different host",
        |ui| {
            if ui
                .checkbox(
                    &mut req.connection.send_auth_on_cross_host_redirect,
                    "Forward across hosts",
                )
                .changed()
            {
                *changed = true;
            }
        },
    );

    let mut exc_csv = req.connection.tls_host_exceptions.join(", ");
    setting_row(
        ui,
        "SSL / TLS Host Exceptions",
        "Host-scoped self-signed cert exceptions for local dev (e.g. localhost:8443)",
        |ui| {
            if swaggen_ui_kit::text_input(ui, &mut exc_csv, "localhost:8443", 230.0, false)
                .changed()
            {
                req.connection.tls_host_exceptions = exc_csv
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
                *changed = true;
            }
        },
    );

    let mut hp = req.connection.proxy.http_proxy.clone().unwrap_or_default();
    let mut bypass_csv = req.connection.proxy.bypass_hosts.join(", ");
    setting_row(
        ui,
        "Local HTTP Proxy",
        "Optional outbound proxy URL & bypass hosts (empty = Direct from this PC)",
        |ui| {
            ui.horizontal_wrapped(|ui| {
                if swaggen_ui_kit::text_input(
                    ui,
                    &mut hp,
                    "Direct from PC (no proxy)",
                    185.0,
                    false,
                )
                .changed()
                {
                    req.connection.proxy.http_proxy =
                        if hp.trim().is_empty() { None } else { Some(hp) };
                    *changed = true;
                }
                if swaggen_ui_kit::text_input(
                    ui,
                    &mut bypass_csv,
                    "Bypass: localhost, 127.0.0.1",
                    165.0,
                    false,
                )
                .changed()
                {
                    req.connection.proxy.bypass_hosts = bypass_csv
                        .split(',')
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                        .collect();
                    *changed = true;
                }
            });
        },
    );

    let mut ca_csv = req.connection.custom_ca_pem_paths.join(", ");
    setting_row(
        ui,
        "Custom Root CA & mTLS Client Cert",
        "Optional PEM Root CA bundle or host-scoped mTLS client certificate (AU07 / AU09)",
        |ui| {
            ui.vertical(|ui| {
                if swaggen_ui_kit::text_input(
                    ui,
                    &mut ca_csv,
                    "Custom CA PEM path (optional)",
                    260.0,
                    false,
                )
                .changed()
                {
                    req.connection.custom_ca_pem_paths = ca_csv
                        .split(',')
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                        .collect();
                    *changed = true;
                }
                if req.connection.client_certs.is_empty() {
                    if ui.small_button("+ Add mTLS Client Certificate").clicked() {
                        req.connection.client_certs.push(ClientCertMapping {
                            host: "localhost".into(),
                            port: 0,
                            cert_pem_path: String::new(),
                            key_pem_path: String::new(),
                            expires_at_unix: None,
                        });
                        *changed = true;
                    }
                } else {
                    let mut rm_cert = None;
                    for (ci, cm) in req.connection.client_certs.iter_mut().enumerate() {
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut cm.host)
                                        .hint_text("host")
                                        .desired_width(90.0),
                                )
                                .changed()
                            {
                                *changed = true;
                            }
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut cm.cert_pem_path)
                                        .hint_text("cert.pem")
                                        .desired_width(115.0),
                                )
                                .changed()
                            {
                                *changed = true;
                            }
                            if ui
                                .add(
                                    egui::TextEdit::singleline(&mut cm.key_pem_path)
                                        .hint_text("key.pem")
                                        .desired_width(115.0),
                                )
                                .changed()
                            {
                                *changed = true;
                            }
                            if ui.small_button("×").clicked() {
                                rm_cert = Some(ci);
                            }
                        });
                    }
                    if let Some(i) = rm_cert {
                        req.connection.client_certs.remove(i);
                        *changed = true;
                    }
                }
            });
        },
    );

    setting_row(
        ui,
        "Idempotency-Key Protection",
        "Prevents duplicate mutations on network retry via deterministic SHA-256 Idempotency-Key",
        |ui| {
            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    for (mode, lbl) in [
                        (
                            IdempotencyMode::AutoOnMutating,
                            "Auto (POST/PATCH/Mutation)",
                        ),
                        (IdempotencyMode::Always, "Always"),
                        (IdempotencyMode::Disabled, "Disabled"),
                    ] {
                        if ui
                            .selectable_label(req.idempotency_mode == mode, lbl)
                            .clicked()
                            && req.idempotency_mode != mode
                        {
                            req.idempotency_mode = mode;
                            *changed = true;
                        }
                    }
                    if swaggen_ui_kit::secondary_button(ui, true, "Rotate Key").clicked() {
                        req.idempotency_nonce = req.idempotency_nonce.saturating_add(1);
                        *changed = true;
                    }
                });
                if req.idempotency_mode != IdempotencyMode::Disabled {
                    ui.horizontal_wrapped(|ui| {
                        if swaggen_ui_kit::text_input(
                            ui,
                            &mut req.custom_idempotency_key,
                            "Auto SHA-256 key (or custom override)",
                            230.0,
                            false,
                        )
                        .changed()
                        {
                            *changed = true;
                        }
                        if let Some(h) = resolved
                            .masked_headers
                            .iter()
                            .find(|h| h.name.eq_ignore_ascii_case("idempotency-key"))
                        {
                            ui.monospace(
                                egui::RichText::new(&h.value)
                                    .size(10.5)
                                    .color(Tokens::STATUS_SUCCESS),
                            );
                        }
                    });
                }
            });
        },
    );
}

/// Responsive 2-column Postman setting row (stacks cleanly on narrow split-screen windows).
fn setting_row(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    add_control: impl FnOnce(&mut egui::Ui),
) {
    let total_w = ui.available_width().max(240.0);
    if total_w >= 540.0 {
        let left_w = (total_w * 0.44).clamp(200.0, 360.0);
        let row_resp = ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(left_w, 34.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.label(swaggen_ui_kit::bold_text(
                        title,
                        12.0,
                        swaggen_ui_kit::text_primary(ui),
                    ));
                    ui.label(
                        egui::RichText::new(subtitle)
                            .size(10.5)
                            .color(swaggen_ui_kit::text_secondary(ui)),
                    );
                },
            );
            ui.add_space(12.0);
            ui.vertical(|ui| {
                add_control(ui);
            });
        });
        ui.add_space(4.0);
        let y = row_resp.response.rect.bottom() + 2.0;
        ui.painter().line_segment(
            [
                egui::pos2(row_resp.response.rect.left(), y),
                egui::pos2(row_resp.response.rect.left() + total_w, y),
            ],
            egui::Stroke::new(1.0, swaggen_ui_kit::border_color(ui)),
        );
        ui.add_space(6.0);
    } else {
        ui.label(swaggen_ui_kit::bold_text(
            title,
            12.0,
            swaggen_ui_kit::text_primary(ui),
        ));
        ui.label(
            egui::RichText::new(subtitle)
                .size(10.5)
                .color(swaggen_ui_kit::text_secondary(ui)),
        );
        ui.add_space(3.0);
        add_control(ui);
        ui.add_space(4.0);
        ui.separator();
    }
}

/// Clean Flat Border-Collapsed Postman Key-Value Grid (F3 — zero nested boxes!).
pub fn edit_postman_kv_grid(
    ui: &mut egui::Ui,
    id_salt: &str,
    key_hint: &str,
    val_hint: &str,
    rows: &mut Vec<KeyValueRow>,
    key_ratio: &mut f32,
    include_ghost_row: bool,
) -> bool {
    let mut changed = false;
    let mut remove_idx = None;
    let border_col = swaggen_ui_kit::border_color(ui);

    ui.push_id(id_salt, |ui| {
        let total_w = ui.available_width().max(260.0);
        let show_desc = total_w >= 540.0;
        let chk_w = 28.0;
        let act_w = 28.0;
        let usable_w = (total_w - chk_w - act_w - 8.0).max(160.0);
        let key_w = (usable_w * (*key_ratio).clamp(0.22, 0.56)).max(72.0);
        let (val_w, desc_w) = if show_desc {
            let rem = (usable_w - key_w).max(100.0);
            (rem * 0.62, rem * 0.38)
        } else {
            ((usable_w - key_w).max(72.0), 0.0)
        };

        egui::Frame::new()
            .fill(swaggen_ui_kit::surface_bg(ui))
            .stroke(egui::Stroke::new(1.0, border_col))
            .corner_radius(4)
            .inner_margin(egui::Margin::ZERO)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                // Header Row
                let hdr_left = ui.available_rect_before_wrap().left();
                let hdr = egui::Frame::new()
                    .fill(swaggen_ui_kit::subtle_bg(ui))
                    .inner_margin(egui::Margin::symmetric(0, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                            ui.allocate_exact_size(egui::vec2(chk_w, 18.0), egui::Sense::hover());
                            paint_v_hairline(ui, border_col);
                            ui.add_space(8.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2((key_w - 8.0).max(40.0), 18.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new("KEY")
                                            .size(10.0)
                                            .color(swaggen_ui_kit::text_secondary(ui))
                                            .strong(),
                                    );
                                },
                            );

                            // Draggable column divider between KEY and VALUE
                            let (_, split_r) = ui.allocate_space(egui::vec2(6.0, 18.0));
                            let split_id = egui::Id::unique(("postman_kv_col_drag", id_salt));
                            let split_resp =
                                ui.interact(split_r, split_id, egui::Sense::click_and_drag());
                            let split_active = split_resp.hovered() || split_resp.dragged();
                            if split_active {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            }
                            if split_resp.dragged()
                                && let Some(pos) = ui.ctx().pointer_interact_pos()
                            {
                                *key_ratio =
                                    ((pos.x - hdr_left - chk_w) / usable_w).clamp(0.22, 0.56);
                                ui.ctx().request_repaint();
                            }
                            ui.painter().line_segment(
                                [
                                    egui::pos2(split_r.center().x, split_r.top() - 4.0),
                                    egui::pos2(split_r.center().x, split_r.bottom() + 4.0),
                                ],
                                egui::Stroke::new(
                                    if split_active { 1.5 } else { 1.0 },
                                    if split_active {
                                        Tokens::STATUS_INFO
                                    } else {
                                        border_col
                                    },
                                ),
                            );

                            ui.add_space(6.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2((val_w - 6.0).max(40.0), 18.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new("VALUE")
                                            .size(10.0)
                                            .color(swaggen_ui_kit::text_secondary(ui))
                                            .strong(),
                                    );
                                },
                            );
                            if show_desc {
                                paint_v_hairline(ui, border_col);
                                ui.add_space(8.0);
                                ui.allocate_ui_with_layout(
                                    egui::vec2((desc_w - 8.0).max(40.0), 18.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new("DESCRIPTION")
                                                .size(10.0)
                                                .color(swaggen_ui_kit::text_secondary(ui))
                                                .strong(),
                                        );
                                    },
                                );
                            }
                            ui.allocate_exact_size(egui::vec2(act_w, 18.0), egui::Sense::hover());
                        });
                    });
                paint_h_hairline(ui, hdr.response.rect, border_col);

                // Data Rows
                for (i, row) in rows.iter_mut().enumerate() {
                    let text_col = if row.enabled {
                        swaggen_ui_kit::text_primary(ui)
                    } else {
                        swaggen_ui_kit::text_secondary(ui)
                    };
                    let r_resp = egui::Frame::new()
                        .fill(swaggen_ui_kit::surface_bg(ui))
                        .inner_margin(egui::Margin::symmetric(0, 3))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                                ui.allocate_ui_with_layout(
                                    egui::vec2(chk_w, 22.0),
                                    egui::Layout::centered_and_justified(
                                        egui::Direction::LeftToRight,
                                    ),
                                    |ui| {
                                        if ui.checkbox(&mut row.enabled, "").changed() {
                                            changed = true;
                                        }
                                    },
                                );
                                paint_v_hairline(ui, border_col);
                                ui.add_space(8.0);
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut row.key)
                                            .id(egui::Id::unique((id_salt, "flat_k", i)))
                                            .hint_text(key_hint)
                                            .text_color(text_col)
                                            .frame(egui::Frame::NONE)
                                            .desired_width((key_w - 8.0).max(40.0)),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                paint_v_hairline_padded(ui, border_col);
                                ui.add_space(6.0);
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(&mut row.value)
                                            .id(egui::Id::unique((id_salt, "flat_v", i)))
                                            .hint_text(val_hint)
                                            .text_color(text_col)
                                            .frame(egui::Frame::NONE)
                                            .desired_width((val_w - 6.0).max(40.0)),
                                    )
                                    .changed()
                                {
                                    changed = true;
                                }
                                if show_desc {
                                    paint_v_hairline(ui, border_col);
                                    ui.add_space(8.0);
                                    if ui
                                        .add(
                                            egui::TextEdit::singleline(&mut row.description)
                                                .id(egui::Id::unique((id_salt, "flat_d", i)))
                                                .hint_text("Description")
                                                .text_color(swaggen_ui_kit::text_secondary(ui))
                                                .frame(egui::Frame::NONE)
                                                .desired_width((desc_w - 8.0).max(40.0)),
                                        )
                                        .changed()
                                    {
                                        changed = true;
                                    }
                                }
                                ui.allocate_ui_with_layout(
                                    egui::vec2(act_w, 22.0),
                                    egui::Layout::centered_and_justified(
                                        egui::Direction::LeftToRight,
                                    ),
                                    |ui| {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    egui::RichText::new("×")
                                                        .size(13.0)
                                                        .color(swaggen_ui_kit::text_secondary(ui)),
                                                )
                                                .frame(false),
                                            )
                                            .on_hover_text("Delete row")
                                            .clicked()
                                        {
                                            remove_idx = Some(i);
                                        }
                                    },
                                );
                            });
                        });
                    paint_h_hairline(ui, r_resp.response.rect, border_col);
                }

                // Inline Ghost Starter Row (borderless, zero extra click needed!)
                let has_trailing_blank = rows
                    .last()
                    .is_some_and(|r| r.key.is_empty() && r.value.is_empty());
                if include_ghost_row && !has_trailing_blank {
                    let next_idx = rows.len();
                    let mut gk = String::new();
                    let mut gv = String::new();
                    let mut gd = String::new();
                    egui::Frame::new()
                        .fill(swaggen_ui_kit::surface_bg(ui))
                        .inner_margin(egui::Margin::symmetric(0, 3))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                                ui.allocate_exact_size(
                                    egui::vec2(chk_w, 22.0),
                                    egui::Sense::hover(),
                                );
                                paint_v_hairline(ui, border_col);
                                ui.add_space(8.0);
                                let kc = ui
                                    .add(
                                        egui::TextEdit::singleline(&mut gk)
                                            .id(egui::Id::unique((id_salt, "flat_k", next_idx)))
                                            .hint_text(key_hint)
                                            .frame(egui::Frame::NONE)
                                            .desired_width((key_w - 8.0).max(40.0)),
                                    )
                                    .changed();
                                paint_v_hairline_padded(ui, border_col);
                                ui.add_space(6.0);
                                let vc = ui
                                    .add(
                                        egui::TextEdit::singleline(&mut gv)
                                            .id(egui::Id::unique((id_salt, "flat_v", next_idx)))
                                            .hint_text(val_hint)
                                            .frame(egui::Frame::NONE)
                                            .desired_width((val_w - 6.0).max(40.0)),
                                    )
                                    .changed();
                                let mut dc = false;
                                if show_desc {
                                    paint_v_hairline(ui, border_col);
                                    ui.add_space(8.0);
                                    dc = ui
                                        .add(
                                            egui::TextEdit::singleline(&mut gd)
                                                .id(egui::Id::unique((id_salt, "flat_d", next_idx)))
                                                .hint_text("Description")
                                                .frame(egui::Frame::NONE)
                                                .desired_width((desc_w - 8.0).max(40.0)),
                                        )
                                        .changed();
                                }
                                ui.allocate_exact_size(
                                    egui::vec2(act_w, 22.0),
                                    egui::Sense::hover(),
                                );
                                if (kc || vc || dc)
                                    && (!gk.is_empty() || !gv.is_empty() || !gd.is_empty())
                                {
                                    let mut r = KeyValueRow::new(gk, gv);
                                    r.description = gd;
                                    rows.push(r);
                                    changed = true;
                                }
                            });
                        });
                }
            });

        if let Some(i) = remove_idx {
            rows.remove(i);
            changed = true;
        }
    });
    changed
}

fn paint_h_hairline(ui: &egui::Ui, row_rect: egui::Rect, color: egui::Color32) {
    ui.painter().line_segment(
        [row_rect.left_bottom(), row_rect.right_bottom()],
        egui::Stroke::new(1.0, color),
    );
}

fn paint_v_hairline(ui: &mut egui::Ui, color: egui::Color32) {
    let (_, r) = ui.allocate_space(egui::vec2(1.0, 22.0));
    ui.painter().line_segment(
        [
            egui::pos2(r.center().x, r.top() - 4.0),
            egui::pos2(r.center().x, r.bottom() + 4.0),
        ],
        egui::Stroke::new(1.0, color),
    );
}

fn paint_v_hairline_padded(ui: &mut egui::Ui, color: egui::Color32) {
    let (_, r) = ui.allocate_space(egui::vec2(6.0, 22.0));
    ui.painter().line_segment(
        [
            egui::pos2(r.center().x, r.top() - 4.0),
            egui::pos2(r.center().x, r.bottom() + 4.0),
        ],
        egui::Stroke::new(1.0, color),
    );
}
