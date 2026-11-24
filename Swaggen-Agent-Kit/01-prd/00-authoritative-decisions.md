# Quyết định có hiệu lực — Swaggen

Ngày chốt yêu cầu: 23-11-2026. Chủ dự án xác nhận chưa lập trình bất kỳ phần nào.

## Thứ tự nguồn

1. Xác nhận của chủ dự án trong cuộc trò chuyện, ghi tại đây.
2. PRD hiệu lực và các ADR đã được xác nhận hoặc có bằng chứng prototype.
3. Hành vi nguồn trong `00-source/PRD-original.md` và 160 yêu cầu trong `requirements.json`.
4. Figma là nguồn thiết kế, trạng thái và thành phần; màn hình không chứng minh tính năng đã chạy.
5. Đề xuất trong kit chỉ là mặc định kỹ thuật có thể điều chỉnh bằng ADR.

## Ràng buộc đã chốt

- Ứng dụng Windows desktop, Rust cho frontend native và backend. Không tự chuyển sang React, TypeScript, Electron, Tauri WebView hoặc ứng dụng web.
- Framework Rust cụ thể chưa được chủ dự án chọn. Đề xuất đánh giá Iced đầu tiên, egui/eframe làm ứng viên so sánh. Slint cần nói rõ DSL và giấy phép nếu cân nhắc.
- Database phía server là MongoDB; áp dụng multi-document transaction cho các bất biến nhiều tài liệu. Không tự thay bằng PostgreSQL/SQLite.
- Backend và MongoDB triển khai trên Coolify VPS. Desktop được build, đóng gói và cài trên Windows; không "deploy frontend desktop" như website trên VPS.
- Mục tiêu là toàn bộ phạm vi A+B+C có kiểm chứng, phát hành theo phase. Không coi A hoặc một prototype là toàn bộ sản phẩm.
- Không tự có mã nguồn, test pass, benchmark, installer hay server thật. Kit này là tài liệu và công cụ hỗ trợ xuất thiết kế.

## Các thay thế đối với PRD gốc

| Nội dung cũ | Quy tắc hiệu lực |
| --- | --- |
| PostgreSQL phía server | MongoDB official Rust driver, replica set, transaction |
| Tauri + React/Svelte, WebView2 | UI native Rust; SDK và renderer theo framework được prototype |
| SQLite local | Chưa được xác nhận. Mặc định đề xuất file journal/snapshot bằng Rust cho offline, không thêm DB khác; phải nghiệm thu durability trước khi dùng |
| Swagger UI nhúng | Renderer tài liệu native Rust; không mở remote HTML có đặc quyền |
| Tauri IPC capabilities | Ranh giới command/service native, file/network/secret và renderer nội dung an toàn |

## Xung đột thiết kế cần giữ nguyên dấu vết

- **Google sign-in:** Figma có A01–A04/S02/Z10 cho Google, PRD WS03 yêu cầu tài khoản nội bộ không phụ thuộc cloud. Mặc định triển khai nội bộ trước; Google provider tùy chọn, tắt mặc định, chỉ thực hiện khi có quyết định sản phẩm và credentials. Không xóa yêu cầu WS03 hoặc giả lập đăng nhập Google thành công. Không đồng nhất OAuth gọi API đích với tài khoản đăng nhập Swaggen.
- **Màu:** node `51:40541` yêu cầu bề mặt page/sidebar/panel/button/badge/notice trắng; chỉ foreground dùng màu ngữ nghĩa. Giữ palette cũ làm dấu vết; không sơn nền pastel theo token lịch sử.
- **Theme:** bản thiết kế hiện tại light-only; PRD UX06 vẫn có light/dark và Việt/Anh ở bản B. Light phải bám Figma; dark là thiết kế bổ sung có token và kiểm tra contrast, chưa có ảnh chuẩn.
- **Local offline:** MongoDB tự host không thay thế lưu cục bộ. Không yêu cầu người dùng cài `mongod` chỉ để mở app; nếu muốn MongoDB trên từng desktop, cần quyết định rõ đánh đổi và prototype riêng.

## Các giả định thiết kế, không phải lời xác nhận của chủ dự án

Iced + Axum là ứng viên mặc định; local journal là đề xuất; topology MongoDB và budget RAM là mục tiêu cần đo; Google login chưa chốt. AI được phép giải quyết lựa chọn kỹ thuật thường lệ bằng ADR có bằng chứng, không cần hỏi lại ràng buộc đã chốt.
