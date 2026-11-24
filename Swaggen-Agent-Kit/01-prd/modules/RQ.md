# RQ — Yêu cầu nguồn

## RQ01 · A · Method và URL

Hỗ trợ GET POST PUT PATCH DELETE HEAD OPTIONS; nhập URL, path params và query params, xem URL thực tế trước khi gửi.

## RQ02 · A · Query chính xác

Bật tắt từng tham số, giữ thứ tự và tham số trùng tên; kiểm soát encoding để không làm thay đổi ý nghĩa request.

## RQ03 · A · Headers

Thêm, sửa, bật tắt header; xem header tự sinh, xử lý giá trị trùng theo quy tắc HTTP và báo giá trị không hợp lệ.

## RQ04 · A · Các loại body

Soạn JSON, text, XML, form URL encoded, multipart và binary; chọn Content Type và báo lỗi JSON trước khi gửi.

## RQ05 · A · Tải file lên

Chọn file từ máy, nhập trường multipart và tên file; đường dẫn riêng không đồng bộ thành file của người khác.

## RQ06 · A · Gửi và hủy

Gửi bất đồng bộ, đặt timeout, hủy request; phân biệt hủy bởi người dùng, lỗi mạng và HTTP trả lỗi.

## RQ07 · A · Request đã phân giải

Xem method URL headers body sau khi thay biến; che secret theo mặc định và cảnh báo thiếu biến.

## RQ08 · A · Nháp và lưu

Tự lưu bản nháp riêng, đánh dấu khác biệt so với bản chung; Publish mới tạo thay đổi cho nhóm.

## RQ09 · A · Sao chép nhanh

Nhân bản request, lưu thành mẫu mới và mở request từ lịch sử mà không sửa bản chung ngoài ý muốn.

## RQ10 · B · Stream khi truyền

Hiển thị tiến độ upload download, giữ UI phản hồi; ghi dữ liệu lớn xuống file thay vì đưa toàn bộ vào RAM.
