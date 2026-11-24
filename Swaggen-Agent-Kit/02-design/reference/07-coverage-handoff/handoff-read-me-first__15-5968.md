# Handoff / Read me first

- Node: `15:5968`
- Page: `07-coverage-handoff`
- Size: 1440 × 750
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-5968
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- DESIGN HANDOFF / SWAGGEN
- 108 desktop screens & states · 160 requirement references
- Start here → 00 Brand & foundations → 01 Component library →
- Desktop only / 1440 × 900 / Light theme / JetBrains Mono
- Google-only authentication supersedes local passwords and re
- A = initial team release · B = extended workflows · C = adva
- Backend: Rust + MongoDB on the existing Coolify VPS. Desktop
- Prototype flows: Access, Workbench, API design + collaborati
- Reuse component instances and semantic variables. Do not det
- Assets: editable Swaggen vector mark; Lucide icons (ISC), Go

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
