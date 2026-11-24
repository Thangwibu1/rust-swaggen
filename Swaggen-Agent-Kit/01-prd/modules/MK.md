# MK — Yêu cầu nguồn

## MK01 · B · Mock từ example

Sinh response mẫu theo endpoint status và media type; chạy local bằng cổng được chọn.

## MK02 · B · Mock từ schema

Sinh dữ liệu mẫu có seed; báo schema không sinh được và cho thay bằng example thủ công.

## MK03 · C · Kịch bản mock

Match query header body, giả lập độ trễ và lỗi; ưu tiên rule rõ ràng, có log đã làm sạch.

## MK04 · B · Mock dùng chung

Host mock ở server nhóm, đặt quyền hoặc token truy cập; không tự mở cổng máy cá nhân ra Internet.

## MK05 · B · CLI Rust

Chạy collection từ project export trên Windows, tham số môi trường và biến từ secret store hoặc biến tiến trình.

## MK06 · B · Kết quả cho CI

Exit code theo thành công thất bại, xuất JSON JUnit; che secret trong stdout stderr và report.

## MK07 · C · Runner tự host

Đăng ký runner với workspace và quyền hạn tối thiểu; tách mạng runner khỏi quyền quản trị server.

## MK08 · C · Lịch chạy kiểm tra

Lịch chạy múi giờ cấu hình, retry có giới hạn; chỉ hoạt động khi runner được chỉ định online.

## MK09 · C · Thông báo lỗi chạy

Thông báo trong app từ runner; webhook tới hệ thống nhóm tự chọn là tùy chọn.

## MK10 · B · Quản lý tài nguyên

Giới hạn số job body retention và log; owner có thể hủy lịch hoặc job để tránh quá tải server.
