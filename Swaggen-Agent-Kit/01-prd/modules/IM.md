# IM — Yêu cầu nguồn

## IM01 · A · Định dạng project mở

Xuất project theo schema có version, ID ổn định và manifest; nêu rõ các file chứa spec request và môi trường công khai.

## IM02 · A · Postman Collection

Import Collection v2.1 với folder request variables auth và examples; xuất phần hỗ trợ được và kèm báo cáo tương thích.

## IM03 · A · Postman Environment

Import tên và cấu trúc biến, cho xem trước giá trị; không tự đồng bộ token được nhập vào server nhóm.

## IM04 · A · OpenAPI import export

Giữ JSON YAML và các extension chưa hiểu khi có thể; mọi chuyển đổi mất dữ liệu phải báo rõ.

## IM05 · A · cURL hai chiều

Dán lệnh để tạo request, sinh lệnh cURL từ request; parse không thực thi shell và xử lý khác biệt quoting Windows.

## IM06 · A · Xem trước import

Liệt kê đối tượng mới, trùng, bị bỏ qua và phần không hỗ trợ; chọn tạo project mới hoặc nhập vào project có sẵn.

## IM07 · B · File đính kèm

Chọn đóng gói file dùng chung, giới hạn kích thước và hash; không tự thu thập file từ đường dẫn riêng của thành viên.

## IM08 · A · Xuất dữ liệu đã làm sạch

Preview tên biến giá trị auth examples và file; mặc định loại secret cùng lịch sử cá nhân.

## IM09 · B · Tích hợp Git

Lưu project dạng text có thứ tự ổn định, xem diff và merge; cho dùng Git server tự host hoặc Git cục bộ.

## IM10 · C · Định dạng bổ sung

Đánh giá HAR HTTP files và công cụ khác; chỉ ghi tương thích cho tập tính năng đã kiểm thử.
