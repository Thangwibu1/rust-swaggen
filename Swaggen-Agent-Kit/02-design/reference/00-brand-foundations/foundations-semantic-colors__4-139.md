# Foundations / Semantic colors

- Node: `4:139`
- Page: `00-brand-foundations`
- Size: 1120 × 1122
- Figma: https://www.figma.com/design/MmPG1xzCaYtPb9HgZxsTno/Swaggen?node-id=4-139
- Requirements (annotation): Chưa có mã trong annotation; cần đối chiếu nghiệp vụ.

## Bằng chứng

Đây là metadata, không phải ảnh render. Chỉ xem PNG/SVG cùng tên là hình gốc nếu file thực sự tồn tại và có trong export-results.json. Tên layer TEXT có thể bị rút gọn; INSTANCE không chứa toàn bộ giá trị text/property trong metadata. Không suy ra màu, font size hay trạng thái runtime từ tọa độ.

## Text layer names

- 02 / SEMANTIC COLOR ROLES
- bg/page
- #FBFCFA
- bg/surface
- #FFFFFF
- bg/sidebar
- #FFEFD2
- bg/selected
- #E8F4F0
- bg/brand
- #6AB0AF
- bg/brand-strong
- #245C59
- bg/danger
- #FFF0EC
- bg/warning
- #FFF2A5
- bg/disabled
- #EDF0ED
- text/primary
- #203735
- text/secondary
- #607572
- text/brand
- #245C59
- text/on-brand
- #FFFFFF
- text/danger
- #A23F3F
- text/warning
- #7B5B1C
- border/default
- #DCE6E1
- border/focus
- #245C59
- status/success
- #286649
- status/info
- #345D8E
- accent/salmon
- #F5A89A
- accent/yellow
- #FFF2A5

## Hành vi cần hiện thực

Đối chiếu mã yêu cầu, xử lý success/loading/empty/error/offline/permission khi áp dụng; dùng domain thật, không hardcode dữ liệu mẫu Figma làm kết quả backend. Màn hình xuất hiện trong Figma không chứng minh tính năng đã được triển khai.
