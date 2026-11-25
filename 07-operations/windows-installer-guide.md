# Hướng dẫn tạo bộ cài đặt Windows Installer cho Swaggen

Dự án đã tích hợp sẵn **3 cách tạo bộ cài đặt Windows (`.exe` và `.msi`)** để người dùng cuối chỉ cần tải 1 file về, nhấn đúp để cài đặt (tạo shortcut Desktop + Start Menu, đăng ký Uninstall trong Windows Settings, thêm `swaggen-cli` vào PATH, **không cần quyền Admin**).

---

## Cách 1: Build ngay 1 file `.exe` trên máy Windows hiện tại (Không cần cài thêm phần mềm)

Sử dụng công cụ `iexpress.exe` có sẵn của Windows qua script `tools\build-installer.ps1`:

```powershell
# Bước 1: Build bản release & gom gói vào result\v0.1.3
powershell -NoProfile -ExecutionPolicy Bypass -File tools\bundle-result.ps1 -Version 0.1.3

# Bước 2: Đóng gói thành 1 file Setup EXE duy nhất
powershell -NoProfile -ExecutionPolicy Bypass -File tools\build-installer.ps1 -Version 0.1.3
```

File đầu ra nằm tại:
- `result\v0.1.3\installer\Swaggen-Setup-0.1.3-x64.exe`
- `result\v0.1.3\installer\SHA256SUMS.txt`

Khi người dùng nhấn đúp `Swaggen-Setup-0.1.3-x64.exe`, bộ cài sẽ tự động:
1. Giải nén app vào `%LOCALAPPDATA%\Swaggen`.
2. Tạo shortcut **Swaggen** trên **Desktop** và **Start Menu**.
3. Đăng ký mục gỡ cài đặt chuẩn trong **Windows Settings → Installed apps** (gỡ sạch bằng 1 click).
4. Thêm `swaggen-cli` vào biến `PATH` của người dùng và hỏi mở Swaggen ngay.

---

## Cách 2: Tự động build Inno Setup (`.exe` chuyên nghiệp) + WiX (`.msi`) + Auto-Update `latest.json` trên GitHub Releases (Khuyên dùng)

Repo đã có sẵn workflow `.github/workflows/release-windows-installer.yml`. Mỗi khi bạn phát hành bản mới, chỉ cần tăng `version` trong `Cargo.toml`, commit rồi đẩy tag:

```powershell
git tag v0.1.3
git push origin v0.1.3
```

GitHub Actions (`windows-latest`) sẽ tự động biên dịch và tạo trang **GitHub Release** đính kèm sẵn **5 file**:
- `Swaggen-Setup-0.1.3-x64.exe` — Wizard cài đặt Inno Setup 6 (tự nhận diện bản cũ, đóng app đang chạy, cập nhật đè lên giữ nguyên toàn bộ dữ liệu + mở lại app)
- `Swaggen-0.1.3-x64.msi` — Bộ cài Windows MSI có `MajorUpgrade`
- `Swaggen-Portable-0.1.3-win64.zip` — Bản chạy portable (cũng nạp thẳng vào chức năng update trong app được)
- `latest.json` — Manifest cập nhật tự động cho nút **Check for updates** ngay trong ứng dụng Desktop
- `SHA256SUMS.txt` — Mã băm kiểm tra toàn vẹn

### Hai cách cập nhật cho người dùng cuối
1. **Tự động ngay trong app (1 click)**:
   - Bấm nút **`Updates`** trên thanh tiêu đề (hoặc vào **Settings → Privacy & Updates** → bấm **Check for updates**).
   - App tự đọc `https://github.com/Thangwibu1/rust-swaggen/releases/latest/download/latest.json`, báo phiên bản mới, tải file `.exe` / `.zip`, **tự kiểm tra SHA-256** rồi cài đè tự động và mở lại app.
2. **Cập nhật bằng file tải về (Offline / Thủ công)**:
   - Tải `Swaggen-Setup-<ver>-x64.exe` hoặc `.msi` từ trang GitHub Releases → **nhấn đúp trực tiếp** (Inno Setup/WiX tự nâng cấp đè lên bản cũ mà không mất workspace hay secrets), **HOẶC** chọn trực tiếp file `.exe` / `.msi` / `.zip` ở mục **Option 2: Install downloaded update file** ngay trong màn hình **Updates** của Swaggen.

---

## Cách 3: Build Inno Setup `.exe` hoặc WiX `.msi` trực tiếp trên máy bạn

Nếu muốn build wizard Inno Setup 6 hoặc MSI ngay trên máy cá nhân:
1. Cài **Inno Setup 6** (`winget install JRSoftware.InnoSetup`) → chạy lại `tools\build-installer.ps1 -Version 0.1.3`, script tự phát hiện `ISCC.exe` và xuất file Inno Setup từ `installer\swaggen.iss`.
2. Hoặc cài **.NET SDK + WiX v4** (`dotnet tool install --global wix`) → chạy lại `tools\build-installer.ps1 -Version 0.1.3`, script tự phát hiện `wix.exe` và xuất file `.msi` từ `installer\swaggen.wxs`.

---

## Lưu ý về Windows SmartScreen (Ký số Code Signing)

- Nếu chưa ký số (unsigned), lần đầu người dùng mở file tải từ Internet có thể thấy cảnh báo xanh *Windows protected your PC* → bấm **More info → Run anyway** là cài bình thường.
- Khi phát hành rộng rãi để không hiện cảnh báo SmartScreen, bạn mua chứng chỉ **OV/EV Code Signing Certificate** (hoặc Azure Trusted Signing) và ký file `.exe` / `.msi` bằng `signtool.exe`:

```powershell
signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f cert.pfx /p <PASSWORD> Swaggen-Setup-0.1.3-x64.exe
```
