//! Version B advanced specification, contract validation, docs export, attachment bundling,
//! and deterministic Git project text tree codecs (OA07-OA10, DC07-DC10, IM07, IM09).
use crate::importers::CompatibilityReport;
use crate::openapi::{
    IssueSeverity, OpenApiVersion, SpecDocument, SpecFormat, SpecOperation, ValidationIssue,
    parse_and_validate,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use swaggen_domain::{
    security::RedactionPolicy,
    workspace::{HttpMethod, RequestItem, WorkspaceState},
};

/// OA07: Complex schema view handling allOf / oneOf / anyOf / enum / discriminator + cycle cap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaTreeNode {
    pub label: String,
    pub kind: String,
    pub enum_values: Vec<String>,
    pub discriminator: Option<String>,
    pub recursive_boundary: bool,
    pub children: Vec<SchemaTreeNode>,
}

pub fn inspect_complex_schema(
    root: &Value,
    schema: &Value,
    label: &str,
    max_depth: usize,
) -> SchemaTreeNode {
    let mut visited = BTreeSet::new();
    inspect_inner(root, schema, label, 0, max_depth.clamp(1, 16), &mut visited)
}

fn inspect_inner(
    root: &Value,
    schema: &Value,
    label: &str,
    depth: usize,
    max_depth: usize,
    visited: &mut BTreeSet<String>,
) -> SchemaTreeNode {
    if depth >= max_depth {
        return SchemaTreeNode {
            label: label.to_owned(),
            kind: "RecursiveBoundary (depth cap)".into(),
            enum_values: Vec::new(),
            discriminator: None,
            recursive_boundary: true,
            children: Vec::new(),
        };
    }

    let mut target = schema;
    let mut inserted_ref: Option<String> = None;
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        if !visited.insert(r.to_owned()) {
            return SchemaTreeNode {
                label: format!("{label} -> {r}"),
                kind: "RecursiveBoundary (cycle)".into(),
                enum_values: Vec::new(),
                discriminator: None,
                recursive_boundary: true,
                children: Vec::new(),
            };
        }
        inserted_ref = Some(r.to_owned());
        if let Some(resolved) = r.strip_prefix('#').and_then(|ptr| root.pointer(ptr)) {
            target = resolved;
        }
    }

    let discriminator = target
        .get("discriminator")
        .and_then(|d| d.get("propertyName"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let enum_values = target
        .get("enum")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().map(Value::to_string).collect())
        .unwrap_or_default();

    let mut children = Vec::new();
    let kind = if let Some(all_of) = target.get("allOf").and_then(Value::as_array) {
        for (i, item) in all_of.iter().enumerate() {
            children.push(inspect_inner(
                root,
                item,
                &format!("allOf[{i}]"),
                depth + 1,
                max_depth,
                visited,
            ));
        }
        "allOf".to_owned()
    } else if let Some(one_of) = target.get("oneOf").and_then(Value::as_array) {
        for (i, item) in one_of.iter().enumerate() {
            children.push(inspect_inner(
                root,
                item,
                &format!("oneOf[{i}]"),
                depth + 1,
                max_depth,
                visited,
            ));
        }
        "oneOf".to_owned()
    } else if let Some(any_of) = target.get("anyOf").and_then(Value::as_array) {
        for (i, item) in any_of.iter().enumerate() {
            children.push(inspect_inner(
                root,
                item,
                &format!("anyOf[{i}]"),
                depth + 1,
                max_depth,
                visited,
            ));
        }
        "anyOf".to_owned()
    } else {
        if let Some(props) = target.get("properties").and_then(Value::as_object) {
            for (k, v) in props {
                children.push(inspect_inner(root, v, k, depth + 1, max_depth, visited));
            }
        }
        if let Some(items) = target.get("items") {
            children.push(inspect_inner(
                root,
                items,
                "items[]",
                depth + 1,
                max_depth,
                visited,
            ));
        }
        target
            .get("type")
            .map(|t| t.to_string())
            .unwrap_or_else(|| "object".into())
    };

    if let Some(r) = inserted_ref {
        visited.remove(&r);
    }

    SchemaTreeNode {
        label: label.to_owned(),
        kind,
        enum_values,
        discriminator,
        recursive_boundary: false,
        children,
    }
}

/// OA08: Confined multi-file $ref resolver and bundler (blocks path traversal and default external URLs).
pub fn resolve_and_bundle_multifile(
    entry_file: &Path,
    project_root: &Path,
    allow_external_urls: bool,
) -> Result<(Value, Vec<ValidationIssue>), String> {
    let canon_root =
        fs::canonicalize(project_root).map_err(|e| format!("Invalid project root: {e}"))?;
    let canon_entry =
        fs::canonicalize(entry_file).map_err(|e| format!("Invalid entry file: {e}"))?;
    if !canon_entry.starts_with(&canon_root) {
        return Err("Entry specification must reside inside the project directory".into());
    }
    let raw = fs::read_to_string(&canon_entry).map_err(|e| format!("Read failed: {e}"))?;
    let mut root: Value =
        serde_json::from_str(&raw).map_err(|e| format!("JSON parse failed: {e}"))?;
    let mut issues = Vec::new();
    let base_dir = canon_entry.parent().unwrap_or(&canon_root);
    inline_file_refs(
        &mut root,
        base_dir,
        &canon_root,
        allow_external_urls,
        0,
        &mut issues,
    )?;
    Ok((root, issues))
}

fn inline_file_refs(
    node: &mut Value,
    cur_dir: &Path,
    canon_root: &Path,
    allow_external_urls: bool,
    depth: usize,
    issues: &mut Vec<ValidationIssue>,
) -> Result<(), String> {
    if depth > 16 {
        issues.push(ValidationIssue {
            severity: IssueSeverity::Error,
            pointer: "$".into(),
            message: "Multi-file $ref depth exceeded limit (16)".into(),
        });
        return Ok(());
    }
    match node {
        Value::Object(map) => {
            if let Some(ref_val) = map
                .get("$ref")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
            {
                if ref_val.starts_with('#') {
                    return Ok(());
                }
                if ref_val.starts_with("http://") || ref_val.starts_with("https://") {
                    if !allow_external_urls {
                        issues.push(ValidationIssue {
                            severity: IssueSeverity::Error,
                            pointer: "$ref".into(),
                            message: format!("External URL $ref '{ref_val}' blocked by policy"),
                        });
                    }
                    return Ok(());
                }
                let (file_part, ptr_part) = ref_val.split_once('#').unwrap_or((&ref_val, ""));
                let joined = cur_dir.join(file_part);
                let canon_target = fs::canonicalize(&joined)
                    .map_err(|_| format!("Referenced file not found: {file_part}"))?;
                if !canon_target.starts_with(canon_root) {
                    return Err(format!(
                        "Path traversal blocked for external reference: {ref_val}"
                    ));
                }
                let text = fs::read_to_string(&canon_target).map_err(|e| e.to_string())?;
                let mut ext_val: Value = serde_json::from_str(&text)
                    .map_err(|e| format!("Invalid JSON in {file_part}: {e}"))?;
                let next_dir = canon_target.parent().unwrap_or(canon_root);
                inline_file_refs(
                    &mut ext_val,
                    next_dir,
                    canon_root,
                    allow_external_urls,
                    depth + 1,
                    issues,
                )?;
                let picked = if ptr_part.is_empty() {
                    ext_val
                } else {
                    ext_val
                        .pointer(ptr_part)
                        .cloned()
                        .ok_or_else(|| format!("Pointer #{ptr_part} missing in {file_part}"))?
                };
                *node = picked;
                return Ok(());
            }
            for (_, v) in map.iter_mut() {
                inline_file_refs(
                    v,
                    cur_dir,
                    canon_root,
                    allow_external_urls,
                    depth + 1,
                    issues,
                )?;
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                inline_file_refs(
                    item,
                    cur_dir,
                    canon_root,
                    allow_external_urls,
                    depth + 1,
                    issues,
                )?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// OA09: Swagger 2.0 -> OpenAPI 3.0 conversion with explicit compatibility loss report + 3.2 version probe.
pub fn convert_swagger2_to_openapi3(
    raw_json: &str,
) -> Result<(SpecDocument, CompatibilityReport), String> {
    let v: Value = serde_json::from_str(raw_json).map_err(|e| format!("Invalid JSON: {e}"))?;
    if v.get("swagger").and_then(Value::as_str) != Some("2.0") {
        return Err("Document does not specify \"swagger\": \"2.0\"".into());
    }
    let title = v
        .pointer("/info/title")
        .and_then(Value::as_str)
        .unwrap_or("Swagger 2.0 API");
    let version = v
        .pointer("/info/version")
        .and_then(Value::as_str)
        .unwrap_or("1.0.0");
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

    let mut report = CompatibilityReport {
        imported_collections: 0,
        imported_folders: 0,
        imported_requests: 0,
        imported_variables: 0,
        warnings: Vec::new(),
        unsupported_items: Vec::new(),
    };
    let mut new_paths = serde_json::Map::new();
    if let Some(paths) = v.get("paths").and_then(Value::as_object) {
        for (p, pitem) in paths {
            let mut new_pitem = serde_json::Map::new();
            if let Some(obj) = pitem.as_object() {
                for (m, op) in obj {
                    let mut new_op = op.clone();
                    if let Some(op_obj) = new_op.as_object_mut() {
                        if op_obj.contains_key("consumes") || op_obj.contains_key("produces") {
                            report.warnings.push(format!(
                                "{m} {p}: consumes/produces moved or simplified; review media types"
                            ));
                        }
                        if let Some(params) =
                            op_obj.get("parameters").and_then(Value::as_array).cloned()
                        {
                            let mut kept = Vec::new();
                            for pr in params {
                                if pr.get("in").and_then(Value::as_str) == Some("body") {
                                    let schema = pr
                                        .get("schema")
                                        .cloned()
                                        .unwrap_or(json!({"type":"object"}));
                                    op_obj.insert(
                                        "requestBody".into(),
                                        json!({"required": pr.get("required").and_then(Value::as_bool).unwrap_or(false), "content":{"application/json":{"schema":schema}}}),
                                    );
                                } else if pr.get("in").and_then(Value::as_str) == Some("formData") {
                                    report.unsupported_items.push(format!("{m} {p}: formData parameter requires manual multipart review"));
                                } else {
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
                                        json!({"application/json":{"schema":schema}}),
                                    );
                                }
                            }
                        }
                    }
                    new_pitem.insert(m.clone(), new_op);
                    report.imported_requests += 1;
                }
            }
            new_paths.insert(p.clone(), Value::Object(new_pitem));
        }
    }

    let mut components = serde_json::Map::new();
    if let Some(defs) = v.get("definitions") {
        components.insert("schemas".into(), defs.clone());
    }
    let converted = json!({
        "openapi": "3.0.3",
        "info": { "title": title, "version": version },
        "servers": [{ "url": format!("{scheme}://{host}{base_path}") }],
        "paths": Value::Object(new_paths),
        "components": Value::Object(components)
    });
    let mut text = serde_json::to_string_pretty(&converted).unwrap_or_default();
    text = text.replace("#/definitions/", "#/components/schemas/");
    let doc = parse_and_validate(
        format!("swg2-{}", title.to_ascii_lowercase()),
        title.to_owned(),
        &text,
        &[],
    );
    Ok((doc, report))
}

pub fn assess_openapi_version(doc: &SpecDocument) -> String {
    match doc.version.as_ref() {
        Some(OpenApiVersion::V3_0(v)) => format!("OpenAPI {v}: natively supported"),
        Some(OpenApiVersion::V3_1(v)) => {
            format!("OpenAPI {v}: natively supported (JSON Schema 2020-12 subset)")
        }
        Some(OpenApiVersion::Unsupported(v)) if v.starts_with("3.2") => {
            format!(
                "OpenAPI {v}: explicitly marked unsupported pending schema dialect rules; do not assume 3.1 compatibility"
            )
        }
        Some(OpenApiVersion::Unsupported(v)) => {
            format!("Version '{v}': unsupported; import rejected or flagged")
        }
        None => "OpenAPI version missing or unparsed".into(),
    }
}

/// OA10: Team API Convention Lint Rules (kept strictly distinct from structural errors).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamLintRules {
    pub require_operation_id: bool,
    pub require_description: bool,
    pub require_error_response: bool,
    pub require_security_scheme: bool,
}

impl Default for TeamLintRules {
    fn default() -> Self {
        Self {
            require_operation_id: true,
            require_description: true,
            require_error_response: true,
            require_security_scheme: false,
        }
    }
}

pub fn lint_spec_conventions(doc: &SpecDocument, rules: &TeamLintRules) -> Vec<ValidationIssue> {
    let mut out = Vec::new();
    for ep in &doc.operations {
        let ptr = format!(
            "/paths/{}/{}",
            ep.path,
            ep.method.as_str().to_ascii_lowercase()
        );
        if rules.require_operation_id && ep.operation_id.as_deref().unwrap_or("").trim().is_empty()
        {
            out.push(ValidationIssue {
                severity: IssueSeverity::Warning,
                pointer: ptr.clone(),
                message: "Lint [convention]: missing operationId".into(),
            });
        }
        if rules.require_description
            && ep.summary.trim().is_empty()
            && ep.description.trim().is_empty()
        {
            out.push(ValidationIssue {
                severity: IssueSeverity::Warning,
                pointer: ptr.clone(),
                message: "Lint [convention]: missing summary/description".into(),
            });
        }
        if rules.require_error_response
            && !ep.responses.iter().any(|r| {
                r.status.starts_with('4') || r.status.starts_with('5') || r.status == "default"
            })
        {
            out.push(ValidationIssue {
                severity: IssueSeverity::Warning,
                pointer: ptr,
                message: "Lint [convention]: no 4xx/5xx/default error response documented".into(),
            });
        }
    }
    if rules.require_security_scheme
        && doc.format == SpecFormat::Json
        && let Ok(v) = serde_json::from_str::<Value>(&doc.raw_text)
        && v.pointer("/components/securitySchemes").is_none()
    {
        out.push(ValidationIssue {
            severity: IssueSeverity::Warning,
            pointer: "/components/securitySchemes".into(),
            message: "Lint [convention]: missing components.securitySchemes definition".into(),
        });
    }
    out
}

/// DC07: Propose an OpenAPI patch from a manual request for user review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecPatchProposal {
    pub target_path: String,
    pub method: HttpMethod,
    pub snippet_json: String,
    pub review_notice: String,
}

pub fn propose_spec_patch_from_request(req: &RequestItem) -> SpecPatchProposal {
    let path_only = req
        .url
        .split('?')
        .next()
        .unwrap_or("/")
        .split("://")
        .nth(1)
        .and_then(|s| s.find('/').map(|i| &s[i..]))
        .unwrap_or("/");
    let params: Vec<Value> = req
        .query_params
        .iter()
        .filter(|q| q.enabled)
        .map(|q| json!({"name": q.key, "in": "query", "required": false, "schema": {"type": "string"}}))
        .collect();
    let patch = json!({
        path_only: {
            req.method.as_str().to_ascii_lowercase(): {
                "summary": req.name,
                "parameters": params,
                "responses": {
                    "200": { "description": "Proposed from manual request — review status and schema" }
                }
            }
        }
    });
    SpecPatchProposal {
        target_path: path_only.to_owned(),
        method: req.method,
        snippet_json: serde_json::to_string_pretty(&patch).unwrap_or_default(),
        review_notice: "Generated draft contract proposal from manual request; review fields before publishing.".into(),
    }
}

/// DC08: Runtime response validation against OpenAPI spec status, content-type, and basic schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractValidationReport {
    pub matched: bool,
    pub schema_dialect: String,
    pub mismatches: Vec<String>,
    pub unsupported_keywords: Vec<String>,
}

pub fn validate_response_against_endpoint(
    doc: &SpecDocument,
    ep: &SpecOperation,
    status: u16,
    content_type: &str,
    body_bytes: &[u8],
) -> ContractValidationReport {
    let mut mismatches = Vec::new();
    let status_str = status.to_string();
    let resp_spec = ep
        .responses
        .iter()
        .find(|r| r.status == status_str || r.status == "default");
    if let Some(rs) = resp_spec {
        if !rs.content_types.is_empty() {
            let ct_base = content_type.split(';').next().unwrap_or("").trim();
            if !rs
                .content_types
                .iter()
                .any(|ct| ct.eq_ignore_ascii_case(ct_base))
            {
                mismatches.push(format!(
                    "Content-Type mismatch at $: expected one of {:?}, got '{ct_base}'",
                    rs.content_types
                ));
            }
        }
    } else {
        mismatches.push(format!("Undocumented HTTP status {status} at $"));
    }

    // Also validate basic JSON property schema if present in JSON spec
    if content_type.to_ascii_lowercase().contains("json")
        && let Ok(spec_v) = serde_json::from_str::<Value>(&doc.raw_text)
    {
        let m_lc = ep.method.as_str().to_ascii_lowercase();
        let schema_ptr = format!(
            "/paths/{}/{m_lc}/responses/{status_str}/content/application~1json/schema",
            ep.path.replace('~', "~0").replace('/', "~1")
        );
        if let Some(schema) = spec_v.pointer(&schema_ptr) {
            match serde_json::from_slice::<Value>(body_bytes) {
                Ok(body_v) => check_json_schema(&spec_v, schema, &body_v, "$", &mut mismatches),
                Err(e) => mismatches.push(format!("Invalid JSON response body at $: {e}")),
            }
        }
    }

    let unsupported: Vec<String> = doc
        .issues
        .iter()
        .filter(|i| i.severity == IssueSeverity::UnsupportedKeyword)
        .map(|i| format!("{}: {}", i.pointer, i.message))
        .collect();

    ContractValidationReport {
        matched: mismatches.is_empty(),
        schema_dialect: format!("{:?}", doc.version),
        mismatches,
        unsupported_keywords: unsupported,
    }
}

fn check_json_schema(root: &Value, schema: &Value, val: &Value, ptr: &str, out: &mut Vec<String>) {
    let target = if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        r.strip_prefix('#')
            .and_then(|p| root.pointer(p))
            .unwrap_or(schema)
    } else {
        schema
    };
    if let Some(ty) = target.get("type").and_then(Value::as_str) {
        let ok = match ty {
            "object" => val.is_object(),
            "array" => val.is_array(),
            "string" => val.is_string(),
            "integer" => val.as_i64().is_some() || val.as_u64().is_some(),
            "number" => val.is_number(),
            "boolean" => val.is_boolean(),
            "null" => val.is_null(),
            _ => true,
        };
        if !ok {
            out.push(format!("{ptr}: expected type '{ty}', got {val}"));
            return;
        }
    }
    if let Some(req) = target.get("required").and_then(Value::as_array)
        && let Some(obj) = val.as_object()
    {
        for k in req.iter().filter_map(Value::as_str) {
            if !obj.contains_key(k) {
                out.push(format!("{ptr}.{k}: missing required property"));
            }
        }
    }
    if let Some(props) = target.get("properties").and_then(Value::as_object)
        && let Some(obj) = val.as_object()
    {
        for (k, sub_schema) in props {
            if let Some(sub_val) = obj.get(k) {
                check_json_schema(root, sub_schema, sub_val, &format!("{ptr}.{k}"), out);
            }
        }
    }
}

/// DC09: Detect linked requests referencing removed or deprecated operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftItem {
    pub request_id: String,
    pub request_name: String,
    pub issue: String,
}

pub fn detect_contract_drift(doc: &SpecDocument, requests: &[RequestItem]) -> Vec<DriftItem> {
    let mut items = Vec::new();
    for r in requests {
        let Some(ref op_uid) = r.linked_operation_uid else {
            continue;
        };
        match doc.operations.iter().find(|e| &e.operation_uid == op_uid) {
            None => items.push(DriftItem {
                request_id: r.id.clone(),
                request_name: r.name.clone(),
                issue: format!(
                    "Removed operation (uid={op_uid}) still referenced by linked request {}",
                    r.url
                ),
            }),
            Some(ep) if ep.deprecated => items.push(DriftItem {
                request_id: r.id.clone(),
                request_name: r.name.clone(),
                issue: format!(
                    "Linked operation {} {} is deprecated in spec",
                    ep.method.as_str(),
                    ep.path
                ),
            }),
            Some(_) => {}
        }
    }
    items
}

/// DC10: Sanitized self-contained Markdown and HTML documentation export.
pub fn export_docs_markdown(doc: &SpecDocument, version_label: &str) -> String {
    let policy = RedactionPolicy::default();
    let mut out = format!("# {} ({version_label})\n\n", doc.title);
    for ep in &doc.operations {
        out.push_str(&format!(
            "## `{}` `{}`{}\n\n",
            ep.method.as_str(),
            ep.path,
            if ep.deprecated { " [DEPRECATED]" } else { "" }
        ));
        if !ep.summary.is_empty() {
            let clean =
                swaggen_domain::security::sanitize_text(&ep.summary, &policy).sanitized_text;
            out.push_str(&format!("{clean}\n\n"));
        }
        if !ep.parameters.is_empty() {
            out.push_str("**Parameters**:\n");
            for p in &ep.parameters {
                out.push_str(&format!(
                    "- `{}` ({}, {}): {}\n",
                    p.name,
                    p.location,
                    p.schema_type,
                    if p.required { "required" } else { "optional" }
                ));
            }
            out.push('\n');
        }
    }
    out
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn export_docs_html(doc: &SpecDocument, version_label: &str) -> String {
    let md_clean = export_docs_markdown(doc, version_label);
    let mut body = String::new();
    for line in md_clean.lines() {
        if let Some(h1) = line.strip_prefix("# ") {
            body.push_str(&format!("<h1>{}</h1>\n", escape_html(h1)));
        } else if let Some(h2) = line.strip_prefix("## ") {
            body.push_str(&format!("<h2>{}</h2>\n", escape_html(h2)));
        } else if !line.trim().is_empty() {
            body.push_str(&format!("<p>{}</p>\n", escape_html(line)));
        }
    }
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title><style>body{{font-family:Segoe UI,sans-serif;max-width:920px;margin:2rem auto;background:#fff;color:#111}}h2{{border-bottom:1px solid #e5e7eb;padding-bottom:4px}}</style></head><body>{body}</body></html>\n",
        escape_html(&doc.title)
    )
}

/// IM07: Shared attachment bundler enforcing project confinement, max file size, and SHA-256 verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedAttachmentManifestEntry {
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

pub fn bundle_shared_attachments(
    files: &[PathBuf],
    shared_dir: &Path,
    max_file_bytes: u64,
) -> Result<Vec<SharedAttachmentManifestEntry>, String> {
    let canon_dir = fs::canonicalize(shared_dir).map_err(|e| format!("Invalid shared dir: {e}"))?;
    let mut out = Vec::new();
    for path in files {
        let canon = fs::canonicalize(path)
            .map_err(|_| format!("Missing attachment: {}", path.display()))?;
        if !canon.starts_with(&canon_dir) {
            return Err(format!(
                "Refusing personal unshared path outside shared directory: {}",
                path.display()
            ));
        }
        let bytes = fs::read(&canon).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > max_file_bytes {
            return Err(format!(
                "Attachment {} exceeds max limit ({} > {max_file_bytes})",
                path.display(),
                bytes.len()
            ));
        }
        let rel = canon
            .strip_prefix(&canon_dir)
            .unwrap_or(&canon)
            .to_string_lossy()
            .replace('\\', "/");
        out.push(SharedAttachmentManifestEntry {
            relative_path: rel,
            size_bytes: bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(&bytes)),
        });
    }
    out.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(out)
}

/// IM09: Deterministic sorted text export for Git versioning without secret leakage.
pub fn export_git_project_tree(
    state: &WorkspaceState,
    specs: &[SpecDocument],
    repo_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    fs::create_dir_all(repo_dir.join("collections")).map_err(|e| e.to_string())?;
    fs::create_dir_all(repo_dir.join("specs")).map_err(|e| e.to_string())?;
    fs::create_dir_all(repo_dir.join("environments")).map_err(|e| e.to_string())?;
    let mut written = Vec::new();

    let sanitized_json = state
        .export_sanitized_json(&RedactionPolicy::default())
        .map_err(|e| e.to_string())?;
    let state_clean: WorkspaceState =
        serde_json::from_str(&sanitized_json).map_err(|e| e.to_string())?;

    let mut reqs_by_col: BTreeMap<String, Vec<&RequestItem>> = BTreeMap::new();
    for r in &state_clean.requests {
        reqs_by_col
            .entry(r.collection_id.clone())
            .or_default()
            .push(r);
    }
    for col in &state_clean.collections {
        let mut reqs = reqs_by_col.remove(&col.id).unwrap_or_default();
        reqs.sort_by(|a, b| a.name.cmp(&b.name));
        let p = repo_dir
            .join("collections")
            .join(format!("{}.json", col.id));
        let content = serde_json::to_string_pretty(&json!({"collection": col, "requests": reqs}))
            .map_err(|e| e.to_string())?;
        fs::write(&p, format!("{content}\n")).map_err(|e| e.to_string())?;
        written.push(p);
    }
    for env in &state_clean.environments {
        let p = repo_dir
            .join("environments")
            .join(format!("{}.json", env.id));
        let content = serde_json::to_string_pretty(env).map_err(|e| e.to_string())?;
        fs::write(&p, format!("{content}\n")).map_err(|e| e.to_string())?;
        written.push(p);
    }
    for spec in specs {
        let ext = if spec.format == SpecFormat::Yaml {
            "yaml"
        } else {
            "json"
        };
        let p = repo_dir.join("specs").join(format!("{}.{ext}", spec.id));
        fs::write(&p, &spec.raw_text).map_err(|e| e.to_string())?;
        written.push(p);
    }
    written.sort();
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swaggen_domain::LocalId;

    #[test]
    fn complex_schema_cycle_cap_swagger2_lint_contract_validation_and_docs_escape_work() {
        let schema_doc = json!({
            "openapi":"3.0.3",
            "info":{"title":"Shop","version":"1.0"},
            "paths":{
                "/orders":{
                    "get":{
                        "responses":{
                            "200":{
                                "description":"ok",
                                "content":{"application/json":{"schema":{
                                    "type":"object",
                                    "required":["id","status"],
                                    "properties":{
                                        "id":{"type":"integer"},
                                        "status":{"type":"string","enum":["open","closed"]},
                                        "self_ref":{"$ref":"#/components/schemas/Node"}
                                    }
                                }}}
                            }
                        }
                    }
                }
            },
            "components":{
                "schemas":{
                    "Node":{
                        "type":"object",
                        "properties":{"next":{"$ref":"#/components/schemas/Node"}}
                    }
                }
            }
        });
        let tree = inspect_complex_schema(
            &schema_doc,
            &json!({"$ref": "#/components/schemas/Node"}),
            "Node",
            8,
        );
        assert!(tree.children.iter().any(|c| c.recursive_boundary));

        let spec_txt = serde_json::to_string_pretty(&schema_doc).unwrap();
        let doc = parse_and_validate("shop", "Shop", &spec_txt, &[]);
        let lint = lint_spec_conventions(&doc, &TeamLintRules::default());
        assert!(lint.iter().any(|l| l.message.contains("operationId")));

        let ep = &doc.operations[0];
        let ok_rep = validate_response_against_endpoint(
            &doc,
            ep,
            200,
            "application/json",
            br#"{"id":1,"status":"open"}"#,
        );
        assert!(ok_rep.matched);
        let bad_rep = validate_response_against_endpoint(
            &doc,
            ep,
            200,
            "application/json",
            br#"{"id":"oops"}"#,
        );
        assert!(!bad_rep.matched);
        assert!(
            bad_rep
                .mismatches
                .iter()
                .any(|m| m.contains("$.status: missing required property"))
        );

        let html = export_docs_html(&doc, "v1");
        assert!(html.contains("Shop (v1)"));

        // Swagger 2.0 convert test
        let swg2 = json!({
            "swagger":"2.0",
            "info":{"title":"Legacy","version":"1.0"},
            "host":"api.test",
            "basePath":"/v1",
            "schemes":["https"],
            "paths":{"/ping":{"get":{"responses":{"200":{"description":"ok","schema":{"$ref":"#/definitions/Pong"}}}}}},
            "definitions":{"Pong":{"type":"object","properties":{"ok":{"type":"boolean"}}}}
        });
        let (conv_doc, _) = convert_swagger2_to_openapi3(&swg2.to_string()).unwrap();
        assert_eq!(conv_doc.operations.len(), 1);
        assert!(conv_doc.raw_text.contains("#/components/schemas/Pong"));

        // Deterministic Git export test
        let dir = std::env::temp_dir().join(format!("swaggen-git-tree-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let st = WorkspaceState::new_personal(LocalId::parse("personal").unwrap(), "Personal");
        let files = export_git_project_tree(&st, &[doc], &dir).unwrap();
        assert!(files.len() >= 3);
        let _ = fs::remove_dir_all(&dir);
    }
}
