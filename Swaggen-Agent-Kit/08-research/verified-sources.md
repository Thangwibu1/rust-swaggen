# Nguồn chính thức đã kiểm tra

Ngày nghiên cứu: 23-11-2026. Các URL /latest/ và /current/ có thể thay đổi; AI phải mở lại, chốt phiên bản và chạy smoke test trước triển khai. Đây là kiểm tra tài liệu, chưa cài SDK hoặc build ứng dụng.

| Nguồn | Kết quả áp dụng |
| --- | --- |
| https://docs.rs/iced/latest/iced/ | Snapshot đọc: 0.14.0; ứng viên UI Rust, không phải chứng nhận production suitability |
| https://docs.iced.rs/iced/ | Snapshot đọc có 0.15.0-dev; không trộn API dev vào dependency stable |
| https://docs.rs/egui/latest/egui/ | Ứng viên native Rust thay thế; cần đo UI/editor/IME/accessibility |
| https://slint.dev/ | Framework Rust ecosystem có ngôn ngữ UI khai báo; cần xem DSL/license trước chọn |
| https://github.com/slint-ui/slint/blob/master/LICENSE.md | Nguồn license, không suy ra mọi use case đều không có điều kiện |
| https://docs.rs/axum/latest/axum/ | Framework backend gắn Tokio/Hyper; Axum là đề xuất |
| https://docs.rs/mongodb/latest/mongodb/ | Driver chính thức; async Tokio, requirements/features và cảnh báo cancellation cần đọc theo phiên bản |
| https://www.mongodb.com/docs/drivers/rust/current/crud/transactions/ | ClientSession cùng Client, session phải truyền vào từng operation; không operation song song trong một transaction |
| https://www.mongodb.com/docs/manual/core/transactions/ | Single-document atomicity và multi-document transaction là hai mức khác nhau |
| https://www.mongodb.com/docs/manual/core/transactions-production-consideration/ | Standalone không hỗ trợ transaction; giới hạn transaction/runtime/BSON cần kiểm tra |
| https://www.mongodb.com/docs/manual/tutorial/deploy-replica-set/ | Production replica-set topology, hostnames và failure domains |
| https://coolify.io/docs/databases/mongodb | Resource MongoDB mặc định được mô tả là standalone; không đủ để khẳng định transaction sẵn sàng |
| https://coolify.io/docs/docs/applications/builds/docker-compose | Compose từ Git, context/base directory, network/volumes/healthchecks |
| https://rust-lang.org/tools/install/ | Rustup/toolchain chính thức |
| https://rust-lang.github.io/rustup/installation/windows-msvc.html | Windows MSVC linker và SDK prerequisites |
| https://developers.figma.com/docs/plugins/api/properties/nodes-exportasync/ | Cơ chế export native plugin; công cụ bulk export trong kit dùng PNG/SVG |

Không áp dụng số RAM marketing của framework cho toàn bộ ứng dụng. Không sao chép code docs theo phiên bản khác dependency lock.
