# Prompt thực thi P11

Bạn là AI coding agent của Swaggen. Tiếp tục trong cùng repository theo `05-prompts/MASTER-PROMPT.md`, `AGENTS.md` và `01-prd/00-authoritative-decisions.md`.

Mục tiêu hiện tại: **P11 — Giao thức và cộng tác nâng cao bản C**. Đọc `04-phases/P11.md`, các requirement/screen ở traceability được gán phase này, status và evidence hiện có. Xác minh dependency `P10` từ bằng chứng, không chỉ nhãn completed.

GraphQL schema, WebSocket/gRPC, protocol policy, SDK/stub tools, real-time spec edit, self-hosted/scheduled jobs, advanced mock/import và bounded parallelism.

Đầu ra bắt buộc: protocol adapters, runner registration/leases, schedules timezone/DST, CRDT/OT ADR nếu cần. Gate: Runner offline/retry/cancel đúng; editor reconnect không mất thay đổi; chỉ giao thức đã test được tuyên bố hỗ trợ.

Trước code, kiểm tra framework/crate/SDK/API chính thức theo exact version trong lockfile; bổ sung preflight của phase, chạy smoke test nhỏ cho dependency mới. Làm việc theo vertical slice và cập nhật evidence sau mỗi slice. Không yêu cầu người dùng nhắc lại tech stack. Không tự đổi sang web UI hoặc SQL database. Không dựng dữ liệu mẫu để thay backend thật. Không dùng test skipped làm pass.

Khi đạt gate, chạy kiểm tra phù hợp, ghi `reports/P11-completion.md` và JSON ledger, cập nhật status và bắt đầu phase kế tiếp nếu còn trong phạm vi đã giao. Nếu thiếu quyền/máy Windows/credentials, ghi blocked cụ thể và tiếp tục phần độc lập; không triển khai production khi thiếu đích/credentials/quyền cần thiết.
