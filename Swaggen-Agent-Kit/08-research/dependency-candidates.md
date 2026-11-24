# Danh mục dependency cần kiểm chứng ở P00

Tất cả dưới đây là ứng viên, không phải lockfile đã build. AI phải kiểm tra registry/docs chính thức và license/advisory trước thêm vào Cargo.

| Nhóm | Ứng viên / công cụ | Gate |
| --- | --- | --- |
| UI native | iced; egui + eframe dự phòng | Editor, IME, virtual list, rendering, Narrator, DPI, build Windows |
| Async / server | tokio, tokio-util, axum, tower, tower-http | Feature flags tương thích, bounded channels, shutdown/timeout |
| HTTP | reqwest, rustls, url, http, bytes, cookie_store | Duplicate headers/query, proxy/CA/mTLS, redirect auth, streaming/cancel; reuse client |
| Data | serde, serde_json, BSON từ driver mongodb, UUID crate | Large JSON policy, BSON version compatibility, deterministic serialization |
| MongoDB | mongodb official driver | Session APIs đúng phiên bản, retry labels, cancellation, replica-set integration test |
| YAML / OpenAPI | Chọn parser YAML được duy trì; kiểm tra openapiv3/utoipa/jsonschema theo use case | Không giả định deserialize = validation 3.0+3.1; unknown keyword và refs |
| Secrets | keyring hoặc Windows APIs phù hợp, secrecy, zeroize | Windows Credential Manager/DPAPI binding, no Debug leak, backup/key loss |
| Auth | argon2, password-hash, rand, TLS/session lib đã xác minh | Rate limits, session rotation/revoke, credential storage; không tự crypto |
| Rendering | thư viện Markdown native, SVG renderer, font handling | Không thực thi HTML/script; untrusted URLs/images; license font |
| Editor / search | Rust text rope/editor/highlight crate được prototype; regex có giới hạn | UTF-8, large document, incremental parse, cancellation, catastrophic cost |
| Import/export | zip/tar crate theo format, CSV | Zip-slip, symlink/path traversal, decompression bomb, schema/version manifest |
| Observability | tracing, tracing-subscriber, metrics crate | Redaction tập trung; telemetry ngoài tắt mặc định |
| Tests | proptest, criterion, testcontainers nếu phù hợp, HTTP fixture server | Docker availability; integration DB thật; benchmark Windows thật |
| Audit | cargo-audit, cargo-deny, cargo metadata/tree | Phiên bản tools ghim, license/advisory policy; không silently ignore failures |
| Packaging | WiX Toolset hoặc installer tool chính thức đã xác minh; Windows SDK signtool | MSI/EXE install/uninstall/upgrade, per-user permissions, signature verification |
| gRPC | tonic/prost, protoc khi phiên bản yêu cầu | Chỉ cài ở phase gRPC; proto import hạn chế; unary/streaming scope rõ |
| Scripts | runtime JS/Wasm cách ly đã kiểm chứng | Time/memory/resource limits cưỡng chế; không eval có toàn quyền |
| Generator | OpenAPI generator và runtime nó cần nếu chọn | Ghim binary/JAR/container hash; Java/Docker chỉ khi thực sự cần; không chạy output tự động |
| Deployment | Docker Engine + Compose, Coolify, mongosh, MongoDB database tools | Image digest, auth/replica set health, backup restore test |

## Doctor phải tạo báo cáo

`reports/preflight.md` và JSON: OS/CPU/RAM/disk, tool/version/path hợp lệ, rustc -Vv, cargo/rustup, target MSVC, linker/SDK, Cargo metadata/feature graph, Docker/Compose daemon, mongosh/dbtools, server topology/FCV, DNS/TLS/domain, ports, permissions data dirs, font licenses và optional tooling. Mỗi check = PASS/FAIL/BLOCKED/NOT_APPLICABLE với command, exit code và thời điểm. Không in URI/secret vào report.

Check local tools trước build; check remote health trước run/sync; không bắt mọi user cài Docker, MongoDB, Java, Node hoặc protoc khi app không cần. Developer prerequisites khác runtime requirements. Nếu chỉ có Linux: kiểm tra server tại đó, còn GUI Windows là BLOCKED, không giả PASS.

## Trình tự gate

1. Inspect môi trường, existing files và nguồn; chưa chạy installer hoặc bootstrap app.
2. Tra dependency/SDK từ docs chính thức, lập bảng compatibility, resolve version và features.
3. Dựng spike tối thiểu trong thư mục riêng, compile và smoke test target; chưa dựng toàn bộ sản phẩm.
4. Chạy MongoDB transaction commit+rollback thật, kiểm chứng topology; ghi ADR.
5. Chỉ khi gate qua mới scaffold workspace sản phẩm. Các feature gate nâng cao lặp lại trước phase tương ứng.
