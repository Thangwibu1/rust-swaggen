# Kế hoạch triển khai theo gate

Không ước lượng bằng số ngày cố định trước P00. Mỗi phase bàn giao luồng end-to-end và evidence. Bảo mật, giới hạn tài nguyên, durability đi xuyên suốt; phase nâng cao không trì hoãn kiểm soát cần có cho tính năng sớm.

| Phase | Mục tiêu | Phụ thuộc | Mã nguồn gán chính |
| --- | --- | --- | --- |
| [P00](P00.md) | Preflight và prototype | none | 0 |
| [P01](P01.md) | Nền tảng workspace và UI kit | P00 | 5 |
| [P02](P02.md) | Lõi API cá nhân bản A | P01 | 41 |
| [P03](P03.md) | OpenAPI và hợp đồng bản A | P02 | 20 |
| [P04](P04.md) | Server MongoDB và tài khoản nội bộ | P01 | 5 |
| [P05](P05.md) | Publish, đồng bộ và cộng tác bản A | P02 + P03 + P04 | 14 |
| [P06](P06.md) | Phát hành bản A trên Windows và Coolify | P05 | 8 |
| [P07](P07.md) | Mở rộng network, spec và import bản B | P06 | 18 |
| [P08](P08.md) | Kiểm thử, mock và developer tools bản B | P07 | 17 |
| [P09](P09.md) | Review, trải nghiệm và vận hành bản B | P08 | 15 |
| [P10](P10.md) | Sandbox, vault và chính sách nâng cao bản C | P09 | 4 |
| [P11](P11.md) | Giao thức và cộng tác nâng cao bản C | P10 | 13 |
| [P12](P12.md) | Hoàn thiện toàn bộ sản phẩm và tối ưu | P11 | 0 |

Các yêu cầu gán một phase chính để không đếm trùng. P00/P01/P06/P12 có gate xuyên suốt ngoài số mã gán; P12 phải revalidate toàn bộ, không chỉ các mã mới. P04 có thể làm sau P03 theo thứ tự mặc định hoặc độc lập khi framework/API contract đã thống nhất. Mọi thay đổi thứ tự phải giữ dependency và cập nhật traceability.
