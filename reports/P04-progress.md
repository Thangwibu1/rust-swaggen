# P04 — Server MongoDB & Internal Accounts (Local Gate Passed; Live Mongo Env-Gated)

Status: **local HTTP + DPAPI + RBAC + index/transaction code verified** (2026-11-24). Proceeding to P05 while live MongoDB execution remains gated on `SWAGGEN_MONGO_URI`.

## Implemented and verified

- **`crates/mongo-store/` (`crypto.rs`, `error.rs`, `repo.rs`, `memory_backend.rs`, `mongo_backend.rs`, `lib.rs`)**:
  - `Argon2id` password hashing (`argon2 = "=0.5.3"`) with 16-byte random salt per password; 32-byte CSPRNG opaque session/invite tokens stored ONLY as SHA-256 hex hashes in DB; 15-minute sliding login failure rate limiter (`LoginRateLimiter`).
  - `MongoStore` (`mongodb = "=3.10.2"`, `redact-errors`, `bson = "=2.15.0"`): replica-set `hello` primary/session topology check, idempotent index/migration initializer across 16 MongoDB collections (`users`, `sessions`, `invitations`, `workspaces`, `memberships`, `workspace_permission_fences`, `workspace_counters`, `entities`, `revisions`, `applied_operations`, `change_events`, `comments`, `notifications`, `attachments`, `audit_events`, `schema_migrations`), `ClientSession` multi-document transactions (`ReadConcern::snapshot()`, `WriteConcern::majority()`, transient & `UnknownTransactionCommitResult` retry), permission-fence check inside entity write transactions, and `run_transaction_rollback_smoke_test()`.
  - `InMemoryStore`: implements the exact `WorkspaceRepository` contract & RBAC/isolation/one-time-invite/expiry/rate-limit invariants for offline test suites and `--in-memory-dev`.
- **`crates/api-contract/src/lib.rs` & `apps/server/` & `07-operations/server-openapi.json`**:
  - Axum `/v1/auth/*` and `/v1/workspaces/*` endpoints, `Role` matrix (`Owner/Admin/Editor/Viewer`), cross-workspace isolation on every route, Viewer write/invite/audit blocking (`403`), one-time invitation create/revoke/expiry/account-mismatch enforcement, exported backend OpenAPI 3.1 contract (`07-operations/server-openapi.json`), and `swaggen-server doctor-mongo` command.
- **`crates/domain/src/server_profiles.rs` & `crates/local-store/src/server_profiles_store.rs` & `apps/desktop/` (`server_client.rs`, `server_worker.rs`, `views/server.rs`)**:
  - Multi-server profile persistence (`servers.journal`, TLS `https://` validation with host-scoped self-signed override, API version capability probe), Windows DPAPI isolation for session tokens (`SecretRef` under `server-auth` — plaintext tokens never written to `servers.journal`), and native UI screens (`A05/A06/W03/W04`, `Z02`, `A01`, `A10/W05/W19/W20/W21`, `C01/C02/W06/X03`, `S07`).

## Verification results

- `cargo +stable test --workspace --offline --locked --target x86_64-pc-windows-msvc`: **50 passed, 0 failed, 0 ignored** (including `swaggen-mongo-store` 2/2, `swaggen-server` full HTTP E2E 1/1, `swaggen-desktop` worker server/DPAPI isolation 7/7).
- `cargo +stable clippy --workspace --all-targets --offline --locked --target x86_64-pc-windows-msvc -- -D warnings`: **PASS** (0 warnings).
- `cargo +stable tree -p swaggen-desktop --offline --locked --target x86_64-pc-windows-msvc | Select-String "mongodb|bson|argon2"`: **0 hits** (DB/server crypto strictly excluded from desktop).
- Windows workspace `cargo-audit` (`reports/raw/p04-audit.json`): **361 packages, 0 vulnerabilities, 0 warnings**.
- Linux server closure `cargo-audit` (`reports/raw/p04-linux-server-audit.json`): **218 packages, 0 vulnerabilities, 0 warnings**.

## Env-gated / open product items

- Live MongoDB replica-set execution (`swaggen-server doctor-mongo`) awaits `SWAGGEN_MONGO_URI` in the runtime environment.
- External Google OAuth (`A01-A04/S02/Z10`) remains unimplemented pending product-owner Google credential decision (`01-prd/00-authoritative-decisions.md`).
