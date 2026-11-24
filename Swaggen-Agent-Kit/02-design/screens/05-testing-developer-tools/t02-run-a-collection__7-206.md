# T02 / Run a collection

- Node: `7:206`
- Page: `05-testing-developer-tools`
- Size: 1440 × 900
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=7-206
- Requirements (annotation): TS02, TS03, TS09

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- Run a collection
- DisNote API / Developer tools
- Configuration
- Collection
- Workspace configuration
- Environment
- Workspace configuration
- Iterations
- Workspace configuration
- Delay between requests
- Workspace configuration
- On failure
- Workspace configuration
- Access & sharing
- Changes are saved locally before they are published to your team.

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
