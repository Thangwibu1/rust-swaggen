> Chuyển từ DOCX, giữ nội dung gốc, thứ tự, bảng và mã yêu cầu. Những lựa chọn công nghệ cũ được thay bởi 01-prd/00-authoritative-decisions.md; không dùng bản gốc để ghi đè xác nhận của chủ dự án.

# Đặc tả chức năng ứng dụng cộng tác API

*Windows desktop với lõi Rust*

Phiên bản 1.0  |  Ngày 23 tháng 11 năm 2026  |  160 chức năng đề xuất

Ứng dụng hợp nhất việc thiết kế API, viết tài liệu, gửi request và kiểm thử để một nhóm bạn có thể phát triển phần mềm cùng nhau. Dữ liệu được lưu trên máy và đồng bộ qua máy chủ do nhóm tự quản lý, không bắt buộc dùng tài khoản Postman, SwaggerHub hay dịch vụ cloud của nhà cung cấp.

Phạm vi sản phẩm là ứng dụng Windows desktop. Máy chủ cộng tác là thành phần hạ tầng tự host, có thể chạy trên VPS Linux hoặc máy Windows luôn bật; không phát sinh yêu cầu xây thêm ứng dụng web cho người dùng.

### Kết quả cần đạt

- Một người thiết kế endpoint và mô tả dữ liệu; bạn bè nhận thay đổi trong workspace chung.

- Mỗi thành viên gọi API từ máy của mình, dùng token và môi trường cá nhân.

- Tài liệu OpenAPI, request mẫu, ví dụ response, bình luận và lịch sử thay đổi liên kết với nhau.

- Mất mạng vẫn soạn request và tài liệu; khi nối lại, ứng dụng đồng bộ và chỉ rõ các xung đột.

- Không đặt phí bản quyền theo số ghế trong sản phẩm tự xây. Nhóm vẫn tự chi trả và vận hành hạ tầng nếu dùng VPS.

### Cách đọc tài liệu

Các chức năng được đánh mã để chuyển thành backlog và kiểm thử. Đây là danh mục sản phẩm đề xuất đầy đủ cho nhu cầu nhóm, không phải cam kết tương thích 100 phần trăm với mọi tính năng của Postman hoặc toàn bộ hệ sinh thái Swagger.

| Nhãn | Ý nghĩa |
| --- | --- |
| A | Bản đầu dùng được cho nhóm: bắt buộc có trước khi mời bạn bè dùng hằng ngày. |
| B | Bản mở rộng: hoàn thiện kiểm thử, review và tự động hóa. |
| C | Nâng cao: giao thức bổ sung, đồng biên tập và tích hợp chuyên sâu. |

## 1 Phạm vi và kiến trúc sản phẩm

### Ba thành phần chính

| Thành phần | Trách nhiệm |
| --- | --- |
| Ứng dụng Windows | Giao diện, cơ sở dữ liệu cục bộ, kho token, HTTP engine Rust, hàng đợi đồng bộ. |
| Máy chủ cộng tác Rust | Đăng nhập nội bộ, thành viên, quyền workspace, lưu phiên bản, đồng bộ, bình luận và sự kiện. |
| Cơ sở dữ liệu máy chủ | PostgreSQL được đề xuất cho dữ liệu nhóm. File đính kèm lưu trên ổ đĩa có backup hoặc kho đối tượng tự host khi cần. |

Request thử API mặc định đi thẳng từ máy Windows đến API đích. Máy chủ cộng tác chỉ nhận dữ liệu dự án được chia sẻ; không mặc định nhận token cá nhân, cookie, lịch sử chạy hay toàn bộ response. Localhost của A và localhost của B là hai máy khác nhau. Đồng bộ request không tự làm API của A truy cập được từ máy B.

### Lựa chọn giao diện với Rust

| Phương án | Đánh đổi |
| --- | --- |
| Khuyến nghị Tauri 2 | Lõi Rust, UI TypeScript với React hoặc Svelte, WebView2 trên Windows. Thuận tiện tích hợp editor và thành phần tài liệu web. Đây không phải toàn bộ mã nguồn đều là Rust. [1] |
| Nếu muốn Rust toàn bộ | Có thể đánh giá egui hoặc Slint cho UI. Cần prototype editor JSON/YAML, cây schema, phím tắt và khả năng truy cập trước khi chốt; việc nhúng Swagger UI có thể phức tạp hơn. |

Đề xuất kỹ thuật: Tokio cho tác vụ bất đồng bộ; reqwest cho HTTP; Serde cho trao đổi dữ liệu; SQLite cho lưu cục bộ; Axum cho máy chủ cộng tác; PostgreSQL cho dữ liệu nhóm. Đây là lựa chọn thiết kế, cần kiểm chứng thư viện và giấy phép khi triển khai. reqwest có cơ chế client dùng lại kết nối cùng các tùy chọn body, cookie, proxy và TLS. [2]

Rust giúp kiểm soát tài nguyên và xử lý đồng thời; không tự bảo đảm ứng dụng nhanh hơn mọi đối thủ. Cần phân trang dữ liệu, render danh sách ảo, stream body lớn và đo cả tiến trình WebView2 khi đánh giá bộ nhớ.

## 2 Quyền thành viên và ranh giới dữ liệu

Mỗi workspace có một Owner và các vai trò Admin, Editor, Viewer. Quyền được kiểm tra ở máy chủ với từng thao tác; ẩn nút trong giao diện không thay thế việc kiểm tra quyền.

| Thao tác | Owner | Admin | Editor | Viewer |
| --- | --- | --- | --- | --- |
| Đọc tài liệu và request | Có | Có | Có | Có |
| Chạy API từ máy cá nhân | Có | Có | Có | Có |
| Sửa request và đặc tả chung | Có | Có | Có | Không |
| Bình luận | Có | Có | Có | Có |
| Mời và quản lý Editor Viewer | Có | Có | Không | Không |
| Cấp hoặc gỡ Admin | Có | Không | Không | Không |
| Xuất dữ liệu chia sẻ của workspace | Có | Có | Có | Có |
| Xóa workspace chuyển chủ sở hữu | Có | Không | Không | Không |

Viewer vẫn có thể gọi API và tạo bản nháp cá nhân; quyền này không cho sửa dữ liệu chung. Bất kỳ ai đã đọc dữ liệu đều có thể sao chép nội dung, nên quyền đọc không thể bảo đảm ngăn xuất thông tin bằng mọi cách.

### Phân loại dữ liệu mặc định

| Dữ liệu | Quy tắc |
| --- | --- |
| Chia sẻ trong workspace | Request template, đặc tả, tên biến, giá trị cấu hình không nhạy cảm, ví dụ đã làm sạch, bình luận và phiên bản. |
| Riêng trên máy | Token, mật khẩu, khóa riêng, cookie jar, giá trị biến cá nhân, tab đang mở, lịch sử và kết quả chạy chưa chia sẻ. |
| Chỉ chia sẻ chủ động | Response mẫu, báo cáo kiểm thử, file đính kèm và cấu hình bí mật dùng chung nếu sau này bật vault nhóm. |

Bản A không có vault bí mật dùng chung. Chỉ chia sẻ tên biến và hướng dẫn điền giá trị. Bản C có thể bổ sung vault với phân quyền, quản lý khóa và nhật ký truy cập; không tự nhận là mã hóa đầu cuối nếu chưa xây đúng cơ chế đó.

## 3 Workspace và quản lý dự án

Tạo không gian chung cho từng dự án, giữ tách biệt với dữ liệu cá nhân.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| WS01 | Chế độ cá nhân<br>Mở ứng dụng và làm việc offline không cần tạo tài khoản; tạo workspace cục bộ và chuyển sang workspace nhóm khi muốn. | A |
| WS02 | Kết nối server<br>Nhập địa chỉ máy chủ tự host, kiểm tra TLS và phiên bản giao thức; lưu nhiều cấu hình server và chuyển đổi rõ ràng. | A |
| WS03 | Tài khoản nội bộ<br>Owner tạo lời mời một lần có hạn dùng; thành viên đăng ký, đăng nhập, đổi mật khẩu và đăng xuất thiết bị mà không cần tài khoản dịch vụ ngoài. | A |
| WS04 | Quản lý workspace<br>Tạo, đổi tên, mô tả, lưu trữ workspace; phân tách dự án cá nhân và nhóm trên cùng ứng dụng. | A |
| WS05 | Thành viên và vai trò<br>Mời, chấp nhận, thu hồi lời mời; gán vai trò theo ma trận quyền và xem danh sách thành viên. | A |
| WS06 | Collection và folder<br>Tạo cây collection, folder lồng nhau, kéo thả thứ tự; sao chép hoặc chuyển request với xác nhận đích. | A |
| WS07 | Tìm kiếm và nhãn<br>Tìm theo tên, method, URL, tag và nội dung mô tả; lọc trong workspace hiện tại. | A |
| WS08 | Thiết lập dự án<br>Đặt server API mặc định, môi trường, quy ước đặt tên và hướng dẫn bắt đầu cho thành viên mới. | A |
| WS09 | Quản lý vòng đời<br>Lưu trữ hoặc đưa dữ liệu chung vào thùng rác; khôi phục theo thời hạn lưu giữ và quyền thao tác. | B |
| WS10 | Chuyển chủ sở hữu<br>Owner chuyển quyền cho thành viên phù hợp; không cho rời workspace khi chưa có người nhận quyền. | B |

## 4 Soạn và gửi HTTP request

Đây là phần request builder kết hợp với dữ liệu từ tài liệu API.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| RQ01 | Method và URL<br>Hỗ trợ GET POST PUT PATCH DELETE HEAD OPTIONS; nhập URL, path params và query params, xem URL thực tế trước khi gửi. | A |
| RQ02 | Query chính xác<br>Bật tắt từng tham số, giữ thứ tự và tham số trùng tên; kiểm soát encoding để không làm thay đổi ý nghĩa request. | A |
| RQ03 | Headers<br>Thêm, sửa, bật tắt header; xem header tự sinh, xử lý giá trị trùng theo quy tắc HTTP và báo giá trị không hợp lệ. | A |
| RQ04 | Các loại body<br>Soạn JSON, text, XML, form URL encoded, multipart và binary; chọn Content Type và báo lỗi JSON trước khi gửi. | A |
| RQ05 | Tải file lên<br>Chọn file từ máy, nhập trường multipart và tên file; đường dẫn riêng không đồng bộ thành file của người khác. | A |
| RQ06 | Gửi và hủy<br>Gửi bất đồng bộ, đặt timeout, hủy request; phân biệt hủy bởi người dùng, lỗi mạng và HTTP trả lỗi. | A |
| RQ07 | Request đã phân giải<br>Xem method URL headers body sau khi thay biến; che secret theo mặc định và cảnh báo thiếu biến. | A |
| RQ08 | Nháp và lưu<br>Tự lưu bản nháp riêng, đánh dấu khác biệt so với bản chung; Publish mới tạo thay đổi cho nhóm. | A |
| RQ09 | Sao chép nhanh<br>Nhân bản request, lưu thành mẫu mới và mở request từ lịch sử mà không sửa bản chung ngoài ý muốn. | A |
| RQ10 | Stream khi truyền<br>Hiển thị tiến độ upload download, giữ UI phản hồi; ghi dữ liệu lớn xuống file thay vì đưa toàn bộ vào RAM. | B |

## 5 Xác thực và kết nối mạng

Cấu hình xác thực là mẫu chia sẻ; thông tin nhạy cảm là giá trị riêng.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| AU01 | No Auth và Bearer<br>Chọn không xác thực hoặc Bearer token bằng biến cá nhân; token được che trong giao diện và log. | A |
| AU02 | Basic và API key<br>Basic Auth, API key trong header hoặc query; cảnh báo URL có secret và loại bỏ secret khi xuất mặc định. | A |
| AU03 | Kế thừa xác thực<br>Request kế thừa từ folder hoặc collection; có thể ghi đè hoặc tắt và xem nguồn cấu hình đang áp dụng. | A |
| AU04 | Cookie jar<br>Lưu, xem, sửa, xóa cookie theo domain path và expiry; tách theo workspace hoặc môi trường cá nhân. | A |
| AU05 | TLS mặc định<br>Kiểm tra chứng chỉ theo mặc định; cho phép ngoại lệ riêng theo host với cảnh báo rõ và không áp dụng toàn cục. | A |
| AU06 | Redirect<br>Cấu hình theo hoặc dừng redirect, giới hạn số lần; không chuyển Authorization sang host khác ngoài chính sách cho phép. | A |
| AU07 | Proxy và CA riêng<br>Cấu hình proxy, bypass, CA của tổ chức; không ghi mật khẩu proxy vào dữ liệu chung. | B |
| AU08 | OAuth 2 PKCE<br>Authorization Code với PKCE, Client Credentials, scope và token refresh; callback loopback được xác thực trạng thái. | B |
| AU09 | Chứng chỉ client<br>Cấu hình mTLS theo host và port; khóa riêng giữ trên máy, hỗ trợ xóa liên kết và báo hết hạn. | B |
| AU10 | Xác thực bổ sung<br>Đánh giá Digest, AWS Signature và cơ chế khác theo nhu cầu; hiển thị rõ loại nào chưa được hỗ trợ. | C |

## 6 Biến môi trường và bí mật

Tên biến được chia sẻ để mọi người hiểu cấu hình; giá trị cá nhân được bảo vệ.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| EV01 | Môi trường<br>Tạo local dev staging prod; hiển thị môi trường đang dùng cạnh nút gửi request. | A |
| EV02 | Biến theo cấp<br>Hỗ trợ workspace, collection, environment và request; trình kiểm tra chỉ rõ giá trị hiệu lực và nguồn. | A |
| EV03 | Giá trị cá nhân<br>Đặt local override cho biến chung; bạn A và B có token khác nhau nhưng dùng cùng request template. | A |
| EV04 | Thay biến<br>Dùng cú pháp {{ten_bien}} trong URL header body auth; phát hiện thiếu biến và vòng tham chiếu. | A |
| EV05 | Đánh dấu secret<br>Đánh dấu nhạy cảm, che giá trị và lưu bằng cơ chế bảo vệ Windows; chỉ Rust core truy xuất khi cần gửi. | A |
| EV06 | Quy tắc xuất<br>Mặc định xuất placeholder và tên biến; có bước xem trước nếu người dùng chủ động đưa giá trị nhạy cảm vào file. | A |
| EV07 | Bảo vệ production<br>Màu môi trường khác biệt; yêu cầu xác nhận khi gửi request thay đổi dữ liệu đến môi trường đánh dấu production. | A |
| EV08 | Biến động<br>Sinh UUID timestamp và giá trị ngẫu nhiên có quy tắc; preview trước khi gửi để dễ tái hiện. | B |
| EV09 | Trích xuất giá trị<br>Lấy trường JSON hoặc header từ response để điền biến cục bộ cho request tiếp theo; không tự publish token. | B |
| EV10 | Vault nhóm<br>Kho secret riêng có quyền truy cập, luân chuyển khóa, nhật ký; thiết kế và nghiệm thu riêng trước khi bật. | C |

## 7 Response và chẩn đoán

Giúp thành viên hiểu API trả gì và phân biệt lỗi API với lỗi kết nối.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| RS01 | Thông tin tổng quan<br>Hiển thị status code, thời gian tổng, kích thước và thời điểm chạy; phân biệt body nén và giải nén nếu có số đo. | A |
| RS02 | Hiển thị body<br>Raw text, JSON có tô màu và gấp nhánh, XML text; nhận diện binary và cho tải file xuống. | A |
| RS03 | Response headers<br>Xem toàn bộ header, gồm các header lặp; xem cookie do server trả về. | A |
| RS04 | Tìm và sao chép<br>Tìm kiếm trong body, sao chép giá trị hoặc JSON path, format và lưu response thành file. | A |
| RS05 | Thông báo lỗi<br>Phân loại DNS, connection refused, TLS, timeout, hủy và lỗi parse; hướng dẫn xử lý theo tình huống. | A |
| RS06 | Response lớn<br>Giới hạn phần preview, phân trang hoặc render ảo; vẫn cho lưu raw file và thông báo khi nội dung bị cắt hiển thị. | A |
| RS07 | Lịch sử riêng<br>Lưu lần chạy trên máy với bộ lọc và thời hạn giữ; xóa theo request hoặc toàn bộ mà không xóa template. | A |
| RS08 | Ví dụ chia sẻ<br>Lưu response thành example sau khi xem trước và làm sạch dữ liệu; liên kết với endpoint và status tương ứng. | A |
| RS09 | So sánh response<br>Diff JSON hoặc text giữa hai lần chạy, bỏ qua trường biến động theo cấu hình của người dùng. | B |
| RS10 | Đo thời gian chi tiết<br>Đo DNS TCP TLS TTFB khi transport cung cấp được; ghi không khả dụng thay vì ước đoán thành số đo thật. | B |

## 8 Thiết kế đặc tả OpenAPI

OpenAPI là hợp đồng API chính; hỗ trợ phiên bản phải được khai báo cụ thể.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| OA01 | Import đặc tả<br>Mở JSON YAML từ file hoặc URL; ưu tiên OpenAPI 3.0 và 3.1, báo phiên bản không hỗ trợ; URL riêng lấy từ máy người dùng. | A |
| OA02 | Editor đặc tả<br>Tô màu, format, tìm thay thế, đánh dấu dòng lỗi; giữ nguyên bản nháp khi validation thất bại. | A |
| OA03 | Kiểm tra cấu trúc<br>Kiểm tra cú pháp, trường bắt buộc, path params và operationId; không báo đạt khi thư viện không hiểu keyword. | A |
| OA04 | Quản lý endpoint<br>Thêm method path summary description tags và deprecated; có kiểm tra trùng method cùng path. | A |
| OA05 | Định nghĩa dữ liệu<br>Khai báo parameters requestBody responses content types và examples; phân biệt required nullable và optional theo phiên bản. | A |
| OA06 | Components dùng lại<br>Quản lý schema parameters responses và security schemes; giải tham chiếu nội bộ và phát hiện tham chiếu không tồn tại. | A |
| OA07 | Schema phức tạp<br>Hiển thị allOf oneOf anyOf enum discriminator và đệ quy với giới hạn; không render vô hạn khi có vòng tham chiếu. | B |
| OA08 | Tham chiếu nhiều file<br>Giải relative reference trong project, bundle khi export; tham chiếu URL ngoài cần cho phép rõ và giới hạn tài nguyên. | B |
| OA09 | Swagger 2 và phiên bản mới<br>Chuyển Swagger 2.0 bằng công cụ phù hợp, đánh dấu phần mất mát; đánh giá OpenAPI 3.2 riêng, không mặc định coi tương thích. | B |
| OA10 | Lint quy ước nhóm<br>Quy tắc đặt operationId, mô tả, error schema và security; phân biệt lỗi đặc tả với cảnh báo quy ước. | B |

## 9 Tài liệu và liên kết request

Một endpoint có thể có nhiều request ví dụ; mọi thay đổi hợp đồng đều có phiên bản.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| DC01 | Tài liệu tự sinh<br>Hiển thị endpoint theo tag, method, path và mô tả từ đặc tả; mở được offline trong app. | A |
| DC02 | Chi tiết hợp đồng<br>Hiển thị params required type default enum, body schema, status và ví dụ; tìm theo endpoint hoặc schema. | A |
| DC03 | Chạy từ tài liệu<br>Try request mở request builder cùng environment auth và path params; gọi bằng Rust engine trên máy. | A |
| DC04 | Liên kết định danh<br>Lưu ID endpoint ổn định và ánh xạ operationId khi có; đổi path không tự tạo request trùng hoặc làm mất liên kết. | A |
| DC05 | Sinh request mẫu<br>Tạo request từ operation và example; đánh dấu trường do spec quản lý và phần override của người dùng. | A |
| DC06 | Cập nhật từ đặc tả<br>Khi spec đổi, hiển thị diff và đề xuất cập nhật request; giữ token cục bộ và báo override không còn phù hợp. | A |
| DC07 | Đề xuất sửa hợp đồng<br>Từ request thủ công, tạo draft patch cho spec; người dùng review trước publish, không tự suy ra hợp đồng đầy đủ từ một response. | B |
| DC08 | Kiểm tra response theo schema<br>Đối chiếu status content type và schema; báo vị trí dữ liệu sai, keyword chưa hỗ trợ và phiên bản schema. | B |
| DC09 | Chênh lệch hợp đồng<br>Phát hiện request hoặc test còn tham chiếu endpoint cũ; liệt kê phần cần sửa khi xóa hoặc đổi operation. | B |
| DC10 | Xuất tài liệu<br>Xuất HTML tự chứa hoặc Markdown, có bước làm sạch secret; tạo snapshot gắn version để chia sẻ ngoài app. | B |

## 10 Cộng tác và trao đổi trong nhóm

Bản A có chia sẻ thay đổi đáng tin cậy; đồng gõ cùng một tài liệu thuộc bản C.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| CO01 | Workspace chung<br>Thành viên xem cùng collection spec và cấu hình công khai qua server tự host; không có giới hạn số ghế do mô hình tính phí của app. | A |
| CO02 | Publish thay đổi<br>Lưu nháp riêng rồi publish nội dung chung với ghi chú; người khác nhận đúng phiên bản đã công bố. | A |
| CO03 | Thông báo thay đổi<br>Hiện endpoint hoặc request vừa được sửa, tác giả và thời gian; refresh bằng sự kiện server cùng cơ chế lấy bù. | A |
| CO04 | Bình luận theo đối tượng<br>Thread tại endpoint request hoặc schema, trả lời và đánh dấu đã xử lý; lưu liên kết tới revision được thảo luận. | A |
| CO05 | Nhắc thành viên<br>Mention thành viên tạo thông báo trong app; không bắt buộc email hay dịch vụ nhắn tin ngoài. | B |
| CO06 | Luồng hoạt động<br>Xem ai tạo sửa xóa publish và thay quyền; lọc theo người hoặc đối tượng trong workspace. | A |
| CO07 | Trạng thái chỉnh sửa<br>Hiện ai đang mở hoặc sửa đối tượng khi online; đây là tín hiệu hỗ trợ, không thay thế kiểm soát xung đột. | B |
| CO08 | Chia sẻ kết quả chạy<br>Chủ động gửi báo cáo hoặc example đã làm sạch vào workspace; người nhận không thấy token cá nhân. | B |
| CO09 | Liên kết nội bộ<br>Copy deep link tới workspace endpoint và comment; mở trong app, luôn yêu cầu đăng nhập và kiểm tra quyền. | B |
| CO10 | Đồng biên tập<br>Cùng sửa một nội dung với CRDT hoặc OT, con trỏ và undo phù hợp; phải hỗ trợ offline và thu hồi quyền trước phát hành. | C |

## 11 Đồng bộ offline và phiên bản

Không chấp nhận ghi đè âm thầm khi hai thành viên sửa cùng dữ liệu.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| SY01 | Lưu cục bộ trước<br>Ghi nháp và thao tác chờ bằng transaction SQLite; crash không được làm mất thay đổi đã báo lưu thành công. | A |
| SY02 | Hàng đợi đồng bộ<br>Có pending syncing synced conflict failed; cho retry và xem nguyên nhân, không dùng một biểu tượng xanh mơ hồ. | A |
| SY03 | Phiên bản đối tượng<br>Publish gửi base revision; server chỉ chấp nhận khi revision hợp lệ và trả conflict nếu dữ liệu đã đổi. | A |
| SY04 | Giải quyết xung đột<br>Hiện bản gốc bản của tôi và bản server; chọn giữ hoặc ghép, preview rồi publish lại; giữ bản thua làm bản nháp. | A |
| SY05 | Sự kiện lấy bù<br>Client giữ cursor; reconnect lấy thay đổi bỏ lỡ, cursor hết hạn thì lấy snapshot rồi áp lại thao tác còn chờ. | A |
| SY06 | Chống nhân đôi<br>Mỗi thao tác có idempotency key; retry hoặc mất ACK không tạo request comment hay revision trùng. | A |
| SY07 | Xóa và sửa đồng thời<br>Tombstone cho xóa; không tự hồi sinh dữ liệu khi thiết bị offline đẩy bản sửa cũ lên. | A |
| SY08 | Lịch sử phiên bản<br>Xem diff tác giả thời gian và ghi chú; restore bằng revision mới, không xóa lịch sử đã công bố. | A |
| SY09 | Thu hồi quyền khi offline<br>Server từ chối publish sau khi bị gỡ quyền; cho giữ nháp riêng. Không hứa xóa được mọi bản sao đã tải xuống máy. | A |
| SY10 | Nhánh và review<br>Fork hoặc branch dự án, gửi đề nghị hợp nhất, review và kiểm tra xung đột trước merge. | B |

## 12 Import export và chuyển đổi dữ liệu

Dữ liệu phải mang đi được mà không lệ thuộc server đang chạy.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| IM01 | Định dạng project mở<br>Xuất project theo schema có version, ID ổn định và manifest; nêu rõ các file chứa spec request và môi trường công khai. | A |
| IM02 | Postman Collection<br>Import Collection v2.1 với folder request variables auth và examples; xuất phần hỗ trợ được và kèm báo cáo tương thích. | A |
| IM03 | Postman Environment<br>Import tên và cấu trúc biến, cho xem trước giá trị; không tự đồng bộ token được nhập vào server nhóm. | A |
| IM04 | OpenAPI import export<br>Giữ JSON YAML và các extension chưa hiểu khi có thể; mọi chuyển đổi mất dữ liệu phải báo rõ. | A |
| IM05 | cURL hai chiều<br>Dán lệnh để tạo request, sinh lệnh cURL từ request; parse không thực thi shell và xử lý khác biệt quoting Windows. | A |
| IM06 | Xem trước import<br>Liệt kê đối tượng mới, trùng, bị bỏ qua và phần không hỗ trợ; chọn tạo project mới hoặc nhập vào project có sẵn. | A |
| IM07 | File đính kèm<br>Chọn đóng gói file dùng chung, giới hạn kích thước và hash; không tự thu thập file từ đường dẫn riêng của thành viên. | B |
| IM08 | Xuất dữ liệu đã làm sạch<br>Preview tên biến giá trị auth examples và file; mặc định loại secret cùng lịch sử cá nhân. | A |
| IM09 | Tích hợp Git<br>Lưu project dạng text có thứ tự ổn định, xem diff và merge; cho dùng Git server tự host hoặc Git cục bộ. | B |
| IM10 | Định dạng bổ sung<br>Đánh giá HAR HTTP files và công cụ khác; chỉ ghi tương thích cho tập tính năng đã kiểm thử. | C |

## 13 Kiểm thử và chạy collection

Bản đầu xác minh đơn giản không cần chạy mã JavaScript tùy ý.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| TS01 | Assertion cơ bản<br>Kiểm tra status header thời gian và JSON path bằng form; lưu expected values với request. | A |
| TS02 | Collection runner<br>Chạy tuần tự request trong folder hoặc collection, chọn environment số vòng và khoảng nghỉ; dừng khi người dùng hủy. | B |
| TS03 | Chuỗi phụ thuộc<br>Trích token hoặc ID từ response cho bước kế; quy tắc fail fast hoặc tiếp tục được cấu hình rõ. | B |
| TS04 | Chạy theo dữ liệu<br>Nhập CSV JSON làm data set, xem trước kiểu và dòng lỗi; dữ liệu nhạy cảm không tự publish. | B |
| TS05 | Báo cáo kết quả<br>Hiển thị pass fail skipped cùng assertion và thời gian; chỉ rõ lỗi hạ tầng khác với assertion thất bại. | B |
| TS06 | Kiểm thử hợp đồng<br>Chạy validation OpenAPI với status media type và schema tương ứng; liệt kê giới hạn validator. | B |
| TS07 | Pre request và test script<br>Runtime JavaScript cách ly, giới hạn thời gian bộ nhớ và API; không cho script đọc file hay chạy shell mặc định. | C |
| TS08 | Tương thích script Postman<br>Công bố tập pm API hỗ trợ, migration report và case lỗi; không tuyên bố tương thích toàn bộ chỉ vì chạy được JavaScript. | C |
| TS09 | Chạy song song có giới hạn<br>Giới hạn concurrency và rate, chỉ chạy request độc lập; không coi đây là công cụ load test chuyên dụng. | C |
| TS10 | Kết quả tái hiện<br>Gắn run với revision project, môi trường công khai và phiên bản engine; không lưu token thật vào báo cáo. | B |

## 14 Mock server CLI và tự động hóa

Tác vụ chạy khi tắt app cần runner hoặc server luôn bật.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| MK01 | Mock từ example<br>Sinh response mẫu theo endpoint status và media type; chạy local bằng cổng được chọn. | B |
| MK02 | Mock từ schema<br>Sinh dữ liệu mẫu có seed; báo schema không sinh được và cho thay bằng example thủ công. | B |
| MK03 | Kịch bản mock<br>Match query header body, giả lập độ trễ và lỗi; ưu tiên rule rõ ràng, có log đã làm sạch. | C |
| MK04 | Mock dùng chung<br>Host mock ở server nhóm, đặt quyền hoặc token truy cập; không tự mở cổng máy cá nhân ra Internet. | B |
| MK05 | CLI Rust<br>Chạy collection từ project export trên Windows, tham số môi trường và biến từ secret store hoặc biến tiến trình. | B |
| MK06 | Kết quả cho CI<br>Exit code theo thành công thất bại, xuất JSON JUnit; che secret trong stdout stderr và report. | B |
| MK07 | Runner tự host<br>Đăng ký runner với workspace và quyền hạn tối thiểu; tách mạng runner khỏi quyền quản trị server. | C |
| MK08 | Lịch chạy kiểm tra<br>Lịch chạy múi giờ cấu hình, retry có giới hạn; chỉ hoạt động khi runner được chỉ định online. | C |
| MK09 | Thông báo lỗi chạy<br>Thông báo trong app từ runner; webhook tới hệ thống nhóm tự chọn là tùy chọn. | C |
| MK10 | Quản lý tài nguyên<br>Giới hạn số job body retention và log; owner có thể hủy lịch hoặc job để tránh quá tải server. | B |

## 15 Giao thức và công cụ mở rộng

REST là lõi; giao thức khác được đưa vào sau khi cộng tác hoạt động ổn định.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| PR01 | GraphQL cơ bản<br>Editor query variables và headers; hiển thị data errors, lưu request trong collection. | B |
| PR02 | GraphQL schema<br>Import SDL hoặc introspection được cho phép; gợi ý trường và kiểm tra query theo schema. | C |
| PR03 | WebSocket<br>Kết nối URL auth, gửi nhận message, ngắt kết nối; lưu message mẫu và giới hạn bộ nhớ lịch sử. | C |
| PR04 | Server Sent Events<br>Hiển thị event stream theo thời gian, hủy và cấu hình reconnect; không giữ vô hạn nội dung trong UI. | B |
| PR05 | gRPC<br>Import proto, gọi unary trước; hiển thị metadata status và lỗi; streaming là khả năng phát hành riêng. | C |
| PR06 | HTTP nâng cao<br>Chọn policy HTTP 1.1 hoặc 2 khi transport hỗ trợ; HTTP 3 cần đánh giá và không nằm trong bản A. | C |
| PR07 | Sinh code gọi API<br>Sinh snippet cURL Rust reqwest JavaScript fetch Dart và Go; placeholder thay cho secret và không tự thực thi. | B |
| PR08 | Sinh SDK và stub<br>Gọi generator theo phiên bản được ghim, chọn ngôn ngữ và xem output; dependency ngoài phải được thông báo. | C |
| PR09 | Giải mã và chuyển đổi<br>URL encode decode Base64 JSON format và chuyển JSON YAML; chạy cục bộ và không gửi dữ liệu ra dịch vụ ngoài. | B |
| PR10 | Extension có quyền<br>Plugin được bật theo quyền mạng file secret, cách ly và có thể gỡ; không cho plugin chạy với mọi quyền mặc định. | C |

## 16 Trải nghiệm Windows desktop

Tối ưu cho bàn phím, nhiều tab và màn hình làm việc của lập trình viên.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| UX01 | Bố cục làm việc<br>Sidebar project, tab request hoặc spec, vùng response; kéo đổi kích thước và nhớ bố cục từng máy. | A |
| UX02 | Tab và khôi phục<br>Mở nhiều tab, ghim và đóng nhóm; khôi phục nháp và tab sau restart hoặc crash. | A |
| UX03 | Phím tắt<br>Ctrl Enter gửi, Ctrl S lưu nháp, Ctrl P tìm nhanh; xem danh sách phím và tránh xung đột khi đang gõ trong editor. | A |
| UX04 | Command palette<br>Tìm hành động, đổi environment, mở request và chuyển workspace qua bàn phím. | B |
| UX05 | Hiển thị Windows<br>Hỗ trợ DPI scaling nhiều màn hình, resize và cửa sổ nhỏ; chữ và control không bị cắt ở mức zoom thông dụng. | A |
| UX06 | Theme và ngôn ngữ<br>Theme sáng tối, lựa chọn tiếng Việt hoặc tiếng Anh; giữ nguyên tên biến và nội dung API của người dùng. | B |
| UX07 | Khả năng truy cập<br>Focus rõ, điều hướng bàn phím và nhãn control; prototype kiểm tra screen reader trên UI được chọn. | B |
| UX08 | Mở file và liên kết<br>Mở file project hoặc đặc tả từ Explorer, kéo thả import; deep link không tự chạy request hay lệnh hệ thống. | B |
| UX09 | Trạng thái kết nối<br>Hiện server đang dùng, offline và số thay đổi chờ; lỗi sync không chặn soạn request cục bộ. | A |
| UX10 | Quản lý dữ liệu máy<br>Xem dung lượng history cache downloads, đổi thư mục dữ liệu và đặt retention; dọn cache không xóa project hoặc secret. | A |

## 17 Bảo mật và quyền riêng tư

App xử lý dữ liệu API thật nên các kiểm soát này nằm trong yêu cầu chức năng.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| SC01 | Kho bảo mật Windows<br>Lưu token khóa và session bằng cơ chế Windows phù hợp; không coi SQLite thường là kho bí mật đã mã hóa. | A |
| SC02 | Làm sạch dữ liệu<br>Che auth cookie password và trường được chỉ định trong log report export; preview lại vì không thể nhận diện mọi secret tự động. | A |
| SC03 | Đăng nhập máy chủ<br>Hash mật khẩu bằng thuật toán phù hợp, TLS, session có hạn và thu hồi; rate limit đăng nhập và token mời một lần. | A |
| SC04 | Cách ly workspace<br>Kiểm tra quyền ở mọi API tài nguyên file và kênh sự kiện; ID bị đoán được cũng không cho vượt workspace. | A |
| SC05 | Nội dung không tin cậy<br>Không thực thi HTML hoặc script từ response Markdown hay example; khử nội dung nguy hiểm trong renderer. | A |
| SC06 | Quyền ứng dụng desktop<br>Giới hạn IPC command và phạm vi file; không cấp quyền Rust hoặc hệ thống cho trang remote trong WebView. [3] | A |
| SC07 | Không tự gửi telemetry<br>Mặc định tắt analytics và báo crash qua dịch vụ ngoài; diagnostic bundle được xem trước và gửi chủ động. | A |
| SC08 | Chặn SSRF trên server<br>Mock runner fetch spec hoặc webhook ở server phải có allowlist và kiểm tra IP DNS redirect; client cá nhân vẫn gọi localhost theo chủ đích. | B |
| SC09 | Phục hồi tài khoản và MFA<br>Owner quản lý reset an toàn; bổ sung TOTP hoặc giải pháp nội bộ, có recovery code và thu hồi session. | B |
| SC10 | Audit và giới hạn lưu<br>Ghi thao tác quản trị publish restore và quyền; đặt retention, export audit và không hứa log bất biến nếu chưa có cơ chế chống sửa. | B |

## 18 Cài đặt vận hành và sao lưu

Nhóm tự chủ triển khai nhưng cần khả năng phục hồi và nâng cấp dữ liệu.

| Mã | Chức năng và hành vi cần có | Bản |
| --- | --- | --- |
| OP01 | Bộ cài Windows<br>Đóng gói MSI hoặc EXE, kiểm tra prerequisite gồm WebView2 nếu dùng Tauri; bản đầu nhắm Windows 11 x64. | A |
| OP02 | Chế độ không có cloud<br>App cá nhân chạy offline; bản nhóm chỉ kết nối địa chỉ self host được cấu hình, không cần activation server bên ngoài. | A |
| OP03 | Khởi tạo server<br>Cung cấp cấu hình Docker Compose hoặc binary, tạo Owner lần đầu và vô hiệu cơ chế bootstrap sau khi xong. | A |
| OP04 | Cấu hình triển khai<br>Biến cấu hình domain TLS DB data path và giới hạn tải; hỗ trợ reverse proxy và volume bền vững. | A |
| OP05 | Sao lưu server<br>Backup nhất quán database file và metadata phiên bản; xuất manifest kiểm tra và cấu hình lịch backup do nhóm quản lý. | A |
| OP06 | Khôi phục dữ liệu<br>Restore sang server sạch, kiểm tra liên kết attachment và version; quy trình xử lý mất key hoặc thiếu file được ghi rõ. | A |
| OP07 | Nâng cấp và migration<br>Migration có version và backup trước nâng cấp; kiểm tra tương thích client server, không hạ DB bằng thao tác thiếu kiểm soát. | A |
| OP08 | Phân phối cập nhật<br>Bản đầu cập nhật thủ công; bản sau hỗ trợ feed tự host và xác minh chữ ký, không tin file cập nhật không được kiểm tra. | B |
| OP09 | Theo dõi sức khỏe<br>Health endpoint, log có request ID, dung lượng DB file và hàng đợi; log không chứa body hoặc secret mặc định. | A |
| OP10 | Quota vận hành<br>Đặt giới hạn upload retention và số job theo năng lực máy chủ; đây là giới hạn tài nguyên do nhóm quản trị, không phải phí theo ghế. | B |

## 19 Quy tắc đồng bộ và hợp đồng dữ liệu

### Luồng lưu và chia sẻ

Sửa nội dung → lưu nháp trên máy → chọn Publish → gửi operation ID cùng base revision → server kiểm tra quyền và revision → lưu transaction và phát sự kiện → thiết bị khác lấy phiên bản mới. UI chỉ báo đã đồng bộ sau khi nhận xác nhận hợp lệ từ server.

### Quy tắc bắt buộc

- Không dùng last write wins để âm thầm giải quyết nội dung API. Bản A từ chối cả bản sửa khi revision cũ; ghép từng trường là nâng cấp sau.

- Mọi retry dùng lại operation ID. Tạo revision, đổi dữ liệu và ghi sự kiện đồng bộ phải có ranh giới transaction nhất quán.

- Xung đột xóa và sửa phải có màn hình xử lý. Thiết bị offline không được làm dữ liệu đã xóa sống lại chỉ vì reconnect.

- Client mới quá hoặc cũ quá phải nhận thông báo tương thích. Bản local vẫn đọc được và dữ liệu chưa publish vẫn xuất được.

- Đối với file, upload hoàn tất và kiểm tra hash trước khi publish tham chiếu; file thừa được dọn sau thời gian an toàn.

- Máy chủ là nơi quyết định quyền và thứ tự phiên bản; không dùng đồng hồ máy thành viên để xác định ai thắng.

### Quan hệ giữa tài liệu và request

Spec revision là nguồn hợp đồng API. Một operation liên kết nhiều request example và nhiều test case. Request có thể override body header hoặc URL cho thử nghiệm; override không tự sửa spec. Khi spec đổi, ứng dụng tính diff và yêu cầu chọn cập nhật, giữ override hoặc bỏ liên kết.

Định dạng OpenAPI không chứa đầy đủ ngữ nghĩa collection runner, test script hay workflow. Project export phải có định dạng riêng cho các dữ liệu đó, kèm file OpenAPI chuẩn để dùng với công cụ khác. [4]

## 20 Mô hình dữ liệu cốt lõi

Mô hình dưới đây xác định những thực thể cần có để tránh gộp nhầm dữ liệu riêng và dữ liệu nhóm. Đây là thiết kế logic, chưa phải schema SQL cuối cùng.

| Thực thể | Dữ liệu và quan hệ chính |
| --- | --- |
| ServerProfile và User | Địa chỉ server, user ID, thông tin hồ sơ; session token chỉ lưu trong kho bảo mật máy. |
| Workspace và Membership | Workspace ID, owner, vai trò, trạng thái và revision quyền; một người có nhiều workspace. |
| Collection Folder Request | Cây tổ chức và request template; ID ổn định, parent ID, thứ tự và revision. |
| ApiSpec và OperationLink | Spec JSON YAML, phiên bản OpenAPI, revision, operation ID nội bộ và liên kết request. |
| Environment Variable | Tên biến, cấp áp dụng, shared value không nhạy cảm; local override và SecretRef lưu riêng. |
| Example và Attachment | Response mẫu đã làm sạch, content type, hash, kích thước, quyền và tham chiếu file. |
| Comment và Notification | Thread, tác giả, trạng thái, đối tượng cùng revision; trạng thái đã đọc theo thành viên. |
| Revision và ChangeEvent | Snapshot hoặc patch, base revision, tác giả, operation ID và cursor đồng bộ. |
| LocalDraft và Outbox | Bản gốc, bản sửa, trạng thái retry và lỗi; lưu cục bộ bằng transaction. |
| History CookieJar và Run | Dữ liệu chạy riêng; chỉ tạo SharedRunReport khi người dùng chủ động chia sẻ. |
| TestCase và RunnerJob | Assertion, dữ liệu chạy, revision project, cấu hình job và kết quả đã làm sạch. |
| Tombstone và AuditEvent | Đánh dấu xóa, thời hạn giữ, thao tác quản trị và thông tin phục hồi. |

## 21 Các luồng sử dụng quan trọng

### Luồng một tạo dự án với bạn bè

A chạy server tự host, tạo workspace rồi tạo lời mời. B cài ứng dụng Windows, nhập địa chỉ server và dùng lời mời. A import đặc tả, sinh collection, khai báo base_url và token dưới dạng tên biến. B điền giá trị riêng, chọn môi trường và gửi request. Không có bước trả phí hoặc tạo tài khoản cloud của bên cung cấp công cụ.

### Luồng hai backend và frontend thống nhất hợp đồng

A thêm POST /tasks vào bản nháp đặc tả, ghi body response và ví dụ rồi publish. B nhận thay đổi, mở tài liệu, tạo request từ endpoint và để lại bình luận về trường due_at. A sửa spec, publish revision mới; B xem diff và cập nhật request đã liên kết. Từ bản B, B có thể dùng mock dùng chung trong lúc backend chưa sẵn sàng.

### Luồng ba hai người cùng chỉnh sửa

A và B cùng bắt đầu từ revision 12. A publish được revision 13. B gửi base revision 12 thì bị từ chối và nhận bản server; B xem ba phiên bản, ghép thay đổi rồi publish từ revision 13. Cả bản nháp của B và lịch sử revision 12 vẫn còn để đối chiếu.

### Luồng bốn làm việc khi mất mạng

B sửa request và spec khi offline, các mục hiện pending. B vẫn gọi API truy cập được từ mạng của mình. Khi có mạng, ứng dụng xác thực lại, lấy sự kiện còn thiếu rồi publish phần hợp lệ; mục xung đột được giữ lại để xử lý. Nếu B đã bị gỡ quyền, thay đổi không được đẩy lên workspace.

### Luồng năm chia sẻ API trên máy cá nhân

Nếu API của A chỉ nghe ở localhost thì B không gọi được bằng URL localhost của B. Nhóm phải triển khai backend chung hoặc cấu hình LAN VPN hay kết nối riêng phù hợp. Ứng dụng chỉ chia sẻ cấu hình địa chỉ sau khi nhóm thiết lập khả năng truy cập; không tự mở tunnel ra bên ngoài.

### Luồng sáu chuyển server và rời công cụ

Owner export project hoặc tạo backup server. Trên server mới, restore bằng bộ dữ liệu được kiểm tra; các client đổi server theo hướng dẫn và xác nhận định danh workspace. Mọi người vẫn mở được OpenAPI và dữ liệu export mà không cần server cũ hoạt động.

## 22 Tiêu chí nghiệm thu bản đầu cho nhóm

Bản A được xem là hoàn thành khi đáp ứng tất cả tình huống dưới đây trên hai máy Windows và một máy chủ thử nghiệm. Số liệu hiệu năng là mục tiêu đề xuất, cần đo lại sau prototype và ghi cấu hình máy vào báo cáo.

| Mã | Điều kiện nghiệm thu |
| --- | --- |
| AC01 | A mời B vào workspace; B thấy collection và spec; Viewer không thể sửa qua UI hoặc gọi trực tiếp API server. |
| AC02 | A publish thay đổi, B online nhận trong mục tiêu 3 giây ở mạng ổn định, không tính thời gian truyền file lớn. |
| AC03 | Hai người sửa cùng revision phải xuất hiện conflict; không mất bản nháp và không ghi đè âm thầm. |
| AC04 | Mất ACK rồi gửi lại cùng operation ID chỉ sinh một thay đổi; reconnect lấy đủ sự kiện bị bỏ lỡ. |
| AC05 | Offline tạo sửa dữ liệu, restart app rồi reconnect vẫn còn pending; thu hồi quyền phải chặn publish. |
| AC06 | Import spec thử nghiệm 3.0 và 3.1, giải ref nội bộ, sinh request và cập nhật theo diff; tính năng chưa hỗ trợ được báo. |
| AC07 | Token của A khác B; không tìm thấy token cá nhân trong payload đồng bộ, export mặc định, log hoặc example chia sẻ. |
| AC08 | Server thử nghiệm xác minh query trùng, encoding, JSON multipart cookies redirect timeout và cancel. |
| AC09 | Body 20 MB không làm UI treo; preview có giới hạn và lưu raw file được. Hủy stream kết thúc tác vụ và giải phóng tài nguyên. |
| AC10 | Dự án 1.000 request mở và tìm kiếm dùng được; mục tiêu tương tác thường dưới 200 ms trên máy 4 core RAM 8 GB SSD. |
| AC11 | Backup rồi restore trên server sạch giữ số đối tượng revision quyền và hash file; có hướng dẫn xử lý khác biệt. |
| AC12 | Cài trên Windows 11 x64 sạch, xử lý WebView2 nếu cần; app cá nhân chạy offline và không cần cloud activation. |

Bổ sung kiểm thử cách ly workspace, nội dung HTML độc hại, file import lỗi, attachment quá lớn, hết dung lượng và migration thất bại. Đo bộ nhớ của Rust core cùng toàn bộ WebView2 liên quan; không công bố mức RAM cố định trước khi có benchmark.

## 23 Kế hoạch triển khai với AI coding

Đi theo từng luồng xuyên suốt UI Rust DB và sync. Không để AI dựng hết giao diện trước rồi mới giải quyết cộng tác, vì mô hình lưu trữ và revision ảnh hưởng đến gần như mọi màn hình.

| Mốc | Kết quả cần bàn giao |
| --- | --- |
| M0 Prototype | Chọn Tauri hoặc UI Rust thuần; thử editor JSON YAML, response lớn, phím tắt, Windows DPI và mô hình IPC. |
| M1 Lõi cá nhân | HTTP engine Rust, collection SQLite, biến và kho secret; lưu nháp, lịch sử, import export cơ bản. |
| M2 Hợp đồng API | Import validate OpenAPI, tài liệu offline, liên kết operation request, sinh request và đề xuất cập nhật từ diff. |
| M3 Bản A cho nhóm | Server nội bộ, quyền thành viên, publish revision, outbox reconnect conflict, comment và backup restore. Đây là mốc đầu phục vụ mục tiêu cộng tác. |
| M4 Bản B | Runner, assertion mở rộng, mock chung, review nhánh, báo cáo, CLI và Git; hoàn thiện migration Postman. |
| M5 Bản C | Script sandbox, vault nhóm, đồng biên tập, runner theo lịch và các giao thức bổ sung theo nhu cầu thật. |

### Quy tắc giao việc cho AI

- Mỗi tác vụ nêu mã chức năng, hành vi quan sát được, dữ liệu được phép chia sẻ và cách xử lý lỗi.

- Yêu cầu tách domain model, transport HTTP, storage, sync và UI để thay thành phần mà không viết lại toàn bộ.

- Chỉ chấp nhận hoàn thành khi có luồng thực trên hai client nếu tác vụ liên quan cộng tác; mock UI không phải đồng bộ thật.

- Review thủ công các phần quyền, secret, migration, conflict và sandbox. Với thao tác mạng và dữ liệu nhóm cần test có ý nghĩa.

- Mỗi mốc lưu commit và hướng dẫn chạy trên Windows; giữ bảng chức năng đã làm, còn thiếu và giới hạn tương thích.

Vibe code khả thi cho dự án này nếu bạn giữ phạm vi theo mốc và kiểm chứng các phần khó. Không thể ước lượng đáng tin bằng một số ngày cố định khi chưa chốt UI, kinh nghiệm Rust và mức tương thích cần đạt.

## 24 Phạm vi tương thích và tài liệu tham khảo

### Các ranh giới cần ghi trong README

Sản phẩm có thương hiệu và giao diện riêng, học từ các quy trình của Postman và Swagger. Không sao chép mã nguồn hoặc tài sản khi chưa có quyền sử dụng; việc tái sử dụng thư viện cần kiểm tra license và nghĩa vụ đi kèm. Không lấy tên clone làm lời hứa thay thế mọi chức năng của các sản phẩm gốc.

Bản A tập trung REST OpenAPI và cộng tác. Không bao gồm thanh toán, marketplace, AI cloud bắt buộc, hệ thống chat thay thế Discord, tunnel Internet tự động, giám sát 24 giờ khi mọi máy đều tắt, hay tương thích toàn bộ pm API. Các chức năng B và C là backlog sản phẩm, không được hiển thị như đã hoàn thành.

Không có phí theo ghế do nhà cung cấp áp đặt trong ứng dụng tự xây. Chi phí máy chủ tên miền lưu trữ backup và chứng thư ký ứng dụng nếu dùng vẫn do nhóm quyết định. Máy chủ luôn bật cần thiết cho đồng bộ khi người tạo workspace tắt máy; dùng máy cá nhân làm host thì cộng tác dừng khi host offline.

### Nguồn kỹ thuật chính thức

Các nguồn dưới đây dùng để kiểm tra vai trò công cụ, cấu trúc nền tảng và định dạng dữ liệu. Danh mục chức năng, ưu tiên A B C, kiến trúc tự host và chỉ tiêu nghiệm thu là đề xuất thiết kế dành cho dự án này. Truy cập ngày 23 tháng 11 năm 2026.

[1] Tauri Process Model

https://v2.tauri.app/concept/process-model/

[2] reqwest Documentation

https://docs.rs/reqwest/latest/reqwest/

[3] Tauri Capabilities

https://v2.tauri.app/security/capabilities/

[4] OpenAPI Specification 3.1.1

https://spec.openapis.org/oas/v3.1.1.html

[5] Swagger UI

https://swagger.io/open-source/swagger-ui/

[6] Swagger Editor

https://swagger.io/open-source/swagger-editor/

[7] Postman Workspaces Overview

https://learning.postman.com/docs/collaborating-in-postman/using-workspaces/overview

[8] Postman Export Data

https://learning.postman.com/docs/getting-started/importing-and-exporting/exporting-data/
