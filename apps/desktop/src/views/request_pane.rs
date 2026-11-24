//! Multi-tab request composer: method, URL, environment selector, params, headers,
//! body types (JSON/Text/XML/Form/Multipart/Binary), auth inheritance, TLS/redirects,
//! and masked resolved request preview (RQ01-RQ09, AU01-AU06, EV01-EV07, UX02).
use crate::app::{App, RequestSubTab};
use eframe::egui;
use swaggen_domain::workspace::{
    ApiKeyPlacement, AssertionRule, AssertionTarget, AuthConfig, HttpMethod, KeyValueRow,
    MultipartField, MultipartValueKind, RequestBody, VariableContext, folder_chain, new_uid,
    resolve_effective_auth, resolve_request_preview,
};
use swaggen_ui_kit::{NoticeKind, Tokens};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    // 1. Multi-tab bar (UX02).
    ui.horizontal_wrapped(|ui| {
        let mut switch_tab: Option<String> = None;
        let mut close_tab: Option<String> = None;
        let mut toggle_pin: Option<String> = None;
        for tab in &app.state.open_tabs {
            let label = app
                .state
                .requests
                .iter()
                .find(|r| r.id == tab.request_id)
                .map(|r| format!("{} {}", r.method.as_str(), r.name))
                .unwrap_or_else(|| "Missing".into());
            let active = app.state.active_tab_id.as_deref() == Some(tab.id.as_str());
            let pin_tag = if tab.pinned { "[pin] " } else { "" };
            if ui
                .selectable_label(active, format!("{pin_tag}{label}"))
                .clicked()
            {
                switch_tab = Some(tab.id.clone());
            }
            if ui
                .small_button(if tab.pinned { "Unpin" } else { "Pin" })
                .clicked()
            {
                toggle_pin = Some(tab.id.clone());
            }
            if !tab.pinned && ui.small_button("x").clicked() {
                close_tab = Some(tab.id.clone());
            }
            ui.separator();
        }
        if ui.small_button("Close unpinned").clicked() {
            app.state.close_unpinned_tabs();
            app.mark_dirty();
        }
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
    ui.separator();

    let Some(req_id) = app.active_request_id() else {
        ui.label("Select or create a request from the sidebar.");
        return;
    };

    let Some(req_idx) = app.state.requests.iter().position(|r| r.id == req_id) else {
        ui.label("Selected request not found.");
        return;
    };

    // 2. Method + URL + Environment + Send/Cancel row (RQ01, RQ06, EV01, EV07).
    let mut changed = false;
    let is_inflight = app.inflight_request_id.as_deref() == Some(req_id.as_str());
    let active_env_is_prod = app
        .state
        .active_environment()
        .is_some_and(|e| e.is_production);

    ui.horizontal_wrapped(|ui| {
        let mut method = app.state.requests[req_idx].method;
        egui::ComboBox::from_id_salt("req-method")
            .selected_text(
                egui::RichText::new(method.as_str())
                    .color(swaggen_ui_kit::method_color(method.as_str()))
                    .strong(),
            )
            .width(95.0)
            .show_ui(ui, |ui| {
                for m in HttpMethod::ALL {
                    if ui
                        .selectable_value(
                            &mut method,
                            m,
                            egui::RichText::new(m.as_str())
                                .color(swaggen_ui_kit::method_color(m.as_str())),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                }
            });
        app.state.requests[req_idx].method = method;

        let url_w = (ui.available_width() - 360.0).max(180.0);
        if ui
            .add(
                egui::TextEdit::singleline(&mut app.state.requests[req_idx].url)
                    .hint_text("https://api.example.test/v1/tasks")
                    .desired_width(url_w),
            )
            .changed()
        {
            changed = true;
        }

        // Environment selector right next to Send button (EV01).
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
            Tokens::TEXT_BRAND
        };
        egui::ComboBox::from_id_salt("active-env")
            .selected_text(egui::RichText::new(env_label).color(env_color).strong())
            .width(135.0)
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
            if let Some((_, phase, done, total)) = app.transfer_progress {
                let txt = match total {
                    Some(t) => format!("{phase}: {done}/{t} B"),
                    None => format!("{phase}: {done} B"),
                };
                ui.label(egui::RichText::new(txt).color(Tokens::STATUS_INFO).small());
            }
            if swaggen_ui_kit::danger_button(ui, true, "Cancel").clicked() {
                app.cancel_inflight();
            }
        } else if swaggen_ui_kit::primary_button(ui, app.loaded, "Send (Ctrl+Enter)").clicked() {
            app.send_active_request(false);
        }
    });

    // Request title / tags / move / duplicate bar.
    ui.horizontal_wrapped(|ui| {
        ui.label("Name:");
        if ui
            .add(
                egui::TextEdit::singleline(&mut app.state.requests[req_idx].name)
                    .desired_width(180.0),
            )
            .changed()
        {
            changed = true;
        }
        let mut tags_csv = app.state.requests[req_idx].tags.join(", ");
        ui.label("Tags:");
        if ui
            .add(
                egui::TextEdit::singleline(&mut tags_csv)
                    .hint_text("auth, v1")
                    .desired_width(120.0),
            )
            .changed()
        {
            app.state.requests[req_idx].tags = tags_csv
                .split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect();
            changed = true;
        }
        if swaggen_ui_kit::secondary_button(ui, true, "Duplicate").clicked()
            && let Some(new_id) = app.state.duplicate_request(&req_id)
        {
            app.state.open_or_focus_tab(&new_id);
            changed = true;
        }
        if app.state.requests[req_idx].dirty_vs_shared {
            swaggen_ui_kit::status_badge(
                ui,
                NoticeKind::Warning,
                "Draft differs from shared (Q23)",
            );
        }
    });

    // Compute live resolved preview using masked secret placeholders (EV05 / RQ07).
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

    // 3. Request sub-tabs.
    ui.horizontal_wrapped(|ui| {
        for (tab, label) in [
            (RequestSubTab::Params, "Params"),
            (RequestSubTab::Headers, "Headers"),
            (RequestSubTab::Body, "Body"),
            (RequestSubTab::Auth, "Auth"),
            (RequestSubTab::Assertions, "Assertions (TS01)"),
            (RequestSubTab::Connection, "Connection"),
            (RequestSubTab::Resolved, "Resolved preview"),
        ] {
            if ui
                .selectable_label(app.request_subtab == tab, label)
                .clicked()
            {
                app.request_subtab = tab;
            }
        }
    });
    ui.separator();

    let mut pending_oauth_cmd: Option<crate::worker::Command> = None;
    egui::ScrollArea::vertical()
        .id_salt("req-editor-scroll")
        .max_height(210.0)
        .show(ui, |ui| match app.request_subtab {
            RequestSubTab::Params => {
                ui.label(egui::RichText::new("Effective URL preview (Q03)").strong());
                swaggen_ui_kit::code_surface(ui, &resolved.masked_url);
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Path parameters (:name or {name})").strong());
                if edit_kv_table(ui, "path-params", &mut app.state.requests[req_idx].path_params) {
                    changed = true;
                }
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Ordered query parameters (duplicates preserved · RQ02)")
                        .strong(),
                );
                if edit_kv_table(ui, "query-params", &mut app.state.requests[req_idx].query_params) {
                    changed = true;
                }
            }
            RequestSubTab::Headers => {
                ui.label(
                    egui::RichText::new("Ordered headers (duplicates preserved · RQ03)").strong(),
                );
                if edit_kv_table(ui, "req-headers", &mut app.state.requests[req_idx].headers) {
                    changed = true;
                }
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Generated headers (Q04)").strong());
                for h in resolved.masked_headers.iter().filter(|h| h.generated) {
                    ui.monospace(format!("{}: {}", h.name, h.value));
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
                        (6, "Binary file"),
                    ] {
                        if ui.selectable_label(kind_idx == idx, lbl).clicked() && kind_idx != idx {
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
                        ui.label("No request body (Q13).");
                    }
                    RequestBody::Json(text) => {
                        if !text.trim().is_empty()
                            && let Err(err) = serde_json::from_str::<serde_json::Value>(text)
                        {
                            swaggen_ui_kit::notice_banner(
                                ui,
                                NoticeKind::Warning,
                                "JSON syntax check (Q10)",
                                &err.to_string(),
                            );
                        }
                        if ui
                            .add(
                                egui::TextEdit::multiline(text)
                                    .desired_width(ui.available_width())
                                    .desired_rows(6)
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
                                    .desired_rows(6)
                                    .code_editor(),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                    }
                    RequestBody::FormUrlEncoded(rows) => {
                        if edit_kv_table(ui, "form-body", rows) {
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
                                                    .desired_width(220.0),
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
                                                    .desired_width(200.0),
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
                                                "Missing file (Q17)",
                                            );
                                        }
                                    }
                                }
                                if ui.small_button("x").clicked() {
                                    del_idx = Some(i);
                                }
                            });
                        }
                        if let Some(i) = del_idx {
                            fields.remove(i);
                            changed = true;
                        }
                        ui.horizontal(|ui| {
                            if swaggen_ui_kit::secondary_button(ui, true, "+ Text field").clicked() {
                                fields.push(MultipartField {
                                    name: String::new(),
                                    value: MultipartValueKind::Text(String::new()),
                                    enabled: true,
                                });
                                changed = true;
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
                            ui.label("Local file path (local only · RQ05):");
                            if ui
                                .add(egui::TextEdit::singleline(&mut p).desired_width(280.0))
                                .changed()
                            {
                                *local_path = if p.trim().is_empty() { None } else { Some(p) };
                                changed = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Content-Type:");
                            if ui
                                .add(egui::TextEdit::singleline(content_type).desired_width(220.0))
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
                ui.label(
                    egui::RichText::new(format!("Effective auth source: {}", eff.source))
                        .color(Tokens::TEXT_BRAND),
                );
                let mut auth_idx = match &app.state.requests[req_idx].auth {
                    AuthConfig::Inherit => 0,
                    AuthConfig::NoAuth => 1,
                    AuthConfig::Bearer { .. } => 2,
                    AuthConfig::Basic { .. } => 3,
                    AuthConfig::ApiKey { .. } => 4,
                };
                ui.horizontal_wrapped(|ui| {
                    for (idx, lbl) in [
                        (0, "Inherit"),
                        (1, "No Auth"),
                        (2, "Bearer"),
                        (3, "Basic"),
                        (4, "API Key"),
                    ] {
                        if ui.selectable_label(auth_idx == idx, lbl).clicked() && auth_idx != idx {
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
                        ui.label("Inherits auth from parent folder or collection (H06).");
                    }
                    AuthConfig::NoAuth => {
                        ui.label("Authorization disabled for this request (H02).");
                    }
                    AuthConfig::Bearer { token } => {
                        ui.label("Bearer token (use {{secret_var}} or OAuth2 PKCE below):");
                        if ui
                            .add(egui::TextEdit::singleline(token).desired_width(300.0))
                            .changed()
                        {
                            changed = true;
                        }
                        ui.add_space(6.0);
                        ui.collapsing("OAuth 2.0 PKCE + Loopback CSRF Helper (AU08)", |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Auth URL:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut app.oauth_auth_url)
                                        .desired_width(240.0),
                                );
                            });
                            ui.horizontal(|ui| {
                                ui.label("Token URL:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut app.oauth_token_url)
                                        .desired_width(240.0),
                                );
                            });
                            ui.horizontal(|ui| {
                                ui.label("Client ID:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut app.oauth_client_id)
                                        .desired_width(150.0),
                                );
                                ui.label("Scope:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut app.oauth_scope)
                                        .desired_width(130.0),
                                );
                            });
                            if swaggen_ui_kit::secondary_button(
                                ui,
                                !app.oauth_auth_url.trim().is_empty()
                                    && !app.oauth_token_url.trim().is_empty(),
                                "Start OAuth2 PKCE Loopback Flow",
                            )
                            .clicked()
                            {
                                pending_oauth_cmd = Some(crate::worker::Command::RunOAuth2Pkce {
                                    auth_url: app.oauth_auth_url.trim().to_owned(),
                                    token_url: app.oauth_token_url.trim().to_owned(),
                                    client_id: app.oauth_client_id.trim().to_owned(),
                                    scope: app.oauth_scope.trim().to_owned(),
                                    downloads_dir: app.root.join("downloads"),
                                });
                            }
                            if !app.oauth_authorize_url.is_empty() {
                                ui.horizontal(|ui| {
                                    ui.label("Authorize URL:");
                                    ui.text_edit_singleline(&mut app.oauth_authorize_url);
                                });
                            }
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
                                .selectable_label(*placement == ApiKeyPlacement::Query, "Query")
                                .clicked()
                            {
                                *placement = ApiKeyPlacement::Query;
                                changed = true;
                            }
                        });
                        if *placement == ApiKeyPlacement::Query {
                            swaggen_ui_kit::notice_banner(
                                ui,
                                NoticeKind::Warning,
                                "API key in query string (H05)",
                                "Query secrets appear in URL logs and are scrubbed on default export.",
                            );
                        }
                    }
                }
            }
            RequestSubTab::Assertions => {
                ui.label(
                    egui::RichText::new("Response assertions (TS01 · saved with request)")
                        .strong(),
                );
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
                                    .desired_width(110.0),
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
                            .width(90.0)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(matches!(r.target, AssertionTarget::StatusEquals), "Status").clicked() {
                                    r.target = AssertionTarget::StatusEquals;
                                    changed = true;
                                }
                                if ui.selectable_label(matches!(r.target, AssertionTarget::MaxDurationMs), "Max ms").clicked() {
                                    r.target = AssertionTarget::MaxDurationMs;
                                    changed = true;
                                }
                                if ui.selectable_label(matches!(r.target, AssertionTarget::JsonPathEquals { .. }), "JSON path").clicked() {
                                    r.target = AssertionTarget::JsonPathEquals { path: "$.ok".into() };
                                    changed = true;
                                }
                                if ui.selectable_label(matches!(r.target, AssertionTarget::HeaderEquals { .. }), "Header").clicked() {
                                    r.target = AssertionTarget::HeaderEquals { name: "content-type".into() };
                                    changed = true;
                                }
                                if ui.selectable_label(matches!(r.target, AssertionTarget::BodyContains), "Contains").clicked() {
                                    r.target = AssertionTarget::BodyContains;
                                    changed = true;
                                }
                            });
                        match &mut r.target {
                            AssertionTarget::JsonPathEquals { path } => {
                                if ui
                                    .add(egui::TextEdit::singleline(path).desired_width(110.0))
                                    .changed()
                                {
                                    changed = true;
                                }
                            }
                            AssertionTarget::HeaderEquals { name } => {
                                if ui
                                    .add(egui::TextEdit::singleline(name).desired_width(110.0))
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
                                    .desired_width(120.0),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                        if ui.small_button("x").clicked() {
                            del_idx = Some(i);
                        }
                    });
                }
                if let Some(i) = del_idx {
                    rules.remove(i);
                    changed = true;
                }
                if swaggen_ui_kit::secondary_button(ui, true, "+ Add assertion").clicked() {
                    rules.push(AssertionRule {
                        id: new_uid(),
                        label: "Status 200".into(),
                        target: AssertionTarget::StatusEquals,
                        expected: "200".into(),
                        enabled: true,
                    });
                    changed = true;
                }
                ui.separator();
                ui.label(
                    egui::RichText::new("Dynamic Vars (EV08) & Response Extractors (EV09)")
                        .strong(),
                );
                ui.label("Dynamic placeholders supported everywhere: {{$uuid}}, {{$timestamp}}, {{$isoTimestamp}}, {{$randomInt}}, {{$randomHex}}");
                let extractors = &mut app.state.requests[req_idx].extractors;
                let mut rm_ext = None;
                for (i, ex) in extractors.iter_mut().enumerate() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Var:");
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut ex.target_var)
                                    .desired_width(110.0),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                ex.source == swaggen_domain::dynamic_vars::ExtractorSource::BodyJsonPath,
                                "JSONPath",
                            )
                            .clicked()
                        {
                            ex.source = swaggen_domain::dynamic_vars::ExtractorSource::BodyJsonPath;
                            changed = true;
                        }
                        if ui
                            .selectable_label(
                                ex.source == swaggen_domain::dynamic_vars::ExtractorSource::Header,
                                "Header",
                            )
                            .clicked()
                        {
                            ex.source = swaggen_domain::dynamic_vars::ExtractorSource::Header;
                            changed = true;
                        }
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut ex.expression)
                                    .hint_text("$.token or X-Request-Id")
                                    .desired_width(150.0),
                            )
                            .changed()
                        {
                            changed = true;
                        }
                        if ui.checkbox(&mut ex.mark_secret, "Secret").changed() {
                            changed = true;
                        }
                        if ui.small_button("x").clicked() {
                            rm_ext = Some(i);
                        }
                    });
                }
                if let Some(i) = rm_ext {
                    extractors.remove(i);
                    changed = true;
                }
                if swaggen_ui_kit::secondary_button(ui, true, "+ Add extractor").clicked() {
                    extractors.push(swaggen_domain::dynamic_vars::ResponseExtractorRule {
                        id: new_uid(),
                        target_var: "extracted_id".into(),
                        source: swaggen_domain::dynamic_vars::ExtractorSource::BodyJsonPath,
                        expression: "$.id".into(),
                        mark_secret: false,
                    });
                    changed = true;
                }
            }
            RequestSubTab::Connection => {
                let conn = &mut app.state.requests[req_idx].connection;
                ui.horizontal(|ui| {
                    ui.label("Timeout (ms):");
                    if ui
                        .add(egui::DragValue::new(&mut conn.timeout_ms).range(100..=300_000))
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
                    ui.label("Max redirects:");
                    if ui
                        .add(egui::DragValue::new(&mut conn.max_redirects).range(0..=20))
                        .changed()
                    {
                        changed = true;
                    }
                });
                if ui
                    .checkbox(
                        &mut conn.send_auth_on_cross_host_redirect,
                        "Allow forwarding Authorization/Cookie across hosts on redirect (off by default · AU06)",
                    )
                    .changed()
                {
                    changed = true;
                }
                let mut exc_csv = conn.tls_host_exceptions.join(", ");
                ui.label("Host-scoped TLS exceptions (comma-separated host or host:port · AU05/H12):");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut exc_csv)
                            .hint_text("localhost:8443")
                            .desired_width(360.0),
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
                ui.separator();
                ui.label(egui::RichText::new("Proxy & Custom CA / mTLS (AU07 / AU09)").strong());
                let mut hp = conn.proxy.http_proxy.clone().unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label("HTTP proxy:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut hp)
                                .hint_text("http://127.0.0.1:8888")
                                .desired_width(240.0),
                        )
                        .changed()
                    {
                        conn.proxy.http_proxy = if hp.trim().is_empty() { None } else { Some(hp) };
                        changed = true;
                    }
                    let mut bypass_csv = conn.proxy.bypass_hosts.join(", ");
                    ui.label("Bypass:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut bypass_csv)
                                .hint_text("localhost, 127.0.0.1")
                                .desired_width(180.0),
                        )
                        .changed()
                    {
                        conn.proxy.bypass_hosts = bypass_csv
                            .split(',')
                            .map(|s| s.trim().to_owned())
                            .filter(|s| !s.is_empty())
                            .collect();
                        changed = true;
                    }
                });
                let mut ca_csv = conn.custom_ca_pem_paths.join(", ");
                ui.horizontal(|ui| {
                    ui.label("Custom CA PEM paths:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut ca_csv)
                                .hint_text("C:\\certs\\corp-ca.pem")
                                .desired_width(340.0),
                        )
                        .changed()
                    {
                        conn.custom_ca_pem_paths = ca_csv
                            .split(',')
                            .map(|s| s.trim().to_owned())
                            .filter(|s| !s.is_empty())
                            .collect();
                        changed = true;
                    }
                });
                let mut rm_cert = None;
                for (i, mc) in conn.client_certs.iter_mut().enumerate() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("mTLS Host:");
                        if ui.add(egui::TextEdit::singleline(&mut mc.host).desired_width(110.0)).changed() {
                            changed = true;
                        }
                        if ui.add(egui::DragValue::new(&mut mc.port).range(0..=65535)).changed() {
                            changed = true;
                        }
                        if ui
                            .add(egui::TextEdit::singleline(&mut mc.cert_pem_path).hint_text("cert.pem").desired_width(130.0))
                            .changed()
                        {
                            changed = true;
                        }
                        if ui
                            .add(egui::TextEdit::singleline(&mut mc.key_pem_path).hint_text("key.pem").desired_width(130.0))
                            .changed()
                        {
                            changed = true;
                        }
                        if ui.small_button("x").clicked() {
                            rm_cert = Some(i);
                        }
                    });
                }
                if let Some(i) = rm_cert {
                    conn.client_certs.remove(i);
                    changed = true;
                }
                if swaggen_ui_kit::secondary_button(ui, true, "+ Add mTLS client cert").clicked() {
                    conn.client_certs.push(swaggen_domain::workspace::ClientCertMapping {
                        host: "api.example.test".into(),
                        port: 443,
                        cert_pem_path: String::new(),
                        key_pem_path: String::new(),
                        expires_at_unix: None,
                    });
                    changed = true;
                }
            }
            RequestSubTab::Resolved => {
                for err in &resolved.blocking_errors {
                    swaggen_ui_kit::notice_banner(ui, NoticeKind::Danger, "Blocked", err);
                }
                for w in &resolved.warnings {
                    swaggen_ui_kit::notice_banner(ui, NoticeKind::Warning, "Warning", w);
                }
                ui.label(format!(
                    "{} {} (Auth: {})",
                    resolved.method.as_str(),
                    resolved.masked_url,
                    resolved.auth_source
                ));
                let mut preview_lines = Vec::new();
                for h in &resolved.masked_headers {
                    preview_lines.push(format!("{}: {}", h.name, h.value));
                }
                if !resolved.masked_body_preview.is_empty() {
                    preview_lines.push(String::new());
                    preview_lines.push(resolved.masked_body_preview.clone());
                }
                swaggen_ui_kit::code_surface(ui, &preview_lines.join("\n"));
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Code snippet (PR07 · secret-safe):").strong());
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
                    if ui.small_button("Copy snippet").clicked() {
                        ui.ctx().copy_text(snippet.clone());
                    }
                });
                let snippet = swaggen_runner::protocols_and_tools::generate_code_snippet(
                    &resolved,
                    app.snippet_target,
                );
                swaggen_ui_kit::code_surface(ui, &snippet);
            }
        });

    if let Some(cmd) = pending_oauth_cmd {
        app.submit(cmd);
    }
    if changed {
        app.state.requests[req_idx].dirty_vs_shared = true;
        app.mark_dirty();
    }
}

fn edit_kv_table(ui: &mut egui::Ui, id_salt: &str, rows: &mut Vec<KeyValueRow>) -> bool {
    let mut changed = false;
    let mut remove_idx = None;
    let mut swap: Option<(usize, usize)> = None;
    ui.push_id(id_salt, |ui| {
        for (i, row) in rows.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                if ui.checkbox(&mut row.enabled, "").changed() {
                    changed = true;
                }
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut row.key)
                            .hint_text("key")
                            .desired_width(130.0),
                    )
                    .changed()
                {
                    changed = true;
                }
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut row.value)
                            .hint_text("value")
                            .desired_width(210.0),
                    )
                    .changed()
                {
                    changed = true;
                }
                if i > 0 && ui.small_button("Up").clicked() {
                    swap = Some((i, i - 1));
                }
                if ui.small_button("x").clicked() {
                    remove_idx = Some(i);
                }
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
        if swaggen_ui_kit::secondary_button(ui, true, "+ Row").clicked() {
            rows.push(KeyValueRow::new("", ""));
            changed = true;
        }
    });
    changed
}
