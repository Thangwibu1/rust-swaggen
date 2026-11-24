use std::{
    io::{self, Write},
    path::PathBuf,
    process::{Command, Stdio},
};
use swaggen_p00::journal::{Batch, Journal};

fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() == 3 && args[1] == "child" {
        let (mut journal, _) = Journal::open(&PathBuf::from(&args[2]))?;
        journal.append(&Batch {
            draft: b"durable draft".to_vec(),
            outbox: b"same durable intent".to_vec(),
        })?;
        println!("DURABLE");
        io::stdout().flush()?;
        // Parent forcibly terminates this process after observing post-fsync ACK.
        loop {
            std::thread::park();
        }
    }
    let path = std::env::temp_dir().join(format!(
        "swaggen-process-crash-{}.journal",
        std::process::id()
    ));
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "fixture already exists",
        ));
    }
    let mut child = Command::new(std::env::current_exe()?)
        .arg("child")
        .arg(&path)
        .stdout(Stdio::piped())
        .spawn()?;
    use std::io::BufRead;
    let mut line = String::new();
    io::BufReader::new(child.stdout.take().unwrap()).read_line(&mut line)?;
    assert_eq!(line.trim(), "DURABLE");
    assert!(
        Journal::open(&path).is_err(),
        "another process must not acquire the writer fence"
    );
    child.kill()?;
    let status = child.wait()?;
    assert!(!status.success());
    let (j, recovered) = Journal::open(&path)?;
    assert_eq!(
        recovered,
        vec![Batch {
            draft: b"durable draft".to_vec(),
            outbox: b"same durable intent".to_vec()
        }]
    );
    drop(j);
    std::fs::remove_file(path)?;
    println!(
        "PASS: acknowledged draft+outbox survived forced process termination (not a power-loss test)"
    );
    Ok(())
}
