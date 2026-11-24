# P11 — Advanced Protocols, CRDT, Runners/Schedules & SDK Stubs (Gate Passed)

Status: **All 13 P11 requirements (`AU10, CO10, IM10, TS09, MK03, MK07, MK08, MK09, PR02, PR03, PR05, PR06, PR08`) implemented, tested, and verified** (2026-11-24).

## Implemented and verified

- **AWS SigV4 & HTTP Digest Auth (`crates/runner/src/advanced_protocols.rs`, `apps/desktop/src/views/runner.rs`)**:
  - `AU10`: Canonical AWS4-HMAC-SHA256 signer (`sign_aws_sigv4`), RFC 7616 SHA-256 Digest auth (`build_http_digest_sha256`), and explicit `auth_scheme_matrix()` marking NTLM/Kerberos as unsupported without native SSPI bridge.
- **CRDT Convergence & Publish Revoke Guard (`crates/sync/src/crdt.rs`)**:
  - `CO10`: `CollaborativeTextCrdt` with deterministic `(lamport, actor, seq)` ordering and tombstones verified commutative & idempotent; `merge_if_authorized` rejects `Viewer`/revoked writes.
- **HAR 1.2 & `.http` Importers (`crates/spec-engine/src/har_http.rs`, `apps/desktop/src/views/import_export.rs`)**:
  - `IM10`: `import_har` and `import_dot_http` import requests and `@var` variables while scrubbing `Authorization`/`Cookie`/`X-Api-Key` headers cleanly with compatibility warnings.
- **Bounded Parallel Collection Runner (`crates/runner/src/collection_runner.rs`)**:
  - `TS09`: `run_collection_parallel` executes shards concurrently via bounded `tokio::sync::Semaphore` with isolated environments and deterministic result order.
- **Scenario Mocks, Runner Leases, DST Cron & Failure Webhooks (`crates/runner/src/advanced_protocols.rs`, `crates/api-contract/src/lib.rs`, `apps/server/src/lib.rs`)**:
  - `MK03`: `match_scenario_rule` matches header, query, and body substring with delay & sanitized logs.
  - `MK07`–`MK09`: `/v1/workspaces/{id}/runners/register` & `/heartbeat` leases; `CronSchedule::next_fire_unix` skips spring-forward gaps & fall-back duplicate slots; `/v1/workspaces/{id}/schedules` validates `failure_webhook_url` against `SsrfPolicy` and emits redacted failure webhook payloads.
- **GraphQL SDL, RFC 6455 WebSocket, gRPC Wire Codec, HTTP/2 Policy & SDK Stubs (`crates/runner/src/advanced_protocols.rs`, `apps/desktop/src/views/specs.rs`)**:
  - `PR02`, `PR03`, `PR05`, `PR06`, `PR08`: `validate_graphql_against_sdl`; pure-Rust RFC 3174 SHA-1 + RFC 6455 masked WebSocket handshake/echo (`ws_exchange_text`); 5-byte gRPC length framing + protobuf varint/string codec (`encode_grpc_frame`, `decode_grpc_frame`, `encode_proto_string_field`); `HttpProtocolPreference`; deterministic Rust `reqwest` & TypeScript `fetch` SDK stub generators (`generate_sdk_stub`).

## Verification Evidence

- `cargo +stable clippy --workspace --all-targets --offline --locked --target x86_64-pc-windows-msvc -- -D warnings`: **PASS** (0 warnings).
- `cargo +stable test --workspace --offline --locked --target x86_64-pc-windows-msvc`: **68 passed, 0 failed, 0 ignored**.
- `cargo +stable tree -p swaggen-desktop --offline --locked | Select-String "mongodb|bson|argon2"`: **0 matches**.
