# A04 / Your session has expired

- Node: `6:273`
- Page: `02-access-workspaces`
- Size: 1440 × 900
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=6-273
- Requirements (annotation): SC03, WS03

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- swaggen
- Welcome back
- A shared home for your requests, API contracts and team conversat
- YOUR SERVER. YOUR WORKFLOW.
- Session expired
- Continue with your Google account to access your team workspace.
- Google is the only sign-in method.
- Your tokens stay on your device.
- Team server  api.swaggen.team
- By continuing, you agree to your team’s workspace policy.

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
