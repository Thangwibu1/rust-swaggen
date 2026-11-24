use axum::{Router, body::Body, http::header, routing::get};
use std::{
    convert::Infallible,
    fs::OpenOptions,
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub const BODY_SIZE: usize = 20 * 1024 * 1024;
pub const PREVIEW_CAP: usize = 64 * 1024;
const CHUNK: usize = 8192;

#[derive(Debug, Clone)]
pub struct Transfer {
    pub bytes: usize,
    pub preview: String,
    pub path: PathBuf,
    pub milliseconds: u128,
}

/// Controlled local HTTP fixture, never used as a fake remote API response.
pub async fn fixture() -> io::Result<(SocketAddr, tokio::sync::oneshot::Sender<()>)> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let app = Router::new().route(
        "/large",
        get(|| async {
            let chunks = futures_util::stream::iter(
                (0..BODY_SIZE / CHUNK).map(|_| Ok::<_, Infallible>(vec![b'x'; CHUNK])),
            );
            (
                [
                    (header::CONTENT_LENGTH, BODY_SIZE.to_string()),
                    (header::CONTENT_TYPE, "application/octet-stream".into()),
                ],
                Body::from_stream(chunks),
            )
        }),
    );
    let (stop, stopped) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await;
    });
    Ok((address, stop))
}

/// Deliberately fixture-only HTTP/1.1 client, not the production HTTP engine.
/// No TLS, redirects, credentials, cookies or arbitrary URLs accepted.
pub fn download_fixture(address: SocketAddr, directory: &Path) -> io::Result<Transfer> {
    if !address.ip().is_loopback() {
        return Err(io::Error::other("fixture must be loopback"));
    }
    std::fs::create_dir_all(directory)?;
    let path = directory.join(format!(
        "body-{}-{}.bin",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let start = Instant::now();
    let mut socket = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    socket.set_read_timeout(Some(Duration::from_secs(10)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    write!(
        socket,
        "GET /large HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )?;
    let mut reader = BufReader::with_capacity(CHUNK, socket);
    let mut line = String::new();
    reader.by_ref().take(4096).read_line(&mut line)?;
    if !line.starts_with("HTTP/1.1 200 ") {
        return Err(io::Error::other("fixture status mismatch"));
    }
    let mut header_bytes = line.len();
    let mut length = None;
    loop {
        line.clear();
        reader.by_ref().take(4096).read_line(&mut line)?;
        header_bytes += line.len();
        if header_bytes > 16 * 1024 || line.is_empty() {
            return Err(io::Error::other("fixture headers invalid"));
        }
        if line == "\r\n" {
            break;
        }
        if let Some((key, value)) = line.split_once(':')
            && key.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    if length != Some(BODY_SIZE) {
        return Err(io::Error::other("fixture length mismatch"));
    }
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let mut preview = Vec::with_capacity(PREVIEW_CAP);
        let mut buffer = [0u8; CHUNK];
        let mut bytes = 0;
        while bytes < BODY_SIZE {
            let max = CHUNK.min(BODY_SIZE - bytes);
            let n = reader.read(&mut buffer[..max])?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "truncated fixture body",
                ));
            }
            // Deterministic fixture integrity check; not just a byte count.
            if buffer[..n].iter().any(|b| *b != b'x') {
                return Err(io::Error::other("fixture body mismatch"));
            }
            file.write_all(&buffer[..n])?;
            let keep = n.min(PREVIEW_CAP - preview.len());
            preview.extend_from_slice(&buffer[..keep]);
            bytes += n;
        }
        file.sync_all()?;
        Ok(Transfer {
            bytes,
            preview: String::from_utf8(preview).unwrap(),
            path: path.clone(),
            milliseconds: start.elapsed().as_millis(),
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    result
}

pub async fn run(directory: PathBuf) -> Result<Transfer, String> {
    let (address, stop) = fixture().await.map_err(|e| e.to_string())?;
    let result = tokio::task::spawn_blocking(move || download_fixture(address, &directory))
        .await
        .map_err(|e| e.to_string());
    let _ = stop.send(());
    result?.map_err(|e| e.to_string())
}
