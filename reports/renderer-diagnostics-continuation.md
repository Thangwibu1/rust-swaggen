# P00 renderer and diagnostic continuation

Initial reconciliation found no changes among the 16 source fingerprints from the previous review. The existing source, 160 requirement mappings, 316 screen entries, prior raw test results and checkpoint were retained. This directory still has no Git metadata; no git diff, commit, reset or fresh product scaffold is claimed.

## New work and evidence

- Added an opt-in software drawing configuration to the existing native probe while preserving the old default feature set through `iced/default`. Separate Windows build output avoids replacing the prior executable.
- Software build, its three existing native model tests and Clippy completed successfully. A default-configuration cargo check also completed successfully. No application performance comparison follows from these checks.
- Validated the selected Cargo build tree and renderer feature flags: tiny-skia is enabled, wgpu-bare is disabled, and the selected tree excludes iced_wgpu/wgpu and the MongoDB driver. Optional metadata nodes alone were not treated as selected build dependencies.
- Launched the actual software-only binary by exact path. Raised the selected native window after an occluded screenshot, then observed shell/editor rendering, Vietnamese fixture paste and Ctrl+Z. Its internal controls still did not appear in the accessibility tree.
- Added feature-gated Mongo diagnostic classification using the pinned driver's public error variants and labels. Three scoped tests passed: socket-permission classification, private-message suppression and malformed-URI classification without connecting to a server. The probe builds and Clippy passes.
- The configured real Mongo invocation again exited 2. Its safe result is BLOCKED at topology, category CONNECTIVITY_OR_SERVER_SELECTION. No topology/FCV/auth/transaction success was observed. The original error body and credential were not logged. Fixture setup was moved inside the cleanup scope for the same disposable test database.

The 11 journal unit cases and six compaction process-crash cases were not rerun for this unrelated renderer/diagnostic slice; their prior evidence and unchanged journal source remain intact. There are now 17 unique unit cases with scoped passing evidence (11 journal + 3 native model + 3 diagnostic), not 17 newly executed cases or a full product suite.

## Gate and next actions

P00 is still blocked. Explicit software drawing support does not establish native accessibility, real IME, Narrator, OS DPI/multi-monitor, automatic device-loss recovery, release performance, storage production adoption or Mongo transaction readiness.

The owner-facing prerequisites were provided during the run. The current reports still contain no new successful eframe comparison fetch or Mongo transaction acceptance. In a terminal with registry/database permissions:

```powershell
cd D:\code\potmen
powershell -NoProfile -ExecutionPolicy Bypass -File .\tools\install-p00-deps.ps1 -CompareUI
powershell -NoProfile -ExecutionPolicy Bypass -File .\tools\p00-mongo.ps1
```

The Mongo helper prompts for a hidden URI if no private environment value is set. No confirmation to reselect the stack or install routine dependencies is needed; the remaining issue is executable access to the required network resources. Do not automate a terminal through the desktop UI or bypass execution restrictions. Continue from the actual command results when available; P01–P12 remain not_started.
