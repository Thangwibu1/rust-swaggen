# Coverage / OA

- Node: `15:6256`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6256
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- OA / REQUIREMENT COVERAGE
- OA01
- Import đặc tả
- Stage A
- D01 · I01
- OA02
- Editor đặc tả
- Stage A
- D01
- OA03
- Kiểm tra cấu trúc
- Stage A
- D02
- OA04
- Quản lý endpoint
- Stage A
- D01 · D03 · D12
- OA05
- Định nghĩa dữ liệu
- Stage A
- D01 · D03 · D04
- OA06
- Components dùng lại
- Stage A
- D01 · D04 · D05
- OA07
- Schema phức tạp
- Stage B
- D02 · D04 · D05
- OA08
- Tham chiếu nhiều file
- Stage B
- D02 · D05
- OA09
- Swagger 2 và phiên bản mới
- Stage B
- D02
- OA10
- Lint quy ước nhóm
- Stage B
- D10

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
