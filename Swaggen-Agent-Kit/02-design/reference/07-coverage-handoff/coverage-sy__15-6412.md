# Coverage / SY

- Node: `15:6412`
- Page: `07-coverage-handoff`
- Size: 1440 × 736
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6412
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- SY / REQUIREMENT COVERAGE
- SY01
- Lưu cục bộ trước
- Stage A
- C07 · C08 · X04
- SY02
- Hàng đợi đồng bộ
- Stage A
- C07 · C08
- SY03
- Phiên bản đối tượng
- Stage A
- C05 · C06
- SY04
- Giải quyết xung đột
- Stage A
- C06
- SY05
- Sự kiện lấy bù
- Stage A
- C07 · X06
- SY06
- Chống nhân đôi
- Stage A
- C05 · C07
- SY07
- Xóa và sửa đồng thời
- Stage A
- C06 · X07 · M04
- SY08
- Lịch sử phiên bản
- Stage A
- C10 · C11
- SY09
- Thu hồi quyền khi offline
- Stage A
- C09
- SY10
- Nhánh và review
- Stage B
- C12 · C13 · I04

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
