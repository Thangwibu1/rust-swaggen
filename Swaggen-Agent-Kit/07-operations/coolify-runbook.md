# Runbook Coolify VPS và Windows release

Tài liệu triển khai mục tiêu; chưa có server thật, Dockerfile app hoặc credentials. AI tạo các file runtime sau khi dependency và phase gates qua, không phát hành mẫu placeholder như deploy-ready.

## Backend trên Coolify

1. Build server binary Rust bằng multi-stage Dockerfile, toolchain/dependencies ghim; runtime non-root, readonly rootfs nếu phù hợp, data volume rõ, health tools thực sự có trong image. Build context/base directory nhất quán với Coolify Compose.
2. Deploy replica set MongoDB qua Compose/custom topology riêng. Không mặc định resource standalone của Coolify có transaction. Auth thành viên bằng keyfile/certificate, file ownership đúng UID, giữ secret ngoài Git; init chỉ một lần, idempotent và không reset DB.
3. Backend kết nối internal seed hostnames, `replicaSet` và authSource đúng, DB role tối thiểu. Không publish port MongoDB ra Internet; Coolify reverse proxy chỉ route HTTPS backend. DNS, TLS, WebSocket/SSE timeouts và body limits được kiểm tra từ desktop.
4. Pin Mongo image digest + compatibility matrix; persistent DB/config/attachment volumes; CPU/memory/disk quotas. Ba DB members trên cùng VPS chỉ là topology thử nghiệm, không phải HA qua failure domain.
5. Bootstrap Owner bằng cơ chế một lần; tắt sau tạo. Không để default password, không ghi URI/token vào logs. Chạy migrations/indexes bằng job kiểm soát, lock migration và backup trước thay đổi không tương thích.
6. Readiness phải kiểm tra DB session/topology/migration version. Job doctor riêng chạy write transaction+rollback trong DB test có scope, không mutate dữ liệu người dùng trong health probe.
7. Smoke test: client connect, login/invite, publish, two-client sync, file, disconnect/reconnect, server restart, unknown commit handling, secret redaction. Kiểm tra transaction không chỉ ping DB.

## Backup và restore

- Xác định RPO/RTO theo nhu cầu, ví dụ mục tiêu đề xuất RPO 24h/RTO 2h; phải đo và chủ dự án có thể chỉnh.
- Backup có điểm nhất quán DB+events+attachments; dùng quiesce hoặc phương pháp snapshot/oplog đã kiểm chứng đúng MongoDB tool/version/topology, không gọi mọi mongodump trong lúc ghi là consistent tùy ý.
- Manifest chứa schema/protocol/app version, snapshot/cursor, counts, hashes, key dependencies. Encryption key không nhét công khai cùng backup; hướng dẫn key mất/rotate.
- Copy backup sang failure domain khác VPS. Volume trên cùng máy không phải off-host backup. Retention và disk-full alert.
- Restore vào môi trường sạch riêng, verify hash/count/revisions/memberships/events; chạy two-client sync và token ownership; chỉ khi pass mới ghi restore verified.
- Upgrade: backup → migration preflight → compatibility gate → deploy → smoke → monitor. Binary rollback không đồng nghĩa DB downgrade an toàn; ghi forward-fix/restore path và downtime.

## Windows desktop

Build/test trên Windows target MSVC thật; bundle font/icon đã có license và native runtime dependencies thực tế. Không bắt WebView2 khi Rust native không dùng WebView. Tạo MSI/EXE cài sạch, uninstall/upgrade và đường dẫn dữ liệu per-user. Tránh admin cho runtime nếu không cần. Secret qua Windows protection; migration folder atomic có rollback/checksum.

Signing cần certificate/quyền thực tế. Nếu chưa có, build unsigned có nhãn rõ và để ký BLOCKED; không tạo chứng chỉ giả rồi gọi trusted. Update self-hosted feed phải verify signature/hash/version trước apply; ký ứng dụng và ký manifest là các trách nhiệm riêng.
