# MongoDB transaction smoke - isolated fixture passed

Current evidence: the owner's 2026-11-24 invocation of `tools/p00-mongo.ps1` completed with exit 0. `reports/p00-checks.json` and `reports/raw/mongo-runtime.txt` record a writable replica-set primary, server 7.0.48 / FCV 7.0, multi-document commit and uncommitted invisibility, explicit rollback, duplicate-key-error rollback and isolated fixture cleanup, all PASS. Driver compilation also passed on x86_64-pc-windows-msvc. This is fixture acceptance, not production deployment or product transaction acceptance.

Historical attempts below remain failures. The older `reports/mongo-check.json` is a prior exit-2 attempt; the report generator selects the latest timestamped observation rather than treating that older record as current. Earlier DNS restrictions do not negate the later successful invocation or establish general network availability for the agent shell.

Earlier diagnostic continuation: the probe records an allowlisted failure stage/category instead of a combined generic status. That invocation exited 2, with status BLOCKED, stage topology and category CONNECTIVITY_OR_SERVER_SELECTION. The category does not guess whether the underlying selection problem is DNS, socket permission, routing, server availability or another cause. Three scoped diagnostic tests cover socket error 10013, suppression of a private error-message marker, and malformed-URI classification without connecting to a server. They are not database integration tests.

Fixture setup now runs within the cleanup scope, so setup errors also reach cleanup of the same uniquely named disposable database. Existing database/user/replica-set configuration is never reset. Unknown commit labels retain an unresolved category rather than a definite failed-commit claim. Live authentication/authorization/unknown-commit diagnostic branches remain unverified against a server.

`spikes/p00/src/bin/mongo-smoke.rs` reads `SWAGGEN_MONGO_URI` from the process environment. The supplied credential was used only in a temporary process environment for this attempt; it is not stored in source, reports or command examples. Output suppresses driver error bodies and prints only allowlisted evidence. No Docker is used.

The probe first requires an authenticated writable replica-set primary with logical sessions and reads server build/FCV. Only after that does it create a unique `swaggen_p00_<ObjectId>` fixture database. It does not configure replica sets, step down primaries, run failpoints, bootstrap accounts or reset user databases. It checks two-collection uncommitted invisibility and commit, explicit rollback, and actual duplicate-key-error rollback. Session operations are sequential. Cleanup drops only the fixture database created by this invocation. If setup fails after creating a collection, a disposable fixture may remain for scoped cleanup; the current DNS failure occurred before any successful DB result was observed.

The code includes a bounded unknown-commit retry loop; that branch has not been fault-injected. Product idempotency/ACK recovery, transient retries, conditional revisions, permission revoke fencing, cross-workspace isolation, process death around commit, primary failure and attachment invariants still require implementation and integration evidence in their appropriate phases. They are not covered by merely compiling this probe.

For a future authorized scoped rerun, use the private-input helper (a successful test does not need rerunning just to refresh reports):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/p00-mongo.ps1
```

See the current `mongo-runtime` record in `reports/p00-checks.json` and `reports/raw/mongo-runtime.txt`. Direct connection from this desktop does not validate server-side seed hostname discovery, a least-privilege backend role or production network topology.
