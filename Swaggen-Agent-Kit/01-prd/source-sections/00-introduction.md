# Đặc tả chức năng ứng dụng cộng tác API

*Windows desktop với lõi Rust*

Phiên bản 1.0  |  Ngày 23 tháng 11 năm 2026  |  160 chức năng đề xuất

Ứng dụng hợp nhất việc thiết kế API, viết tài liệu, gửi request và kiểm thử để một nhóm bạn có thể phát triển phần mềm cùng nhau. Dữ liệu được lưu trên máy và đồng bộ qua máy chủ do nhóm tự quản lý, không bắt buộc dùng tài khoản Postman, SwaggerHub hay dịch vụ cloud của nhà cung cấp.

Phạm vi sản phẩm là ứng dụng Windows desktop. Máy chủ cộng tác là thành phần hạ tầng tự host, có thể chạy trên VPS Linux hoặc máy Windows luôn bật; không phát sinh yêu cầu xây thêm ứng dụng web cho người dùng.

### Kết quả cần đạt

- Một người thiết kế endpoint và mô tả dữ liệu; bạn bè nhận thay đổi trong workspace chung.

- Mỗi thành viên gọi API từ máy của mình, dùng token và môi trường cá nhân.

- Tài liệu OpenAPI, request mẫu, ví dụ response, bình luận và lịch sử thay đổi liên kết với nhau.

- Mất mạng vẫn soạn request và tài liệu; khi nối lại, ứng dụng đồng bộ và chỉ rõ các xung đột.

- Không đặt phí bản quyền theo số ghế trong sản phẩm tự xây. Nhóm vẫn tự chi trả và vận hành hạ tầng nếu dùng VPS.

### Cách đọc tài liệu

Các chức năng được đánh mã để chuyển thành backlog và kiểm thử. Đây là danh mục sản phẩm đề xuất đầy đủ cho nhu cầu nhóm, không phải cam kết tương thích 100 phần trăm với mọi tính năng của Postman hoặc toàn bộ hệ sinh thái Swagger.

| Nhãn | Ý nghĩa |
| --- | --- |
| A | Bản đầu dùng được cho nhóm: bắt buộc có trước khi mời bạn bè dùng hằng ngày. |
| B | Bản mở rộng: hoàn thiện kiểm thử, review và tự động hóa. |
| C | Nâng cao: giao thức bổ sung, đồng biên tập và tích hợp chuyên sâu. |
