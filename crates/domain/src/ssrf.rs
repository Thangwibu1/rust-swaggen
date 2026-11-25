//! Outbound SSRF policy & validation: blocks loopback, RFC1918 private, link-local/metadata,
//! ULA, unspecified, IPv4-mapped IPv6, and disallowed schemes unless explicitly allowlisted (SC08).
use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsrfPolicy {
    pub allow_private_networks: bool,
    pub allowed_hosts: BTreeSet<String>,
    pub allow_http: bool,
}

impl Default for SsrfPolicy {
    fn default() -> Self {
        Self {
            allow_private_networks: false,
            allowed_hosts: BTreeSet::new(),
            allow_http: true,
        }
    }
}

pub fn is_restricted_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_restricted_v4(v4),
        IpAddr::V6(v6) => is_restricted_v6(v6),
    }
}

fn is_restricted_v4(ip: Ipv4Addr) -> bool {
    let oct = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || (oct[0] == 100 && (oct[1] & 0b1100_0000) == 64)
        || (oct[0] == 169 && oct[1] == 254)
        || (oct[0] == 192 && oct[1] == 0 && oct[2] == 0)
        || (oct[0] == 0)
}

fn is_restricted_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_restricted_v4(v4);
    }
    let seg = ip.segments();
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (seg[0] & 0xfe00) == 0xfc00
        || (seg[0] & 0xffc0) == 0xfe80
}

pub fn validate_outbound_url(
    raw_url: &str,
    policy: &SsrfPolicy,
) -> Result<(Url, String, u16), String> {
    let url = Url::parse(raw_url.trim()).map_err(|e| format!("Invalid target URL: {e}"))?;
    let scheme_ok = url.scheme() == "https" || (policy.allow_http && url.scheme() == "http");
    if !scheme_ok {
        return Err(format!(
            "Blocked URL scheme '{}' under SSRF policy",
            url.scheme()
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| "Target URL missing host".to_owned())?
        .to_ascii_lowercase();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "Target URL missing known port".to_owned())?;

    if policy.allowed_hosts.contains(&host) || policy.allow_private_networks {
        return Ok((url, host, port));
    }

    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
    {
        return Err(format!("SSRF blocked internal hostname '{host}'"));
    }

    let trimmed_ip = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = trimmed_ip.parse::<IpAddr>()
        && is_restricted_ip(ip)
    {
        return Err(format!("SSRF blocked non-public IP '{ip}'"));
    }
    Ok((url, host, port))
}
