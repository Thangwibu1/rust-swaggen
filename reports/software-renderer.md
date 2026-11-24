# P00 software renderer probe

The existing native package gained an isolated `software` feature. Its default feature forwards `iced/default`, preserving the previous candidate's default feature set. Iced remains pinned at 0.14.0. No UI framework was selected, no product scaffold was created, and no Docker is used.

`tools/p00-renderer-check.ps1` builds the Windows MSVC software variant into a separate `target-software` directory, then runs its three history/virtualization unit tests and Clippy. Build/test/lint returned exit 0. The probe did not replace the original native binary. Its lockfile was used with --offline --locked; no new dependency version was invented or installed.

The selected renderer is iced_tiny_skia 0.14.1, via iced_renderer 0.14.0. Source and feature checks show tiny-skia enabled and wgpu-bare disabled. The Cargo tree for normal/build dependencies contains neither iced_wgpu nor wgpu, and excludes the MongoDB driver from the native probe. The first metadata-only checker encountered optional weak-feature nodes retained in Cargo metadata; it was corrected to validate the selected Cargo build tree as well as renderer feature flags. No failed checker result was represented as success. See `software-renderer-evidence-debug.json` for the observed debug binary hash, features and limits; release-mode builds require separate runtime evidence.

## Observed interactive run

Launched the compiled executable by its exact software-target path, then selected the returned window by executable path/title. The first screenshot was occluded; raising the selected native window produced the actual rendered shell. Only that correctly displayed native window was used for subsequent input. The shell, 1,000-row fixture list, highlighted multiline editor and white controls visibly rendered. Pasting a Vietnamese fixture JSON object displayed the diacritics, and Ctrl+Z restored the initial JSON.

This validates the isolated software drawing path and those selected input operations. It does not certify automatic GPU-device-loss recovery, a disabled physical GPU, OS compositor behavior, real Vietnamese IME composition, OS DPI, Narrator or release performance. The accessibility tree still exposed only native window chrome and no internal controls; changing renderer did not resolve the accessibility gate.

Screenshots were observed through the computer-use tool; no local screenshot files, source-design pixel parity or benchmarks are claimed. The probe window remains available; no unrelated user window was changed. The previous 20MiB interactive transfer evidence remains separate and was not rerun as a software-renderer acceptance in this continuation.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/p00-renderer-check.ps1
python -X utf8 tools/p00-software-evidence.py
```

P00 remains blocked by UI accessibility/framework selection, real Mongo transaction acceptance and remaining quality/storage gates.
