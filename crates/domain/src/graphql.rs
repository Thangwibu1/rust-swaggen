//! Pure-Rust GraphQL document parser, operation/variable extractor,
//! N+1 & query-complexity static analyzer, and `__schema` introspection parser.
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Compact standard GraphQL introspection query compatible with GraphQL June 2018 / Oct 2021 servers.
pub const INTROSPECTION_QUERY: &str = r#"query SwaggenIntrospection {
  __schema {
    queryType { name }
    mutationType { name }
    subscriptionType { name }
    types {
      kind
      name
      description
      fields(includeDeprecated: false) {
        name
        description
        args {
          name
          type { kind name ofType { kind name ofType { kind name } } }
        }
        type { kind name ofType { kind name ofType { kind name } } }
      }
    }
  }
}"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphQlOperationKind {
    Query,
    Mutation,
    Subscription,
}

impl GraphQlOperationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Mutation => "mutation",
            Self::Subscription => "subscription",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphQlOperationInfo {
    pub kind: GraphQlOperationKind,
    pub name: Option<String>,
    /// `(var_name_without_dollar, raw_type_expr)`
    pub variables: Vec<(String, String)>,
    pub max_depth: usize,
    pub field_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphQlAnalysis {
    pub operations: Vec<GraphQlOperationInfo>,
    pub active_kind: GraphQlOperationKind,
    pub max_depth: usize,
    pub total_fields: usize,
    pub potential_n_plus_1_paths: Vec<String>,
    pub complexity_warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphQlFieldSummary {
    pub name: String,
    pub args: Vec<(String, String)>,
    pub return_type: String,
    pub is_object_or_list_of_object: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphQlSchemaSummary {
    pub query_fields: Vec<GraphQlFieldSummary>,
    pub mutation_fields: Vec<GraphQlFieldSummary>,
    pub type_fields: std::collections::BTreeMap<String, Vec<GraphQlFieldSummary>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Name(String),
    Var(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Colon,
    Bang,
    Equals,
    At,
    Spread,
    StringLit,
    Other,
}

fn tokenize_graphql(src: &str) -> Vec<Token> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() || b == b',' {
            i += 1;
            continue;
        }
        if b == b'#' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'"' {
            // Handle block string `"""` or single-line string `"`
            if i + 2 < bytes.len() && bytes[i + 1] == b'"' && bytes[i + 2] == b'"' {
                i += 3;
                while i + 2 < bytes.len()
                    && !(bytes[i] == b'"' && bytes[i + 1] == b'"' && bytes[i + 2] == b'"')
                {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                i = (i + 3).min(bytes.len());
            } else {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                i = (i + 1).min(bytes.len());
            }
            out.push(Token::StringLit);
            continue;
        }
        match b {
            b'(' => {
                out.push(Token::LParen);
                i += 1;
            }
            b')' => {
                out.push(Token::RParen);
                i += 1;
            }
            b'{' => {
                out.push(Token::LBrace);
                i += 1;
            }
            b'}' => {
                out.push(Token::RBrace);
                i += 1;
            }
            b'[' => {
                out.push(Token::LBracket);
                i += 1;
            }
            b']' => {
                out.push(Token::RBracket);
                i += 1;
            }
            b':' => {
                out.push(Token::Colon);
                i += 1;
            }
            b'!' => {
                out.push(Token::Bang);
                i += 1;
            }
            b'=' => {
                out.push(Token::Equals);
                i += 1;
            }
            b'@' => {
                out.push(Token::At);
                i += 1;
            }
            b'.' if i + 2 < bytes.len() && bytes[i + 1] == b'.' && bytes[i + 2] == b'.' => {
                out.push(Token::Spread);
                i += 3;
            }
            b'$' => {
                i += 1;
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                if i > start {
                    out.push(Token::Var(src[start..i].to_owned()));
                }
            }
            _ if b.is_ascii_alphabetic() || b == b'_' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                out.push(Token::Name(src[start..i].to_owned()));
            }
            _ => {
                i += 1;
                while i < bytes.len()
                    && !bytes[i].is_ascii_whitespace()
                    && !matches!(
                        bytes[i],
                        b',' | b'('
                            | b')'
                            | b'{'
                            | b'}'
                            | b'['
                            | b']'
                            | b':'
                            | b'!'
                            | b'='
                            | b'@'
                            | b'"'
                            | b'#'
                    )
                {
                    i += 1;
                }
                out.push(Token::Other);
            }
        }
    }
    out
}

/// Parses operations, declared variables, selection depth, and potential nested N+1 paths.
pub fn analyze_graphql_document(
    query: &str,
    selected_operation: Option<&str>,
    max_allowed_depth: usize,
    max_allowed_fields: usize,
) -> GraphQlAnalysis {
    let tokens = tokenize_graphql(query);
    let mut operations = Vec::new();
    let mut n_plus_1_paths = Vec::new();
    let mut i = 0usize;

    while i < tokens.len() {
        match &tokens[i] {
            Token::Name(kw) if matches!(kw.as_str(), "query" | "mutation" | "subscription") => {
                let kind = match kw.as_str() {
                    "mutation" => GraphQlOperationKind::Mutation,
                    "subscription" => GraphQlOperationKind::Subscription,
                    _ => GraphQlOperationKind::Query,
                };
                i += 1;
                let op_name = if let Some(Token::Name(n)) = tokens.get(i) {
                    i += 1;
                    Some(n.clone())
                } else {
                    None
                };
                let vars = if matches!(tokens.get(i), Some(Token::LParen)) {
                    parse_var_defs(&tokens, &mut i)
                } else {
                    Vec::new()
                };
                skip_directives(&tokens, &mut i);
                let (max_depth, field_count) = if matches!(tokens.get(i), Some(Token::LBrace)) {
                    let mut path = Vec::new();
                    walk_selection_set(&tokens, &mut i, 1, &mut path, &mut n_plus_1_paths)
                } else {
                    (0, 0)
                };
                operations.push(GraphQlOperationInfo {
                    kind,
                    name: op_name,
                    variables: vars,
                    max_depth,
                    field_count,
                });
            }
            Token::Name(kw) if kw == "fragment" => {
                // Skip fragment header until `{ ... }`
                i += 1;
                while i < tokens.len() && !matches!(tokens[i], Token::LBrace) {
                    i += 1;
                }
                if matches!(tokens.get(i), Some(Token::LBrace)) {
                    let mut frag_path = Vec::new();
                    let _ =
                        walk_selection_set(&tokens, &mut i, 1, &mut frag_path, &mut n_plus_1_paths);
                }
            }
            Token::LBrace => {
                // Shorthand anonymous query `{ viewer { id } }`
                let mut path = Vec::new();
                let (max_depth, field_count) =
                    walk_selection_set(&tokens, &mut i, 1, &mut path, &mut n_plus_1_paths);
                operations.push(GraphQlOperationInfo {
                    kind: GraphQlOperationKind::Query,
                    name: None,
                    variables: Vec::new(),
                    max_depth,
                    field_count,
                });
            }
            _ => {
                i += 1;
            }
        }
    }

    let chosen = selected_operation
        .filter(|s| !s.trim().is_empty())
        .and_then(|target| {
            operations
                .iter()
                .find(|op| op.name.as_deref() == Some(target.trim()))
        })
        .or_else(|| operations.first());

    let active_kind = chosen
        .map(|o| o.kind)
        .unwrap_or(GraphQlOperationKind::Query);
    let max_depth = chosen.map(|o| o.max_depth).unwrap_or(0);
    let total_fields = chosen.map(|o| o.field_count).unwrap_or(0);

    let mut complexity_warnings = Vec::new();
    if max_depth > max_allowed_depth {
        complexity_warnings.push(format!(
            "Selection depth {max_depth} exceeds recommended limit ({max_allowed_depth})"
        ));
    }
    if total_fields > max_allowed_fields {
        complexity_warnings.push(format!(
            "Total selected fields ({total_fields}) exceeds recommended limit ({max_allowed_fields})"
        ));
    }

    GraphQlAnalysis {
        operations,
        active_kind,
        max_depth,
        total_fields,
        potential_n_plus_1_paths: n_plus_1_paths,
        complexity_warnings,
    }
}

fn parse_var_defs(tokens: &[Token], i: &mut usize) -> Vec<(String, String)> {
    let mut vars = Vec::new();
    if !matches!(tokens.get(*i), Some(Token::LParen)) {
        return vars;
    }
    *i += 1; // consume `(`
    let mut paren_depth = 1usize;
    while *i < tokens.len() && paren_depth > 0 {
        match &tokens[*i] {
            Token::LParen => {
                paren_depth += 1;
                *i += 1;
            }
            Token::RParen => {
                paren_depth -= 1;
                *i += 1;
            }
            Token::Var(vname) if paren_depth == 1 => {
                let name = vname.clone();
                *i += 1;
                if matches!(tokens.get(*i), Some(Token::Colon)) {
                    *i += 1;
                }
                let mut type_str = String::new();
                while *i < tokens.len() {
                    match &tokens[*i] {
                        Token::Name(tn) => {
                            type_str.push_str(tn);
                            *i += 1;
                        }
                        Token::LBracket => {
                            type_str.push('[');
                            *i += 1;
                        }
                        Token::RBracket => {
                            type_str.push(']');
                            *i += 1;
                        }
                        Token::Bang => {
                            type_str.push('!');
                            *i += 1;
                        }
                        _ => break,
                    }
                }
                // Skip optional `= defaultValue`
                if matches!(tokens.get(*i), Some(Token::Equals)) {
                    *i += 1;
                    skip_value(tokens, i);
                }
                skip_directives(tokens, i);
                vars.push((name, type_str));
            }
            _ => {
                *i += 1;
            }
        }
    }
    vars
}

fn skip_value(tokens: &[Token], i: &mut usize) {
    match tokens.get(*i) {
        Some(Token::LBrace) => {
            *i += 1;
            let mut d = 1usize;
            while *i < tokens.len() && d > 0 {
                match tokens[*i] {
                    Token::LBrace => d += 1,
                    Token::RBrace => d -= 1,
                    _ => {}
                }
                *i += 1;
            }
        }
        Some(Token::LBracket) => {
            *i += 1;
            let mut d = 1usize;
            while *i < tokens.len() && d > 0 {
                match tokens[*i] {
                    Token::LBracket => d += 1,
                    Token::RBracket => d -= 1,
                    _ => {}
                }
                *i += 1;
            }
        }
        Some(_) => {
            *i += 1;
        }
        None => {}
    }
}

fn skip_parens(tokens: &[Token], i: &mut usize) {
    if !matches!(tokens.get(*i), Some(Token::LParen)) {
        return;
    }
    *i += 1;
    let mut d = 1usize;
    while *i < tokens.len() && d > 0 {
        match tokens[*i] {
            Token::LParen => d += 1,
            Token::RParen => d -= 1,
            _ => {}
        }
        *i += 1;
    }
}

fn skip_directives(tokens: &[Token], i: &mut usize) {
    while matches!(tokens.get(*i), Some(Token::At)) {
        *i += 1;
        if matches!(tokens.get(*i), Some(Token::Name(_))) {
            *i += 1;
        }
        skip_parens(tokens, i);
    }
}

fn looks_like_plural_relation(field: &str) -> bool {
    let lower = field.to_ascii_lowercase();
    (lower.ends_with('s')
        && !lower.ends_with("status")
        && !lower.ends_with("address")
        && !lower.starts_with("__"))
        || matches!(
            lower.as_str(),
            "nodes" | "edges" | "items" | "children" | "list" | "data" | "results"
        )
}

fn walk_selection_set(
    tokens: &[Token],
    i: &mut usize,
    depth: usize,
    path: &mut Vec<String>,
    n_plus_1_paths: &mut Vec<String>,
) -> (usize, usize) {
    if !matches!(tokens.get(*i), Some(Token::LBrace)) {
        return (depth, 0);
    }
    *i += 1; // consume `{`
    let mut max_d = depth;
    let mut count = 0usize;

    while *i < tokens.len() {
        match &tokens[*i] {
            Token::RBrace => {
                *i += 1;
                break;
            }
            Token::Spread => {
                *i += 1;
                // Inline fragment `... on Type { ... }` or fragment spread `...FragName`
                if let Some(Token::Name(kw)) = tokens.get(*i) {
                    if kw == "on" {
                        *i += 1;
                        if matches!(tokens.get(*i), Some(Token::Name(_))) {
                            *i += 1;
                        }
                    } else {
                        *i += 1;
                    }
                }
                skip_directives(tokens, i);
                if matches!(tokens.get(*i), Some(Token::LBrace)) {
                    let (sub_d, sub_c) = walk_selection_set(tokens, i, depth, path, n_plus_1_paths);
                    max_d = max_d.max(sub_d);
                    count += sub_c;
                }
            }
            Token::Name(first_name) => {
                let mut field_name = first_name.clone();
                *i += 1;
                // Handle field alias `alias: realField`
                if matches!(tokens.get(*i), Some(Token::Colon)) {
                    *i += 1;
                    if let Some(Token::Name(actual)) = tokens.get(*i) {
                        field_name = actual.clone();
                        *i += 1;
                    }
                }
                skip_parens(tokens, i);
                skip_directives(tokens, i);
                count += 1;

                if matches!(tokens.get(*i), Some(Token::LBrace)) {
                    path.push(field_name.clone());
                    // Detect nested 1-to-many -> child object/relation traversal (`listA -> listB` or `listA -> obj -> listB`)
                    let plural_ancestors = path
                        .iter()
                        .filter(|seg| looks_like_plural_relation(seg))
                        .count();
                    if plural_ancestors >= 2 {
                        let joined = path.join(".");
                        if !n_plus_1_paths.contains(&joined) {
                            n_plus_1_paths.push(joined);
                        }
                    }
                    let (sub_d, sub_c) =
                        walk_selection_set(tokens, i, depth + 1, path, n_plus_1_paths);
                    path.pop();
                    max_d = max_d.max(sub_d);
                    count += sub_c;
                }
            }
            _ => {
                *i += 1;
            }
        }
    }
    (max_d, count)
}

/// Builds a starter JSON object for GraphQL `variables` from declared `$var: Type` definitions,
/// preserving any keys already set in `existing_json`.
pub fn generate_variables_skeleton(vars: &[(String, String)], existing_json: &str) -> String {
    let mut map: Map<String, Value> = serde_json::from_str(existing_json).unwrap_or_default();
    for (name, ty) in vars {
        if map.contains_key(name) {
            continue;
        }
        let clean = ty.trim_matches(|c| matches!(c, '!' | '[' | ']'));
        let default_val = if ty.starts_with('[') {
            Value::Array(Vec::new())
        } else {
            match clean {
                "Int" | "Long" => Value::Number(0.into()),
                "Float" => serde_json::Number::from_f64(0.0)
                    .map(Value::Number)
                    .unwrap_or(Value::Null),
                "Boolean" => Value::Bool(false),
                _ => Value::String(String::new()),
            }
        };
        map.insert(name.clone(), default_val);
    }
    serde_json::to_string_pretty(&Value::Object(map)).unwrap_or_else(|_| "{}".into())
}

/// Parses a GraphQL `__schema` introspection JSON response into a browsable summary.
pub fn parse_introspection_schema(raw_json: &str) -> Result<GraphQlSchemaSummary, String> {
    let root: Value =
        serde_json::from_str(raw_json).map_err(|e| format!("Invalid introspection JSON: {e}"))?;
    let schema = root
        .get("data")
        .and_then(|d| d.get("__schema"))
        .or_else(|| root.get("__schema"))
        .ok_or_else(|| "Response does not contain `data.__schema`".to_owned())?;

    let query_type_name = schema
        .get("queryType")
        .and_then(|t| t.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("Query");
    let mutation_type_name = schema
        .get("mutationType")
        .and_then(|t| t.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("Mutation");

    let mut type_fields = std::collections::BTreeMap::new();
    if let Some(types) = schema.get("types").and_then(Value::as_array) {
        for t in types {
            let Some(tname) = t.get("name").and_then(Value::as_str) else {
                continue;
            };
            if tname.starts_with("__") {
                continue;
            }
            let Some(fields) = t.get("fields").and_then(Value::as_array) else {
                continue;
            };
            let mut list = Vec::new();
            for f in fields {
                let Some(fname) = f.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let (ret_str, is_obj) = format_type_ref(f.get("type"));
                let mut args = Vec::new();
                if let Some(arg_arr) = f.get("args").and_then(Value::as_array) {
                    for a in arg_arr {
                        if let Some(aname) = a.get("name").and_then(Value::as_str) {
                            let (aty, _) = format_type_ref(a.get("type"));
                            args.push((aname.to_owned(), aty));
                        }
                    }
                }
                list.push(GraphQlFieldSummary {
                    name: fname.to_owned(),
                    args,
                    return_type: ret_str,
                    is_object_or_list_of_object: is_obj,
                });
            }
            type_fields.insert(tname.to_owned(), list);
        }
    }

    let query_fields = type_fields
        .get(query_type_name)
        .cloned()
        .unwrap_or_default();
    let mutation_fields = type_fields
        .get(mutation_type_name)
        .cloned()
        .unwrap_or_default();

    Ok(GraphQlSchemaSummary {
        query_fields,
        mutation_fields,
        type_fields,
    })
}

fn format_type_ref(ty: Option<&Value>) -> (String, bool) {
    let Some(t) = ty else {
        return ("Any".into(), false);
    };
    let kind = t.get("kind").and_then(Value::as_str).unwrap_or("");
    match kind {
        "NON_NULL" => {
            let (inner, is_obj) = format_type_ref(t.get("ofType"));
            (format!("{inner}!"), is_obj)
        }
        "LIST" => {
            let (inner, is_obj) = format_type_ref(t.get("ofType"));
            (format!("[{inner}]"), is_obj)
        }
        "OBJECT" | "INTERFACE" | "UNION" => {
            let name = t.get("name").and_then(Value::as_str).unwrap_or("Object");
            (name.to_owned(), true)
        }
        _ => {
            let name = t.get("name").and_then(Value::as_str).unwrap_or("Scalar");
            (name.to_owned(), false)
        }
    }
}

/// Generates a clean starter GraphQL query or mutation template for a selected root field.
pub fn build_operation_template(
    kind: GraphQlOperationKind,
    field: &GraphQlFieldSummary,
    schema: &GraphQlSchemaSummary,
) -> (String, String) {
    let op_prefix = kind.as_str();
    let pascal: String = {
        let mut chars = field.name.chars();
        match chars.next() {
            Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
            None => "Op".into(),
        }
    };
    let var_defs: Vec<String> = field
        .args
        .iter()
        .map(|(n, t)| format!("${n}: {t}"))
        .collect();
    let arg_pass: Vec<String> = field
        .args
        .iter()
        .map(|(n, _)| format!("{n}: ${n}"))
        .collect();

    let header = if var_defs.is_empty() {
        format!("{op_prefix} {pascal}")
    } else {
        format!("{op_prefix} {pascal}({})", var_defs.join(", "))
    };
    let call_args = if arg_pass.is_empty() {
        String::new()
    } else {
        format!("({})", arg_pass.join(", "))
    };

    let base_type = field
        .return_type
        .trim_matches(|c| matches!(c, '!' | '[' | ']'));
    let subfields: Vec<String> = schema
        .type_fields
        .get(base_type)
        .map(|fields| {
            fields
                .iter()
                .filter(|sf| !sf.is_object_or_list_of_object)
                .take(6)
                .map(|sf| format!("    {}", sf.name))
                .collect()
        })
        .unwrap_or_default();

    let query_text = if field.is_object_or_list_of_object {
        let inner = if subfields.is_empty() {
            "    __typename".to_owned()
        } else {
            subfields.join("\n")
        };
        format!(
            "{header} {{\n  {}{call_args} {{\n{inner}\n  }}\n}}",
            field.name
        )
    } else {
        format!("{header} {{\n  {}{call_args}\n}}", field.name)
    };

    let vars_json = generate_variables_skeleton(&field.args, "{}");
    (query_text, vars_json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzes_operations_variables_depth_and_nested_n_plus_1_paths() {
        let doc = r#"
            query GetWorkspaceTree($workspaceId: ID!, $limit: Int = 20) {
              workspaces(limit: $limit) {
                id
                name
                entities {
                  id
                  comments {
                    id
                    body
                  }
                }
              }
            }

            mutation CreateItem($name: String!) {
              createItem(name: $name) {
                id
              }
            }
        "#;
        let analysis = analyze_graphql_document(doc, Some("GetWorkspaceTree"), 8, 100);
        assert_eq!(analysis.operations.len(), 2);
        assert_eq!(analysis.active_kind, GraphQlOperationKind::Query);
        assert_eq!(analysis.max_depth, 4);
        assert_eq!(
            analysis.operations[0].variables,
            vec![
                ("workspaceId".into(), "ID!".into()),
                ("limit".into(), "Int".into())
            ]
        );
        assert!(
            analysis
                .potential_n_plus_1_paths
                .contains(&"workspaces.entities".to_owned())
        );
        assert!(
            analysis
                .potential_n_plus_1_paths
                .contains(&"workspaces.entities.comments".to_owned())
        );

        let mut_analysis = analyze_graphql_document(doc, Some("CreateItem"), 8, 100);
        assert_eq!(mut_analysis.active_kind, GraphQlOperationKind::Mutation);
        let skel = generate_variables_skeleton(&analysis.operations[0].variables, "{}");
        assert!(skel.contains("\"workspaceId\""));
        assert!(skel.contains("\"limit\": 0"));
    }
}
