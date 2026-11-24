# Prompt thực thi P12

Bạn là AI coding agent của Swaggen. Tiếp tục trong cùng repository theo `05-prompts/MASTER-PROMPT.md`, `AGENTS.md` và `01-prd/00-authoritative-decisions.md`.

Mục tiêu hiện tại: **P12 — Hoàn thiện toàn bộ sản phẩm và tối ưu**. Đọc `04-phases/P12.md`, các requirement/screen ở traceability được gán phase này, status và evidence hiện có. Xác minh dependency `P11` từ bằng chứng, không chỉ nhãn completed.

Đối chiếu đủ 160 mã và mọi screen/state; xử lý backlog/gap; profile/optimize measured bottlenecks; full Windows regression, package, restore, soak và release docs.

Đầu ra bắt buộc: full release candidate, 160-item verified ledger, screen coverage, performance raw data, SBOM/license notices. Gate: Không còn requirement chưa verified, trừ phần sản phẩm được chủ dự án thay đổi rõ; không tuyên bố complete khi blocked. Nếu thiếu Google quyết định, báo phạm vi chờ riêng.

Trước code, kiểm tra framework/crate/SDK/API chính thức theo exact version trong lockfile; bổ sung preflight của phase, chạy smoke test nhỏ cho dependency mới. Làm việc theo vertical slice và cập nhật evidence sau mỗi slice. Không yêu cầu người dùng nhắc lại tech stack. Không tự đổi sang web UI hoặc SQL database. Không dựng dữ liệu mẫu để thay backend thật. Không dùng test skipped làm pass.

Khi đạt gate, chạy kiểm tra phù hợp, ghi `reports/P12-completion.md` và JSON ledger, cập nhật status và bắt đầu phase kế tiếp nếu còn trong phạm vi đã giao. Nếu thiếu quyền/máy Windows/credentials, ghi blocked cụ thể và tiếp tục phần độc lập; không triển khai production khi thiếu đích/credentials/quyền cần thiết.
