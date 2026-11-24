# Current checkpoint — P01 started, first local vertical slice verified

P00 remains passed (13/13); P01 is **in progress**, P02-P12 not started. Resume from `reports/P01-progress.md`, `p01-progress.json`, `p01-checks.json`, `p01-ui-checks.json`, `p01-dependency-check.json` and the product files. Do not rebuild from scratch. No Git repository/diff/commit exists.

New product Cargo workspace: apps/desktop, crates/domain/local-store/secret-store/ui-kit. Root excludes original spikes; their source/locks remain unchanged. Exact product versions were source-reviewed and separately audited as a full Windows dependency closure (169 external packages, deny warnings, no ignores). New DPAPI APIs reviewed from windows-sys0.61.2 and Microsoft docs; actual Windows DPAPI test passed. P00 report/evidence tools now acknowledge a product scaffold and preserve in-progress ledgers rather than claiming no app/not-started forever.

Latest code checks: Rust1.100, fmt, 22 Windows Rust tests (no ignored), all-target Clippy, executable build, product forced-process-crash recovery passed. Actual UI: offline startup, Vietnamese/plain-script-text draft Ctrl+S save; restart restored revision1, unsaved-close return preserved edits then revision2 saved. Final rebuilt executable restored117-byte revision2; secret reference retrieval after restart returned the disposable value; Hide removed plaintext from UIA and correctly said Secret value hidden. Native keyboard traversal worked on selected controls. Actual app journal bytes contain no plaintext disposable secret. See p01-ui-binary.json fingerprint and p01-data-observation.json. No full DPI/IME/accessibility or performance/installer acceptance is claimed.

The app is left open, disposable test secret hidden. Native app's normal Windows profile data differs from sandbox-redirected shell LOCALAPPDATA; use the actual app path for read-only evidence. Background shell GUI attempts were not counted as native validation. Remote Mongo was not rerun, and no secret URI was read or production service changed. Existing P00 prototype fixture/data stays intact.

Progress: SC01/SC02/SC05/SC06/SC07 in_progress; E03/S06 in_progress, not verified/complete. All other requirement/screen statuses retained. Remaining P01 tasks: service/error/recovery/retry boundaries and tests; reusable focus/components and diagnostics; path/ACL/race failure qualification; server and Windows/Linux CI foundation; additional product native checks. Start with P01-progress.md remaining-gate list. Do not start P02 or mark P01 passed based on this slice.

The following entries are historical snapshots and do not override current P01 status.

# Current completion - P00 foundation passed, 2026-11-24

Authoritative generated result: `reports/p00-gate.json` and `reports/preflight.md`, 13/13 foundation criteria passed. P01-P12 remain not started; no product scaffold, installer, production deployment or product requirement completion is claimed.

The owner's security runner installed cargo-audit 0.23.0 and downloaded the current RustSec DB. Original audits passed p00/transport and failed egui/Iced. Investigation found egui's three maintenance warnings are Linux-only packages absent from its Windows dependency closure. Iced's Windows closure genuinely contains unmaintained packages and unsound lru 0.16.4; Iced is rejected for both accessibility and safety, must not be distributed, and its FAIL evidence remains intact.

Completed explicit security disposition and ADR update before acceptance. Fresh Cargo-resolved complete Windows graphs for adopted p00 (with Mongo), p00-egui and p00-transport passed with warnings denied and no advisory ignores. Audited the original full locks and rejected Iced too, preserving FAIL results. Generated audit-only locks bind every reachable transitive/build/proc-macro package to original versions/sources/checksums. Hash checks invalidate evidence when input manifests, locks, metadata or projections change. Native Rust source, original lockfiles and dependency choices were not changed during this security disposition. See `p00-security-disposition.md` for the reason this is a platform/rejected-candidate disposition, not a warning waiver.

The local repeat used the owner's freshly fetched DB with `--no-fetch`, extra <=48-hour commit-age guard and no `--stale`. The security helper normally fetches current data; `-UseLocalDatabase` permits this guarded local reproduction. Installation uses its private cache, then restores the application Cargo cache for metadata. All 21 current Python regressions passed; earlier real Windows UI, 14 journal unit, 6 crash-matrix, 3 native model and 5 egui editor/splitter results remain valid and were not relabelled as new runs. The successful remote Mongo fixture was not rerun or reconfigured.

Chosen foundation: eframe/egui 0.37.0 native Windows, Axum 0.8.9/Tokio 1.54.1 transport/server, official MongoDB driver 3.10.2 on the verified replica-set fixture, and framed Rust local journal with tested Windows replace/write-through/reopen/flush compaction. Details and limits: ADR-0001/0002 and p00-final-native.md.

Next authorized phase can be P01 after reading its phase plan. Preserve 160 requirement and 316 screen ledgers as not_started until actual product slices are implemented. Full screen/design parity, all-DPI/multi-monitor/accessibility matrices, product sync/security/migrations, release percentiles, exact upstream notice completion, clean installer/signing and deployment remain their own later gates. The following sections preserve historical state and must not override this completion.

# Swaggen continuation checkpoint — 2026-11-24

## Latest foundation completion work

Authoritative current gate: `reports/p00-gate.json`, generated from explicit required evidence by `tools/p00-gate.py` / `tools/p00-report.py`. The gate is no longer hardcoded false. Current Windows/source/storage/transaction foundation checks are recorded; the current advisory audit is the remaining external blocker. Do not mark that audit PASS from the owner's instruction to continue or from successful source compilation.

Implemented and observed: JSON/YAML lexical highlighting with bounded cache; Unicode-safe match navigation; real native Telex key input; undo/redo; keyboard-modal isolation; virtual request-number and Alt+Up/Down navigation; pointer event batching and width retention fixes for splitter; native window resizing at actual175% scale; Windows Narrator speech verified from visible Speech recap. Final release repeated Telex/undo/redo, request1000/999 navigation and real journal restore by keyboard. See `p00-final-native.md`, `p00-completion-ui-checks.json`, and `egui-final-binary.json`. Narrator was stopped after the test; its absence was observed.

Factored shared real loopback transport into `spikes/p00-transport`; egui streamed an actual20MiB response with64KiB preview. Every raw byte/hash and saved journal CRC/draft/outbox were independently verified in `egui-artifact-integrity.json`. No production API, sync or remote database call was introduced into the desktop.

Strengthened Windows compaction with exact windows-sys0.61.2 MoveFileExW replace/write-through. Windows refused the operation with open data handles; the implementation now closes those handles while retaining the sidecar writer fence, replaces, reopens and flushes. Failure after replacement poisons the writer until explicit recovery. Current14 journal unit cases,6 process-compaction crash cases,3 native model cases,5 egui editor/splitter cases, builds and Clippy passed. Future production migrations/scoping, full all-screen/all-DPI/accessibility matrices, physical disk/power-loss qualification, signing/installers and percentile budgets remain later-phase requirements.

Dependency source policy reviewed439 selected Windows packages: versions, MSRV, official registry and license choices; preserved ANDed font/data license terms; collected available notices.42 published crates omit notice files, so complete their exact upstream notices before distribution; this is not described as a complete release bundle. CPU model/logical processors/physical memory and workspace file permissions were recorded in `host-check.json`. Attempted idle telemetry could not uniquely select the release process; no idle budget is certified.

Remaining action: from an ordinary network-enabled terminal at this project, run `powershell -NoProfile -ExecutionPolicy Bypass -File tools/p00-security-check.ps1`. It installs pinned cargo-audit0.23.0 inside `.local`, denies warnings, audits all four lockfiles with current RustSec data, and refreshes the formal gate/evidence reports. No DB credentials are needed. Current agent attempts failed DNS for index.crates.io and GitHub. If it finds an advisory, fix/review it; never add ignores or accept stale data solely to make P00 green. P01–P12 remain not started and product ledgers160/316 remain untouched.

The remaining sections below are historical snapshots, superseded by this section and the generated gate where they describe older pending cache/IME/Narrator/selection states.

## Current continuation — Mongo PASS and real egui comparison

P00 remains blocked, P01–P12 not started. Current sources include `spikes/p00-egui` and `tools/p00-egui-check.ps1`; this is another isolated spike, not a product scaffold. See `egui-comparison.md` for the exact Windows observations and unresolved limitations.

- Owner's Mongo probe passed: replica-set primary, server 7.0.48 / FCV 7.0, commit/invisibility, explicit rollback, duplicate-key rollback and fixture cleanup. The latest `mongo-runtime` record in `p00-checks.json` supersedes the older blocked `mongo-check.json`. No credential was read or reused by this continuation and no DB operation was rerun.
- Eframe/egui 0.37.0 is now cached and pinned. Source/API/MSRV/license identifiers were inspected. Native MSVC debug build, format, Clippy and target-filtered metadata passed. Internal controls are exposed to Windows accessibility APIs. Unicode paste, virtual list to request 1000, UIA search, selected Tab traversal and dialog focus restoration were observed. System font fallback fixed missing glyphs and repeated accented extracted text. Framework remains proposed; geometry, IME, full Narrator/keyboard/modal behavior, DPI, splitter/resize and release testing remain incomplete.
- Re-ran 11 journal unit cases, journal process-crash smoke, six compaction process-crash cases, Mongo compilation, three native model tests, formatting and Clippy; all passed. These remain scoped experiments, not power-loss or product acceptance.
- Fixed evidence retention in PowerShell 5.1: `p00-evidence-store.ps1` keeps scopes not rerun and replaces a rerun scope even when it fails. Its real PowerShell 5.1 regression checks flat arrays and preserves the Mongo result. An initial bad merge was preserved in `raw/p00-runner-first-merge.json`, then flattened from its original records without changing their statuses/timestamps. Six Python report regressions cover time ordering, failure supersession, timezone validation, preserved precision of legacy observation dates, unrelated scopes and existing ledger preservation. See reports/evidence-regression-checks.json for the captured run.
- Report generator now includes egui metadata/UI evidence, selects the newest Mongo record by actual timestamp and does not list the successful Mongo fixture as a blocker. Developer doctor output explicitly identifies its preliminary scope.
- The comparison window is left open with disposable fixture text. No user app/system settings/permissions were changed. No installer, signing, performance or deployment acceptance. Requirement/screen ledgers remain 160/316 with zero product entries verified.

Current next steps: finish UIA geometry/Invoke/text ranges and modal/virtual keyboard coverage; real IME/Narrator/OS DPI/multi-monitor tests; candidate highlighted editor, local durability and large-response integration; release performance/renderer recovery; complete transitive dependency review/current RustSec audit; local-store migration/directory durability/power-loss evidence. Do not advance to P01 before the required compatibility gate. Do not repeat the successful Mongo probe just to refresh reports.

Verification commands: `python -X utf8 -m unittest discover -s tools/tests -p test_p00_report.py`; `powershell -NoProfile -ExecutionPolicy Bypass -File tools/tests/test-p00-evidence-store.ps1`; `python -X utf8 tools/p00-report.py`; `python -X utf8 tools/p00-evidence-audit.py`. Close only the comparison fixture before rebuilding it. `tools/p00-egui-check.ps1` compiles/lints it without claiming UI acceptance.

## Historical baseline before this continuation

The following notes describe the prior state and attempts. Current results above supersede their old cache/Mongo descriptions and next actions.

Active phase: P00, BLOCKED with substantial independent evidence. P01–P12 have not started. No product scaffold exists. No Git repository or commit exists in this directory.

Latest continuation: see `reports/renderer-diagnostics-continuation.md` and `reports/software-renderer.md`. Scoped evidence now includes 11 journal, 3 native model and 3 Mongo diagnostic unit cases, plus six compaction process-crash cases. These are not all one new run; older results are preserved with their commands/timestamps. The software drawing build and input smoke are observed, while accessibility and Mongo transaction gates remain blocked.

Current source review baseline: `reports/code-fingerprints.json` (review-time hashes, not a test signature or Git diff). The older `durability-evidence.json` fingerprints belong to their historical review and are preserved. Software mode can be rechecked with `tools/p00-renderer-check.ps1`; diagnostic cases with `tools/p00-mongo-diagnostics-check.ps1`. Software UI observations are debug-profile-specific and do not promote release or accessibility gates.

## Current evidence

- Owner successfully ran the dependency installer: candidate crate fetch exit 0. Iced 0.14.0, Axum 0.8.9 and Windows transitive sources are cached and locked. Rust 1.100.0/MSVC/Windows SDK were reused.
- Built `spikes/p00-native` on Windows. Real native app was launched through the native app tool; screenshot and accessibility observations are documented in `reports/native-ui.md`. The app was left available after user input was detected.
- Observed Vietnamese Unicode paste, Ctrl+Z/Ctrl+Y, exact search, virtual scrolling to request 1000, Ctrl+P/Escape, real 20MiB local HTTP streaming with a 64KiB preview, and local journal save/readback.
- Verified every byte and SHA-256 of the actual 20MiB file. Journal readback verified the 146-byte UTF-8 fixture draft and no publish intent.
- Application-scale 200% initially clipped/collapsed the layout. Added compact/wrapping controls and re-observed the editor at 200%; 125% was also observed. OS per-monitor DPI is not certified.
- Accessibility probe exposes window chrome only; internal editor/list/buttons are absent. Iced is not approved. A bridge or egui/eframe comparison is required.
- Journal now has stable cross-process writer fence, bounded replay/file quota, full-state compaction/atomic replacement and poisoned-writer recovery. 10 journal unit tests and 3 native history/virtualization tests passed; process termination and multi-process lock smoke passed. Formatting and Clippy passed for both packages.
- Shell HTTP test failed with Windows socket access error 10013. Native interactive HTTP succeeded separately; the failed shell result is preserved, not relabeled green.
- Retried Mongo using process-local supplied URI; exit 2, no accepted topology/FCV/transaction result. No credential was saved. Execution-environment DNS/socket restrictions remain.
- Native comparison `eframe@0.37.0` was not found in the offline cache. Installer now supports `-CompareUI`.
- Requirement/screen ledgers remain 160/316, with zero product requirements or screens verified. Missing source renders remain 464. No Docker, production deployment, installer/signing or performance acceptance.

## Next actions

1. Read `reports/preflight.md`, `native-ui.md`, the dependency matrix and UI/storage ADRs. Keep P00 blocked; do not bootstrap the product from prototype success.
2. Fetch the native comparison candidate from a terminal with network access: `powershell -NoProfile -ExecutionPolicy Bypass -File tools/install-p00-deps.ps1 -CompareUI`. Inspect its exact APIs/license/MSRV before implementing a comparison; alternatively establish an actual accessible Iced integration.
3. Run the isolated Mongo test from a terminal with network/socket permissions: `powershell -NoProfile -ExecutionPolicy Bypass -File tools/p00-mongo.ps1`. It reuses a private environment URI or prompts without echo/persistence. Do not store the URI in command history or files. Do not reset/reconfigure the existing database server.
4. The Mongo helper refreshes reports after its run; the report generator selects the latest timestamped Mongo record from either p00-checks.json or mongo-check.json, preserving older attempts without treating them as current.
5. Complete accessibility, real Vietnamese IME composition, Tab traversal, Narrator, OS DPI/multi-monitor, resize/split panes, automatic GPU-device-loss recovery and measured release benchmarks before UI selection. Explicit software-mode drawing/input now has separate evidence; it is not full device-loss or release acceptance. Clipboard Unicode is not IME acceptance.
6. Complete storage migration, directory durability, physical disk-full and power-loss work before store adoption. Six deterministic compaction process-crash boundaries and partial-write StorageFull/WriteZero injections are now covered. No production encryption/secrets adapter exists yet.
7. `powershell -NoProfile -ExecutionPolicy Bypass -File tools/p00-check.ps1 -NativeLibrary -Durability` reruns library/unit/Clippy and compaction crash checks without replacing a running GUI binary. `-Native` also builds binaries and attempts the shell HTTP fixture; close the probe before rebuilding binaries.
8. `python -X utf8 tools/p00-report.py` and `python -X utf8 tools/p00-evidence-audit.py` refresh/audit the handoff. These never promote P00 or reset an existing product ledger.
9. Only after P00 passes, finalize evidence-based ADRs and proceed P01→P12 with genuine vertical slices.

Root working files are authoritative. The nested `Swaggen-Agent-Kit/` source copy is unchanged. Raw outputs, current records and preserved initial failures are in reports/. Source-only dependency metadata is not a RustSec/license clearance or a performance benchmark.
