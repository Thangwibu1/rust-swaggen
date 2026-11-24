# AU — Yêu cầu nguồn

## AU01 · A · No Auth và Bearer

Chọn không xác thực hoặc Bearer token bằng biến cá nhân; token được che trong giao diện và log.

## AU02 · A · Basic và API key

Basic Auth, API key trong header hoặc query; cảnh báo URL có secret và loại bỏ secret khi xuất mặc định.

## AU03 · A · Kế thừa xác thực

Request kế thừa từ folder hoặc collection; có thể ghi đè hoặc tắt và xem nguồn cấu hình đang áp dụng.

## AU04 · A · Cookie jar

Lưu, xem, sửa, xóa cookie theo domain path và expiry; tách theo workspace hoặc môi trường cá nhân.

## AU05 · A · TLS mặc định

Kiểm tra chứng chỉ theo mặc định; cho phép ngoại lệ riêng theo host với cảnh báo rõ và không áp dụng toàn cục.

## AU06 · A · Redirect

Cấu hình theo hoặc dừng redirect, giới hạn số lần; không chuyển Authorization sang host khác ngoài chính sách cho phép.

## AU07 · B · Proxy và CA riêng

Cấu hình proxy, bypass, CA của tổ chức; không ghi mật khẩu proxy vào dữ liệu chung.

## AU08 · B · OAuth 2 PKCE

Authorization Code với PKCE, Client Credentials, scope và token refresh; callback loopback được xác thực trạng thái.

## AU09 · B · Chứng chỉ client

Cấu hình mTLS theo host và port; khóa riêng giữ trên máy, hỗ trợ xóa liên kết và báo hết hạn.

## AU10 · C · Xác thực bổ sung

Đánh giá Digest, AWS Signature và cơ chế khác theo nhu cầu; hiển thị rõ loại nào chưa được hỗ trợ.
