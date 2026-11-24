//! Strictly isolated, pure-Rust pre-request & test script sandbox (TS07)
//! plus Postman `pm.*` compatibility subset & migration reporter (TS08).
//! Enforces instruction step budget, wall-clock timeout, heap string/array cap,
//! and exposes zero filesystem, shell, network, or raw secret APIs.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxLimits {
    pub max_steps: usize,
    pub max_duration_ms: u64,
    pub max_memory_bytes: usize,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            max_steps: 20_000,
            max_duration_ms: 250,
            max_memory_bytes: 512 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptTestResult {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptCompatibilityReport {
    pub supported_count: usize,
    pub unsupported_apis: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionReport {
    pub steps_executed: usize,
    pub duration_ms: u64,
    pub mutated_env: BTreeMap<String, Option<String>>,
    pub test_results: Vec<ScriptTestResult>,
    pub logs: Vec<String>,
    pub compatibility: ScriptCompatibilityReport,
    pub error: Option<String>,
}

/// Explicitly scans a Postman or Swaggen script and reports unsupported APIs cleanly (TS08).
pub fn analyze_postman_script(script: &str) -> ScriptCompatibilityReport {
    let mut unsupported = Vec::new();
    let mut supported = 0usize;
    let checks = [
        ("pm.sendRequest", "Unsupported async pm.sendRequest; convert to a chained Collection Runner step"),
        ("require(", "Unsupported CommonJS require(); sandbox blocks module/filesystem loading"),
        ("eval(", "Blocked dynamic eval(); rewrite declaratively"),
        ("xml2Json", "Unsupported xml2Json helper; use JSON responses or assertions"),
        ("setTimeout", "Blocked async timers in bounded pre/test script sandbox"),
        ("fs.", "Blocked filesystem access"),
        ("child_process", "Blocked subprocess spawning"),
    ];
    for (pat, note) in checks {
        if script.contains(pat) {
            unsupported.push(format!("{pat}: {note}"));
        }
    }
    for ok_pat in [
        "pm.environment.",
        "pm.variables.",
        "pm.collectionVariables.",
        "pm.test(",
        "pm.expect(",
        "pm.response",
        "console.log(",
        "JSON.parse",
        "JSON.stringify",
    ] {
        if script.contains(ok_pat) {
            supported += 1;
        }
    }
    let notes = if unsupported.is_empty() {
        vec!["Script uses only supported Swaggen pm.* compatibility subset.".into()]
    } else {
        vec!["Some legacy Postman APIs require manual migration before unattended runs.".into()]
    };
    ScriptCompatibilityReport {
        supported_count: supported,
        unsupported_apis: unsupported,
        notes,
    }
}

#[derive(Debug, Clone)]
enum JsVal {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<JsVal>),
    Obj(BTreeMap<String, JsVal>),
}

impl JsVal {
    fn as_truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(b) => *b,
            Self::Num(n) => *n != 0.0 && !n.is_nan(),
            Self::Str(s) => !s.is_empty(),
            Self::Arr(_) | Self::Obj(_) => true,
        }
    }

    fn as_text(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(b) => b.to_string(),
            Self::Num(n) => {
                if n.fract() == 0.0 {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
            Self::Str(s) => s.clone(),
            Self::Arr(a) => format!("[array len={}]", a.len()),
            Self::Obj(m) => format!("{{object keys={}}}", m.len()),
        }
    }

    fn byte_estimate(&self) -> usize {
        match self {
            Self::Null | Self::Bool(_) | Self::Num(_) => 16,
            Self::Str(s) => 24 + s.len(),
            Self::Arr(a) => 24 + a.iter().map(Self::byte_estimate).sum::<usize>(),
            Self::Obj(m) => 24 + m.iter().map(|(k, v)| k.len() + v.byte_estimate()).sum::<usize>(),
        }
    }
}

fn json_to_js(v: &Value) -> JsVal {
    match v {
        Value::Null => JsVal::Null,
        Value::Bool(b) => JsVal::Bool(*b),
        Value::Number(n) => JsVal::Num(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => JsVal::Str(s.clone()),
        Value::Array(a) => JsVal::Arr(a.iter().map(json_to_js).collect()),
        Value::Object(m) => JsVal::Obj(m.iter().map(|(k, v)| (k.clone(), json_to_js(v))).collect()),
    }
}

fn js_to_json(v: &JsVal) -> Value {
    match v {
        JsVal::Null => Value::Null,
        JsVal::Bool(b) => Value::Bool(*b),
        JsVal::Num(n) => serde_json::Number::from_f64(*n).map(Value::Number).unwrap_or(Value::Null),
        JsVal::Str(s) => Value::String(s.clone()),
        JsVal::Arr(a) => Value::Array(a.iter().map(js_to_json).collect()),
        JsVal::Obj(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), js_to_json(v))).collect()),
    }
}

pub struct ScriptContextSnapshot {
    pub env_vars: BTreeMap<String, String>,
    pub response_status: u16,
    pub response_time_ms: u64,
    pub response_body: String,
    pub response_headers: Vec<(String, String)>,
}

struct Vm {
    limits: SandboxLimits,
    start: Instant,
    steps: usize,
    mem_bytes: usize,
    locals: BTreeMap<String, JsVal>,
    env: BTreeMap<String, String>,
    mutated_env: BTreeMap<String, Option<String>>,
    resp_status: u16,
    resp_time_ms: u64,
    resp_body: String,
    resp_headers: BTreeMap<String, String>,
    tests: Vec<ScriptTestResult>,
    logs: Vec<String>,
}

impl Vm {
    fn tick(&mut self) -> Result<(), String> {
        self.steps += 1;
        if self.steps > self.limits.max_steps {
            return Err(format!(
                "Sandbox terminated: exceeded max instruction steps ({})",
                self.limits.max_steps
            ));
        }
        if self.steps.is_multiple_of(128)
            && self.start.elapsed() > Duration::from_millis(self.limits.max_duration_ms)
        {
            return Err(format!(
                "Sandbox terminated: exceeded time budget ({} ms)",
                self.limits.max_duration_ms
            ));
        }
        if self.mem_bytes > self.limits.max_memory_bytes {
            return Err(format!(
                "Sandbox terminated: exceeded memory cap ({} bytes)",
                self.limits.max_memory_bytes
            ));
        }
        Ok(())
    }

    fn alloc(&mut self, bytes: usize) -> Result<(), String> {
        self.mem_bytes = self.mem_bytes.saturating_add(bytes);
        if self.mem_bytes > self.limits.max_memory_bytes {
            return Err(format!(
                "Sandbox terminated: exceeded memory cap ({} bytes)",
                self.limits.max_memory_bytes
            ));
        }
        Ok(())
    }

    fn exec_block(&mut self, block: &str) -> Result<(), String> {
        for stmt in split_statements(block) {
            self.exec_stmt(&stmt)?;
        }
        Ok(())
    }

    fn exec_stmt(&mut self, stmt_raw: &str) -> Result<(), String> {
        self.tick()?;
        let stmt = stmt_raw.trim().trim_end_matches(';').trim();
        if stmt.is_empty() || stmt.starts_with("//") {
            return Ok(());
        }
        // Check while(...) { ... }
        if let Some(after_w) = stmt.strip_prefix("while") {
            let trimmed = after_w.trim_start();
            if let Some((cond, body)) = extract_paren_and_brace(trimmed) {
                loop {
                    self.tick()?;
                    if !self.eval_expr(cond)?.as_truthy() {
                        break;
                    }
                    self.exec_block(body)?;
                }
                return Ok(());
            }
        }
        // Check if(...) { ... }
        if let Some(after_if) = stmt.strip_prefix("if") {
            let trimmed = after_if.trim_start();
            if let Some((cond, body)) = extract_paren_and_brace(trimmed) {
                if self.eval_expr(cond)?.as_truthy() {
                    self.exec_block(body)?;
                }
                return Ok(());
            }
        }
        // Variable declaration: const / let / var x = expr
        for kw in ["const ", "let ", "var "] {
            if let Some(rest) = stmt.strip_prefix(kw)
                && let Some((name, expr)) = rest.split_once('=')
            {
                let v = self.eval_expr(expr)?;
                self.alloc(v.byte_estimate())?;
                self.locals.insert(name.trim().to_owned(), v);
                return Ok(());
            }
        }
        // Assignment: x = expr (not == or ===)
        if let Some(eq_idx) = find_single_assign(stmt) {
            let name = stmt[..eq_idx].trim();
            let expr = stmt[eq_idx + 1..].trim();
            let v = self.eval_expr(expr)?;
            self.alloc(v.byte_estimate())?;
            self.locals.insert(name.to_owned(), v);
            return Ok(());
        }
        // pm.test("name", () => { ... }) or function() { ... }
        if let Some(inner) = stmt
            .strip_prefix("pm.test(")
            .and_then(|s| s.strip_suffix(')'))
            && let Some((title_expr, body)) = split_first_top_comma(inner)
        {
            let test_name = self.eval_expr(title_expr)?.as_text();
            let body_block = extract_fn_body(body.trim())
                .ok_or_else(|| "Unsupported pm.test callback syntax".to_owned())?;
            match self.exec_block(body_block) {
                Ok(()) => self.tests.push(ScriptTestResult {
                    name: test_name,
                    passed: true,
                    error: None,
                }),
                Err(err) => self.tests.push(ScriptTestResult {
                    name: test_name,
                    passed: false,
                    error: Some(err),
                }),
            }
            return Ok(());
        }
        // Evaluate expression statement (e.g. pm.environment.set, pm.expect, console.log)
        let _ = self.eval_expr(stmt)?;
        Ok(())
    }

    fn eval_expr(&mut self, expr_raw: &str) -> Result<JsVal, String> {
        self.tick()?;
        let expr = expr_raw.trim().trim_end_matches(';').trim();
        if expr.is_empty() {
            return Ok(JsVal::Null);
        }
        // Built-in pm.expect(actual).to.eql(expected) / .equal(expected) / .include(sub)
        if let Some(rest) = expr.strip_prefix("pm.expect(")
            && let Some((lhs_raw, after)) = split_balanced_paren(rest)
        {
            let lhs = self.eval_expr(lhs_raw)?;
            let tail = after.trim();
            if let Some(exp_arg) = tail
                .strip_prefix(".to.eql(")
                .or_else(|| tail.strip_prefix(".to.equal("))
                .and_then(|s| s.strip_suffix(')'))
            {
                let rhs = self.eval_expr(exp_arg)?;
                if lhs.as_text() == rhs.as_text() {
                    return Ok(JsVal::Bool(true));
                }
                return Err(format!(
                    "Assertion failed: expected '{}', got '{}'",
                    rhs.as_text(),
                    lhs.as_text()
                ));
            }
            if let Some(exp_arg) = tail.strip_prefix(".to.include(").and_then(|s| s.strip_suffix(')')) {
                let rhs = self.eval_expr(exp_arg)?;
                if lhs.as_text().contains(&rhs.as_text()) {
                    return Ok(JsVal::Bool(true));
                }
                return Err(format!(
                    "Assertion failed: '{}' does not include '{}'",
                    lhs.as_text(),
                    rhs.as_text()
                ));
            }
            if tail == ".to.exist" {
                if !matches!(lhs, JsVal::Null) {
                    return Ok(JsVal::Bool(true));
                }
                return Err("Assertion failed: expected value to exist".into());
            }
        }
        // Binary equality / comparisons (top-level)
        for (op, is_eq) in [("===", true), ("==", true), ("!==", false), ("!=", false)] {
            if let Some((l, r)) = split_top_op(expr, op) {
                let lv = self.eval_expr(l)?.as_text();
                let rv = self.eval_expr(r)?.as_text();
                return Ok(JsVal::Bool(if is_eq { lv == rv } else { lv != rv }));
            }
        }
        if let Some((l, r)) = split_top_op(expr, "+") {
            let lv = self.eval_expr(l)?;
            let rv = self.eval_expr(r)?;
            return match (&lv, &rv) {
                (JsVal::Num(a), JsVal::Num(b)) => Ok(JsVal::Num(a + b)),
                _ => {
                    let s = format!("{}{}", lv.as_text(), rv.as_text());
                    self.alloc(s.len())?;
                    Ok(JsVal::Str(s))
                }
            };
        }

        // String / number / bool / null literals
        if ((expr.starts_with('"') && expr.ends_with('"'))
            || (expr.starts_with('\'') && expr.ends_with('\'')))
            && expr.len() >= 2
        {
            let s = expr[1..expr.len() - 1].to_owned();
            self.alloc(s.len())?;
            return Ok(JsVal::Str(s));
        }
        if expr == "true" {
            return Ok(JsVal::Bool(true));
        }
        if expr == "false" {
            return Ok(JsVal::Bool(false));
        }
        if expr == "null" || expr == "undefined" {
            return Ok(JsVal::Null);
        }
        if let Ok(n) = expr.parse::<f64>() {
            return Ok(JsVal::Num(n));
        }

        // pm.environment / pm.variables / pm.collectionVariables / console / JSON
        for prefix in [
            "pm.environment.",
            "pm.variables.",
            "pm.collectionVariables.",
        ] {
            if let Some(call) = expr.strip_prefix(prefix) {
                if let Some(arg) = call.strip_prefix("get(").and_then(|s| s.strip_suffix(')')) {
                    let k = self.eval_expr(arg)?.as_text();
                    return Ok(self
                        .env
                        .get(&k)
                        .cloned()
                        .map(JsVal::Str)
                        .unwrap_or(JsVal::Null));
                }
                if let Some(args) = call.strip_prefix("set(").and_then(|s| s.strip_suffix(')'))
                    && let Some((k_e, v_e)) = split_first_top_comma(args)
                {
                    let k = self.eval_expr(k_e)?.as_text();
                    let v = self.eval_expr(v_e)?.as_text();
                    self.alloc(k.len() + v.len())?;
                    self.env.insert(k.clone(), v.clone());
                    self.mutated_env.insert(k, Some(v));
                    return Ok(JsVal::Null);
                }
                if let Some(arg) = call.strip_prefix("unset(").and_then(|s| s.strip_suffix(')')) {
                    let k = self.eval_expr(arg)?.as_text();
                    self.env.remove(&k);
                    self.mutated_env.insert(k, None);
                    return Ok(JsVal::Null);
                }
            }
        }
        if let Some(arg) = expr.strip_prefix("console.log(").and_then(|s| s.strip_suffix(')')) {
            let txt = self.eval_expr(arg)?.as_text();
            self.alloc(txt.len())?;
            if self.logs.len() < 100 {
                self.logs.push(txt);
            }
            return Ok(JsVal::Null);
        }
        if let Some(arg) = expr.strip_prefix("JSON.parse(").and_then(|s| s.strip_suffix(')')) {
            let txt = self.eval_expr(arg)?.as_text();
            let v: Value = serde_json::from_str(&txt).map_err(|e| format!("JSON.parse error: {e}"))?;
            let j = json_to_js(&v);
            self.alloc(j.byte_estimate())?;
            return Ok(j);
        }
        if let Some(arg) = expr.strip_prefix("JSON.stringify(").and_then(|s| s.strip_suffix(')')) {
            let v = self.eval_expr(arg)?;
            let s = js_to_json(&v).to_string();
            self.alloc(s.len())?;
            return Ok(JsVal::Str(s));
        }

        // pm.response helpers
        if expr == "pm.response.code" {
            return Ok(JsVal::Num(f64::from(self.resp_status)));
        }
        if expr == "pm.response.responseTime" {
            return Ok(JsVal::Num(self.resp_time_ms as f64));
        }
        if expr == "pm.response.text()" {
            return Ok(JsVal::Str(self.resp_body.clone()));
        }
        if expr == "pm.response.json()" {
            let v: Value = serde_json::from_str(&self.resp_body)
                .map_err(|e| format!("pm.response.json error: {e}"))?;
            let j = json_to_js(&v);
            self.alloc(j.byte_estimate())?;
            return Ok(j);
        }
        if let Some(arg) = expr
            .strip_prefix("pm.response.headers.get(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let hk = self.eval_expr(arg)?.as_text().to_ascii_lowercase();
            return Ok(self
                .resp_headers
                .get(&hk)
                .cloned()
                .map(JsVal::Str)
                .unwrap_or(JsVal::Null));
        }

        // Property lookup chain e.g. data.user.id
        if let Some((root_name, chain)) = expr.split_once('.')
            && let Some(mut cur) = self.locals.get(root_name).cloned()
        {
            for seg in chain.split('.') {
                match cur {
                    JsVal::Obj(map) => {
                        cur = map.get(seg.trim()).cloned().unwrap_or(JsVal::Null);
                    }
                    JsVal::Arr(arr) => {
                        if seg.trim() == "length" {
                            cur = JsVal::Num(arr.len() as f64);
                        } else if let Ok(i) = seg.trim().parse::<usize>() {
                            cur = arr.get(i).cloned().unwrap_or(JsVal::Null);
                        } else {
                            cur = JsVal::Null;
                        }
                    }
                    JsVal::Str(s) if seg.trim() == "length" => {
                        cur = JsVal::Num(s.len() as f64);
                    }
                    _ => cur = JsVal::Null,
                }
            }
            return Ok(cur);
        }

        if let Some(v) = self.locals.get(expr) {
            return Ok(v.clone());
        }
        Err(format!("Unsupported or blocked identifier/expression in sandbox: '{expr}'"))
    }
}

fn split_statements(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut brace = 0usize;
    let mut paren = 0usize;
    let mut in_str: Option<char> = None;
    let mut prev = '\0';
    for c in src.chars() {
        cur.push(c);
        if let Some(q) = in_str {
            if c == q && prev != '\\' {
                in_str = None;
            }
        } else {
            match c {
                '"' | '\'' => in_str = Some(c),
                '{' => brace += 1,
                '}' => {
                    brace = brace.saturating_sub(1);
                    if brace == 0 && paren == 0 {
                        out.push(std::mem::take(&mut cur));
                    }
                }
                '(' => paren += 1,
                ')' => paren = paren.saturating_sub(1),
                ';' | '\n' if brace == 0 && paren == 0 => {
                    out.push(std::mem::take(&mut cur));
                }
                _ => {}
            }
        }
        prev = c;
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

fn split_first_top_comma(s: &str) -> Option<(&str, &str)> {
    let mut paren = 0usize;
    let mut brace = 0usize;
    let mut in_str: Option<char> = None;
    let mut prev = '\0';
    for (idx, c) in s.char_indices() {
        if let Some(q) = in_str {
            if c == q && prev != '\\' {
                in_str = None;
            }
        } else {
            match c {
                '"' | '\'' => in_str = Some(c),
                '(' => paren += 1,
                ')' => paren = paren.saturating_sub(1),
                '{' => brace += 1,
                '}' => brace = brace.saturating_sub(1),
                ',' if paren == 0 && brace == 0 => return Some((&s[..idx], &s[idx + 1..])),
                _ => {}
            }
        }
        prev = c;
    }
    None
}

fn split_top_op<'a>(s: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    let mut paren = 0usize;
    let mut brace = 0usize;
    let mut in_str: Option<char> = None;
    let mut prev = '\0';
    for (idx, c) in s.char_indices() {
        if let Some(q) = in_str {
            if c == q && prev != '\\' {
                in_str = None;
            }
        } else {
            match c {
                '"' | '\'' => in_str = Some(c),
                '(' => paren += 1,
                ')' => paren = paren.saturating_sub(1),
                '{' => brace += 1,
                '}' => brace = brace.saturating_sub(1),
                _ if paren == 0 && brace == 0 && s[idx..].starts_with(op) => {
                    return Some((&s[..idx], &s[idx + op.len()..]));
                }
                _ => {}
            }
        }
        prev = c;
    }
    None
}

fn find_single_assign(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut paren = 0usize;
    let mut in_str: Option<u8> = None;
    for i in 0..bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q {
                in_str = None;
            }
            continue;
        }
        match b {
            b'"' | b'\'' => in_str = Some(b),
            b'(' => paren += 1,
            b')' => paren = paren.saturating_sub(1),
            b'=' if paren == 0 => {
                let prev = i.checked_sub(1).and_then(|j| bytes.get(j)).copied();
                let next = bytes.get(i + 1).copied();
                if !matches!(prev, Some(b'=' | b'!' | b'<' | b'>')) && next != Some(b'=') {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_balanced_paren(s: &str) -> Option<(&str, &str)> {
    let mut depth = 1usize;
    let mut in_str: Option<char> = None;
    for (idx, c) in s.char_indices() {
        if let Some(q) = in_str {
            if c == q {
                in_str = None;
            }
        } else {
            match c {
                '"' | '\'' => in_str = Some(c),
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((&s[..idx], &s[idx + 1..]));
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn extract_paren_and_brace(s: &str) -> Option<(&str, &str)> {
    let after_p = s.strip_prefix('(')?;
    let (cond, rest) = split_balanced_paren(after_p)?;
    let body_raw = rest.trim_start().strip_prefix('{')?;
    let body = body_raw.strip_suffix('}')?;
    Some((cond, body))
}

fn extract_fn_body(s: &str) -> Option<&str> {
    let open = s.find('{')?;
    let close = s.rfind('}')?;
    if close > open {
        Some(&s[open + 1..close])
    } else {
        None
    }
}

/// Runs a script strictly inside `SandboxLimits` without filesystem/shell/network access (TS07, TS08).
pub fn execute_sandboxed_script(
    script: &str,
    ctx: &ScriptContextSnapshot,
    limits: SandboxLimits,
) -> SandboxExecutionReport {
    let compat = analyze_postman_script(script);
    let headers_lc: BTreeMap<String, String> = ctx
        .response_headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
        .collect();
    let mut vm = Vm {
        limits,
        start: Instant::now(),
        steps: 0,
        mem_bytes: 0,
        locals: BTreeMap::new(),
        env: ctx.env_vars.clone(),
        mutated_env: BTreeMap::new(),
        resp_status: ctx.response_status,
        resp_time_ms: ctx.response_time_ms,
        resp_body: ctx.response_body.clone(),
        resp_headers: headers_lc,
        tests: Vec::new(),
        logs: Vec::new(),
    };
    let error = if !compat.unsupported_apis.is_empty() && script.contains("require(") {
        Some(compat.unsupported_apis.join("; "))
    } else {
        vm.exec_block(script).err()
    };
    SandboxExecutionReport {
        steps_executed: vm.steps,
        duration_ms: vm.start.elapsed().as_millis() as u64,
        mutated_env: vm.mutated_env,
        test_results: vm.tests,
        logs: vm.logs,
        compatibility: compat,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_executes_postman_subset_blocks_fs_and_enforces_infinite_loop_budget() {
        let mut env = BTreeMap::new();
        env.insert("role".into(), "admin".into());
        let ctx = ScriptContextSnapshot {
            env_vars: env,
            response_status: 200,
            response_time_ms: 14,
            response_body: r#"{"token":"tk-99","user":{"name":"Alice"}}"#.into(),
            response_headers: vec![("X-Env".into(), "staging".into())],
        };

        let script = r#"
const payload = pm.response.json();
pm.environment.set("extracted_user", payload.user.name);
pm.test("Status is 200 and token matches", () => {
    pm.expect(pm.response.code).to.eql(200);
    pm.expect(payload.token).to.include("tk-");
});
console.log("done");
"#;
        let rep = execute_sandboxed_script(script, &ctx, SandboxLimits::default());
        assert!(rep.error.is_none(), "unexpected error: {:?}", rep.error);
        assert_eq!(rep.mutated_env.get("extracted_user").unwrap().as_deref(), Some("Alice"));
        assert_eq!(rep.test_results.len(), 1);
        assert!(rep.test_results[0].passed);
        assert_eq!(rep.logs, vec!["done".to_owned()]);

        // Infinite loop must terminate under step cap
        let inf = execute_sandboxed_script(
            "while (true) { let x = \"a\"; }",
            &ctx,
            SandboxLimits {
                max_steps: 250,
                max_duration_ms: 500,
                max_memory_bytes: 64 * 1024,
            },
        );
        assert!(inf.error.as_deref().unwrap_or("").contains("max instruction steps"));

        // Unsupported Postman API triggers explicit migration report & blocks require('fs')
        let fs_script = "const fs = require('fs'); pm.sendRequest('https://evil.test');";
        let fs_rep = execute_sandboxed_script(fs_script, &ctx, SandboxLimits::default());
        assert!(fs_rep.error.is_some());
        assert!(fs_rep.compatibility.unsupported_apis.len() >= 2);
    }
}
