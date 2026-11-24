# Prompt thực thi P10

Bạn là AI coding agent của Swaggen. Tiếp tục trong cùng repository theo `05-prompts/MASTER-PROMPT.md`, `AGENTS.md` và `01-prd/00-authoritative-decisions.md`.

Mục tiêu hiện tại: **P10 — Sandbox, vault và chính sách nâng cao bản C**. Đọc `04-phases/P10.md`, các requirement/screen ở traceability được gán phase này, status và evidence hiện có. Xác minh dependency `P09` từ bằng chứng, không chỉ nhãn completed.

Secret vault permissions/rotation/audit/key recovery; isolated scripting/extensions; documented Postman pm compatibility subset.

Đầu ra bắt buộc: threat model, sandbox enforcement tests, vault key lifecycle, capability policies. Gate: Time/memory/network/file/secret limits enforced; không sandbox giả; không tuyên bố E2EE nếu chưa thiết kế đúng.

Trước code, kiểm tra framework/crate/SDK/API chính thức theo exact version trong lockfile; bổ sung preflight của phase, chạy smoke test nhỏ cho dependency mới. Làm việc theo vertical slice và cập nhật evidence sau mỗi slice. Không yêu cầu người dùng nhắc lại tech stack. Không tự đổi sang web UI hoặc SQL database. Không dựng dữ liệu mẫu để thay backend thật. Không dùng test skipped làm pass.

Khi đạt gate, chạy kiểm tra phù hợp, ghi `reports/P10-completion.md` và JSON ledger, cập nhật status và bắt đầu phase kế tiếp nếu còn trong phạm vi đã giao. Nếu thiếu quyền/máy Windows/credentials, ghi blocked cụ thể và tiếp tục phần độc lập; không triển khai production khi thiếu đích/credentials/quyền cần thiết.
