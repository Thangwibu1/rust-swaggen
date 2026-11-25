//! Native async HTTP(S) client engine with ordered headers/query, host-scoped TLS
//! exceptions, custom CA/proxy/mTLS, cross-host redirect auth stripping, bounded 1 MiB response
//! preview streamed to disk, transport timings, progress events, and instant cancellation (P02/P07).
pub mod oauth2;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::read::{DeflateDecoder, GzDecoder};
use http::{HeaderMap, HeaderName, HeaderValue, Method};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use rustls::{
    ClientConfig, DigitallySignedStruct, SignatureScheme,
    client::WebPkiServerVerifier,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
};
use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use swaggen_domain::workspace::{
    CookieRecord, HttpMethod, MultipartValueKind, PREVIEW_CAP_BYTES, RequestBody, ResolvedRequest,
};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpErrorKind {
    Canceled,
    Timeout,
    Dns,
    ConnectionRefused,
    TlsVerification,
    RedirectLimit,
    InvalidRequest,
    LocalFileMissing,
    ResponseIo,
}

impl HttpErrorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Canceled => "CANCELED",
            Self::Timeout => "TIMEOUT",
            Self::Dns => "DNS_ERROR",
            Self::ConnectionRefused => "CONNECTION_REFUSED",
            Self::TlsVerification => "TLS_ERROR",
            Self::RedirectLimit => "REDIRECT_LIMIT",
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::LocalFileMissing => "LOCAL_FILE_MISSING",
            Self::ResponseIo => "RESPONSE_IO",
        }
    }

    pub fn guidance(&self) -> &'static str {
        match self {
            Self::Canceled => "Request was canceled before completion.",
            Self::Timeout => "Increase timeout in Connection settings or verify server latency.",
            Self::Dns => "Check hostname spelling, DNS configuration, or offline state.",
            Self::ConnectionRefused => {
                "Target server refused TCP connection; verify port and service status."
            }
            Self::TlsVerification => {
                "Certificate verification failed. Check trust chain or add a host-scoped exception only for trusted local hosts."
            }
            Self::RedirectLimit => {
                "Exceeded maximum redirects; adjust redirect limit or inspect Location headers."
            }
            Self::InvalidRequest => "Fix invalid URL, headers, or body formatting before sending.",
            Self::LocalFileMissing => "Selected upload file no longer exists on disk.",
            Self::ResponseIo => "Failed while reading or writing response stream to disk.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpExecutionError {
    pub kind: HttpErrorKind,
    pub message: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedirectHop {
    pub status: u16,
    pub from_url: String,
    pub to_url: String,
    pub stripped_auth: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TimingBreakdown {
    pub dns_ms: Option<u64>,
    pub tcp_ms: Option<u64>,
    pub tls_ms: Option<u64>,
    pub ttfb_ms: Option<u64>,
    pub total_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferProgress {
    pub phase: &'static str,
    pub bytes_done: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponseData {
    pub status_code: u16,
    pub reason: String,
    pub headers: Vec<(String, String)>,
    pub captured_cookies: Vec<CookieRecord>,
    pub redirect_chain: Vec<RedirectHop>,
    pub final_url: String,
    pub duration_ms: u64,
    pub wire_bytes: u64,
    pub decoded_bytes: u64,
    pub preview_bytes: Vec<u8>,
    pub preview_truncated: bool,
    pub is_binary: bool,
    pub raw_body_path: PathBuf,
    pub tls_exception_used: bool,
    pub timings: TimingBreakdown,
}

#[derive(Debug)]
struct HostScopedVerifier {
    inner: Arc<WebPkiServerVerifier>,
    allowed_hosts: BTreeSet<String>,
}

impl ServerCertVerifier for HostScopedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        match self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        ) {
            Ok(v) => Ok(v),
            Err(err) => {
                let host = server_name.to_str().to_ascii_lowercase();
                if self.allowed_hosts.contains(&host) {
                    Ok(ServerCertVerified::assertion())
                } else {
                    Err(err)
                }
            }
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

pub async fn execute_request(
    resolved: &ResolvedRequest,
    downloads_dir: &Path,
    cancel: CancellationToken,
) -> Result<HttpResponseData, HttpExecutionError> {
    execute_request_with_progress(resolved, downloads_dir, cancel, None).await
}

pub async fn execute_request_with_progress(
    resolved: &ResolvedRequest,
    downloads_dir: &Path,
    cancel: CancellationToken,
    progress: Option<tokio::sync::mpsc::UnboundedSender<TransferProgress>>,
) -> Result<HttpResponseData, HttpExecutionError> {
    let started = Instant::now();
    let elapsed_ms = || started.elapsed().as_millis() as u64;

    if !resolved.blocking_errors.is_empty() {
        return Err(HttpExecutionError {
            kind: HttpErrorKind::InvalidRequest,
            message: resolved.blocking_errors.join("; "),
            duration_ms: elapsed_ms(),
        });
    }

    fs::create_dir_all(downloads_dir).map_err(|e| HttpExecutionError {
        kind: HttpErrorKind::ResponseIo,
        message: format!("Cannot prepare downloads directory: {e}"),
        duration_ms: elapsed_ms(),
    })?;

    let (body_bytes, generated_content_type) =
        encode_body(&resolved.body).map_err(|err| HttpExecutionError {
            kind: err.0,
            message: err.1,
            duration_ms: elapsed_ms(),
        })?;
    if let Some(ref tx) = progress {
        let _ = tx.send(TransferProgress {
            phase: "upload_ready",
            bytes_done: body_bytes.len() as u64,
            total_bytes: Some(body_bytes.len() as u64),
        });
    }

    let timeout = Duration::from_millis(resolved.connection.timeout_ms.max(100));
    let fut = execute_with_redirects(
        resolved,
        body_bytes,
        generated_content_type,
        downloads_dir,
        started,
        progress,
    );

    tokio::select! {
        _ = cancel.cancelled() => Err(HttpExecutionError {
            kind: HttpErrorKind::Canceled,
            message: "Canceled by user".into(),
            duration_ms: elapsed_ms(),
        }),
        timed = tokio::time::timeout(timeout, fut) => match timed {
            Ok(res) => res,
            Err(_) => Err(HttpExecutionError {
                kind: HttpErrorKind::Timeout,
                message: format!("Timed out after {} ms", resolved.connection.timeout_ms),
                duration_ms: elapsed_ms(),
            }),
        }
    }
}

async fn execute_with_redirects(
    resolved: &ResolvedRequest,
    initial_body: Vec<u8>,
    generated_content_type: Option<String>,
    downloads_dir: &Path,
    started: Instant,
    progress: Option<tokio::sync::mpsc::UnboundedSender<TransferProgress>>,
) -> Result<HttpResponseData, HttpExecutionError> {
    let elapsed_ms = || started.elapsed().as_millis() as u64;
    let mut current_url = url::Url::parse(&resolved.url).map_err(|e| HttpExecutionError {
        kind: HttpErrorKind::InvalidRequest,
        message: format!("Invalid URL: {e}"),
        duration_ms: elapsed_ms(),
    })?;
    let mut current_method = to_hyper_method(resolved.method);
    let mut current_headers: Vec<(String, String)> = resolved
        .headers
        .iter()
        .map(|h| (h.name.clone(), h.value.clone()))
        .collect();
    if let Some(ct) = generated_content_type
        && !current_headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
    {
        current_headers.push(("Content-Type".into(), ct));
    }
    let mut current_body = initial_body;
    let mut redirect_chain = Vec::new();
    let mut captured_cookies = Vec::new();
    let mut tls_exception_used = false;
    let max_hops = resolved.connection.max_redirects.min(20);

    loop {
        let host_lower = current_url
            .host_str()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let port_host = match current_url.port() {
            Some(p) => format!("{host_lower}:{p}"),
            None => host_lower.clone(),
        };
        let host_allowed_tls = resolved.connection.tls_host_exceptions.iter().any(|e| {
            let e = e.trim().to_ascii_lowercase();
            e == host_lower || e == port_host
        });
        if current_url.scheme() == "https" && host_allowed_tls {
            tls_exception_used = true;
        }

        let (status, reason, resp_headers, raw_wire, mut timings) = send_single_http(
            &current_url,
            current_method.clone(),
            &current_headers,
            &current_body,
            &resolved.connection,
            started,
            progress.as_ref(),
        )
        .await?;

        let now_secs = time::OffsetDateTime::now_utc().unix_timestamp();
        for (name, val) in &resp_headers {
            if name.eq_ignore_ascii_case("set-cookie")
                && let Some(cookie) = CookieRecord::parse_set_cookie(val, &host_lower, now_secs)
            {
                captured_cookies.push(cookie);
            }
        }

        let is_redirect = matches!(status, 301 | 302 | 303 | 307 | 308);
        if is_redirect && resolved.connection.follow_redirects {
            if redirect_chain.len() >= max_hops {
                return Err(HttpExecutionError {
                    kind: HttpErrorKind::RedirectLimit,
                    message: format!("Exceeded max redirects ({max_hops})"),
                    duration_ms: elapsed_ms(),
                });
            }
            if let Some((_, loc)) = resp_headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("location"))
                && let Ok(next_url) = current_url.join(loc)
            {
                let cross_host = next_url.host_str() != current_url.host_str()
                    || next_url.port_or_known_default() != current_url.port_or_known_default()
                    || next_url.scheme() != current_url.scheme();
                let mut stripped_auth = false;
                if cross_host && !resolved.connection.send_auth_on_cross_host_redirect {
                    let before = current_headers.len();
                    current_headers.retain(|(k, _)| {
                        !k.eq_ignore_ascii_case("authorization")
                            && !k.eq_ignore_ascii_case("proxy-authorization")
                            && !k.eq_ignore_ascii_case("cookie")
                    });
                    stripped_auth = current_headers.len() != before;
                }
                if matches!(status, 301..=303)
                    && !matches!(current_method, Method::GET | Method::HEAD)
                {
                    current_method = Method::GET;
                    current_body.clear();
                }
                redirect_chain.push(RedirectHop {
                    status,
                    from_url: current_url.to_string(),
                    to_url: next_url.to_string(),
                    stripped_auth,
                });
                current_url = next_url;
                continue;
            }
        }

        // Write final body to disk and capture bounded <= 1 MiB preview.
        let wire_bytes = raw_wire.len() as u64;
        let encoding = resp_headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-encoding"))
            .map(|(_, v)| v.to_ascii_lowercase());
        let decoded =
            decode_content(&raw_wire, encoding.as_deref()).map_err(|e| HttpExecutionError {
                kind: HttpErrorKind::ResponseIo,
                message: format!("Decompression failed: {e}"),
                duration_ms: elapsed_ms(),
            })?;
        let decoded_bytes = decoded.len() as u64;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let raw_path = downloads_dir.join(format!("resp-{}-{stamp}.bin", std::process::id()));
        let mut out_file = File::create(&raw_path).map_err(|e| HttpExecutionError {
            kind: HttpErrorKind::ResponseIo,
            message: format!("Failed to create response file: {e}"),
            duration_ms: elapsed_ms(),
        })?;
        out_file
            .write_all(&decoded)
            .map_err(|e| HttpExecutionError {
                kind: HttpErrorKind::ResponseIo,
                message: format!("Failed to write response file: {e}"),
                duration_ms: elapsed_ms(),
            })?;
        out_file.sync_all().map_err(|e| HttpExecutionError {
            kind: HttpErrorKind::ResponseIo,
            message: format!("Failed to flush response file: {e}"),
            duration_ms: elapsed_ms(),
        })?;

        let preview_truncated = decoded.len() > PREVIEW_CAP_BYTES;
        let preview_slice = &decoded[..decoded.len().min(PREVIEW_CAP_BYTES)];
        let is_binary = looks_binary(preview_slice, &resp_headers);
        timings.total_ms = elapsed_ms();
        return Ok(HttpResponseData {
            status_code: status,
            reason,
            headers: resp_headers,
            captured_cookies,
            redirect_chain,
            final_url: current_url.to_string(),
            duration_ms: timings.total_ms,
            wire_bytes,
            decoded_bytes,
            preview_bytes: preview_slice.to_vec(),
            preview_truncated,
            is_binary,
            raw_body_path: raw_path,
            tls_exception_used,
            timings,
        });
    }
}

fn decode_pem_blocks(pem: &str, expected_label_sub: &str) -> Result<Vec<Vec<u8>>, String> {
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut b64 = String::new();
    for line in pem.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("-----BEGIN ")
            && rest.contains(expected_label_sub)
        {
            in_block = true;
            b64.clear();
            continue;
        }
        if t.starts_with("-----END ") && in_block {
            in_block = false;
            let bytes = STANDARD
                .decode(&b64)
                .map_err(|e| format!("Invalid PEM base64: {e}"))?;
            blocks.push(bytes);
            continue;
        }
        if in_block {
            b64.push_str(t);
        }
    }
    Ok(blocks)
}

fn should_bypass_proxy(host: &str, bypass: &[String]) -> bool {
    let h = host.to_ascii_lowercase();
    bypass.iter().any(|pat| {
        let p = pat.trim().to_ascii_lowercase();
        !p.is_empty() && (h == p || h.ends_with(&format!(".{p}")))
    })
}

async fn send_single_http(
    url: &url::Url,
    method: Method,
    headers: &[(String, String)],
    body: &[u8],
    policy: &swaggen_domain::workspace::RequestConnectionPolicy,
    started: Instant,
    progress: Option<&tokio::sync::mpsc::UnboundedSender<TransferProgress>>,
) -> Result<(u16, String, Vec<(String, String)>, Vec<u8>, TimingBreakdown), HttpExecutionError> {
    let elapsed_ms = || started.elapsed().as_millis() as u64;
    let host = url.host_str().ok_or_else(|| HttpExecutionError {
        kind: HttpErrorKind::InvalidRequest,
        message: "Missing host in URL".into(),
        duration_ms: elapsed_ms(),
    })?;
    let port = url.port_or_known_default().unwrap_or(80);
    let mut target_host = host.to_owned();
    let mut target_port = port;
    let mut using_http_proxy = false;
    if url.scheme() == "http"
        && let Some(ref proxy_raw) = policy.proxy.http_proxy
        && !proxy_raw.trim().is_empty()
        && !should_bypass_proxy(host, &policy.proxy.bypass_hosts)
        && let Ok(pu) = url::Url::parse(proxy_raw.trim())
        && let Some(ph) = pu.host_str()
    {
        target_host = ph.to_owned();
        target_port = pu.port_or_known_default().unwrap_or(8080);
        using_http_proxy = true;
    }

    let uri_str = if using_http_proxy {
        url.as_str().to_owned()
    } else {
        let mut path_q = url.path().to_owned();
        if let Some(q) = url.query() {
            path_q.push('?');
            path_q.push_str(q);
        }
        path_q
    };
    let uri: hyper::Uri = uri_str.parse().map_err(|e| HttpExecutionError {
        kind: HttpErrorKind::InvalidRequest,
        message: format!("Invalid URI: {e}"),
        duration_ms: elapsed_ms(),
    })?;

    let mut header_map = HeaderMap::new();
    for (k, v) in headers {
        let name = HeaderName::from_bytes(k.as_bytes()).map_err(|e| HttpExecutionError {
            kind: HttpErrorKind::InvalidRequest,
            message: format!("Invalid header name '{k}': {e}"),
            duration_ms: elapsed_ms(),
        })?;
        let val = HeaderValue::from_str(v).map_err(|e| HttpExecutionError {
            kind: HttpErrorKind::InvalidRequest,
            message: format!("Invalid header value for '{k}': {e}"),
            duration_ms: elapsed_ms(),
        })?;
        header_map.append(name, val);
    }
    if !header_map.contains_key(http::header::HOST) {
        let host_hdr = if let Some(p) = url.port() {
            format!("{host}:{p}")
        } else {
            host.to_owned()
        };
        if let Ok(v) = HeaderValue::from_str(&host_hdr) {
            header_map.insert(http::header::HOST, v);
        }
    }
    if !header_map.contains_key(http::header::USER_AGENT) {
        header_map.insert(
            http::header::USER_AGENT,
            HeaderValue::from_static("Swaggen/0.1"),
        );
    }
    if !header_map.contains_key(http::header::ACCEPT_ENCODING) {
        header_map.insert(
            http::header::ACCEPT_ENCODING,
            HeaderValue::from_static("gzip, deflate"),
        );
    }

    let mut req_builder = hyper::Request::builder().method(method).uri(uri);
    if let Some(map) = req_builder.headers_mut() {
        *map = header_map;
    }
    let req = req_builder
        .body(Full::new(Bytes::copy_from_slice(body)))
        .map_err(|e| HttpExecutionError {
            kind: HttpErrorKind::InvalidRequest,
            message: format!("Request build error: {e}"),
            duration_ms: elapsed_ms(),
        })?;

    let t_dns = Instant::now();
    let addrs: Vec<std::net::SocketAddr> =
        tokio::net::lookup_host((target_host.as_str(), target_port))
            .await
            .map_err(|e| classify_io_error(&e, elapsed_ms()))?
            .collect();
    if addrs.is_empty() {
        return Err(HttpExecutionError {
            kind: HttpErrorKind::Dns,
            message: format!("DNS returned no addresses for '{target_host}'"),
            duration_ms: elapsed_ms(),
        });
    }
    let dns_ms = Some(t_dns.elapsed().as_millis() as u64);

    let t_tcp = Instant::now();
    let stream = TcpStream::connect(addrs.as_slice())
        .await
        .map_err(|e| classify_io_error(&e, elapsed_ms()))?;
    let tcp_ms = Some(t_tcp.elapsed().as_millis() as u64);

    let (resp, tls_ms) = match url.scheme() {
        "http" => {
            let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
                .await
                .map_err(|e| classify_hyper_error(&e.to_string(), elapsed_ms()))?;
            tokio::spawn(async move {
                let _ = conn.await;
            });
            let t_req = Instant::now();
            let r = sender
                .send_request(req)
                .await
                .map_err(|e| classify_hyper_error(&e.to_string(), elapsed_ms()))?;
            (r, Some((None, t_req.elapsed().as_millis() as u64)))
        }
        "https" => {
            let mut root_store = rustls::RootCertStore::empty();
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            for ca_path in &policy.custom_ca_pem_paths {
                let pem = fs::read_to_string(ca_path).map_err(|e| HttpExecutionError {
                    kind: HttpErrorKind::TlsVerification,
                    message: format!("Failed to read custom CA file '{ca_path}': {e}"),
                    duration_ms: elapsed_ms(),
                })?;
                for der in
                    decode_pem_blocks(&pem, "CERTIFICATE").map_err(|e| HttpExecutionError {
                        kind: HttpErrorKind::TlsVerification,
                        message: e,
                        duration_ms: elapsed_ms(),
                    })?
                {
                    let _ = root_store.add(CertificateDer::from(der));
                }
            }

            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let inner =
                WebPkiServerVerifier::builder_with_provider(Arc::new(root_store), provider.clone())
                    .build()
                    .map_err(|e| HttpExecutionError {
                        kind: HttpErrorKind::TlsVerification,
                        message: format!("TLS verifier error: {e}"),
                        duration_ms: elapsed_ms(),
                    })?;
            let allowed_hosts: BTreeSet<String> = policy
                .tls_host_exceptions
                .iter()
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            let verifier = Arc::new(HostScopedVerifier {
                inner,
                allowed_hosts,
            });
            let base_builder = ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .map_err(|e| HttpExecutionError {
                    kind: HttpErrorKind::TlsVerification,
                    message: format!("TLS protocol config error: {e}"),
                    duration_ms: elapsed_ms(),
                })?
                .dangerous()
                .with_custom_certificate_verifier(verifier);

            let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
            let matched_cert = policy
                .client_certs
                .iter()
                .find(|c| c.host.eq_ignore_ascii_case(host) && (c.port == 0 || c.port == port));
            let config = if let Some(mc) = matched_cert {
                if let Some(exp) = mc.expires_at_unix
                    && exp <= now_unix
                {
                    return Err(HttpExecutionError {
                        kind: HttpErrorKind::TlsVerification,
                        message: format!("Client mTLS certificate for {host} expired"),
                        duration_ms: elapsed_ms(),
                    });
                }
                let cert_pem =
                    fs::read_to_string(&mc.cert_pem_path).map_err(|e| HttpExecutionError {
                        kind: HttpErrorKind::TlsVerification,
                        message: format!("Failed to read mTLS client cert: {e}"),
                        duration_ms: elapsed_ms(),
                    })?;
                let key_pem =
                    fs::read_to_string(&mc.key_pem_path).map_err(|e| HttpExecutionError {
                        kind: HttpErrorKind::TlsVerification,
                        message: format!("Failed to read mTLS client key: {e}"),
                        duration_ms: elapsed_ms(),
                    })?;
                let certs: Vec<CertificateDer<'static>> =
                    decode_pem_blocks(&cert_pem, "CERTIFICATE")
                        .map_err(|e| HttpExecutionError {
                            kind: HttpErrorKind::TlsVerification,
                            message: e,
                            duration_ms: elapsed_ms(),
                        })?
                        .into_iter()
                        .map(CertificateDer::from)
                        .collect();
                let key_der = decode_pem_blocks(&key_pem, "PRIVATE KEY")
                    .map_err(|e| HttpExecutionError {
                        kind: HttpErrorKind::TlsVerification,
                        message: e,
                        duration_ms: elapsed_ms(),
                    })?
                    .into_iter()
                    .next()
                    .ok_or_else(|| HttpExecutionError {
                        kind: HttpErrorKind::TlsVerification,
                        message: "No private key found in client cert PEM".into(),
                        duration_ms: elapsed_ms(),
                    })?;
                base_builder
                    .with_client_auth_cert(
                        certs,
                        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_der)),
                    )
                    .map_err(|e| HttpExecutionError {
                        kind: HttpErrorKind::TlsVerification,
                        message: format!("mTLS client cert rejected: {e}"),
                        duration_ms: elapsed_ms(),
                    })?
            } else {
                base_builder.with_no_client_auth()
            };

            let connector = TlsConnector::from(Arc::new(config));
            let server_name =
                ServerName::try_from(host.to_owned()).map_err(|_| HttpExecutionError {
                    kind: HttpErrorKind::InvalidRequest,
                    message: format!("Invalid TLS server name '{host}'"),
                    duration_ms: elapsed_ms(),
                })?;
            let t_tls = Instant::now();
            let tls_stream =
                connector
                    .connect(server_name, stream)
                    .await
                    .map_err(|e| HttpExecutionError {
                        kind: HttpErrorKind::TlsVerification,
                        message: format!("TLS handshake failed: {e}"),
                        duration_ms: elapsed_ms(),
                    })?;
            let tls_dur = t_tls.elapsed().as_millis() as u64;
            let (mut sender, conn) =
                hyper::client::conn::http1::handshake(TokioIo::new(tls_stream))
                    .await
                    .map_err(|e| classify_hyper_error(&e.to_string(), elapsed_ms()))?;
            tokio::spawn(async move {
                let _ = conn.await;
            });
            let t_req = Instant::now();
            let r = sender
                .send_request(req)
                .await
                .map_err(|e| classify_hyper_error(&e.to_string(), elapsed_ms()))?;
            (r, Some((Some(tls_dur), t_req.elapsed().as_millis() as u64)))
        }
        other => {
            return Err(HttpExecutionError {
                kind: HttpErrorKind::InvalidRequest,
                message: format!("Unsupported URL scheme '{other}'"),
                duration_ms: elapsed_ms(),
            });
        }
    };

    let (tls_opt, ttfb_ms) = match tls_ms {
        Some((t, f)) => (t, Some(f)),
        None => (None, None),
    };
    let timings = TimingBreakdown {
        dns_ms,
        tcp_ms,
        tls_ms: tls_opt,
        ttfb_ms,
        total_ms: elapsed_ms(),
    };

    let status = resp.status().as_u16();
    let reason = resp.status().canonical_reason().unwrap_or("").to_owned();
    let resp_headers: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect();
    let expected_total = resp
        .headers()
        .get(http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    let mut incoming_body = resp.into_body();
    let mut wire_buf = Vec::new();
    while let Some(frame_res) = incoming_body.frame().await {
        let frame = frame_res.map_err(|e| HttpExecutionError {
            kind: HttpErrorKind::ResponseIo,
            message: format!("Response body error: {e}"),
            duration_ms: elapsed_ms(),
        })?;
        if let Some(chunk) = frame.data_ref() {
            wire_buf.extend_from_slice(chunk);
            if let Some(tx) = progress {
                let _ = tx.send(TransferProgress {
                    phase: "downloading",
                    bytes_done: wire_buf.len() as u64,
                    total_bytes: expected_total,
                });
            }
        }
    }
    Ok((status, reason, resp_headers, wire_buf, timings))
}

fn encode_body(body: &RequestBody) -> Result<(Vec<u8>, Option<String>), (HttpErrorKind, String)> {
    match body {
        RequestBody::None => Ok((Vec::new(), None)),
        RequestBody::Json(s) => Ok((s.as_bytes().to_vec(), Some("application/json".into()))),
        RequestBody::Text(s) => Ok((
            s.as_bytes().to_vec(),
            Some("text/plain; charset=utf-8".into()),
        )),
        RequestBody::Xml(s) => Ok((s.as_bytes().to_vec(), Some("application/xml".into()))),
        RequestBody::FormUrlEncoded(rows) => {
            let mut ser = url::form_urlencoded::Serializer::new(String::new());
            for row in rows.iter().filter(|r| r.enabled && !r.key.is_empty()) {
                ser.append_pair(&row.key, &row.value);
            }
            Ok((
                ser.finish().into_bytes(),
                Some("application/x-www-form-urlencoded".into()),
            ))
        }
        RequestBody::Multipart(fields) => {
            let boundary = format!("swaggen-boundary-{}", std::process::id());
            let mut buf = Vec::new();
            for f in fields.iter().filter(|f| f.enabled && !f.name.is_empty()) {
                buf.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
                match &f.value {
                    MultipartValueKind::Text(val) => {
                        buf.extend_from_slice(
                            format!(
                                "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                                f.name
                            )
                            .as_bytes(),
                        );
                        buf.extend_from_slice(val.as_bytes());
                        buf.extend_from_slice(b"\r\n");
                    }
                    MultipartValueKind::File {
                        local_path,
                        file_name,
                        content_type,
                    } => {
                        let bytes = fs::read(local_path).map_err(|_| {
                            (
                                HttpErrorKind::LocalFileMissing,
                                format!("Missing upload file: {file_name}"),
                            )
                        })?;
                        let ct = if content_type.is_empty() {
                            "application/octet-stream"
                        } else {
                            content_type.as_str()
                        };
                        buf.extend_from_slice(
                            format!(
                                "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\nContent-Type: {ct}\r\n\r\n",
                                f.name, file_name
                            )
                            .as_bytes(),
                        );
                        buf.extend_from_slice(&bytes);
                        buf.extend_from_slice(b"\r\n");
                    }
                }
            }
            buf.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
            Ok((
                buf,
                Some(format!("multipart/form-data; boundary={boundary}")),
            ))
        }
        RequestBody::Binary {
            local_path,
            content_type,
        } => {
            let Some(path) = local_path else {
                return Err((
                    HttpErrorKind::LocalFileMissing,
                    "No binary file chosen".into(),
                ));
            };
            let bytes = fs::read(path).map_err(|_| {
                (
                    HttpErrorKind::LocalFileMissing,
                    "Binary body file not found".into(),
                )
            })?;
            let ct = if content_type.is_empty() {
                "application/octet-stream".into()
            } else {
                content_type.clone()
            };
            Ok((bytes, Some(ct)))
        }
    }
}

fn decode_content(raw: &[u8], encoding: Option<&str>) -> std::io::Result<Vec<u8>> {
    match encoding {
        Some(enc) if enc.contains("gzip") => {
            let mut dec = GzDecoder::new(raw);
            let mut out = Vec::new();
            dec.read_to_end(&mut out)?;
            Ok(out)
        }
        Some(enc) if enc.contains("deflate") => {
            let mut dec = DeflateDecoder::new(raw);
            let mut out = Vec::new();
            dec.read_to_end(&mut out)?;
            Ok(out)
        }
        _ => Ok(raw.to_vec()),
    }
}

fn looks_binary(bytes: &[u8], headers: &[(String, String)]) -> bool {
    if bytes.contains(&0) {
        return true;
    }
    if let Some((_, ct)) = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
    {
        let lower = ct.to_ascii_lowercase();
        if lower.starts_with("image/")
            || lower.starts_with("video/")
            || lower.starts_with("audio/")
            || lower.contains("octet-stream")
            || lower.contains("pdf")
        {
            return true;
        }
    }
    std::str::from_utf8(bytes).is_err()
}

fn to_hyper_method(method: HttpMethod) -> Method {
    match method {
        HttpMethod::Get => Method::GET,
        HttpMethod::Post => Method::POST,
        HttpMethod::Put => Method::PUT,
        HttpMethod::Patch => Method::PATCH,
        HttpMethod::Delete => Method::DELETE,
        HttpMethod::Head => Method::HEAD,
        HttpMethod::Options => Method::OPTIONS,
    }
}

fn classify_io_error(err: &std::io::Error, duration_ms: u64) -> HttpExecutionError {
    let kind = match err.kind() {
        std::io::ErrorKind::ConnectionRefused => HttpErrorKind::ConnectionRefused,
        std::io::ErrorKind::TimedOut => HttpErrorKind::Timeout,
        _ => HttpErrorKind::Dns,
    };
    HttpExecutionError {
        kind,
        message: err.to_string(),
        duration_ms,
    }
}

fn classify_hyper_error(msg: &str, duration_ms: u64) -> HttpExecutionError {
    let lower = msg.to_ascii_lowercase();
    let kind = if lower.contains("refused") {
        HttpErrorKind::ConnectionRefused
    } else if lower.contains("dns") || lower.contains("resolve") || lower.contains("lookup") {
        HttpErrorKind::Dns
    } else if lower.contains("timed out") {
        HttpErrorKind::Timeout
    } else {
        HttpErrorKind::ResponseIo
    };
    HttpExecutionError {
        kind,
        message: msg.to_owned(),
        duration_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::collections::BTreeMap;
    use swaggen_domain::workspace::{
        KeyValueRow, RequestItem, VariableContext, resolve_request_preview,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn start_fixture_server() -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 65536];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let raw = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let first_line = raw.lines().next().unwrap_or("");
                    if first_line.contains("/echo") {
                        let body = serde_json::json!({
                            "request_line": first_line,
                            "raw_headers": raw.split("\r\n\r\n").next().unwrap_or("")
                        })
                        .to_string();
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nSet-Cookie: sid=ck123; Path=/\r\nX-Dup: one\r\nX-Dup: two\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = sock.write_all(resp.as_bytes()).await;
                    } else if first_line.contains("/gzip") {
                        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
                        let repeated = "hello-world-".repeat(32);
                        let plain = format!(r#"{{"compressed":"{repeated}"}}"#);
                        enc.write_all(plain.as_bytes()).unwrap();
                        let gz = enc.finish().unwrap();
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            gz.len()
                        );
                        let _ = sock.write_all(head.as_bytes()).await;
                        let _ = sock.write_all(&gz).await;
                    } else if first_line.contains("/redirect-other") {
                        let target_port = addr.port();
                        // Redirect from 127.0.0.1 -> localhost (cross-host!)
                        let resp = format!(
                            "HTTP/1.1 302 Found\r\nLocation: http://localhost:{target_port}/echo\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        );
                        let _ = sock.write_all(resp.as_bytes()).await;
                    } else if first_line.contains("/slow") {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        let _ = sock
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                            .await;
                    } else if first_line.contains("/large") {
                        let total = 20 * 1024 * 1024usize;
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n"
                        );
                        let _ = sock.write_all(head.as_bytes()).await;
                        let chunk = vec![b'A'; 64 * 1024];
                        let mut sent = 0;
                        while sent < total {
                            let to_send = (total - sent).min(chunk.len());
                            if sock.write_all(&chunk[..to_send]).await.is_err() {
                                break;
                            }
                            sent += to_send;
                        }
                    }
                });
            }
        });
        addr
    }

    fn temp_downloads(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "swaggen-http-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[tokio::test]
    async fn duplicate_query_headers_gzip_cookies_and_cross_host_redirect_work() {
        let addr = start_fixture_server().await;
        let dl = temp_downloads("echo");
        let ctx = VariableContext {
            workspace_vars: &[],
            collection_vars: &[],
            environment: None,
            request_vars: &[],
            local_secrets: &BTreeMap::new(),
        };

        // 1. Duplicate query & headers + Set-Cookie capture.
        let mut req =
            RequestItem::new("c1", "Echo", HttpMethod::Get, format!("http://{addr}/echo"));
        req.query_params.push(KeyValueRow::new("k", "1"));
        req.query_params.push(KeyValueRow::new("k", "2"));
        req.headers.push(KeyValueRow::new("X-Custom", "a"));
        req.headers.push(KeyValueRow::new("X-Custom", "b"));
        let resolved = resolve_request_preview(&req, None, &[], &ctx, &[], 100);
        let res = execute_request(&resolved, &dl, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(res.status_code, 200);
        assert_eq!(res.captured_cookies.len(), 1);
        assert_eq!(res.captured_cookies[0].name, "sid");
        let body_txt = String::from_utf8(res.preview_bytes).unwrap();
        assert!(body_txt.contains("k=1&k=2"));
        assert_eq!(
            res.headers
                .iter()
                .filter(|(k, _)| k.eq_ignore_ascii_case("x-dup"))
                .count(),
            2
        );

        // 2. Gzip wire vs decoded size accounting.
        let gz_req = RequestItem::new("c1", "Gz", HttpMethod::Get, format!("http://{addr}/gzip"));
        let gz_resolved = resolve_request_preview(&gz_req, None, &[], &ctx, &[], 100);
        let gz_res = execute_request(&gz_resolved, &dl, CancellationToken::new())
            .await
            .unwrap();
        assert!(gz_res.decoded_bytes > gz_res.wire_bytes);
        assert!(String::from_utf8_lossy(&gz_res.preview_bytes).contains("hello-world"));

        // 3. Cross-host redirect strips Authorization.
        let mut red_req = RequestItem::new(
            "c1",
            "Redir",
            HttpMethod::Get,
            format!("http://127.0.0.1:{}/redirect-other", addr.port()),
        );
        red_req
            .headers
            .push(KeyValueRow::new("Authorization", "Bearer top-secret-token"));
        let red_resolved = resolve_request_preview(&red_req, None, &[], &ctx, &[], 100);
        let red_res = execute_request(&red_resolved, &dl, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(red_res.redirect_chain.len(), 1);
        assert!(red_res.redirect_chain[0].stripped_auth);
        assert!(!String::from_utf8_lossy(&red_res.preview_bytes).contains("top-secret-token"));

        let _ = fs::remove_dir_all(dl);
    }

    #[tokio::test]
    async fn timeout_cancel_and_20mb_streaming_preview_cap_work() {
        let addr = start_fixture_server().await;
        let dl = temp_downloads("stream");
        let ctx = VariableContext {
            workspace_vars: &[],
            collection_vars: &[],
            environment: None,
            request_vars: &[],
            local_secrets: &BTreeMap::new(),
        };

        // 1. Timeout.
        let mut slow =
            RequestItem::new("c1", "Slow", HttpMethod::Get, format!("http://{addr}/slow"));
        slow.connection.timeout_ms = 150;
        let slow_resolved = resolve_request_preview(&slow, None, &[], &ctx, &[], 100);
        let err = execute_request(&slow_resolved, &dl, CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(err.kind, HttpErrorKind::Timeout);

        // 2. Cancellation.
        let mut cancel_req = RequestItem::new(
            "c1",
            "Cancel",
            HttpMethod::Get,
            format!("http://{addr}/slow"),
        );
        cancel_req.connection.timeout_ms = 10_000;
        let cancel_resolved = resolve_request_preview(&cancel_req, None, &[], &ctx, &[], 100);
        let token = CancellationToken::new();
        let trigger = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            trigger.cancel();
        });
        let cerr = execute_request(&cancel_resolved, &dl, token)
            .await
            .unwrap_err();
        assert_eq!(cerr.kind, HttpErrorKind::Canceled);

        // 3. 20 MiB response streamed to file with <= 1 MiB preview cap.
        let large_req = RequestItem::new(
            "c1",
            "Large",
            HttpMethod::Get,
            format!("http://{addr}/large"),
        );
        let large_resolved = resolve_request_preview(&large_req, None, &[], &ctx, &[], 100);
        let lres = execute_request(&large_resolved, &dl, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(lres.decoded_bytes, 20 * 1024 * 1024);
        assert_eq!(lres.preview_bytes.len(), PREVIEW_CAP_BYTES);
        assert!(lres.preview_truncated);
        assert_eq!(
            fs::metadata(&lres.raw_body_path).unwrap().len(),
            20 * 1024 * 1024
        );
        assert!(lres.timings.dns_ms.is_some());
        assert!(lres.timings.tcp_ms.is_some());
        assert!(lres.timings.tls_ms.is_none());
        assert!(lres.timings.ttfb_ms.is_some());

        // 4. Streaming progress channel reports phase updates (RQ10)
        let (ptx, mut prx) = tokio::sync::mpsc::unbounded_channel();
        let p_req = RequestItem::new("c1", "Prog", HttpMethod::Get, format!("http://{addr}/gzip"));
        let p_res = execute_request_with_progress(
            &resolve_request_preview(&p_req, None, &[], &ctx, &[], 100),
            &dl,
            CancellationToken::new(),
            Some(ptx),
        )
        .await
        .unwrap();
        assert_eq!(p_res.status_code, 200);
        let mut phases = Vec::new();
        while let Ok(ev) = prx.try_recv() {
            phases.push(ev.phase);
        }
        assert!(phases.contains(&"upload_ready"));
        assert!(phases.contains(&"downloading"));

        let _ = fs::remove_dir_all(dl);
    }
}
