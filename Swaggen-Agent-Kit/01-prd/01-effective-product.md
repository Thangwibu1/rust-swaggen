# PRD hiệu lực

## Mục tiêu

Một workspace Windows native để thiết kế hợp đồng API, viết tài liệu, gửi request từ máy cá nhân, kiểm thử và cộng tác qua máy chủ tự host. Cá nhân dùng offline không cần tài khoản cloud. Nhóm chia sẻ template/spec/example sạch; secret và lịch sử chạy giữ riêng.

## Luồng bắt buộc

1. Personal workspace → collection/folder/request → chọn environment → điền secret cục bộ → gửi API → xem response/history → restart còn nháp.
2. Import OpenAPI 3.0/3.1 → validation trung thực → sửa endpoint/schema → tài liệu native → sinh request có operation identity → review diff khi spec đổi.
3. Kết nối server → lời mời một lần → vai trò Owner/Admin/Editor/Viewer → publish → MongoDB transaction → ACK → client khác catch-up.
4. Offline → sửa và lưu local → reconnect xác thực lại → kéo sự kiện → gửi operation cũ với cùng ID → conflict giữ cả base/local/server → merge và publish mới.
5. Chọn chia sẻ example/report → preview làm sạch → publish → người khác mở được; file riêng không bị tự upload.
6. Backup DB+attachment+version manifest → restore server sạch → so số lượng, revision, quyền và hash → client reconnect có định danh workspace rõ.

## Ranh giới

Request tới API đích đi từ desktop. Localhost của A khác localhost của B. Máy chủ cộng tác không tự proxy token, cookie và response. Không tự tạo tunnel. MongoDB chỉ được backend truy cập; không phân phối MongoDB credentials trong desktop.

## Phạm vi đầy đủ

Toàn bộ 160 yêu cầu gốc được giữ ở `modules/`, `requirements.json` và `source-sections/`. A = lõi dùng cho nhóm; B = mở rộng; C = nâng cao. Thứ tự là kế hoạch giao hàng, không phải xóa backlog. Hành vi cụ thể của từng mã trong nguồn vẫn có hiệu lực, trừ thay thế công nghệ đã ghi tại 00-authoritative-decisions.md.

## Tiêu chí hoàn thành

Mỗi yêu cầu có code thật, kiểm thử thích hợp, ánh xạ screen/state, bằng chứng chạy và trạng thái. "Đã vẽ UI", "TODO", nút disabled, mock hardcoded, mã chỉ compile Linux và test bỏ qua không đồng nghĩa hoàn thành Windows app. Những hạng mục đang chờ thông tin/credentials phải ghi blocked với phần độc lập đã làm, không báo green.

## Yêu cầu bổ sung do kiến trúc mới

- MongoDB replica set readiness + transaction commit/rollback/retry tests trước mọi mốc cộng tác.
- Không có driver MongoDB trên desktop production, không query DB trực tiếp từ UI.
- Local journal phải có atomic batch cho draft+outbox, versioned format, checksum, crash recovery, lock tiến trình, compaction và migration; nếu không đạt, ghi ADR chọn phương án bền vững khác phù hợp xác nhận DB.
- Native renderer/editor phải vượt bài thử IME tiếng Việt, phím tắt, DPI, accessibility, nội dung lớn và bộ nhớ; chỉ tuyên bố hiệu năng qua đo release build Windows.
