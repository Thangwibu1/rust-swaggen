# P01 — Foundation & UI Kit (Gate Passed)

Status: **passed P01 foundation gate** (2026-11-24). Proceeding to P02.

## Implemented and verified in P01

- **Cargo workspace & toolchain pin**: `rust-toolchain.toml` pins `1.100.0`; `Cargo.lock` locks all workspace crates (`apps/desktop`, `apps/server`, `crates/domain`, `crates/api-contract`, `crates/local-store`, `crates/secret-store`, `crates/ui-kit`). `.github/workflows/foundation.yml` defines Windows + Linux server CI jobs (Linux job not executed locally on this Windows host).
- **Domain & Security (`crates/domain`)**:
  - Validated `LocalId`, `SecretRef` workspace/name binding, `Secret` with `Drop` zeroization & redacted `Debug`, payload-free `Error`.
  - `SC02` `RedactionPolicy` & `sanitize_text`: redacts sensitive headers (`Authorization`, `Cookie`, `Set-Cookie`, `X-Api-Key`, etc.), query params, JSON fields, key=value tokens, and user-designated fields/values with explicit manual-review warning.
  - `SC05` `sanitize_imported_markup` (`Z15`): neutralizes `<script>`, `<iframe>`, `<svg>`, inline `on*=` handlers, `javascript:`/`vbscript:`/`data:text/html` URIs, and flags external `http(s)://` references without auto-fetching.
  - `SC06`/`SC07` `DesktopPolicy`: analytics off, crash upload off, remote HTML execution blocked, external ref fetch blocked, typed-commands-only boundary.
- **Local persistence (`crates/local-store`)**:
  - Framed versioned journal with CRC32, `sync_all`, Windows `MoveFileExW` (`REPLACE_EXISTING | WRITE_THROUGH`) compaction, sidecar writer lock fence, workspace isolation, reparse/junction/symlink rejection, read-only poisoning & reopen recovery, and explicit non-destructive `quarantine_corrupt_draft` / `quarantine_corrupt_secrets`.
- **Windows DPAPI secrets (`crates/secret-store`)**:
  - Current-user `CryptProtectData` / `CryptUnprotectData` (`CRYPTPROTECT_UI_FORBIDDEN`) with workspace/name entropy binding, `put`, `get`, `list_names` (names only), `delete`, corrupt-file preservation.
- **UI Kit & Native Desktop (`crates/ui-kit`, `apps/desktop`)**:
  - White-surface Design System (`51:40541`, `4:139`, `4:120`): reusable `Tokens`, `section`, `surface_card`, `primary_button`, `secondary_button`, `danger_button`, `method_badge`, `status_badge`, `notice_banner`, `code_surface`.
  - Bounded non-blocking worker actor (`mpsc::sync_channel(8)`), `Local draft` with recovery/quarantine actions, `Personal secrets (E03/V09)` with list/save/reveal/hide/delete, `Sanitizer & preview (SC02/SC05/Z15)`, `Privacy & permissions (S06/Z07)` with manual clipboard/file export.
- **Server transport foundation (`crates/api-contract`, `apps/server`)**:
  - Versioned `API_VERSION = 1` (`ServiceStatus`, `Capabilities`, `ErrorEnvelope`), `/health/live`, `/health/ready` (fail-closed 503 `foundation_only`), `/v1/health/*`, `/v1/capabilities`, `x-request-id` correlation middleware, canary non-disclosure, graceful shutdown.

## Verified command results

| Check | Result | Evidence |
| --- | --- | --- |
| Toolchain & fmt | PASS (`rustc 1.100.0`) | `reports/p01-checks.json` |
| Windows workspace tests | PASS (35 passed, 0 failed, 0 ignored) | `reports/raw/p01-unit.txt` |
| Clippy (`--all-targets -- -D warnings`) | PASS | `reports/raw/p01-clippy.txt` |
| Desktop debug build | PASS | `reports/raw/p01-build.txt`; `reports/p01-ui-binary.json` |
| Product forced process crash probe | PASS | `reports/raw/p01-process-crash.txt` |
| Windows advisory audit (`cargo-audit 0.23.0`) | PASS (200 packages, 0 ignores, 0 warnings) | `reports/p01-dependency-check.json` |
| Desktop offline UIA launch smoke | PASS (20 UIA controls exposed) | `reports/p01-ui-binary.json` |

## Cross-phase scope notes

- `SC07` (`S06`, `Z07`) is verified in `reports/requirement-ledger.json` (no git repo exists so `commit` is null).
- `SC01`, `SC02`, `SC05`, `SC06` have verified P01 local/foundation slices and remain `in_progress` because they also govern P02–P10 screens (`S12`, `V07`, `Q32`, `H05`, `C20`, `S11`, `S13`, `T18`, `P10`).
- 464 Figma renders remain missing (`02-design/export-status.md`). Linux CI runner, release benchmarks, MSI installer, and signing remain later-phase gates.
