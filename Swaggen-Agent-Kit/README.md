# Swaggen — AI Agent Implementation Kit

Bộ bàn giao để AI agent xây Swaggen từ đầu: Windows desktop Rust native + backend Rust + MongoDB transactions + Coolify VPS. Chưa có application code; đây là đặc tả, nguồn thiết kế, phase, prompt và helper.

**Bắt đầu:** đọc [05-prompts/START-HERE.md](05-prompts/START-HERE.md), rồi đưa [MASTER-PROMPT.md](05-prompts/MASTER-PROMPT.md) cho AI agent có quyền đọc toàn bộ thư mục.

**Giới hạn quan trọng:** việc tải toàn bộ ảnh chưa hoàn thành do quota Figma MCP và fallback browser thiếu WebGL. Có metadata đủ 316 screen/state, nhưng chỉ 58/522 target có PNG riêng; 464 còn thiếu. Xem [export-status.md](02-design/export-status.md). Có bulk exporter native để chủ file bổ sung. Không coi metadata là ảnh screenshot.

## Thư mục

| Folder | Nội dung |
| --- | --- |
| 00-source | DOCX gốc, Markdown chuyển đầy đủ, provenance |
| 01-prd | Quyết định hiệu lực, PRD chuẩn hóa, 24 section nguồn + mở đầu, 16 module và 160 requirement JSON |
| 02-design | 15 XML page, 316 screen metadata, component/variant metadata, PNG/SVG thực có, missing-assets và integrity manifest |
| 03-architecture | Native/server/storage boundary, MongoDB schema/transaction, sync contract, framework gate |
| 04-phases | 13 phase P00–P12, dependency/gates, requirement→phase→screen traceability |
| 05-prompts | Master, prompt bắt đầu, prompt từng phase, tiếp tục và kiểm toán |
| 06-quality | AC01–AC12, performance/security/DoD, kit/helper validation |
| 07-operations | Coolify, replica set, backup/restore, migration, Windows packaging |
| 08-research | Nguồn chính thức đã kiểm tra, dependency/SDK candidates và gate |
| tools | Validator Python, doctor PowerShell, native Figma bulk exporter và test mô phỏng |

## Cách dùng

1. Giải nén toàn bộ vào một thư mục dự án. Giữ cấu trúc và original sources; mở thư mục bằng AI coding agent.
2. Đọc decisions: frontend/backend Rust; MongoDB server; framework tên cụ thể và local persistence vẫn là đề xuất phải qua prototype. Không dùng Tauri/React/PostgreSQL trong nguồn cũ làm stack mặc định.
3. Bổ sung ảnh bằng native Figma exporter nếu cần đối chiếu toàn bộ visual. Có thể bắt đầu research và thiết kế domain từ metadata/PRD, nhưng không báo visual parity đầy đủ khi chưa có ảnh/native validation.
4. Gửi prompt trong START-HERE; agent thực hiện P00 trước, kiểm tra crates/SDK/toolchain và transaction rồi mới scaffold.
5. Theo dõi reports và traceability; P06 là release A, P09 release B, P12 kiểm chứng toàn bộ A+B+C. Tiến độ không được tính bằng số screen dựng xong.

## Quy mô và kiểm chứng

160 yêu cầu = 93 A + 50 B + 17 C. 12 acceptance criteria nguồn. Tất cả 160 có mapping screen và phase. Đã kiểm tra không thiếu 949 đoạn text run khi chuyển DOCX và 24 bảng. Framework/SDK của app chưa cài/build: bắt buộc agent kiểm tra lại, không hiểu nhầm việc đọc docs là runtime pass.

## Kiểm tra kit

```text
python tools/validate_kit.py
python tools/validate_kit.py --require-all-images
node tools/tests/test-exporter.cjs
powershell -File tools/doctor.ps1 -ServerTools
```

Python validator dùng standard library. Test mock exporter cần Node và Python; Node chỉ dùng cho helper này, không phải frontend app. PowerShell doctor chỉ đọc và báo cáo, không tự cài phần mềm; exit 2 khi có prerequisite/gate chưa được kiểm chứng. Đọc helper-validation.md để biết test nào đã chạy và test nào chưa.

Prompt không tự cấp quyền Figma, Windows, ký ứng dụng hoặc VPS. Khi thiếu điều kiện, agent phải báo blocker và tiếp tục phần độc lập; không bịa benchmark, successful deploy hay test pass.
