# OP — Yêu cầu nguồn

## OP01 · A · Bộ cài Windows

Đóng gói MSI hoặc EXE, kiểm tra prerequisite gồm WebView2 nếu dùng Tauri; bản đầu nhắm Windows 11 x64.

## OP02 · A · Chế độ không có cloud

App cá nhân chạy offline; bản nhóm chỉ kết nối địa chỉ self host được cấu hình, không cần activation server bên ngoài.

## OP03 · A · Khởi tạo server

Cung cấp cấu hình Docker Compose hoặc binary, tạo Owner lần đầu và vô hiệu cơ chế bootstrap sau khi xong.

## OP04 · A · Cấu hình triển khai

Biến cấu hình domain TLS DB data path và giới hạn tải; hỗ trợ reverse proxy và volume bền vững.

## OP05 · A · Sao lưu server

Backup nhất quán database file và metadata phiên bản; xuất manifest kiểm tra và cấu hình lịch backup do nhóm quản lý.

## OP06 · A · Khôi phục dữ liệu

Restore sang server sạch, kiểm tra liên kết attachment và version; quy trình xử lý mất key hoặc thiếu file được ghi rõ.

## OP07 · A · Nâng cấp và migration

Migration có version và backup trước nâng cấp; kiểm tra tương thích client server, không hạ DB bằng thao tác thiếu kiểm soát.

## OP08 · B · Phân phối cập nhật

Bản đầu cập nhật thủ công; bản sau hỗ trợ feed tự host và xác minh chữ ký, không tin file cập nhật không được kiểm tra.

## OP09 · A · Theo dõi sức khỏe

Health endpoint, log có request ID, dung lượng DB file và hàng đợi; log không chứa body hoặc secret mặc định.

## OP10 · B · Quota vận hành

Đặt giới hạn upload retention và số job theo năng lực máy chủ; đây là giới hạn tài nguyên do nhóm quản trị, không phải phí theo ghế.
