# P00 preflight - PASS

Generated: 2026-11-24T10:29:49.527550+00:00

P00 Windows native UI, local storage and controlled-transfer spike evidence only. Current implementation phase is recorded separately in 04-phases/status.md.

| Foundation criterion | Result |
| --- | --- |
| Windows toolchain and link | PASS |
| Selected UI compile and editor regression | PASS |
| Native editor, search and keyboard | PASS |
| Native Vietnamese input | PASS |
| Native resize and DPI smoke | PASS |
| Screen reader | PASS |
| Integrated local save and restore | PASS |
| Integrated bounded 20 MiB response | PASS |
| Journal crash and corruption | PASS |
| Real replica-set transactions | PASS |
| Declared source license/MSRV review | PASS |
| Current advisory audit | PASS |
| Recorded framework/storage decisions | PASS |

## Raw observations, including historical rejected candidates

| Check | Status | Exit | Evidence / command |
| --- | --- | --- | --- |
| Host | PASS | None | PowerShell presence inspection; see tools/doctor.ps1 |
| Rust compiler | PASS | 0 | rustc +stable -Vv |
| Cargo | PASS | 0 | cargo +stable --version |
| Rustup target | PASS | 0 | rustup target list --installed --toolchain stable |
| Rustfmt | PASS | 0 | cargo +stable fmt --version |
| Clippy | PASS | 0 | cargo +stable clippy --version |
| Git | PASS | 0 | git --version |
| MSVC installation | PASS | None | PowerShell presence inspection; see tools/doctor.ps1 |
| Windows SDK | PASS | None | PowerShell presence inspection; see tools/doctor.ps1 |
| MSVC link smoke test | PASS | 0 | reports/raw/journal-unit.txt |
| Windows GUI/IME/DPI/Narrator | PASS | None | reports/p00-final-native.md |
| Dependency compatibility and licenses | PASS | None | PowerShell presence inspection; see tools/doctor.ps1 |
| mongo-runtime | PASS | 0 | reports/raw/mongo-runtime.txt |
| format | PASS | 0 | reports/raw/format.txt |
| journal-unit | PASS | 0 | reports/raw/journal-unit.txt |
| journal-process-crash | PASS | 0 | reports/raw/journal-process-crash.txt |
| journal-crash-matrix | PASS | 0 | reports/raw/journal-crash-matrix.txt |
| mongo-build | PASS | 0 | reports/raw/mongo-build.txt |
| clippy | PASS | 0 | reports/raw/clippy.txt |
| native-format | PASS | 0 | reports/raw/native-format.txt |
| native-unit | PASS | 0 | reports/raw/native-unit.txt |
| native-clippy | PASS | 0 | reports/raw/native-clippy.txt |
| index.crates.io | BLOCKED | None | Resolve-DnsName index.crates.io |
| coolify.be-learning.io.vn | BLOCKED | None | Resolve-DnsName coolify.be-learning.io.vn |
| windows-target | PASS | 0 | reports/raw/install-windows-target.txt |
| rustfmt | PASS | 0 | reports/raw/install-rustfmt.txt |
| candidate-crates | PASS | 0 | reports/raw/install-candidate-crates.txt |
| software-build-debug | PASS | 0 | reports/raw/software-build-debug.txt |
| software-unit | PASS | 0 | reports/raw/software-unit.txt |
| software-clippy | PASS | 0 | reports/raw/software-clippy.txt |
| software-metadata | PASS | 0 | reports/raw/software-metadata.txt |
| mongo-diagnostics-unit | PASS | 0 | reports/raw/mongo-diagnostics-unit.txt |
| mongo-diagnostics-build | PASS | 0 | reports/raw/mongo-diagnostics-build.txt |
| mongo-diagnostics-clippy | PASS | 0 | reports/raw/mongo-diagnostics-clippy.txt |
| egui-format | PASS | 0 | reports/raw/egui-format.txt |
| egui-unit | PASS | 0 | reports/raw/egui-unit.txt |
| egui-build | PASS | 0 | reports/raw/egui-build.txt |
| egui-release-build | PASS | 0 | reports/raw/egui-release-build.txt |
| egui-clippy | PASS | 0 | reports/raw/egui-clippy.txt |
| egui-metadata | PASS | 0 | reports/raw/egui-metadata.txt |
| egui-windows-control-semantics | PASS | None | reports/egui-comparison.md |
| egui-unicode-font-fallback | PASS | None | reports/egui-comparison.md |
| egui-search-and-selected-tab-focus | PASS | None | reports/egui-comparison.md |
| egui-virtual-list | PASS | None | reports/egui-comparison.md |
| egui-first-build-undo-redo | PASS | None | reports/egui-comparison.md |
| egui-index-click | FAIL | None | reports/egui-comparison.md |
| egui-splitter-drag | FAIL | None | reports/egui-comparison.md |
| egui-ime-narrator-osdpi | BLOCKED | None | reports/egui-comparison.md |
| evidence-store-regression-ps51 | PASS | 0 | reports/raw/evidence-store-regression-ps51.txt |
| foundation-python-regression | PASS | 0 | reports/raw/foundation-python-regression.txt |
| rustsec-p00 | PASS | 0 | reports/raw/audit-p00.json |
| rustsec-windows-p00 | PASS | 0 | reports/raw/audit-windows-p00.json |
| rustsec-p00-native | FAIL | 1 | reports/raw/audit-p00-native.json |
| rustsec-windows-p00-native | FAIL | 1 | reports/raw/audit-windows-p00-native.json |
| rustsec-p00-egui | FAIL | 1 | reports/raw/audit-p00-egui.json |
| rustsec-windows-p00-egui | PASS | 0 | reports/raw/audit-windows-p00-egui.json |
| rustsec-p00-transport | PASS | 0 | reports/raw/audit-p00-transport.json |
| rustsec-windows-p00-transport | PASS | 0 | reports/raw/audit-windows-p00-transport.json |
| dependency-source-license-msrv | PASS | 0 | reports/dependency-source-review.json |
| p00-editor-highlight-search | PASS | None | reports/p00-final-native.md |
| p00-keyboard-modal | PASS | None | reports/p00-final-native.md |
| p00-telex-input | PASS | None | reports/p00-final-native.md |
| p00-native-resize-dpi | PASS | None | reports/p00-final-native.md |
| p00-narrator | PASS | None | reports/p00-final-native.md |
| p00-ui-local-restore | PASS | None | reports/p00-final-native.md |
| p00-ui-large-response | PASS | None | reports/p00-final-native.md |
| p00-ui-decision | PASS | None | reports/ADR-0001-ui.md |
| p00-storage-decision | PASS | None | reports/ADR-0002-local-store.md |
| host-hardware-and-workspace | PASS | 0 | reports/host-check.json |
| software-renderer-feature-graph-debug | PASS | 0 | reports/software-renderer-evidence-debug.json |
| native-default-check | PASS | 0 | reports/raw/native-default-check.txt |
| software-native-render-and-input | PASS | None | reports/software-renderer.md |
| software-accessibility | FAIL | None | reports/software-renderer.md |
| native-render | PASS | None | reports/native-ui.md |
| native-unicode-undo-search | PASS | None | reports/native-ui.md |
| native-virtual-list | PASS | None | reports/native-ui.md |
| native-ui-transfer | PASS | None | reports/native-ui-http-artifact.json |
| native-ui-local-save | PASS | 0 | reports/raw/native-ui-save-reopen.txt |
| native-app-scale-stress | PASS | None | reports/native-ui.md |
| native-accessibility | FAIL | None | reports/native-ui.md |
| native-ime-osdpi-narrator | BLOCKED | None | reports/native-ui.md |
| native-ui-http-artifact | PASS | 0 | reports/native-ui-http-artifact.json |

Mongo isolated commit/rollback probe passed. Product publish/idempotency/revoke-race tests remain phase-specific; this successful fixture is not a P00 blocker.

## Acceptance blockers


## Later phase evidence still required

- Full product keyboard/accessibility/DPI matrices, multiple monitors, automatic device-loss recovery and release performance budgets are phase-specific; selected P00 smoke observations do not certify them.
- Production store schema migrations, workspace scoping and physical disk/power-loss qualification remain separate from the observed process-crash and OS-flush contract.
- Some upstream published crates omit notice files; the release notice bundle must be completed from exact upstream revisions before distribution. See dependency-source-review.json.
- 464 design renders missing; no full visual parity claim.
- No git repository detected; no commit evidence.

## Limits of the evidence

Journal unit and process-crash tests do not establish power-loss safety or a production-ready store. Driver compilation does not establish Mongo connectivity or transaction correctness. Tool/SDK presence does not establish native GUI accessibility, IME, DPI or performance.

No Docker/Compose commands were executed. No existing application server, user database, production service or replica-set configuration was changed. Controlled loopback HTTP fixtures are isolated test servers. Mongo credentials were process-local and are absent from source and reports.

See dependency-matrix.md, ADR-0001-ui.md, ADR-0002-local-store.md, transaction-smoke.md and checkpoint.md.
