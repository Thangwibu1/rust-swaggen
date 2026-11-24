# P00 native foundation validation — 2026-11-24

Selected foundation: eframe/egui 0.37.0 on Windows MSVC, Rust 1.100.0. This report records actual interactive observations; it is not full product, all-DPI or performance acceptance.

## Corrected implementation and observed results

| Case | Evidence observed in real Windows app | Scope |
| --- | --- | --- |
| JSON/YAML editor | Loaded both fixture formats; coloured string, number and punctuation spans rendered. Unicode source remained intact | Lexical display, not syntax validity/OpenAPI validation |
| Search navigation | A two-match Vietnamese fixture reported 2 matches; F3 selected the literal `Việt` and focused the editor | Character-index conversion is unit-tested, including non-ASCII prefix/emoji |
| Undo/redo | Final release Ctrl+Z emptied the committed Telex phrase; Ctrl+Y restored `tiếng việt` exactly | Large-document history limits still need product qualification |
| Modal isolation | Ctrl+P removed background nodes from the Windows accessibility tree, focused Return to editor immediately; repeated Tab stayed there; Escape returned editor focus and kept content | Real controls and keyboard input, not a generated screenshot |
| Local save/restore | Saved accented fixture through Ctrl+S; changed editor to `temporary unsaved fixture`; Restore returned the original accented JSON from the real journal | Background worker, empty outbox, no simulated publish |
| Large response | New egui integration transferred 20,971,520 actual HTTP fixture bytes, preview 65,536 bytes; raw file existed. Dialog opened after transfer initiation and app remained usable | Debug transfer displayed 494 ms in that run; not a percentile or release benchmark |
| Splitter | Initial natural-content sizing shrank the pane. Keeping allocated width fixed that collapse; ordered pointer processing fixed drags batched within one frame. Release pointer drag changed the pane from 250 to 419 points and preserved the new width | Two regression tests cover coalesced press/move/release and capture loss |
| OS window resize / DPI smoke | Release window resized through actual Windows system menu from 1440×900 to 951×900, then 720×480, and maximized to 1646×903. All observed states reported native 1.75 pixels/point. Controls/editor stayed available; text wrapped and scrollbars appeared as needed | Native Windows current-scale prototype test, not claims for OS DPI 100/125/150/200% or multiple monitors |
| Actual Vietnamese keyboard input | With editor focused, individual key events `t i e e n g s space v i e e t j space` produced `tiếng việt`. This was not Unicode paste or synthetic egui IME events | Current installed Vietnamese Telex input path only. Partial `tiee` displayed `tiê`; Escape retained the fragment and removed editor focus. No claim that this proves every OS IME cancellation model |
| Narrator | Launched Windows Narrator, focused Find text then Previous through the app's keyboard navigation. Opened Microsoft's Speech recap with Insert+Alt+X. The visible recap contained `Swaggen P00 - egui Accessibility Comparison region, Find text, edit,` and `Swaggen P00 - egui Accessibility Comparison region, Previous (Shift+F3), button,` | Actual Narrator transcript of selected controls; not inferred from process launch/UIA presence. Full screen-reader routes/text-range matrix remains product work |
| Narrator session cleanup | Insert+Escape ended the Narrator test session; subsequent window inventory contained neither Narrator nor Speech recap | No Narrator/privacy/system-display settings were changed |

Final executable cross-check: set Request number to 1000 through UIA, then Alt+Up selected 999 and exposed rows 960–1000 with 41 drawn rows. Native Telex and undo/redo were repeated on that executable. Ctrl+F followed by 44 Shift+Tab steps reached Restore fixture; Enter invoked it and restored the previously saved accented JSON. This also confirmed actual keyboard activation without relying on the helper's unsupported indexed click geometry. Final binary fingerprint is in `egui-final-binary.json`.

The Speech recap window has higher Windows integrity than the automation helper. Its text was read from the visible screenshot after the supported Raise action; no elevation or security setting change was attempted. It contains only selected speech evidence in this report, not a saved full speech history.

Microsoft's reference for speech recap and its shortcut: https://support.microsoft.com/en-us/accessibility/windows/narrator/complete-guide-to-narrator .

## Remaining limits

Index-based `click` still returned `coordinate input geometry is unavailable` for ordinary buttons as well as the editor. Screenshot-coordinate clicks, UIA SetValue, native keyboard input and actual Narrator reading worked. This unsupported automation geometry path is retained as a failed observation; it is not relabelled as a passing Invoke-pattern test. No unrestricted accessibility/performance claim follows.

Historical Iced accessibility failures are rejected-candidate evidence, not the selected egui's result. Full design parity, clean installer/signing, all-screen accessibility/DPI, multiple monitors, automatic GPU loss recovery, physical disk/power-loss certification and product synchronization are later phase gates, not claims established here.
