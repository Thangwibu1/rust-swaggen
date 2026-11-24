# Coverage / IM

- Node: `15:6464`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6464
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- IM / REQUIREMENT COVERAGE
- IM01
- Định dạng project mở
- Stage A
- I01 · I03
- IM02
- Postman Collection
- Stage A
- I01 · I02
- IM03
- Postman Environment
- Stage A
- I01
- IM04
- OpenAPI import export
- Stage A
- D11 · I01
- IM05
- cURL hai chiều
- Stage A
- I01
- IM06
- Xem trước import
- Stage A
- I02
- IM07
- File đính kèm
- Stage B
- I03
- IM08
- Xuất dữ liệu đã làm sạch
- Stage A
- I03 · M05
- IM09
- Tích hợp Git
- Stage B
- I04
- IM10
- Định dạng bổ sung
- Stage C
- I01

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
