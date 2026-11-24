# Hiệu năng, bảo mật và bằng chứng nghiệm thu

Các budget dưới đây là mục tiêu kỹ thuật đề xuất, không phải số đo đã đạt. Đánh giá trên Windows 11 x64, 4 core/8GB RAM/SSD, độ phân giải và GPU được ghi, release build, fixture deterministic, 5 warm-up + >=30 samples cho thao tác ngắn; startup >=10 lần. Báo p50/p95/p99 khi sample đủ, cold/warm tách riêng, commit/toolchain/data size kèm raw measurements. Không dùng thời gian mạng Internet làm proxy cho tốc độ UI.

| Workload | Gate đề xuất |
| --- | --- |
| First usable startup local | p95 <= 2 giây, không chờ server online |
| Open/search 1.000 request | p95 <= 200ms theo AC10; dataset 10.000 là stress extension |
| Interaction / typing | p95 event-to-visible <= 50ms; frame đang tương tác mục tiêu <=16.7ms ở 60Hz, đo spikes |
| Idle CPU | mục tiêu dưới 1% trung bình 60s trên baseline, không busy repaint/poll liên tục |
| Idle desktop memory | baseline mục tiêu private bytes <=200MiB; phải đo trước và điều chỉnh bằng ADR có lý do, không quảng cáo số chưa đo |
| 20MB response | preview cap đề xuất 1MiB, stream raw file, UI không treo; đo delta memory thay vì chỉ total |
| 200MB response stress | bounded buffers/preview, tốc độ ổ đĩa, cancel cleanup; không O(body) clone trong UI |
| Cancel HTTP | UI phản hồi <100ms, transport/file cleanup mục tiêu <=1 giây trong fixture điều khiển được |
| Publish→client khác | p95 <=3 giây mạng LAN ổn định, không tính upload file lớn; đối chiếu AC02 |
| Server transaction | đo p95/p99 theo 1/5/20 concurrent publishers, dataset/index/topology ghi rõ; không đặt SLA bịa |
| Restart/crash | local durable draft/outbox đã ACK không mất, replay idempotent, journal tail corruption có xử lý |

## Cách đạt mục tiêu

Virtualize tree/table/history; keyset pagination; lazy load body/schema; incremental parsing và syntax highlighting; debounce search/validation có cancellation; bounded channels + backpressure; reuse HTTP/Mongo client pools; release optimizer và LTO chỉ sau đo compile/runtime tradeoff. Profile allocations/locks/flamegraph, xử lý bottleneck có bằng chứng. Không thêm unsafe, custom allocator, lock-free structure hoặc cache không giới hạn chỉ vì "hiệu năng cao".

## Test matrix tối thiểu

- Domain unit/property: variable precedence/cycles, duplicate query/headers/order/encoding, permissions, stable operation identity, diff, tombstones, state transitions.
- HTTP fixtures: JSON/XML/text/multipart/binary, cookie domain/path/expiry, 30x cross-host auth stripping, gzip sizes, TLS error/host-scoped exceptions, timeout/cancel, DNS/refused, invalid UTF-8, chunked/large bodies.
- Mongo integration: atomic commit/rollback, idempotency/unknown commit, conditional revisions, revoke race, transaction scope và workspace isolation. Mock không thay DB thật.
- Two-client E2E: invitation/role enforcement API+UI, publish/catch-up, conflict base-local-server, offline restart, revoked with pending edits, delete-vs-edit, cursor-expired snapshot, attachments, backup restore.
- UI Windows: all route/state entries trong screen map, keyboard/focus/IME, DPI 100–200%, screen reader, resize, modal, loading/error/empty/read-only, paste content an toàn; screenshot comparison có mask timestamp/font tolerance.
- Security: log/export/report không secret, imported HTML/Markdown không thực thi, zip-slip/decompression bomb, external refs, SSRF DNS+redirect, access checks object/files/events, script/plugin capability limits và key loss.
- Release: clean Windows install, run offline không DB/cloud/activation, upgrade migrate/rollback theo tài liệu, uninstall giữ/xóa user data đúng lựa chọn, signature và update tamper rejection.

## Definition of Done từng requirement

Yêu cầu → screen/state → domain/service → persistent model/migration → API nếu có → tests → measured evidence. Trạng thái cho phép: not_started, in_progress, implemented_unverified, verified, blocked. `verified` phải có commit + command + result + evidence path, không đánh dấu cả module bằng một screenshot.

Không đánh đồng benchmark tool tự trả số với đo hợp lệ; CI Linux không thay Windows GUI. Không bỏ test hoặc sửa expected chỉ để xanh. Chỉ gate đã thực sự chạy mới PASS; điều kiện thiếu phải BLOCKED.
