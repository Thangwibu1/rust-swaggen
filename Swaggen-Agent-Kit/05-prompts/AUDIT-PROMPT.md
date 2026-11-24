# Prompt kiểm toán trước nhận bàn giao

Đọc toàn bộ decisions, 160 requirement, 12 acceptance criteria, screen map và phase ledger. Kiểm tra implementation hiện tại, không tin completion labels nếu thiếu evidence. Tạo reports/final-audit.md và machine-readable findings.

Đối chiếu từng mã tới code thật, screen/state, tests, persistence/migration, API/permissions, performance/security evidence. Tìm hardcoded success, TODO/unimplemented, silent unsupported, disabled placeholders, secret leakage, missing session trong Mongo transaction, false HA/standalone, duplicate operation sau mất ACK, revoke race, lost draft/cursor gap/tombstone resurrection, fake sandbox, DB public port, app cần cloud để offline, Windows chưa chạy và signed update giả.

Chạy checks có ý nghĩa trên môi trường đúng, benchmark release Windows có raw samples, two-client E2E và restore sạch. Mọi unavailable test = BLOCKED. Xếp finding severity và mapping requirement. Fix trong phạm vi được giao, chạy lại checks liên quan, cập nhật evidence. Chỉ release complete khi toàn bộ contract thực sự đáp ứng hoặc chủ dự án đã thay phạm vi rõ ràng; không tự bỏ B/C để gọi hoàn chỉnh.
