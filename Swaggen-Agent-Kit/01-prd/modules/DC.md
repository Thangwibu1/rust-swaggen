# DC — Yêu cầu nguồn

## DC01 · A · Tài liệu tự sinh

Hiển thị endpoint theo tag, method, path và mô tả từ đặc tả; mở được offline trong app.

## DC02 · A · Chi tiết hợp đồng

Hiển thị params required type default enum, body schema, status và ví dụ; tìm theo endpoint hoặc schema.

## DC03 · A · Chạy từ tài liệu

Try request mở request builder cùng environment auth và path params; gọi bằng Rust engine trên máy.

## DC04 · A · Liên kết định danh

Lưu ID endpoint ổn định và ánh xạ operationId khi có; đổi path không tự tạo request trùng hoặc làm mất liên kết.

## DC05 · A · Sinh request mẫu

Tạo request từ operation và example; đánh dấu trường do spec quản lý và phần override của người dùng.

## DC06 · A · Cập nhật từ đặc tả

Khi spec đổi, hiển thị diff và đề xuất cập nhật request; giữ token cục bộ và báo override không còn phù hợp.

## DC07 · B · Đề xuất sửa hợp đồng

Từ request thủ công, tạo draft patch cho spec; người dùng review trước publish, không tự suy ra hợp đồng đầy đủ từ một response.

## DC08 · B · Kiểm tra response theo schema

Đối chiếu status content type và schema; báo vị trí dữ liệu sai, keyword chưa hỗ trợ và phiên bản schema.

## DC09 · B · Chênh lệch hợp đồng

Phát hiện request hoặc test còn tham chiếu endpoint cũ; liệt kê phần cần sửa khi xóa hoặc đổi operation.

## DC10 · B · Xuất tài liệu

Xuất HTML tự chứa hoặc Markdown, có bước làm sạch secret; tạo snapshot gắn version để chia sẻ ngoài app.
