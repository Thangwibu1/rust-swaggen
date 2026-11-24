# SWAGGEN — MASTER IMPLEMENTATION PROMPT

Bạn là AI coding agent chịu trách nhiệm hiện thực Swaggen từ con số 0 thành một ứng dụng Windows desktop có thể cài đặt và sử dụng thật. Hãy làm việc trên toàn bộ bộ tài liệu này, tạo code, chạy kiểm thử và tiếp tục theo phase tới khi hoàn thành phạm vi được giao; không dừng sau một kế hoạch, skeleton hoặc bản UI giả.

## 1. Đích sản phẩm và ràng buộc không tự thay đổi

Swaggen hợp nhất API request client, OpenAPI design/documentation, testing và cộng tác cho nhóm qua server tự host. Frontend native dùng Rust và Rust GUI framework; backend Rust; database server MongoDB có transaction; deployment backend+MongoDB lên Coolify VPS; desktop là binary/installer Windows. Chủ dự án chưa lập trình phần nào lúc tạo kit.

Không thay frontend bằng React/TypeScript/HTML/WebView/Electron/Tauri hoặc web app. Không tự thay MongoDB bằng PostgreSQL/SQLite. Không coi MongoDB ở VPS là local persistence offline. Không ép người dùng desktop cài database server hoặc dev SDK để chạy app. Không triển khai dịch vụ cloud bắt buộc hay thanh toán theo ghế.

Framework UI/backend chưa được chủ dự án chọn tên. Bắt đầu đánh giá Iced và Axum/Tokio theo gate có điều kiện, so sánh egui/eframe khi cần. Không tự khẳng định framework nào nhanh nhất. Slint có DSL và license cần làm rõ nếu chọn. Local durable file journal/snapshot là đề xuất của kit, phải được spike chứng minh; không tự gọi đề xuất này là yêu cầu chủ dự án.

## 2. Nạp nguồn và giải quyết mâu thuẫn

Đọc theo thứ tự:

1. `AGENTS.md`, `README.md`, `01-prd/00-authoritative-decisions.md`.
2. `01-prd/01-effective-product.md`, `01-prd/requirements.json`, các module/section gốc liên quan; `00-source/PRD-original.md` để không bỏ sót hành vi.
3. `02-design/README.md`, `02-design/export-status.md`, `node-inventory.json`, `screen-requirement-map.json`. Chỉ ảnh tồn tại/được xác thực mới là evidence; metadata không phải screenshot.
4. `03-architecture/*`, `04-phases/README.md`, phase hiện tại, `traceability.json`, `status.md`.
5. `06-quality/*`, `07-operations/*`, `08-research/*` và reports/ADR mới nhất của repository.

Kit có 160 requirement và 12 acceptance criteria gốc, 316 screen/state/dialog. Tìm theo ID và đọc theo module, không nhét toàn bộ ảnh/XML vào context. Figma annotation cho coverage không chứng minh chức năng hoạt động.

Xác nhận chủ dự án ưu tiên hơn lựa chọn công nghệ trong DOCX cũ. PRD xác định nghiệp vụ; Figma xác định giao diện/trạng thái, có thể có bổ sung chưa chốt. Google sign-in ở Figma khác tài khoản nội bộ WS03: giữ auth nội bộ làm luồng mặc định không cloud, tạo Google adapter chỉ khi quyết định sản phẩm/credentials có thật; ghi pending, không hardcode success. OAuth của API đích là chức năng riêng. Foreground-only note `51:40541` ưu tiên hơn pastel surfaces cũ: ordinary surfaces trắng, màu ở foreground có ý nghĩa. Dark theme B là thiết kế bổ sung, không tự nói có ảnh chuẩn Figma.

## 3. Kiểm tra trước khi lập trình và trước khi chạy

Thực hiện P00 trước scaffold sản phẩm. Inspect môi trường/repository trước, giữ nội dung hiện có. Lập `reports/preflight.md` và JSON; từng check có PASS/FAIL/BLOCKED/NOT_APPLICABLE, command, version, exit code, timestamp. Không bịa đã cài hoặc test pass.

Kiểm tra Rust/rustup/cargo stable và MSRV, target x86_64-pc-windows-msvc, linker MSVC và Windows SDK, git, disk/data directory permissions, graphics/font/IME/accessibility requirements. Kiểm tra Docker+Compose daemon cho dev/server, mongosh/database tools, MongoDB topology/auth/FCV và network. Không bắt runtime desktop cài build tools. Java/protoc/Node chỉ khi tool đã chọn thực sự yêu cầu, không cài hàng loạt vì tên nằm trong backlog.

Tra docs chính thức cho **từng crate/SDK/tool** trước dùng: exact version, stable/dev, maintenance, license, advisories, MSRV, target OS, native deps, feature flags và API signatures. Lập compatibility matrix, pin rust-toolchain.toml/Cargo.lock và container digest. Không dùng API đoán từ trí nhớ hoặc ví dụ phiên bản khác; docs `latest` không thay lockfile. Có network bị chặn thì ghi research blocked, không bịa release mới nhất. Đọc cảnh báo cancellation của MongoDB Rust driver đúng phiên bản.

Chạy prototype native Windows gồm shell, list 1.000 item, editor JSON/YAML multiline/highlight/search/undo, IME tiếng Việt, keyboard, DPI, screen reader và response lớn. Chạy spike durability local và MongoDB transaction commit/rollback thật. Chỉ qua gate tương ứng mới chọn framework/storage/versions bằng ADR có số liệu. Linux compile không là Windows GUI validation. Thiếu môi trường thì tiếp tục phần độc lập, gate Windows vẫn blocked.

Trước mọi lệnh run/test/deploy, doctor scope kiểm tra dependency/config/topology; không in secret. Thiếu SDK/package thì xác định nguồn chính thức và cách cài/khắc phục cụ thể; không chạy installer từ nguồn lạ. Không bootstrap production/reset DB nếu môi trường không đúng.

## 4. Kiến trúc và dữ liệu

Tách UI, domain, HTTP engine, local storage, Mongo storage server-only, sync, spec engine, secrets, tests/mock/scripts. Dùng typed commands/errors/IDs, versioned API contract và migration; UI không query MongoDB, không giữ DB credentials.

Cục bộ phải làm việc offline, giữ draft/outbox/history/tab qua restart/crash. Draft+outbox mutation cần durable atomic batch. Giữ secret bằng cơ chế Windows phù hợp, tham chiếu SecretRef; không coi JSON/DB thường là mã hóa. Không ghi token trong Debug/logs/URLs/report/export. Shared template có tên biến, auth config và public values; token/cookie/history/run results mặc định riêng. Shared example/report có preview redaction chủ động.

MongoDB transaction phải chạy trên replica set hoặc topology hỗ trợ đã kiểm chứng. Coolify MongoDB standalone default không đủ. Một node replica set có thể dev/budget nhưng không HA; nhiều container cùng VPS không phải đa failure domain. DB network riêng, member auth, persistent volumes và least-privilege app user. Không expose MongoDB credentials ra desktop.

Publish transaction chứa conditional entity write theo base revision, revision record, event/cursor, audit cần thiết và operation dedupe ACK trong cùng ClientSession. Mọi operation DB trong transaction phải nhận session. Không chạy song song trong một transaction. Read/write concerns, unique indexes và retry phải thiết kế đúng driver. Callback có thể chạy nhiều lần: không side effect network/file/webhook trong transaction. Transient error retry transaction; unknown commit retry/resolve ACK, không báo failed chắc chắn khi có thể commit. Idempotency cùng operation ID+hash chỉ một revision; ID dùng lại payload khác bị từ chối.

Revoke permission race phải được thiết kế rõ, có permission fence/serialization và integration test; chỉ đọc snapshot membership chưa đủ. Attachments upload verified trước publish reference; GC trì hoãn; filesystem không tự atomic cùng MongoDB.

Sync: autosave local khác Publish. Durable intent → queued → sending → acked/retry/conflicted/blocked. ACK sau server commit. Reconnect auth/capabilities/permission → catch-up cursor → retry cùng operation ID. Giữ base/local/server khi conflict, không silent last-write-wins. Tombstones chống resurrect. Cursor expiry resnapshot có consistent cut, giữ local overlay/outbox. Event log bền là nguồn catch-up, realtime chỉ là notification. Localhost là máy hiện tại; không tự tunnel.

## 5. UI và chức năng thật

Xây reusable Rust UI kit từ component library, semantic tokens và variants, dùng layout responsive; không dựng mỗi screen một bản widget rời. Baseline 1440×900; bề mặt trắng theo note mới, font/license chính thức, không dùng emoji thay icon SVG có sẵn. Hỗ trợ focus/keyboard/IME/DPI/accessibility và mọi loading/empty/error/permission/offline state khi áp dụng.

Không dùng screenshot làm giao diện. Không hardcode response, badge synced, login thành công, member count hay test report. Mock chỉ là fixture test có nhãn; runtime phải nối service/storage thật. Button phải có hành vi hoặc feature status trung thực; chưa implement không được tính complete. UI error giữ draft và có retry/recovery rõ.

HTTP bảo toàn query/header trùng và thứ tự; async send/cancel/timeout, host-scoped TLS exceptions, strip auth cross-host redirect; body stream ra file, preview cap và binary handling. Dùng environment/secret resolution chính xác; production confirmation trước mutation khi yêu cầu.

OpenAPI parse/validate/ref/normalize/render native; 3.0/3.1 semantics khác nhau; unsupported keyword rõ, không báo valid giả. Stable operation ID và request overrides không tự sửa spec. Import/export format có version, loss report, secret-safe preview, deterministic roundtrip; giới hạn archive/ref/network. Advanced protocols/scripts/vault/runner theo requirement, không hứa tương thích cả Postman pm API hoặc OpenAPI version chưa test.

## 6. Hiệu năng bằng phép đo

Bắt đầu bằng thiết kế bounded memory: virtual lists, lazy schema/body, paginated queries, bounded channels/worker pools, backpressure, cancellation, incremental parse/highlight, no full-workspace clone per frame, connection pools reused. Không blocking IO trên UI thread. Đo idle repaint/CPU; không busy loop.

Theo `06-quality/performance-and-security.md`: 1.000 request thao tác p95 <=200ms, response 20MB không treo, sync mục tiêu <=3s mạng ổn định; các budget startup/RAM/frame khác là mục tiêu đề xuất cần đo. Release build Windows, baseline cấu hình máy, raw samples, p50/p95/p99, dataset/commit/toolchain. Profile rồi tối ưu bottleneck; không tự thêm unsafe hoặc allocator "nhanh" nếu không có evidence. Không quảng cáo hiệu năng đã đạt chỉ vì Rust.

## 7. Tiến hành theo phase, không dừng ở MVP

Theo P00→P12, giữ dependency graph. P06 = bản A dùng cho nhóm; tiếp tục B rồi C và full-product gate. Mỗi phase đọc file riêng và prompt execute tương ứng. Làm vertical slice UI→domain→persistence→API→tests, hoàn thiện lỗi/rollback trước chuyển tiếp. Không dựng toàn bộ UI trước khi chứng minh sync và storage model.

Mỗi requirement có trạng thái not_started/in_progress/implemented_unverified/verified/blocked và links: code, screen/state, test command, result, evidence, commit nếu có. Cập nhật traceability và backlog sau slice. Không reset status khi context bị thu gọn. Nếu hết context, ghi checkpoint gồm done/current/next commands/known failures/ADR/evidence, rồi tiếp tục từ checkpoint.

Kiểm thử có ý nghĩa: domain/property tests, HTTP fixture tests, Mongo replica-set transaction tests, two-client E2E, Windows UI/accessibility/DPI, large-body benchmark, secret/isolation/import/sandbox security, backup restore và clean installer. Test skipped/xfail/mocks không thay pass. Failed test phải sửa nguyên nhân, không xóa assertion. Chạy checks phù hợp, không lặp lại toàn suite vô nghĩa khi không có thay đổi.

## 8. Deployment và release

Tạo Dockerfile/Compose thực, images/toolchain pinned, health/readiness đúng, migrations/indexes idempotent, internal DB/volumes/member auth, graceful shutdown và Coolify instructions. Không coi container running là deploy verified. Thử HTTPS từ desktop, auth/publish/sync/file và restart.

Windows MSI/EXE phải cài trên máy sạch, hoạt động offline không cloud activation, data folder migration, update/restore đúng. Signing cần certificate có thật; không có ghi unsigned/blocked. Update feed self-hosted verify signature/hash trước apply. Backup DB+attachment+keys metadata nhất quán; restore sạch verify revision/count/hash/permission và RPO/RTO đo thật. Binary rollback không thay DB migration rollback plan.

Không tự deploy production khi chưa có destination/access/credentials; hoàn thiện file và staging/local tests trước, chỉ hỏi đúng thông tin còn thiếu. Không xin xác nhận lại stack đã chốt, không dừng để hỏi lựa chọn thường lệ có thể giải bằng ADR.

## 9. Đầu ra và điều kiện kết thúc

Bàn giao repository hoàn chỉnh, hướng dẫn developer và user, exact commands Windows/server, doctor, lockfiles, migrations/indexes, installer artifacts, container/deployment config, tests và reports, screenshots native đã chạy, benchmark raw data, SBOM/license notices, backup/restore/upgrade docs và requirement+screen coverage ledger.

Trước nói "hoàn thành toàn bộ", đối chiếu đủ 160 mã, 12 AC, mọi screen/state có disposition, A+B+C verified, không chức năng giả/stub/TODO đóng vai thật, không test quan trọng skipped và không benchmark bịa. Nếu có blocked hãy nói chính xác phần nào và phần đã làm; không gọi partial là complete. Bắt đầu ngay bằng đọc tài liệu và P00; sau gate tiếp tục code thay vì chỉ đưa ra kế hoạch.
