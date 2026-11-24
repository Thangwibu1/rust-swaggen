# Completion review / Flow directory

- Node: `33:34`
- Page: `07-coverage-handoff`
- Size: 1440 × 1406
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=33-34
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- SWAGGEN / COMPLETE DESKTOP DESIGN
- 316 screens and UI states · 160 mapped requirements · Windows desktop
- 208 detailed screens supplement the 108 core screens. Reusable controls remain in the component library. Application sign-in uses Google only; API credentials are configured separately.
- 08 • Workspace & collection details / 22 screens
- Collections, folders and team administration
- 09 • Request & response states / 35 screens
- Request bodies, response formats and errors
- 10 • Authentication & secrets / 35 screens
- API authorization, TLS, token lifecycle and variables
- 11 • OpenAPI & contract details / 25 screens
- Schema composition, references and contract changes
- 12 • Collaboration & sync details / 25 screens
- Branch review, merge conflicts and offline recovery
- 13 • Import, testing & mock details / 34 screens
- Import compatibility, testing, mock and scheduled jobs
- 14 • Protocols & system details / 32 screens
- Protocols, Windows, MongoDB/Coolify and operations
- Scope overrides
- WS03 / Google-only application sign-in (supersedes internal passwords)
- SC03 / Google OAuth sign-in and revocable sessions
- SC09 / Account recovery and MFA managed by Google
- UX06 / White interface and language preference (supersedes dark mode)
- Performance acceptance
- Z16 records benchmark gates, not measured results. Validate cold/warm startup, memory, UI latency, streaming, sync and soak behavior on a defined Windows machine before release.
- Prototype scope
- Each detail page has an entry point. Primary buttons connect representative state transitions; Back returns to the previous frame. This design is not an executable application.

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
