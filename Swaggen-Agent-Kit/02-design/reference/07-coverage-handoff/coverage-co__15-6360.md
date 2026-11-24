# Coverage / CO

- Node: `15:6360`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6360
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- CO / REQUIREMENT COVERAGE
- CO01
- Workspace chung
- Stage A
- A11 · C02
- CO02
- Publish thay đổi
- Stage A
- C05
- CO03
- Thông báo thay đổi
- Stage A
- C03
- CO04
- Bình luận theo đối tượng
- Stage A
- C04
- CO05
- Nhắc thành viên
- Stage B
- C03 · C04
- CO06
- Luồng hoạt động
- Stage A
- A11 · C03
- CO07
- Trạng thái chỉnh sửa
- Stage B
- C14
- CO08
- Chia sẻ kết quả chạy
- Stage B
- R17 · T04
- CO09
- Liên kết nội bộ
- Stage B
- C04
- CO10
- Đồng biên tập
- Stage C
- C14

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
