//! Version B helpers: dynamic variables (EV08), response extractors (EV09), and response diff (RS09).
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtractorSource {
    BodyJsonPath,
    Header,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseExtractorRule {
    pub id: String,
    pub target_var: String,
    pub source: ExtractorSource,
    pub expression: String,
    pub mark_secret: bool,
}

pub fn eval_dynamic_token(expr: &str, seed: u64, now_unix: i64) -> Option<String> {
    match expr {
        "$uuid" => {
            if seed == 0 {
                Some(Uuid::new_v4().to_string())
            } else {
                Some(format!(
                    "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
                    (seed >> 32) as u32,
                    (seed & 0xffff) as u16,
                    (seed.wrapping_mul(31)) & 0x0fff,
                    (seed.wrapping_mul(131)) & 0x0fff,
                    seed.wrapping_mul(0x9e3779b97f4a7c15) & 0xffffffffffff
                ))
            }
        }
        "$timestamp" => Some(now_unix.to_string()),
        "$isoTimestamp" => {
            let dt = time::OffsetDateTime::from_unix_timestamp(now_unix)
                .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
            Some(format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                dt.year(),
                u8::from(dt.month()),
                dt.day(),
                dt.hour(),
                dt.minute(),
                dt.second()
            ))
        }
        "$randomInt" => {
            let v = seed.wrapping_mul(6364136223846793005).wrapping_add(1) % 100_000;
            Some(v.to_string())
        }
        "$randomHex" => {
            let v = seed.wrapping_mul(0x5851f42d4c957f2d).wrapping_add(1);
            Some(format!("{v:016x}"))
        }
        _ => None,
    }
}

pub fn expand_dynamic_placeholders(input: &str, seed: u64, now_unix: i64) -> (String, Vec<(String, String)>) {
    let mut out = String::with_capacity(input.len());
    let mut previews = Vec::new();
    let mut rest = input;
    let mut step = 0u64;
    while let Some(start) = rest.find("{{$") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let name = after[..end].trim();
        step += 1;
        let item_seed = seed.wrapping_add(step * 17);
        if let Some(val) = eval_dynamic_token(name, item_seed, now_unix) {
            previews.push((name.to_owned(), val.clone()));
            out.push_str(&val);
        } else {
            out.push_str("{{");
            out.push_str(name);
            out.push_str("}}");
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    (out, previews)
}

pub fn extract_json_path<'a>(val: &'a Value, path: &str) -> Option<&'a Value> {
    let clean = path.trim().trim_start_matches('$').trim_start_matches('.');
    if clean.is_empty() {
        return Some(val);
    }
    let mut cur = val;
    for seg in clean.split('.') {
        if let Some((field, idx_str)) = seg.split_once('[')
            && let Some(idx_raw) = idx_str.strip_suffix(']')
        {
            if !field.is_empty() {
                cur = cur.get(field)?;
            }
            let idx: usize = idx_raw.parse().ok()?;
            cur = cur.get(idx)?;
        } else {
            cur = cur.get(seg)?;
        }
    }
    Some(cur)
}

pub fn apply_response_extractors(
    rules: &[ResponseExtractorRule],
    headers: &[(String, String)],
    body_bytes: &[u8],
) -> Vec<(String, String, bool)> {
    let parsed_json: Option<Value> = serde_json::from_slice(body_bytes).ok();
    let mut extracted = Vec::new();
    for rule in rules {
        let val = match rule.source {
            ExtractorSource::Header => headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(&rule.expression))
                .map(|(_, v)| v.clone()),
            ExtractorSource::BodyJsonPath => parsed_json
                .as_ref()
                .and_then(|v| extract_json_path(v, &rule.expression))
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                }),
        };
        if let Some(v) = val {
            extracted.push((rule.target_var.clone(), v, rule.mark_secret));
        }
    }
    extracted
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffItem {
    pub path: String,
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseDiffReport {
    pub status_changed: Option<(u16, u16)>,
    pub header_diffs: Vec<DiffItem>,
    pub body_diffs: Vec<DiffItem>,
    pub ignored_paths_applied: usize,
}

fn flatten(val: &Value, prefix: &str, ignore: &BTreeSet<String>, out: &mut BTreeMap<String, String>, skipped: &mut usize) {
    let key_now = if prefix.is_empty() { "$" } else { prefix };
    if ignore.contains(key_now) {
        *skipped += 1;
        return;
    }
    match val {
        Value::Object(m) => {
            for (k, v) in m {
                let p = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                flatten(v, &p, ignore, out, skipped);
            }
        }
        Value::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                let p = format!("{prefix}[{i}]");
                flatten(v, &p, ignore, out, skipped);
            }
        }
        other => {
            out.insert(key_now.to_owned(), other.to_string());
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn compare_responses(
    left_status: u16,
    left_headers: &[(String, String)],
    left_body: &str,
    right_status: u16,
    right_headers: &[(String, String)],
    right_body: &str,
    ignore_json_paths: &[String],
    ignore_headers: &[String],
) -> ResponseDiffReport {
    let ign_h: BTreeSet<String> = ignore_headers.iter().map(|s| s.to_ascii_lowercase()).collect();
    let ign_p: BTreeSet<String> = ignore_json_paths.iter().cloned().collect();
    let lh: BTreeMap<String, String> = left_headers
        .iter()
        .filter(|(k, _)| !ign_h.contains(&k.to_ascii_lowercase()))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
        .collect();
    let rh: BTreeMap<String, String> = right_headers
        .iter()
        .filter(|(k, _)| !ign_h.contains(&k.to_ascii_lowercase()))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
        .collect();
    let mut header_diffs = Vec::new();
    let hkeys: BTreeSet<_> = lh.keys().chain(rh.keys()).cloned().collect();
    for k in hkeys {
        let lv = lh.get(&k).cloned().unwrap_or_else(|| "<unset>".into());
        let rv = rh.get(&k).cloned().unwrap_or_else(|| "<unset>".into());
        if lv != rv {
            header_diffs.push(DiffItem { path: k, left: lv, right: rv });
        }
    }

    let mut skipped = 0usize;
    let mut lb = BTreeMap::new();
    let mut rb = BTreeMap::new();
    match (serde_json::from_str::<Value>(left_body), serde_json::from_str::<Value>(right_body)) {
        (Ok(lv), Ok(rv)) => {
            flatten(&lv, "", &ign_p, &mut lb, &mut skipped);
            flatten(&rv, "", &ign_p, &mut rb, &mut skipped);
        }
        _ => {
            if left_body != right_body {
                lb.insert("body".into(), left_body.to_owned());
                rb.insert("body".into(), right_body.to_owned());
            }
        }
    }
    let bkeys: BTreeSet<_> = lb.keys().chain(rb.keys()).cloned().collect();
    let mut body_diffs = Vec::new();
    for k in bkeys {
        let lv = lb.get(&k).cloned().unwrap_or_else(|| "<unset>".into());
        let rv = rb.get(&k).cloned().unwrap_or_else(|| "<unset>".into());
        if lv != rv {
            body_diffs.push(DiffItem { path: k, left: lv, right: rv });
        }
    }

    ResponseDiffReport {
        status_changed: (left_status != right_status).then_some((left_status, right_status)),
        header_diffs,
        body_diffs,
        ignored_paths_applied: skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_vars_extractors_and_response_diff_with_ignore_work() {
        let (out, prev) = expand_dynamic_placeholders("id={{$uuid}}&t={{$timestamp}}&r={{$randomInt}}", 42, 1_700_000_000);
        assert!(out.contains("t=1700000000"));
        assert_eq!(prev.len(), 3);

        let rules = vec![
            ResponseExtractorRule {
                id: "e1".into(),
                target_var: "access_token".into(),
                source: ExtractorSource::BodyJsonPath,
                expression: "$.auth.token".into(),
                mark_secret: true,
            },
            ResponseExtractorRule {
                id: "e2".into(),
                target_var: "req_id".into(),
                source: ExtractorSource::Header,
                expression: "X-Request-Id".into(),
                mark_secret: false,
            },
        ];
        let ext = apply_response_extractors(
            &rules,
            &[("x-request-id".into(), "rid-99".into())],
            br#"{"auth":{"token":"tok-abc"}}"#,
        );
        assert_eq!(ext.len(), 2);
        assert_eq!(ext[0], ("access_token".into(), "tok-abc".into(), true));
        assert_eq!(ext[1], ("req_id".into(), "rid-99".into(), false));

        let diff = compare_responses(
            200,
            &[("Date".into(), "Mon".into()), ("X-Ver".into(), "1".into())],
            r#"{"id":"111","val":10}"#,
            200,
            &[("Date".into(), "Tue".into()), ("X-Ver".into(), "2".into())],
            r#"{"id":"222","val":20}"#,
            &["id".into()],
            &["date".into()],
        );
        assert!(diff.status_changed.is_none());
        assert_eq!(diff.header_diffs.len(), 1);
        assert_eq!(diff.body_diffs.len(), 1);
        assert_eq!(diff.body_diffs[0].path, "val");
        assert_eq!(diff.ignored_paths_applied, 2);
    }
}
