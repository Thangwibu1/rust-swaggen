# Prompt thực thi P05

Bạn là AI coding agent của Swaggen. Tiếp tục trong cùng repository theo `05-prompts/MASTER-PROMPT.md`, `AGENTS.md` và `01-prd/00-authoritative-decisions.md`.

Mục tiêu hiện tại: **P05 — Publish, đồng bộ và cộng tác bản A**. Đọc `04-phases/P05.md`, các requirement/screen ở traceability được gán phase này, status và evidence hiện có. Xác minh dependency `P02 + P03 + P04` từ bằng chứng, không chỉ nhãn completed.

Durable outbox, operation id/hash/base revision, transaction publish, ordered events/replay, notifications, comments, three-way conflict, offline/revoke/delete handling.

Đầu ra bắt buộc: two-client E2E, replay protocol, conflict UI, fault-injection report. Gate: Mất ACK không duplicate; offline+restart không mất nháp; permission race test; reconnect/cursor consistent.

Trước code, kiểm tra framework/crate/SDK/API chính thức theo exact version trong lockfile; bổ sung preflight của phase, chạy smoke test nhỏ cho dependency mới. Làm việc theo vertical slice và cập nhật evidence sau mỗi slice. Không yêu cầu người dùng nhắc lại tech stack. Không tự đổi sang web UI hoặc SQL database. Không dựng dữ liệu mẫu để thay backend thật. Không dùng test skipped làm pass.

Khi đạt gate, chạy kiểm tra phù hợp, ghi `reports/P05-completion.md` và JSON ledger, cập nhật status và bắt đầu phase kế tiếp nếu còn trong phạm vi đã giao. Nếu thiếu quyền/máy Windows/credentials, ghi blocked cụ thể và tiếp tục phần độc lập; không triển khai production khi thiếu đích/credentials/quyền cần thiết.
