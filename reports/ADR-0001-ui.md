# ADR-0001: eframe/egui selected for the native Windows foundation

Status: selected for P01 after adopted Windows dependency audits passed; Iced rejected for accessibility and dependency safety. Date: 2026-11-24. This is the implementation agent's evidence-based choice under the project's routine technical-decision authorization, not a claim that the owner previously named this framework.

## Current decision

Use eframe/egui 0.37.0 for the initial Windows native foundation. Retain Axum 0.8.9 / Tokio 1.54.1 as the server foundation demonstrated by the isolated HTTP fixture. No HTML/WebView frontend or alternate database was introduced. Iced was evaluated first and is not selected because its tested internal controls were absent from the Windows accessibility tree. No claim is made that one renderer is faster than every alternative.

The selected comparison now has a highlighted multiline JSON/YAML editor, Unicode-safe search selection, undo/redo, keyboard/modal isolation, virtual 1,000-request list, real journal save/restore and real bounded HTTP response handling. Ordered native pointer-event processing fixed splitter drag batching. The release window was resized from 1440×900 to 720×480 and maximized at the actual Windows 175% scale. Individual Telex key events produced `tiếng việt`. Actual Narrator Speech recap confirmed the labelled search edit and Previous button. See `p00-final-native.md`, `egui-checks.json`, and the scoped UI records.

The baseline hardware inventory and source license/MSRV policy review are recorded separately. A release binary compiled and ran, but no full p95, startup, memory budget, all-OS-DPI, multi-monitor, complete screen-reader matrix or GPU-loss recovery claim is justified. Those checks repeat on product slices and release builds. Index-based automation click geometry remains unsupported in the helper while native pointer clicks, UIA values and real Narrator reading work; retain that negative observation.

Current product scaffold is still absent. The adopted Windows closures passed the current RustSec check with warnings denied and no advisory ignores. Full cross-platform egui and both Iced scans retain FAIL results. See `p00-security-disposition.md` for exact findings, package counts, projection integrity and the explicit rejection of Iced; its executable must not be distributed. P00 security acceptance covers the adopted Windows graphs, not the rejected comparison or Linux. P01 may implement the selected foundation after the generated gate passes without asking the owner to choose a framework name again.

## Historical evaluation before the current decision

The following earlier notes preserve the prototype sequence and limitations at those times. Current evidence above supersedes their pending-cache/selection descriptions.

Windows native Rust remains mandatory. Evaluate Iced 0.14.0 first, then egui/eframe if required. Axum remains the server candidate. No frontend web, WebView, React, SQL database or replacement UI framework was introduced.

Rust 1.100.0, MSVC and SDK are available. The owner successfully ran the dependency fetch; Iced 0.14.0 and Axum 0.8.9 are cached, locked and compile on Windows. The real native Iced window was launched and inspected. See `native-ui.md` for clipboard Unicode/undo/redo/search/list/dialog/transfer/save observations and explicit limits. Internal editor/list/button semantics were absent from the accessibility tree, so the accessibility gate has not passed. The first application-scale 200% layout problem was corrected and the compact layout was visually rechecked.

No UI framework is approved on the basis of compilation or the selected interactions alone. The separate native spike at 1440×900 now includes a virtualized 1,000-item list, highlighted multiline JSON/YAML editing, bounded snapshot undo, count-only search, selected keyboard commands, isolated dialog and bounded 20MiB response handling. Before adoption, complete real Vietnamese IME composition, Narrator semantics, tab order, OS DPI variants, resize/draggable split panes, multi-monitor and GPU fallback. Folding and full search navigation are not implemented. The current bounded editor fallback is a spike, not the production editor decision.

Comparison continuation: the owner fetched eframe 0.37.0 with `-CompareUI`. `spikes/p00-egui` now compiles on Windows with pinned features and passed Clippy. The real native window exposes labelled editors, buttons and a dialog, and selected search/Tab/focus/list/Unicode interactions were observed. System font fallback corrected missing Vietnamese glyphs and repeated accented extracted text. See `egui-comparison.md` for the remaining geometry/IME/Narrator/DPI/editor/release limitations. This is comparative evidence, not a selection or a performance ranking.

Measure release-build idle CPU, startup, frame/input latency and memory on an identified Windows machine, preserving raw samples. None of those measurements has been made. No framework performance ranking or Figma fidelity assertion is justified. Full visual comparison also depends on 464 missing source renders.

Software drawing continuation: a separate Windows MSVC debug build with `--no-default-features --features software` selects iced_tiny_skia, not the wgpu renderer. It compiled, passed its three scoped model tests and Clippy, and a real interactive run rendered the shell/editor and accepted Vietnamese fixture paste and Ctrl+Z. See `software-renderer.md` and the profile-specific binary/feature evidence. The accessibility tree still lacked internal controls. This confirms an explicit software drawing path; it does not prove automatic device-loss recovery, a physically disabled GPU or release performance. The default configuration still forwards `iced/default` and passed a separate compile check.

Consequence: no product scaffold or P01 gate advance until compatibility and native validation pass. Continue independently with P00 storage/transaction experiments and source research. User has already authorized installing prerequisites; the present obstacle is unavailable network access, not missing permission to choose ordinary implementation details.
