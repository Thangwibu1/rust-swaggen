# Developer preflight

| Check | Status | Detail |
| --- | --- | --- |
| Host | PASS | Microsoft Windows 10.0.26200  |
| Rust compiler | PASS | rustc 1.100.0 (0ef1bc386 2026-11-09)<br>binary: rustc<br>commit-hash: 0ef1bc386059596dada8fb65b50425aab2ea637a<br>commit-date: 2026-11-09<br>host: x86_64-pc-windows-msvc<br>release: 1.100.0<br>LLVM version: 23.1.5 |
| Cargo | PASS | cargo 1.100.0 (3acc79c0c 2026-11-05) |
| Rustup target | PASS | x86_64-pc-windows-msvc |
| Rustfmt | PASS | rustfmt 1.11.0-stable (0ef1bc3860 2026-11-09) |
| Clippy | PASS | clippy 0.1.100 (0ef1bc3860 2026-11-09) |
| Git | PASS | git version 2.45.1.windows.1 |
| MSVC installation | PASS | D:\Development\MSVC\BuildTools |
| Windows SDK | PASS | C:\Program Files (x86)\Windows Kits\10\ |
| MSVC link smoke test | BLOCKED | Build and launch a pinned Rust native executable from a configured MSVC environment; presence checks alone are insufficient |
| Windows GUI/IME/DPI/Narrator | BLOCKED | Run P00 native UI spike and record measured evidence |
| Dependency compatibility and licenses | BLOCKED | Resolve official exact versions/MSRV/targets/features and advisories; fill dependency-matrix.md before scaffold |
