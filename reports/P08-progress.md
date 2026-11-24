# P08 — Collection Runner, CLI, Mock Server, SSRF Guard, Quotas & GraphQL/SSE/Dev Tools (Gate Passed)

Status: **All 17 P08 features implemented, wired to desktop UI / headless CLI / Axum server, and verified** (2026-11-24).

## Implemented and verified

- **Collection Runner & Reports (`crates/runner/src/collection_runner.rs`, `apps/desktop/src/views/runner.rs`)**:
  - `TS02`–`TS06`, `TS10`: Multi-iteration collection execution with per-step delay, instant `CancellationToken`, response extractor dependency chaining, CSV/JSON `parse_dataset` with inferred column types, strict separation of `Passed`, `FailedAssertions`, `FailedContract`, `Skipped`, and `InfraError`, OpenAPI response validation toggle, SHA-256 `RunManifest`, and JUnit XML / redacted JSON export.
- **Headless CLI (`apps/cli/src/{lib,main}.rs`)**:
  - `MK05`–`MK06`: `swaggen-cli run` & `swaggen-cli snippet` resolve `SWAGGEN_SECRET_*` or DPAPI local secrets without writing plaintext secrets to JUnit XML/JSON/stdout/stderr, returning deterministic CI exit codes (`0` pass, `1` assertion/contract failure, `2` infra/usage error).
- **Local Mock Server, Team Mocks & Quotas (`crates/runner/src/local_mock.rs`, `crates/api-contract/src/lib.rs`, `apps/server/src/lib.rs`)**:
  - `MK01`, `MK02`: Local Axum mock server built from sanitized `ResponseExample`s and deterministic schema-seeded OpenAPI specs with redacted traffic capture.
  - `MK04`, `MK10`: Server `/v1/workspaces/{id}/mocks`, `/mock/{workspace_id}/{*tail}`, and `/v1/workspaces/{id}/quotas` enforce workspace RBAC (`Viewer` blocked `403`, owner/admin/creator delete rules), 5s delay cap, 50-route/workspace quota (`429 quota_exceeded`), and payload size caps (verified via Axum HTTP E2E test over `InMemoryStore`; durable Mongo mock-route collection noted for multi-instance production).
- **SSRF Guard (`crates/domain/src/ssrf.rs`, `crates/runner/src/ssrf.rs`, `apps/server/src/lib.rs`)**:
  - `SC08`: Blocks `localhost`, `.internal`/`.local`, RFC1918 private, loopback, `169.254.169.254` cloud metadata, CGNAT, ULA, unspecified, IPv4-mapped IPv6 (`::ffff:127.0.0.1`), and non-HTTP(S) schemes unless explicitly allowlisted, plus async DNS IP validation and `/v1/security/check-ssrf`.
- **GraphQL, Bounded SSE, Snippets & Dev Utils (`crates/runner/src/protocols_and_tools.rs`, `apps/desktop/src/views/{request_pane,runner}.rs`)**:
  - `PR01`, `PR04`, `PR07`, `PR09`: GraphQL composer separating `data` & `errors`; bounded SSE streaming client with configurable `max_events` & `max_buffer_bytes` FIFO eviction and cancel; secret-masked code snippets (cURL, Rust `reqwest`, JS `fetch`, Dart `http`, Go `net/http`); offline URL, Base64, JSON pretty/minify, and YAML->JSON utilities.

## Verification Evidence

- `cargo +stable clippy --workspace --all-targets --offline --target x86_64-pc-windows-msvc -- -D warnings`: **PASS** (0 warnings across all 13 workspace crates).
- `cargo +stable test --workspace --offline --locked --target x86_64-pc-windows-msvc`: **62 passed, 0 failed, 0 ignored** (`cli` 1, `desktop` 8, `domain` 10, `http-engine` 3, `local-store` 21, `mongo-store` 3, `runner` 4, `secret-store` 3, `server` 2, `spec-engine` 6, `sync` 1).
- `cargo +stable tree -p swaggen-desktop --offline --locked | Select-String "mongodb|bson|argon2"`: **0 matches** (strict client isolation preserved).
- Offline `cargo-audit`: `reports/raw/p08-audit.json` (`0` vulnerabilities).
