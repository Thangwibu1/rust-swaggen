# Kiến trúc triển khai đề xuất

## Tiến trình và ranh giới

| Thành phần | Chạy ở đâu | Trách nhiệm |
| --- | --- | --- |
| desktop-ui | Windows, main thread | Iced hoặc framework Rust đã qua prototype; widgets/state, không làm blocking IO |
| desktop-services | Windows, Tokio worker runtime | HTTP engine, local storage, secret store, spec parser, sync, history |
| collaboration-server | Coolify container Linux | Axum đề xuất; auth, membership, publish, event log, files, jobs |
| MongoDB replica set | Network riêng VPS | Dữ liệu nhóm, revisions, events, idempotency, transaction |
| attachment store | Volume hoặc object storage do nhóm quản lý | Blob theo hash, quota, backup; không phải BSON body không giới hạn |
| CLI/runner | Windows/Linux theo giai đoạn | Tái dùng engine/domain, nhận secret qua kênh riêng, giới hạn tài nguyên |

Desktop kết nối HTTPS backend cho cộng tác và HTTP(S) API đích riêng. Không để frontend mở connection MongoDB. UI communicates với service qua typed messages và bounded channels; event không chứa secret. Kiến trúc nhiều module trong một desktop process trước, không thêm microservice không có lý do.

## Cargo workspace mục tiêu

- `apps/desktop`: entry, UI state, navigation, focus, native lifecycle.
- `apps/server`: HTTP routes, auth, health, deployment binary.
- `apps/cli`, `apps/runner`: khi tới phase tương ứng.
- `crates/domain`: IDs, models, revision, permissions, policy; không phụ thuộc toolkit/DB.
- `crates/api-contract`: versioned request/response DTO, error codes, limits.
- `crates/http-engine`: request resolution, auth, transport, cookies, bounded streaming.
- `crates/local-store`: journal/snapshot hoặc ADR thay thế; transaction local logic.
- `crates/mongo-store`: official driver, repositories và unit-of-work transaction; server-only.
- `crates/sync`: outbox, state machine, reconnect, cursor, conflicts.
- `crates/spec-engine`: JSON/YAML parse, validation, refs, operation identity, diff.
- `crates/secret-store`: Windows credential protection/key references, redaction types.
- `crates/ui-kit`: components/tokens Rust, tập trung theme; không chứa HTTP handlers.
- `crates/test-engine`, `crates/mock-engine`, `crates/script-host`: theo phase, sandbox riêng.
- `xtask`: doctor, fixtures, package, benchmark, reports; không thực thi tool ngoài chưa kiểm chứng.

Không tách crate quá sớm khi module đơn giản đủ. Dependency graph hướng domain → services/adapters → entrypoints; domain không biết MongoDB hay widget.

## State và tác vụ

UI state không là database. Dùng ID + shared immutable snapshot nhỏ; không clone toàn bộ workspace/20MB response mỗi frame. IO qua workers; parsing CPU lớn vào bounded worker pool. Version/generation token loại kết quả search/parse stale. Cancel HTTP có lifecycle rõ; drain hoặc drop theo contract transport. Với MongoDB đọc guidance cancellation riêng, không bọc driver future bằng timeout thô nếu gây trạng thái không nhất quán.

## Local persistence đề xuất

Authoritative server = MongoDB. Offline local = file journal có framed records, checksum, fsync boundary và snapshots versioned, không coi file JSON ghi đè thiếu atomicity là đủ. Mỗi batch record chứa draft và outbox mutation, commit marker và sequence. UI chỉ báo saved sau durable commit. Crash ở tail không làm mất batch đã ACK local; truncated tail bị cách ly; disk full giữ bản trong RAM để export và báo không lưu được. Dùng single-writer actor, process lock, quota và compaction snapshot atomic. Chứng minh crash consistency trên Windows, kể cả rename/flush; phương án này phải được P00 ADR chấp nhận, không ép tự viết storage engine nếu rủi ro vượt lợi ích.

## HTTP và hợp đồng

Headers/query là ordered multimap có enabled flag, không dùng HashMap làm mất key trùng. Body là enum với streaming binary/multipart. Auth có cấu hình chia sẻ và SecretRef riêng. Effective variable resolution có precedence được tài liệu hóa, missing/cycle error. Spec source fidelity được giữ, normalize model cho tài liệu; validator không hiểu keyword phải báo unsupported. Không tuyên bố tương thích toàn bộ OpenAPI bằng một crate chỉ deserialize.

## Khả năng mở rộng

Index đúng theo workspace và keyset pagination trước cache; quotas, bounded queues và load shedding. Event log MongoDB là nguồn catch-up bền vững; WebSocket/SSE chỉ là tín hiệu realtime. Tránh yêu cầu Redis hoặc một hệ thống queue mới nếu chưa có nhu cầu đo được.
