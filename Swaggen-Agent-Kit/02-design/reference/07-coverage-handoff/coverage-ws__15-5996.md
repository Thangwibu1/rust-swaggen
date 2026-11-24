# Coverage / WS

- Node: `15:5996`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-5996
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- WS / REQUIREMENT COVERAGE
- WS01
- Chế độ cá nhân
- Stage A
- A07 · A08
- WS02
- Kết nối server
- Stage A
- A05 · A06
- WS03
- Tài khoản nội bộ
- Stage A
- A01 · A02 · A03 · A04 · S02 · M06
- WS04
- Quản lý workspace
- Stage A
- A07 · A08 · A09 · A11
- WS05
- Thành viên và vai trò
- Stage A
- A10 · C01 · C02 · X03
- WS06
- Collection và folder
- Stage A
- A12 · M01
- WS07
- Tìm kiếm và nhãn
- Stage A
- A13 · X02
- WS08
- Thiết lập dự án
- Stage A
- A07 · A09 · S01
- WS09
- Quản lý vòng đời
- Stage B
- A08 · S01 · X07 · M04
- WS10
- Chuyển chủ sở hữu
- Stage B
- S01 · X08

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
