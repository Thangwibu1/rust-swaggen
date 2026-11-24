# Triển khai Swaggen Backend lên Coolify (với MongoDB Replica Set sẵn có)

Tài liệu này dùng khi bạn đã có `SWAGGEN_MONGO_URI` trỏ tới một MongoDB Replica Set (có transaction) và đẩy toàn bộ workspace lên Git để deploy lên Coolify.

## Các file ở thư mục gốc (`/` của repo)

- [Dockerfile](../Dockerfile): multi-stage build với `rust:1.100.0-bookworm` -> `debian:bookworm-slim` (`swaggen-server`, non-root user `swaggen`, có sẵn `ca-certificates` cho MongoDB TLS và `curl` cho `HEALTHCHECK`).
- [.dockerignore](../.dockerignore): loại bỏ `target/`, `.local/`, `dist/`, tài liệu nguồn, file bí mật và `rust-toolchain.toml` (vì `rust-toolchain.toml` pin target Windows `x86_64-pc-windows-msvc`).
- [docker-compose.yml](../docker-compose.yml): cấu hình Docker Compose ở gốc repo dùng `Dockerfile`, expose cổng `8080`, bind an toàn `127.0.0.1:${SWAGGEN_PORT:-8080}:8080`, rootfs `read_only: true`, và kiểm tra readiness `/v1/health/ready`.
- [.env.example](../.env.example): mẫu biến môi trường an toàn (file `.env` thật đã được `.gitignore` chặn commit).

## Cách 1: Deploy bằng Docker Compose trên Coolify (mặc định tại root)

1. Push toàn bộ workspace lên Git repository kết nối với Coolify (không push file `.env` chứa URI thật).
2. Trong Coolify: **New Resource -> Application** -> chọn Git repo/branch.
3. **Build Pack**: chọn **Docker Compose**, **Base Directory**: `/`, **Docker Compose Location**: `/docker-compose.yml`.
4. Trong Coolify **Environment Variables**, khai báo:
   - `SWAGGEN_MONGO_URI` *(bắt buộc)*: URI MongoDB replica set của bạn.
   - `SWAGGEN_MONGO_DB` *(tùy chọn)*: mặc định `swaggen`.
   - `SWAGGEN_BOOTSTRAP_TOKEN` *(tùy chọn, dùng 1 lần)*: chuỗi ngẫu nhiên (vd 32-byte hex) để tạo Server Owner đầu tiên.
   - `SWAGGEN_PORT` *(tùy chọn)*: mặc định `8080` (chỉ bind `127.0.0.1` trên host; nếu host VPS đã dùng cổng 8080 thì đổi sang số khác như `18080`).
5. Gắn domain HTTPS trên Coolify proxy trỏ vào service port `8080`, rồi nhấn **Deploy**.

## Cách 2: Deploy trực tiếp bằng Dockerfile trên Coolify

1. Trong Coolify chọn **Build Pack**: **Dockerfile**, **Dockerfile Location**: `/Dockerfile`, **Base Directory**: `/`.
2. **Ports Exposes**: `8080`, **Healthcheck Path**: `/v1/health/ready` (port `8080`).
3. Khai báo `SWAGGEN_MONGO_URI`, `SWAGGEN_MONGO_DB` (tùy chọn), `SWAGGEN_BOOTSTRAP_TOKEN` (tùy chọn, 1 lần) và nhấn **Deploy**.

## Kiểm tra sau khi Deploy & khởi tạo Owner lần đầu

1. Mở **Terminal** của container trên Coolify và chạy:
   ```sh
   swaggen-server doctor-mongo
   ```
   Phải thấy `PASS: MongoDB replica-set topology, migrations, and transaction rollback verified`.
2. Kiểm tra qua HTTPS domain:
   ```sh
   curl -fsS https://<domain-cua-ban>/v1/health/ready
   curl -fsS https://<domain-cua-ban>/v1/capabilities
   ```
3. Mở app desktop Windows (`swaggen-desktop.exe`), kết nối tới `https://<domain-cua-ban>`, tạo **Server Owner** đầu tiên bằng `SWAGGEN_BOOTSTRAP_TOKEN`.
4. Xóa/làm rỗng `SWAGGEN_BOOTSTRAP_TOKEN` trong Coolify và **Redeploy** để khóa đường bootstrap.
