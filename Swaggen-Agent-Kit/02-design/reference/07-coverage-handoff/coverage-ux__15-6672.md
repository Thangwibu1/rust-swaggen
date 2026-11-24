# Coverage / UX

- Node: `15:6672`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6672
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- UX / REQUIREMENT COVERAGE
- UX01
- Bố cục làm việc
- Stage A
- R01 · A11
- UX02
- Tab và khôi phục
- Stage A
- X01 · X04
- UX03
- Phím tắt
- Stage A
- S04 · M03
- UX04
- Command palette
- Stage B
- A13 · S04 · M03
- UX05
- Hiển thị Windows
- Stage A
- S03
- UX06
- Theme và ngôn ngữ
- Stage B
- S03
- UX07
- Khả năng truy cập
- Stage B
- S03
- UX08
- Mở file và liên kết
- Stage B
- I01 · C04
- UX09
- Trạng thái kết nối
- Stage A
- A06 · C08
- UX10
- Quản lý dữ liệu máy
- Stage A
- S05 · X05

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
