# EV — Yêu cầu nguồn

## EV01 · A · Môi trường

Tạo local dev staging prod; hiển thị môi trường đang dùng cạnh nút gửi request.

## EV02 · A · Biến theo cấp

Hỗ trợ workspace, collection, environment và request; trình kiểm tra chỉ rõ giá trị hiệu lực và nguồn.

## EV03 · A · Giá trị cá nhân

Đặt local override cho biến chung; bạn A và B có token khác nhau nhưng dùng cùng request template.

## EV04 · A · Thay biến

Dùng cú pháp {{ten_bien}} trong URL header body auth; phát hiện thiếu biến và vòng tham chiếu.

## EV05 · A · Đánh dấu secret

Đánh dấu nhạy cảm, che giá trị và lưu bằng cơ chế bảo vệ Windows; chỉ Rust core truy xuất khi cần gửi.

## EV06 · A · Quy tắc xuất

Mặc định xuất placeholder và tên biến; có bước xem trước nếu người dùng chủ động đưa giá trị nhạy cảm vào file.

## EV07 · A · Bảo vệ production

Màu môi trường khác biệt; yêu cầu xác nhận khi gửi request thay đổi dữ liệu đến môi trường đánh dấu production.

## EV08 · B · Biến động

Sinh UUID timestamp và giá trị ngẫu nhiên có quy tắc; preview trước khi gửi để dễ tái hiện.

## EV09 · B · Trích xuất giá trị

Lấy trường JSON hoặc header từ response để điền biến cục bộ cho request tiếp theo; không tự publish token.

## EV10 · C · Vault nhóm

Kho secret riêng có quyền truy cập, luân chuyển khóa, nhật ký; thiết kế và nghiệm thu riêng trước khi bật.
