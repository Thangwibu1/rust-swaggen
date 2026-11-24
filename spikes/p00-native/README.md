> REJECTED COMPARISON — do not distribute or adopt this executable. Iced 0.14.0 failed the accessibility comparison and its actual Windows dependency graph contains unmaintained crates and unsound lru 0.16.4 (RUSTSEC-2026-0253). Full and Windows audit FAIL results are retained. The selected native foundation is ../p00-egui. See ../../reports/p00-security-disposition.md and ADR-0001-ui.md. The following instructions preserve the historical experiment only.

# P00 native framework experiment

Separate compatibility package; no product scaffold or framework approval. Uses pinned Iced 0.14.0, Axum 0.8.9 and Tokio 1.54.1, plus the local journal spike. The UI labels its 1,000 requests and HTTP server as fixtures.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/p00-check.ps1 -Native
cargo +stable run --manifest-path spikes/p00-native/Cargo.toml --offline --locked --bin native-gate
```

The harness verifies the installed stable alias is Rust 1.100.0 and checks MSVC/SDK before building. Standard Cargo commands with the root rust-toolchain pin require the named toolchain; `+stable` is an existing-installation workaround after exact-version verification.

Native probe: 1440×900 logical window, virtual list with only visible rows and overscan built, syntax-highlighted multiline JSON/YAML, 128KiB editor cap, 16 snapshot undo/redo states, exact substring match count, worker-thread journal save, isolated focus dialog and application scale controls. Search does not navigate/highlight matches; folding, draggable split panes, complete Tab traversal and production editor semantics are not claimed. App scale buttons are not Windows per-monitor DPI validation. There is no production API client, workspace, account or sync implementation.

Send starts an actual local Axum server on an ephemeral loopback port and transfers a deterministic 20MiB body over HTTP/1.1. Raw bytes are checked and streamed to a unique fixture file; only a 64KiB preview is retained. The transport accepts only its local fixture address; it does not accept arbitrary URLs, TLS, redirects or credentials. Fixture outputs are explicitly confined to `.local/p00-native` under the project, regardless of launcher working directory. UI operations use async tasks/spawn_blocking and allow at most one save and transfer at a time.

The shell execution environment may deny socket operations (observed Windows error 10013). That is a blocked runtime environment, not a successful HTTP test. Keep shell and interactive app observations separate. Read `reports/native-ui.md` for the actual manual UI observations and remaining accessibility/IME/DPI/performance gates.

If the first framework cannot meet the accessibility gate, download the native comparison candidate through the existing official-registry installer:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/install-p00-deps.ps1 -CompareUI
```

This fetches eframe 0.37.0 alongside the initial candidates. Its exact APIs/license/MSRV and native behavior must be inspected and tested before selection; no fallback is silently adopted. No Docker or web frontend is used.

## Isolated software renderer

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/p00-renderer-check.ps1
python -X utf8 tools/p00-software-evidence.py
```

The package's default feature forwards `iced/default` to preserve the earlier candidate configuration. `--no-default-features --features software` instead selects tiny-skia without the wgpu renderer. The helper builds into `target-software`, preserving the earlier binary. Compile/test/Clippy and feature-tree results are distinct from the observed interactive run described in `reports/software-renderer.md`. No physical GPU disabling, automatic device-loss recovery or release benchmark is claimed. A release build option is available for subsequent measurement work; it does not create runtime acceptance evidence automatically.
