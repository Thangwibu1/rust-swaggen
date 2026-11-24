use swaggen_p00_native::transport::{self, BODY_SIZE, PREVIEW_CAP};
#[tokio::main]
async fn main() {
    let path = std::env::current_dir().unwrap().join(".local/p00-http");
    match transport::run(path).await {
        Ok(result) => {
            assert_eq!(result.bytes, BODY_SIZE);
            assert_eq!(result.preview.len(), PREVIEW_CAP);
            assert_eq!(
                std::fs::metadata(&result.path).unwrap().len(),
                BODY_SIZE as u64
            );
            println!(
                "PASS: real loopback HTTP, {} bytes verified and synced, preview={} bytes, elapsed={}ms (single fixture sample, not a UI benchmark)",
                result.bytes,
                result.preview.len(),
                result.milliseconds
            );
            std::fs::remove_file(result.path).unwrap();
        }
        Err(error) => {
            eprintln!("FAIL: {error}");
            std::process::exit(2);
        }
    }
}
