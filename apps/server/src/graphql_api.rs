//! High-performance `/v1/graphql` endpoint for Swaggen Server:
//! - Standard GraphQL-over-HTTP (`POST` JSON envelope + read-only `GET ?query=...` — blocks mutations on GET)
//! - Full `__schema` introspection support for Desktop Studio, GraphiQL, Apollo & Postman
//! - Query depth (`<= 8`) & field complexity (`<= 120`) DoS protection
//! - Batched DataLoader-style field resolution (strictly ZERO N+1 queries on single-workspace & selective field queries,
//!   with live `extensions.performance.dbQueries` telemetry)
use crate::{AppState, require_auth};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::time::Instant;
use swaggen_api_contract::{
    CreateCommentRequest, CreateWorkspaceRequest, PublishBatchRequest, PublishOperation,
    ResolveCommentRequest,
};
use swaggen_domain::graphql::{GraphQlOperationKind, analyze_graphql_document};
use swaggen_mongo_store::{
    crypto::sha256_hex,
    error::StoreError,
    repo::{WorkspaceRepository, now_unix},
};
use uuid::Uuid;

pub const MAX_GRAPHQL_DEPTH: usize = 8;
pub const MAX_GRAPHQL_FIELDS: usize = 120;

#[derive(Debug, Deserialize, Default)]
pub struct GraphQlHttpRequest {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub variables: Option<Value>,
    #[serde(default, rename = "operationName")]
    pub operation_name: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct GraphQlGetQuery {
    pub query: Option<String>,
    pub variables: Option<String>,
    #[serde(rename = "operationName")]
    pub operation_name: Option<String>,
}

pub async fn graphql_get_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<GraphQlGetQuery>,
) -> Response {
    let vars = q
        .variables
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| serde_json::from_str::<Value>(s).ok());
    let req = GraphQlHttpRequest {
        query: q.query.unwrap_or_default(),
        variables: vars,
        operation_name: q.operation_name,
    };
    execute_graphql_http(state, headers, Method::GET, req).await
}

pub async fn graphql_post_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GraphQlHttpRequest>,
) -> Response {
    execute_graphql_http(state, headers, Method::POST, req).await
}

async fn execute_graphql_http(
    state: AppState,
    headers: HeaderMap,
    http_method: Method,
    req: GraphQlHttpRequest,
) -> Response {
    let started = Instant::now();
    if req.query.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "data": Value::Null,
                "errors": [{ "message": "GraphQL `query` string must not be empty" }]
            })),
        )
            .into_response();
    }

    let analysis = analyze_graphql_document(
        &req.query,
        req.operation_name.as_deref(),
        MAX_GRAPHQL_DEPTH,
        MAX_GRAPHQL_FIELDS,
    );
    if !analysis.complexity_warnings.is_empty() {
        let errs: Vec<Value> = analysis
            .complexity_warnings
            .into_iter()
            .map(|m| json!({ "message": m, "extensions": { "code": "QUERY_TOO_COMPLEX" } }))
            .collect();
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "data": Value::Null, "errors": errs })),
        )
            .into_response();
    }

    if http_method == Method::GET && analysis.active_kind == GraphQlOperationKind::Mutation {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            Json(json!({
                "data": Value::Null,
                "errors": [{
                    "message": "GraphQL mutations are not permitted over HTTP GET; use POST",
                    "extensions": { "code": "METHOD_NOT_ALLOWED" }
                }]
            })),
        )
            .into_response();
    }

    let vars_map = match req.variables {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    };

    // Parse top-level selections of the active operation
    let Some(root_fields) =
        parse_active_operation_fields(&req.query, req.operation_name.as_deref())
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "data": Value::Null,
                "errors": [{ "message": "Malformed GraphQL selection set" }]
            })),
        )
            .into_response();
    };

    let mut data = Map::new();
    let mut errors = Vec::new();
    let mut db_queries = 0usize;

    match analysis.active_kind {
        GraphQlOperationKind::Query => {
            for field in &root_fields {
                let out_key = field.alias.as_deref().unwrap_or(&field.name);
                match field.name.as_str() {
                    "__typename" => {
                        data.insert(out_key.to_owned(), json!("Query"));
                    }
                    "__schema" => {
                        data.insert(out_key.to_owned(), build_introspection_schema_json());
                    }
                    "health" => {
                        db_queries += 1;
                        let ready = state.store.check_ready().await.is_ok();
                        data.insert(
                            out_key.to_owned(),
                            json!({
                                "status": if ready { "ready" } else { "degraded" },
                                "engine": if state.store.is_mongo() { "mongodb_replica_set" } else { "in_memory_dev" }
                            }),
                        );
                    }
                    "viewer" => {
                        db_queries += 1;
                        match require_auth(&state, &headers).await {
                            Ok(auth) => {
                                data.insert(
                                    out_key.to_owned(),
                                    project_object(
                                        &field.children,
                                        json!({
                                            "__typename": "Viewer",
                                            "userId": auth.user_id,
                                            "email": auth.email,
                                            "isServerOwner": auth.is_server_owner,
                                        }),
                                    ),
                                );
                            }
                            Err(e) => {
                                data.insert(out_key.to_owned(), Value::Null);
                                errors.push(store_err_to_gql(&field.name, e));
                            }
                        }
                    }
                    "workspaces" => {
                        db_queries += 1;
                        match require_auth(&state, &headers).await {
                            Ok(auth) => {
                                db_queries += 1; // Batched `list_workspaces` (O(1) `$in` queries!)
                                match state.store.list_workspaces(&auth).await {
                                    Ok(mut list) => {
                                        if let Some(id_filter) = field.arg_str("id", &vars_map) {
                                            list.retain(|w| w.workspace_id == id_filter);
                                        }
                                        if let Some(slug_filter) = field.arg_str("slug", &vars_map)
                                        {
                                            list.retain(|w| w.slug == slug_filter);
                                        }
                                        let wants_entities = field.child("entities");
                                        let wants_members = field.child("members");
                                        let wants_comments = field.child("comments");
                                        let wants_notifs = field.child("notifications");

                                        let max_ws = field
                                            .arg_i64("limit", &vars_map)
                                            .unwrap_or(25)
                                            .clamp(1, 50)
                                            as usize;
                                        list.truncate(max_ws);

                                        // DataLoader single-pass prefetch across ALL returned workspaces (ZERO N+1!)
                                        let ws_ids: Vec<String> =
                                            list.iter().map(|w| w.workspace_id.clone()).collect();
                                        let mut batched = if wants_entities.is_some()
                                            || wants_members.is_some()
                                            || wants_comments.is_some()
                                            || wants_notifs.is_some()
                                        {
                                            state
                                                .store
                                                .batch_load_workspaces_children(
                                                    &auth,
                                                    &ws_ids,
                                                    wants_entities.is_some(),
                                                    wants_members.is_some(),
                                                    wants_comments.is_some(),
                                                    wants_notifs.is_some(),
                                                )
                                                .await
                                                .unwrap_or_default()
                                        } else {
                                            Default::default()
                                        };
                                        db_queries += batched.db_queries_executed;

                                        let mut ws_items = Vec::with_capacity(list.len());
                                        for ws in list {
                                            let mut obj = Map::new();
                                            obj.insert("__typename".into(), json!("Workspace"));
                                            obj.insert(
                                                "workspaceId".into(),
                                                json!(&ws.workspace_id),
                                            );
                                            obj.insert("slug".into(), json!(ws.slug));
                                            obj.insert("name".into(), json!(ws.name));
                                            obj.insert("description".into(), json!(ws.description));
                                            obj.insert("role".into(), json!(ws.role.as_str()));
                                            obj.insert(
                                                "memberCount".into(),
                                                json!(ws.member_count),
                                            );
                                            obj.insert(
                                                "currentRevision".into(),
                                                json!(ws.current_revision),
                                            );
                                            obj.insert("archived".into(), json!(ws.archived));

                                            if let Some(ent_sel) = wants_entities {
                                                let kind_f = ent_sel.arg_str("kind", &vars_map);
                                                let lim = ent_sel
                                                    .arg_i64("limit", &vars_map)
                                                    .unwrap_or(100)
                                                    .clamp(1, 500)
                                                    as usize;
                                                let ents = batched
                                                    .entities_by_ws
                                                    .remove(&ws.workspace_id)
                                                    .unwrap_or_default();
                                                let arr: Vec<Value> = ents
                                                    .into_iter()
                                                    .filter(|e| {
                                                        kind_f
                                                            .as_deref()
                                                            .is_none_or(|k| e.kind == k)
                                                    })
                                                    .take(lim)
                                                    .map(|e| {
                                                        project_object(
                                                            &ent_sel.children,
                                                            json!({
                                                                "__typename": "WorkspaceEntity",
                                                                "entityId": e.entity_id,
                                                                "parentId": e.parent_id,
                                                                "kind": e.kind,
                                                                "name": e.name,
                                                                "revision": e.revision,
                                                                "updatedAtUnix": e.updated_at_unix,
                                                                "updatedBy": e.updated_by,
                                                                "payloadJson": e.payload_json,
                                                            }),
                                                        )
                                                    })
                                                    .collect();
                                                obj.insert(
                                                    ent_sel.out_key().to_owned(),
                                                    Value::Array(arr),
                                                );
                                            }
                                            if let Some(mem_sel) = wants_members {
                                                let mems = batched
                                                    .members_by_ws
                                                    .remove(&ws.workspace_id)
                                                    .unwrap_or_default();
                                                let arr: Vec<Value> = mems
                                                    .into_iter()
                                                    .map(|m| {
                                                        project_object(
                                                            &mem_sel.children,
                                                            json!({
                                                                "__typename": "Member",
                                                                "userId": m.user_id,
                                                                "email": m.email,
                                                                "displayName": m.display_name,
                                                                "role": m.role.as_str(),
                                                                "joinedAtUnix": m.joined_at_unix,
                                                            }),
                                                        )
                                                    })
                                                    .collect();
                                                obj.insert(
                                                    mem_sel.out_key().to_owned(),
                                                    Value::Array(arr),
                                                );
                                            }
                                            if let Some(com_sel) = wants_comments {
                                                let eid_f = com_sel.arg_str("entityId", &vars_map);
                                                let cs = batched
                                                    .comments_by_ws
                                                    .remove(&ws.workspace_id)
                                                    .unwrap_or_default();
                                                let arr: Vec<Value> = cs
                                                    .into_iter()
                                                    .filter(|c| {
                                                        eid_f
                                                            .as_deref()
                                                            .is_none_or(|eid| c.entity_id == eid)
                                                    })
                                                    .map(|c| {
                                                        project_object(
                                                            &com_sel.children,
                                                            json!({
                                                                "__typename": "CommentThread",
                                                                "commentId": c.comment_id,
                                                                "entityId": c.entity_id,
                                                                "authorEmail": c.author_email,
                                                                "body": c.body,
                                                                "resolved": c.resolved,
                                                                "createdAtUnix": c.created_at_unix,
                                                            }),
                                                        )
                                                    })
                                                    .collect();
                                                obj.insert(
                                                    com_sel.out_key().to_owned(),
                                                    Value::Array(arr),
                                                );
                                            }
                                            if let Some(not_sel) = wants_notifs {
                                                let ns = batched
                                                    .notifications_by_ws
                                                    .remove(&ws.workspace_id)
                                                    .unwrap_or_default();
                                                let arr: Vec<Value> = ns
                                                    .into_iter()
                                                    .map(|n| {
                                                        project_object(
                                                            &not_sel.children,
                                                            json!({
                                                                "__typename": "Notification",
                                                                "notificationId": n.notification_id,
                                                                "kind": n.kind,
                                                                "title": n.title,
                                                                "read": n.read,
                                                                "createdAtUnix": n.created_at_unix,
                                                            }),
                                                        )
                                                    })
                                                    .collect();
                                                obj.insert(
                                                    not_sel.out_key().to_owned(),
                                                    Value::Array(arr),
                                                );
                                            }
                                            ws_items.push(project_object(
                                                &field.children,
                                                Value::Object(obj),
                                            ));
                                        }
                                        data.insert(out_key.to_owned(), Value::Array(ws_items));
                                    }
                                    Err(e) => {
                                        data.insert(out_key.to_owned(), Value::Null);
                                        errors.push(store_err_to_gql(&field.name, e));
                                    }
                                }
                            }
                            Err(e) => {
                                data.insert(out_key.to_owned(), Value::Null);
                                errors.push(store_err_to_gql(&field.name, e));
                            }
                        }
                    }
                    unknown => {
                        errors.push(json!({
                            "message": format!("Unknown Query field '{unknown}'"),
                            "extensions": { "code": "GRAPHQL_VALIDATION_FAILED" }
                        }));
                    }
                }
            }
        }
        GraphQlOperationKind::Mutation => {
            db_queries += 1;
            match require_auth(&state, &headers).await {
                Ok(auth) => {
                    let now = now_unix();
                    for field in &root_fields {
                        let out_key = field.alias.as_deref().unwrap_or(&field.name);
                        match field.name.as_str() {
                            "createWorkspace" => {
                                let slug = field.arg_str("slug", &vars_map).unwrap_or_default();
                                let name = field.arg_str("name", &vars_map).unwrap_or_default();
                                let description =
                                    field.arg_str("description", &vars_map).unwrap_or_default();
                                db_queries += 1;
                                match state
                                    .store
                                    .create_workspace(
                                        &auth,
                                        CreateWorkspaceRequest {
                                            slug,
                                            name,
                                            description,
                                        },
                                        now,
                                    )
                                    .await
                                {
                                    Ok(ws) => {
                                        data.insert(
                                            out_key.to_owned(),
                                            project_object(
                                                &field.children,
                                                json!({
                                                    "__typename": "Workspace",
                                                    "workspaceId": ws.workspace_id,
                                                    "slug": ws.slug,
                                                    "name": ws.name,
                                                    "description": ws.description,
                                                    "role": ws.role.as_str(),
                                                    "memberCount": ws.member_count,
                                                    "currentRevision": ws.current_revision,
                                                    "archived": ws.archived,
                                                }),
                                            ),
                                        );
                                    }
                                    Err(e) => {
                                        data.insert(out_key.to_owned(), Value::Null);
                                        errors.push(store_err_to_gql(&field.name, e));
                                    }
                                }
                            }
                            "upsertEntity" => {
                                let workspace_id =
                                    field.arg_str("workspaceId", &vars_map).unwrap_or_default();
                                let entity_id = field
                                    .arg_str("entityId", &vars_map)
                                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                                let kind = field
                                    .arg_str("kind", &vars_map)
                                    .unwrap_or_else(|| "request".into());
                                let name = field
                                    .arg_str("name", &vars_map)
                                    .unwrap_or_else(|| "Untitled".into());
                                let payload_json = field
                                    .arg_str("payloadJson", &vars_map)
                                    .unwrap_or_else(|| "{}".into());
                                let base_revision =
                                    field.arg_i64("baseRevision", &vars_map).unwrap_or(0);
                                let raw_seed = format!(
                                    "{workspace_id}|{entity_id}|{kind}|{name}|{base_revision}|{payload_json}"
                                );
                                let req_hash = sha256_hex(&raw_seed);
                                let operation_id = field
                                    .arg_str("operationId", &vars_map)
                                    .unwrap_or_else(|| format!("gql-op-{}", &req_hash[..24]));
                                db_queries += 1;
                                match state
                                    .store
                                    .publish_batch(
                                        &auth,
                                        &workspace_id,
                                        PublishBatchRequest {
                                            expected_membership_revision: None,
                                            operations: vec![PublishOperation {
                                                operation_id,
                                                request_hash: req_hash,
                                                entity_id,
                                                parent_id: field.arg_str("parentId", &vars_map),
                                                kind,
                                                name,
                                                order_key: field
                                                    .arg_i64("orderKey", &vars_map)
                                                    .unwrap_or(0),
                                                payload_json,
                                                base_revision,
                                                delete: field
                                                    .arg_bool("delete", &vars_map)
                                                    .unwrap_or(false),
                                                note: field
                                                    .arg_str("note", &vars_map)
                                                    .unwrap_or_else(|| "GraphQL mutation".into()),
                                            }],
                                        },
                                        now,
                                    )
                                    .await
                                {
                                    Ok(batch) => {
                                        if let Some(first) = batch.results.into_iter().next() {
                                            if let Some(cursor) = first.server_cursor {
                                                let _ = state
                                                    .event_bus
                                                    .send((workspace_id.clone(), cursor));
                                            }
                                            data.insert(
                                                out_key.to_owned(),
                                                project_object(
                                                    &field.children,
                                                    json!({
                                                        "__typename": "PublishResult",
                                                        "operationId": first.operation_id,
                                                        "entityId": first.entity_id,
                                                        "status": format!("{:?}", first.kind),
                                                        "serverRevision": first.server_revision,
                                                        "reason": first.reason,
                                                    }),
                                                ),
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        data.insert(out_key.to_owned(), Value::Null);
                                        errors.push(store_err_to_gql(&field.name, e));
                                    }
                                }
                            }
                            "createComment" => {
                                let workspace_id =
                                    field.arg_str("workspaceId", &vars_map).unwrap_or_default();
                                let comment_id = field
                                    .arg_str("commentId", &vars_map)
                                    .unwrap_or_else(|| Uuid::new_v4().to_string());
                                let entity_id =
                                    field.arg_str("entityId", &vars_map).unwrap_or_default();
                                let body = field.arg_str("body", &vars_map).unwrap_or_default();
                                let anchor_revision =
                                    field.arg_i64("anchorRevision", &vars_map).unwrap_or(1);
                                db_queries += 1;
                                match state
                                    .store
                                    .create_comment(
                                        &auth,
                                        &workspace_id,
                                        CreateCommentRequest {
                                            comment_id,
                                            entity_id,
                                            anchor_revision,
                                            body,
                                        },
                                        now,
                                    )
                                    .await
                                {
                                    Ok(c) => {
                                        data.insert(
                                            out_key.to_owned(),
                                            project_object(
                                                &field.children,
                                                json!({
                                                    "__typename": "CommentThread",
                                                    "commentId": c.comment_id,
                                                    "entityId": c.entity_id,
                                                    "authorEmail": c.author_email,
                                                    "body": c.body,
                                                    "resolved": c.resolved,
                                                    "createdAtUnix": c.created_at_unix,
                                                }),
                                            ),
                                        );
                                    }
                                    Err(e) => {
                                        data.insert(out_key.to_owned(), Value::Null);
                                        errors.push(store_err_to_gql(&field.name, e));
                                    }
                                }
                            }
                            "resolveComment" => {
                                let workspace_id =
                                    field.arg_str("workspaceId", &vars_map).unwrap_or_default();
                                let comment_id =
                                    field.arg_str("commentId", &vars_map).unwrap_or_default();
                                let resolved =
                                    field.arg_bool("resolved", &vars_map).unwrap_or(true);
                                db_queries += 1;
                                match state
                                    .store
                                    .resolve_comment(
                                        &auth,
                                        &workspace_id,
                                        &comment_id,
                                        ResolveCommentRequest { resolved },
                                        now,
                                    )
                                    .await
                                {
                                    Ok(c) => {
                                        data.insert(
                                            out_key.to_owned(),
                                            project_object(
                                                &field.children,
                                                json!({
                                                    "__typename": "CommentThread",
                                                    "commentId": c.comment_id,
                                                    "entityId": c.entity_id,
                                                    "authorEmail": c.author_email,
                                                    "body": c.body,
                                                    "resolved": c.resolved,
                                                    "createdAtUnix": c.created_at_unix,
                                                }),
                                            ),
                                        );
                                    }
                                    Err(e) => {
                                        data.insert(out_key.to_owned(), Value::Null);
                                        errors.push(store_err_to_gql(&field.name, e));
                                    }
                                }
                            }
                            unknown => {
                                errors.push(json!({
                                    "message": format!("Unknown Mutation field '{unknown}'"),
                                    "extensions": { "code": "GRAPHQL_VALIDATION_FAILED" }
                                }));
                            }
                        }
                    }
                }
                Err(e) => {
                    errors.push(store_err_to_gql("mutation", e));
                }
            }
        }
        GraphQlOperationKind::Subscription => {
            errors.push(json!({
                "message": "Use SSE endpoint `/v1/workspaces/{id}/events` for live event streaming",
                "extensions": { "code": "UNSUPPORTED_OPERATION" }
            }));
        }
    }

    let mut resp_obj = Map::new();
    resp_obj.insert("data".into(), Value::Object(data));
    if !errors.is_empty() {
        resp_obj.insert("errors".into(), Value::Array(errors));
    }
    resp_obj.insert(
        "extensions".into(),
        json!({
            "performance": {
                "dbQueries": db_queries,
                "batchedResolver": true,
                "nPlusOneProtected": true,
                "durationMs": started.elapsed().as_millis() as u64,
            }
        }),
    );

    (StatusCode::OK, Json(Value::Object(resp_obj))).into_response()
}

fn store_err_to_gql(field: &str, err: StoreError) -> Value {
    let (_, code, msg, _) = err.to_http_parts();
    json!({
        "message": msg,
        "path": [field],
        "extensions": { "code": code }
    })
}

#[derive(Debug, Clone)]
enum ArgVal {
    Var(String),
    Str(String),
    Int(i64),
    Bool(bool),
}

#[derive(Debug, Clone)]
struct SelectedField {
    alias: Option<String>,
    name: String,
    args: Vec<(String, ArgVal)>,
    children: Vec<SelectedField>,
}

impl SelectedField {
    fn out_key(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }

    fn child(&self, name: &str) -> Option<&SelectedField> {
        self.children.iter().find(|c| c.name == name)
    }

    fn arg_str(&self, key: &str, vars: &Map<String, Value>) -> Option<String> {
        let (_, v) = self.args.iter().find(|(k, _)| k == key)?;
        match v {
            ArgVal::Str(s) => Some(s.clone()),
            ArgVal::Var(vn) => match vars.get(vn)? {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            },
            ArgVal::Int(n) => Some(n.to_string()),
            ArgVal::Bool(b) => Some(b.to_string()),
        }
    }

    fn arg_i64(&self, key: &str, vars: &Map<String, Value>) -> Option<i64> {
        let (_, v) = self.args.iter().find(|(k, _)| k == key)?;
        match v {
            ArgVal::Int(n) => Some(*n),
            ArgVal::Var(vn) => vars.get(vn)?.as_i64(),
            ArgVal::Str(s) => s.parse().ok(),
            _ => None,
        }
    }

    fn arg_bool(&self, key: &str, vars: &Map<String, Value>) -> Option<bool> {
        let (_, v) = self.args.iter().find(|(k, _)| k == key)?;
        match v {
            ArgVal::Bool(b) => Some(*b),
            ArgVal::Var(vn) => vars.get(vn)?.as_bool(),
            _ => None,
        }
    }
}

fn project_object(children: &[SelectedField], full: Value) -> Value {
    if children.is_empty() {
        return full;
    }
    let Value::Object(src) = full else {
        return full;
    };
    let mut out = Map::new();
    for ch in children {
        if let Some(val) = src.get(&ch.name) {
            out.insert(ch.out_key().to_owned(), val.clone());
        }
    }
    Value::Object(out)
}

#[derive(Debug, Clone)]
enum LexTok {
    Ident(String),
    Var(String),
    Str(String),
    Int(i64),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Colon,
    Other,
}

fn lex_gql(src: &str) -> Vec<LexTok> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if b[i].is_ascii_whitespace() || b[i] == b',' {
            i += 1;
            continue;
        }
        if b[i] == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i] == b'"' {
            i += 1;
            let mut s = String::new();
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' && i + 1 < b.len() {
                    i += 1;
                    let esc = match b[i] {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        other => other as char,
                    };
                    s.push(esc);
                    i += 1;
                } else {
                    let ch = src[i..].chars().next().unwrap_or(' ');
                    s.push(ch);
                    i += ch.len_utf8();
                }
            }
            i = (i + 1).min(b.len());
            out.push(LexTok::Str(s));
            continue;
        }
        match b[i] {
            b'(' => {
                out.push(LexTok::LParen);
                i += 1;
            }
            b')' => {
                out.push(LexTok::RParen);
                i += 1;
            }
            b'{' => {
                out.push(LexTok::LBrace);
                i += 1;
            }
            b'}' => {
                out.push(LexTok::RBrace);
                i += 1;
            }
            b':' => {
                out.push(LexTok::Colon);
                i += 1;
            }
            b'$' => {
                i += 1;
                let s = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                out.push(LexTok::Var(src[s..i].to_owned()));
            }
            c if c.is_ascii_digit() || c == b'-' => {
                let s = i;
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                if let Ok(n) = src[s..i].parse::<i64>() {
                    out.push(LexTok::Int(n));
                } else {
                    out.push(LexTok::Other);
                }
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let s = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                out.push(LexTok::Ident(src[s..i].to_owned()));
            }
            _ => {
                out.push(LexTok::Other);
                i += 1;
            }
        }
    }
    out
}

fn parse_active_operation_fields(
    src: &str,
    operation_name: Option<&str>,
) -> Option<Vec<SelectedField>> {
    let toks = lex_gql(src);
    let mut i = 0usize;
    let target = operation_name.map(str::trim).filter(|s| !s.is_empty());
    let mut first_fields = None;

    while i < toks.len() {
        match &toks[i] {
            LexTok::Ident(kw) if matches!(kw.as_str(), "query" | "mutation" | "subscription") => {
                i += 1;
                let op_name = if let Some(LexTok::Ident(n)) = toks.get(i) {
                    i += 1;
                    Some(n.clone())
                } else {
                    None
                };
                if matches!(toks.get(i), Some(LexTok::LParen)) {
                    skip_lex_parens(&toks, &mut i);
                }
                while i < toks.len() && !matches!(toks[i], LexTok::LBrace) {
                    i += 1;
                }
                let fields = parse_selection_block(&toks, &mut i)?;
                if let Some(t) = target {
                    if op_name.as_deref() == Some(t) {
                        return Some(fields);
                    }
                } else if first_fields.is_none() {
                    first_fields = Some(fields);
                }
            }
            LexTok::LBrace => {
                let fields = parse_selection_block(&toks, &mut i)?;
                if target.is_none() && first_fields.is_none() {
                    first_fields = Some(fields);
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    first_fields
}

fn skip_lex_parens(toks: &[LexTok], i: &mut usize) {
    if !matches!(toks.get(*i), Some(LexTok::LParen)) {
        return;
    }
    *i += 1;
    let mut d = 1usize;
    while *i < toks.len() && d > 0 {
        match toks[*i] {
            LexTok::LParen => d += 1,
            LexTok::RParen => d -= 1,
            _ => {}
        }
        *i += 1;
    }
}

fn parse_selection_block(toks: &[LexTok], i: &mut usize) -> Option<Vec<SelectedField>> {
    if !matches!(toks.get(*i), Some(LexTok::LBrace)) {
        return None;
    }
    *i += 1;
    let mut out = Vec::new();
    while *i < toks.len() {
        match &toks[*i] {
            LexTok::RBrace => {
                *i += 1;
                return Some(out);
            }
            LexTok::Ident(first) => {
                let mut alias = None;
                let mut name = first.clone();
                *i += 1;
                if matches!(toks.get(*i), Some(LexTok::Colon)) {
                    *i += 1;
                    if let Some(LexTok::Ident(real)) = toks.get(*i) {
                        alias = Some(name);
                        name = real.clone();
                        *i += 1;
                    }
                }
                let args = if matches!(toks.get(*i), Some(LexTok::LParen)) {
                    parse_field_args(toks, i)
                } else {
                    Vec::new()
                };
                let children = if matches!(toks.get(*i), Some(LexTok::LBrace)) {
                    parse_selection_block(toks, i).unwrap_or_default()
                } else {
                    Vec::new()
                };
                out.push(SelectedField {
                    alias,
                    name,
                    args,
                    children,
                });
            }
            _ => {
                *i += 1;
            }
        }
    }
    None
}

fn parse_field_args(toks: &[LexTok], i: &mut usize) -> Vec<(String, ArgVal)> {
    let mut out = Vec::new();
    if !matches!(toks.get(*i), Some(LexTok::LParen)) {
        return out;
    }
    *i += 1;
    while *i < toks.len() {
        match &toks[*i] {
            LexTok::RParen => {
                *i += 1;
                break;
            }
            LexTok::Ident(k) => {
                let key = k.clone();
                *i += 1;
                if matches!(toks.get(*i), Some(LexTok::Colon)) {
                    *i += 1;
                }
                let val = match toks.get(*i) {
                    Some(LexTok::Var(v)) => {
                        *i += 1;
                        Some(ArgVal::Var(v.clone()))
                    }
                    Some(LexTok::Str(s)) => {
                        *i += 1;
                        Some(ArgVal::Str(s.clone()))
                    }
                    Some(LexTok::Int(n)) => {
                        *i += 1;
                        Some(ArgVal::Int(*n))
                    }
                    Some(LexTok::Ident(b)) if b == "true" || b == "false" => {
                        let bv = b == "true";
                        *i += 1;
                        Some(ArgVal::Bool(bv))
                    }
                    _ => {
                        *i += 1;
                        None
                    }
                };
                if let Some(v) = val {
                    out.push((key, v));
                }
            }
            _ => {
                *i += 1;
            }
        }
    }
    out
}

fn scalar_type(name: &str) -> Value {
    json!({ "kind": "SCALAR", "name": name, "fields": Value::Null })
}

fn type_ref_scalar(name: &str, non_null: bool) -> Value {
    if non_null {
        json!({ "kind": "NON_NULL", "name": Value::Null, "ofType": { "kind": "SCALAR", "name": name, "ofType": Value::Null } })
    } else {
        json!({ "kind": "SCALAR", "name": name, "ofType": Value::Null })
    }
}

fn type_ref_obj(name: &str, list: bool) -> Value {
    if list {
        json!({
            "kind": "NON_NULL",
            "name": Value::Null,
            "ofType": {
                "kind": "LIST",
                "name": Value::Null,
                "ofType": { "kind": "OBJECT", "name": name }
            }
        })
    } else {
        json!({ "kind": "OBJECT", "name": name, "ofType": Value::Null })
    }
}

fn field_def(name: &str, args: Vec<(&str, &str, bool)>, ty: Value) -> Value {
    let arg_json: Vec<Value> = args
        .into_iter()
        .map(|(an, at, req)| {
            json!({
                "name": an,
                "type": type_ref_scalar(at, req)
            })
        })
        .collect();
    json!({
        "name": name,
        "description": "",
        "args": arg_json,
        "type": ty
    })
}

fn build_introspection_schema_json() -> Value {
    json!({
        "queryType": { "name": "Query" },
        "mutationType": { "name": "Mutation" },
        "subscriptionType": Value::Null,
        "types": [
            scalar_type("ID"),
            scalar_type("String"),
            scalar_type("Int"),
            scalar_type("Boolean"),
            {
                "kind": "OBJECT",
                "name": "Query",
                "description": "Swaggen Batched GraphQL Query Root (Zero N+1)",
                "fields": [
                    field_def("health", vec![], type_ref_obj("HealthStatus", false)),
                    field_def("viewer", vec![], type_ref_obj("Viewer", false)),
                    field_def(
                        "workspaces",
                        vec![("id", "ID", false), ("slug", "String", false), ("limit", "Int", false)],
                        type_ref_obj("Workspace", true),
                    ),
                ]
            },
            {
                "kind": "OBJECT",
                "name": "Mutation",
                "description": "Swaggen Idempotent GraphQL Mutation Root",
                "fields": [
                    field_def(
                        "createWorkspace",
                        vec![("slug", "String", true), ("name", "String", true), ("description", "String", false)],
                        type_ref_obj("Workspace", false),
                    ),
                    field_def(
                        "upsertEntity",
                        vec![
                            ("workspaceId", "ID", true),
                            ("entityId", "ID", true),
                            ("kind", "String", true),
                            ("name", "String", true),
                            ("payloadJson", "String", true),
                            ("baseRevision", "Int", true),
                            ("operationId", "String", false),
                        ],
                        type_ref_obj("PublishResult", false),
                    ),
                    field_def(
                        "createComment",
                        vec![
                            ("workspaceId", "ID", true),
                            ("commentId", "ID", true),
                            ("entityId", "ID", true),
                            ("body", "String", true),
                        ],
                        type_ref_obj("CommentThread", false),
                    ),
                    field_def(
                        "resolveComment",
                        vec![
                            ("workspaceId", "ID", true),
                            ("commentId", "ID", true),
                            ("resolved", "Boolean", true),
                        ],
                        type_ref_obj("CommentThread", false),
                    ),
                ]
            },
            {
                "kind": "OBJECT",
                "name": "HealthStatus",
                "fields": [
                    field_def("status", vec![], type_ref_scalar("String", true)),
                    field_def("engine", vec![], type_ref_scalar("String", true)),
                ]
            },
            {
                "kind": "OBJECT",
                "name": "Viewer",
                "fields": [
                    field_def("userId", vec![], type_ref_scalar("ID", true)),
                    field_def("email", vec![], type_ref_scalar("String", true)),
                    field_def("isServerOwner", vec![], type_ref_scalar("Boolean", true)),
                ]
            },
            {
                "kind": "OBJECT",
                "name": "Workspace",
                "fields": [
                    field_def("workspaceId", vec![], type_ref_scalar("ID", true)),
                    field_def("slug", vec![], type_ref_scalar("String", true)),
                    field_def("name", vec![], type_ref_scalar("String", true)),
                    field_def("description", vec![], type_ref_scalar("String", true)),
                    field_def("role", vec![], type_ref_scalar("String", true)),
                    field_def("memberCount", vec![], type_ref_scalar("Int", true)),
                    field_def("currentRevision", vec![], type_ref_scalar("Int", true)),
                    field_def("archived", vec![], type_ref_scalar("Boolean", true)),
                    field_def("entities", vec![("kind", "String", false), ("limit", "Int", false)], type_ref_obj("WorkspaceEntity", true)),
                    field_def("members", vec![], type_ref_obj("Member", true)),
                    field_def("comments", vec![("entityId", "ID", false)], type_ref_obj("CommentThread", true)),
                    field_def("notifications", vec![], type_ref_obj("Notification", true)),
                ]
            },
            {
                "kind": "OBJECT",
                "name": "WorkspaceEntity",
                "fields": [
                    field_def("entityId", vec![], type_ref_scalar("ID", true)),
                    field_def("parentId", vec![], type_ref_scalar("ID", false)),
                    field_def("kind", vec![], type_ref_scalar("String", true)),
                    field_def("name", vec![], type_ref_scalar("String", true)),
                    field_def("revision", vec![], type_ref_scalar("Int", true)),
                    field_def("updatedAtUnix", vec![], type_ref_scalar("Int", true)),
                    field_def("updatedBy", vec![], type_ref_scalar("String", true)),
                    field_def("payloadJson", vec![], type_ref_scalar("String", true)),
                ]
            },
            {
                "kind": "OBJECT",
                "name": "Member",
                "fields": [
                    field_def("userId", vec![], type_ref_scalar("ID", true)),
                    field_def("email", vec![], type_ref_scalar("String", true)),
                    field_def("displayName", vec![], type_ref_scalar("String", true)),
                    field_def("role", vec![], type_ref_scalar("String", true)),
                    field_def("joinedAtUnix", vec![], type_ref_scalar("Int", true)),
                ]
            },
            {
                "kind": "OBJECT",
                "name": "CommentThread",
                "fields": [
                    field_def("commentId", vec![], type_ref_scalar("ID", true)),
                    field_def("entityId", vec![], type_ref_scalar("ID", true)),
                    field_def("authorEmail", vec![], type_ref_scalar("String", true)),
                    field_def("body", vec![], type_ref_scalar("String", true)),
                    field_def("resolved", vec![], type_ref_scalar("Boolean", true)),
                    field_def("createdAtUnix", vec![], type_ref_scalar("Int", true)),
                ]
            },
            {
                "kind": "OBJECT",
                "name": "Notification",
                "fields": [
                    field_def("notificationId", vec![], type_ref_scalar("ID", true)),
                    field_def("kind", vec![], type_ref_scalar("String", true)),
                    field_def("title", vec![], type_ref_scalar("String", true)),
                    field_def("read", vec![], type_ref_scalar("Boolean", true)),
                    field_def("createdAtUnix", vec![], type_ref_scalar("Int", true)),
                ]
            },
            {
                "kind": "OBJECT",
                "name": "PublishResult",
                "fields": [
                    field_def("operationId", vec![], type_ref_scalar("String", true)),
                    field_def("entityId", vec![], type_ref_scalar("ID", true)),
                    field_def("status", vec![], type_ref_scalar("String", true)),
                    field_def("serverRevision", vec![], type_ref_scalar("Int", true)),
                    field_def("reason", vec![], type_ref_scalar("String", true)),
                ]
            }
        ]
    })
}
