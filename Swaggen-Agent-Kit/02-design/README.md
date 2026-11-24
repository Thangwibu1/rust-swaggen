# Bộ nguồn thiết kế

- `metadata/`: XML của toàn bộ 15 trang Figma, lấy trực tiếp từ file Swaggen.
- `node-inventory.json`: 413 node cấp trang + 109 component variants = 522 mục tiêu xuất.
- `screens/`: 316 màn hình/trạng thái/dialog, mỗi node có Markdown + JSON mô tả cấu trúc và liên kết nguồn.
- `components/`: thư viện component, icon, shell và variants; hình/vector chỉ có khi file thực sự tồn tại.
- `reference/`: brand/foundations/coverage/handoff, tách khỏi màn hình sản phẩm.
- `screen-requirement-map.json`: mã yêu cầu theo annotation Figma, không phải bằng chứng implementation.
- `export-results.json`, `export-status.md`: kết quả tải thực tế; danh mục không có ảnh không được coi là đã render.
- `palette-labels.json`: nhãn màu đọc được, không phải bản export đầy đủ variable bindings.

## Quy tắc tên

`<category>/<page-number>-<page-slug>/<node-name-slug>__<node-id>.<ext>`. Dùng node ID tránh trùng các variant tên Default/Selected. PNG dùng tham chiếu thị giác; SVG là vector asset; JSON/Markdown phục vụ agent. Giữ liên kết Figma để đối chiếu thay đổi. Không dựng hình giả từ metadata rồi gọi đó là screenshot gốc.

## Độ đầy đủ

Metadata bao gồm IDs, layer names, positions, sizes, một phần text names; không bao gồm đầy đủ fills, fonts, effects, variable bindings, instance overrides, prototype interactions. Đối chiếu nguồn và file render khi cần. Xuất thiếu do quota không cho phép agent bịa chi tiết; dùng bulk exporter trong tools/figma-bulk-export để bổ sung khi chạy được trong Figma desktop.

## Hướng dẫn hiện thực UI

Tái sử dụng Rust component cho Button/Input/Select/Tab/Tree row/Notice/Dialog/Code editor và shell. Không dùng PNG nguyên màn hình làm UI. Triển khai hover/focus/disabled/loading/selected/error từ variants. Lấy kích thước 1440×900 làm baseline, responsive resize theo Windows; không scale canvas cứng. Bề mặt trắng theo note mới `51:40541`; màu foreground theo ý nghĩa. JetBrains Mono là tên font thấy trong foundations; phải lấy file/license chính thức và xác minh fallback hỗ trợ tiếng Việt.
