# Prompt thực thi P03

Bạn là AI coding agent của Swaggen. Tiếp tục trong cùng repository theo `05-prompts/MASTER-PROMPT.md`, `AGENTS.md` và `01-prd/00-authoritative-decisions.md`.

Mục tiêu hiện tại: **P03 — OpenAPI và hợp đồng bản A**. Đọc `04-phases/P03.md`, các requirement/screen ở traceability được gán phase này, status và evidence hiện có. Xác minh dependency `P02` từ bằng chứng, không chỉ nhãn completed.

Import 3.0/3.1, editor native, validation/ref nội bộ, schema/endpoint/docs native, operation identity, linked request overrides/diff, export sạch và assertions cơ bản.

Đầu ra bắt buộc: spec fixtures, capability/unsupported matrix, doc renderer, import/export roundtrip. Gate: Spec invalid giữ draft, keyword chưa hiểu không báo valid; đổi path không mất link; Try dùng engine thật.

Trước code, kiểm tra framework/crate/SDK/API chính thức theo exact version trong lockfile; bổ sung preflight của phase, chạy smoke test nhỏ cho dependency mới. Làm việc theo vertical slice và cập nhật evidence sau mỗi slice. Không yêu cầu người dùng nhắc lại tech stack. Không tự đổi sang web UI hoặc SQL database. Không dựng dữ liệu mẫu để thay backend thật. Không dùng test skipped làm pass.

Khi đạt gate, chạy kiểm tra phù hợp, ghi `reports/P03-completion.md` và JSON ledger, cập nhật status và bắt đầu phase kế tiếp nếu còn trong phạm vi đã giao. Nếu thiếu quyền/máy Windows/credentials, ghi blocked cụ thể và tiếp tục phần độc lập; không triển khai production khi thiếu đích/credentials/quyền cần thiết.
