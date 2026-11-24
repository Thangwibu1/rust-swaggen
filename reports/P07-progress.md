# P07 — Version B Core Client, OpenAPI & Specs (Gate Passed)

Status: **Version B Core Client, OAuth2 PKCE/Proxy/CA/mTLS, Dynamic Vars, Response Diff, Timings, and Advanced OpenAPI/Docs/Git Export verified** (2026-11-24).

## Implemented and verified

- **Network, Transport Timings & Progress (`crates/http-engine/src/lib.rs`, `apps/desktop/src/worker.rs`, `apps/desktop/src/views/request_pane.rs`, `response_pane.rs`)**:
  - `RQ10`: `TransferProgress` channel streams upload/download phase & byte counts via `Event::HttpProgress` to the active request bar.
  - `RS10`: `TimingBreakdown` measures DNS (`dns_ms`), TCP (`tcp_ms`), TLS (`tls_ms`), TTFB (`ttfb_ms`), and total duration (`total_ms`) per execution and displays them in Response pane.
  - `AU07` & `AU09`: `ProxyPolicy` (`http_proxy`, `https_proxy`, `bypass_hosts`), `custom_ca_pem_paths`, and host/port `client_certs` mTLS (including expiry validation & pure-Rust PEM decoding) wired into `RequestConnectionPolicy` & desktop Connection tab.
  - `AU08`: `crates/http-engine/src/oauth2.rs` implements S256 `PkceChallenge`, `127.0.0.1` loopback listener with strict CSRF `state` verification (`await_loopback_callback`), and form-encoded token exchange storing access tokens in local DPAPI `SecretStore`.
- **Dynamic Variables, Response Extractors & Diff (`crates/domain/src/dynamic_vars.rs`, `workspace.rs`, `apps/desktop/src/`)**:
  - `EV08`: Dynamic tokens `{{$uuid}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}`, `{{$randomHex}}` evaluated cleanly during variable substitution & execution.
  - `EV09`: `ResponseExtractorRule` (`BodyJsonPath` / `Header`) extracts response values after HTTP completion into environment variables or DPAPI secrets (`mark_secret`).
  - `RS09`: `compare_responses` flattens JSON bodies and headers with configurable ignore-path rules, rendered in the Response `Diff (RS09)` tab.
- **Advanced OpenAPI, Docs & Git/Attachments (`crates/spec-engine/src/advanced.rs`, `apps/desktop/src/views/specs.rs`, `import_export.rs`)**:
  - `OA07`–`OA10`: `inspect_complex_schema` (`allOf`/`oneOf`/`anyOf`/`discriminator`/depth+cycle bounds), `resolve_and_bundle_multifile` (traversal-safe local `$ref`), `convert_swagger2_to_openapi3`, and `lint_spec_conventions`.
  - `DC07`–`DC10`, `IM07`, `IM09`: `validate_response_against_endpoint` + `detect_contract_drift`, XSS-safe `export_docs_html`/`export_docs_markdown`, `propose_spec_patch_from_request`, `bundle_shared_attachments`, and deterministic `export_git_project_tree`.

## Verification Evidence

- `cargo +stable clippy --workspace --all-targets --offline --locked --target x86_64-pc-windows-msvc -- -D warnings`: **PASS** (0 warnings).
- `cargo +stable test --workspace --offline --locked --target x86_64-pc-windows-msvc`: **57 passed, 0 failed, 0 ignored** (`desktop` 8, `domain` 10, `http-engine` 3, `local-store` 21, `mongo-store` 3, `secret-store` 3, `server` 2, `spec-engine` 6, `sync` 1).
