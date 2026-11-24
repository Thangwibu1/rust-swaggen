# Swaggen desktop foundation (P01, in progress)

Native Rust/eframe Windows application. This is the first local vertical slice, not the complete API client, collaboration product or installer.

From the project root (installed stable must be Rust 1.100.0):

```powershell
python -X utf8 tools/p01-dependency-check.py
python -X utf8 tools/p01-check.py
& "$env:USERPROFILE/.cargo/bin/cargo.exe" +stable run -p swaggen-desktop --offline --locked --target x86_64-pc-windows-msvc
```

Normal data is under `%LOCALAPPDATA%/Swaggen/personal`. An explicit absolute `--data-dir` and validated `--workspace` are development options. For disposable UI testing:

```powershell
& .\target\x86_64-pc-windows-msvc\debug\swaggen-desktop.exe --data-dir D:\code\potmen\.local\p01-ui --workspace smoke
```

The application starts without an account, database service or server connection. Write a draft, save with Ctrl+S and restart to recover its acknowledged revision. Only a durable journal commit permits the saved indicator. Editing during a save keeps later edits dirty. Closing with unsaved text opens a warning. Imported markup is displayed as plain text and is never evaluated.

Personal secrets accepts a validated name and a masked new value. Save protects it through current-user Windows DPAPI with workspace/name binding, then commits only ciphertext. Reveal is explicit; Hide or navigation removes the revealed buffer. There is no clipboard copy action. Protect/retrieve and all filesystem work run in one bounded worker actor, outside the UI thread. Secret-save failures return the unsaved input to the UI. DPAPI is not protection against processes already running as the same Windows user or administrator. UI/framework memory is not claimed to be completely zeroized.

Drafts are plaintext local content, not a secret vault. Never paste real credentials into draft text. Error/status events carry fixed error codes/messages, not payloads or system error strings. Draft Debug and Secret Debug redact content. No telemetry, automatic crash upload, remote content execution, extension host or network request sender exists in this slice. Team vaults, sharing, sanitized export/reports and full capability policy remain unimplemented.

The original P00 spikes and lockfiles are retained and excluded from this Cargo workspace. Local-store journal primitives were promoted from the reviewed P00 source; production adapters add version/workspace validation, bounded snapshots and compaction. Unsupported versions/foreign workspace identities fail closed without overwriting complete records. Existing reparse points are rejected; malicious same-user filesystem races and full ACL qualification remain security-hardening work.

Full screen rendering/parity is not asserted: E03/S06 metadata is available, their full reference PNGs are missing. P01 remains in progress until its remaining UI/security/recovery, server/CI and phase acceptance work is verified. No Git commit, Linux CI, release build, installer, signing, performance budget or deployment is claimed.
