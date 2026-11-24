//! Async DNS + outbound SSRF verification built atop `swaggen_domain::ssrf` (SC08).
use std::net::SocketAddr;
pub use swaggen_domain::ssrf::{SsrfPolicy, is_restricted_ip, validate_outbound_url};
use url::Url;

/// Performs DNS resolution and verifies every returned IP address against `SsrfPolicy` (SC08).
pub async fn resolve_and_verify_outbound(
    raw_url: &str,
    policy: &SsrfPolicy,
) -> Result<(Url, Vec<SocketAddr>), String> {
    let (url, host, port) = validate_outbound_url(raw_url, policy)?;
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|e| format!("DNS resolution failed for '{host}': {e}"))?
        .collect();
    if addrs.is_empty() {
        return Err(format!("DNS returned no addresses for '{host}'"));
    }
    if !policy.allow_private_networks && !policy.allowed_hosts.contains(&host) {
        for addr in &addrs {
            if is_restricted_ip(addr.ip()) {
                return Err(format!(
                    "SSRF blocked DNS resolution of '{host}' to restricted IP {}",
                    addr.ip()
                ));
            }
        }
    }
    Ok((url, addrs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssrf_blocks_metadata_private_loopback_schemes_and_ipv4_mapped_v6() {
        let pol = SsrfPolicy::default();
        assert!(validate_outbound_url("http://169.254.169.254/latest/meta-data", &pol).is_err());
        assert!(validate_outbound_url("http://127.0.0.1:8080/admin", &pol).is_err());
        assert!(validate_outbound_url("http://10.0.0.5/internal", &pol).is_err());
        assert!(validate_outbound_url("http://[::ffff:127.0.0.1]:8080/", &pol).is_err());
        assert!(validate_outbound_url("http://[::ffff:169.254.169.254]/", &pol).is_err());
        assert!(validate_outbound_url("file:///C:/Windows/win.ini", &pol).is_err());
        assert!(validate_outbound_url("https://api.example.com/v1/health", &pol).is_ok());

        let mut allow = SsrfPolicy::default();
        allow.allowed_hosts.insert("127.0.0.1".into());
        assert!(validate_outbound_url("http://127.0.0.1:8080/hook", &allow).is_ok());
    }
}
