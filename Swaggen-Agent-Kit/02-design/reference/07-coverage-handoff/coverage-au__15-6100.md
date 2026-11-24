# Coverage / AU

- Node: `15:6100`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6100
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- AU / REQUIREMENT COVERAGE
- AU01
- No Auth và Bearer
- Stage A
- R05
- AU02
- Basic và API key
- Stage A
- R05
- AU03
- Kế thừa xác thực
- Stage A
- R05
- AU04
- Cookie jar
- Stage A
- R12
- AU05
- TLS mặc định
- Stage A
- R13
- AU06
- Redirect
- Stage A
- R13
- AU07
- Proxy và CA riêng
- Stage B
- R13
- AU08
- OAuth 2 PKCE
- Stage B
- R06
- AU09
- Chứng chỉ client
- Stage B
- R13
- AU10
- Xác thực bổ sung
- Stage C
- R13

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
