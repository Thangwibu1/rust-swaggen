//! Deterministic Windows release benchmark for P06 (AC09, AC10, 06-quality/performance-and-security.md).
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};
use swaggen_domain::{
    LocalId,
    workspace::{
        HttpMethod, PREVIEW_CAP_BYTES, RequestConnectionPolicy, RequestItem, ResolvedRequest,
        WorkspaceState,
    },
};
use swaggen_http_engine::execute_request;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

fn percentile_ms(samples: &mut [f64], p: f64) -> f64 {
    samples.sort_by(f64::total_cmp);
    if samples.is_empty() {
        return 0.0;
    }
    let idx = ((samples.len() as f64 - 1.0) * p).round() as usize;
    samples[idx.min(samples.len() - 1)]
}

#[tokio::main]
async fn main() {
    // 1. 1,000-request WorkspaceState construction + serialization + round-trip open + search (5 warm-up + 40 samples)
    let ws_id = LocalId::parse("bench1k").unwrap();
    let mut state = WorkspaceState::new_personal(ws_id, "Bench 1K");
    let col_id = state.collections[0].id.clone();
    for i in 0..999 {
        let mut req = RequestItem::new(
            col_id.clone(),
            format!("Order Endpoint {i:04}"),
            if i % 3 == 0 { HttpMethod::Post } else { HttpMethod::Get },
            format!("https://api.example.test/v1/orders/{i}?filter=active"),
        );
        req.tags = vec![format!("grp-{}", i % 10)];
        state.requests.push(req);
    }
    let payload = serde_json::to_vec(&state).unwrap();

    for _ in 0..5 {
        let parsed: WorkspaceState = serde_json::from_slice(&payload).unwrap();
        let _hits = parsed.search_requests("0555", None, None);
    }
    let mut open_search_samples = Vec::with_capacity(40);
    for _ in 0..40 {
        let t0 = Instant::now();
        let parsed: WorkspaceState = serde_json::from_slice(&payload).unwrap();
        let hits = parsed.search_requests("Order Endpoint 07", None, None);
        assert!(!hits.is_empty());
        open_search_samples.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    let p50_1k = percentile_ms(&mut open_search_samples, 0.50);
    let p95_1k = percentile_ms(&mut open_search_samples, 0.95);
    let p99_1k = percentile_ms(&mut open_search_samples, 0.99);

    // 2. 20 MiB streaming response (5 warm-up + 10 samples) + cancel benchmark
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let chunk = vec![b'x'; 65536];
        loop {
            if let Ok((mut sock, _)) = listener.accept().await {
                let c = chunk.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 2048];
                    let _ = sock.read(&mut buf).await;
                    let total = 20 * 1024 * 1024usize;
                    if String::from_utf8_lossy(&buf).contains("/slow") {
                        let _ = sock
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\n\r\npartial")
                            .await;
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        return;
                    }
                    let hdr = format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n");
                    if sock.write_all(hdr.as_bytes()).await.is_err() {
                        return;
                    }
                    let mut sent = 0usize;
                    while sent < total {
                        let n = (total - sent).min(c.len());
                        if sock.write_all(&c[..n]).await.is_err() {
                            break;
                        }
                        sent += n;
                    }
                });
            }
        }
    });

    let dl_dir = std::env::temp_dir().join(format!("swaggen-bench-{}", std::process::id()));
    let _ = fs::create_dir_all(&dl_dir);

    let stream_req = |url: String| ResolvedRequest {
        method: HttpMethod::Get,
        url: url.clone(),
        masked_url: url,
        headers: Vec::new(),
        masked_headers: Vec::new(),
        body: swaggen_domain::workspace::RequestBody::None,
        masked_body_preview: String::new(),
        auth_source: "None".into(),
        warnings: Vec::new(),
        blocking_errors: Vec::new(),
        requires_production_confirm: false,
        connection: RequestConnectionPolicy::default(),
    };

    for _ in 0..3 {
        let r = execute_request(&stream_req(format!("http://{addr}/20mb")), &dl_dir, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(r.wire_bytes, 20 * 1024 * 1024);
        assert_eq!(r.preview_bytes.len(), PREVIEW_CAP_BYTES);
    }
    let mut stream_samples = Vec::with_capacity(10);
    for _ in 0..10 {
        let t0 = Instant::now();
        let r = execute_request(&stream_req(format!("http://{addr}/20mb")), &dl_dir, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(r.wire_bytes, 20 * 1024 * 1024);
        assert_eq!(r.preview_bytes.len(), PREVIEW_CAP_BYTES);
        stream_samples.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    let stream_p95 = percentile_ms(&mut stream_samples, 0.95);

    // Cancellation latency check
    let mut cancel_samples = Vec::with_capacity(10);
    for _ in 0..10 {
        let tok = CancellationToken::new();
        let tok2 = tok.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(15)).await;
            tok2.cancel();
        });
        let t0 = Instant::now();
        let err = execute_request(&stream_req(format!("http://{addr}/slow")), &dl_dir, tok)
            .await
            .unwrap_err();
        assert_eq!(err.kind.as_str(), "CANCELED");
        cancel_samples.push((t0.elapsed().as_secs_f64() * 1000.0 - 15.0).max(0.0));
    }
    let cancel_p95 = percentile_ms(&mut cancel_samples, 0.95);

    let _ = fs::remove_dir_all(&dl_dir);

    let report = serde_json::json!({
        "profile": "release",
        "target": "x86_64-pc-windows-msvc",
        "workspace_requests": 1000,
        "workspace_payload_bytes": payload.len(),
        "open_search_1k_samples": 40,
        "open_search_1k_p50_ms": p50_1k,
        "open_search_1k_p95_ms": p95_1k,
        "open_search_1k_p99_ms": p99_1k,
        "stream_20mb_samples": 10,
        "stream_20mb_p95_ms": stream_p95,
        "stream_preview_cap_bytes": PREVIEW_CAP_BYTES,
        "cancel_after_signal_p95_ms": cancel_p95
    });
    let out_path = PathBuf::from("reports/benchmarks/p06-engine-release.json");
    if let Some(p) = out_path.parent() {
        let _ = fs::create_dir_all(p);
    }
    fs::write(&out_path, format!("{}\n", serde_json::to_string_pretty(&report).unwrap())).unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
