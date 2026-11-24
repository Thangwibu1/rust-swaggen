//! Disposable process probe for the product adapter, not a user-data tool.
use std::{
    io::{BufRead, Write},
    process::{Command, Stdio},
};
use swaggen_domain::LocalId;
use swaggen_local_store::{Store, WorkspaceDirectory};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 3 && args[1] == "child" {
        let dir =
            WorkspaceDirectory::open(std::path::Path::new(&args[2]), LocalId::parse("personal")?)?;
        let mut store = Store::open(dir)?;
        store.save("Tiếng Việt — acknowledged before crash".into())?;
        println!("DURABLE");
        std::io::stdout().flush()?;
        loop {
            std::thread::park();
        }
    }
    let root = std::env::temp_dir().join(format!(
        "swaggen-p01-process-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    let mut child = Command::new(std::env::current_exe()?)
        .arg("child")
        .arg(&root)
        .stdout(Stdio::piped())
        .spawn()?;
    let mut line = String::new();
    std::io::BufReader::new(child.stdout.take().unwrap()).read_line(&mut line)?;
    assert_eq!(line.trim(), "DURABLE");
    child.kill()?;
    child.wait()?;
    let dir = WorkspaceDirectory::open(&root, LocalId::parse("personal")?)?;
    let store = Store::open(dir)?;
    assert_eq!(
        store.snapshot().draft,
        "Tiếng Việt — acknowledged before crash"
    );
    assert_eq!(store.snapshot().revision, 1);
    drop(store);
    std::fs::remove_dir_all(root)?;
    println!(
        "PASS: product workspace adapter recovered acknowledged revision after forced process termination"
    );
    Ok(())
}
