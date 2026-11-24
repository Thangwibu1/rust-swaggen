# Coverage / RS

- Node: `15:6204`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6204
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- RS / REQUIREMENT COVERAGE
- RS01
- Thông tin tổng quan
- Stage A
- R07
- RS02
- Hiển thị body
- Stage A
- R07
- RS03
- Response headers
- Stage A
- R07
- RS04
- Tìm và sao chép
- Stage A
- R07
- RS05
- Thông báo lỗi
- Stage A
- R08
- RS06
- Response lớn
- Stage A
- R09
- RS07
- Lịch sử riêng
- Stage A
- R14 · S05
- RS08
- Ví dụ chia sẻ
- Stage A
- R17
- RS09
- So sánh response
- Stage B
- R15
- RS10
- Đo thời gian chi tiết
- Stage B
- R15

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
