//! In-app GitHub Releases auto-updater & local update package installer (.exe / .msi / .zip).
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use swaggen_domain::workspace::{
    HttpMethod, RequestBody, RequestConnectionPolicy, ResolvedHeader, ResolvedRequest,
};
use swaggen_http_engine::execute_request;
use tokio_util::sync::CancellationToken;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_UPDATE_FEED_URL: &str =
    "https://github.com/Thangwibu1/rust-swaggen/releases/latest/download/latest.json";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub installer_url: String,
    #[serde(default)]
    pub installer_sha256: String,
    #[serde(default)]
    pub msi_url: String,
    #[serde(default)]
    pub msi_sha256: String,
    #[serde(default)]
    pub portable_url: String,
    #[serde(default)]
    pub portable_sha256: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub auto_install: bool,
    #[serde(default)]
    pub broadcast_id: String,
}

/// Picks `(download_url, expected_sha256)` best suited for how Swaggen is currently installed:
/// - Installed under `%LOCALAPPDATA%\Swaggen` -> prefers `installer_url` (`.exe` Setup) so shortcuts & registry version update cleanly.
/// - Running from a portable folder -> prefers `portable_url` (`.zip` in-place swap & restart), falling back to `installer_url`.
pub fn select_best_package_for_current_install(
    manifest: &UpdateManifest,
) -> Option<(String, String)> {
    let running_in_localappdata = std::env::current_exe()
        .ok()
        .and_then(|p| {
            let exe_lc = p.to_string_lossy().to_ascii_lowercase();
            std::env::var("LOCALAPPDATA")
                .ok()
                .map(|la| exe_lc.starts_with(&la.to_ascii_lowercase()))
        })
        .unwrap_or(false);

    let has_installer = !manifest.installer_url.trim().is_empty();
    let has_portable = !manifest.portable_url.trim().is_empty();

    if running_in_localappdata {
        if has_installer {
            Some((
                manifest.installer_url.trim().to_owned(),
                manifest.installer_sha256.trim().to_owned(),
            ))
        } else if has_portable {
            Some((
                manifest.portable_url.trim().to_owned(),
                manifest.portable_sha256.trim().to_owned(),
            ))
        } else {
            None
        }
    } else if has_portable {
        Some((
            manifest.portable_url.trim().to_owned(),
            manifest.portable_sha256.trim().to_owned(),
        ))
    } else if has_installer {
        Some((
            manifest.installer_url.trim().to_owned(),
            manifest.installer_sha256.trim().to_owned(),
        ))
    } else {
        None
    }
}

/// Silently polls `{server_base_url}/v1/releases/latest` (and optionally `fallback_feed_url`)
/// for admin-broadcast version updates without raising UI errors when offline or `204 No Content`.
pub async fn poll_release_broadcast(
    server_base_url: Option<&str>,
    fallback_feed_url: &str,
    check_fallback: bool,
    scratch_dir: &Path,
) -> Option<UpdateCheckOutcome> {
    if let Some(base) = server_base_url.map(str::trim).filter(|s| !s.is_empty()) {
        let srv_feed = format!("{}/v1/releases/latest", base.trim_end_matches('/'));
        if validate_update_url(&srv_feed).is_ok()
            && let Ok(outcome) = fetch_latest_manifest(&srv_feed, scratch_dir).await
        {
            return Some(outcome);
        }
    }
    if check_fallback && !fallback_feed_url.trim().is_empty() {
        fetch_latest_manifest(fallback_feed_url.trim(), scratch_dir)
            .await
            .ok()
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct UpdateCheckOutcome {
    pub current_version: String,
    pub latest: UpdateManifest,
    pub update_available: bool,
}

fn parse_semver(raw: &str) -> (u64, u64, u64) {
    let trimmed = raw.trim().trim_start_matches('v').trim_start_matches('V');
    let core = trimmed.split(['-', '+']).next().unwrap_or(trimmed);
    let mut parts = core.split('.');
    let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

pub fn is_newer_version(current: &str, candidate: &str) -> bool {
    parse_semver(candidate) > parse_semver(current)
}

fn validate_update_url(url: &str) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.starts_with("https://")
        || trimmed.starts_with("http://127.0.0.1")
        || trimmed.starts_with("http://localhost")
    {
        Ok(())
    } else {
        Err("Update URL must use HTTPS.".into())
    }
}

pub fn compute_file_sha256(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("Failed to open update file: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65_536];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("Failed reading update file: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub async fn fetch_latest_manifest(
    feed_url: &str,
    scratch_dir: &Path,
) -> Result<UpdateCheckOutcome, String> {
    validate_update_url(feed_url)?;
    let headers = vec![
        ResolvedHeader {
            name: "Accept".into(),
            value: "application/json".into(),
            generated: true,
        },
        ResolvedHeader {
            name: "User-Agent".into(),
            value: format!("Swaggen-Desktop/{APP_VERSION}"),
            generated: true,
        },
    ];
    let resolved = ResolvedRequest {
        method: HttpMethod::Get,
        url: feed_url.trim().to_owned(),
        masked_url: feed_url.trim().to_owned(),
        headers: headers.clone(),
        masked_headers: headers,
        body: RequestBody::None,
        masked_body_preview: String::new(),
        auth_source: "PublicReleaseFeed".into(),
        warnings: Vec::new(),
        blocking_errors: Vec::new(),
        requires_production_confirm: false,
        connection: RequestConnectionPolicy {
            timeout_ms: 20_000,
            follow_redirects: true,
            max_redirects: 8,
            send_auth_on_cross_host_redirect: false,
            ..Default::default()
        },
    };
    let resp = execute_request(&resolved, scratch_dir, CancellationToken::new())
        .await
        .map_err(|e| format!("Update check failed: {}", e.message))?;
    if !(200..300).contains(&resp.status_code) {
        return Err(format!(
            "Update feed returned HTTP {} (make sure a GitHub Release with latest.json is published).",
            resp.status_code
        ));
    }
    let manifest: UpdateManifest = serde_json::from_slice(&resp.preview_bytes)
        .map_err(|e| format!("Invalid update manifest JSON: {e}"))?;
    let update_available = is_newer_version(APP_VERSION, &manifest.version);
    Ok(UpdateCheckOutcome {
        current_version: APP_VERSION.to_owned(),
        latest: manifest,
        update_available,
    })
}

pub async fn download_update_package(
    download_url: &str,
    expected_sha256: &str,
    version: &str,
    updates_dir: &Path,
) -> Result<PathBuf, String> {
    validate_update_url(download_url)?;
    fs::create_dir_all(updates_dir)
        .map_err(|e| format!("Failed creating updates directory: {e}"))?;

    let headers = vec![ResolvedHeader {
        name: "User-Agent".into(),
        value: format!("Swaggen-Desktop/{APP_VERSION}"),
        generated: true,
    }];
    let resolved = ResolvedRequest {
        method: HttpMethod::Get,
        url: download_url.trim().to_owned(),
        masked_url: download_url.trim().to_owned(),
        headers: headers.clone(),
        masked_headers: headers,
        body: RequestBody::None,
        masked_body_preview: String::new(),
        auth_source: "PublicReleaseDownload".into(),
        warnings: Vec::new(),
        blocking_errors: Vec::new(),
        requires_production_confirm: false,
        connection: RequestConnectionPolicy {
            timeout_ms: 180_000,
            follow_redirects: true,
            max_redirects: 10,
            send_auth_on_cross_host_redirect: false,
            ..Default::default()
        },
    };
    let resp = execute_request(&resolved, updates_dir, CancellationToken::new())
        .await
        .map_err(|e| format!("Failed to download update: {}", e.message))?;
    if !(200..300).contains(&resp.status_code) {
        let _ = fs::remove_file(&resp.raw_body_path);
        return Err(format!(
            "Update download failed with HTTP {}.",
            resp.status_code
        ));
    }

    let actual_sha = compute_file_sha256(&resp.raw_body_path)?;
    let expected_clean = expected_sha256.trim();
    if !expected_clean.is_empty() && !actual_sha.eq_ignore_ascii_case(expected_clean) {
        let _ = fs::remove_file(&resp.raw_body_path);
        return Err(format!(
            "SHA-256 verification failed! Expected {expected_clean}, got {actual_sha}. Download rejected."
        ));
    }

    let safe_ver: String = version
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
        .collect();
    let lower = download_url.to_ascii_lowercase();
    let ext = if lower.ends_with(".msi") {
        "msi"
    } else if lower.ends_with(".zip") {
        "zip"
    } else {
        "exe"
    };
    let filename = format!(
        "Swaggen-Update-{}.{ext}",
        if safe_ver.is_empty() {
            "latest"
        } else {
            &safe_ver
        }
    );
    let dest = updates_dir.join(filename);
    fs::copy(&resp.raw_body_path, &dest)
        .map_err(|e| format!("Failed saving verified update package: {e}"))?;
    let _ = fs::remove_file(&resp.raw_body_path);
    Ok(dest)
}

pub fn launch_local_update_package(
    package_path: &Path,
    expected_sha256: Option<&str>,
    silent: bool,
) -> Result<String, String> {
    if !package_path.is_file() {
        return Err(format!("Update file not found: {}", package_path.display()));
    }
    if let Some(expected) = expected_sha256.map(str::trim).filter(|s| !s.is_empty()) {
        let actual = compute_file_sha256(package_path)?;
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(format!(
                "SHA-256 mismatch for local file! Expected {expected}, got {actual}."
            ));
        }
    }

    let ext = package_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        match ext.as_str() {
            "exe" => {
                let mut cmd = std::process::Command::new(package_path);
                if silent {
                    // Swaggen-Setup-x64.exe is built with Windows IExpress (`/Q` invokes silent install + auto-relaunch)
                    cmd.arg("/Q");
                }
                cmd.spawn()
                    .map_err(|e| format!("Failed to launch installer .exe: {e}"))?;
                Ok(format!(
                    "Launched installer {}. Swaggen will close and restart after upgrading.",
                    package_path.display()
                ))
            }
            "msi" => {
                let mut cmd = std::process::Command::new("msiexec.exe");
                cmd.arg("/i").arg(package_path);
                if silent {
                    cmd.arg("/passive");
                }
                cmd.spawn()
                    .map_err(|e| format!("Failed to launch MSI installer: {e}"))?;
                Ok(format!(
                    "Launched MSI upgrade {}. Follow Windows Installer prompts to complete.",
                    package_path.display()
                ))
            }
            "zip" => {
                let current_exe = std::env::current_exe()
                    .map_err(|e| format!("Cannot resolve current executable path: {e}"))?;
                let bin_dir = current_exe
                    .parent()
                    .ok_or_else(|| "Cannot determine current binary directory".to_owned())?;
                let install_root = if bin_dir
                    .file_name()
                    .is_some_and(|n| n.eq_ignore_ascii_case("bin"))
                {
                    bin_dir.parent().unwrap_or(bin_dir)
                } else {
                    bin_dir
                };
                let pid = std::process::id();
                let zip_str = package_path.display().to_string().replace('\'', "''");
                let dst_str = install_root.display().to_string().replace('\'', "''");
                let exe_str = current_exe.display().to_string().replace('\'', "''");
                let script = format!(
                    "Start-Sleep -Seconds 1; try {{ Stop-Process -Id {pid} -Force -ErrorAction SilentlyContinue }} catch {{}}; Start-Sleep -Milliseconds 600; Expand-Archive -Path '{zip_str}' -DestinationPath '{dst_str}' -Force; if (Test-Path '{exe_str}') {{ Start-Process -FilePath '{exe_str}' }}"
                );
                std::process::Command::new("powershell.exe")
                    .args([
                        "-NoProfile",
                        "-ExecutionPolicy",
                        "Bypass",
                        "-WindowStyle",
                        "Hidden",
                        "-Command",
                        &script,
                    ])
                    .spawn()
                    .map_err(|e| format!("Failed launching ZIP updater helper: {e}"))?;
                Ok(format!(
                    "Applying portable ZIP update {} into {} — Swaggen will restart automatically.",
                    package_path.display(),
                    install_root.display()
                ))
            }
            _ => Err("Unsupported update package format. Use .exe, .msi, or .zip.".into()),
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (ext, silent);
        Err("In-place Windows update installation is only supported on Windows.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn semver_comparison_works() {
        assert!(is_newer_version("0.1.3", "0.1.4"));
        assert!(is_newer_version("0.1.3", "v0.2.0"));
        assert!(is_newer_version("0.1.3", "1.0.0"));
        assert!(!is_newer_version("0.1.3", "0.1.3"));
        assert!(!is_newer_version("0.1.3", "0.1.2"));
    }

    #[test]
    fn fetch_manifest_and_download_verify_sha256_over_loopback() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let payload = b"FAKE_SWAGGEN_INSTALLER_PAYLOAD_V014";
            let mut hasher = Sha256::new();
            hasher.update(payload);
            let good_sha = hex::encode(hasher.finalize());

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let good_sha_srv = good_sha.clone();
            tokio::spawn(async move {
                for i in 0..3 {
                    if let Ok((mut sock, _)) = listener.accept().await {
                        let mut buf = vec![0u8; 2048];
                        let _ = sock.read(&mut buf).await;
                        if i == 0 {
                            let body = format!(
                                r#"{{"version":"9.9.9","tag":"v9.9.9","installer_url":"http://{addr}/Swaggen-Setup-9.9.9-x64.exe","installer_sha256":"{good_sha_srv}","notes":"Test release"}}"#
                            );
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                                body.len()
                            );
                            let _ = sock.write_all(resp.as_bytes()).await;
                        } else {
                            let header = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                payload.len()
                            );
                            let _ = sock.write_all(header.as_bytes()).await;
                            let _ = sock.write_all(payload).await;
                        }
                    }
                }
            });

            let dir = std::env::temp_dir().join(format!(
                "swaggen-updater-test-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&dir).unwrap();

            let outcome = fetch_latest_manifest(&format!("http://{addr}/latest.json"), &dir)
                .await
                .unwrap();
            assert!(outcome.update_available);
            assert_eq!(outcome.latest.version, "9.9.9");

            // Good SHA-256 succeeds
            let downloaded = download_update_package(
                &outcome.latest.installer_url,
                &good_sha,
                "9.9.9",
                &dir,
            )
            .await
            .unwrap();
            assert!(downloaded.is_file());

            // Tampered SHA-256 is rejected
            let err = download_update_package(
                &outcome.latest.installer_url,
                "0000000000000000000000000000000000000000000000000000000000000000",
                "9.9.9",
                &dir,
            )
            .await
            .unwrap_err();
            assert!(err.contains("SHA-256 verification failed"));

            let _ = fs::remove_dir_all(&dir);
        });
    }
}
