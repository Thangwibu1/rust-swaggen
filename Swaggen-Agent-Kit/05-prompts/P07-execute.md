# Prompt thực thi P07

Bạn là AI coding agent của Swaggen. Tiếp tục trong cùng repository theo `05-prompts/MASTER-PROMPT.md`, `AGENTS.md` và `01-prd/00-authoritative-decisions.md`.

Mục tiêu hiện tại: **P07 — Mở rộng network, spec và import bản B**. Đọc `04-phases/P07.md`, các requirement/screen ở traceability được gán phase này, status và evidence hiện có. Xác minh dependency `P06` từ bằng chứng, không chỉ nhãn completed.

OAuth PKCE, proxy/CA/mTLS, response diff/timing có dữ liệu thật, schema nâng cao/multi-file/lint, import compatibility, file/export/Git, snippets và utilities.

Đầu ra bắt buộc: B adapters, security fixtures, import loss report, native editor evidence. Gate: Không rò secret; external refs có policy; schema unsupported rõ; OAuth state/PKCE callback verified.

Trước code, kiểm tra framework/crate/SDK/API chính thức theo exact version trong lockfile; bổ sung preflight của phase, chạy smoke test nhỏ cho dependency mới. Làm việc theo vertical slice và cập nhật evidence sau mỗi slice. Không yêu cầu người dùng nhắc lại tech stack. Không tự đổi sang web UI hoặc SQL database. Không dựng dữ liệu mẫu để thay backend thật. Không dùng test skipped làm pass.

Khi đạt gate, chạy kiểm tra phù hợp, ghi `reports/P07-completion.md` và JSON ledger, cập nhật status và bắt đầu phase kế tiếp nếu còn trong phạm vi đã giao. Nếu thiếu quyền/máy Windows/credentials, ghi blocked cụ thể và tiếp tục phần độc lập; không triển khai production khi thiếu đích/credentials/quyền cần thiết.
