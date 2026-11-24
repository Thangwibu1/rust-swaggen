# P09 — Governance, UX B, MFA/Audit & Self-Hosted Updates (Gate Passed)

Status: **All 15 P09 requirements implemented, tested, and verified** (2026-11-24).

## Implemented and verified

- **Workspace Governance (`crates/mongo-store/src/{repo,memory_backend,mongo_backend}.rs`, `apps/server/src/lib.rs`)**:
  - `WS09`: Owner-only `/v1/workspaces/{id}/archive` toggle (`archived: bool`) with audit trail across both `InMemoryStore` and `MongoStore` transactions.
  - `WS10`: Atomic `transfer_workspace_ownership` (`Owner` -> `Admin`, target -> `Owner`, bumps `membership_revision` + `workspace_permission_fences`) and `leave_workspace` strictly blocking un-transferred `Owner` (`409 Conflict`).
- **Team Collaboration, Presence, Sanitized Share & Deep Links (`crates/domain/src/governance.rs`, `apps/server/src/lib.rs`, `apps/desktop/src/`)**:
  - `CO05`: `parse_mentions` + server `create_comment` generate `mention` notifications to mentioned workspace members.
  - `CO07`: `/v1/workspaces/{id}/presence` TTL-bounded editing presence heartbeat & active viewer feed.
  - `CO08`: `create_sanitized_share_package` strips secrets, cookies, authorization headers, and local file paths cleanly (`Response -> Copy Sanitized Share JSON`).
  - `CO09`, `SY10`: `parse_deep_link` enforces safe path segments + active workspace membership check; `/v1/workspaces/{id}/branches` and `/merge-requests` evaluate `evaluate_branch_merge` and reject conflicted merges (`409`).
- **TOTP MFA, Hash-Chained Audit & Signed Updates (`crates/domain/src/governance.rs`, `apps/server/src/lib.rs`, `apps/desktop/src/views/panels.rs`)**:
  - `SC09`: RFC 6238 HMAC-SHA256 TOTP (`verify_totp_sha256` with +/-1 skew), single-use SHA-256 recovery codes (`verify_and_consume_recovery_code`), and Owner/Admin MFA reset (`/members/{user_id}/reset-mfa`).
  - `SC10`, `OP10`: `/v1/workspaces/{id}/audit/export` retention-filtered hash-chained export (`build_hash_chained_audit_export`, explicitly noting WORM/SIEM boundary limits) + seat-unrestricted resource quotas.
  - `OP08`: `verify_signed_update` verifies HMAC-SHA256 update manifests + SHA-256 artifact checksums, rejecting modified manifests or tampered binaries.
- **UX B (`crates/ui-kit/src/lib.rs`, `apps/desktop/src/{app,views/panels}.rs`)**:
  - `UX04`, `UX06`, `UX07`, `UX08`: `Ctrl+K` Command Palette (`Escape` close), Light/Dark theme switch & English/Vietnamese UI string switcher (preserving API payloads), AccessKit keyboard navigation (`Tab`/focus/tree), and OS file drop safe OpenAPI/spec import.

## Verification Evidence

- `cargo +stable clippy --workspace --all-targets --offline --locked --target x86_64-pc-windows-msvc -- -D warnings`: **PASS** (0 warnings).
- `cargo +stable test --workspace --offline --locked --target x86_64-pc-windows-msvc`: **63 passed, 0 failed, 0 ignored**.
- `cargo +stable tree -p swaggen-desktop --offline --locked | Select-String "mongodb|bson|argon2"`: **0 matches**.
