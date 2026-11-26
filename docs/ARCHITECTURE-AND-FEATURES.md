# Swaggen — Tài liệu Kiến trúc Hệ thống & Tổng hợp Tính năng (`v0.1.20`)

> **Swaggen** là nền tảng kiểm thử, thiết kế và cộng tác API (REST, GraphQL, SSE, OpenAPI) viết hoàn toàn bằng **Rust** (`edition = "2024"`, MSRV `1.100`) theo kiến trúc **Windows Native Desktop + Cloud Collaboration Backend**.
>
> Nguyên tắc cốt lõi: mọi request API/GraphQL từ Workbench đều được thực thi **trực tiếp từ máy tính của người dùng (`Direct from this PC`)** qua Rust HTTP engine nội bộ (`hyper` + `rustls`), không đi qua server trung gian; dữ liệu cục bộ được lưu bền vững theo cơ chế atomic journal và bảo vệ bí mật bằng **Windows DPAPI**.

---

## 1. Tech Stack & Phiên bản chính

| Tầng | Công nghệ / Crate | Vai trò |
| :--- | :--- | :--- |
| **Desktop UI (Frontend)** | `eframe` / `egui 0.37.0` (`wgpu`, `accesskit`) | Giao diện Native Windows Pure White Surface, không dùng WebView/Electron |
| **HTTP Client Engine** | `hyper 1.13.0`, `hyper-util 0.1.21`, `tokio-rustls 0.26.6`, `rustls 0.23.46` | Thực thi HTTP/1.1, TLS 1.2/1.3, Streaming, Redirects, Multipart, OAuth2 PKCE |
| **Backend API Server** | `axum 0.8.9`, `tokio 1.54.1` | REST API (`/v1/*`), GraphQL (`/v1/graphql`), SSE realtime, Team Mock |
| **Database & Transactions** | `mongodb 3.10.2`, `bson 2.15.0` | MongoDB Replica Set Multi-Document ACID Transactions + In-Memory Store cho test |
| **Bảo mật & Mật mã** | `windows-sys 0.61.2` (DPAPI), `argon2 0.5.3`, `sha2 0.11.1`, `hmac 0.13.0`, `ring 0.17.14` | Bảo vệ secret trên Windows, băm mật khẩu Argon2id, ký số & Idempotency SHA-256 |
| **Xác thực** | Firebase Auth (Google Sign-In Loopback PKCE) + Session Token nội bộ | Đăng nhập Google qua trình duyệt, quản lý thiết bị/phiên, TOTP MFA |
| **Hạ tầng triển khai** | Docker, Docker Compose, Coolify VPS, Inno Setup (`.exe`) & WiX (`.msi`) | Đóng gói bộ cài Windows x64 và triển khai Server lên VPS |

---

## 2. Cấu trúc Thư mục Dự án (Workspace Layout)

Dự án được tổ chức theo mô hình **Rust Cargo Workspace** gồm **3 ứng dụng (`apps/`)** và **10 thư viện dùng chung (`crates/`)**:

```text
d:\code\potmen\
├── Cargo.toml                  # Cấu hình Workspace, version (0.1.20) và pin dependency tập trung
├── Dockerfile                  # Build image production cho swaggen-server
├── docker-compose.yml          # Triển khai Server + MongoDB Replica Set
├── 07-operations/              # Hướng dẫn triển khai Coolify VPS & Windows Installer
├── assets/                     # Icon ứng dụng (.ico, .png) và bộ icon Figma (assets/icons/)
├── tools/                      # Script kiểm tra P00-P01, bundle-result.ps1, build-installer.ps1
├── docs/                       # Tài liệu tổng hợp kiến trúc & tính năng
├── result/                     # Output build release theo từng phiên bản (gitignored), vd: result/v0.1.20/
│
├── apps/                       # === CÁC ỨNG DỤNG CHÍNH (BINARIES) ===
│   ├── desktop/                # [FRONTEND] Ứng dụng Windows Desktop (swaggen-desktop.exe)
│   ├── server/                 # [BACKEND]  Cloud Collaboration & GraphQL Server (swaggen-server.exe)
│   └── cli/                    # [CLI]      Headless Runner & Snippet Generator (swaggen-cli.exe)
│
└── crates/                     # === CÁC CRATE DÙNG CHUNG (LIBRARIES) ===
    ├── domain/                 # Thực thể nghiệp vụ, Variable Resolution, GraphQL Parser/Analyzer, Security/SSRF
    ├── api-contract/           # DTO & Schema giao tiếp chung giữa Desktop và Server
    ├── ui-kit/                 # Design System (Tokens, Buttons, Tabs, Splitters, Icons, Code Viewer)
    ├── http-engine/            # Engine gửi HTTP thực tế từ PC + OAuth2 PKCE Loopback
    ├── local-store/            # Lưu trữ bền vững cục bộ (Atomic Write, Journal, Quarantine)
    ├── secret-store/           # Lưu trữ bí mật mã hóa theo user Windows (DPAPI)
    ├── mongo-store/            # Tầng Repository MongoDB (ACID Transactions, Batched Loaders) & InMemoryStore
    ├── spec-engine/            # OpenAPI 3.0/3.1, Swagger 2.0, Postman/cURL/HAR/.http Importer & Assertions
    ├── runner/                 # Collection Runner, Local Mock Server, Sandboxed Script, Code Snippets, SSE
    └── sync/                   # Outbox bền vững, CRDT merge, Conflict Resolution & Offline Sync
```

### Phân biệt rõ vị trí Frontend và Backend

| Thành phần | Thư mục | File đầu vào / Quan trọng | Trách nhiệm |
| :--- | :--- | :--- | :--- |
| **Frontend (Desktop UI)** | [`apps/desktop/`](file:///d:/code/potmen/apps/desktop/src) | [`main.rs`](file:///d:/code/potmen/apps/desktop/src/main.rs), [`app.rs`](file:///d:/code/potmen/apps/desktop/src/app.rs), [`views/`](file:///d:/code/potmen/apps/desktop/src/views) | Toàn bộ giao diện Workbench, Sidebar, GraphQL Studio, Response Inspector, Runners, Sync, Settings |
| **Frontend UI Kit** | [`crates/ui-kit/`](file:///d:/code/potmen/crates/ui-kit/src/lib.rs) | [`lib.rs`](file:///d:/code/potmen/crates/ui-kit/src/lib.rs) | Token màu sắc/typography, splitter kéo thả 1:1, icon SVG/PNG từ Figma, code viewer đánh số dòng |
| **Frontend Background Workers** | [`apps/desktop/src/`](file:///d:/code/potmen/apps/desktop/src) | [`worker.rs`](file:///d:/code/potmen/apps/desktop/src/worker.rs), [`server_worker.rs`](file:///d:/code/potmen/apps/desktop/src/server_worker.rs), [`sync_worker.rs`](file:///d:/code/potmen/apps/desktop/src/sync_worker.rs) | Chạy bất đồng bộ ngoài luồng UI: gửi HTTP cục bộ, mở trình duyệt Google Sign-In, sync team, lưu disk/DPAPI |
| **Backend (API Server)** | [`apps/server/`](file:///d:/code/potmen/apps/server/src) | [`main.rs`](file:///d:/code/potmen/apps/server/src/main.rs), [`lib.rs`](file:///d:/code/potmen/apps/server/src/lib.rs), [`graphql_api.rs`](file:///d:/code/potmen/apps/server/src/graphql_api.rs), [`firebase_auth.rs`](file:///d:/code/potmen/apps/server/src/firebase_auth.rs) | REST API `/v1/*`, GraphQL `/v1/graphql`, Idempotency middleware, Firebase JWT verifier, Team Mock, RBAC |
| **Backend Data Layer** | [`crates/mongo-store/`](file:///d:/code/potmen/crates/mongo-store/src) | [`repo.rs`](file:///d:/code/potmen/crates/mongo-store/src/repo.rs), [`mongo_backend.rs`](file:///d:/code/potmen/crates/mongo-store/src/mongo_backend.rs), [`memory_backend.rs`](file:///d:/code/potmen/crates/mongo-store/src/memory_backend.rs) | `WorkspaceRepository` trait, MongoDB transactions, batched `$in` queries, `idempotency_records`, Argon2id |
| **Shared Contract & Core** | [`crates/domain/`](file:///d:/code/potmen/crates/domain/src), [`crates/api-contract/`](file:///d:/code/potmen/crates/api-contract/src/lib.rs) | [`workspace.rs`](file:///d:/code/potmen/crates/domain/src/workspace.rs), [`graphql.rs`](file:///d:/code/potmen/crates/domain/src/graphql.rs), [`lib.rs`](file:///d:/code/potmen/crates/api-contract/src/lib.rs) | Model chung, parser GraphQL, resolver biến `{{var}}`, sinh `Idempotency-Key`, chính sách bảo mật & SSRF |

---

## 3. Sơ đồ Kiến trúc Tổng thể

```mermaid
flowchart TB
    subgraph PC["Máy tính người dùng (Windows Desktop)"]
        UI["apps/desktop (egui UI + views/*)\nWorkbench · GraphQL Studio · Sidebar · Runner"]
        UIKit["crates/ui-kit\nDesign System · Splitters · Icons"]
        Worker["Desktop Workers\nworker.rs · server_worker.rs · sync_worker.rs"]
        HttpEng["crates/http-engine\nhyper + rustls (Direct from this PC)"]
        LocalStore["crates/local-store\nAtomic JSON + Journal + Outbox"]
        DPAPI["crates/secret-store\nWindows DPAPI Encrypted Secrets"]

        UI <--> UIKit
        UI <-->|"Command / Event Channels"| Worker
        Worker --> HttpEng
        Worker --> LocalStore
        Worker --> DPAPI
    end

    TargetAPI["API / GraphQL Đích của người dùng\n(MockAPI, localhost, Staging, Prod...)"]
    HttpEng -->|"100% Gửi trực tiếp từ PC\n(Không qua Swaggen Server)"| TargetAPI

    subgraph VPS["Swaggen Collaboration Server (Coolify VPS)"]
        Server["apps/server (Axum 0.8.9)\nREST /v1/* · GraphQL /v1/graphql\nIdempotency Middleware · SSE Events"]
        Firebase["firebase_auth.rs\nGoogle OAuth2 + ID Token Verify"]
        MongoStore["crates/mongo-store\nBatched Loaders (Zero N+1) · Transactions"]
        MongoDB[("MongoDB Replica Set\nWorkspaces · Entities · Revisions\nIdempotency Records (TTL 24h)")]

        Server --> Firebase
        Server --> MongoStore
        MongoStore --> MongoDB
    end

    Worker <-->|"Sync, Auth, RBAC, Team Comments, Vault"| Server
```

---

## 4. Chi tiết Kiến trúc Frontend (`apps/desktop`)

Giao diện Desktop được thiết kế theo mô hình **Actor / Message-Passing** để đảm bảo luồng vẽ UI (`egui` 60fps) **không bao giờ bị đơ** khi gửi mạng hay đọc/ghi ổ đĩa:

- **Main UI State & Shell ([`apps/desktop/src/app.rs`](file:///d:/code/potmen/apps/desktop/src/app.rs))**:
  - Quản lý `WorkspaceState`, `open_tabs`, `responses`, trạng thái dirty (`generation` vs `saved_generation`), phím tắt toàn cục (`Ctrl+K`, `Ctrl+S`, `Ctrl+Enter`, `Ctrl+B`, `Ctrl+P`, `Ctrl+W`) và điều hướng `MainMode`.
- **Các màn hình chính ([`apps/desktop/src/views/`](file:///d:/code/potmen/apps/desktop/src/views/mod.rs))**:
  - [`sidebar.rs`](file:///d:/code/potmen/apps/desktop/src/views/sidebar.rs): Cây Collection/Folder/Request hỗ trợ **kéo-thả (Drag & Drop)** linh hoạt, tìm kiếm theo tên/method/tag, popup menu `⋯` chuẩn `PopupCloseBehavior::CloseOnClickOutside`, khu vực `PERSONAL` có thanh kéo chỉnh chiều cao.
  - [`request_pane.rs`](file:///d:/code/potmen/apps/desktop/src/views/request_pane.rs): Thanh tab request (nút `+` tạo REST, `+ GQL` tạo GraphQL, icon chuyển layout Ngang/Dọc), thanh URL với nút chuyển `REST | GQL`, đồng bộ 2 chiều URL ↔ `Query String` & `Path Params`, ẩn tab `Body` với `GET`/`HEAD`, tab **GraphQL Studio**, Auth, Assertions/Extractors, Settings (`Idempotency-Key` + Proxy/TLS), và Code Preview.
  - [`response_pane.rs`](file:///d:/code/potmen/apps/desktop/src/views/response_pane.rs): Hiển thị Status, TTFB, dung lượng, huy hiệu `Idempotent Replay`, cảnh báo `GQL Errors`, `DB Queries: N (Batched)`, chế độ `Expand` toàn màn hình, Pretty JSON + cây JSONPath, Headers/Cookies, Test Results, Redirect Chain, Diff giữa 2 lần gọi và Save Sanitized Example.
  - [`runner.rs`](file:///d:/code/potmen/apps/desktop/src/views/runner.rs): Collection Runner (CSV/JSON dataset), Local Mock Server + Traffic Logs, GraphQL & SSE Tester, Dev Utilities.
  - [`specs.rs`](file:///d:/code/potmen/apps/desktop/src/views/specs.rs): Trình soạn thảo & kiểm định OpenAPI 3.0/3.1 / Swagger 2.0, diff spec, schema tree, contract validation.
  - [`import_export.rs`](file:///d:/code/potmen/apps/desktop/src/views/import_export.rs): Import cURL, Postman v2.1 (gồm cả GraphQL), HAR, `.http`, xuất workspace sạch secret.
  - [`server.rs`](file:///d:/code/potmen/apps/desktop/src/views/server.rs) & [`welcome.rs`](file:///d:/code/potmen/apps/desktop/src/views/welcome.rs): Quản lý profile Server, đăng nhập Google qua trình duyệt (Firebase Loopback), quản lý Workspace/Members/Invitations/Sessions/MFA.
  - [`sync.rs`](file:///d:/code/potmen/apps/desktop/src/views/sync.rs): Outbox offline, Publish/Pull, giải quyết xung đột (Conflict Resolution), lịch sử revision & khôi phục, Comment thread, Team Vault, Notifications.
  - [`panels.rs`](file:///d:/code/potmen/apps/desktop/src/views/panels.rs): Environments (Production confirm guard), History, Cookies, Windows DPAPI Secrets, Sanitizer, Storage & Migration, Privacy, Shortcuts & Auto-Update.

---

## 5. Chi tiết Kiến trúc Backend (`apps/server` & `crates/mongo-store`)

Backend (`axum 0.8.9`) phục vụ đồng bộ đội nhóm, quản lý quyền, GraphQL API và lưu trữ an toàn trên MongoDB Replica Set:

### Middleware & Bảo vệ tầng HTTP ([`apps/server/src/lib.rs`](file:///d:/code/potmen/apps/server/src/lib.rs))
1. **`attach_request_id`**: Gắn và trả về header `X-Request-Id` để truy vết log đầu-cuối.
2. **`enforce_idempotency_key`**: Áp dụng nguyên tử cho mọi endpoint ghi dữ liệu (`POST`, `PUT`, `PATCH`, `DELETE`, và GraphQL `mutation`, bỏ qua GraphQL `query` chỉ đọc).
   - Băm `(method, path_and_query, body)` bằng SHA-256.
   - Gọi `claim_idempotency` với unique index `{ scope: 1, key: 1 }` trên collection `idempotency_records` (TTL 24h):
     - Trùng key + cùng payload hash → Trả lại response `2xx` đã cache kèm header `X-Idempotent-Replayed: true`.
     - Trùng key + khác payload hash hoặc đang có request song song chưa xong → Trả về **`409 Conflict`**.
     - Request thất bại (`4xx`/`5xx`) → Tự động `release_idempotency` để client có thể sửa và gửi lại.

### Các nhóm Endpoint chính của Backend
- **Health & Metadata**: `/health/live`, `/health/ready`, `/v1/health/metrics`, `/v1/capabilities`.
- **GraphQL API** ([`graphql_api.rs`](file:///d:/code/potmen/apps/server/src/graphql_api.rs)): `GET/POST /v1/graphql` (hỗ trợ `__schema` introspection, chặn mutation trên `GET` với `405`, giới hạn `depth <= 8` & `fields <= 120`, DataLoader batching `batch_load_workspaces_children`).
- **Auth & Sessions**: `/v1/auth/bootstrap-owner`, `/v1/auth/login`, `/v1/auth/firebase`, `/v1/auth/firebase/browser`, `/v1/auth/logout`, `/v1/auth/logout-all`, `/v1/auth/sessions`, `/v1/auth/invitations/pending`, `/v1/auth/invitations/{id}/accept`, `/v1/auth/invitations/{id}/decline`, `/v1/auth/invite-links/join`, `/v1/auth/mfa/*`.
- **Workspaces, RBAC & Audit**: `/v1/workspaces`, `/v1/workspaces/{id}/members`, `/v1/workspaces/{id}/invitations`, `/v1/workspaces/{id}/invite-links`, `/v1/workspaces/{id}/invite-links/{link_id}/revoke`, `/v1/workspaces/{id}/audit`, `/v1/workspaces/{id}/quotas`, `/v1/workspaces/{id}/archive`, `/v1/workspaces/{id}/transfer-ownership`, `/v1/workspaces/{id}/leave`.
- **Sync, Branches & Collaboration**: `/v1/workspaces/{id}/entities`, `/v1/workspaces/{id}/publish`, `/v1/workspaces/{id}/changes`, `/v1/workspaces/{id}/snapshot`, `/v1/workspaces/{id}/events` (SSE), `/v1/workspaces/{id}/entities/{eid}/revisions`, `/v1/workspaces/{id}/entities/{eid}/restore`, `/v1/workspaces/{id}/comments/*`, `/v1/workspaces/{id}/notifications/*`, `/v1/workspaces/{id}/branches`, `/v1/workspaces/{id}/merge-requests/*`.
- **Team Vault, Mocks, Runners & Schedules**: `/v1/workspaces/{id}/vault/*`, `/v1/workspaces/{id}/mocks/*`, `/mock/{workspace_id}/{*tail}`, `/v1/workspaces/{id}/runners/*`, `/v1/workspaces/{id}/schedules`, `/v1/security/check-ssrf`.

### Tối ưu Hiệu năng & Zero N+1 Query ([`mongo_backend.rs`](file:///d:/code/potmen/crates/mongo-store/src/mongo_backend.rs))
- **GraphQL Batched Loader (`batch_load_workspaces_children`)**: Gom toàn bộ `workspace_ids` để tải `entities`, `members` (+ `users`), `comments`, `notifications` bằng toán tử `$in` trong số lượng truy vấn DB hằng số (`O(1)` DB round-trips thay vì `O(N)`).
- **`list_workspaces`**: Thay thế vòng lặp `3N + 1` bằng truy vấn `$in` trên `workspaces`, `workspace_permission_fences` và `$match + $group` aggregation trên `memberships` (index `{ user_id: 1 }`).
- **`list_members_and_invitations`**: Gom `user_id` thành 1 truy vấn `$in` duy nhất trên `users`.
- **`publish_batch`**: Prefetch hàng loạt `applied_operations` và `entities` bằng `$in`, tăng sequence bằng một `$inc` duy nhất và ghi hàng loạt bằng `insert_many`.
- **`kv_list`, `restore_entity_revision`, `reply_comment`, `resolve_comment`**: Truy vấn trực tiếp theo khóa hoặc dải B-tree prefix (`$gte`/`$lt`), loại bỏ toàn bộ full-collection scan.

---

## 6. Tổng hợp Toàn bộ Tính năng Đã Hoàn thiện (Tính đến `v0.1.19`)

### A. API Workbench (Postman-Style Flat UI, REST & GraphQL)
1. **Thực thi HTTP cục bộ trực tiếp từ PC**: Hỗ trợ `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS` và `GRAPHQL` qua `swaggen-http-engine`, tuỳ chọn `HTTP Version` (`Auto / HTTP/1.1 / HTTP/2`), Local HTTP Proxy, Custom Root CA & mTLS Client Certificate, hiển thị tiến độ upload/download và `Cancel` tức thì.
2. **Giao diện chuẩn Postman & Responsive Split-Screen (`v0.1.19`)**:
   - **Flat Open Tab Strip** với viền nhấn xanh `#1D4ED8` phía trên, thanh URL hợp nhất `[ Method ▾ | URL ] [ Send ] [ ⋯ ]` (hỗ trợ phím tắt `Ctrl+S` lưu và `Enter` khi không focus ô nhập để gửi request ngay).
   - **Tab `Params` tách 2 bảng rõ ràng**: `Query Params (?key=value)` và `Path Variables (:id / {id})` đồng bộ trực tiếp với URL, cùng bộ icon vector `mini_icon_button` (`+`, `-`, `×`, `⋯`, `↑`, `↓`) bo góc hiện đại.
   - **Màu sắc Collection, đổi tên trực tiếp & Thùng rác xoá mềm (`Trash & Restore`)**: Menu `⋯` của Collection cho phép chọn màu thẻ/thư mục và đổi tên; mọi thao tác xoá Collection, Folder, Request, Environment đều là xoá mềm có khôi phục 1-click.
   - **Logo `[S]` chuyển thẳng về Home** và **Workspace Switcher gọn đẹp** ở đầu Sidebar (bỏ chữ `SWAGGEN` thừa).
3. **Biến Môi trường 3 Tầng, Live `{{variable}}` Pills & Gợi ý tự động khi gõ `{` (`v0.1.20`)**:
   - Quản lý trực quan 3 tầng biến trong `Environments`: **1. Deployment Environments** (`Local / Development / Staging / Production`), **2. Collection Variables**, **3. Global Workspace Variables**; click chọn môi trường là kích hoạt ngay kèm bộ chọn nhanh ở góc phải thanh Header.
   - **Gợi ý biến môi trường tức thì khi gõ `{` hoặc `{{`**: xuất hiện popup gợi ý (kèm giá trị xem trước & tầng biến, hỗ trợ `↑`/`↓`, `Tab`/`Enter` hoặc click) tại thanh URL, bảng Query/Path/Header/Form, Auth, Body (`JSON/Raw/XML`) và GraphQL Query/Variables; cộng với **Live `{{variable}}` Status Pills** để xem/gán nhanh chỉ với 1 click.
4. **GraphQL Studio toàn diện**:
   - Chọn `GRAPHQL` ngay trong Method dropdown hoặc nút `+ GQL`.
   - Trình soạn thảo đôi **Query/Mutation** ↔ **Variables (JSON)** có thanh kéo phân cách `1px`, bộ chọn `Operation`, nút `Sync $Vars`.
   - **Live Schema Introspection (`Fetch Schema`)**, **Schema Explorer** bấm `Use` sinh mẫu Query/Mutation, phân tích tĩnh độ sâu/tổng field/N+1 và hiển thị `DB Queries: N (Batched)`.
5. **Auth đầy đủ chuẩn Doanh nghiệp**:
   - Kế thừa Auth (`Collection` → `Folder` → `Request`): `Bearer`, `Basic`, `API Key` (Header/Query), **OAuth 2.0** (`Authorization Code + PKCE` loopback & `Client Credentials` 1-click fetch token), **AWS Signature v4** (HMAC-SHA256 canonical signing), và **Digest Auth** (RFC 7616 SHA-256).
6. **Pre-request & Post-response JS Automation trên từng Request (`v0.1.16`)**:
   - Tab **`Scripts` (`Pre-request` | `Post-response (Tests)`)** trên mọi Request (và kế thừa từ `Collection` → `Folder` → `Request`, tự động import từ Postman v2.1 `event`).
   - Sandbox JS thuần Rust cô lập hoàn toàn, hỗ trợ song song cú pháp **Postman `pm.*`** (`pm.environment/collectionVariables/globals/variables.get/set`, `pm.request.headers.upsert`, `pm.response.json()`, `pm.test`, `pm.expect`) VÀ **Playwright `test()` / `expect()`** (`.toBe`, `.toEqual`, `.toContain`, `.toHaveProperty`, `.toBeLessThan`), `Date.now()`, `crypto.randomUUID()`, `btoa`/`atob`, kèm thư viện mẫu 1-click và `Dry-run`.

### B. Playwright-Style Test Runner, Realtime Protocols, OpenAPI Studio & CLI
7. **Playwright-Style Test Runner Studio (`v0.1.16`)**:
   - Hỗ trợ 4 chế độ kiểm thử rõ ràng: **Unit Test**, **Functional (Data-driven CSV/JSON)**, **Integration**, và **End-to-End (E2E Workflow)** với chạy tuần tự hoặc **song song đa luồng (`Workers`)**.
   - Bố cục 2 cột kiểu Playwright UI: Cây chọn Endpoint `[✓]` bên trái + Timeline thực thi & **Step Trace Inspector** (`Response` | `Request` | `Tests & Console` | `Variables & Timings`) bên phải; xuất báo cáo JSON & JUnit XML.
8. **Scenario Mock Server & Giao thức Realtime (`WebSocket · SSE · gRPC`)**:
   - Local Mock Server tuỳ chỉnh cổng (`Port`), hỗ trợ **Conditional Scenario Rules** (khớp method/path/query/header để trả status code và độ trễ giả lập `delay_ms`).
   - Hỗ trợ kết nối trực tiếp **WebSocket (`ws://`/`wss://`)**, **Server-Sent Events (`SSE`)**, và **gRPC Unary over HTTP/2**.
9. **OpenAPI / Swagger Docs Studio đa định dạng & Drag-Drop Import (`v0.1.19`)**:
   - Hỗ trợ đầy đủ **OpenAPI 3.0 / 3.1 (JSON/YAML)**, **Swagger 2.0 (JSON/YAML)**, **Swagger UI / ReDoc HTML** (đọc toàn bộ body phản hồi, tự theo dõi `<script src="...swagger-initializer.js">` / URL spec nhúng và báo lỗi rõ nếu trang HTML không chứa spec thay vì tạo tab rỗng), và **Postman Collection v2.1 JSON**.
   - **Import & Export Hub hỗ trợ kéo-thả trực tiếp file `.json` / `.csv` / `.yaml` / `.http`**: tự động nhận diện Postman v2.1, Environment JSON, OpenAPI/Swagger, HAR 1.2, `.http` và bảng CSV endpoint (`name,method,url,headers,body,description`) để tạo thành Collection chạy test ngay trong Workbench.
10. **Headless CLI (`swaggen-cli`)**: Chạy test suite trong CI/CD (`swaggen-cli run`) và xuất code snippet (`cURL`, `Rust`, `JavaScript`, `Python`, `Go`, `C#`, `Java`, `PHP`, `Dart`).

### C. Cộng tác Đội nhóm, Xoá mềm DB, Typography JetBrains Mono & Đóng gói
11. **Workspace Collaboration tinh gọn & Xoá mềm trên MongoDB (`v0.1.19`)**: Tập trung hoàn toàn vào cộng tác Workspace (Google Sign-In qua Firebase, lời mời in-app `Accept / Decline`, **Public Expiring Invite Link** `swaggen://join/...`, đổi vai trò và **Owner xoá thành viên trực tiếp**); mọi thao tác xoá trên MongoDB & InMemoryStore đều ghi dấu xoá mềm (`deleted_at_unix` / `released_at_unix`) thay vì xoá cứng.
12. **Offline-First Sync & Versioning**: Hàng đợi Outbox bền vững trên ổ đĩa, đồng bộ delta/cursor, giải quyết xung đột 3 chiều (Base / Local / Server), lịch sử Revision & Restore, Branch & Merge Request, Comment Thread trên từng API, Realtime SSE Notifications.
13. **End-to-End Idempotency**: Chống double-click trên UI, tự động sinh `Idempotency-Key` SHA-256 cho `POST`/`PATCH`/GraphQL `mutation`, nút `Rotate Key`, và middleware Server replay `2xx` / chặn `409 Conflict`.
14. **Bảo mật, Typography `JetBrains Mono` đồng nhất & Đóng gói Windows**: Áp dụng **`JetBrains Mono`** thống nhất trên toàn bộ giao diện, lưới KV & Environment có clipping + padding từng ô chống tràn chữ, lưu secret bằng **Windows DPAPI**, **SSRF Guard**, Production Environment confirmation dialog, Light/Dark theme + `English / Tiếng Việt`, đóng gói Portable `.zip`, Inno Setup `.exe`, `.msi`, `SHA256SUMS.txt` và cập nhật ký số (`latest.json`).
