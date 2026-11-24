# Coverage / TS

- Node: `15:6516`
- Page: `07-coverage-handoff`
- Size: 1440 × 716
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=15-6516
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- TS / REQUIREMENT COVERAGE
- TS01
- Assertion cơ bản
- Stage A
- T01
- TS02
- Collection runner
- Stage B
- T02
- TS03
- Chuỗi phụ thuộc
- Stage B
- T02
- TS04
- Chạy theo dữ liệu
- Stage B
- T03
- TS05
- Báo cáo kết quả
- Stage B
- T04
- TS06
- Kiểm thử hợp đồng
- Stage B
- T05
- TS07
- Pre request và test script
- Stage C
- T06
- TS08
- Tương thích script Postman
- Stage C
- T06 · I02
- TS09
- Chạy song song có giới hạn
- Stage C
- T02
- TS10
- Kết quả tái hiện
- Stage B
- T04

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
