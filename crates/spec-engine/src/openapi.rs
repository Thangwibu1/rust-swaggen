//! Native OpenAPI 3.0/3.1 JSON & YAML parser, validator, internal $ref resolver,
//! documentation model, stable operation identity, linked-request generator & diff (P03).
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use swaggen_domain::workspace::{HttpMethod, KeyValueRow, RequestBody, RequestItem, new_uid};
use yaml_rust2::{Yaml, YamlLoader};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecFormat {
    Json,
    Yaml,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenApiVersion {
    V3_0(String),
    V3_1(String),
    Unsupported(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueSeverity {
    Error,
    Warning,
    UnsupportedKeyword,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub severity: IssueSeverity,
    pub pointer: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecParam {
    pub name: String,
    pub location: String,
    pub required: bool,
    pub nullable: bool,
    pub schema_type: String,
    pub default_value: Option<String>,
    pub enum_values: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecBodySummary {
    pub required: bool,
    pub content_types: Vec<String>,
    pub example_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecResponseSummary {
    pub status: String,
    pub description: String,
    pub content_types: Vec<String>,
    pub example_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecOperation {
    /// Stable internal identity preserved across path/summary edits (DC04).
    pub operation_uid: String,
    pub operation_id: Option<String>,
    pub method: HttpMethod,
    pub path: String,
    pub tags: Vec<String>,
    pub summary: String,
    pub description: String,
    pub deprecated: bool,
    pub parameters: Vec<SpecParam>,
    pub request_body: Option<SpecBodySummary>,
    pub responses: Vec<SpecResponseSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecPropertySummary {
    pub name: String,
    pub schema_type: String,
    pub required: bool,
    pub nullable: bool,
    pub constraints: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecSchemaSummary {
    pub name: String,
    pub schema_type: String,
    pub properties: Vec<SpecPropertySummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecDocument {
    pub id: String,
    pub name: String,
    pub format: SpecFormat,
    /// Raw text is preserved even when parsing or validation fails (OA02).
    pub raw_text: String,
    pub version: Option<OpenApiVersion>,
    pub title: String,
    pub api_version: String,
    pub servers: Vec<String>,
    pub issues: Vec<ValidationIssue>,
    pub operations: Vec<SpecOperation>,
    pub schemas: Vec<SpecSchemaSummary>,
    pub security_schemes: Vec<(String, String)>,
}

impl SpecDocument {
    pub fn is_strictly_valid(&self) -> bool {
        matches!(
            self.version,
            Some(OpenApiVersion::V3_0(_) | OpenApiVersion::V3_1(_))
        ) && self.issues.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        !matches!(
            self.version,
            Some(OpenApiVersion::V3_0(_) | OpenApiVersion::V3_1(_))
        ) || self
            .issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Error)
    }

    pub fn search(&self, query: &str) -> (Vec<&SpecOperation>, Vec<&SpecSchemaSummary>) {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return (
                self.operations.iter().collect(),
                self.schemas.iter().collect(),
            );
        }
        let ops = self
            .operations
            .iter()
            .filter(|op| {
                op.path.to_ascii_lowercase().contains(&q)
                    || op.summary.to_ascii_lowercase().contains(&q)
                    || op.description.to_ascii_lowercase().contains(&q)
                    || op
                        .operation_id
                        .as_deref()
                        .unwrap_or("")
                        .to_ascii_lowercase()
                        .contains(&q)
                    || op.tags.iter().any(|t| t.to_ascii_lowercase().contains(&q))
            })
            .collect();
        let schemas = self
            .schemas
            .iter()
            .filter(|s| {
                s.name.to_ascii_lowercase().contains(&q)
                    || s.properties
                        .iter()
                        .any(|p| p.name.to_ascii_lowercase().contains(&q))
            })
            .collect();
        (ops, schemas)
    }
}

const UNSUPPORTED_KEYWORDS: &[&str] = &[
    "$dynamicRef",
    "$dynamicAnchor",
    "unevaluatedProperties",
    "unevaluatedItems",
    "patternProperties",
    "dependentSchemas",
    "dependentRequired",
    "contains",
    "minContains",
    "maxContains",
    "propertyNames",
    "if",
    "then",
    "else",
    "not",
];

pub fn parse_and_validate(
    id: impl Into<String>,
    name: impl Into<String>,
    raw_text: &str,
    previous_ops: &[SpecOperation],
) -> SpecDocument {
    let id = id.into();
    let name = name.into();
    let trimmed = raw_text.trim_start();
    let (format, parsed) = if trimmed.starts_with('{') {
        (
            SpecFormat::Json,
            serde_json::from_str::<Value>(raw_text).map_err(|e| format!("JSON syntax error: {e}")),
        )
    } else {
        (SpecFormat::Yaml, parse_yaml_value(raw_text))
    };

    let mut issues = Vec::new();
    let root = match parsed {
        Ok(v) => v,
        Err(msg) => {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Error,
                pointer: "$".into(),
                message: msg,
            });
            return SpecDocument {
                id,
                name: name.clone(),
                format,
                raw_text: raw_text.to_owned(),
                version: None,
                title: name,
                api_version: String::new(),
                servers: Vec::new(),
                issues,
                operations: Vec::new(),
                schemas: Vec::new(),
                security_schemes: Vec::new(),
            };
        }
    };

    let Some(root_obj) = root.as_object() else {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            pointer: "$".into(),
            message: "OpenAPI root must be an object/mapping".into(),
        });
        return SpecDocument {
            id,
            name: name.clone(),
            format,
            raw_text: raw_text.to_owned(),
            version: None,
            title: name,
            api_version: String::new(),
            servers: Vec::new(),
            issues,
            operations: Vec::new(),
            schemas: Vec::new(),
            security_schemes: Vec::new(),
        };
    };

    // Version check (OA01).
    let version = if let Some(v) = root_obj.get("openapi").and_then(Value::as_str) {
        if v.starts_with("3.0.") {
            Some(OpenApiVersion::V3_0(v.to_owned()))
        } else if v.starts_with("3.1.") {
            Some(OpenApiVersion::V3_1(v.to_owned()))
        } else {
            issues.push(ValidationIssue {
                severity: IssueSeverity::Error,
                pointer: "#/openapi".into(),
                message: format!("Unsupported OpenAPI version '{v}' (supported: 3.0.x, 3.1.x)"),
            });
            Some(OpenApiVersion::Unsupported(v.to_owned()))
        }
    } else if let Some(sw) = root_obj.get("swagger").and_then(Value::as_str) {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            pointer: "#/swagger".into(),
            message: format!(
                "Swagger {sw} requires conversion to OpenAPI 3.0/3.1 before full support"
            ),
        });
        Some(OpenApiVersion::Unsupported(format!("swagger:{sw}")))
    } else {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            pointer: "#/openapi".into(),
            message: "Missing required 'openapi' field".into(),
        });
        None
    };

    let is_3_1 = matches!(version, Some(OpenApiVersion::V3_1(_)));

    let info = root_obj.get("info").and_then(Value::as_object);
    let title = info
        .and_then(|i| i.get("title"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let api_version = info
        .and_then(|i| i.get("version"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if title.is_empty() {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            pointer: "#/info/title".into(),
            message: "Missing required 'info.title'".into(),
        });
    }
    if api_version.is_empty() {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            pointer: "#/info/version".into(),
            message: "Missing required 'info.version'".into(),
        });
    }

    let servers: Vec<String> = root_obj
        .get("servers")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.get("url").and_then(Value::as_str).map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default();

    // Scan entire document for $ref validity and unsupported schema keywords (OA03, OA06, O15, O23).
    let mut visited_refs = BTreeSet::new();
    scan_refs_and_keywords(&root, &root, "$", &mut visited_refs, &mut issues);

    // Parse components/schemas & securitySchemes.
    let mut schemas = Vec::new();
    let mut security_schemes = Vec::new();
    if let Some(comp) = root_obj.get("components").and_then(Value::as_object) {
        if let Some(smap) = comp.get("schemas").and_then(Value::as_object) {
            for (sname, sval) in smap {
                schemas.push(summarize_schema(sname, sval, &root, is_3_1));
            }
        }
        if let Some(sec) = comp.get("securitySchemes").and_then(Value::as_object) {
            for (k, v) in sec {
                let kind = v
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_owned();
                security_schemes.push((k.clone(), kind));
            }
        }
    }

    // Parse paths & operations, enforcing unique operationId and checking path template params (OA03, OA04).
    let mut operations = Vec::new();
    let mut seen_op_ids = BTreeSet::new();
    if let Some(paths) = root_obj.get("paths").and_then(Value::as_object) {
        for (path_key, path_item) in paths {
            if !path_key.starts_with('/') {
                issues.push(ValidationIssue {
                    severity: IssueSeverity::Error,
                    pointer: format!("#/paths/{path_key}"),
                    message: format!("Path '{path_key}' must start with '/'"),
                });
            }
            let Some(path_obj) = path_item.as_object() else {
                continue;
            };
            let path_level_params = parse_param_list(path_obj.get("parameters"), &root, is_3_1);
            let template_params = extract_path_template_params(path_key);

            for (method_key, op_val) in path_obj {
                let Some(method) = HttpMethod::parse(method_key) else {
                    continue;
                };
                let Some(op_obj) = op_val.as_object() else {
                    continue;
                };
                let operation_id = op_obj
                    .get("operationId")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                if let Some(ref oid) = operation_id
                    && !seen_op_ids.insert(oid.clone())
                {
                    issues.push(ValidationIssue {
                        severity: IssueSeverity::Error,
                        pointer: format!("#/paths/{path_key}/{method_key}/operationId"),
                        message: format!("Duplicate operationId '{oid}' (O04)"),
                    });
                }

                let mut params = path_level_params.clone();
                for p in parse_param_list(op_obj.get("parameters"), &root, is_3_1) {
                    params.retain(|existing| {
                        !(existing.name == p.name && existing.location == p.location)
                    });
                    params.push(p);
                }
                for tp in &template_params {
                    if !params.iter().any(|p| p.location == "path" && p.name == *tp) {
                        issues.push(ValidationIssue {
                            severity: IssueSeverity::Error,
                            pointer: format!("#/paths/{path_key}/{method_key}/parameters"),
                            message: format!(
                                "Path template parameter '{{{tp}}}' is not declared in parameters"
                            ),
                        });
                    }
                }

                let tags = op_obj
                    .get("tags")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                let summary = op_obj
                    .get("summary")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let description = op_obj
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let deprecated = op_obj
                    .get("deprecated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let request_body = summarize_request_body(op_obj.get("requestBody"), &root);
                let responses = summarize_responses(op_obj.get("responses"), &root);

                let stable_uid =
                    match_previous_uid(previous_ops, operation_id.as_deref(), method, path_key);

                operations.push(SpecOperation {
                    operation_uid: stable_uid,
                    operation_id,
                    method,
                    path: path_key.clone(),
                    tags,
                    summary,
                    description,
                    deprecated,
                    parameters: params,
                    request_body,
                    responses,
                });
            }
        }
    } else if root_obj.get("openapi").is_some() {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Warning,
            pointer: "#/paths".into(),
            message: "Document has no 'paths' object".into(),
        });
    }

    SpecDocument {
        id,
        name: name.clone(),
        format,
        raw_text: raw_text.to_owned(),
        version,
        title: if title.is_empty() { name } else { title },
        api_version,
        servers,
        issues,
        operations,
        schemas,
        security_schemes,
    }
}

fn match_previous_uid(
    previous_ops: &[SpecOperation],
    operation_id: Option<&str>,
    method: HttpMethod,
    path: &str,
) -> String {
    if let Some(oid) = operation_id
        && let Some(prev) = previous_ops
            .iter()
            .find(|o| o.operation_id.as_deref() == Some(oid))
    {
        return prev.operation_uid.clone();
    }
    if let Some(prev) = previous_ops
        .iter()
        .find(|o| o.method == method && o.path == path)
    {
        return prev.operation_uid.clone();
    }
    new_uid()
}

pub fn format_spec_text(raw: &str, target_format: SpecFormat) -> Result<String, String> {
    let val = if raw.trim_start().starts_with('{') {
        serde_json::from_str::<Value>(raw).map_err(|e| e.to_string())?
    } else {
        parse_yaml_value(raw)?
    };
    match target_format {
        SpecFormat::Json => serde_json::to_string_pretty(&val).map_err(|e| e.to_string()),
        SpecFormat::Yaml => Ok(value_to_yaml(&val, 0)),
    }
}

pub fn find_replace_spec(raw: &str, find: &str, replace: &str) -> (String, usize) {
    if find.is_empty() {
        return (raw.to_owned(), 0);
    }
    let count = raw.matches(find).count();
    (raw.replace(find, replace), count)
}

/// Adds an endpoint to an OpenAPI JSON/YAML document, rejecting duplicate (method, path) (OA04).
pub fn add_endpoint_to_spec(
    raw: &str,
    format: SpecFormat,
    path: &str,
    method: HttpMethod,
    operation_id: &str,
    summary: &str,
) -> Result<String, String> {
    if !path.starts_with('/') {
        return Err("Endpoint path must start with '/'".into());
    }
    let mut val = if raw.trim_start().starts_with('{') {
        serde_json::from_str::<Value>(raw).map_err(|e| e.to_string())?
    } else {
        parse_yaml_value(raw)?
    };
    let obj = val
        .as_object_mut()
        .ok_or_else(|| "OpenAPI root must be an object".to_owned())?;
    let paths = obj
        .entry("paths")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| "'paths' must be an object".to_owned())?;
    let path_item = paths
        .entry(path.to_owned())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| "Path entry must be an object".to_owned())?;
    let mkey = method.as_str().to_ascii_lowercase();
    if path_item.contains_key(&mkey) {
        return Err(format!(
            "Duplicate operation: {} {path} already exists (O04)",
            method.as_str()
        ));
    }
    let mut op = Map::new();
    if !operation_id.trim().is_empty() {
        op.insert(
            "operationId".into(),
            Value::String(operation_id.trim().to_owned()),
        );
    }
    op.insert("summary".into(), Value::String(summary.to_owned()));
    let mut responses = Map::new();
    let mut ok_resp = Map::new();
    ok_resp.insert(
        "description".into(),
        Value::String("Successful response".into()),
    );
    responses.insert("200".into(), Value::Object(ok_resp));
    op.insert("responses".into(), Value::Object(responses));
    path_item.insert(mkey, Value::Object(op));

    match format {
        SpecFormat::Json => serde_json::to_string_pretty(&val).map_err(|e| e.to_string()),
        SpecFormat::Yaml => Ok(value_to_yaml(&val, 0)),
    }
}

/// Generates a linked RequestItem from an OpenAPI operation (DC03, DC05).
pub fn generate_linked_request(
    doc: &SpecDocument,
    op: &SpecOperation,
    collection_id: &str,
) -> RequestItem {
    let base = doc
        .servers
        .first()
        .map(String::as_str)
        .unwrap_or("{{base_url}}")
        .trim_end_matches('/');
    let mut req = RequestItem::new(
        collection_id,
        if op.summary.is_empty() {
            op.operation_id
                .clone()
                .unwrap_or_else(|| format!("{} {}", op.method.as_str(), op.path))
        } else {
            op.summary.clone()
        },
        op.method,
        format!("{base}{}", op.path),
    );
    req.description = op.description.clone();
    req.tags = op.tags.clone();
    req.linked_operation_uid = Some(op.operation_uid.clone());
    for p in &op.parameters {
        let val = p.default_value.clone().unwrap_or_default();
        match p.location.as_str() {
            "path" => req.path_params.push(KeyValueRow::new(&p.name, val)),
            "query" => req.query_params.push(KeyValueRow::new(&p.name, val)),
            "header" => req.headers.push(KeyValueRow::new(&p.name, val)),
            _ => {}
        }
    }
    if let Some(ref body) = op.request_body
        && let Some(ref ex) = body.example_json
    {
        req.body = RequestBody::Json(ex.clone());
    }
    req
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecDiffEntry {
    pub operation_uid: String,
    pub summary: String,
    pub kind: String,
    pub detail: String,
}

pub fn diff_and_sync_linked_requests(
    old_ops: &[SpecOperation],
    new_doc: &SpecDocument,
    requests: &mut [RequestItem],
) -> Vec<SpecDiffEntry> {
    let mut diffs = Vec::new();
    let new_by_uid: BTreeMap<&str, &SpecOperation> = new_doc
        .operations
        .iter()
        .map(|o| (o.operation_uid.as_str(), o))
        .collect();
    let old_by_uid: BTreeMap<&str, &SpecOperation> = old_ops
        .iter()
        .map(|o| (o.operation_uid.as_str(), o))
        .collect();

    for new_op in &new_doc.operations {
        match old_by_uid.get(new_op.operation_uid.as_str()) {
            None => diffs.push(SpecDiffEntry {
                operation_uid: new_op.operation_uid.clone(),
                summary: format!("{} {}", new_op.method.as_str(), new_op.path),
                kind: "added".into(),
                detail: "New operation added to specification".into(),
            }),
            Some(old_op) => {
                if old_op.path != new_op.path || old_op.method != new_op.method {
                    diffs.push(SpecDiffEntry {
                        operation_uid: new_op.operation_uid.clone(),
                        summary: new_op.summary.clone(),
                        kind: "path_changed".into(),
                        detail: format!(
                            "{} {} -> {} {}",
                            old_op.method.as_str(),
                            old_op.path,
                            new_op.method.as_str(),
                            new_op.path
                        ),
                    });
                }
            }
        }
    }
    for old_op in old_ops {
        if !new_by_uid.contains_key(old_op.operation_uid.as_str()) {
            diffs.push(SpecDiffEntry {
                operation_uid: old_op.operation_uid.clone(),
                summary: format!("{} {}", old_op.method.as_str(), old_op.path),
                kind: "removed".into(),
                detail: "Operation removed from specification; linked requests retained for review"
                    .into(),
            });
        }
    }

    // Update linked requests preserving local auth/headers/token variables (DC04 / DC06).
    let base = new_doc
        .servers
        .first()
        .map(String::as_str)
        .unwrap_or("{{base_url}}")
        .trim_end_matches('/');
    for req in requests.iter_mut() {
        let Some(ref uid) = req.linked_operation_uid else {
            continue;
        };
        if let Some(new_op) = new_by_uid.get(uid.as_str()) {
            req.method = new_op.method;
            req.url = format!("{base}{}", new_op.path);
            for p in &new_op.parameters {
                if p.location == "query" && !req.query_params.iter().any(|q| q.key == p.name) {
                    req.query_params.push(KeyValueRow::new(
                        &p.name,
                        p.default_value.clone().unwrap_or_default(),
                    ));
                } else if p.location == "path" && !req.path_params.iter().any(|pp| pp.key == p.name)
                {
                    req.path_params.push(KeyValueRow::new(
                        &p.name,
                        p.default_value.clone().unwrap_or_default(),
                    ));
                }
            }
            req.dirty_vs_shared = true;
        }
    }
    diffs
}

pub fn resolve_internal_ref<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let pointer = reference.strip_prefix('#')?;
    if pointer.is_empty() {
        return Some(root);
    }
    let path = pointer.strip_prefix('/')?;
    let mut current = root;
    for raw_seg in path.split('/') {
        let seg = raw_seg.replace("~1", "/").replace("~0", "~");
        current = current.get(&seg)?;
    }
    Some(current)
}

fn scan_refs_and_keywords(
    root: &Value,
    node: &Value,
    pointer: &str,
    visited_refs: &mut BTreeSet<String>,
    issues: &mut Vec<ValidationIssue>,
) {
    match node {
        Value::Object(map) => {
            if let Some(ref_str) = map.get("$ref").and_then(Value::as_str) {
                if ref_str.starts_with('#') {
                    if resolve_internal_ref(root, ref_str).is_none() {
                        issues.push(ValidationIssue {
                            severity: IssueSeverity::Error,
                            pointer: format!("{pointer}/$ref"),
                            message: format!("Missing internal reference '{ref_str}' (O15)"),
                        });
                    } else if visited_refs.insert(ref_str.to_owned())
                        && let Some(target) = resolve_internal_ref(root, ref_str)
                    {
                        scan_refs_and_keywords(root, target, ref_str, visited_refs, issues);
                    }
                } else {
                    issues.push(ValidationIssue {
                        severity: IssueSeverity::Warning,
                        pointer: format!("{pointer}/$ref"),
                        message: format!(
                            "External reference '{ref_str}' blocked by default privacy/offline policy (O15)"
                        ),
                    });
                }
            }
            for (k, v) in map {
                if UNSUPPORTED_KEYWORDS.contains(&k.as_str()) {
                    issues.push(ValidationIssue {
                        severity: IssueSeverity::UnsupportedKeyword,
                        pointer: format!("{pointer}/{k}"),
                        message: format!(
                            "Unsupported JSON Schema keyword '{k}' (cannot assert valid · O23)"
                        ),
                    });
                }
                scan_refs_and_keywords(root, v, &format!("{pointer}/{k}"), visited_refs, issues);
            }
        }
        Value::Array(arr) => {
            for (idx, item) in arr.iter().enumerate() {
                scan_refs_and_keywords(
                    root,
                    item,
                    &format!("{pointer}[{idx}]"),
                    visited_refs,
                    issues,
                );
            }
        }
        _ => {}
    }
}

fn resolve_maybe_ref<'a>(val: &'a Value, root: &'a Value) -> &'a Value {
    let mut cur = val;
    for _ in 0..8 {
        if let Some(r) = cur.get("$ref").and_then(Value::as_str)
            && let Some(target) = resolve_internal_ref(root, r)
        {
            cur = target;
            continue;
        }
        break;
    }
    cur
}

fn summarize_schema(name: &str, raw: &Value, root: &Value, is_3_1: bool) -> SpecSchemaSummary {
    let obj = resolve_maybe_ref(raw, root);
    let (schema_type, _) = extract_type_and_nullable(obj, is_3_1);
    let required_set: BTreeSet<String> = obj
        .get("required")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let mut properties = Vec::new();
    if let Some(props) = obj.get("properties").and_then(Value::as_object) {
        for (pname, pval) in props {
            let p_resolved = resolve_maybe_ref(pval, root);
            let (ptype, nullable) = extract_type_and_nullable(p_resolved, is_3_1);
            let mut constraints = Vec::new();
            if let Some(fmt) = p_resolved.get("format").and_then(Value::as_str) {
                constraints.push(format!("format={fmt}"));
            }
            if let Some(min_l) = p_resolved.get("minLength").and_then(Value::as_u64) {
                constraints.push(format!("minLength={min_l}"));
            }
            if let Some(max_l) = p_resolved.get("maxLength").and_then(Value::as_u64) {
                constraints.push(format!("maxLength={max_l}"));
            }
            properties.push(SpecPropertySummary {
                name: pname.clone(),
                schema_type: ptype,
                required: required_set.contains(pname),
                nullable,
                constraints: constraints.join(", "),
            });
        }
    }
    SpecSchemaSummary {
        name: name.to_owned(),
        schema_type,
        properties,
    }
}

fn extract_type_and_nullable(schema: &Value, is_3_1: bool) -> (String, bool) {
    if is_3_1 && let Some(arr) = schema.get("type").and_then(Value::as_array) {
        let types: Vec<&str> = arr.iter().filter_map(Value::as_str).collect();
        let nullable = types.contains(&"null");
        let non_null: Vec<&str> = types.into_iter().filter(|t| *t != "null").collect();
        return (
            if non_null.is_empty() {
                "null".into()
            } else {
                non_null.join(" | ")
            },
            nullable,
        );
    }
    let stype = schema
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("object")
        .to_owned();
    let nullable = schema
        .get("nullable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    (stype, nullable)
}

fn parse_param_list(list: Option<&Value>, root: &Value, is_3_1: bool) -> Vec<SpecParam> {
    let Some(arr) = list.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in arr {
        let p = resolve_maybe_ref(item, root);
        let Some(name) = p.get("name").and_then(Value::as_str) else {
            continue;
        };
        let location = p
            .get("in")
            .and_then(Value::as_str)
            .unwrap_or("query")
            .to_owned();
        let required =
            p.get("required").and_then(Value::as_bool).unwrap_or(false) || location == "path";
        let schema = p
            .get("schema")
            .map(|s| resolve_maybe_ref(s, root))
            .unwrap_or(&Value::Null);
        let (schema_type, nullable) = extract_type_and_nullable(schema, is_3_1);
        let default_value = schema.get("default").map(|d| match d {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        });
        let enum_values = schema
            .get("enum")
            .and_then(Value::as_array)
            .map(|e| {
                e.iter()
                    .map(|v| match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let description = p
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        out.push(SpecParam {
            name: name.to_owned(),
            location,
            required,
            nullable,
            schema_type,
            default_value,
            enum_values,
            description,
        });
    }
    out
}

fn summarize_request_body(raw: Option<&Value>, root: &Value) -> Option<SpecBodySummary> {
    let body = resolve_maybe_ref(raw?, root);
    let required = body
        .get("required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let content = body.get("content").and_then(Value::as_object)?;
    let content_types: Vec<String> = content.keys().cloned().collect();
    let example_json = content
        .get("application/json")
        .or_else(|| content.values().next())
        .and_then(|media| {
            media
                .get("example")
                .or_else(|| {
                    media
                        .get("examples")
                        .and_then(Value::as_object)
                        .and_then(|m| m.values().next())
                        .and_then(|e| e.get("value"))
                })
                .cloned()
                .or_else(|| {
                    media
                        .get("schema")
                        .map(|s| synthesize_example_from_schema(s, root, 0))
                })
                .and_then(|v| serde_json::to_string_pretty(&v).ok())
        });
    Some(SpecBodySummary {
        required,
        content_types,
        example_json,
    })
}

fn summarize_responses(raw: Option<&Value>, root: &Value) -> Vec<SpecResponseSummary> {
    let Some(map) = raw.and_then(Value::as_object) else {
        return Vec::new();
    };
    map.iter()
        .map(|(status, val)| {
            let resp = resolve_maybe_ref(val, root);
            let description = resp
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let content = resp.get("content").and_then(Value::as_object);
            let content_types = content
                .map(|c| c.keys().cloned().collect())
                .unwrap_or_default();
            let example_json = content.and_then(|c| {
                c.get("application/json")
                    .or_else(|| c.values().next())
                    .and_then(|m| {
                        m.get("example").cloned().or_else(|| {
                            m.get("schema")
                                .map(|s| synthesize_example_from_schema(s, root, 0))
                        })
                    })
                    .and_then(|v| serde_json::to_string_pretty(&v).ok())
            });
            SpecResponseSummary {
                status: status.clone(),
                description,
                content_types,
                example_json,
            }
        })
        .collect()
}

fn synthesize_example_from_schema(schema: &Value, root: &Value, depth: usize) -> Value {
    if depth >= 5 {
        return Value::Object(Map::new());
    }
    let resolved = resolve_maybe_ref(schema, root);
    if let Some(ex) = resolved.get("example") {
        return ex.clone();
    }
    if let Some(def) = resolved.get("default") {
        return def.clone();
    }
    if let Some(en) = resolved
        .get("enum")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
    {
        return en.clone();
    }
    if let Some(one_of) = resolved
        .get("oneOf")
        .or_else(|| resolved.get("anyOf"))
        .and_then(Value::as_array)
        .and_then(|a| a.first())
    {
        return synthesize_example_from_schema(one_of, root, depth + 1);
    }
    if let Some(all_of) = resolved.get("allOf").and_then(Value::as_array) {
        let mut merged = Map::new();
        for sub in all_of {
            if let Value::Object(m) = synthesize_example_from_schema(sub, root, depth + 1) {
                for (k, v) in m {
                    merged.insert(k, v);
                }
            }
        }
        return Value::Object(merged);
    }
    let ty = resolved
        .get("type")
        .and_then(|t| {
            t.as_str().or_else(|| {
                t.as_array()
                    .and_then(|a| a.iter().filter_map(Value::as_str).find(|s| *s != "null"))
            })
        })
        .unwrap_or_else(|| {
            if resolved.get("properties").is_some() {
                "object"
            } else if resolved.get("items").is_some() {
                "array"
            } else {
                "string"
            }
        });
    match ty {
        "integer" => Value::Number(0.into()),
        "number" => serde_json::json!(0.0),
        "boolean" => Value::Bool(true),
        "array" => {
            let item = resolved
                .get("items")
                .map(|it| synthesize_example_from_schema(it, root, depth + 1))
                .unwrap_or_else(|| Value::String("item".into()));
            Value::Array(vec![item])
        }
        "object" => {
            let mut map = Map::new();
            if let Some(props) = resolved.get("properties").and_then(Value::as_object) {
                for (k, v) in props {
                    map.insert(
                        k.clone(),
                        synthesize_example_from_schema(v, root, depth + 1),
                    );
                }
            }
            Value::Object(map)
        }
        _ => match resolved.get("format").and_then(Value::as_str) {
            Some("date-time") => Value::String("2026-01-01T00:00:00Z".into()),
            Some("date") => Value::String("2026-01-01".into()),
            Some("email") => Value::String("user@example.com".into()),
            Some("uuid") => Value::String("00000000-0000-0000-0000-000000000000".into()),
            Some("uri") | Some("url") => Value::String("https://example.com".into()),
            _ => Value::String("string".into()),
        },
    }
}

/// Universal API documentation parser supporting OpenAPI 3.0/3.1 (JSON/YAML), Swagger 2.0 (JSON/YAML),
/// Postman Collection v2.1/v2.0 JSON, and Swagger/ReDoc HTML with embedded spec JSON!
pub fn parse_api_documentation(
    id: impl Into<String>,
    name: impl Into<String>,
    raw_text: &str,
    previous_ops: &[SpecOperation],
) -> SpecDocument {
    let id_str = id.into();
    let name_str = name.into();
    let trimmed = raw_text.trim_start();
    let trimmed_low: String = trimmed
        .chars()
        .take(64)
        .collect::<String>()
        .to_ascii_lowercase();

    // 1. HTML containing embedded inline JSON spec (`spec: { ... }` or `<script type="application/json">`)
    if trimmed_low.starts_with("<!doctype html") || trimmed_low.starts_with("<html") {
        if let Some(embedded_json) = extract_inline_json_from_html(raw_text) {
            return parse_api_documentation(id_str, name_str, &embedded_json, previous_ops);
        }
        return parse_and_validate(id_str, name_str, raw_text, previous_ops);
    }

    let parsed = if trimmed.starts_with('{') {
        serde_json::from_str::<Value>(raw_text).ok()
    } else {
        parse_yaml_value(raw_text).ok()
    };

    if let Some(ref root) = parsed
        && let Some(obj) = root.as_object()
    {
        // 2. Swagger 2.0 JSON or YAML -> auto-upgrade cleanly to OpenAPI 3.0.3 JSON
        if obj.contains_key("swagger")
            && !obj.contains_key("openapi")
            && let Some(oa3_json) = swagger2_value_to_openapi3_json(root)
        {
            let mut doc = parse_and_validate(id_str, name_str, &oa3_json, previous_ops);
            if !doc.title.trim().is_empty() && doc.title != doc.name {
                doc.name = doc.title.clone();
            }
            return doc;
        }

        // 3. Postman Collection v2.1 / v2.0 JSON -> auto-convert to OpenAPI 3.0.3 JSON
        let is_postman = obj.contains_key("item")
            && obj
                .get("info")
                .and_then(|i| i.get("schema"))
                .and_then(Value::as_str)
                .is_some_and(|s| s.contains("getpostman.com"))
            || (obj.contains_key("item") && obj.get("info").and_then(|i| i.get("name")).is_some());
        if is_postman && !obj.contains_key("openapi") && !obj.contains_key("paths") {
            let oa3_json = postman_value_to_openapi3_json(root);
            let mut doc = parse_and_validate(id_str, name_str, &oa3_json, previous_ops);
            if !doc.title.trim().is_empty() {
                doc.name = doc.title.clone();
            }
            return doc;
        }
    }

    let mut doc = parse_and_validate(id_str, name_str, raw_text, previous_ops);
    if !doc.title.trim().is_empty()
        && (doc.name.eq_ignore_ascii_case("Imported URL Spec")
            || doc.name.eq_ignore_ascii_case("openapi.json")
            || doc.name.eq_ignore_ascii_case("swagger.json")
            || doc.name.eq_ignore_ascii_case("openapi.yaml"))
    {
        doc.name = doc.title.clone();
    }
    doc
}

/// Extracts Swagger UI initialization script URLs (e.g. `swagger-initializer.js`, `swagger-ui-init.js`)
/// from an HTML page so the fetcher can inspect external JS configs too.
pub fn extract_swagger_script_urls_from_html(html: &str, base_url: &str) -> Vec<String> {
    let mut raw = Vec::new();
    let mut search_from = 0usize;
    while let Some(pos) = html[search_from..].find("src=") {
        let abs = search_from + pos + 4;
        let slice = &html[abs..html.len().min(abs + 320)];
        if let Some(q) = extract_first_quoted_string(slice) {
            let low = q.to_ascii_lowercase();
            if (low.contains("swagger") || low.contains("redoc") || low.contains("openapi"))
                && low.ends_with(".js")
                && !low.contains("swagger-ui-bundle")
                && !low.contains("swagger-ui-standalone")
                && !raw.contains(&q)
            {
                raw.push(q);
            }
        }
        search_from = abs;
    }
    let Ok(base) = url::Url::parse(base_url) else {
        return raw;
    };
    let mut resolved = Vec::new();
    for cand in raw {
        let final_url = if cand.starts_with("http://") || cand.starts_with("https://") {
            cand
        } else if let Ok(joined) = base.join(&cand) {
            joined.into()
        } else {
            continue;
        };
        if !resolved.contains(&final_url) {
            resolved.push(final_url);
        }
    }
    resolved
}

/// Extracts candidate OpenAPI/Swagger JSON/YAML URLs from Swagger UI or ReDoc HTML/JS,
/// resolving relative paths cleanly against `base_url`.
pub fn extract_swagger_urls_from_html(html: &str, base_url: &str) -> Vec<String> {
    let mut raw_candidates: Vec<String> = Vec::new();
    for needle in [
        "url:",
        "url :",
        "\"url\":",
        "'url':",
        "configUrl:",
        "configUrl :",
        "\"configUrl\":",
        "spec-url=",
        "data-spec-url=",
        "Redoc.init(",
        "SwaggerUIBundle(",
    ] {
        let mut search_from = 0usize;
        while let Some(pos) = html[search_from..].find(needle) {
            let abs = search_from + pos + needle.len();
            let slice = &html[abs..html.len().min(abs + 420)];
            if let Some(q) = extract_first_quoted_string(slice)
                && (q.contains(".json")
                    || q.contains(".yaml")
                    || q.contains(".yml")
                    || q.contains("openapi")
                    || q.contains("swagger")
                    || q.contains("api-docs"))
                && !raw_candidates.contains(&q)
            {
                raw_candidates.push(q);
            }
            search_from = abs;
        }
    }

    // Also include well-known Swagger/OpenAPI endpoints on the same origin as fallbacks
    // ONLY when the HTML actually appears to be Swagger/ReDoc UI or has no discovered candidates.
    let html_low = html.to_ascii_lowercase();
    let looks_like_api_docs_ui = !raw_candidates.is_empty()
        || html_low.contains("swagger")
        || html_low.contains("redoc")
        || html_low.contains("openapi")
        || html_low.contains("rapidoc");
    if looks_like_api_docs_ui {
        for fallback in [
            "/openapi.json",
            "/api/v3/openapi.json",
            "/v3/api-docs",
            "/v2/api-docs",
            "/swagger/v1/swagger.json",
            "/swagger.json",
            "/openapi.yaml",
        ] {
            if !raw_candidates.iter().any(|c| c == fallback) {
                raw_candidates.push(fallback.to_owned());
            }
        }
    }

    let Ok(base) = url::Url::parse(base_url) else {
        return raw_candidates;
    };
    let mut resolved = Vec::new();
    for cand in raw_candidates {
        let final_url = if cand.starts_with("http://") || cand.starts_with("https://") {
            cand
        } else if let Ok(joined) = base.join(&cand) {
            joined.into()
        } else {
            continue;
        };
        if !resolved.contains(&final_url) {
            resolved.push(final_url);
        }
    }
    resolved
}

fn extract_first_quoted_string(s: &str) -> Option<String> {
    let trimmed = s.trim_start();
    let quote_pos = trimmed.find(['"', '\''])?;
    if quote_pos > 140 {
        return None;
    }
    let quote = trimmed.as_bytes()[quote_pos] as char;
    let rest = &trimmed[quote_pos + 1..];
    let end = rest.find(quote)?;
    let val = rest[..end].trim();
    if val.is_empty() || val.len() > 512 {
        None
    } else {
        Some(val.to_owned())
    }
}

fn extract_inline_json_from_html(html: &str) -> Option<String> {
    for marker in ["\"openapi\":", "\"swagger\":", "'openapi':"] {
        if let Some(idx) = html.find(marker) {
            let prefix = &html[..idx];
            if let Some(open_brace) = prefix.rfind('{') {
                let candidate = &html[open_brace..];
                let mut depth = 0i32;
                let mut in_str = false;
                let mut esc = false;
                for (offset, ch) in candidate.char_indices() {
                    if in_str {
                        if esc {
                            esc = false;
                        } else if ch == '\\' {
                            esc = true;
                        } else if ch == '"' {
                            in_str = false;
                        }
                        continue;
                    }
                    match ch {
                        '"' => in_str = true,
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                let json_slice = &candidate[..=offset];
                                if serde_json::from_str::<Value>(json_slice).is_ok() {
                                    return Some(json_slice.to_owned());
                                }
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    None
}

fn swagger2_value_to_openapi3_json(v: &Value) -> Option<String> {
    let title = v
        .pointer("/info/title")
        .and_then(Value::as_str)
        .unwrap_or("Swagger 2.0 API");
    let version = v
        .pointer("/info/version")
        .and_then(Value::as_str)
        .unwrap_or("1.0.0");
    let description = v
        .pointer("/info/description")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let host = v
        .get("host")
        .and_then(Value::as_str)
        .unwrap_or("api.example.test");
    let base_path = v.get("basePath").and_then(Value::as_str).unwrap_or("");
    let scheme = v
        .get("schemes")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(Value::as_str)
        .unwrap_or("https");

    let mut new_paths = Map::new();
    if let Some(paths) = v.get("paths").and_then(Value::as_object) {
        for (p, pitem) in paths {
            let mut new_pitem = Map::new();
            if let Some(obj) = pitem.as_object() {
                for (m, op) in obj {
                    let mut new_op = op.clone();
                    if let Some(op_obj) = new_op.as_object_mut() {
                        if let Some(params) =
                            op_obj.get("parameters").and_then(Value::as_array).cloned()
                        {
                            let mut kept = Vec::new();
                            for mut pr in params {
                                let loc = pr.get("in").and_then(Value::as_str).unwrap_or("");
                                if loc == "body" {
                                    let schema = pr
                                        .get("schema")
                                        .cloned()
                                        .unwrap_or(serde_json::json!({"type":"object"}));
                                    op_obj.insert(
                                        "requestBody".into(),
                                        serde_json::json!({
                                            "required": pr.get("required").and_then(Value::as_bool).unwrap_or(false),
                                            "content": { "application/json": { "schema": schema } }
                                        }),
                                    );
                                } else {
                                    // Lift Swagger 2.0 parameter `type`/`format`/`enum` into `schema` if missing
                                    if pr.get("schema").is_none()
                                        && let Some(pobj) = pr.as_object_mut()
                                    {
                                        let mut sch = Map::new();
                                        for k in ["type", "format", "enum", "default"] {
                                            if let Some(val) = pobj.get(k) {
                                                sch.insert(k.into(), val.clone());
                                            }
                                        }
                                        if !sch.is_empty() {
                                            pobj.insert("schema".into(), Value::Object(sch));
                                        }
                                    }
                                    kept.push(pr);
                                }
                            }
                            op_obj.insert("parameters".into(), Value::Array(kept));
                        }
                        if let Some(resps) =
                            op_obj.get_mut("responses").and_then(Value::as_object_mut)
                        {
                            for (_, rv) in resps.iter_mut() {
                                if let Some(robj) = rv.as_object_mut()
                                    && let Some(schema) = robj.remove("schema")
                                {
                                    robj.insert(
                                        "content".into(),
                                        serde_json::json!({"application/json":{"schema":schema}}),
                                    );
                                }
                            }
                        }
                    }
                    new_pitem.insert(m.clone(), new_op);
                }
            }
            new_paths.insert(p.clone(), Value::Object(new_pitem));
        }
    }
    let mut components = Map::new();
    if let Some(defs) = v.get("definitions") {
        components.insert("schemas".into(), defs.clone());
    }
    if let Some(sec_defs) = v.get("securityDefinitions") {
        components.insert("securitySchemes".into(), sec_defs.clone());
    }
    let converted = serde_json::json!({
        "openapi": "3.0.3",
        "info": { "title": title, "version": version, "description": description },
        "servers": [{ "url": format!("{scheme}://{host}{base_path}") }],
        "paths": Value::Object(new_paths),
        "components": Value::Object(components),
    });
    let text = serde_json::to_string_pretty(&converted).ok()?;
    Some(text.replace("#/definitions/", "#/components/schemas/"))
}

fn postman_value_to_openapi3_json(root: &Value) -> String {
    let title = root
        .pointer("/info/name")
        .and_then(Value::as_str)
        .unwrap_or("Postman API Collection");
    let description = root
        .pointer("/info/description")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut servers: Vec<String> = Vec::new();
    let mut paths_map: Map<String, Value> = Map::new();

    fn walk_pm(
        items: &[Value],
        tag: &str,
        servers: &mut Vec<String>,
        paths_map: &mut Map<String, Value>,
    ) {
        for item in items {
            if let Some(sub_items) = item.get("item").and_then(Value::as_array) {
                let next_tag = item
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .unwrap_or(tag);
                walk_pm(sub_items, next_tag, servers, paths_map);
                continue;
            }
            let Some(req) = item.get("request").and_then(Value::as_object) else {
                continue;
            };
            let req_name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let method = req
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("GET")
                .to_ascii_lowercase();
            let raw_url = match req.get("url") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Object(u)) => u
                    .get("raw")
                    .and_then(Value::as_str)
                    .unwrap_or("/")
                    .to_owned(),
                _ => "/".into(),
            };
            let (server_url, path_only) = split_postman_url_for_openapi(&raw_url);
            if !server_url.is_empty() && !servers.contains(&server_url) {
                servers.push(server_url);
            }

            let mut params = Vec::new();
            if let Some(url_obj) = req.get("url").and_then(Value::as_object) {
                if let Some(qlist) = url_obj.get("query").and_then(Value::as_array) {
                    for q in qlist {
                        if let Some(key) = q.get("key").and_then(Value::as_str)
                            && !key.is_empty()
                        {
                            params.push(serde_json::json!({
                                "name": key,
                                "in": "query",
                                "required": false,
                                "description": q.get("description").and_then(Value::as_str).unwrap_or(""),
                                "schema": {
                                    "type": "string",
                                    "default": q.get("value").and_then(Value::as_str).unwrap_or("")
                                }
                            }));
                        }
                    }
                }
                if let Some(vars) = url_obj.get("variable").and_then(Value::as_array) {
                    for v in vars {
                        if let Some(key) = v.get("key").and_then(Value::as_str)
                            && !key.is_empty()
                        {
                            params.push(serde_json::json!({
                                "name": key,
                                "in": "path",
                                "required": true,
                                "description": v.get("description").and_then(Value::as_str).unwrap_or(""),
                                "schema": {
                                    "type": "string",
                                    "default": v.get("value").and_then(Value::as_str).unwrap_or("")
                                }
                            }));
                        }
                    }
                }
            }
            for tp in extract_path_template_params(&path_only) {
                if !params
                    .iter()
                    .any(|p| p.get("name").and_then(Value::as_str) == Some(tp.as_str()))
                {
                    params.push(serde_json::json!({
                        "name": tp,
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" }
                    }));
                }
            }
            if let Some(hdrs) = req.get("header").and_then(Value::as_array) {
                for h in hdrs {
                    if let Some(key) = h.get("key").and_then(Value::as_str)
                        && !key.is_empty()
                    {
                        params.push(serde_json::json!({
                            "name": key,
                            "in": "header",
                            "required": false,
                            "schema": {
                                "type": "string",
                                "default": h.get("value").and_then(Value::as_str).unwrap_or("")
                            }
                        }));
                    }
                }
            }

            let mut op_obj = Map::new();
            op_obj.insert("summary".into(), Value::String(req_name));
            op_obj.insert(
                "description".into(),
                Value::String(
                    req.get("description")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                ),
            );
            op_obj.insert("tags".into(), serde_json::json!([tag]));
            op_obj.insert("parameters".into(), Value::Array(params));

            if let Some(body) = req.get("body").and_then(Value::as_object)
                && let Some(raw_body) = body.get("raw").and_then(Value::as_str)
                && !raw_body.trim().is_empty()
            {
                let example_val = serde_json::from_str::<Value>(raw_body)
                    .unwrap_or_else(|_| Value::String(raw_body.to_owned()));
                op_obj.insert(
                    "requestBody".into(),
                    serde_json::json!({
                        "required": true,
                        "content": { "application/json": { "example": example_val } }
                    }),
                );
            }

            let mut resps = Map::new();
            if let Some(examples) = item.get("response").and_then(Value::as_array) {
                for ex in examples {
                    let code = ex
                        .get("code")
                        .and_then(Value::as_u64)
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "200".into());
                    let ex_name = ex
                        .get("status")
                        .or_else(|| ex.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("Response");
                    let body_str = ex.get("body").and_then(Value::as_str).unwrap_or("");
                    let body_val = serde_json::from_str::<Value>(body_str)
                        .unwrap_or_else(|_| Value::String(body_str.to_owned()));
                    resps.insert(
                        code,
                        serde_json::json!({
                            "description": ex_name,
                            "content": { "application/json": { "example": body_val } }
                        }),
                    );
                }
            }
            if resps.is_empty() {
                resps.insert(
                    "200".into(),
                    serde_json::json!({ "description": "Successful response" }),
                );
            }
            op_obj.insert("responses".into(), Value::Object(resps));

            let p_entry = paths_map
                .entry(path_only)
                .or_insert_with(|| Value::Object(Map::new()));
            if let Some(pmap) = p_entry.as_object_mut() {
                pmap.insert(method, Value::Object(op_obj));
            }
        }
    }

    if let Some(items) = root.get("item").and_then(Value::as_array) {
        walk_pm(items, "Collection", &mut servers, &mut paths_map);
    }
    if servers.is_empty() {
        servers.push("{{base_url}}".into());
    }
    let srv_objs: Vec<Value> = servers
        .into_iter()
        .map(|s| serde_json::json!({ "url": s }))
        .collect();
    let doc = serde_json::json!({
        "openapi": "3.0.3",
        "info": { "title": title, "version": "1.0.0", "description": description },
        "servers": srv_objs,
        "paths": Value::Object(paths_map)
    });
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}

fn split_postman_url_for_openapi(raw: &str) -> (String, String) {
    let before_q = raw.split(['?', '#']).next().unwrap_or(raw).trim();
    let (host, raw_path) = if let Some(scheme_pos) = before_q.find("://") {
        let after_scheme = &before_q[scheme_pos + 3..];
        if let Some(slash) = after_scheme.find('/') {
            (
                before_q[..scheme_pos + 3 + slash].to_owned(),
                &after_scheme[slash..],
            )
        } else {
            (before_q.to_owned(), "/")
        }
    } else if before_q.starts_with("{{")
        && let Some(end_var) = before_q.find("}}")
    {
        (before_q[..end_var + 2].to_owned(), &before_q[end_var + 2..])
    } else {
        (String::new(), before_q)
    };
    let mut normalized = if raw_path.starts_with('/') {
        raw_path.to_owned()
    } else {
        format!("/{raw_path}")
    };
    // Convert Postman `:id` path vars into OpenAPI `{id}` path template params
    let segs: Vec<String> = normalized
        .split('/')
        .map(|seg| {
            if let Some(name) = seg.strip_prefix(':') {
                format!("{{{name}}}")
            } else {
                seg.to_owned()
            }
        })
        .collect();
    normalized = segs.join("/");
    (host, normalized)
}

fn extract_path_template_params(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            break;
        };
        let name = after[..end].trim();
        if !name.is_empty() {
            out.push(name.to_owned());
        }
        rest = &after[end + 1..];
    }
    out
}

pub fn parse_yaml_value(raw: &str) -> Result<Value, String> {
    let docs = YamlLoader::load_from_str(raw).map_err(|e| format!("YAML syntax error: {e}"))?;
    let Some(first) = docs.into_iter().next() else {
        return Err("Empty YAML document".into());
    };
    yaml_to_json(&first)
}

fn yaml_to_json(y: &Yaml) -> Result<Value, String> {
    Ok(match y {
        Yaml::Null | Yaml::BadValue => Value::Null,
        Yaml::Boolean(b) => Value::Bool(*b),
        Yaml::Integer(i) => Value::Number((*i).into()),
        Yaml::Real(s) => s
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(s.clone())),
        Yaml::String(s) => Value::String(s.clone()),
        Yaml::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(yaml_to_json(item)?);
            }
            Value::Array(out)
        }
        Yaml::Hash(map) => {
            let mut obj = Map::new();
            for (k, v) in map {
                let key = match k {
                    Yaml::String(s) => s.clone(),
                    Yaml::Integer(i) => i.to_string(),
                    Yaml::Boolean(b) => b.to_string(),
                    _ => return Err("YAML mapping key must be scalar".into()),
                };
                obj.insert(key, yaml_to_json(v)?);
            }
            Value::Object(obj)
        }
        Yaml::Alias(_) => return Err("Unsupported YAML alias".into()),
    })
}

pub fn value_to_yaml(val: &Value, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    match val {
        Value::Null => "null\n".into(),
        Value::Bool(b) => format!("{b}\n"),
        Value::Number(n) => format!("{n}\n"),
        Value::String(s) => {
            if s.contains('\n') || s.contains(':') || s.contains('#') || s.is_empty() {
                format!(
                    "{}\n",
                    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
                )
            } else {
                format!("{s}\n")
            }
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                return "[]\n".into();
            }
            let mut out = String::new();
            for item in arr {
                match item {
                    Value::Object(_) | Value::Array(_) => {
                        out.push_str(&format!("{pad}-\n"));
                        out.push_str(&value_to_yaml(item, indent + 1));
                    }
                    scalar => {
                        let rendered = value_to_yaml(scalar, 0);
                        out.push_str(&format!("{pad}- {rendered}"));
                    }
                }
            }
            out
        }
        Value::Object(map) => {
            if map.is_empty() {
                return "{}\n".into();
            }
            let mut out = String::new();
            for (k, v) in map {
                match v {
                    Value::Object(m) if !m.is_empty() => {
                        out.push_str(&format!("{pad}{k}:\n"));
                        out.push_str(&value_to_yaml(v, indent + 1));
                    }
                    Value::Array(a) if !a.is_empty() => {
                        out.push_str(&format!("{pad}{k}:\n"));
                        out.push_str(&value_to_yaml(v, indent + 1));
                    }
                    scalar => {
                        let rendered = value_to_yaml(scalar, 0);
                        out.push_str(&format!("{pad}{k}: {rendered}"));
                    }
                }
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_openapi_30_31_unsupported_keywords_and_missing_refs() {
        let yaml_31 = r#"
openapi: 3.1.0
info:
  title: DisNote API
  version: 1.0.0
servers:
  - url: https://api.example.test
paths:
  /tasks/{taskId}:
    get:
      operationId: getTask
      summary: Get a task
      tags: [Tasks]
      parameters:
        - name: taskId
          in: path
          required: true
          schema:
            type: ["string", "null"]
      responses:
        "200":
          description: OK
components:
  schemas:
    Task:
      type: object
      required: [id]
      properties:
        id:
          type: string
        note:
          type: ["string", "null"]
"#;
        let doc = parse_and_validate("s1", "DisNote", yaml_31, &[]);
        assert!(doc.is_strictly_valid(), "issues: {:?}", doc.issues);
        assert_eq!(doc.operations.len(), 1);
        assert!(doc.operations[0].parameters[0].nullable);
        assert!(
            doc.schemas[0]
                .properties
                .iter()
                .any(|p| p.name == "note" && p.nullable)
        );

        // Unsupported keyword must NOT report valid (OA03 / O23).
        let with_unsupported = r##"
openapi: 3.0.3
info: { title: X, version: "1.0" }
paths: {}
components:
  schemas:
    Bad:
      type: object
      unevaluatedProperties: false
      properties:
        x:
          $ref: "#/components/schemas/DoesNotExist"
"##;
        let bad = parse_and_validate("s2", "Bad", with_unsupported, &[]);
        assert!(!bad.is_strictly_valid());
        assert!(
            bad.issues
                .iter()
                .any(|i| i.severity == IssueSeverity::UnsupportedKeyword)
        );
        assert!(
            bad.issues
                .iter()
                .any(|i| i.message.contains("Missing internal reference"))
        );
    }

    #[test]
    fn renaming_path_preserves_stable_operation_uid_and_local_auth() {
        let v1 = r#"{
          "openapi": "3.0.3",
          "info": { "title": "API", "version": "1.0" },
          "servers": [{ "url": "https://api.example.test" }],
          "paths": {
            "/v1/tasks": {
              "get": { "operationId": "listTasks", "summary": "List tasks", "responses": { "200": { "description": "OK" } } }
            }
          }
        }"#;
        let doc1 = parse_and_validate("s1", "API", v1, &[]);
        let mut req = generate_linked_request(&doc1, &doc1.operations[0], "c1");
        req.auth = swaggen_domain::workspace::AuthConfig::Bearer {
            token: "{{my_local_token}}".into(),
        };
        let orig_uid = doc1.operations[0].operation_uid.clone();

        let v2 = v1.replace("/v1/tasks", "/v2/tasks");
        let doc2 = parse_and_validate("s1", "API", &v2, &doc1.operations);
        assert_eq!(doc2.operations[0].operation_uid, orig_uid);
        let mut reqs = vec![req];
        let diffs = diff_and_sync_linked_requests(&doc1.operations, &doc2, &mut reqs);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].kind, "path_changed");
        assert_eq!(reqs[0].url, "https://api.example.test/v2/tasks");
        assert!(matches!(
            reqs[0].auth,
            swaggen_domain::workspace::AuthConfig::Bearer { .. }
        ));
    }

    #[test]
    fn universal_docs_parser_handles_swagger2_postman_and_swagger_html() {
        let sw2 = r#"{
          "swagger": "2.0",
          "info": { "title": "Petstore V2", "version": "2.0.1" },
          "host": "petstore.swagger.io",
          "basePath": "/v2",
          "paths": {
            "/pets": {
              "get": { "tags": ["Pets"], "summary": "List pets", "responses": { "200": { "description": "OK" } } }
            }
          }
        }"#;
        let d_sw2 = parse_api_documentation("id1", "Imported URL Spec", sw2, &[]);
        assert_eq!(d_sw2.title, "Petstore V2");
        assert_eq!(d_sw2.operations.len(), 1);

        let pm = r#"{
          "info": { "name": "Orders Postman", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" },
          "item": [{
            "name": "Orders",
            "item": [{
              "name": "Create Order",
              "request": {
                "method": "POST",
                "url": "{{base_url}}/orders/:id?dry=true",
                "body": { "mode": "raw", "raw": "{\"item\":\"book\"}" }
              }
            }]
          }]
        }"#;
        let d_pm = parse_api_documentation("id2", "Orders", pm, &[]);
        assert_eq!(d_pm.title, "Orders Postman");
        assert_eq!(d_pm.operations.len(), 1);
        assert_eq!(d_pm.operations[0].path, "/orders/{id}");
        assert!(d_pm.operations[0].request_body.is_some());

        let html = r#"<!doctype html><html><script>SwaggerUIBundle({ url: "/api/v3/openapi.json" })</script></html>"#;
        let urls = extract_swagger_urls_from_html(html, "https://petstore3.swagger.io/");
        assert_eq!(urls[0], "https://petstore3.swagger.io/api/v3/openapi.json");
    }
}
