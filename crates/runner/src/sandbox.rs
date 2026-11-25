//! Strictly isolated, pure-Rust pre-request & post-response JS sandbox (TS07)
//! supporting both Postman `pm.*` and Playwright `test(...)` / `expect(...)` syntax (TS08).
//! Enforces instruction step budget, wall-clock timeout, and heap string/array cap,
//! while exposing zero filesystem, shell, network, or raw secret APIs.
use base64::{Engine as _, engine::general_purpose::STANDARD};
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
            max_steps: 25_000,
            max_duration_ms: 350,
            max_memory_bytes: 768 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptTestResult {
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ScriptCompatibilityReport {
    pub supported_count: usize,
    pub unsupported_apis: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SandboxExecutionReport {
    pub steps_executed: usize,
    pub duration_ms: u64,
    pub mutated_env: BTreeMap<String, Option<String>>,
    #[serde(default)]
    pub mutated_collection_vars: BTreeMap<String, Option<String>>,
    #[serde(default)]
    pub mutated_globals: BTreeMap<String, Option<String>>,
    #[serde(default)]
    pub mutated_request_vars: BTreeMap<String, Option<String>>,
    #[serde(default)]
    pub mutated_request_headers: Vec<(String, Option<String>)>,
    pub test_results: Vec<ScriptTestResult>,
    pub logs: Vec<String>,
    pub compatibility: ScriptCompatibilityReport,
    pub error: Option<String>,
}

/// Explicitly scans a Postman, Playwright, or Swaggen script and reports unsupported APIs cleanly (TS08).
pub fn analyze_postman_script(script: &str) -> ScriptCompatibilityReport {
    let mut unsupported = Vec::new();
    let mut supported = 0usize;
    let checks = [
        (
            "pm.sendRequest",
            "Unsupported async pm.sendRequest; convert to a chained Test Runner step",
        ),
        (
            "require(",
            "Unsupported CommonJS require(); sandbox blocks module/filesystem loading",
        ),
        ("eval(", "Blocked dynamic eval(); rewrite declaratively"),
        (
            "xml2Json",
            "Unsupported xml2Json helper; use JSON responses or assertions",
        ),
        (
            "setTimeout",
            "Blocked async timers in bounded pre/post script sandbox",
        ),
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
        "pm.globals.",
        "pm.request.",
        "pm.test(",
        "pm.expect(",
        "test(",
        "expect(",
        "pm.response",
        "console.log(",
        "JSON.parse",
        "JSON.stringify",
        "crypto.randomUUID",
        "Date.now",
    ] {
        if script.contains(ok_pat) {
            supported += 1;
        }
    }
    let notes = if unsupported.is_empty() {
        vec!["Script uses supported Swaggen pm.* / Playwright expect() subset.".into()]
    } else {
        vec![
            "Some legacy Node/Postman APIs require manual migration before unattended runs.".into(),
        ]
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

    fn as_num(&self) -> Option<f64> {
        match self {
            Self::Num(n) => Some(*n),
            Self::Str(s) => s.trim().parse::<f64>().ok(),
            Self::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn as_text(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(b) => b.to_string(),
            Self::Num(n) => {
                if n.fract() == 0.0 && *n >= (i64::MIN as f64) && *n <= (i64::MAX as f64) {
                    format!("{}", *n as i64)
                } else {
                    n.to_string()
                }
            }
            Self::Str(s) => s.clone(),
            Self::Arr(_) | Self::Obj(_) => js_to_json(self).to_string(),
        }
    }

    fn byte_estimate(&self) -> usize {
        match self {
            Self::Null | Self::Bool(_) | Self::Num(_) => 16,
            Self::Str(s) => 24 + s.len(),
            Self::Arr(a) => 24 + a.iter().map(Self::byte_estimate).sum::<usize>(),
            Self::Obj(m) => {
                24 + m
                    .iter()
                    .map(|(k, v)| k.len() + v.byte_estimate())
                    .sum::<usize>()
            }
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
        JsVal::Num(n) => serde_json::Number::from_f64(*n)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        JsVal::Str(s) => Value::String(s.clone()),
        JsVal::Arr(a) => Value::Array(a.iter().map(js_to_json).collect()),
        JsVal::Obj(m) => Value::Object(m.iter().map(|(k, v)| (k.clone(), js_to_json(v))).collect()),
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScriptContextSnapshot {
    pub env_vars: BTreeMap<String, String>,
    pub collection_vars: BTreeMap<String, String>,
    pub global_vars: BTreeMap<String, String>,
    pub request_vars: BTreeMap<String, String>,
    pub request_method: String,
    pub request_url: String,
    pub request_headers: Vec<(String, String)>,
    pub request_body: String,
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
    collection_vars: BTreeMap<String, String>,
    globals: BTreeMap<String, String>,
    request_vars: BTreeMap<String, String>,
    mutated_env: BTreeMap<String, Option<String>>,
    mutated_collection_vars: BTreeMap<String, Option<String>>,
    mutated_globals: BTreeMap<String, Option<String>>,
    mutated_request_vars: BTreeMap<String, Option<String>>,
    req_method: String,
    req_url: String,
    req_headers: BTreeMap<String, String>,
    mutated_req_headers: Vec<(String, Option<String>)>,
    req_body: String,
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
        let cleaned = strip_comments(block);
        for stmt in split_statements(&cleaned) {
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

        // while (...) { ... }
        if let Some(after_w) = stmt.strip_prefix("while")
            && after_w
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || c == '(')
            && let Some((cond, body, _tail)) = extract_paren_brace_and_tail(after_w.trim_start())
        {
            loop {
                self.tick()?;
                if !self.eval_expr(cond)?.as_truthy() {
                    break;
                }
                self.exec_block(body)?;
            }
            return Ok(());
        }

        // for (init; cond; step) { ... }
        if let Some(after_f) = stmt.strip_prefix("for")
            && after_f
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || c == '(')
            && let Some((hdr, body, _)) = extract_paren_brace_and_tail(after_f.trim_start())
        {
            let parts: Vec<&str> = hdr.splitn(3, ';').collect();
            if parts.len() == 3 {
                if !parts[0].trim().is_empty() {
                    self.exec_stmt(parts[0])?;
                }
                loop {
                    self.tick()?;
                    if !parts[1].trim().is_empty() && !self.eval_expr(parts[1])?.as_truthy() {
                        break;
                    }
                    self.exec_block(body)?;
                    if !parts[2].trim().is_empty() {
                        self.exec_step_inc(parts[2].trim())?;
                    }
                }
                return Ok(());
            }
        }

        // if (...) { ... } [else if (...) { ... }] [else { ... }]
        if let Some(after_if) = stmt.strip_prefix("if")
            && after_if
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace() || c == '(')
            && let Some((cond, body, tail)) = extract_paren_brace_and_tail(after_if.trim_start())
        {
            if self.eval_expr(cond)?.as_truthy() {
                self.exec_block(body)?;
            } else {
                let else_part = tail.trim();
                if let Some(after_else) = else_part.strip_prefix("else") {
                    let after_else_t = after_else.trim();
                    if after_else_t.starts_with("if") {
                        self.exec_stmt(after_else_t)?;
                    } else if let Some(inner) = after_else_t
                        .strip_prefix('{')
                        .and_then(|s| s.strip_suffix('}'))
                    {
                        self.exec_block(inner)?;
                    }
                }
            }
            return Ok(());
        }

        // pm.test("name", () => { ... }) OR Playwright test("name", () => { ... })
        for test_prefix in ["pm.test(", "test("] {
            if let Some(inner) = stmt
                .strip_prefix(test_prefix)
                .and_then(|s| s.strip_suffix(')'))
                && let Some((title_expr, body)) = split_first_top_comma(inner)
            {
                let test_name = self.eval_expr(title_expr)?.as_text();
                let body_block = extract_fn_body(body.trim())
                    .ok_or_else(|| "Unsupported test callback syntax".to_owned())?;
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
        }

        // Variable declaration: const / let / var x = expr
        for kw in ["const ", "let ", "var "] {
            if let Some(rest) = stmt.strip_prefix(kw) {
                if let Some((name, expr)) = rest.split_once('=') {
                    let v = self.eval_expr(expr)?;
                    self.alloc(v.byte_estimate())?;
                    self.locals.insert(name.trim().to_owned(), v);
                } else {
                    self.locals.insert(rest.trim().to_owned(), JsVal::Null);
                }
                return Ok(());
            }
        }

        // Increment / Decrement x++ / x--
        if stmt.ends_with("++") || stmt.ends_with("--") {
            return self.exec_step_inc(stmt);
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

        let _ = self.eval_expr(stmt)?;
        Ok(())
    }

    fn exec_step_inc(&mut self, s: &str) -> Result<(), String> {
        if let Some(var) = s.strip_suffix("++") {
            let k = var.trim();
            let n = self.locals.get(k).and_then(JsVal::as_num).unwrap_or(0.0);
            self.locals.insert(k.to_owned(), JsVal::Num(n + 1.0));
            return Ok(());
        }
        if let Some(var) = s.strip_suffix("--") {
            let k = var.trim();
            let n = self.locals.get(k).and_then(JsVal::as_num).unwrap_or(0.0);
            self.locals.insert(k.to_owned(), JsVal::Num(n - 1.0));
            return Ok(());
        }
        self.exec_stmt(s)
    }

    fn eval_expr(&mut self, expr_raw: &str) -> Result<JsVal, String> {
        self.tick()?;
        let expr = expr_raw.trim().trim_end_matches(';').trim();
        if expr.is_empty() {
            return Ok(JsVal::Null);
        }

        // Strip outer grouping parens `(...)` if balanced across the entire expression
        if expr.starts_with('(')
            && expr.ends_with(')')
            && let Some((inner, rest)) = split_balanced_paren(&expr[1..])
            && rest.trim().is_empty()
        {
            return self.eval_expr(inner);
        }

        // pm.response.to.have.status(200)
        if let Some(code_arg) = expr
            .strip_prefix("pm.response.to.have.status(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let expected = self.eval_expr(code_arg)?;
            if expected.as_text() == self.resp_status.to_string() {
                return Ok(JsVal::Bool(true));
            }
            return Err(format!(
                "Expected HTTP status {}, got {}",
                expected.as_text(),
                self.resp_status
            ));
        }

        // Postman `pm.expect(actual)...`
        if let Some(rest) = expr.strip_prefix("pm.expect(")
            && let Some((lhs_raw, after)) = split_balanced_paren(rest)
        {
            let lhs = self.eval_expr(lhs_raw)?;
            return self.eval_postman_expect(lhs, after.trim());
        }

        // Playwright `expect(actual)...`
        if let Some(rest) = expr.strip_prefix("expect(")
            && let Some((lhs_raw, after)) = split_balanced_paren(rest)
        {
            let lhs = self.eval_expr(lhs_raw)?;
            return self.eval_playwright_expect(lhs, after.trim());
        }

        // Logical OR / AND
        if let Some((l, r)) = split_top_op(expr, "||") {
            let lv = self.eval_expr(l)?;
            if lv.as_truthy() {
                return Ok(lv);
            }
            return self.eval_expr(r);
        }
        if let Some((l, r)) = split_top_op(expr, "&&") {
            let lv = self.eval_expr(l)?;
            if !lv.as_truthy() {
                return Ok(lv);
            }
            return self.eval_expr(r);
        }

        // Equality & relational comparisons
        for (op, is_eq) in [("===", true), ("==", true), ("!==", false), ("!=", false)] {
            if let Some((l, r)) = split_top_op(expr, op) {
                let lv = self.eval_expr(l)?.as_text();
                let rv = self.eval_expr(r)?.as_text();
                return Ok(JsVal::Bool(if is_eq { lv == rv } else { lv != rv }));
            }
        }
        for op in ["<=", ">=", "<", ">"] {
            if let Some((l, r)) = split_top_op(expr, op) {
                let ln = self.eval_expr(l)?.as_num().unwrap_or(0.0);
                let rn = self.eval_expr(r)?.as_num().unwrap_or(0.0);
                let res = match op {
                    "<=" => ln <= rn,
                    ">=" => ln >= rn,
                    "<" => ln < rn,
                    ">" => ln > rn,
                    _ => false,
                };
                return Ok(JsVal::Bool(res));
            }
        }

        // Binary + / - / * / /
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
        for op in ["-", "*", "/"] {
            if let Some((l, r)) = split_top_op(expr, op)
                && !l.trim().is_empty()
            {
                let ln = self.eval_expr(l)?.as_num().unwrap_or(0.0);
                let rn = self.eval_expr(r)?.as_num().unwrap_or(1.0);
                let v = match op {
                    "-" => ln - rn,
                    "*" => ln * rn,
                    "/" => {
                        if rn == 0.0 {
                            0.0
                        } else {
                            ln / rn
                        }
                    }
                    _ => 0.0,
                };
                return Ok(JsVal::Num(v));
            }
        }

        // Unary !expr
        if let Some(inner) = expr.strip_prefix('!') {
            let v = self.eval_expr(inner)?;
            return Ok(JsVal::Bool(!v.as_truthy()));
        }

        // Template literal `` `hello ${name}` ``
        if expr.starts_with('`') && expr.ends_with('`') && expr.len() >= 2 {
            let body = &expr[1..expr.len() - 1];
            let mut out = String::new();
            let mut rest = body;
            while let Some(idx) = rest.find("${") {
                out.push_str(&rest[..idx]);
                let after = &rest[idx + 2..];
                if let Some(end) = after.find('}') {
                    let sub_val = self.eval_expr(&after[..end])?.as_text();
                    out.push_str(&sub_val);
                    rest = &after[end + 1..];
                } else {
                    out.push_str("${");
                    rest = after;
                }
            }
            out.push_str(rest);
            self.alloc(out.len())?;
            return Ok(JsVal::Str(out));
        }

        // String / number / bool / null literals
        if ((expr.starts_with('"') && expr.ends_with('"'))
            || (expr.starts_with('\'') && expr.ends_with('\'')))
            && expr.len() >= 2
        {
            let s = unescape_js_str(&expr[1..expr.len() - 1]);
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

        // Array or Object JSON-like literals (`[ ... ]` or `{ ... }`)
        if (expr.starts_with('[') && expr.ends_with(']'))
            || (expr.starts_with('{') && expr.ends_with('}'))
        {
            if let Ok(v) = serde_json::from_str::<Value>(expr) {
                let j = json_to_js(&v);
                self.alloc(j.byte_estimate())?;
                return Ok(j);
            }
            if expr.starts_with('{') && expr.ends_with('}') {
                let inner = &expr[1..expr.len() - 1];
                let mut map = BTreeMap::new();
                for pair in split_top_commas(inner) {
                    if let Some((k_raw, v_raw)) = pair.split_once(':') {
                        let k = k_raw.trim().trim_matches('"').trim_matches('\'').to_owned();
                        let v = self.eval_expr(v_raw)?;
                        map.insert(k, v);
                    }
                }
                let j = JsVal::Obj(map);
                self.alloc(j.byte_estimate())?;
                return Ok(j);
            }
        }

        // Built-in Pre-request / Post-response JS utilities
        if expr == "Date.now()" {
            let ms = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
            return Ok(JsVal::Num(ms as f64));
        }
        if expr == "new Date().toISOString()" {
            let now = time::OffsetDateTime::now_utc();
            let iso = format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                now.year(),
                now.month() as u8,
                now.day(),
                now.hour(),
                now.minute(),
                now.second()
            );
            return Ok(JsVal::Str(iso));
        }
        if expr == "crypto.randomUUID()" {
            return Ok(JsVal::Str(swaggen_domain::workspace::new_uid()));
        }
        if expr == "Math.random()" {
            let r: u32 = rand::random();
            return Ok(JsVal::Num(f64::from(r) / f64::from(u32::MAX)));
        }
        if let Some(arg) = expr
            .strip_prefix("Math.floor(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let n = self.eval_expr(arg)?.as_num().unwrap_or(0.0);
            return Ok(JsVal::Num(n.floor()));
        }
        if let Some(arg) = expr
            .strip_prefix("Math.ceil(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let n = self.eval_expr(arg)?.as_num().unwrap_or(0.0);
            return Ok(JsVal::Num(n.ceil()));
        }
        if let Some(arg) = expr
            .strip_prefix("Math.round(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let n = self.eval_expr(arg)?.as_num().unwrap_or(0.0);
            return Ok(JsVal::Num(n.round()));
        }
        if let Some(arg) = expr.strip_prefix("btoa(").and_then(|s| s.strip_suffix(')')) {
            let txt = self.eval_expr(arg)?.as_text();
            return Ok(JsVal::Str(STANDARD.encode(txt.as_bytes())));
        }
        if let Some(arg) = expr.strip_prefix("atob(").and_then(|s| s.strip_suffix(')')) {
            let b64 = self.eval_expr(arg)?.as_text();
            let bytes = STANDARD
                .decode(b64.trim())
                .map_err(|e| format!("atob decode error: {e}"))?;
            return Ok(JsVal::Str(String::from_utf8_lossy(&bytes).into_owned()));
        }
        if let Some(arg) = expr
            .strip_prefix("encodeURIComponent(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let txt = self.eval_expr(arg)?.as_text();
            let enc: String = url::form_urlencoded::byte_serialize(txt.as_bytes()).collect();
            return Ok(JsVal::Str(enc.replace('+', "%20")));
        }
        if let Some(arg) = expr
            .strip_prefix("String(")
            .and_then(|s| s.strip_suffix(')'))
        {
            return Ok(JsVal::Str(self.eval_expr(arg)?.as_text()));
        }
        if let Some(arg) = expr
            .strip_prefix("Number(")
            .or_else(|| expr.strip_prefix("parseInt("))
            .and_then(|s| s.strip_suffix(')'))
        {
            let first = split_first_top_comma(arg).map(|(a, _)| a).unwrap_or(arg);
            let n = self.eval_expr(first)?.as_num().unwrap_or(0.0);
            return Ok(JsVal::Num(n));
        }

        // Scoped variable stores: pm.environment / pm.collectionVariables / pm.globals / pm.variables
        if let Some(call) = expr.strip_prefix("pm.environment.") {
            return self.eval_var_store_call(call, VarScope::Env);
        }
        if let Some(call) = expr.strip_prefix("pm.collectionVariables.") {
            return self.eval_var_store_call(call, VarScope::Collection);
        }
        if let Some(call) = expr.strip_prefix("pm.globals.") {
            return self.eval_var_store_call(call, VarScope::Globals);
        }
        if let Some(call) = expr.strip_prefix("pm.variables.") {
            return self.eval_var_store_call(call, VarScope::Merged);
        }

        // pm.request helpers (for Pre-request dynamic header injection & inspection!)
        if expr == "pm.request.url" || expr == "pm.request.url.toString()" {
            return Ok(JsVal::Str(self.req_url.clone()));
        }
        if expr == "pm.request.method" {
            return Ok(JsVal::Str(self.req_method.clone()));
        }
        if expr == "pm.request.body.raw" {
            return Ok(JsVal::Str(self.req_body.clone()));
        }
        if let Some(call) = expr.strip_prefix("pm.request.headers.") {
            if let Some(arg) = call.strip_prefix("get(").and_then(|s| s.strip_suffix(')')) {
                let hk = self.eval_expr(arg)?.as_text().to_ascii_lowercase();
                return Ok(self
                    .req_headers
                    .get(&hk)
                    .cloned()
                    .map(JsVal::Str)
                    .unwrap_or(JsVal::Null));
            }
            if let Some(arg) = call
                .strip_prefix("add(")
                .or_else(|| call.strip_prefix("upsert("))
                .and_then(|s| s.strip_suffix(')'))
            {
                if let Some((k_e, v_e)) = split_first_top_comma(arg) {
                    let k = self.eval_expr(k_e)?.as_text();
                    let v = self.eval_expr(v_e)?.as_text();
                    self.req_headers.insert(k.to_ascii_lowercase(), v.clone());
                    self.mutated_req_headers.push((k, Some(v)));
                    return Ok(JsVal::Null);
                }
                let obj = self.eval_expr(arg)?;
                if let JsVal::Obj(map) = obj
                    && let Some(k) = map.get("key").map(JsVal::as_text)
                {
                    let v = map.get("value").map(JsVal::as_text).unwrap_or_default();
                    self.req_headers.insert(k.to_ascii_lowercase(), v.clone());
                    self.mutated_req_headers.push((k, Some(v)));
                    return Ok(JsVal::Null);
                }
            }
            if let Some(arg) = call
                .strip_prefix("remove(")
                .and_then(|s| s.strip_suffix(')'))
            {
                let k = self.eval_expr(arg)?.as_text();
                self.req_headers.remove(&k.to_ascii_lowercase());
                self.mutated_req_headers.push((k, None));
                return Ok(JsVal::Null);
            }
        }

        // console.log(a, b, ...) / console.warn / console.error
        for log_fn in ["console.log(", "console.warn(", "console.error("] {
            if let Some(args_raw) = expr.strip_prefix(log_fn).and_then(|s| s.strip_suffix(')')) {
                let mut parts = Vec::new();
                for piece in split_top_commas(args_raw) {
                    if !piece.trim().is_empty() {
                        parts.push(self.eval_expr(piece)?.as_text());
                    }
                }
                let txt = parts.join(" ");
                self.alloc(txt.len())?;
                if self.logs.len() < 150 {
                    self.logs.push(txt);
                }
                return Ok(JsVal::Null);
            }
        }

        if let Some(arg) = expr
            .strip_prefix("JSON.parse(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let txt = self.eval_expr(arg)?.as_text();
            let v: Value =
                serde_json::from_str(&txt).map_err(|e| format!("JSON.parse error: {e}"))?;
            let j = json_to_js(&v);
            self.alloc(j.byte_estimate())?;
            return Ok(j);
        }
        if let Some(arg) = expr
            .strip_prefix("JSON.stringify(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let first = split_first_top_comma(arg).map(|(a, _)| a).unwrap_or(arg);
            let v = self.eval_expr(first)?;
            let s = js_to_json(&v).to_string();
            self.alloc(s.len())?;
            return Ok(JsVal::Str(s));
        }

        // pm.response & Playwright `response` helpers
        if expr == "pm.response.code" || expr == "response.status()" || expr == "response.status" {
            return Ok(JsVal::Num(f64::from(self.resp_status)));
        }
        if expr == "pm.response.responseTime" || expr == "response.responseTime" {
            return Ok(JsVal::Num(self.resp_time_ms as f64));
        }
        if expr == "pm.response.text()" || expr == "response.text()" {
            return Ok(JsVal::Str(self.resp_body.clone()));
        }
        if expr == "pm.response.json()" || expr == "response.json()" {
            let v: Value = serde_json::from_str(&self.resp_body)
                .map_err(|e| format!("response.json() parse error: {e}"))?;
            let j = json_to_js(&v);
            self.alloc(j.byte_estimate())?;
            return Ok(j);
        }
        if let Some(arg) = expr
            .strip_prefix("pm.response.headers.get(")
            .or_else(|| expr.strip_prefix("response.headers.get("))
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

        // Property & bracket lookup chain e.g. `payload.items[0].id`, `user.name.includes("A")`
        if let Some(resolved) = self.eval_property_chain(expr)? {
            return Ok(resolved);
        }

        if let Some(v) = self.locals.get(expr) {
            return Ok(v.clone());
        }
        Err(format!(
            "Unsupported or blocked identifier/expression in sandbox: '{expr}'"
        ))
    }

    fn eval_var_store_call(&mut self, call: &str, scope: VarScope) -> Result<JsVal, String> {
        if let Some(arg) = call.strip_prefix("get(").and_then(|s| s.strip_suffix(')')) {
            let k = self.eval_expr(arg)?.as_text();
            let val = match scope {
                VarScope::Env => self.env.get(&k).cloned(),
                VarScope::Collection => self.collection_vars.get(&k).cloned(),
                VarScope::Globals => self.globals.get(&k).cloned(),
                VarScope::Merged => self
                    .request_vars
                    .get(&k)
                    .or_else(|| self.env.get(&k))
                    .or_else(|| self.collection_vars.get(&k))
                    .or_else(|| self.globals.get(&k))
                    .cloned(),
            };
            return Ok(val.map(JsVal::Str).unwrap_or(JsVal::Null));
        }
        if let Some(args) = call.strip_prefix("set(").and_then(|s| s.strip_suffix(')'))
            && let Some((k_e, v_e)) = split_first_top_comma(args)
        {
            let k = self.eval_expr(k_e)?.as_text();
            let v = self.eval_expr(v_e)?.as_text();
            self.alloc(k.len() + v.len())?;
            match scope {
                VarScope::Env => {
                    self.env.insert(k.clone(), v.clone());
                    self.mutated_env.insert(k, Some(v));
                }
                VarScope::Collection => {
                    self.collection_vars.insert(k.clone(), v.clone());
                    self.mutated_collection_vars.insert(k, Some(v));
                }
                VarScope::Globals => {
                    self.globals.insert(k.clone(), v.clone());
                    self.mutated_globals.insert(k, Some(v));
                }
                VarScope::Merged => {
                    self.request_vars.insert(k.clone(), v.clone());
                    self.mutated_request_vars.insert(k.clone(), Some(v.clone()));
                    self.env.insert(k.clone(), v.clone());
                    self.mutated_env.insert(k, Some(v));
                }
            }
            return Ok(JsVal::Null);
        }
        if let Some(arg) = call
            .strip_prefix("unset(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let k = self.eval_expr(arg)?.as_text();
            match scope {
                VarScope::Env => {
                    self.env.remove(&k);
                    self.mutated_env.insert(k, None);
                }
                VarScope::Collection => {
                    self.collection_vars.remove(&k);
                    self.mutated_collection_vars.insert(k, None);
                }
                VarScope::Globals => {
                    self.globals.remove(&k);
                    self.mutated_globals.insert(k, None);
                }
                VarScope::Merged => {
                    self.request_vars.remove(&k);
                    self.mutated_request_vars.insert(k.clone(), None);
                    self.env.remove(&k);
                    self.mutated_env.insert(k, None);
                }
            }
            return Ok(JsVal::Null);
        }
        Err(format!("Unsupported pm variable method: '{call}'"))
    }

    fn eval_postman_expect(&mut self, lhs: JsVal, tail: &str) -> Result<JsVal, String> {
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
        if let Some(exp_arg) = tail
            .strip_prefix(".to.include(")
            .and_then(|s| s.strip_suffix(')'))
        {
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
        if let Some(exp_arg) = tail
            .strip_prefix(".to.be.below(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let limit = self.eval_expr(exp_arg)?.as_num().unwrap_or(0.0);
            let actual = lhs.as_num().unwrap_or(f64::MAX);
            if actual < limit {
                return Ok(JsVal::Bool(true));
            }
            return Err(format!(
                "Assertion failed: expected {actual} to be below {limit}"
            ));
        }
        if let Some(exp_arg) = tail
            .strip_prefix(".to.be.above(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let limit = self.eval_expr(exp_arg)?.as_num().unwrap_or(0.0);
            let actual = lhs.as_num().unwrap_or(f64::MIN);
            if actual > limit {
                return Ok(JsVal::Bool(true));
            }
            return Err(format!(
                "Assertion failed: expected {actual} to be above {limit}"
            ));
        }
        if tail == ".to.exist" || tail == ".to.not.be.null" {
            if !matches!(lhs, JsVal::Null) {
                return Ok(JsVal::Bool(true));
            }
            return Err("Assertion failed: expected value to exist".into());
        }
        if tail == ".to.be.true" {
            if matches!(lhs, JsVal::Bool(true)) {
                return Ok(JsVal::Bool(true));
            }
            return Err(format!(
                "Assertion failed: expected true, got '{}'",
                lhs.as_text()
            ));
        }
        if tail == ".to.be.false" {
            if matches!(lhs, JsVal::Bool(false)) {
                return Ok(JsVal::Bool(true));
            }
            return Err(format!(
                "Assertion failed: expected false, got '{}'",
                lhs.as_text()
            ));
        }
        if tail == ".to.not.be.empty" {
            let non_empty = match &lhs {
                JsVal::Str(s) => !s.is_empty(),
                JsVal::Arr(a) => !a.is_empty(),
                JsVal::Obj(m) => !m.is_empty(),
                JsVal::Null => false,
                _ => true,
            };
            if non_empty {
                return Ok(JsVal::Bool(true));
            }
            return Err("Assertion failed: expected value not to be empty".into());
        }
        Err(format!("Unsupported pm.expect matcher: '{tail}'"))
    }

    fn eval_playwright_expect(&mut self, lhs: JsVal, tail_raw: &str) -> Result<JsVal, String> {
        let (negated, tail) = if let Some(rest) = tail_raw.strip_prefix(".not.") {
            (true, format!(".{rest}"))
        } else {
            (false, tail_raw.to_owned())
        };
        let check = |cond: bool, fail_msg: String| -> Result<JsVal, String> {
            let ok = if negated { !cond } else { cond };
            if ok {
                Ok(JsVal::Bool(true))
            } else {
                Err(fail_msg)
            }
        };

        if let Some(arg) = tail
            .strip_prefix(".toBe(")
            .or_else(|| tail.strip_prefix(".toEqual("))
            .and_then(|s| s.strip_suffix(')'))
        {
            let rhs = self.eval_expr(arg)?;
            return check(
                lhs.as_text() == rhs.as_text(),
                format!(
                    "expect(received).{}toBe(expected): expected '{}', received '{}'",
                    if negated { "not." } else { "" },
                    rhs.as_text(),
                    lhs.as_text()
                ),
            );
        }
        if let Some(arg) = tail
            .strip_prefix(".toContain(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let rhs = self.eval_expr(arg)?;
            let sub = rhs.as_text();
            let has = match &lhs {
                JsVal::Arr(items) => items.iter().any(|i| i.as_text() == sub),
                _ => lhs.as_text().contains(&sub),
            };
            return check(
                has,
                format!(
                    "expect(received).toContain('{}') failed on '{}'",
                    sub,
                    lhs.as_text()
                ),
            );
        }
        if tail == ".toBeTruthy()" {
            return check(
                lhs.as_truthy(),
                format!(
                    "expect(received).toBeTruthy() failed on '{}'",
                    lhs.as_text()
                ),
            );
        }
        if tail == ".toBeFalsy()" {
            return check(
                !lhs.as_truthy(),
                format!("expect(received).toBeFalsy() failed on '{}'", lhs.as_text()),
            );
        }
        if tail == ".toBeDefined()" {
            return check(
                !matches!(lhs, JsVal::Null),
                "expect(received).toBeDefined() failed (got null/undefined)".into(),
            );
        }
        if tail == ".toBeNull()" {
            return check(
                matches!(lhs, JsVal::Null),
                format!("expect(received).toBeNull() failed on '{}'", lhs.as_text()),
            );
        }
        if let Some(arg) = tail
            .strip_prefix(".toBeLessThan(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let n = self.eval_expr(arg)?.as_num().unwrap_or(0.0);
            let actual = lhs.as_num().unwrap_or(f64::MAX);
            return check(
                actual < n,
                format!("expect({actual}).toBeLessThan({n}) failed"),
            );
        }
        if let Some(arg) = tail
            .strip_prefix(".toBeLessThanOrEqual(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let n = self.eval_expr(arg)?.as_num().unwrap_or(0.0);
            let actual = lhs.as_num().unwrap_or(f64::MAX);
            return check(
                actual <= n,
                format!("expect({actual}).toBeLessThanOrEqual({n}) failed"),
            );
        }
        if let Some(arg) = tail
            .strip_prefix(".toBeGreaterThan(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let n = self.eval_expr(arg)?.as_num().unwrap_or(0.0);
            let actual = lhs.as_num().unwrap_or(f64::MIN);
            return check(
                actual > n,
                format!("expect({actual}).toBeGreaterThan({n}) failed"),
            );
        }
        if let Some(arg) = tail
            .strip_prefix(".toBeGreaterThanOrEqual(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let n = self.eval_expr(arg)?.as_num().unwrap_or(0.0);
            let actual = lhs.as_num().unwrap_or(f64::MIN);
            return check(
                actual >= n,
                format!("expect({actual}).toBeGreaterThanOrEqual({n}) failed"),
            );
        }
        if let Some(arg) = tail
            .strip_prefix(".toHaveProperty(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let prop = self.eval_expr(arg)?.as_text();
            let has = match &lhs {
                JsVal::Obj(map) => map.contains_key(&prop),
                _ => false,
            };
            return check(
                has,
                format!("expect(object).toHaveProperty('{prop}') failed"),
            );
        }
        Err(format!("Unsupported expect() matcher: '{tail_raw}'"))
    }

    fn eval_property_chain(&mut self, expr: &str) -> Result<Option<JsVal>, String> {
        let tokens = parse_access_chain(expr);
        if tokens.len() < 2 {
            return Ok(None);
        }
        let AccessSeg::Prop(root_name) = &tokens[0] else {
            return Ok(None);
        };
        let Some(mut cur) = self.locals.get(root_name).cloned() else {
            return Ok(None);
        };
        for seg in &tokens[1..] {
            match seg {
                AccessSeg::Prop(p) => {
                    if let Some(arg) = p
                        .strip_prefix("includes(")
                        .and_then(|s| s.strip_suffix(')'))
                    {
                        let needle = self.eval_expr(arg)?.as_text();
                        let found = match &cur {
                            JsVal::Str(s) => s.contains(&needle),
                            JsVal::Arr(items) => items.iter().any(|x| x.as_text() == needle),
                            _ => false,
                        };
                        cur = JsVal::Bool(found);
                    } else if p == "toLowerCase()" {
                        cur = JsVal::Str(cur.as_text().to_ascii_lowercase());
                    } else if p == "toUpperCase()" {
                        cur = JsVal::Str(cur.as_text().to_ascii_uppercase());
                    } else if p == "trim()" {
                        cur = JsVal::Str(cur.as_text().trim().to_owned());
                    } else if p == "toString()" {
                        cur = JsVal::Str(cur.as_text());
                    } else {
                        match cur {
                            JsVal::Obj(map) => {
                                cur = map.get(p.as_str()).cloned().unwrap_or(JsVal::Null);
                            }
                            JsVal::Arr(arr) if p == "length" => {
                                cur = JsVal::Num(arr.len() as f64);
                            }
                            JsVal::Str(s) if p == "length" => {
                                cur = JsVal::Num(s.len() as f64);
                            }
                            _ => cur = JsVal::Null,
                        }
                    }
                }
                AccessSeg::Index(idx_expr) => {
                    let idx_val = self.eval_expr(idx_expr)?;
                    match (cur, idx_val) {
                        (JsVal::Arr(arr), JsVal::Num(n)) if n >= 0.0 => {
                            cur = arr.get(n as usize).cloned().unwrap_or(JsVal::Null);
                        }
                        (JsVal::Obj(map), k) => {
                            cur = map.get(&k.as_text()).cloned().unwrap_or(JsVal::Null);
                        }
                        _ => cur = JsVal::Null,
                    }
                }
            }
        }
        Ok(Some(cur))
    }
}

#[derive(Clone, Copy)]
enum VarScope {
    Env,
    Collection,
    Globals,
    Merged,
}

enum AccessSeg {
    Prop(String),
    Index(String),
}

fn parse_access_chain(expr: &str) -> Vec<AccessSeg> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut in_str: Option<char> = None;
    for c in expr.chars() {
        if let Some(q) = in_str {
            if c == q {
                in_str = None;
            }
            cur.push(c);
            continue;
        }
        match c {
            '"' | '\'' | '`' => {
                in_str = Some(c);
                cur.push(c);
            }
            '(' => {
                paren += 1;
                cur.push(c);
            }
            ')' => {
                paren = paren.saturating_sub(1);
                cur.push(c);
            }
            '[' if paren == 0 && bracket == 0 => {
                if !cur.trim().is_empty() {
                    out.push(AccessSeg::Prop(std::mem::take(&mut cur).trim().to_owned()));
                } else {
                    cur.clear();
                }
                bracket = 1;
            }
            '[' if bracket > 0 => {
                bracket += 1;
                cur.push(c);
            }
            ']' if bracket == 1 && paren == 0 => {
                bracket = 0;
                out.push(AccessSeg::Index(std::mem::take(&mut cur).trim().to_owned()));
            }
            ']' if bracket > 1 => {
                bracket -= 1;
                cur.push(c);
            }
            '.' if paren == 0 && bracket == 0 => {
                if !cur.trim().is_empty() {
                    out.push(AccessSeg::Prop(std::mem::take(&mut cur).trim().to_owned()));
                } else {
                    cur.clear();
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(AccessSeg::Prop(cur.trim().to_owned()));
    }
    out
}

fn unescape_js_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\'') => out.push('\''),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut in_str: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = in_str {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                i += 1;
                out.push(chars[i]);
            } else if c == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        if matches!(c, '"' | '\'' | '`') {
            in_str = Some(c);
            out.push(c);
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn split_statements(src: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut brace = 0usize;
    let mut paren = 0usize;
    let mut in_str: Option<char> = None;
    let mut prev = '\0';
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        cur.push(c);
        if let Some(q) = in_str {
            if c == q && prev != '\\' {
                in_str = None;
            }
        } else {
            match c {
                '"' | '\'' | '`' => in_str = Some(c),
                '{' => brace += 1,
                '}' => {
                    brace = brace.saturating_sub(1);
                    if brace == 0 && paren == 0 {
                        // Peek ahead to keep `else` attached to `if (...) { ... }`
                        let mut clone_it = chars.clone();
                        let mut next_word = String::new();
                        while let Some(&nc) = clone_it.peek() {
                            if nc.is_whitespace() {
                                clone_it.next();
                            } else {
                                break;
                            }
                        }
                        for _ in 0..4 {
                            if let Some(nc) = clone_it.next() {
                                next_word.push(nc);
                            }
                        }
                        if next_word != "else" {
                            out.push(std::mem::take(&mut cur));
                        }
                    }
                }
                '(' => paren += 1,
                ')' => paren = paren.saturating_sub(1),
                ';' | '\n' if brace == 0 && paren == 0 => {
                    if !cur.trim().is_empty() {
                        out.push(std::mem::take(&mut cur));
                    } else {
                        cur.clear();
                    }
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

fn split_top_commas(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut paren = 0usize;
    let mut brace = 0usize;
    let mut bracket = 0usize;
    let mut in_str: Option<char> = None;
    let mut prev = '\0';
    let mut start = 0usize;
    for (idx, c) in s.char_indices() {
        if let Some(q) = in_str {
            if c == q && prev != '\\' {
                in_str = None;
            }
        } else {
            match c {
                '"' | '\'' | '`' => in_str = Some(c),
                '(' => paren += 1,
                ')' => paren = paren.saturating_sub(1),
                '{' => brace += 1,
                '}' => brace = brace.saturating_sub(1),
                '[' => bracket += 1,
                ']' => bracket = bracket.saturating_sub(1),
                ',' if paren == 0 && brace == 0 && bracket == 0 => {
                    out.push(&s[start..idx]);
                    start = idx + 1;
                }
                _ => {}
            }
        }
        prev = c;
    }
    out.push(&s[start..]);
    out
}

fn split_first_top_comma(s: &str) -> Option<(&str, &str)> {
    let mut paren = 0usize;
    let mut brace = 0usize;
    let mut bracket = 0usize;
    let mut in_str: Option<char> = None;
    let mut prev = '\0';
    for (idx, c) in s.char_indices() {
        if let Some(q) = in_str {
            if c == q && prev != '\\' {
                in_str = None;
            }
        } else {
            match c {
                '"' | '\'' | '`' => in_str = Some(c),
                '(' => paren += 1,
                ')' => paren = paren.saturating_sub(1),
                '{' => brace += 1,
                '}' => brace = brace.saturating_sub(1),
                '[' => bracket += 1,
                ']' => bracket = bracket.saturating_sub(1),
                ',' if paren == 0 && brace == 0 && bracket == 0 => {
                    return Some((&s[..idx], &s[idx + 1..]));
                }
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
    let mut bracket = 0usize;
    let mut in_str: Option<char> = None;
    let mut prev = '\0';
    for (idx, c) in s.char_indices() {
        if let Some(q) = in_str {
            if c == q && prev != '\\' {
                in_str = None;
            }
        } else {
            match c {
                '"' | '\'' | '`' => in_str = Some(c),
                '(' => paren += 1,
                ')' => paren = paren.saturating_sub(1),
                '{' => brace += 1,
                '}' => brace = brace.saturating_sub(1),
                '[' => bracket += 1,
                ']' => bracket = bracket.saturating_sub(1),
                _ if paren == 0 && brace == 0 && bracket == 0 && s[idx..].starts_with(op) => {
                    // Guard `<` and `>` from matching `<=`, `>=`, `=>`
                    if (op == "<" || op == ">") && s[idx + 1..].starts_with('=') {
                        prev = c;
                        continue;
                    }
                    if op == ">" && prev == '=' {
                        prev = c;
                        continue;
                    }
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
    let mut brace = 0usize;
    let mut in_str: Option<u8> = None;
    for i in 0..bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q && (i == 0 || bytes[i - 1] != b'\\') {
                in_str = None;
            }
            continue;
        }
        match b {
            b'"' | b'\'' | b'`' => in_str = Some(b),
            b'(' => paren += 1,
            b')' => paren = paren.saturating_sub(1),
            b'{' => brace += 1,
            b'}' => brace = brace.saturating_sub(1),
            b'=' if paren == 0 && brace == 0 => {
                let prev = i.checked_sub(1).and_then(|j| bytes.get(j)).copied();
                let next = bytes.get(i + 1).copied();
                if !matches!(prev, Some(b'=' | b'!' | b'<' | b'>'))
                    && !matches!(next, Some(b'=' | b'>'))
                {
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
    let mut prev = '\0';
    for (idx, c) in s.char_indices() {
        if let Some(q) = in_str {
            if c == q && prev != '\\' {
                in_str = None;
            }
        } else {
            match c {
                '"' | '\'' | '`' => in_str = Some(c),
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
        prev = c;
    }
    None
}

fn split_balanced_brace(s: &str) -> Option<(&str, &str)> {
    let mut depth = 1usize;
    let mut in_str: Option<char> = None;
    let mut prev = '\0';
    for (idx, c) in s.char_indices() {
        if let Some(q) = in_str {
            if c == q && prev != '\\' {
                in_str = None;
            }
        } else {
            match c {
                '"' | '\'' | '`' => in_str = Some(c),
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((&s[..idx], &s[idx + 1..]));
                    }
                }
                _ => {}
            }
        }
        prev = c;
    }
    None
}

fn extract_paren_brace_and_tail(s: &str) -> Option<(&str, &str, &str)> {
    let after_p = s.strip_prefix('(')?;
    let (cond, rest) = split_balanced_paren(after_p)?;
    let after_b = rest.trim_start().strip_prefix('{')?;
    let (body, tail) = split_balanced_brace(after_b)?;
    Some((cond, body, tail))
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
    if script.trim().is_empty() {
        return SandboxExecutionReport {
            compatibility: analyze_postman_script(script),
            ..SandboxExecutionReport::default()
        };
    }
    let compat = analyze_postman_script(script);
    let headers_lc: BTreeMap<String, String> = ctx
        .response_headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v.clone()))
        .collect();
    let req_headers_lc: BTreeMap<String, String> = ctx
        .request_headers
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
        collection_vars: ctx.collection_vars.clone(),
        globals: ctx.global_vars.clone(),
        request_vars: ctx.request_vars.clone(),
        mutated_env: BTreeMap::new(),
        mutated_collection_vars: BTreeMap::new(),
        mutated_globals: BTreeMap::new(),
        mutated_request_vars: BTreeMap::new(),
        req_method: ctx.request_method.clone(),
        req_url: ctx.request_url.clone(),
        req_headers: req_headers_lc,
        mutated_req_headers: Vec::new(),
        req_body: ctx.request_body.clone(),
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
        mutated_collection_vars: vm.mutated_collection_vars,
        mutated_globals: vm.mutated_globals,
        mutated_request_vars: vm.mutated_request_vars,
        mutated_request_headers: vm.mutated_req_headers,
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
    fn sandbox_executes_postman_and_playwright_scripts_and_enforces_limits() {
        let mut env = BTreeMap::new();
        env.insert("role".into(), "admin".into());
        let ctx = ScriptContextSnapshot {
            env_vars: env,
            response_status: 200,
            response_time_ms: 14,
            response_body: r#"{"token":"tk-99","items":[{"id":7}],"user":{"name":"Alice"}}"#.into(),
            response_headers: vec![("X-Env".into(), "staging".into())],
            ..ScriptContextSnapshot::default()
        };

        let script = r#"
// Pre/post automation with Postman + Playwright syntax
const payload = pm.response.json();
const nonce = crypto.randomUUID();
pm.environment.set("extracted_user", payload.user.name);
pm.collectionVariables.set("first_item_id", payload.items[0].id);
pm.globals.set("req_nonce", nonce);
pm.request.headers.upsert("X-Trace-Id", `trace-${payload.items[0].id}`);

pm.test("Status is 200 and token matches", () => {
    pm.response.to.have.status(200);
    pm.expect(pm.response.code).to.eql(200);
    pm.expect(payload.token).to.include("tk-");
});

test("Playwright style assertions work cleanly", () => {
    expect(response.status()).toBe(200);
    expect(payload.items[0].id).toBe(7);
    expect(payload.user).toHaveProperty("name");
    expect(response.responseTime).toBeLessThan(500);
});
console.log("done", payload.user.name);
"#;
        let rep = execute_sandboxed_script(script, &ctx, SandboxLimits::default());
        assert!(rep.error.is_none(), "unexpected error: {:?}", rep.error);
        assert_eq!(
            rep.mutated_env.get("extracted_user").unwrap().as_deref(),
            Some("Alice")
        );
        assert_eq!(
            rep.mutated_collection_vars
                .get("first_item_id")
                .unwrap()
                .as_deref(),
            Some("7")
        );
        assert!(rep.mutated_globals.contains_key("req_nonce"));
        assert_eq!(
            rep.mutated_request_headers,
            vec![("X-Trace-Id".to_owned(), Some("trace-7".to_owned()))]
        );
        assert_eq!(rep.test_results.len(), 2);
        assert!(rep.test_results.iter().all(|t| t.passed));
        assert_eq!(rep.logs, vec!["done Alice".to_owned()]);

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
        assert!(
            inf.error
                .as_deref()
                .unwrap_or("")
                .contains("max instruction steps")
        );

        // Unsupported Postman API triggers explicit migration report & blocks require('fs')
        let fs_script = "const fs = require('fs'); pm.sendRequest('https://evil.test');";
        let fs_rep = execute_sandboxed_script(fs_script, &ctx, SandboxLimits::default());
        assert!(fs_rep.error.is_some());
        assert!(fs_rep.compatibility.unsupported_apis.len() >= 2);
    }
}
