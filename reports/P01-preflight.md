# P01 preflight and scope — 2026-11-24

P00 was re-evaluated from current evidence: 13/13 criteria passed. Its recorded source fingerprints matched at resume. The directory has no Git repository, so git status/diff could not be obtained; no existing implementation was reset or replaced. Original P00 source/locks are retained and excluded from the new product workspace.

## Current slice

Windows offline shell -> typed bounded actor -> versioned workspace draft journal and current-user DPAPI secret store. No Mongo/network operation is required or performed by this slice. The already successful remote Mongo transaction test was not repeated. The no-Docker override remains in force.

## Dependency/API review

- Rust 1.100.0 and native MSVC target; exact source versions pinned by product Cargo.lock.
- Eframe/egui 0.37.0, same reviewed Windows features as the accepted P00 foundation. Panel API checked in the exact cached `egui-0.37.0/src/containers/panel.rs`; an initial use of removed old panel names failed compilation, preserved in raw/p01-unit-initial-api-failure.txt and corrected to Panel/show/default_size.
- All 169 external product Windows packages are exact-version subsets of the P00 source/license/MSRV review. This is separately checked; original P00 audit results are not substituted for a product audit. The complete product Windows graph is audited with cargo-audit 0.23.0, warnings denied, zero advisory ignores. The same freshly fetched advisory database is used with an extra <=48-hour commit-age guard.
- Serde 1.0.229 / serde_json 1.0.154: cached manifests, exact serde derivation and JSON APIs, existing permissive license/MSRV review. Local IDs deserialize through validation; snapshots reject unknown fields and unsupported versions.
- windows-sys 0.61.2 (MIT OR Apache-2.0; MSRV 1.71): new feature Win32_Security_Cryptography plus Foundation. Cached API signatures inspected for CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB and LocalFree. Current-user scope only; UI_FORBIDDEN, null prompt/reserved/description, no LOCAL_MACHINE. Workspace/name entropy binding, owned live pointers, LocalFree cleanup, best-effort volatile wipe of decrypted buffers. Actual DPAPI roundtrip/persistence/wrong-binding/truncated-blob tests run on Windows; no cross-account test is claimed.

Primary references reviewed:

- https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata
- https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptunprotectdata
- https://docs.rs/windows-sys/0.61.2/windows_sys/Win32/Security/Cryptography/index.html
- https://docs.rs/eframe/0.37.0/eframe/

Design sources inspected: E03 / Personal secrets (6:5509) and S06 / Privacy & permissions (7:4033), plus authoritative foreground-only white-surface decision. These are metadata, not rendered screenshots. The foundational shell is not declared a completed E03/S06 implementation.

Commands/results are captured in p01-checks.json and raw/p01-*.txt; native observations are recorded separately after actual UI execution. Passing code checks do not imply UI/IME/accessibility or full P01 acceptance.
