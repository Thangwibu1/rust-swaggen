# Coverage / EV

- Node: `15:6152`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6152
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- EV / REQUIREMENT COVERAGE
- EV01
- Môi trường
- Stage A
- E01 · M02
- EV02
- Biến theo cấp
- Stage A
- E01 · E02
- EV03
- Giá trị cá nhân
- Stage A
- E01 · E02
- EV04
- Thay biến
- Stage A
- R10 · E01
- EV05
- Đánh dấu secret
- Stage A
- E02 · E03
- EV06
- Quy tắc xuất
- Stage A
- M05 · I03
- EV07
- Bảo vệ production
- Stage A
- R16 · M02
- EV08
- Biến động
- Stage B
- E02
- EV09
- Trích xuất giá trị
- Stage B
- E02
- EV10
- Vault nhóm
- Stage C
- S12

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
