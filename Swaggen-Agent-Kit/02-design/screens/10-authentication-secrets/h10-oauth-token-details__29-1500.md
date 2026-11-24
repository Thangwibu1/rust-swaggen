# H10 / OAuth token details

- Node: `29:1500`
- Page: `10-authentication-secrets`
- Size: 1440 × 900
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=29-1500
- Requirements (annotation): AU08

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- OAuth token details
- DisNote API / auth / H10
- Access token
- Expires at
- 23 Nov 2026 · 15:00
- Scopes
- tasks:read tasks:write
- Refresh token
- Stored locally · never published
- Auto refresh
- Enabled

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
