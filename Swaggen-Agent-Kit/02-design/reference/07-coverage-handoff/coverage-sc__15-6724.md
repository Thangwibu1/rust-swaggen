# Coverage / SC

- Node: `15:6724`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6724
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- SC / REQUIREMENT COVERAGE
- SC01
- Kho bảo mật Windows
- Stage A
- E03 · S12
- SC02
- Làm sạch dữ liệu
- Stage A
- S06 · M05
- SC03
- Đăng nhập máy chủ
- Stage A
- A01 · A02 · A03 · A04 · S02 · M06
- SC04
- Cách ly workspace
- Stage A
- C01 · C09 · S07 · X03
- SC05
- Nội dung không tin cậy
- Stage A
- S06
- SC06
- Quyền ứng dụng desktop
- Stage A
- S06 · S11 · S13
- SC07
- Không tự gửi telemetry
- Stage A
- S06
- SC08
- Chặn SSRF trên server
- Stage B
- S13
- SC09
- Phục hồi tài khoản và MFA
- Stage B
- S02
- SC10
- Audit và giới hạn lưu
- Stage B
- S07

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
