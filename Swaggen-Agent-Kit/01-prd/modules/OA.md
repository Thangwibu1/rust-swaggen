# OA — Yêu cầu nguồn

## OA01 · A · Import đặc tả

Mở JSON YAML từ file hoặc URL; ưu tiên OpenAPI 3.0 và 3.1, báo phiên bản không hỗ trợ; URL riêng lấy từ máy người dùng.

## OA02 · A · Editor đặc tả

Tô màu, format, tìm thay thế, đánh dấu dòng lỗi; giữ nguyên bản nháp khi validation thất bại.

## OA03 · A · Kiểm tra cấu trúc

Kiểm tra cú pháp, trường bắt buộc, path params và operationId; không báo đạt khi thư viện không hiểu keyword.

## OA04 · A · Quản lý endpoint

Thêm method path summary description tags và deprecated; có kiểm tra trùng method cùng path.

## OA05 · A · Định nghĩa dữ liệu

Khai báo parameters requestBody responses content types và examples; phân biệt required nullable và optional theo phiên bản.

## OA06 · A · Components dùng lại

Quản lý schema parameters responses và security schemes; giải tham chiếu nội bộ và phát hiện tham chiếu không tồn tại.

## OA07 · B · Schema phức tạp

Hiển thị allOf oneOf anyOf enum discriminator và đệ quy với giới hạn; không render vô hạn khi có vòng tham chiếu.

## OA08 · B · Tham chiếu nhiều file

Giải relative reference trong project, bundle khi export; tham chiếu URL ngoài cần cho phép rõ và giới hạn tài nguyên.

## OA09 · B · Swagger 2 và phiên bản mới

Chuyển Swagger 2.0 bằng công cụ phù hợp, đánh dấu phần mất mát; đánh giá OpenAPI 3.2 riêng, không mặc định coi tương thích.

## OA10 · B · Lint quy ước nhóm

Quy tắc đặt operationId, mô tả, error schema và security; phân biệt lỗi đặc tả với cảnh báo quy ước.
