# Prompt thực thi P06

Bạn là AI coding agent của Swaggen. Tiếp tục trong cùng repository theo `05-prompts/MASTER-PROMPT.md`, `AGENTS.md` và `01-prd/00-authoritative-decisions.md`.

Mục tiêu hiện tại: **P06 — Phát hành bản A trên Windows và Coolify**. Đọc `04-phases/P06.md`, các requirement/screen ở traceability được gán phase này, status và evidence hiện có. Xác minh dependency `P05` từ bằng chứng, không chỉ nhãn completed.

Hoàn thiện toàn bộ A, bootstrap/deploy/backup/restore/migration/health, installer Windows, quotas và benchmark baseline.

Đầu ra bắt buộc: A release candidate, Compose/Dockerfile thật, operations runbooks, AC01–AC12 evidence. Gate: 100% yêu cầu A verified, AC01–12 pass trên Windows+server; không dùng UI mock thay sync; restore sạch pass.

Trước code, kiểm tra framework/crate/SDK/API chính thức theo exact version trong lockfile; bổ sung preflight của phase, chạy smoke test nhỏ cho dependency mới. Làm việc theo vertical slice và cập nhật evidence sau mỗi slice. Không yêu cầu người dùng nhắc lại tech stack. Không tự đổi sang web UI hoặc SQL database. Không dựng dữ liệu mẫu để thay backend thật. Không dùng test skipped làm pass.

Khi đạt gate, chạy kiểm tra phù hợp, ghi `reports/P06-completion.md` và JSON ledger, cập nhật status và bắt đầu phase kế tiếp nếu còn trong phạm vi đã giao. Nếu thiếu quyền/máy Windows/credentials, ghi blocked cụ thể và tiếp tục phần độc lập; không triển khai production khi thiếu đích/credentials/quyền cần thiết.
