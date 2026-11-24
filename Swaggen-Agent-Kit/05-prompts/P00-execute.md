# Prompt thực thi P00

Bạn là AI coding agent của Swaggen. Tiếp tục trong cùng repository theo `05-prompts/MASTER-PROMPT.md`, `AGENTS.md` và `01-prd/00-authoritative-decisions.md`.

Mục tiêu hiện tại: **P00 — Preflight và prototype**. Đọc `04-phases/P00.md`, các requirement/screen ở traceability được gán phase này, status và evidence hiện có. Xác minh dependency `none` từ bằng chứng, không chỉ nhãn completed.

Kiểm tra docs/SDK/toolchain/license/MSRV; chạy native UI/editor/IME/DPI spike, storage durability spike và MongoDB transaction thật. Chọn framework và local store bằng ADR; tạo dependency matrix.

Đầu ra bắt buộc: preflight.md, dependency-matrix.md, ADR-0001-ui.md, ADR-0002-local-store.md, transaction-smoke.md. Gate: Gate có bằng chứng Windows và DB; thiếu SDK/máy Windows ghi BLOCKED đúng scope; không scaffold full app trước compatibility gate.

Trước code, kiểm tra framework/crate/SDK/API chính thức theo exact version trong lockfile; bổ sung preflight của phase, chạy smoke test nhỏ cho dependency mới. Làm việc theo vertical slice và cập nhật evidence sau mỗi slice. Không yêu cầu người dùng nhắc lại tech stack. Không tự đổi sang web UI hoặc SQL database. Không dựng dữ liệu mẫu để thay backend thật. Không dùng test skipped làm pass.

Khi đạt gate, chạy kiểm tra phù hợp, ghi `reports/P00-completion.md` và JSON ledger, cập nhật status và bắt đầu phase kế tiếp nếu còn trong phạm vi đã giao. Nếu thiếu quyền/máy Windows/credentials, ghi blocked cụ thể và tiếp tục phần độc lập; không triển khai production khi thiếu đích/credentials/quyền cần thiết.
