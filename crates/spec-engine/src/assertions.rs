//! Basic request assertion evaluator (TS01 / T01 / T13).
use serde::{Deserialize, Serialize};
use swaggen_domain::workspace::{AssertionRule, AssertionTarget};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertionResult {
    pub rule_id: String,
    pub label: String,
    pub passed: bool,
    pub detail: String,
}

pub fn evaluate_assertions(
    rules: &[AssertionRule],
    status_code: u16,
    duration_ms: u64,
    headers: &[(String, String)],
    body_bytes: &[u8],
) -> Vec<AssertionResult> {
    let body_text = String::from_utf8_lossy(body_bytes);
    let parsed_json: Option<serde_json::Value> = serde_json::from_str(&body_text).ok();

    rules
        .iter()
        .filter(|r| r.enabled)
        .map(|rule| {
            let (passed, detail) = match &rule.target {
                AssertionTarget::StatusEquals => match rule.expected.trim().parse::<u16>() {
                    Ok(exp) => (
                        status_code == exp,
                        format!("expected status {exp}, got {status_code}"),
                    ),
                    Err(_) => (
                        false,
                        format!("invalid expected status '{}'", rule.expected),
                    ),
                },
                AssertionTarget::MaxDurationMs => match rule.expected.trim().parse::<u64>() {
                    Ok(max_ms) => (
                        duration_ms <= max_ms,
                        format!("duration {duration_ms} ms <= {max_ms} ms"),
                    ),
                    Err(_) => (false, format!("invalid max duration '{}'", rule.expected)),
                },
                AssertionTarget::HeaderEquals { name } => {
                    let found = headers
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(name.trim()))
                        .map(|(_, v)| v.as_str());
                    match found {
                        Some(val) => (
                            val == rule.expected,
                            format!("header '{name}' = '{val}' (expected '{}')", rule.expected),
                        ),
                        None => (false, format!("header '{name}' missing")),
                    }
                }
                AssertionTarget::JsonPathEquals { path } => match &parsed_json {
                    Some(root) => match lookup_json_path(root, path) {
                        Some(val) => {
                            let actual = match val {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            (
                                actual == rule.expected,
                                format!("{path} = '{actual}' (expected '{}')", rule.expected),
                            )
                        }
                        None => (false, format!("JSON path '{path}' not found")),
                    },
                    None => (false, "response body is not valid JSON".into()),
                },
                AssertionTarget::BodyContains => (
                    body_text.contains(&rule.expected),
                    if body_text.contains(&rule.expected) {
                        format!("body contains '{}'", rule.expected)
                    } else {
                        format!("body does not contain '{}'", rule.expected)
                    },
                ),
            };
            AssertionResult {
                rule_id: rule.id.clone(),
                label: rule.label.clone(),
                passed,
                detail,
            }
        })
        .collect()
}

pub fn lookup_json_path<'a>(
    root: &'a serde_json::Value,
    path: &str,
) -> Option<&'a serde_json::Value> {
    let trimmed = path.trim().trim_start_matches('$').trim_start_matches('.');
    if trimmed.is_empty() {
        return Some(root);
    }
    let mut current = root;
    for segment in trimmed.split('.') {
        if let Some((key, rest)) = segment.split_once('[') {
            if !key.is_empty() {
                current = current.get(key)?;
            }
            let mut rem = rest;
            loop {
                let end = rem.find(']')?;
                let idx: usize = rem[..end].parse().ok()?;
                current = current.get(idx)?;
                let after = &rem[end + 1..];
                if let Some(next_open) = after.strip_prefix('[') {
                    rem = next_open;
                } else {
                    break;
                }
            }
        } else {
            current = current.get(segment)?;
        }
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_status_duration_header_json_path_and_contains() {
        let rules = vec![
            AssertionRule {
                id: "1".into(),
                label: "200 OK".into(),
                target: AssertionTarget::StatusEquals,
                expected: "200".into(),
                enabled: true,
            },
            AssertionRule {
                id: "2".into(),
                label: "Fast".into(),
                target: AssertionTarget::MaxDurationMs,
                expected: "500".into(),
                enabled: true,
            },
            AssertionRule {
                id: "3".into(),
                label: "JSON user".into(),
                target: AssertionTarget::JsonPathEquals {
                    path: "$.items[1].name".into(),
                },
                expected: "beta".into(),
                enabled: true,
            },
            AssertionRule {
                id: "4".into(),
                label: "Content-Type".into(),
                target: AssertionTarget::HeaderEquals {
                    name: "content-type".into(),
                },
                expected: "application/json".into(),
                enabled: true,
            },
            AssertionRule {
                id: "5".into(),
                label: "Body contains".into(),
                target: AssertionTarget::BodyContains,
                expected: "beta".into(),
                enabled: true,
            },
        ];
        let body = br#"{"items":[{"name":"alpha"},{"name":"beta"}]}"#;
        let headers = vec![("Content-Type".into(), "application/json".into())];
        let results = evaluate_assertions(&rules, 200, 120, &headers, body);
        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| r.passed));
    }
}
