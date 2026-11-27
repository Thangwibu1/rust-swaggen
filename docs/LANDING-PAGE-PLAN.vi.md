# Kế hoạch landing page Swaggen

Ngày lập: 26/11/2026. Phạm vi: giới thiệu sản phẩm, thu hút người dùng thử và hỗ trợ triển khai cho nhóm. Đây là kế hoạch triển khai website marketing; ứng dụng Windows và backend Rust giữ kiến trúc hiện tại.

## 1. Đề xuất chính

Định vị Swaggen là **workspace API cho Windows, kết hợp gửi request, tài liệu OpenAPI, kiểm thử và cộng tác qua máy chủ tự host**.

Giữ tagline đã có trong bộ nhận diện: **One workspace. Every API.** Ngay bên dưới cần một câu giải thích cụ thể để khách mới hiểu sản phẩm trong vài giây. Câu chuyện của trang đi từ công việc cá nhân đến công việc nhóm: mở workspace → import hoặc tạo request → gọi API và đọc response → kiểm thử → chia sẻ thay đổi.

Đối tượng ưu tiên là backend/full-stack developer đang dùng Windows; tiếp theo là QA/API tester và tech lead của nhóm nhỏ. Lợi ích đầu tiên phải nhìn thấy là công việc làm được trong app. Rust, MongoDB và chi tiết triển khai xuất hiện ở phần dành cho người muốn tìm hiểu sâu.

Mục tiêu chuyển đổi chính: người truy cập đủ thông tin để tải và thử bản Windows được xác minh. Mục tiêu phụ: đọc hướng dẫn, xem demo và tìm hiểu cách cộng tác bằng server tự host. Không bắt đăng nhập hoặc nhập email để xem thông tin và hướng dẫn tải.

## 2. Cơ sở lập kế hoạch và hiện trạng

Đã đối chiếu các nhóm nguồn sau:

- `AGENTS.md`, kit `01-prd/00-authoritative-decisions.md`, `05-prompts/MASTER-PROMPT.md`.
- DOCX đặc tả gốc, PRD hiệu lực và 16 module: WS, RQ, AU, EV, RS, OA, DC, CO, SY, IM, TS, MK, PR, UX, SC, OP.
- `docs/ARCHITECTURE-AND-FEATURES.md`, Cargo workspace và các luồng chính trong desktop, server, CLI, HTTP engine, local store, secret store, spec engine, runner, sync và Mongo store.
- Trạng thái phase, checkpoint, báo cáo P00–P12, requirement/screen ledger và tài liệu đóng gói.
- Logo, token UI, bộ nhận diện, nguồn asset Figma và tình trạng export.
- `result/v0.1.22/version.json`, README và danh sách artifact thực có trong bundle.

Đây là rà soát tài liệu, cấu trúc và mã nguồn phục vụ kế hoạch marketing. Không chạy lại kiểm thử Rust, không nghiệm thu GUI, không cài thử installer, không xác minh server production hoặc GitHub Release trong công việc này. Trạng thái ghi trong báo cáo cũ không tự trở thành bằng chứng cho binary hiện tại.

| Phát hiện | Ý nghĩa đối với landing page |
| --- | --- |
| Workspace có 13 Cargo crate; phiên bản hiện tại `0.1.22` | Có nền tảng sản phẩm cụ thể để giới thiệu; cần đồng nhất nội dung với release được phân phối |
| Có desktop native, server cộng tác và CLI | Phân biệt rõ ứng dụng tải về, server tùy chọn và website giới thiệu |
| Personal workspace có luồng mở cục bộ; request Workbench chạy từ PC | Giải thích giá trị dùng cá nhân và gọi localhost; API từ xa vẫn cần kết nối mạng |
| Có REST/GraphQL Workbench, environment, response inspector, spec studio, runner, import/export | Đây là nhóm nội dung chính, dùng ảnh/video app thật |
| `result/v0.1.22` có Setup EXE, portable ZIP và ba binary | Có ứng viên tải về; sự tồn tại của file chưa chứng minh cài sạch, ký số hoặc tải công khai thành công |
| Manifest v0.1.22 ghi `UNSIGNED_PORTABLE` | Trình bày rõ trạng thái ký của từng artifact sau khi kiểm tra; không gọi bản này là signed |
| `dist/windows-x64` còn manifest/bundle v0.1.0 | Trang tải không được vô tình lấy bản cũ này làm bản mới nhất |
| README/checkpoint còn mô tả P01; P12 và ledger mô tả phạm vi lớn hơn | Nội dung marketing phải bám code/release thực tế, có bước rà soát bằng chứng |
| Ledger hiện ghi 142/160 verified, 17 implemented_unverified, 1 blocked | Đây là trạng thái do repository ghi nhận; không quảng cáo “hoàn thiện 100%” hoặc dùng tỷ lệ này làm số liệu quảng bá |
| 252/316 screen ghi verified; phần lớn render Figma vẫn thiếu | Không biến screen count thành feature count; cần chụp app hiện tại cho landing |
| EULA có `All rights reserved` | Chưa có cơ sở gắn nhãn open source hoặc đưa lời hứa cấp phép lâu dài |

Một số giới hạn cần giữ trong nội dung chi tiết:

1. `assess_openapi_version` ghi OpenAPI 3.1 với JSON Schema 2020-12 subset; OpenAPI 3.2 chưa được hỗ trợ. Không viết “tương thích toàn bộ OpenAPI”.
2. `ws_exchange_text` hiện từ chối scheme khác `ws`; không quảng bá `wss://` theo mô tả tổng hợp chưa khớp code.
3. gRPC hiện có giao diện unary với payload field string giới hạn. Chưa đưa gRPC toàn diện/proto import/streaming vào thông điệp chính.
4. Script sandbox có compatibility report và API không hỗ trợ; không gọi là tương thích mọi Postman script hoặc Playwright runtime.
5. `save_workspace()` có thể gọi `publish_current_on_save()` khi có team target. Không viết “Ctrl+S luôn chỉ lưu riêng”. Luồng autosave/lưu/publish cần xác minh trước khi quay demo nhóm.
6. Có Google/Firebase adapter trong code. Chế độ cá nhân và cấu hình xác thực của server cần giải thích riêng; tránh tuyên bố mọi luồng đều không cần dịch vụ ngoài.
7. Số đo benchmark P06 là bằng chứng lịch sử, không phải cam kết hiệu năng cho v0.1.22 hoặc so sánh với đối thủ.

## 3. Thông điệp theo người dùng

| Người dùng | Công việc cần giải quyết | Thông điệp | Bằng chứng trên trang |
| --- | --- | --- | --- |
| Backend/full-stack developer | Gọi REST/GraphQL, đổi môi trường, kiểm tra response | Từ request đến response trong cùng workspace | Workbench, environment và response inspector |
| QA/API tester | Chạy lại collection, xem assertion và kết quả | Biến thao tác kiểm tra API thành bộ kiểm thử có thể chạy lại | Runner, dataset, kết quả và JUnit/JSON thực |
| Tech lead/nhóm nhỏ | Chia sẻ request/spec và theo dõi thay đổi | Cộng tác API qua server nhóm tự quản lý | Hai client, lịch sử thay đổi và vai trò đã kiểm chứng |

Bốn trụ cột nội dung:

- **Build your API workflow:** request, environment, response và OpenAPI cùng một chỗ.
- **Start locally:** dùng workspace cá nhân trên Windows, giữ công việc cục bộ.
- **Bring your existing work:** import Postman Collection, OpenAPI, cURL và các định dạng đã xác minh.
- **Collaborate on your infrastructure:** dùng server cộng tác do nhóm quản lý, có hướng dẫn trách nhiệm vận hành.

Nội dung cần nói rõ phạm vi: request gửi tới API đích vẫn truyền dữ liệu tới API đó; cộng tác cần server và mạng; tác vụ runner/mock trên server có đường truyền riêng. Không dùng câu “dữ liệu không bao giờ rời máy”.

## 4. Bố cục trang chủ

Header gọn: logo Swaggen, Features, Workflow, Self-hosting, Docs, nút EN/VI và CTA chính. Các mục đầu là anchor; Docs đi tới hướng dẫn thực. Header cố định vừa đủ, không che nội dung hoặc focus trên mobile.

| Thứ tự | Khối | Nội dung và cách trình bày | Hành động |
| --- | --- | --- | --- |
| 1 | Hero | Tagline, một câu mô tả sản phẩm, nhãn Windows, ảnh Workbench lớn | Tải bản Windows đã xác minh; xem demo |
| 2 | Giá trị cốt lõi | Ba ý ngắn: workspace cá nhân, gọi API từ PC, server cộng tác tự host | Đi tới phần giải thích tương ứng |
| 3 | Workflow demo | Import → gửi request → đọc response → chạy test; video 45–60 giây | Phát video có điều khiển |
| 4 | API Workbench | REST/GraphQL, biến môi trường, auth và response; ảnh có chú thích điểm cần nhìn | Xem quick start |
| 5 | OpenAPI & docs | Import spec, xem endpoint/schema, tạo request và theo dõi thay đổi trong phạm vi hỗ trợ | Đọc giới hạn tương thích |
| 6 | Testing & automation | Collection runner, assertions, dataset, mock và CLI | Mở hướng dẫn chạy collection |
| 7 | Team & self-hosting | Workspace nhóm, vai trò, lịch sử/diff và server tự quản | Xem hướng dẫn self-host |
| 8 | Import & migration | Những định dạng đã kiểm chứng, ví dụ chuyển một collection có báo cáo giới hạn | Làm theo migration guide |
| 9 | Bắt đầu trong 3 bước | Tải → mở Personal workspace → import hoặc tạo request đầu tiên | Tải Windows |
| 10 | FAQ & CTA cuối | Giải đáp Windows, offline, tài khoản, dữ liệu, self-host và bản thử nghiệm | Lặp lại CTA chính |
| 11 | Footer | Docs, Downloads, Changelog, Privacy, License/Terms và kênh hỗ trợ có thật | Tới trang tương ứng |

Mỗi khối tính năng đi theo cùng công thức: một lợi ích → một minh chứng trực quan → một cách bắt đầu. Tránh dàn 160 yêu cầu thành hàng chục thẻ tính năng. Không thêm logo khách hàng, testimonial, số người dùng hay huy hiệu chứng nhận khi chưa có dữ liệu và quyền sử dụng.

Mobile: nội dung một cột; ảnh tổng quan đi kèm crop phóng lớn, người dùng mở được ảnh đầy đủ. Không thu nhỏ nguyên cửa sổ desktop đến mức chữ không đọc được. Thứ tự đọc và CTA giữ nhất quán với desktop.

## 5. Bản nháp copy có thể dùng khi thiết kế

### Hero tiếng Anh

Eyebrow: **Native API workspace for Windows**

Headline: **One workspace. Every API.**

Description: **Send REST and GraphQL requests, work with OpenAPI, and run API tests in one Windows workspace. Start with personal projects and connect to a self-hosted server for team collaboration.**

CTA sau khi có release đủ điều kiện: **Download for Windows**. CTA phụ: **Watch the workflow**.

Dòng thông tin ngắn: **Windows x64 · Personal workspace · Self-hosted collaboration**. Chi tiết OS tối thiểu và trạng thái unsigned/beta hiển thị theo bản phát hành đã xác minh ở trang tải.

### Hero tiếng Việt

Eyebrow: **Workspace API native dành cho Windows**

Headline: **Một workspace. Kết nối toàn bộ công việc API.**

Description: **Gửi request REST và GraphQL, làm việc với OpenAPI và chạy kiểm thử API trong một ứng dụng Windows. Bắt đầu với dự án cá nhân, kết nối server tự host khi cần cộng tác cùng nhóm.**

CTA: **Tải cho Windows**. CTA phụ: **Xem cách hoạt động**.

### Nội dung khối chính

| Khối | Tiêu đề tiếng Anh | Nội dung tiếng Việt để diễn giải |
| --- | --- | --- |
| Workbench | From request to response, in one place. | Soạn request, chọn môi trường và xem kết quả ngay trong cùng không gian làm việc |
| OpenAPI | Keep your API contract close to your requests. | Đặt đặc tả, tài liệu và request cạnh nhau để thuận tiện kiểm tra và cập nhật |
| Testing | Turn API checks into repeatable runs. | Gom request thành collection, thêm assertion và xem kết quả từng lần chạy |
| Team | Bring your team to your own server. | Kết nối workspace nhóm với hạ tầng do nhóm quản lý và theo dõi thay đổi qua lịch sử |
| Import | Start with the work you already have. | Nhập dữ liệu đang có; xem rõ phần chuyển được và phần cần điều chỉnh |

Đây là copy dự thảo, cần qua checklist release ở mục 12 trước khi xuất bản. “Every API” là tagline thương hiệu; phần tính năng phải công bố rõ các giao thức và giới hạn thực tế.

## 6. Hình ảnh, video và nhận diện

Nguồn dùng lại: `assets/brand-mark.png`, `assets/swaggen-icon-256.png`, `assets/favicon.png`, `assets/icons/*`, `assets/figma/SOURCES.json` và token trong `crates/ui-kit/src/lib.rs`. Logo đã có hình chữ S xanh teal và điểm salmon; giữ nguyên tỷ lệ.

Thiết kế đề xuất: nền trắng, chữ đậm rõ, đường phân cách mảnh, khoảng trắng rộng. Dùng `#202124` cho chữ chính, `#65686F` cho chữ phụ, `#E5E7EB` cho đường viền, teal `#245C59` và xanh `#1D4ED8` cho điểm nhấn có ý nghĩa. Ưu tiên button nền trắng, nhãn/viền rõ theo hướng foreground-only; kiểm tra tương phản và khả năng nhận biết hành động.

JetBrains Mono dành cho tagline, nhãn kỹ thuật, method và code; phần giải thích dài dùng sans-serif có dấu tiếng Việt tốt để dễ đọc. Đây là đề xuất typography cho website marketing, không thay font app. Có thể dùng system font trước; nếu thêm font thì kiểm tra nguồn và giấy phép trước khi đưa vào bundle.

| Asset cần tạo | Cảnh quay/chụp | Mục đích |
| --- | --- | --- |
| Hero Workbench | Collection + request REST + response JSON của fixture local | Chứng minh sản phẩm trong màn hình đầu |
| GraphQL | Query, Variables và response có dữ liệu mẫu | Minh họa một workflow thực |
| OpenAPI Studio | Spec và tài liệu endpoint tương ứng | Nối tài liệu với thao tác API |
| Runner | Assertion pass/fail thực, timeline và report | Chứng minh kiểm thử có thể chạy lại |
| Team history | Hai client, thay đổi có tác giả và diff | Chỉ dùng sau khi kiểm chứng server test riêng |
| Demo 45–60 giây | Import → đổi environment → Send → response → runner | Giảm công sức tìm hiểu sản phẩm |
| Open Graph | Logo, tagline và crop Workbench | Chia sẻ link trên mạng xã hội |

Chụp từ đúng release/commit dự định giới thiệu, dùng dữ liệu giả lập có nhãn demo, domain `.test` và secret rỗng hoặc che hoàn toàn. Chỉ crop, scale và tối ưu ảnh; không sửa ảnh để thể hiện tính năng chưa chạy. Có bản gốc và manifest ghi version/commit, viewport, nguồn cảnh và ngày chụp.

Video có poster, phụ đề, điều khiển và không autoplay có âm thanh. Không buộc tải video trước khi người dùng bấm. Chuyển động nhẹ, có `prefers-reduced-motion`. Hình trang trí được phép tối giản; screenshot app phải là ảnh thực.

## 7. Sitemap và nội dung hỗ trợ

Đề xuất EN là bản mặc định để tiếp cận developer quốc tế; cung cấp VI cho cộng đồng hiện tại. Đây là giả định kế hoạch, có thể thay khi xác định thị trường ưu tiên. Website không cần chờ app có thêm locale.

| URL đề xuất | Mục đích | Đợt |
| --- | --- | --- |
| `/` | Landing tiếng Anh | 1 |
| `/vi/` | Landing tiếng Việt | 1 |
| `/download/`, `/vi/download/` | Chọn bản Windows, checksum, yêu cầu hệ thống, trạng thái release | 1 |
| `/docs/getting-started/`, bản VI tương ứng | Tạo request đầu tiên và thử collection mẫu | 1 |
| `/docs/importing/` | Migration và giới hạn định dạng/script | 1 |
| `/docs/self-hosting/` | Phân biệt dùng cá nhân và cộng tác; hướng dẫn server đã kiểm chứng | 1, cần bằng chứng |
| `/changelog/` | Thay đổi theo phiên bản có thể tải | 1 |
| `/privacy/`, `/terms/` | Thông tin thực tế về website, phần mềm và quyền sử dụng | 1 |
| `/guides/*` | Bài hướng dẫn theo công việc tìm kiếm | 2 |

Không xuất bản route rỗng. Với nội dung chỉ có EN, giữ link và nhãn EN rõ ràng; không giả vờ đã có bản dịch. Có trang 404 và điều hướng quay về nội dung chính.

FAQ ban đầu cần trả lời: Swaggen dùng để làm gì? Hỗ trợ Windows nào? Có cần tài khoản để dùng Personal? Offline làm được những gì? Request có đi qua server Swaggen không? Nhóm có phải tự vận hành server không? Import được gì? Giới hạn Postman script/OpenAPI là gì? Bản tải là thử nghiệm hay ổn định và đã ký số chưa?

Không tạo bảng Free/Pro/Enterprise hoặc công bố giá khi chưa có chính sách phân phối. Tài liệu gốc định hướng không thu phí theo ghế cho sản phẩm tự xây; đó chưa phải cam kết thương mại công khai. Nếu sau này dùng thông điệp này cần mô tả rõ phiên bản, quyền sử dụng và chi phí vận hành server.

## 8. Hướng triển khai website

Đề xuất **Astro xuất site tĩnh + TypeScript cho phần dữ liệu/logic nhỏ + CSS tokens**, nằm trong `apps/landing/`. Astro phù hợp với nội dung HTML tĩnh và chỉ thêm JavaScript ở phần cần tương tác; hỗ trợ tổ chức route đa ngôn ngữ. Không cần SPA, đăng nhập, database hoặc gọi backend sản phẩm để hiển thị landing.

Web là kênh giới thiệu độc lập. Không chuyển giao diện Windows sang web, không nhúng website marketing làm UI app, không thay MongoDB hoặc Cargo workspace. Việc thêm `apps/landing/` là hạng mục tương lai; kế hoạch này chưa tạo scaffold hay cài dependency.

Tổ chức dự kiến:

- `src/components/`: Header, Hero, Workflow, FeatureSection, ImportFormats, FAQ, DownloadCTA, Footer.
- `src/layouts/`: layout chung cho metadata, navigation và locale.
- `src/pages/`: landing, download và trang tài liệu theo ngôn ngữ.
- `src/content/`: quick start, migration, self-hosting, changelog.
- `src/data/`: copy EN/VI, feature metadata và thông tin release được duyệt.
- `public/brand/`, `public/media/`: logo và ảnh/video đã kiểm tra.

Version, URL tải, target, checksum, kích thước, channel và signing status nằm trong một nguồn release metadata dùng chung. Mỗi artifact có thông tin riêng; không suy signing status của Setup từ portable ZIP. Không hardcode version ở nhiều component hoặc đọc trực tiếp file local từ browser.

Ưu tiên URL artifact theo version đã kiểm chứng. CI kiểm tra metadata/schema và link; khi bản mới chưa đạt điều kiện thì giữ release đã duyệt gần nhất. Nếu chưa có release công khai, bản preview chỉ mở demo/hướng dẫn và không tạo nút tải hỏng hoặc thông báo đăng ký thành công giả.

Build website tách khỏi Rust CI. Ghim phiên bản Node/package manager/dependency sau khi kiểm tra tài liệu chính thức ở thời điểm triển khai, commit lockfile và chạy build/check. Không cần chọn phiên bản thư viện trong giai đoạn lập kế hoạch.

Đầu ra là thư mục static có thể phục vụ qua HTTPS trên host/CDN phù hợp. Phương án dùng VPS hiện có: phục vụ file bằng web server sẵn có, không yêu cầu Docker. Chưa chọn domain, tạo dịch vụ, mở cổng, đổi DNS hoặc triển khai production trong phạm vi này.

## 9. SEO, truy cập và tốc độ

- Tiêu đề đề xuất: `Swaggen — API Workspace for Windows | REST, GraphQL & OpenAPI`.
- Mỗi route có title/description riêng, canonical, Open Graph và heading có thứ tự; một H1 cho landing.
- `sitemap.xml`, `robots.txt`, liên kết nội bộ và `hreflang` cho cặp trang đã dịch. Preview đặt noindex; production kiểm tra lại.
- Nội dung chủ đề đầu tiên: API client for Windows, OpenAPI workflow, import Postman Collection, API collection testing, self-hosted API collaboration. Đây là giả thuyết chủ đề, chưa có dữ liệu search volume.
- Schema `SoftwareApplication` chỉ chứa dữ kiện đúng. Không tự thêm rating, review, giá, award hoặc đảm bảo Google hiển thị rich result.
- Hero ưu tiên tải, có kích thước ảnh rõ; ảnh dưới fold lazy-load, có WebP/AVIF và srcset phù hợp. Nội dung chính có thể đọc khi tắt JS.
- Bàn phím điều khiển được menu, FAQ, demo và CTA; focus rõ, nhãn/alt phù hợp, hỗ trợ zoom và không truyền thông tin chỉ bằng màu.
- Mục tiêu hiệu năng thực địa ở percentile 75: LCP ≤ 2,5 giây, INP ≤ 200 ms, CLS ≤ 0,1. Đây là mục tiêu cần đo, không phải kết quả đã đạt. Trước launch dùng lab audit; sau launch cần traffic thật để đánh giá chỉ số thực địa.

## 10. Quảng bá và đo chuyển đổi

Đợt đầu ưu tiên hướng dẫn và demo giải quyết công việc thật. Chưa đề xuất chi tiền quảng cáo diện rộng khi đường tải/cài/bắt đầu chưa được xác minh.

| Hoạt động | Nội dung | Đích chuyển đổi |
| --- | --- | --- |
| Bài giới thiệu sản phẩm | Video workflow, ai nên dùng, phạm vi bản thử nghiệm | Landing có UTM |
| Bài hướng dẫn nhập dữ liệu | Một collection mẫu, thao tác import và giới hạn | Quick start/import guide |
| Demo kiểm thử | Collection nhỏ với case pass/fail thực | Runner guide và download |
| Bài self-hosting | Dùng Personal và dùng Team khác nhau thế nào | Tài liệu self-host đã kiểm chứng |
| Changelog định kỳ theo release | Nêu thay đổi người dùng nhìn thấy | Release tương ứng |

Kênh đề xuất: cộng đồng developer Windows/backend và QA, bài viết kỹ thuật, video ngắn, GitHub Release nếu có phát hành công khai. Chỉ đăng bài/gửi thông báo khi được yêu cầu; kế hoạch này không thực hiện xuất bản hoặc liên hệ ai.

Website analytics là quyết định riêng với telemetry của app. Mặc định chưa thêm tracking bên thứ ba. Nếu bật đo lường, thu thập tối thiểu và mô tả đúng trong Privacy; không ghi URL API, token, nội dung workspace hoặc email vào event analytics.

| Event dự kiến | Dữ liệu tối thiểu | Câu hỏi trả lời |
| --- | --- | --- |
| `cta_click` | vị trí hero/footer, locale, campaign | Thông điệp nào dẫn tới hành động? |
| `demo_play` | demo ID, locale | Người xem có muốn tìm hiểu workflow? |
| `download_click` | version, artifact, nguồn CTA | Có bao nhiêu lượt chuyển tới trang/file tải? |
| `docs_open` | guide slug, locale | Người dùng cần hướng dẫn nào? |

`download_click` chỉ đo click, không chứng minh file tải hoàn tất hay app cài thành công. Đánh giá kích hoạt bằng phiên dùng thử tự nguyện hoặc phản hồi có chủ đích; không tự thêm telemetry vào app để đo.

Giai đoạn đầu lấy baseline trước khi đặt chỉ tiêu tăng trưởng: lượt truy cập đủ dữ liệu, tỷ lệ click tải, tỷ lệ mở hướng dẫn và lý do người dùng thử dừng lại. Mục tiêu nghiên cứu ban đầu có thể là 5–10 phiên dùng thử có phản hồi; đây là chỉ tiêu đề xuất, không phải số người dùng đã có.

## 11. Backlog và thứ tự triển khai

Ước lượng dưới đây tính theo ngày công tập trung; phụ thuộc vào tài nguyên và không bao gồm thời gian mua domain, ký số, hoàn thiện tính năng sản phẩm hoặc chờ chứng thực server.

| Mốc | Công việc | Đầu ra để review | Ước lượng |
| --- | --- | --- | --- |
| M0 | Chọn release/channel, đối chiếu feature/copy, thống nhất đường tải | Bảng claim và release metadata hợp lệ | 0,5–1 ngày |
| M1 | Chốt câu chuyện, copy EN/VI, wireframe desktop/mobile | Bố cục đầy đủ và nội dung từng khối | 1–1,5 ngày |
| M2 | Chuẩn bị fixture, chụp app và quay demo | 4–6 ảnh thực, video, OG image, asset manifest | 1–2 ngày |
| M3 | Xây Astro, responsive, download và docs nền | Preview cục bộ có chức năng đầy đủ | 2–3 ngày |
| M4 | Rà soát nội dung, link, accessibility, performance và release | Báo cáo kiểm tra, bản có thể xuất bản | 1–1,5 ngày |
| M5 | Gắn domain/HTTPS khi có cấu hình và yêu cầu xuất bản | Website live và kiểm tra sau deploy | 0,5–1 ngày |
| M6 | Hướng dẫn mở rộng và thử thông điệp từ phản hồi | Bài viết, demo và copy cải tiến | Sau launch |

Tổng dự kiến: khoảng **6–10 ngày công** cho landing và nội dung hỗ trợ đầu tiên khi sản phẩm/release phục vụ demo đã sẵn sàng. M1 có thể bắt đầu ngay; M2/M3 phụ thuộc bộ release và dữ liệu demo. Phần Team có thể tiếp tục chuẩn bị nội dung trong khi chờ nghiệm thu server, nhưng không trình diễn như tính năng đã xác minh.

Đầu ra lần triển khai đầu tiên: trang chủ EN/VI, trang tải đúng release, quick start, import guide, self-host guide đúng phạm vi, changelog, FAQ, nội dung Privacy/Terms đúng thực tế, asset demo, source website và báo cáo kiểm tra. Chỉ mở các mục có nội dung hoàn chỉnh.

## 12. Điều kiện hoàn thành

### Nội dung và bằng chứng

- Copy và screenshot bám cùng release/commit; giới hạn giao thức, import và script được nói đúng.
- Không có số người dùng, testimonial, benchmark đối thủ, chứng nhận, giá hoặc license bị suy diễn.
- Hành vi lưu/publish đã thống nhất với demo và tài liệu; không hứa riêng tư khác với đường dữ liệu thực.
- Có hướng dẫn phân biệt Personal, Team, xác thực và trách nhiệm vận hành server.

### Tải và bắt đầu sử dụng

- Artifact có URL tải thật, trả file đúng định dạng/version, size/hash khớp metadata.
- Kiểm tra status ký thực tế riêng cho Setup/ZIP; nêu rõ unsigned khi áp dụng.
- Thử cài/chạy/gỡ trên môi trường Windows sạch phù hợp; thử Personal, tạo hoặc import request, gọi fixture và mở lại dữ liệu.
- Không gọi đây là stable nếu chưa có quyết định release và bằng chứng cần thiết; có thể phát hành thử nghiệm với giới hạn rõ.
- Khả năng tải trên mobile/non-Windows được giải thích rõ; không tự bịa link macOS/Linux.

### Website

- Build/check thành công, không có link rỗng, route hỏng, asset thiếu hoặc lỗi console trong luồng chính.
- Kiểm tra các viewport đại diện 360/390/768/1280/1440 px và zoom 200%; không tràn ngang hoặc mất CTA.
- Luồng menu/FAQ/video/CTA dùng được bằng keyboard; kiểm tra tương phản và reduced motion.
- Không có secret/dữ liệu thật trong HTML, source map, ảnh, video, form hoặc event.
- Kiểm tra metadata, canonical/hreflang, sitemap/robots, trang 404, HTTPS và file download sau deploy.

## 13. Quyết định còn mở và cách tiếp tục

| Quyết định | Giả định hiện tại | Khi nào cần chốt |
| --- | --- | --- |
| Người dùng ưu tiên | Developer Windows, sau đó QA và nhóm nhỏ | Trước chốt copy M1 |
| Ngôn ngữ chính | EN mặc định, VI đi cùng | M1 |
| Channel phát hành | Bản thử nghiệm cho đến khi kiểm tra release hoàn tất | M0/M4 |
| Domain, host và liên kết tải công khai | Chưa xác minh | Trước M5 |
| Quyền phân phối/giá | Dùng thông tin đã chốt; chưa đưa bảng giá | Trước công bố lời hứa thương mại |
| Hỗ trợ và analytics website | Chỉ hiển thị kênh thực; tracking chưa bật | Trước M4/M5 |

Các quyết định này không cản việc thiết kế wireframe, chuẩn bị copy và xây preview. Bước thực thi tiếp theo là M0/M1, sau đó chuẩn bị asset thật và triển khai `apps/landing/`. Chưa thay đổi mã nguồn sản phẩm hoặc publish website trong nhiệm vụ lập kế hoạch này.

## 14. Nguồn tham chiếu

Nguồn repository quan trọng:

- [Kiến trúc và tính năng](ARCHITECTURE-AND-FEATURES.md)
- [Quyết định hiệu lực](../Swaggen-Agent-Kit/01-prd/00-authoritative-decisions.md)
- [PRD hiệu lực](../Swaggen-Agent-Kit/01-prd/01-effective-product.md)
- [Tình trạng export](../Swaggen-Agent-Kit/02-design/export-status.md)
- [Requirement ledger](../reports/requirement-ledger.json)
- [Screen ledger](../reports/screen-ledger.json)
- [Báo cáo P12 lịch sử](../reports/P12-final.md)
- [Cargo workspace](../Cargo.toml)
- [Hành vi lưu/publish](../apps/desktop/src/app.rs)
- [Giới hạn OpenAPI](../crates/spec-engine/src/advanced.rs)
- [Giao thức bổ sung](../crates/runner/src/advanced_protocols.rs)
- [Compatibility của script](../crates/runner/src/sandbox.rs)
- [License installer](../installer/LICENSE.txt)
- [Nguồn nhận diện](../assets/figma/SOURCES.json)

Tài liệu chính thức đã tra cứu ngày 26/11/2026:

- [Astro Islands](https://docs.astro.build/en/concepts/islands/): cơ sở cho đề xuất HTML tĩnh và tương tác chọn lọc.
- [Astro i18n routing](https://docs.astro.build/en/guides/internationalization/): tổ chức URL đa ngôn ngữ.
- [Web Vitals](https://web.dev/articles/vitals): ngưỡng và cách đánh giá hiệu năng thực địa.
- [Google SoftwareApplication](https://developers.google.com/search/docs/appearance/structured-data/software-app): dữ liệu có cấu trúc cho ứng dụng; không thay thế bằng chứng sản phẩm hoặc đảm bảo rich result.
