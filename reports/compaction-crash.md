# P00 compaction interruption and partial-write evidence

This continuation kept the existing journal format, UI spike and requirement mappings. Git status/diff could not be obtained because this directory has no Git repository; no repository was initialized. Read `resume-git.json` for the observed inspection result. Code was extended in place, not replaced with a fresh scaffold.

## Real process-termination matrix

`journal-crash-matrix` creates unique fixture journals under the project's `.local/p00-crash-matrix` directory. Every fixture first acknowledges two complete draft+outbox batches. A child process then compacts the latest full state, signals a deterministic boundary and is forcibly terminated by the parent. The parent checks the writer fence while the child is alive, reopens after termination, compares complete data and sequences, appends another batch and reopens again.

| Termination boundary | Expected recovered history | Temporary orphan files |
| --- | --- | --- |
| Temporary file created | Both acknowledged batches | 1 |
| Partial replacement write | Both acknowledged batches | 1 |
| Complete write before sync | Both acknowledged batches | 1 |
| Synced replacement before rename | Both acknowledged batches | 1 |
| After atomic replacement | One complete snapshot of the latest state | 0 |
| After compact returns/ACK | One complete snapshot of the latest state | 0 |

The actual run passed all six boundaries. In each case the last acknowledged draft and outbox remained together, the writer fence released after process death, sequence progression allowed a new append, and prepared orphan files were never replayed as committed data. `reports/compaction-crash-results.csv` is copied from the run's actual artifact, not generated expected results. The raw command/exit/timestamp are in `p00-checks.json` and `raw/journal-crash-matrix.txt`.

## Partial-write failure injection

The append implementation now has an internal record-write seam while its public API still uses File::write_all followed by sync_all. A unit test injects StorageFull and WriteZero after every incomplete record prefix. It verifies the error is returned without advancing the acknowledged sequence, the writer is poisoned until reopen, the prior complete draft+outbox survives, and a subsequent append succeeds after recovery. No test assertion was removed. This tests journal error handling; it does not claim a physical disk was filled.

## Remaining gate

Forced process termination leaves the operating system and disk running. These tests do not establish power-loss behavior, parent-directory metadata flush guarantees, physical disk-full behavior, schema migrations, encryption or production workspace isolation. `fault-injection` is opt-in for the P00 experiment; it is not enabled by the native UI dependency. P00 remains blocked by native accessibility and MongoDB runtime acceptance, with other framework/storage/advisory checks still pending.
