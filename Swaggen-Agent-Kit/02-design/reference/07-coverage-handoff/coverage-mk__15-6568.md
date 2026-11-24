# Coverage / MK

- Node: `15:6568`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6568
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- MK / REQUIREMENT COVERAGE
- MK01
- Mock từ example
- Stage B
- T07 · T08
- MK02
- Mock từ schema
- Stage B
- T07 · T08
- MK03
- Kịch bản mock
- Stage C
- T08 · T09
- MK04
- Mock dùng chung
- Stage B
- T07 · T08
- MK05
- CLI Rust
- Stage B
- T10
- MK06
- Kết quả cho CI
- Stage B
- T04 · T10
- MK07
- Runner tự host
- Stage C
- T11
- MK08
- Lịch chạy kiểm tra
- Stage C
- T12
- MK09
- Thông báo lỗi chạy
- Stage C
- T12
- MK10
- Quản lý tài nguyên
- Stage B
- T11

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
