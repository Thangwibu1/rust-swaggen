# Coverage / RQ

- Node: `15:6048`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6048
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- RQ / REQUIREMENT COVERAGE
- RQ01
- Method và URL
- Stage A
- A14 · R01
- RQ02
- Query chính xác
- Stage A
- R01
- RQ03
- Headers
- Stage A
- R02
- RQ04
- Các loại body
- Stage A
- R03
- RQ05
- Tải file lên
- Stage A
- R04
- RQ06
- Gửi và hủy
- Stage A
- R08 · R11
- RQ07
- Request đã phân giải
- Stage A
- R01 · R10
- RQ08
- Nháp và lưu
- Stage A
- A12 · A14
- RQ09
- Sao chép nhanh
- Stage A
- A14 · R14
- RQ10
- Stream khi truyền
- Stage B
- R04 · R09

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
