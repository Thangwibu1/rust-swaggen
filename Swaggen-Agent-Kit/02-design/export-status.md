# Tình trạng export thực tế

**CHƯA ĐỦ TOÀN BỘ ẢNH.** Tài liệu/metadata hoàn thành, nhưng xuất render bị chặn bởi giới hạn Figma MCP trên gói Education của tài khoản kết nối. Không có thay đổi thiết kế nào được thực hiện. Browser fallback mở đúng file nhưng Figma báo không hỗ trợ WebGL; không thể xuất từ giao diện trong môi trường này.

| Hạng mục | Kết quả |
| --- | --- |
| Trang có XML metadata | 15/15 |
| Screen/state/dialog có MD + JSON và source link | 316/316 |
| Node cấp trang đã kiểm kê | 413 |
| Component variants bổ sung | 109 |
| Tổng mục tiêu render | 522 |
| Mục tiêu có PNG riêng thực sự tải về | 58/522 |
| Màn hình sản phẩm có PNG riêng | 1/316 (A01) |
| Brand/foundations có PNG | 6 |
| Node thư viện component có PNG | 51 |
| PNG file | 59 (58 mục tiêu + 1 overview trang 04) |
| SVG asset file | 125 |
| Render còn thiếu | 464 |

Overview trang 04 là một ảnh tổng quan nhiều màn hình; không tính thành 26 ảnh screen riêng và không thay cho render full-size. SVG assets là các vector trả về trong subtree component đã xuất; không tuyên bố đó là toàn bộ component library.

`missing-images.json` liệt kê chính xác từng node còn thiếu, tên, page và đường dẫn dự kiến. `asset-integrity.json` ghi kích thước/hash của từng ảnh/vector thực có. `export-results.json` chỉ ghi lượt export thành công cùng file; không chứa URL tạm. JSON/MD đặt cạnh node không phải ảnh render.

## Cách bổ sung

Dùng `tools/figma-bulk-export/README.md` trong Figma Desktop, tải ZIP của từng trang và report, giải nén vào cùng kit. Plugin native không sửa canvas hay truyền ra mạng; xuất PNG/SVG + metadata chi tiết. Plugin đã qua syntax/control-flow mock và ZIP integrity tests, chưa có native Figma runtime smoke test trong môi trường này.

Sau bổ sung chạy `python tools/validate_kit.py --require-all-images`. Nếu Figma thay đổi, đối chiếu ID với native manifest để cập nhật inventory. Chỉ khi các file thực có, report không cancelled/errors và coverage đầy đủ mới đổi trạng thái export thành complete. Không render ảnh thay thế từ metadata rồi gọi là ảnh gốc.
