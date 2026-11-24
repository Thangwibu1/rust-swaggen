# SY — Yêu cầu nguồn

## SY01 · A · Lưu cục bộ trước

Ghi nháp và thao tác chờ bằng transaction SQLite; crash không được làm mất thay đổi đã báo lưu thành công.

## SY02 · A · Hàng đợi đồng bộ

Có pending syncing synced conflict failed; cho retry và xem nguyên nhân, không dùng một biểu tượng xanh mơ hồ.

## SY03 · A · Phiên bản đối tượng

Publish gửi base revision; server chỉ chấp nhận khi revision hợp lệ và trả conflict nếu dữ liệu đã đổi.

## SY04 · A · Giải quyết xung đột

Hiện bản gốc bản của tôi và bản server; chọn giữ hoặc ghép, preview rồi publish lại; giữ bản thua làm bản nháp.

## SY05 · A · Sự kiện lấy bù

Client giữ cursor; reconnect lấy thay đổi bỏ lỡ, cursor hết hạn thì lấy snapshot rồi áp lại thao tác còn chờ.

## SY06 · A · Chống nhân đôi

Mỗi thao tác có idempotency key; retry hoặc mất ACK không tạo request comment hay revision trùng.

## SY07 · A · Xóa và sửa đồng thời

Tombstone cho xóa; không tự hồi sinh dữ liệu khi thiết bị offline đẩy bản sửa cũ lên.

## SY08 · A · Lịch sử phiên bản

Xem diff tác giả thời gian và ghi chú; restore bằng revision mới, không xóa lịch sử đã công bố.

## SY09 · A · Thu hồi quyền khi offline

Server từ chối publish sau khi bị gỡ quyền; cho giữ nháp riêng. Không hứa xóa được mọi bản sao đã tải xuống máy.

## SY10 · B · Nhánh và review

Fork hoặc branch dự án, gửi đề nghị hợp nhất, review và kiểm tra xung đột trước merge.
