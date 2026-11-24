# RS — Yêu cầu nguồn

## RS01 · A · Thông tin tổng quan

Hiển thị status code, thời gian tổng, kích thước và thời điểm chạy; phân biệt body nén và giải nén nếu có số đo.

## RS02 · A · Hiển thị body

Raw text, JSON có tô màu và gấp nhánh, XML text; nhận diện binary và cho tải file xuống.

## RS03 · A · Response headers

Xem toàn bộ header, gồm các header lặp; xem cookie do server trả về.

## RS04 · A · Tìm và sao chép

Tìm kiếm trong body, sao chép giá trị hoặc JSON path, format và lưu response thành file.

## RS05 · A · Thông báo lỗi

Phân loại DNS, connection refused, TLS, timeout, hủy và lỗi parse; hướng dẫn xử lý theo tình huống.

## RS06 · A · Response lớn

Giới hạn phần preview, phân trang hoặc render ảo; vẫn cho lưu raw file và thông báo khi nội dung bị cắt hiển thị.

## RS07 · A · Lịch sử riêng

Lưu lần chạy trên máy với bộ lọc và thời hạn giữ; xóa theo request hoặc toàn bộ mà không xóa template.

## RS08 · A · Ví dụ chia sẻ

Lưu response thành example sau khi xem trước và làm sạch dữ liệu; liên kết với endpoint và status tương ứng.

## RS09 · B · So sánh response

Diff JSON hoặc text giữa hai lần chạy, bỏ qua trường biến động theo cấu hình của người dùng.

## RS10 · B · Đo thời gian chi tiết

Đo DNS TCP TLS TTFB khi transport cung cấp được; ghi không khả dụng thay vì ước đoán thành số đo thật.
