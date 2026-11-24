# SC — Yêu cầu nguồn

## SC01 · A · Kho bảo mật Windows

Lưu token khóa và session bằng cơ chế Windows phù hợp; không coi SQLite thường là kho bí mật đã mã hóa.

## SC02 · A · Làm sạch dữ liệu

Che auth cookie password và trường được chỉ định trong log report export; preview lại vì không thể nhận diện mọi secret tự động.

## SC03 · A · Đăng nhập máy chủ

Hash mật khẩu bằng thuật toán phù hợp, TLS, session có hạn và thu hồi; rate limit đăng nhập và token mời một lần.

## SC04 · A · Cách ly workspace

Kiểm tra quyền ở mọi API tài nguyên file và kênh sự kiện; ID bị đoán được cũng không cho vượt workspace.

## SC05 · A · Nội dung không tin cậy

Không thực thi HTML hoặc script từ response Markdown hay example; khử nội dung nguy hiểm trong renderer.

## SC06 · A · Quyền ứng dụng desktop

Giới hạn IPC command và phạm vi file; không cấp quyền Rust hoặc hệ thống cho trang remote trong WebView. [3]

## SC07 · A · Không tự gửi telemetry

Mặc định tắt analytics và báo crash qua dịch vụ ngoài; diagnostic bundle được xem trước và gửi chủ động.

## SC08 · B · Chặn SSRF trên server

Mock runner fetch spec hoặc webhook ở server phải có allowlist và kiểm tra IP DNS redirect; client cá nhân vẫn gọi localhost theo chủ đích.

## SC09 · B · Phục hồi tài khoản và MFA

Owner quản lý reset an toàn; bổ sung TOTP hoặc giải pháp nội bộ, có recovery code và thu hồi session.

## SC10 · B · Audit và giới hạn lưu

Ghi thao tác quản trị publish restore và quyền; đặt retention, export audit và không hứa log bất biến nếu chưa có cơ chế chống sửa.
