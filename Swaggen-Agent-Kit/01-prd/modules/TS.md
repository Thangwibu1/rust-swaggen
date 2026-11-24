# TS — Yêu cầu nguồn

## TS01 · A · Assertion cơ bản

Kiểm tra status header thời gian và JSON path bằng form; lưu expected values với request.

## TS02 · B · Collection runner

Chạy tuần tự request trong folder hoặc collection, chọn environment số vòng và khoảng nghỉ; dừng khi người dùng hủy.

## TS03 · B · Chuỗi phụ thuộc

Trích token hoặc ID từ response cho bước kế; quy tắc fail fast hoặc tiếp tục được cấu hình rõ.

## TS04 · B · Chạy theo dữ liệu

Nhập CSV JSON làm data set, xem trước kiểu và dòng lỗi; dữ liệu nhạy cảm không tự publish.

## TS05 · B · Báo cáo kết quả

Hiển thị pass fail skipped cùng assertion và thời gian; chỉ rõ lỗi hạ tầng khác với assertion thất bại.

## TS06 · B · Kiểm thử hợp đồng

Chạy validation OpenAPI với status media type và schema tương ứng; liệt kê giới hạn validator.

## TS07 · C · Pre request và test script

Runtime JavaScript cách ly, giới hạn thời gian bộ nhớ và API; không cho script đọc file hay chạy shell mặc định.

## TS08 · C · Tương thích script Postman

Công bố tập pm API hỗ trợ, migration report và case lỗi; không tuyên bố tương thích toàn bộ chỉ vì chạy được JavaScript.

## TS09 · C · Chạy song song có giới hạn

Giới hạn concurrency và rate, chỉ chạy request độc lập; không coi đây là công cụ load test chuyên dụng.

## TS10 · B · Kết quả tái hiện

Gắn run với revision project, môi trường công khai và phiên bản engine; không lưu token thật vào báo cáo.
