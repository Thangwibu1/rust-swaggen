# P00 experiments

This is a separate experiment package, not the Swaggen application scaffold. No native GUI framework or production storage decision has passed its gate.

From the project root on Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/p00-check.ps1
```

The process-scoped execution-policy argument applies only to the inspected local script; it does not change machine policy. The harness uses the installed `stable` alias only after verifying it is exactly Rust 1.100.0. Once registry access works, install the pinned named toolchain for ordinary Cargo commands. Lockfile and offline mode keep these experiments reproducible from the available cache. The build prerequisite check runs before the tests.

For the isolated MongoDB probe, set `SWAGGEN_MONGO_URI` privately in the terminal environment, then pass `-Mongo`. Never put the URI in a committed script or report. The probe uses a random disposable database and never drops an existing user database. It requires permission to read topology/FCV and create/drop its fixture database. Read the probe before running it against an external server.

Raw results and exit codes are stored in `reports/`. `PASS` for an individual console test does not mean P00 or a product requirement is complete. Read `reports/preflight.md` for the current gate.

## Compaction crash and write-failure checks

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/p00-check.ps1 -NativeLibrary -Durability
```

`-Durability` enables the separate `fault-injection` feature only for the crash probe and lints that feature. The native app's normal dependency does not enable probe callbacks. The parent creates unique project-local fixtures, starts a child writer, waits at a deterministic boundary (maximum 10 seconds per case), checks the cross-process writer fence, forcibly terminates the child, then reopens and appends again. Boundaries cover temporary-file creation, partial/full write, sync, replacement and returned ACK. The prepared orphan files are retained for inspection and are never interpreted as committed state.

The journal unit tests additionally inject StorageFull and WriteZero errors after each incomplete record prefix. They verify no ACK/sequence advance, poisoned writer, atomic recovery of draft+outbox and successful append after reopen. Those errors are deterministic simulations, not a physically full disk. Process termination is not device power loss or directory-entry durability. No production storage adoption is implied.
