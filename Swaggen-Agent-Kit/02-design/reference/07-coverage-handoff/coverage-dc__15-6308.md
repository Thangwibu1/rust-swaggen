# Coverage / DC

- Node: `15:6308`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6308
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- DC / REQUIREMENT COVERAGE
- DC01
- Tài liệu tự sinh
- Stage A
- D06
- DC02
- Chi tiết hợp đồng
- Stage A
- D06
- DC03
- Chạy từ tài liệu
- Stage A
- D06 · D07
- DC04
- Liên kết định danh
- Stage A
- D03 · D09
- DC05
- Sinh request mẫu
- Stage A
- D06 · D07
- DC06
- Cập nhật từ đặc tả
- Stage A
- D08 · D09
- DC07
- Đề xuất sửa hợp đồng
- Stage B
- D08 · C13
- DC08
- Kiểm tra response theo schema
- Stage B
- D10 · T05
- DC09
- Chênh lệch hợp đồng
- Stage B
- D08 · D09 · D12
- DC10
- Xuất tài liệu
- Stage B
- D11

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
