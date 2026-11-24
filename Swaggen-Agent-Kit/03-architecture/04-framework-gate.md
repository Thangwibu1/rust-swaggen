# Gate chọn Rust UI và dependency

## Mặc định có điều kiện

Thử **Iced** cho frontend Rust thuần và **Axum + Tokio** cho backend. Đây là đề xuất của kit, chưa phải framework do chủ dự án chốt. Không lấy nhãn "native Rust" làm bằng chứng UI nhanh hoặc ít RAM. egui/eframe là ứng viên so sánh nếu Iced không qua gate. Slint có DSL riêng; chỉ chọn sau khi làm rõ điều đó và nghĩa vụ license.

## Prototype bắt buộc trước khi dựng toàn bộ UI

Reproduce shell 1440×900, sidebar 1.000 request, request editor, response 20MB stream-to-file, OpenAPI editor multiline/selection/undo/search/highlight/folding hoặc fallback được mô tả rõ, split panes, modal focus trap. Test tiếng Việt IME, copy/paste, Ctrl+Enter/Ctrl+S/Ctrl+P, Windows Narrator, tab order, 100/125/150/200% DPI, hai màn hình, resize và GPU fallback. Measure release build Windows CPU idle, frame/p95 latency, working set/private bytes, startup.

Chấm từng ứng viên bằng bằng chứng; ưu tiên editor/IME/accessibility và Figma fidelity trước số widget demo. Native code editor có thể cần thư viện riêng: phải xác minh maintenance/API/license, không lén nhúng Monaco/browser để lách ràng buộc. Nếu không ứng viên nào đáp ứng, ghi blocker và các lựa chọn cụ thể; không tự thay frontend sang web.

## Hồ sơ dependency bắt buộc

Mỗi crate/SDK/tool có: tên + registry/repository chính chủ + exact version + release date + stable/dev status + license + MSRV + targets + feature flags + transitive native requirements + known advisories + maintenance + purpose + alternative + smoke test. Pin rust-toolchain.toml, Cargo.lock (application), container digest, generator/runtime SDK version. Chỉ cập nhật khi có report compatibility. Không trộn ví dụ docs dev với stable crate: tại lần kiểm tra này docs.iced.rs hiển thị dev trong khi docs.rs/iced/latest hiển thị stable khác.

Danh sách ứng viên xem 08-research/dependency-candidates.md. Phiên bản không được coi là đã cài hoặc compile chỉ vì đọc docs.
