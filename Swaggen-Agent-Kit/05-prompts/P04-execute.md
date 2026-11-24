# Prompt thực thi P04

Bạn là AI coding agent của Swaggen. Tiếp tục trong cùng repository theo `05-prompts/MASTER-PROMPT.md`, `AGENTS.md` và `01-prd/00-authoritative-decisions.md`.

Mục tiêu hiện tại: **P04 — Server MongoDB và tài khoản nội bộ**. Đọc `04-phases/P04.md`, các requirement/screen ở traceability được gán phase này, status và evidence hiện có. Xác minh dependency `P01` từ bằng chứng, không chỉ nhãn completed.

Axum backend, auth/session/invitation, workspace/membership/roles, private Mongo topology, migrations/indexes và transaction repository.

Đầu ra bắt buộc: server binary/image, backend OpenAPI, Mongo indexes, auth/permission integration tests. Gate: Invitation one-time, Viewer bị chặn ở server, credentials không trên desktop; commit/rollback/retry tests thật.

Trước code, kiểm tra framework/crate/SDK/API chính thức theo exact version trong lockfile; bổ sung preflight của phase, chạy smoke test nhỏ cho dependency mới. Làm việc theo vertical slice và cập nhật evidence sau mỗi slice. Không yêu cầu người dùng nhắc lại tech stack. Không tự đổi sang web UI hoặc SQL database. Không dựng dữ liệu mẫu để thay backend thật. Không dùng test skipped làm pass.

Khi đạt gate, chạy kiểm tra phù hợp, ghi `reports/P04-completion.md` và JSON ledger, cập nhật status và bắt đầu phase kế tiếp nếu còn trong phạm vi đã giao. Nếu thiếu quyền/máy Windows/credentials, ghi blocked cụ thể và tiếp tục phần độc lập; không triển khai production khi thiếu đích/credentials/quyền cần thiết.
