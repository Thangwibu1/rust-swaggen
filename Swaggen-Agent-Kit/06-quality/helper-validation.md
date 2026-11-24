# Kiểm tra bộ tài liệu và helper

Đã chạy trong môi trường tạo kit:

- PRD: 949 XML text runs của DOCX đều có trong Markdown sau normalize whitespace; 0 run thiếu; giữ 24 bảng, 160 mã, 12 AC.
- Traceability: đủ 160 mã duy nhất, mỗi mã có phase chính và ít nhất một screen annotation; 316 screen metadata; 13 phase P00–P12.
- Assets: 59 PNG mở/verify bằng Pillow, 125 SVG parse XML; hash/kích thước lưu trong asset-integrity.json.
- Exporter: Node syntax code.js và cả script blocks trong ui.html; ZIP CRC/UTF-8/binary/empty roundtrip qua Python zipfile; mock native export dựa trên 15 trang metadata thu đủ 522 target, 0 lỗi.
- Validator: chế độ docs pass; strict image gate cố ý fail do thiếu 464 render.

Chưa chạy: PowerShell doctor trên Windows, native Figma plugin integration, SDK/crate build của ứng dụng, Windows app, server, MongoDB transaction, Coolify deployment hoặc benchmark. Những test đó thuộc P00–P12, không có application code trong kit này.
