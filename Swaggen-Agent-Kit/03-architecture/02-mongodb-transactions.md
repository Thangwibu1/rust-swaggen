# MongoDB: dữ liệu và transaction

## Điều kiện triển khai

MongoDB standalone không đủ cho multi-document transaction. Dùng replica set có primary được bầu, authentication, internal member auth và persistent volumes. URI của server có `replicaSet=<name>` cùng seed hostnames resolve được từ container. Một replica set một node có thể dùng cho dev/budget single VPS nếu kiểm thử transaction thành công; không có HA. Production resilience dùng replica set nhiều member trên các máy/failure domain khác nhau; ba container trên cùng VPS không chống mất VPS.

Kiểm chứng phiên bản server/driver/FCV từ docs chính thức tại thời điểm triển khai. Không lấy image mặc định của Coolify làm bằng chứng topology đúng.

## Collections và index gợi ý

| Collection | Index và bất biến |
| --- | --- |
| users, sessions, invitations | Unique user identity; token chỉ lưu hash; expires_at + TTL khi hợp lý; kiểm tra expiry ở ứng dụng, không dựa TTL xóa tức thì |
| workspaces, memberships | unique(workspace_id,user_id); một Owner; ownership transfer transaction |
| entities | unique(workspace_id,entity_id); index(workspace_id,parent_id,order_key); revision bắt buộc |
| specs, operations | stable internal operation ID; method/path uniqueness đúng scope/spec revision |
| revisions | unique(workspace_id,entity_id,revision); snapshot hoặc patch có đường khôi phục |
| applied_operations | unique(workspace_id,actor_id,operation_id); request_hash chống tái dùng ID cho payload khác; result ACK lưu bền |
| change_events | unique(workspace_id,cursor); index(workspace_id,cursor) cho keyset replay |
| workspace_counters | counter sequence server, update trong publish transaction; benchmark contention |
| comments, notifications | workspace/object indexes, pagination, đọc theo người dùng |
| attachments | unique(workspace_id,hash); lifecycle pending/verified/referenced/tombstoned |
| audit_events, jobs, schedules | scope indexes + retention; claim lease/fencing cho job khi C |

Không nhét toàn bộ workspace hoặc response lớn vào một BSON document. Phân lô import; đo kích thước BSON trước ghi, tôn trọng giới hạn server của phiên bản được ghim. Không tự động TTL revisions/tombstones trước thời hạn replay/resync policy.

## Publish transaction bắt buộc

Input: protocol version, workspace_id, entity_id, base_revision, operation_id, request_hash, sanitized payload, expected membership revision.

1. Xác thực session bên ngoài và kiểm tra lại quyền ghi trong transaction. Membership change và publish cùng serialize qua một document permission fence/workspace authorization epoch; chỉ đọc membership snapshot là chưa đủ để chứng minh không race với revoke.
2. Tìm idempotency record trong cùng scope. Cùng ID+hash → trả ACK đã commit; ID giống nhưng hash khác → lỗi rõ ràng, không apply.
3. Thực hiện conditional write entity với workspace_id + revision = base_revision + chưa tombstone; matched_count phải bằng 1. Base cũ → conflict, abort, giữ local draft.
4. Ghi revision mới, entity, cursor/counter, change_event, audit tối thiểu và applied_operations trong **cùng ClientSession**.
5. Commit transaction; ACK chỉ sau commit. Retry đúng error label và có deadline/backoff; callback có thể chạy lại nên không gửi email/webhook, upload file hoặc push realtime bên trong.
6. Sau commit phát tín hiệu; mất tín hiệu thì polling/reconnect vẫn lấy event log. Không dùng event in-memory làm nguồn dữ liệu duy nhất.

Mỗi transaction chạy các operation tuần tự theo driver Rust; không `join!`/`Promise.all` trên cùng session. Reuse Client/pool, session per transaction. Read/write concern và retry policy được ghim và kiểm thử, gợi ý snapshot + majority khi phù hợp. Không giữ transaction mở trong lúc HTTP upload/download, chờ người dùng merge hay chạy script.

## Unknown commit và retry

`TransientTransactionError`: retry toàn transaction với cùng operation ID, callbacks không side effect ngoài DB. `UnknownTransactionCommitResult`: retry commit theo driver của phiên bản ghim; nếu hết deadline, trạng thái "chưa xác định", query applied_operations trước quyết định gửi lại. Không thông báo failure chắc chắn khi có thể đã commit. Duplicate unique key trong concurrent publish phải được ánh xạ và lookup ACK/conflict đúng.

## File không thuộc DB transaction

Upload tới staging → validate size/hash → marked verified → publish metadata reference trong transaction. Blob GC trì hoãn; lỗi sau upload trước commit không sinh liên kết hỏng. Backup phải có cut/cursor và manifest DB+blobs nhất quán. Không hứa distributed atomicity giữa filesystem và MongoDB.

## Test không được bỏ qua

Commit tất cả; failure giữa chừng rollback tất cả; base stale; retry cùng ID chỉ một revision; ID trùng payload khác; ACK lost; process chết trước/sau commit; primary unavailable/stepdown khi topology cho phép; permission revoke race; cross-workspace ID; expired invitation dùng lại; counters order; attachment orphan; transaction timeout/unknown commit recovery. Chạy với MongoDB replica set thật, không chỉ mock repository.
