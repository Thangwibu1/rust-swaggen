# P00 egui native comparison — partial evidence

Current continuation: the completed foundation observations and passed foundation gate are in `p00-final-native.md`, `p00-gate.json`, and the current ADRs. The selected prototype now includes real lexical highlighting/search navigation, journal save/restore, bounded 20 MiB HTTP transfer, fixed pointer-event splitter handling, actual Telex key input, native 175% resize smoke and actual Narrator Speech recap. The adopted Windows audit passed; original Linux-only warnings remain FAIL, as explained in p00-security-disposition.md. Earlier observations below remain historical evidence and must not be read as the latest feature list. Geometry-based indexed automation click remains unsupported; native pointer/keyboard/UIA value/Narrator paths were observed separately.

Date: 2026-11-24. This is an isolated Windows fixture, not product code or a framework selection. Package: `spikes/p00-egui`; pinned eframe/egui 0.37.0, Rust 1.100.0, Windows MSVC debug build. `reports/egui-checks.json` records formatting, build, Clippy and selected-target metadata. No browser, HTML or WebView is used.

## Exact source review

The owner's `-CompareUI` fetch supplied the official registry sources. Inspected eframe 0.37.0 `Cargo.toml`, `src/lib.rs`, `src/epi.rs`, and egui 0.37.0 panel, modal, text-edit, context and scroll-area source. Eframe declares MIT OR Apache-2.0 and MSRV 1.95; the installed 1.100.0 compiler satisfies that minimum. APIs in this version include `App::ui`, unified `Panel`, `Id::unique`, `TextEdit::labelled_by` through Response, `ScrollArea::show_rows` and `Context::enable_accesskit`. The first compile caught obsolete panel names and a deprecated ID constructor; corrected before the passing build. Its failure is preserved in `reports/raw/egui-first-build.txt`.

Selected eframe features: accesskit, clipboard, default_fonts, system_fonts and wgpu, with default features disabled. Inspection/network server and persistence features are not enabled. The system font fallback uses fonts installed on Windows; no Microsoft font file was copied into the project or licensed for redistribution. Complete transitive license obligations, current RustSec audit, maintenance and release support remain unapproved.

Official versioned reference: https://docs.rs/eframe/0.37.0/eframe/ . Local cached source is the authority for the exact APIs used; download and compilation do not establish an advisory clearance.

## Real interactive observations

Launched the compiled executable and selected the returned window titled `Swaggen P00 - egui Accessibility Comparison`. Read the actual Windows accessibility tree and screenshots through the native app tool. Observations are tool-returned states, not generated screenshots. No screenshot file or pixel-parity result is claimed.

| Check | Observed result | Limit |
| --- | --- | --- |
| Native rendering | Header, request list, editor and search rendered in a real window | Debug only; no release/latency/visual parity acceptance |
| Internal semantics | Buttons, labelled search/body text fields, virtual request buttons, splitter separator and dialog exposed | Does not establish full Narrator or every UIA pattern |
| Initial font defect | Without system_fonts, Vietnamese glyphs were missing and extracted document text repeated parts of accented characters | Historical failure preserved here; do not describe that build as Unicode-compatible |
| Font correction | Enabled system_fonts, rebuilt and relaunched. `Tiếng Việt` and the pasted accented fixture rendered; body Value and Document text matched the supplied text | Selected installed-font fixture only; no IME composition evidence |
| Unicode fixture | Pasted `Xin chào, tiếng Việt có dấu — kiểm thử P00`; final Value and Document text preserved accents | Clipboard, not Telex/VNI/Windows IME preedit/commit/cancel |
| Undo/redo | Ctrl+Z restored initial JSON; Ctrl+Y restored the pasted fixture on first build | Same editor API; not a large-document history benchmark |
| Search | Set labelled search field to `Việt` through UIA; `1 matches` appeared | Counting only; no match navigation or syntax highlighting implemented |
| Keyboard focus | Tab from search focused the labelled body editor on corrected build | Complete traversal of offscreen virtual rows is not accepted |
| Virtual list | Tree reached GET /fixture/1000 and reported 45 rows drawn (44 near top) | No timing percentile or offscreen keyboard navigation acceptance |
| Dialog | Ctrl+P exposed a dialog; Tab focused Return to editor; Escape removed dialog and restored editor focus, retaining draft | Initial dialog focus is the root until Tab; background nodes remain in tree. Full modal Narrator isolation is unverified |
| Geometry/input limits | First index-based editor click returned `coordinate input geometry is unavailable`; screenshot-coordinate click succeeded. A splitter drag produced no measurable change | Bounding rectangle/Invoke patterns and splitter resizing remain unresolved; do not report a successful drag |

The native comparison was left open for review with fixture-only content. No user app was closed and no account, DB, backend, system setting or permission was modified.

## Gate disposition

Egui has demonstrated internal Windows semantics that were absent from the Iced probe, and fixes the selected Vietnamese font/extracted-text defect with system font fallback. It remains a candidate. Before selection: verify real IME, Narrator and UIA text ranges/geometry, complete keyboard traversal and modal isolation, OS DPI/multi-monitor, resize/split behavior, highlighted editor/navigation, large-response/local-store integration in this candidate, release benchmarks, renderer recovery and full dependency audit. P00 remains blocked; P01–P12 have not started.
