# PR — Yêu cầu nguồn

## PR01 · B · GraphQL cơ bản

Editor query variables và headers; hiển thị data errors, lưu request trong collection.

## PR02 · C · GraphQL schema

Import SDL hoặc introspection được cho phép; gợi ý trường và kiểm tra query theo schema.

## PR03 · C · WebSocket

Kết nối URL auth, gửi nhận message, ngắt kết nối; lưu message mẫu và giới hạn bộ nhớ lịch sử.

## PR04 · B · Server Sent Events

Hiển thị event stream theo thời gian, hủy và cấu hình reconnect; không giữ vô hạn nội dung trong UI.

## PR05 · C · gRPC

Import proto, gọi unary trước; hiển thị metadata status và lỗi; streaming là khả năng phát hành riêng.

## PR06 · C · HTTP nâng cao

Chọn policy HTTP 1.1 hoặc 2 khi transport hỗ trợ; HTTP 3 cần đánh giá và không nằm trong bản A.

## PR07 · B · Sinh code gọi API

Sinh snippet cURL Rust reqwest JavaScript fetch Dart và Go; placeholder thay cho secret và không tự thực thi.

## PR08 · C · Sinh SDK và stub

Gọi generator theo phiên bản được ghim, chọn ngôn ngữ và xem output; dependency ngoài phải được thông báo.

## PR09 · B · Giải mã và chuyển đổi

URL encode decode Base64 JSON format và chuyển JSON YAML; chạy cục bộ và không gửi dữ liệu ra dịch vụ ngoài.

## PR10 · C · Extension có quyền

Plugin được bật theo quyền mạng file secret, cách ly và có thể gỡ; không cho plugin chạy với mọi quyền mặc định.
