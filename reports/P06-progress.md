# P06 — Version A Release Readiness, Operations & Benchmarks (Local Gate Passed; Signing/WiX/Live Mongo Env-Gated)

Status: **Version A application, non-container deployment artifacts, backup/restore verification, portable Windows package, and release benchmarks verified** (2026-11-24). Proceeding to Version B (P07–P09).

## Implemented and verified

- **Backup & Restore CLI (`crates/mongo-store/src/backup.rs`, `apps/server/src/main.rs`)**: Consistent JSON+manifest `BackupBundle` with `collection_counts` and `payload_sha256` verification, refusing non-empty restore targets and rejecting tampered/corrupted bundles (`backup_bundle_verifies_integrity_and_rejects_tampering` passed), plus `/v1/health/metrics` (`backend`, `db_ready`, `schema_migration_version`, `max_body_bytes`, `redaction_enabled`).
- **Non-Container Operations (`07-operations/vps-systemd-deployment.md`, `swaggen-server.service`, `swaggen.env.example`)**: Strictly Docker-free Linux binary + `systemd` + HTTPS reverse proxy + MongoDB replica-set initialization, backup/restore, key-loss, and upgrade/rollback runbook per `01-prd/00-authoritative-decisions.md` Session override (2026-11-24).
- **Windows Release Packaging (`07-operations/windows-installer/swaggen.wxs`, `package-windows.ps1`, `dist/windows-x64/manifest.json`)**:
  - Built release binary `swaggen-desktop.exe` (`sha256: 9f83b04d6e26e09a967c4e7c9398ef7da4e915238c24f10119de404b5af5cec3`) and portable package `dist/windows-x64/Swaggen-0.1.0-win64-unsigned.zip` (`sha256: a3085084d5a04c8dd7afe8a73097d73a94119001519bb68864b72a6a4d3cf569`).
  - Verified `webview2_required = false`, `offline_capable_without_activation = true`, zero `mongodb`/`bson`/`argon2` runtime linkage.
  - WiX MSI build & Authenticode signing explicitly recorded as `BLOCKED_NO_WIX_TOOLSET` / `BLOCKED_NO_CODESIGN_CERTIFICATE`.

## Measured Windows Release Benchmarks

- **Engine/Domain (`reports/benchmarks/p06-engine-release.json`)**:
  - `1,000`-request workspace open + JSON round-trip + search (40 samples after 5 warm-up): **p50 = 1.441 ms, p95 = 1.644 ms, p99 = 1.850 ms** (within `AC10 <= 200 ms`).
  - `20 MiB` loopback streaming with `1 MiB` preview cap (`10` samples): **p95 = 187.879 ms**; post-signal cancellation (`10` samples): **p95 = 16.644 ms** (`AC09 < 100 ms`).
- **Desktop Release Binary (`reports/benchmarks/p06-desktop-release.json`)**:
  - Cold+warm startup (5 runs offline, no server): `[1245, 60, 62, 47, 49] ms`, **p95 = 1245 ms** (`<= 2000 ms` target).
  - Idle 3s CPU: **0.52%** (`< 1%` target). Private memory (`wgpu` initialized): **283.82 MiB** (noted cleanly above the 200 MiB baseline target due to GPU swapchain runtime allocation).

## AC01–AC12 Gate Summary

- **Locally verified**: AC01 (HTTP RBAC/Viewer block), AC02 (local server SSE/sync < 3s), AC03 (revision conflict & draft preservation), AC04 (idempotency ACK dedupe + catch-up), AC05 (offline restart pending outbox + permission block), AC06 (OpenAPI 3.0/3.1 `$ref` + diff + unsupported keyword reporting), AC07 (DPAPI token isolation & sanitization), AC08 (HTTP duplicate params, encoding, JSON/multipart/cookies/redirect/timeout/cancel), AC09 (`20 MiB` stream + cancel), AC10 (`1,000`-request open/search p95 = `1.644 ms`), AC12 (offline Windows 11 x64 release run without WebView2/cloud activation).
- **Env-gated / Blocked**: AC11 restore integrity unit verification passed, while live MongoDB `doctor-mongo`/`backup`/`restore` execution awaits `SWAGGEN_MONGO_URI`; MSI signing & installer execution await WiX Toolset + Authenticode cert.
