# Sync contract và API đề xuất

Đây là thiết kế logic cần version thành OpenAPI của backend Swaggen trước khi implement; khác với spec của API người dùng đang thiết kế.

## Endpoint families

| Family | Contract tối thiểu |
| --- | --- |
| /v1/health/live, /v1/health/ready, /v1/capabilities | live process; ready DB/topology/migrations; client-server versions và feature capabilities |
| /v1/auth/* | invitation/register/login/refresh/logout, session revoke; error code ổn định |
| /v1/workspaces/* | scoped membership, roles, entities, permissions |
| /v1/workspaces/{id}/publish | operation ID + payload hash + base revision; ACK hoặc conflict detail |
| /v1/workspaces/{id}/changes | after cursor + bounded limit, has_more, next_cursor; lỗi CURSOR_EXPIRED |
| /v1/workspaces/{id}/snapshot | snapshot có cursor nhất quán, hash/pages, không làm mất draft |
| /v1/workspaces/{id}/events | authenticated WS/SSE notification; không thay durable event log |
| /v1/workspaces/{id}/attachments/* | scoped upload, hash verify, finalize/download, quota |
| /v1/workspaces/{id}/comments/* | thread/object/revision anchor, pagination |

Error envelope: code, safe_message, correlation_id, retryable, details đã làm sạch. Không trả secret, connection URI hay stack trace. Entity ID không tự cấp quyền. Kiểm tra access trên từng route và subscription.

## Client state machine

Local draft → durably queued (chỉ khi user chọn Publish hoặc intent publish rõ) → sending → acked. Network error → retry_wait. Revision mismatch → conflicted. Access revoked → blocked_permission. Object deleted → deleted_remote_conflict. Permanent validation lỗi → failed_action_required. Không tự publish mọi autosave.

Reconnect: authenticate → capabilities → membership → replay missing events → rebase hoặc conflict local intent → retry operation cùng ID. Backoff jitter + max concurrency; giữ across restart. Cập nhật local entity và cursor bằng một durable local batch. Dedupe event ID/cursor; không bỏ qua cursor gap. Server clock/cursor quyết định thứ tự, không dùng đồng hồ desktop.

Cursor hết retention: lấy consistent snapshot vào staging, verify rồi swap; overlay draft/outbox và tombstones, xử lý conflict. Không xóa local trước khi snapshot đầy đủ. Snapshot pagination phải giữ cùng snapshot token/cut, không trộn dữ liệu nhiều thời điểm.

Conflict UI hiển thị base/local/server, từng field hoặc text diff; không last-write-wins âm thầm. Merge tạo operation mới với base revision hiện hành. Restore là revision mới, không sửa lịch sử cũ. Quyền bị gỡ giữ local draft cho export cá nhân phù hợp policy, không đẩy lại bằng retry vô hạn.
