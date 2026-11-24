# Coverage / OP

- Node: `15:6776`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6776
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- OP / REQUIREMENT COVERAGE
- OP01
- Bộ cài Windows
- Stage A
- S10
- OP02
- Chế độ không có cloud
- Stage A
- A08 · C08
- OP03
- Khởi tạo trên Coolify
- Stage A
- A05 · S08
- OP04
- Cấu hình VPS Coolify
- Stage A
- A05 · S08
- OP05
- Sao lưu MongoDB và file
- Stage A
- S09
- OP06
- Khôi phục dữ liệu
- Stage A
- S09
- OP07
- Nâng cấp và migration
- Stage A
- S10 · X06
- OP08
- Phân phối cập nhật
- Stage B
- S10
- OP09
- Theo dõi sức khỏe
- Stage A
- S08
- OP10
- Quota vận hành
- Stage B
- S08

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
