# CO — Yêu cầu nguồn

## CO01 · A · Workspace chung

Thành viên xem cùng collection spec và cấu hình công khai qua server tự host; không có giới hạn số ghế do mô hình tính phí của app.

## CO02 · A · Publish thay đổi

Lưu nháp riêng rồi publish nội dung chung với ghi chú; người khác nhận đúng phiên bản đã công bố.

## CO03 · A · Thông báo thay đổi

Hiện endpoint hoặc request vừa được sửa, tác giả và thời gian; refresh bằng sự kiện server cùng cơ chế lấy bù.

## CO04 · A · Bình luận theo đối tượng

Thread tại endpoint request hoặc schema, trả lời và đánh dấu đã xử lý; lưu liên kết tới revision được thảo luận.

## CO05 · B · Nhắc thành viên

Mention thành viên tạo thông báo trong app; không bắt buộc email hay dịch vụ nhắn tin ngoài.

## CO06 · A · Luồng hoạt động

Xem ai tạo sửa xóa publish và thay quyền; lọc theo người hoặc đối tượng trong workspace.

## CO07 · B · Trạng thái chỉnh sửa

Hiện ai đang mở hoặc sửa đối tượng khi online; đây là tín hiệu hỗ trợ, không thay thế kiểm soát xung đột.

## CO08 · B · Chia sẻ kết quả chạy

Chủ động gửi báo cáo hoặc example đã làm sạch vào workspace; người nhận không thấy token cá nhân.

## CO09 · B · Liên kết nội bộ

Copy deep link tới workspace endpoint và comment; mở trong app, luôn yêu cầu đăng nhập và kiểm tra quyền.

## CO10 · C · Đồng biên tập

Cùng sửa một nội dung với CRDT hoặc OT, con trỏ và undo phù hợp; phải hỗ trợ offline và thu hồi quyền trước phát hành.
