# H25 / Proxy authentication required

- Node: `29:3842`
- Page: `10-authentication-secrets`
- Size: 1440 × 900
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=29-3842
- Requirements (annotation): AU07, RS05

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- Proxy authentication required
- DisNote API / auth / H25
- Action required
- Proxy
- proxy.team.local:3128
- Credential source
- Windows secure storage

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
