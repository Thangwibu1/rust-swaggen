//! Read back the explicitly named P00 UI fixture, without printing document contents.
use swaggen_p00::journal::Journal;

fn main() -> std::io::Result<()> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.local/p00-native/editor.journal");
    if !path.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Save the P00 native fixture before this readback",
        ));
    }
    let (_journal, batches) = Journal::open(&path)?;
    let last = batches.last().expect("saved fixture must have a batch");
    let text = std::str::from_utf8(&last.draft).expect("fixture draft is UTF-8");
    assert!(text.contains("Swaggen P00"));
    assert!(
        last.outbox.is_empty(),
        "local autosave must not imply publish intent"
    );
    println!(
        "PASS: native UI saved fixture reopened with {} UTF-8 draft bytes and no publish intent",
        last.draft.len()
    );
    Ok(())
}
