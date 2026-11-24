# P05 — Publish, Sync & Collaboration A (Local & Two-Client HTTP Verified; Live Mongo Env-Gated)

Status: **outbox durability, two-client HTTP E2E, replay/snapshot, 3-way conflict, comments, history & notifications verified** (2026-11-24). Live Mongo replica-set execution awaits `SWAGGEN_MONGO_URI`. Proceeding to P06.

## Implemented and verified

- **`crates/sync/src/lib.rs`**: Durable client sync state machine (`TeamSyncState`, `OutboxEntry`, `OutboxStatus`, `compute_request_hash` SHA-256, `enqueue_publish`, `apply_publish_response`, `apply_changes_batch`, `stage_and_swap_snapshot` rejecting mixed-cursor pages, `compute_three_way_diff`, `resolve_conflict` preserving losing edit in `drafts`, `apply_network_retry`, `apply_permission_blocked`).
- **`crates/local-store/src/sync_store.rs`**: Atomic crash-recoverable `sync.journal` storing `last_cursor`, `membership_revision`, shared base cache, local drafts, and outbox queue via framed journal.
- **`crates/api-contract/src/lib.rs` & `crates/mongo-store/` & `apps/server/src/lib.rs`**:
  - Multi-document publish transaction (`publish_batch` with permission fence revision check, `applied_operations` idempotency dedupe for lost ACKs, `RequestHash` mismatch rejection, conditional `entities` write preventing silent overwrite or tombstone resurrection, `revisions` history, `workspace_counters` cursor, `change_events` log, `notifications`, `audit_events`).
  - Ordered cursor replay (`/v1/workspaces/{id}/changes`), consistent snapshot pagination (`/v1/workspaces/{id}/snapshot`), authenticated broadcast SSE/long-poll signal (`/v1/workspaces/{id}/events`), revision history & forward-revision restore (`/revisions`, `/restore`), revision-anchored comments + reply + resolve (`/comments/*`), and inbox notifications (`/notifications/*`).
  - Updated `07-operations/server-openapi.json`.
- **`apps/desktop/` (`sync_worker.rs`, `views/sync.rs`, `worker.rs`, `app.rs`)**: Non-blocking sync worker + native Team Sync & Collaboration UI (`C03/C04/C05/C06/C07/C08/C10/C11/C27/C28/Y04/Y05`).

## Verification results

- `cargo +stable clippy --workspace --all-targets --offline --locked --target x86_64-pc-windows-msvc -- -D warnings`: **PASS** (0 warnings).
- `cargo +stable test --workspace --offline --locked --target x86_64-pc-windows-msvc`: **53 passed, 0 failed, 0 ignored**:
  - `swaggen-sync` 1/1 (`outbox_handles_idempotent_retry_conflict_tombstone_and_snapshot_swap`)
  - `swaggen-server` 2/2 (`two_client_publish_idempotency_conflict_tombstone_cursor_expiry_and_comments_e2e`, `full_auth_invitation_rbac_and_workspace_isolation_over_http`)
  - `swaggen-desktop` 8/8 (including `offline_publish_persists_pending_outbox_across_worker_restart`)
  - Plus `swaggen-domain` 9/9, `swaggen-http-engine` 2/2, `swaggen-local-store` 21/21, `swaggen-mongo-store` 2/2, `swaggen-secret-store` 3/3, `swaggen-spec-engine` 5/5.
- Live MongoDB replica-set execution remains env-gated (`BLOCKED` awaiting `SWAGGEN_MONGO_URI`).
