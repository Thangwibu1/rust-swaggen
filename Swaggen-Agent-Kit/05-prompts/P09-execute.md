# Prompt thực thi P09

Bạn là AI coding agent của Swaggen. Tiếp tục trong cùng repository theo `05-prompts/MASTER-PROMPT.md`, `AGENTS.md` và `01-prd/00-authoritative-decisions.md`.

Mục tiêu hiện tại: **P09 — Review, trải nghiệm và vận hành bản B**. Đọc `04-phases/P09.md`, các requirement/screen ở traceability được gán phase này, status và evidence hiện có. Xác minh dependency `P08` từ bằng chứng, không chỉ nhãn completed.

Branches/review theo mã B, ownership/trash/audit/MFA, commands/accessibility/i18n/theme bổ sung/deep links, signed updates, retention và operational quotas.

Đầu ra bắt buộc: B release, UX/security evidence, migrations và rollback plan. Gate: 100% B verified hoặc sản phẩm còn thiếu ghi rõ; light Figma fidelity, dark supplementary reviewed; update tamper rejected.

Trước code, kiểm tra framework/crate/SDK/API chính thức theo exact version trong lockfile; bổ sung preflight của phase, chạy smoke test nhỏ cho dependency mới. Làm việc theo vertical slice và cập nhật evidence sau mỗi slice. Không yêu cầu người dùng nhắc lại tech stack. Không tự đổi sang web UI hoặc SQL database. Không dựng dữ liệu mẫu để thay backend thật. Không dùng test skipped làm pass.

Khi đạt gate, chạy kiểm tra phù hợp, ghi `reports/P09-completion.md` và JSON ledger, cập nhật status và bắt đầu phase kế tiếp nếu còn trong phạm vi đã giao. Nếu thiếu quyền/máy Windows/credentials, ghi blocked cụ thể và tiếp tục phần độc lập; không triển khai production khi thiếu đích/credentials/quyền cần thiết.
