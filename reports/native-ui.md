# P00 native UI observations — partial acceptance only

Date: 2026-11-24. Candidate: Iced 0.14.0 (iced_widget 0.14.2 in Cargo.lock), Windows MSVC debug build. Window title: `Swaggen P00 — Native Framework Gate`. These are observations of a real native app, not Figma metadata or a mock screenshot. Screenshot observations were displayed by the computer-use tool; no local screenshot files or visual parity results are claimed.

## First interactive run

Opened the compiled native executable through the native app launcher. A prior shell Start-Process attempt did not yield a lasting target window; process launch metadata alone was not counted as UI success. Selected the actual returned native window by title and executable.

| Check | Observed result | Acceptance limit |
| --- | --- | --- |
| Native shell | Real 1440×900 client area rendered with editor/sidebar/controls | Debug build; no startup benchmark or source-design parity |
| Multiline Vietnamese Unicode | Pasted a fixture JSON object containing `Xin chào, tiếng Việt có dấu — kiểm thử P00`; diacritics visibly rendered | Clipboard insertion, not a Vietnamese IME composition test |
| Undo / redo | Ctrl+Z restored the original JSON; Ctrl+Y restored the pasted fixture | Bounded snapshot history; cursor/folding/large-editor semantics not accepted |
| Exact search | Searching `Việt` reported one match in the pasted fixture | Match counting only; search navigation/highlight still absent |
| Virtualized list | Scrolled to `/fixture/1000`; 21 rows reported rendered at the bottom, 24 near the top | No p95 interaction latency measurement |
| Dialog shortcut | Ctrl+P replaced background controls with the isolated dialog; Escape returned to editor | Not a complete Tab order/Narrator/focus restoration acceptance |
| Accessibility tree | Only window/title bar/system controls were exposed; internal editor/list/buttons were absent | Accessibility gate NOT PASSED. No claim that Narrator works; a bridge or native comparison is needed |

## Additional validation

The first shell HTTP fixture run failed with Windows socket permission error 10013. `reports/native-first-checks.json` and `reports/raw/native-http.txt` preserve that failure; it was not converted to a passing shell test.

After confining output paths to the project and using white control surfaces, a second interactive run successfully transferred 20,971,520 bytes from the actual local Axum fixture. The UI showed a 65,536-byte preview and raw-file completion. Ctrl+P opened the isolated dialog after transfer initiation; Escape returned to the completed response. A single debug-run transfer duration shown in the UI was 558ms; this is not a release benchmark, p95, startup or interaction-latency acceptance. `tools/p00-native-evidence.py` independently checked every byte in the generated file and captured its SHA-256 in `reports/native-ui-http-artifact.json`.

Ctrl+S reported the fixture draft saved locally. The separate `journal-ui-reopen` executable reopened the actual file and verified a 146-byte UTF-8 draft with no publish intent. See `reports/raw/native-ui-save-reopen.txt`. This proves the specific save/readback fixture, not product restart restoration.

Application-scale stress at 200% initially exposed toolbar clipping and a collapsed editor. The layout was corrected with compact controls, a wrapping toolbar, a smaller sidebar and a shorter preview; a third interactive run visibly retained the editor and controls at 200%. A later 125% application-scale state was also observed after user input. These checks do not certify OS DPI handling, multi-monitor behavior or resizing. The last probe window was left available for the user after input was detected; no user app or unrelated window was closed.

Still pending: real Vietnamese IME preedit/commit/cancel, full Tab order, Narrator, OS DPI 100/125/150/200%, multiple monitors, resizing/split panes, explicit software renderer fallback and measured release performance. Iced is not approved solely because it compiled and rendered. Comparing egui/eframe is permitted by the framework gate; its source is currently not cached in the execution environment.
