# P00 dependency compatibility - adopted Windows source policy and advisory checks passed

The current source review covers 439 unique packages across adopted and rejected Windows comparison graphs, including declared MSRV, license choices and official registry provenance. The adopted p00/egui/transport graphs passed cargo-audit 0.23.0 with warnings denied and no advisory ignores. Iced is rejected for accessibility and genuine Windows maintenance/unsoundness findings; its FAIL audits remain visible. Egui's original cross-platform lock retains three Linux-only maintenance warnings, absent from its complete Cargo-resolved Windows graph. See `p00-security-disposition.md` and `security-checks.json` for exact scope and integrity checks. No product release is certified by this foundation audit.

Available upstream notices were collected; 42 published archives lack notice files. Complete the exact upstream notice bundle before distribution. The source-policy PASS does not imply that distribution paperwork is complete.

## Historical inventory before final selection and security disposition

The rows below preserve earlier candidate observations. Where they say not selected or pending, the current decision above and ADR-0001 take precedence.

Checked 2026-11-24. Exact versions below are either observed installed artifacts or explicitly marked candidates. No product framework has been selected. The owner successfully fetched initial candidates from their terminal; additional network access remains restricted inside the execution environment.

| Component | Exact version | Evidence / license / MSRV | Status and limits |
| --- | --- | --- | --- |
| Rust / Cargo | rustc 1.100.0, Cargo from installed stable | `reports/doctor.json`; x86_64-pc-windows-msvc; Rust distribution MIT/Apache-2.0 | Windows compile/link and execution demonstrated by journal tests. `rust-toolchain.toml` pins 1.100.0; existing installation is named stable. Harness checks exact compiler before using `+stable`. |
| MSVC Build Tools | installation 18.12.12412.261 | vswhere presence; Microsoft terms apply | Present and linker exercised by native Rust spike. Not a claim of GUI or installer validation. |
| Windows SDK | 10.0.26100.0 directory | installed SDK registry/directory presence; Microsoft SDK terms | Console native build works; graphics, accessibility and packaging acceptance pending. |
| Iced | 0.14.0 candidate | Cached official crate: MIT, manifest MSRV 1.88; versioned source APIs inspected; native lockfile | Windows build and selected real UI interactions observed. Accessibility tree lacks internal controls; full native gate not passed. No 0.15-dev APIs. Release/maintenance/advisory review remains incomplete. |
| egui / eframe | 0.37.0 pinned comparison | Cached official manifests: MIT OR Apache-2.0, MSRV 1.95; [versioned docs](https://docs.rs/eframe/0.37.0/eframe/) | Downloaded, Windows build/Clippy passed. Internal controls, Unicode paste, virtual list, search and selected focus/dialog interactions observed. System font fallback corrected missing Vietnamese glyphs/extracted text. See egui-comparison.md for unresolved geometry, IME/Narrator/DPI/editor/release gates. Not selected. |
| Axum | 0.8.9 candidate | Cached official crate: MIT, manifest MSRV 1.80; versioned source APIs inspected | Compiled; real interactive-app loopback fixture transferred and verified 20MiB. Shell fixture failed socket permission 10013 and remains a separate failed run. No product backend scaffold. |
| MongoDB official driver | 3.10.2 pinned | cached official crate, Cargo.lock; Apache-2.0; manifest rust-version 1.88 | Windows compile and isolated transaction runtime passed; server 7.0.48 / FCV 7.0. Docs retrieved via web list 1.94.1+; installed compiler satisfies both, discrepancy retained for review. See transaction-smoke.md; no product publish/revoke-race acceptance. |
| Tokio | 1.54.1 pinned | cached manifest; MIT; rust-version 1.85; [docs](https://docs.rs/tokio/1.54.1/tokio/) | macros, rt-multi-thread, net, time; bounded workers/UI integration not evaluated. |
| serde_json | 1.0.154 pinned | cached manifest; MIT OR Apache-2.0; rust-version 1.71 | Used for safe allowlisted topology output only. No unbounded_depth feature. |
| Journal spike | std only, Rust 1.100.0 | repository source; stable writer sidecar, sync_all, atomic replacement | 10 journal unit tests plus process-termination/multi-process lock smoke passed; local UI save reopened successfully. Not accepted as production local store. |
| Docker / Compose / testcontainers | none | User instruction 2026-11-24 | NOT_APPLICABLE, prohibited. No invocation, installation or container setup. |
| mongosh / database tools | not installed through this work | Rust driver used for P00 connection and transaction probe | Shell not needed for driver smoke. Backup/restore tools must be selected and tested before operations gate. |
| WiX / signing / protoc / Java / Node | not selected for product | phase-specific prerequisites | No bulk installation. No signing, installer, gRPC or generator claim. |

## Locked feature/API checks

`spikes/p00/Cargo.lock` pins the spike dependency graph. Mongo uses default `compat-3-0-0`, `rustls-tls`, `dns-resolver`, plus `redact-errors`. BSON comes from the driver's re-export (bson 2.15.0 in this graph). No desktop application depends on MongoDB.

Read cached source for the exact Mongo version: README cancellation warning, `src/action/transaction.rs`, ClientOptions and transaction methods. The spike polls driver futures to completion, uses internal server-selection/connect timeouts, executes session operations sequentially, sets snapshot/majority on the commit/explicit abort cases, and retries unknown commit results within a bounded loop. An unresolved commit never produces a successful acceptance result. It does not implement product-level transaction retry, ACK dedupe, permission fencing or publish.

`reports/raw/cargo-metadata.json` was captured with `--offline --locked --features mongo --filter-platform x86_64-pc-windows-msvc`. Unfiltered metadata initially requested uncached wasm dependencies; that failed and is not represented as success. `reports/dependency-inventory.json` contains package metadata, not a security audit or complete license notice bundle.

## Remaining research gate

Current RustSec database/tools cannot be fetched in this environment. Full license obligations, release dates, maintenance status, advisories, target support and native requirements must be checked for every selected package before approval. Cached registry metadata is not evidence of no advisories. Framework and storage ADRs remain proposed. No container digest applies under the user's no-Docker constraint; a non-container server deployment path needs validation before release.
