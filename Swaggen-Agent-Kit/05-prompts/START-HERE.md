# Prompt ngắn để bắt đầu

Giải nén toàn bộ kit vào repository/thư mục dự án mà AI agent có quyền đọc. Giữ `00-source` đến `08-research`, `tools`, `AGENTS.md`. Sau đó gửi:

```text
Hãy đọc AGENTS.md và 05-prompts/MASTER-PROMPT.md trong thư mục Swaggen-Agent-Kit.
Thực thi toàn bộ yêu cầu của master prompt dựa trên tài liệu và thiết kế đi kèm.
Bắt đầu P00: kiểm tra framework Rust, crates, SDK, toolchain Windows và MongoDB transaction trước scaffold sản phẩm.
Sau khi các gate tương ứng qua, hiện thực P01 đến P12 với code thật, kiểm thử, bằng chứng Windows và báo cáo từng phase.
Không dừng ở UI demo/MVP; không đổi sang frontend web hay database SQL.
Nếu thiếu môi trường/quyền/credentials, ghi blocker cụ thể, tiếp tục phần độc lập và không báo pass giả.
Đọc export-status.md trước khi nhận định đủ ảnh Figma; không coi metadata là screenshot.
```

Nếu tool của agent không đọc được filesystem đính kèm, cung cấp ZIP và yêu cầu nó giải nén trước. Nếu agent chỉ nhận văn bản, copy toàn bộ MASTER-PROMPT nhưng vẫn cần tải tài liệu vào môi trường của agent; prompt không tự cấp quyền truy cập Figma/VPS hay Windows.

## Sử dụng theo từng phase

Sau master prompt, có thể đưa `P00-execute.md`, rồi `P01-execute.md` ... `P12-execute.md` để kiểm soát phiên làm việc. Mỗi prompt kế thừa master và yêu cầu thật, không cần copy cả 316 màn hình vào chat. Đừng khởi chạy nhiều phiên ghi cùng source thiếu kiểm soát.

## Tiếp tục sau gián đoạn

Dùng RESUME-PROMPT.md và checkpoint/evidence mới nhất, không yêu cầu agent làm lại từ đầu. Dùng AUDIT-PROMPT.md trước khi chấp nhận release.
