# P00 security disposition - 2026-11-24

The adopted native Windows dependency graphs pass cargo-audit 0.23.0 with `--deny warnings`, no ignored advisory IDs and no stale-database override. This is a Windows foundation decision, not a Linux support or distribution approval.

## Scope and reproducible evidence

| Scope | Complete Windows closure | Original cross-platform lock | Disposition |
| --- | --- | --- | --- |
| p00, including Mongo feature | PASS, 185 packages | PASS | Adopted storage/server prototype |
| p00-egui | PASS, 203 packages | FAIL | Adopted Windows UI; three Linux-only maintenance warnings retained |
| p00-transport | PASS, 50 packages | PASS | Adopted loopback HTTP foundation |
| p00-native (Iced) | FAIL, 255 packages | FAIL | Rejected comparison; must not be shipped or adopted |

Package counts include local packages and overlap between scopes; they are not additive. Original Cargo.lock files and application dependencies were not changed to obtain these results.

Cargo metadata is executed afresh with `--offline --locked --filter-platform x86_64-pc-windows-msvc` and default features (plus `mongo` for p00). The Windows host and target are both MSVC Windows. Every reachable package, including build dependencies and procedural macros, is projected with its exact source, version and checksum into a separate audit-only lock. Every dependency edge must exist in the original lock. The projection is never used for building. Hashes bind the original lock, local manifests, resolved metadata, projection and clean audit JSON; report generation rejects mismatched evidence. Tests cover reachable transitive/build dependencies, invalid edges, disconnected nodes, changed manifests/projections and missing evidence.

The fresh advisory database was downloaded by the owner's successful security command. Local reruns used `--no-fetch`, with an additional maximum 48-hour latest-commit age check; no `--stale` option. Database commit: `52384ab5581a4487b62c549eef95cf7ba7e8bad5`, committed 2026-11-23T17:01:22+01:00. Default helper runs still fetch updates. Per-run timestamps, commands, hashes and database metadata are in `security-checks.json`; raw original and Windows audits are retained under `raw/audit-*.json`.

## Findings and decision

Egui's original lock contains `zbus_names 4.3.5`, `zvariant 5.15.1` and `zvariant_derive 5.15.1`, flagged unmaintained by RUSTSEC-2026-0371, RUSTSEC-2026-0370 and RUSTSEC-2026-0372. None is in Cargo's Windows-resolved closure. They are not silently ignored: the full-lock FAIL remains visible. Linux adoption would require resolving them and a new platform gate.

Iced's Windows graph really includes unmaintained bincode 1.3.3, rustybuzz 0.20.1, ttf-parser 0.25.1 and yaml-rust 0.4.5. It also includes unsound lru 0.16.4 (RUSTSEC-2026-0253; panic-safety use-after-free concern; advisory patch range >=0.18.2). Actual dependency chain: swaggen-p00-native -> iced 0.14.0 -> iced_renderer 0.14.0 -> iced_wgpu 0.14.0 -> cryoglyph 0.1.0 -> lru 0.16.4. These are genuine rejection evidence, not waived findings. The candidate was already rejected for inaccessible internal controls; the security result independently reinforces rejection. The source and failed audits remain for traceability. Do not distribute its executable or reuse its renderer dependency graph.

The earlier gate inadvertently required a clean rejected candidate as well as the adopted implementation. It now requires clean complete Windows graphs for the three adopted scopes; it still fails on any warning in those graphs. This disposition is recorded in ADR-0001 before acceptance. Full-lock scans continue to run so future changes cannot hide cross-platform maintenance findings. No warning policy was weakened and no dependency was patched unsafely just to silence an advisory.

Recheck from a network-enabled terminal: `powershell -NoProfile -ExecutionPolicy Bypass -File tools/p00-security-check.ps1`. A local reproduction with the same fresh DB is `python -X utf8 tools/p00-audit.py --no-fetch`, followed by `python -X utf8 tools/p00-gate.py`. Any dependency/feature/target change requires a new source review, audit, applicable build/native checks and ADR review before adoption.
