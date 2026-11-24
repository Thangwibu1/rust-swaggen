# Coverage / PR

- Node: `15:6620`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6620
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- PR / REQUIREMENT COVERAGE
- PR01
- GraphQL cơ bản
- Stage B
- P01
- PR02
- GraphQL schema
- Stage C
- P01
- PR03
- WebSocket
- Stage C
- P02
- PR04
- Server Sent Events
- Stage B
- R11
- PR05
- gRPC
- Stage C
- P03
- PR06
- HTTP nâng cao
- Stage C
- R13
- PR07
- Sinh code gọi API
- Stage B
- R18
- PR08
- Sinh SDK và stub
- Stage C
- P04
- PR09
- Giải mã và chuyển đổi
- Stage B
- P04
- PR10
- Extension có quyền
- Stage C
- S11

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
