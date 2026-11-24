# Bổ sung toàn bộ render bằng Figma Desktop

Công cụ development plugin đọc và xuất file đang mở bằng API native Figma. Không cần Figma API token, không thực hiện API MCP, không có network và không thay đổi canvas. Đây là một lựa chọn xuất file thông thường của chủ file, không thay đổi gói/quyền truy cập.

## Cách chạy

1. Mở Swaggen trong Figma Desktop có WebGL và quyền đọc/export file.
2. Mở Plugins → Development → Import plugin from manifest… và chọn `manifest.json` trong thư mục này. Nếu Figma yêu cầu ID development plugin hợp lệ, tạo plugin local mới từ giao diện Figma, giữ ID Figma tạo rồi thay nội dung manifest/code/UI bằng các file kit (giữ ID đó).
3. Chạy Swaggen Local Bulk Export và bấm **Xuất toàn bộ các trang**.
4. Đợi kết thúc; tải **từng ZIP** được liệt kê (mỗi trang một ZIP, kèm ZIP report). Không đóng plugin trước khi tải các ZIP.
5. Giải nén tất cả vào cùng thư mục `Swaggen-Agent-Kit`, giữ nguyên đường dẫn `02-design/...`; không đặt thêm tầng thư mục lặp. Các file `.native.json` bổ sung metadata chi tiết, không ghi đè JSON nguồn.
6. Chạy `python tools/validate_kit.py --require-all-images`. Chỉ khi inventory không còn thiếu hình và native report không có lỗi/cancelled mới coi việc xuất đầy đủ. Nếu file Figma đã thay đổi, đối chiếu node ID và bổ sung inventory cho node mới từ native manifest.

Các trang mang tên tiếng Anh được chuẩn hóa về slug. Nếu slash/ampersand khác tên trong snapshot, đối chiếu theo node ID; validator tìm theo node ID ngay cả khi slug đã đổi. Ảnh PNG 1× từng frame, SVG cho component/variant; chi tiết gồm text/font/layout/paints và một phần property binding. Không có đầy đủ prototype hoặc binary .fig backup; muốn backup editable file thì dùng Save local copy của Figma riêng.

## Phạm vi và giới hạn

Plugin xuất các frame/group/component/set ở cấp trang hoặc bên trong section và toàn bộ main component/variants, không xuất mỗi frame con của màn hình thành screen riêng. Với snapshot hiện tại, danh mục đích là 522 node. UI đóng gói ZIP STORE không nén, giới hạn 256MiB mỗi trang; nếu vượt thì dừng có nhãn partial. Tên tiếng Việt có thể được rút thành slug ASCII nhưng node ID vẫn giữ duy nhất. Mỗi trang tạo archive độc lập để giảm bộ nhớ.

Plugin đã kiểm tra syntax, ZIP CRC/UTF-8 roundtrip và luồng export bằng mô phỏng API dựa trên metadata. **Chưa chạy trong Figma Desktop thật** do môi trường hiện tại không có WebGL và MCP đã chạm quota; cần smoke test một file nhỏ trước khi xuất toàn bộ. Không coi test mô phỏng là native integration pass. Manifest/API có thể cần điều chỉnh theo phiên bản Figma tại thời điểm chạy.
